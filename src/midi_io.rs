//! Simultaneous multi-device MIDI controller manager, LED state synchronization,
//! and USB port-independent hot-plug persistence (`src/midi_io.rs`).
//!
//! Coordinates multiple hardware MIDI controllers concurrently (e.g., multiple Korg
//! `nanoKONTROL2` units, Akai `MIDI Mix`, Behringer `UMC1820`, generic controllers):
//! - Remembers each device's [`HardwareFingerprint`], assigned preset ID, role, and
//!   per-button LED states in a serializable [`PersistentHardwareConfig`].
//! - Detects hot-plug connection, removal, and USB port migration (e.g., ALSA client
//!   `24:0` changing to `28:0` and `/dev/snd/by-path` changing when plugged into a
//!   different physical USB socket) and automatically reconnects, restores the
//!   assigned preset/role, and generates the hardware MIDI messages to resynchronize
//!   all physical LEDs.

use crate::devices::{get_device_definition, MidiMessageType};
use crate::hardware::{
    discover_connected_midi_hardware, DiscoveredMidiDevice, HardwareFingerprint,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Raw 3-byte MIDI channel voice message (`status`, `data1`, `data2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMidiPacket {
    /// Status byte (`0xB0..=0xBF` for CC, `0x90..=0x9F` for Note On, `0x80..=0x8F` for Note Off).
    pub status: u8,
    /// Controller number or Note number (`0..=127`).
    pub data1: u8,
    /// Value or velocity (`0..=127`).
    pub data2: u8,
}

impl RawMidiPacket {
    /// Encodes a 1-indexed channel (`1..=16`) Control Change message.
    #[must_use]
    pub const fn control_change(channel: u8, cc: u8, value: u8) -> Self {
        let ch = (channel.saturating_sub(1)) & 0x0F;
        Self {
            status: 0xB0 | ch,
            data1: cc & 0x7F,
            data2: value & 0x7F,
        }
    }

    /// Encodes a 1-indexed channel (`1..=16`) Note On message.
    #[must_use]
    pub const fn note_on(channel: u8, note: u8, velocity: u8) -> Self {
        let ch = (channel.saturating_sub(1)) & 0x0F;
        Self {
            status: 0x90 | ch,
            data1: note & 0x7F,
            data2: velocity & 0x7F,
        }
    }

    /// Encodes a 1-indexed channel (`1..=16`) Note Off message.
    #[must_use]
    pub const fn note_off(channel: u8, note: u8, velocity: u8) -> Self {
        let ch = (channel.saturating_sub(1)) & 0x0F;
        Self {
            status: 0x80 | ch,
            data1: note & 0x7F,
            data2: velocity & 0x7F,
        }
    }

    /// Decodes this packet into `(message_type, channel_1_indexed, midi_number, value)`.
    /// Note Off (`0x80..=0x8F`) is normalized to `(MidiMessageType::Note, channel, note, 0)`.
    #[must_use]
    pub const fn decode(self) -> Option<(MidiMessageType, u8, u8, u8)> {
        let status_nibble = self.status & 0xF0;
        let channel = (self.status & 0x0F) + 1;
        let number = self.data1 & 0x7F;
        let value = self.data2 & 0x7F;
        match status_nibble {
            0xB0 => Some((MidiMessageType::Cc, channel, number, value)),
            0x90 => Some((MidiMessageType::Note, channel, number, value)),
            0x80 => Some((MidiMessageType::Note, channel, number, 0)),
            _ => None,
        }
    }

    /// Returns the 3-byte array `[status, data1, data2]`.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 3] {
        [self.status, self.data1, self.data2]
    }
}

/// Role assigned to a managed hardware MIDI device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeviceRole {
    /// Primary CC/Note remapping controller and `8opPerc` performance surface.
    #[default]
    ControllerAndOp8Cockpit,
    /// Dedicated CC/Note remapping controller.
    RemapperOnly,
    /// External hardware synthesizer / drum machine target (e.g. Nord Drum 2 via UMC1820).
    SynthTarget,
    /// Auxiliary / monitor MIDI interface.
    Auxiliary,
}

/// Persistent profile stored on disk for a remembered hardware device, keyed by its
/// USB-port-independent [`HardwareFingerprint`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RememberedDeviceProfile {
    /// Stable hardware fingerprint (`vendor_id:product_id:serial:card_name`).
    pub fingerprint: HardwareFingerprint,
    /// Matched `DeviceDefinition` ID (e.g. `Some("nanokontrol2")`, `Some("akai-midimix")`).
    pub definition_id: Option<String>,
    /// User-facing nickname or display name.
    pub display_name: String,
    /// Assigned functional role in `nkEditor4`.
    pub role: DeviceRole,
    /// Assigned preset ID or name to restore on connection.
    pub assigned_preset: String,
    /// Whether to automatically connect and sync LEDs when this fingerprint appears.
    pub auto_connect: bool,
    /// Last known ALSA Sequencer address (`"24:0"`), updated on every hot-plug.
    pub last_alsa_seq_addr: Option<String>,
    /// Last known ALSA RawMIDI device (`"hw:2,0,0"`), updated on every hot-plug.
    pub last_rawmidi_hw_id: Option<String>,
    /// Last known physical USB path (`/dev/snd/by-path/...`), updated on every hot-plug.
    pub last_by_path_symlink: Option<String>,
    /// Remembered button LED states (`button_name -> is_on`) to resynchronize on hot-plug.
    pub led_states: BTreeMap<String, bool>,
}

/// Serializable configuration file structure storing all remembered hardware profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PersistentHardwareConfig {
    /// Profiles keyed by `fingerprint.canonical_key()`.
    pub profiles: BTreeMap<String, RememberedDeviceProfile>,
}

/// Live connection state of a managed device inside [`MultiDeviceManager`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceConnectionState {
    Connected,
    Disconnected,
}

/// Runtime instance of a managed MIDI controller inside [`MultiDeviceManager`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedDeviceSession {
    /// Stable hardware identity.
    pub fingerprint: HardwareFingerprint,
    /// Device definition ID (`Some("nanokontrol2")`, `Some("akai-midimix")`, or `None`).
    pub definition_id: Option<String>,
    /// Display name.
    pub display_name: String,
    /// Connection state (`Connected` or `Disconnected`).
    pub state: DeviceConnectionState,
    /// Active ALSA Sequencer address (`"24:0"`).
    pub alsa_seq_addr: Option<String>,
    /// Active ALSA RawMIDI device (`"hw:2,0,0"`).
    pub rawmidi_hw_id: Option<String>,
    /// Active `/dev/snd/by-id/` symlink.
    pub by_id_symlink: Option<String>,
    /// Active `/dev/snd/by-path/` symlink.
    pub by_path_symlink: Option<String>,
    /// Assigned role.
    pub role: DeviceRole,
    /// Currently loaded preset ID/name for this controller.
    pub active_preset: String,
    /// Current continuous and button control values (`"track1.knob" -> 64`).
    pub control_values: BTreeMap<String, u8>,
    /// Current hardware LED states (`"track1.solo" -> true`).
    pub led_states: BTreeMap<String, bool>,
    /// Number of times this device has automatically reconnected during the session.
    pub reconnect_count: u32,
}

impl ManagedDeviceSession {
    /// Builds the sequence of 3-byte MIDI packets required to synchronize all physical
    /// LEDs on this controller with its stored `led_states`.
    #[must_use]
    pub fn build_led_sync_packets(&self) -> Vec<RawMidiPacket> {
        let Some(ref def_id) = self.definition_id else {
            return Vec::new();
        };
        let Some(def) = get_device_definition(def_id) else {
            return Vec::new();
        };

        let mut packets = Vec::with_capacity(def.leds.len());
        for led in &def.leds {
            let is_on = self
                .led_states
                .get(&led.button_name)
                .copied()
                .unwrap_or(false);
            let value = if is_on {
                def.led_protocol.on_value
            } else {
                def.led_protocol.off_value
            };
            let pkt = match def.led_protocol.message_type {
                MidiMessageType::Cc => {
                    RawMidiPacket::control_change(def.led_protocol.channel, led.midi_number, value)
                }
                MidiMessageType::Note => {
                    RawMidiPacket::note_on(def.led_protocol.channel, led.midi_number, value)
                }
            };
            packets.push(pkt);
        }
        packets
    }
}

/// Event emitted by [`MultiDeviceManager::reconcile_discovered_devices`] during hot-plug polling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HotplugEvent {
    /// A brand-new controller was connected and registered.
    ConnectedNew {
        fingerprint: HardwareFingerprint,
        alsa_seq_addr: Option<String>,
        assigned_preset: String,
    },
    /// A previously remembered controller reconnected (potentially on a different USB port / ALSA client ID)
    /// and had its preset and LEDs automatically restored.
    ReconnectedRemembered {
        fingerprint: HardwareFingerprint,
        previous_alsa_seq_addr: Option<String>,
        new_alsa_seq_addr: Option<String>,
        previous_by_path: Option<String>,
        new_by_path: Option<String>,
        restored_preset: String,
        led_sync_packet_count: usize,
    },
    /// A connected controller was unplugged.
    Disconnected {
        fingerprint: HardwareFingerprint,
        last_alsa_seq_addr: Option<String>,
    },
}

/// Simultaneous multi-device MIDI controller manager with USB port-independent identity persistence.
#[derive(Debug, Clone, Default)]
pub struct MultiDeviceManager {
    /// Active and remembered sessions keyed by canonical fingerprint string.
    pub sessions: BTreeMap<String, ManagedDeviceSession>,
    /// Persistent hardware configuration.
    pub persistent_config: PersistentHardwareConfig,
    /// Outbox of queued MIDI packets per canonical fingerprint key `(key, packet)`.
    pub outbound_midi_queue: Vec<(String, RawMidiPacket)>,
}

impl MultiDeviceManager {
    /// Creates a new [`MultiDeviceManager`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a manager pre-populated from a [`PersistentHardwareConfig`].
    #[must_use]
    pub fn from_persistent_config(config: PersistentHardwareConfig) -> Self {
        let mut mgr = Self {
            sessions: BTreeMap::new(),
            persistent_config: config,
            outbound_midi_queue: Vec::new(),
        };
        for (key, prof) in &mgr.persistent_config.profiles {
            mgr.sessions.insert(
                key.clone(),
                ManagedDeviceSession {
                    fingerprint: prof.fingerprint.clone(),
                    definition_id: prof.definition_id.clone(),
                    display_name: prof.display_name.clone(),
                    state: DeviceConnectionState::Disconnected,
                    alsa_seq_addr: prof.last_alsa_seq_addr.clone(),
                    rawmidi_hw_id: prof.last_rawmidi_hw_id.clone(),
                    by_id_symlink: None,
                    by_path_symlink: prof.last_by_path_symlink.clone(),
                    role: prof.role.clone(),
                    active_preset: prof.assigned_preset.clone(),
                    control_values: BTreeMap::new(),
                    led_states: prof.led_states.clone(),
                    reconnect_count: 0,
                },
            );
        }
        mgr
    }

    /// Saves the persistent hardware identity & preset configuration to a JSON file.
    pub fn save_config_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json =
            serde_json::to_string_pretty(&self.persistent_config).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Loads the persistent hardware identity & preset configuration from a JSON file.
    pub fn load_config_from_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let config: PersistentHardwareConfig =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(Self::from_persistent_config(config))
    }

    /// Finds an existing session key whose [`HardwareFingerprint`] matches `fp` across USB re-plugs.
    #[must_use]
    pub fn find_matching_session_key(&self, fp: &HardwareFingerprint) -> Option<String> {
        let exact = fp.canonical_key();
        if self.sessions.contains_key(&exact) {
            return Some(exact);
        }
        self.sessions
            .iter()
            .find(|(_, s)| s.fingerprint.matches_replugged(fp))
            .map(|(k, _)| k.clone())
    }

    /// Assigns a preset name/ID and role to a device by its [`HardwareFingerprint`] and persists it.
    pub fn assign_preset_and_role(
        &mut self,
        fp: &HardwareFingerprint,
        preset: impl Into<String>,
        role: DeviceRole,
    ) {
        let preset_str = preset.into();
        if let Some(key) = self.find_matching_session_key(fp) {
            if let Some(session) = self.sessions.get_mut(&key) {
                session.active_preset = preset_str.clone();
                session.role = role.clone();
            }
            if let Some(prof) = self.persistent_config.profiles.get_mut(&key) {
                prof.assigned_preset = preset_str;
                prof.role = role;
            }
        }
    }

    /// Sets a button LED state (`true` = ON, `false` = OFF) on a device and queues the
    /// hardware MIDI message if the device is currently connected.
    pub fn set_led_state(&mut self, fp: &HardwareFingerprint, button_name: &str, is_on: bool) {
        let Some(key) = self.find_matching_session_key(fp) else {
            return;
        };
        if let Some(session) = self.sessions.get_mut(&key) {
            session.led_states.insert(button_name.to_string(), is_on);
            if let Some(prof) = self.persistent_config.profiles.get_mut(&key) {
                prof.led_states.insert(button_name.to_string(), is_on);
            }
            if session.state == DeviceConnectionState::Connected {
                if let Some(ref def_id) = session.definition_id {
                    if let Some(def) = get_device_definition(def_id) {
                        if let Some(led) = def.leds.iter().find(|l| l.button_name == button_name) {
                            let val = if is_on {
                                def.led_protocol.on_value
                            } else {
                                def.led_protocol.off_value
                            };
                            let pkt = match def.led_protocol.message_type {
                                MidiMessageType::Cc => RawMidiPacket::control_change(
                                    def.led_protocol.channel,
                                    led.midi_number,
                                    val,
                                ),
                                MidiMessageType::Note => RawMidiPacket::note_on(
                                    def.led_protocol.channel,
                                    led.midi_number,
                                    val,
                                ),
                            };
                            self.outbound_midi_queue.push((key, pkt));
                        }
                    }
                }
            }
        }
    }

    /// Reconciles a snapshot of discovered hardware devices against current sessions and
    /// remembered profiles. Handles simultaneous multi-device connections, hot-plug removals,
    /// and port-independent reconnections with automatic LED resynchronization.
    pub fn reconcile_discovered_devices(
        &mut self,
        discovered: &[DiscoveredMidiDevice],
    ) -> Vec<HotplugEvent> {
        let mut events = Vec::new();
        let mut seen_keys = Vec::new();

        for dev in discovered {
            if let Some(existing_key) = self.find_matching_session_key(&dev.fingerprint) {
                seen_keys.push(existing_key.clone());
                let session = self.sessions.get_mut(&existing_key).unwrap();
                let was_disconnected = session.state == DeviceConnectionState::Disconnected;
                let addr_changed = session.alsa_seq_addr != dev.alsa_seq_addr;
                let path_changed = session.by_path_symlink != dev.by_path_symlink;

                let prev_addr = session.alsa_seq_addr.clone();
                let prev_path = session.by_path_symlink.clone();

                session.state = DeviceConnectionState::Connected;
                session.alsa_seq_addr = dev.alsa_seq_addr.clone();
                session.rawmidi_hw_id = dev.rawmidi_hw_id.clone();
                session.by_id_symlink = dev.by_id_symlink.clone();
                session.by_path_symlink = dev.by_path_symlink.clone();

                if let Some(prof) = self.persistent_config.profiles.get_mut(&existing_key) {
                    prof.last_alsa_seq_addr = dev.alsa_seq_addr.clone();
                    prof.last_rawmidi_hw_id = dev.rawmidi_hw_id.clone();
                    prof.last_by_path_symlink = dev.by_path_symlink.clone();
                }

                if was_disconnected || addr_changed || path_changed {
                    session.reconnect_count += 1;
                    let sync_packets = session.build_led_sync_packets();
                    let packet_count = sync_packets.len();
                    for pkt in sync_packets {
                        self.outbound_midi_queue.push((existing_key.clone(), pkt));
                    }
                    events.push(HotplugEvent::ReconnectedRemembered {
                        fingerprint: session.fingerprint.clone(),
                        previous_alsa_seq_addr: prev_addr,
                        new_alsa_seq_addr: session.alsa_seq_addr.clone(),
                        previous_by_path: prev_path,
                        new_by_path: session.by_path_symlink.clone(),
                        restored_preset: session.active_preset.clone(),
                        led_sync_packet_count: packet_count,
                    });
                }
            } else {
                // Brand new device discovered
                let key = dev.fingerprint.canonical_key();
                seen_keys.push(key.clone());

                let default_preset = match dev.definition_id.as_deref() {
                    Some("nanokontrol2") => "Default".to_string(),
                    Some("akai-midimix") => "Default MidiMix".to_string(),
                    _ => "Generic Pass-Through".to_string(),
                };
                let default_role = if dev.definition_id.is_some() {
                    DeviceRole::ControllerAndOp8Cockpit
                } else {
                    DeviceRole::Auxiliary
                };

                let mut initial_leds = BTreeMap::new();
                if let Some(ref def_id) = dev.definition_id {
                    if let Some(def) = get_device_definition(def_id) {
                        for led in &def.leds {
                            initial_leds.insert(led.button_name.clone(), false);
                        }
                    }
                }

                let session = ManagedDeviceSession {
                    fingerprint: dev.fingerprint.clone(),
                    definition_id: dev.definition_id.clone(),
                    display_name: dev.display_name.clone(),
                    state: DeviceConnectionState::Connected,
                    alsa_seq_addr: dev.alsa_seq_addr.clone(),
                    rawmidi_hw_id: dev.rawmidi_hw_id.clone(),
                    by_id_symlink: dev.by_id_symlink.clone(),
                    by_path_symlink: dev.by_path_symlink.clone(),
                    role: default_role.clone(),
                    active_preset: default_preset.clone(),
                    control_values: BTreeMap::new(),
                    led_states: initial_leds.clone(),
                    reconnect_count: 0,
                };

                let sync_packets = session.build_led_sync_packets();
                for pkt in sync_packets {
                    self.outbound_midi_queue.push((key.clone(), pkt));
                }

                self.persistent_config.profiles.insert(
                    key.clone(),
                    RememberedDeviceProfile {
                        fingerprint: dev.fingerprint.clone(),
                        definition_id: dev.definition_id.clone(),
                        display_name: dev.display_name.clone(),
                        role: default_role,
                        assigned_preset: default_preset.clone(),
                        auto_connect: true,
                        last_alsa_seq_addr: dev.alsa_seq_addr.clone(),
                        last_rawmidi_hw_id: dev.rawmidi_hw_id.clone(),
                        last_by_path_symlink: dev.by_path_symlink.clone(),
                        led_states: initial_leds,
                    },
                );

                self.sessions.insert(key, session);
                events.push(HotplugEvent::ConnectedNew {
                    fingerprint: dev.fingerprint.clone(),
                    alsa_seq_addr: dev.alsa_seq_addr.clone(),
                    assigned_preset: default_preset,
                });
            }
        }

        // Mark any previously connected sessions not present in `discovered` as Disconnected
        let all_keys: Vec<String> = self.sessions.keys().cloned().collect();
        for key in all_keys {
            if !seen_keys.contains(&key) {
                if let Some(session) = self.sessions.get_mut(&key) {
                    if session.state == DeviceConnectionState::Connected {
                        session.state = DeviceConnectionState::Disconnected;
                        events.push(HotplugEvent::Disconnected {
                            fingerprint: session.fingerprint.clone(),
                            last_alsa_seq_addr: session.alsa_seq_addr.clone(),
                        });
                    }
                }
            }
        }

        events
    }

    /// Scans the live Linux host hardware and reconciles all connected MIDI devices.
    pub fn poll_host_hardware(&mut self) -> Vec<HotplugEvent> {
        let discovered = discover_connected_midi_hardware();
        self.reconcile_discovered_devices(&discovered)
    }

    /// Returns all currently connected device sessions.
    #[must_use]
    pub fn connected_sessions(&self) -> Vec<&ManagedDeviceSession> {
        self.sessions
            .values()
            .filter(|s| s.state == DeviceConnectionState::Connected)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_nk2_on_port(
        card_idx: u32,
        client_id: u32,
        usb_path: &str,
    ) -> DiscoveredMidiDevice {
        DiscoveredMidiDevice {
            fingerprint: HardwareFingerprint::new("0944", "0117", "", "nanoKONTROL2"),
            definition_id: Some("nanokontrol2".to_string()),
            display_name: "nanoKONTROL2".to_string(),
            manufacturer: "Korg".to_string(),
            card_index: Some(card_idx),
            alsa_client_id: Some(client_id),
            alsa_seq_addr: Some(format!("{client_id}:0")),
            alsa_port_name: Some("nanoKONTROL2 _ CTRL".to_string()),
            rawmidi_hw_id: Some(format!("hw:{card_idx},0,0")),
            rawmidi_name: Some("nanoKONTROL2 _ SLIDER/KNOB".to_string()),
            by_id_symlink: Some("usb-KORG_INC._nanoKONTROL2-00".to_string()),
            by_path_symlink: Some(usb_path.to_string()),
        }
    }

    fn mock_midimix_on_port(
        card_idx: u32,
        client_id: u32,
        usb_path: &str,
    ) -> DiscoveredMidiDevice {
        DiscoveredMidiDevice {
            fingerprint: HardwareFingerprint::new("09e8", "0031", "", "MIDIMix"),
            definition_id: Some("akai-midimix".to_string()),
            display_name: "MIDI Mix".to_string(),
            manufacturer: "Akai".to_string(),
            card_index: Some(card_idx),
            alsa_client_id: Some(client_id),
            alsa_seq_addr: Some(format!("{client_id}:0")),
            alsa_port_name: Some("MIDI Mix MIDI 1".to_string()),
            rawmidi_hw_id: Some(format!("hw:{card_idx},0,0")),
            rawmidi_name: Some("MIDI Mix MIDI 1".to_string()),
            by_id_symlink: Some("usb-AKAI_professional_MIDI_Mix-00".to_string()),
            by_path_symlink: Some(usb_path.to_string()),
        }
    }

    #[test]
    fn simultaneous_multi_device_connection_and_usb_port_migration_persistence() {
        let mut mgr = MultiDeviceManager::new();

        // Step 1: Connect Korg nanoKONTROL2 (on card 2, ALSA 24:0, USB port 1-5.3)
        // and Akai MIDI Mix (on card 3, ALSA 28:0, USB port 1-6.1) simultaneously
        let nk2_initial = mock_nk2_on_port(2, 24, "pci-0000:00:14.0-usb-0:5.3:1.0");
        let midimix_initial = mock_midimix_on_port(3, 28, "pci-0000:00:14.0-usb-0:6.1:1.0");

        let events1 = mgr.reconcile_discovered_devices(&[nk2_initial.clone(), midimix_initial]);
        assert_eq!(events1.len(), 2);
        assert_eq!(mgr.connected_sessions().len(), 2);

        // Assign custom preset "Nord Drum 2" and turn on Track 1 Solo + Mute LEDs on nanoKONTROL2
        mgr.assign_preset_and_role(
            &nk2_initial.fingerprint,
            "Nord Drum 2",
            DeviceRole::ControllerAndOp8Cockpit,
        );
        mgr.set_led_state(&nk2_initial.fingerprint, "track1.solo", true);
        mgr.set_led_state(&nk2_initial.fingerprint, "track1.mute", true);
        mgr.outbound_midi_queue.clear();

        // Step 2: Unplug Korg nanoKONTROL2 while Akai MIDI Mix remains connected
        let midimix_still = mock_midimix_on_port(3, 28, "pci-0000:00:14.0-usb-0:6.1:1.0");
        let events2 = mgr.reconcile_discovered_devices(&[midimix_still.clone()]);
        assert_eq!(events2.len(), 1);
        assert!(matches!(
            &events2[0],
            HotplugEvent::Disconnected {
                fingerprint,
                last_alsa_seq_addr: Some(addr),
            } if fingerprint.usb_id() == "0944:0117" && addr == "24:0"
        ));
        assert_eq!(mgr.connected_sessions().len(), 1);

        // Step 3: Plug Korg nanoKONTROL2 into a DIFFERENT physical USB socket!
        // Its ALSA card changes from 2 -> 4, ALSA client changes from 24:0 -> 32:0,
        // and USB bus path changes from usb-0:5.3:1.0 -> usb-0:2.4:1.0.
        let nk2_replugged = mock_nk2_on_port(4, 32, "pci-0000:00:14.0-usb-0:2.4:1.0");
        let events3 = mgr.reconcile_discovered_devices(&[midimix_still, nk2_replugged]);
        assert_eq!(events3.len(), 1);

        match &events3[0] {
            HotplugEvent::ReconnectedRemembered {
                fingerprint,
                previous_alsa_seq_addr,
                new_alsa_seq_addr,
                previous_by_path,
                new_by_path,
                restored_preset,
                led_sync_packet_count,
            } => {
                assert_eq!(fingerprint.canonical_key(), "0944:0117::nanoKONTROL2");
                assert_eq!(previous_alsa_seq_addr.as_deref(), Some("24:0"));
                assert_eq!(new_alsa_seq_addr.as_deref(), Some("32:0"));
                assert_eq!(
                    previous_by_path.as_deref(),
                    Some("pci-0000:00:14.0-usb-0:5.3:1.0")
                );
                assert_eq!(
                    new_by_path.as_deref(),
                    Some("pci-0000:00:14.0-usb-0:2.4:1.0")
                );
                assert_eq!(restored_preset, "Nord Drum 2");
                assert_eq!(*led_sync_packet_count, 28); // All 28 nanoKONTROL2 LEDs resynchronized!
            }
            other => panic!("Expected ReconnectedRemembered, got {other:?}"),
        }

        // Verify the 28 queued LED sync packets include Track 1 Solo (CC 32 = 127) and Track 1 Mute (CC 48 = 127)
        assert_eq!(mgr.outbound_midi_queue.len(), 28);
        assert!(mgr
            .outbound_midi_queue
            .iter()
            .any(|(_, p)| *p == RawMidiPacket::control_change(1, 32, 127)));
        assert!(mgr
            .outbound_midi_queue
            .iter()
            .any(|(_, p)| *p == RawMidiPacket::control_change(1, 48, 127)));
    }

    #[test]
    fn persistent_config_serialization_and_reload_preserves_identity_and_leds() {
        let mut mgr = MultiDeviceManager::new();
        let nk2 = mock_nk2_on_port(2, 24, "pci-0000:00:14.0-usb-0:5.3:1.0");
        mgr.reconcile_discovered_devices(&[nk2.clone()]);
        mgr.assign_preset_and_role(
            &nk2.fingerprint,
            "8opPerc Live Rig",
            DeviceRole::ControllerAndOp8Cockpit,
        );
        mgr.set_led_state(&nk2.fingerprint, "transport.play", true);

        let json = serde_json::to_string_pretty(&mgr.persistent_config).unwrap();
        let decoded: PersistentHardwareConfig = serde_json::from_str(&json).unwrap();
        let mut reloaded = MultiDeviceManager::from_persistent_config(decoded);

        // When the controller appears after app restart on a new ALSA port 28:0, it restores "8opPerc Live Rig"
        let nk2_after_reboot = mock_nk2_on_port(3, 28, "pci-0000:00:14.0-usb-0:1.2:1.0");
        let events = reloaded.reconcile_discovered_devices(&[nk2_after_reboot]);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            HotplugEvent::ReconnectedRemembered {
                restored_preset,
                led_sync_packet_count: 28,
                ..
            } if restored_preset == "8opPerc Live Rig"
        ));
    }

    #[test]
    fn poll_host_hardware_discovers_connected_devices_safely() {
        let mut mgr = MultiDeviceManager::new();
        let events = mgr.poll_host_hardware();
        assert_eq!(events.len(), mgr.connected_sessions().len());
    }
}
