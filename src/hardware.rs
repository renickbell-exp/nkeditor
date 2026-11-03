//! Linux MIDI/USB hardware discovery and port-independent hardware fingerprinting (`src/hardware.rs`).
//!
//! Inspects:
//! - `/sys/class/sound/card*/` (`id`, `number`, `device/../idVendor`, `device/../idProduct`,
//!   `device/../serial`, `device/../manufacturer`, `device/../product`)
//! - `/dev/snd/by-id/` (stable USB identity symlinks, e.g. `usb-KORG_INC._nanoKONTROL2-00 -> ../controlC2`)
//! - `/dev/snd/by-path/` (physical USB port topology symlinks, e.g. `pci-0000:00:14.0-usb-0:5.3:1.0 -> ../controlC2`)
//! - `/proc/asound/seq/clients` (ALSA sequencer client and port table)
//! - Command fallbacks (`aconnect -l` and `amidi -l`)
//!
//! Computes a stable [`HardwareFingerprint`] (`vendor_id:product_id:serial:card_name`, such as
//! `0944:0117::nanoKONTROL2` for Korg nanoKONTROL2 or `1397:0503:B8AAF8D3:UMC1820` for Behringer UMC1820)
//! that remains invariant even when a controller is unplugged and re-plugged into a different
//! physical USB port where its ALSA client ID (`24:0` -> `28:0`), sound card number (`card2` -> `card3`),
//! or `/dev/snd/by-path` symlink changes.

use crate::devices::{get_all_device_definitions, identify_device, DeviceDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Stable, USB-port-independent hardware identity for a MIDI controller or audio/MIDI interface.
///
/// Canonical string format: `vendor_id:product_id:serial:card_name`
/// (e.g. `0944:0117::nanoKONTROL2` or `1397:0503:B8AAF8D3:UMC1820`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HardwareFingerprint {
    /// Lowercase 4-digit hex USB vendor ID (e.g. `"0944"` for Korg, `"09e8"` for Akai), or `"0000"` if non-USB.
    pub vendor_id: String,
    /// Lowercase 4-digit hex USB product ID (e.g. `"0117"` for nanoKONTROL2, `"0031"` for MIDI Mix), or `"0000"`.
    pub product_id: String,
    /// Optional USB serial number (e.g. `"B8AAF8D3"`), empty string `""` when the device has no USB serial descriptor.
    pub serial: String,
    /// Stable ALSA card name / model identifier (e.g. `"nanoKONTROL2"`, `"MIDIMix"`, `"UMC1820"`).
    pub card_name: String,
}

impl HardwareFingerprint {
    /// Creates a normalized [`HardwareFingerprint`].
    #[must_use]
    pub fn new(
        vendor_id: impl Into<String>,
        product_id: impl Into<String>,
        serial: impl Into<String>,
        card_name: impl Into<String>,
    ) -> Self {
        let v = vendor_id.into().trim().to_ascii_lowercase();
        let p = product_id.into().trim().to_ascii_lowercase();
        Self {
            vendor_id: if v.is_empty() {
                "0000".to_string()
            } else {
                v
            },
            product_id: if p.is_empty() {
                "0000".to_string()
            } else {
                p
            },
            serial: serial.into().trim().to_string(),
            card_name: card_name.into().trim().to_string(),
        }
    }

    /// Returns the short `vendor_id:product_id` USB ID pair (e.g. `"0944:0117"`).
    #[must_use]
    pub fn usb_id(&self) -> String {
        format!("{}:{}", self.vendor_id, self.product_id)
    }

    /// Returns the canonical `vendor_id:product_id:serial:card_name` key.
    #[must_use]
    pub fn canonical_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.vendor_id, self.product_id, self.serial, self.card_name
        )
    }

    /// Parses a canonical `vendor_id:product_id:serial:card_name` (or 2-part `vendor_id:product_id`) string.
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.trim().splitn(4, ':').collect();
        match parts.as_slice() {
            [v, p, serial, card_name] => Ok(Self::new(*v, *p, *serial, *card_name)),
            [v, p] => Ok(Self::new(*v, *p, "", "")),
            _ => Err(format!(
                "Invalid HardwareFingerprint '{}': expected vendor_id:product_id:serial:card_name",
                s
            )),
        }
    }

    /// Checks whether this fingerprint matches another fingerprint across a USB re-plug.
    ///
    /// Two fingerprints refer to the same physical hardware unit if:
    /// 1. Both have non-zero USB `vendor_id` and `product_id` that match (`0944:0117`), AND
    ///    their `serial` numbers match (or both are empty), OR
    /// 2. Their canonical keys match case-insensitively.
    #[must_use]
    pub fn matches_replugged(&self, other: &Self) -> bool {
        if self.vendor_id != "0000"
            && self.product_id != "0000"
            && self.vendor_id.eq_ignore_ascii_case(&other.vendor_id)
            && self.product_id.eq_ignore_ascii_case(&other.product_id)
        {
            if !self.serial.is_empty() && !other.serial.is_empty() {
                return self.serial == other.serial;
            }
            return self.card_name.eq_ignore_ascii_case(&other.card_name)
                || self.card_name.is_empty()
                || other.card_name.is_empty();
        }
        self.canonical_key()
            .eq_ignore_ascii_case(&other.canonical_key())
    }
}

impl std::fmt::Display for HardwareFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.canonical_key())
    }
}

/// Information discovered from `/sys/class/sound/cardN/` and `/dev/snd/by-{id,path}/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SysfsSoundCard {
    /// Dynamic ALSA card index (`0`, `1`, `2`, ...).
    pub card_index: u32,
    /// Card ID string from `/sys/class/sound/cardN/id` (e.g. `"nanoKONTROL2"`).
    pub card_id: String,
    /// USB Vendor ID from `device/../idVendor` (e.g. `"0944"`).
    pub vendor_id: Option<String>,
    /// USB Product ID from `device/../idProduct` (e.g. `"0117"`).
    pub product_id: Option<String>,
    /// USB Serial string from `device/../serial` (if present).
    pub serial: Option<String>,
    /// USB Manufacturer string from `device/../manufacturer` (e.g. `"KORG INC."`).
    pub manufacturer: Option<String>,
    /// USB Product string from `device/../product` (e.g. `"nanoKONTROL2"`).
    pub product: Option<String>,
    /// Stable `/dev/snd/by-id/` symlink name (e.g. `"usb-KORG_INC._nanoKONTROL2-00"`).
    pub by_id_symlink: Option<String>,
    /// Physical USB port `/dev/snd/by-path/` symlink name (e.g. `"pci-0000:00:14.0-usb-0:5.3:1.0"`).
    pub by_path_symlink: Option<String>,
    /// Whether this card exposes a `/sys/class/sound/cardN/midi*` or `dmmidi*` device node.
    pub has_midi_device: bool,
}

impl SysfsSoundCard {
    /// Computes the stable [`HardwareFingerprint`] for this sysfs sound card.
    #[must_use]
    pub fn fingerprint(&self) -> HardwareFingerprint {
        HardwareFingerprint::new(
            self.vendor_id.as_deref().unwrap_or("0000"),
            self.product_id.as_deref().unwrap_or("0000"),
            self.serial.as_deref().unwrap_or(""),
            &self.card_id,
        )
    }
}

/// A single ALSA Sequencer port belonging to an [`AlsaSeqClient`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlsaSeqPort {
    /// Port number (`0`, `1`, ...).
    pub port_id: u32,
    /// Port name (e.g. `"nanoKONTROL2 _ CTRL"`).
    pub name: String,
    /// Capability flags string (e.g. `"RWeX"`).
    pub capabilities: String,
}

/// An ALSA Sequencer client parsed from `/proc/asound/seq/clients` or `aconnect -l`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlsaSeqClient {
    /// Dynamic ALSA Sequencer client ID (e.g. `24`).
    pub client_id: u32,
    /// Client name (e.g. `"nanoKONTROL2"`).
    pub name: String,
    /// Client type (`"Kernel Legacy"`, `"kernel"`, `"User Legacy"`, `"user"`).
    pub client_type: String,
    /// Associated ALSA sound card index if known (from `card=N` or sysfs matching).
    pub card_index: Option<u32>,
    /// Ports exposed by this client.
    pub ports: Vec<AlsaSeqPort>,
}

/// An ALSA RawMIDI device entry parsed from `amidi -l` or sysfs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlsaRawMidiEntry {
    /// Direction flags (e.g. `"IO"`).
    pub direction: String,
    /// RawMIDI hardware address (e.g. `"hw:2,0,0"`).
    pub hw_id: String,
    /// Card index extracted from `hw:C,D,S` (e.g. `2`).
    pub card_index: Option<u32>,
    /// Human-readable RawMIDI device name (e.g. `"nanoKONTROL2 _ SLIDER/KNOB"`).
    pub name: String,
}

/// Unified descriptor of a discovered physical or virtual MIDI device on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredMidiDevice {
    /// Stable, USB-port-independent hardware fingerprint.
    pub fingerprint: HardwareFingerprint,
    /// Matched `nkEditor` device definition ID (`Some("nanokontrol2")`, `Some("akai-midimix")`, or `None` for generic/synth).
    pub definition_id: Option<String>,
    /// Human-readable display name.
    pub display_name: String,
    /// Manufacturer name.
    pub manufacturer: String,
    /// Current ALSA sound card index (e.g. `Some(2)`), which may change if re-plugged into another USB port.
    pub card_index: Option<u32>,
    /// Current ALSA Sequencer client ID (e.g. `Some(24)`), which may change across hot-plugs.
    pub alsa_client_id: Option<u32>,
    /// Current primary ALSA Sequencer `client:port` address (e.g. `Some("24:0")`).
    pub alsa_seq_addr: Option<String>,
    /// Current ALSA Sequencer port name (e.g. `Some("nanoKONTROL2 _ CTRL")`).
    pub alsa_port_name: Option<String>,
    /// Current ALSA RawMIDI device ID (e.g. `Some("hw:2,0,0")`).
    pub rawmidi_hw_id: Option<String>,
    /// Current RawMIDI device name (e.g. `Some("nanoKONTROL2 _ SLIDER/KNOB")`).
    pub rawmidi_name: Option<String>,
    /// Stable `/dev/snd/by-id/` symlink name (e.g. `Some("usb-KORG_INC._nanoKONTROL2-00")`).
    pub by_id_symlink: Option<String>,
    /// Current physical USB port `/dev/snd/by-path/` symlink name (e.g. `Some("pci-0000:00:14.0-usb-0:5.3:1.0")`).
    pub by_path_symlink: Option<String>,
}

// =============================================================================
// Sysfs, /dev/snd, /proc/asound, and CLI Parsers
// =============================================================================

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Reads a USB attribute file (`idVendor`, `idProduct`, `serial`, `manufacturer`, `product`)
/// from the USB device parent directory (`device/..`) of `/sys/class/sound/cardN`.
///
/// Crucially, we stop at the directory that contains `idVendor` and `idProduct` so we
/// never walk up into the USB host controller (`usb1`, where `serial` is the PCI bus address
/// `0000:00:14.0` rather than the MIDI controller's serial number).
fn read_usb_attr(card_dir: &Path, attr: &str) -> Option<String> {
    let usb_dev_dir = card_dir.join("device").join("..");
    if usb_dev_dir.join("idVendor").exists() && usb_dev_dir.join("idProduct").exists() {
        return read_trimmed(&usb_dev_dir.join(attr));
    }
    let direct_dev_dir = card_dir.join("device");
    if direct_dev_dir.join("idVendor").exists() && direct_dev_dir.join("idProduct").exists() {
        return read_trimmed(&direct_dev_dir.join(attr));
    }
    None
}

/// Scans `/dev/snd/by-id` or `/dev/snd/by-path` and maps `card_index -> symlink_name`.
#[must_use]
pub fn scan_dev_snd_symlinks(dir: &Path) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return map;
    };
    let mut sorted_entries: Vec<_> = entries.flatten().collect();
    sorted_entries.sort_by_key(|e| e.file_name());

    for entry in sorted_entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Ok(target) = fs::read_link(entry.path()) {
            let target_str = target.to_string_lossy();
            // Target typically looks like "../controlC2"
            if let Some(idx_str) = target_str.rsplit("controlC").next() {
                if let Ok(card_idx) = idx_str.trim().parse::<u32>() {
                    map.entry(card_idx).or_insert(name);
                }
            }
        }
    }
    map
}

/// Scans `/sys/class/sound/card*` and pairs with `/dev/snd/by-id` and `/dev/snd/by-path`.
#[must_use]
pub fn scan_sysfs_sound_cards_in(
    sys_sound_dir: &Path,
    by_id_dir: &Path,
    by_path_dir: &Path,
) -> Vec<SysfsSoundCard> {
    let by_id_map = scan_dev_snd_symlinks(by_id_dir);
    let by_path_map = scan_dev_snd_symlinks(by_path_dir);

    let Ok(entries) = fs::read_dir(sys_sound_dir) else {
        return Vec::new();
    };

    let mut cards = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(idx_str) = file_name.strip_prefix("card") else {
            continue;
        };
        let Ok(card_index) = idx_str.parse::<u32>() else {
            continue;
        };

        let card_path = entry.path();
        let card_id = read_trimmed(&card_path.join("id"))
            .unwrap_or_else(|| format!("card{card_index}"));
        let vendor_id = read_usb_attr(&card_path, "idVendor").map(|s| s.to_ascii_lowercase());
        let product_id = read_usb_attr(&card_path, "idProduct").map(|s| s.to_ascii_lowercase());
        let serial = read_usb_attr(&card_path, "serial");
        let manufacturer = read_usb_attr(&card_path, "manufacturer");
        let product = read_usb_attr(&card_path, "product");

        let has_midi_device = fs::read_dir(&card_path)
            .map(|dir_iter| {
                dir_iter.flatten().any(|sub| {
                    let sub_name = sub.file_name().to_string_lossy().to_string();
                    sub_name.starts_with("midi") || sub_name.starts_with("dmmidi")
                })
            })
            .unwrap_or(false);

        cards.push(SysfsSoundCard {
            card_index,
            card_id,
            vendor_id,
            product_id,
            serial,
            manufacturer,
            product,
            by_id_symlink: by_id_map.get(&card_index).cloned(),
            by_path_symlink: by_path_map.get(&card_index).cloned(),
            has_midi_device,
        });
    }

    cards.sort_by_key(|c| c.card_index);
    cards
}

/// Scans the host's `/sys/class/sound`, `/dev/snd/by-id`, and `/dev/snd/by-path`.
#[must_use]
pub fn scan_sysfs_sound_cards() -> Vec<SysfsSoundCard> {
    scan_sysfs_sound_cards_in(
        Path::new("/sys/class/sound"),
        Path::new("/dev/snd/by-id"),
        Path::new("/dev/snd/by-path"),
    )
}

/// Parses the contents of `/proc/asound/seq/clients` into [`AlsaSeqClient`] entries.
#[must_use]
pub fn parse_proc_asound_seq_clients(content: &str) -> Vec<AlsaSeqClient> {
    let mut clients: Vec<AlsaSeqClient> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Client ") {
            // Example: `Client  24 : "nanoKONTROL2" [Kernel Legacy]`
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() == 2 {
                if let Ok(client_id) = parts[0].trim().parse::<u32>() {
                    let rhs = parts[1].trim();
                    let name = rhs
                        .split('"')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let client_type = rhs
                        .rsplit_once('[')
                        .and_then(|(_, t)| t.strip_suffix(']'))
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    clients.push(AlsaSeqClient {
                        client_id,
                        name,
                        client_type,
                        card_index: None,
                        ports: Vec::new(),
                    });
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("Port ") {
            // Example: `Port   0 : "nanoKONTROL2 _ CTRL" (RWeX) [In/Out]`
            if let Some(current_client) = clients.last_mut() {
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                if parts.len() == 2 {
                    if let Ok(port_id) = parts[0].trim().parse::<u32>() {
                        let rhs = parts[1].trim();
                        let name = rhs
                            .split('"')
                            .nth(1)
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        let capabilities = rhs
                            .split('(')
                            .nth(1)
                            .and_then(|s| s.split(')').next())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        current_client.ports.push(AlsaSeqPort {
                            port_id,
                            name,
                            capabilities,
                        });
                    }
                }
            }
        }
    }

    clients
}

/// Parses the output of `aconnect -l` as a fallback or enrichment source (includes `card=N`).
#[must_use]
pub fn parse_aconnect_output(content: &str) -> Vec<AlsaSeqClient> {
    let mut clients: Vec<AlsaSeqClient> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("client ") {
            // Example: `client 24: 'nanoKONTROL2' [type=kernel,card=2]`
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() == 2 {
                if let Ok(client_id) = parts[0].trim().parse::<u32>() {
                    let rhs = parts[1].trim();
                    let name = rhs
                        .split('\'')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let bracket = rhs
                        .rsplit_once('[')
                        .and_then(|(_, b)| b.strip_suffix(']'))
                        .unwrap_or("");
                    let mut client_type = String::new();
                    let mut card_index = None;
                    for kv in bracket.split(',') {
                        if let Some(t) = kv.trim().strip_prefix("type=") {
                            client_type = t.to_string();
                        } else if let Some(c) = kv.trim().strip_prefix("card=") {
                            card_index = c.parse::<u32>().ok();
                        }
                    }
                    clients.push(AlsaSeqClient {
                        client_id,
                        name,
                        client_type,
                        card_index,
                        ports: Vec::new(),
                    });
                }
            }
        } else if !line.starts_with('\t') && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            // Example: `0 'nanoKONTROL2 _ CTRL'`
            if let Some(current_client) = clients.last_mut() {
                let mut parts = trimmed.splitn(2, '\'');
                if let Some(id_str) = parts.next() {
                    if let Ok(port_id) = id_str.trim().parse::<u32>() {
                        let name = parts
                            .next()
                            .and_then(|s| s.split('\'').next())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        current_client.ports.push(AlsaSeqPort {
                            port_id,
                            name,
                            capabilities: "RWeX".to_string(),
                        });
                    }
                }
            }
        }
    }

    clients
}

/// Parses the output of `amidi -l` into [`AlsaRawMidiEntry`] items.
#[must_use]
pub fn parse_amidi_output(content: &str) -> Vec<AlsaRawMidiEntry> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Dir ") {
            continue;
        }
        // Example: `IO  hw:2,0,0  nanoKONTROL2 _ SLIDER/KNOB`
        let mut tokens = trimmed.split_whitespace();
        let Some(direction) = tokens.next() else {
            continue;
        };
        let Some(hw_id) = tokens.next() else {
            continue;
        };
        if !hw_id.starts_with("hw:") {
            continue;
        }
        let name = tokens.collect::<Vec<&str>>().join(" ");
        let card_index = hw_id
            .strip_prefix("hw:")
            .and_then(|s| s.split(',').next())
            .and_then(|c| c.parse::<u32>().ok());
        entries.push(AlsaRawMidiEntry {
            direction: direction.to_string(),
            hw_id: hw_id.to_string(),
            card_index,
            name,
        });
    }
    entries
}

/// Reads ALSA sequencer clients from `/proc/asound/seq/clients`, falling back to `aconnect -l`.
#[must_use]
pub fn scan_alsa_seq_clients() -> Vec<AlsaSeqClient> {
    let proc_clients = fs::read_to_string("/proc/asound/seq/clients")
        .ok()
        .map(|s| parse_proc_asound_seq_clients(&s))
        .unwrap_or_default();

    let aconnect_clients = Command::new("aconnect")
        .arg("-l")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| parse_aconnect_output(&s))
        .unwrap_or_default();

    if proc_clients.is_empty() {
        return aconnect_clients;
    }

    // Enrich proc_clients with `card_index` from `aconnect -l` if available
    let mut merged = proc_clients;
    for client in &mut merged {
        if client.card_index.is_none() {
            if let Some(ac) = aconnect_clients
                .iter()
                .find(|a| a.client_id == client.client_id)
            {
                client.card_index = ac.card_index;
            }
        }
    }
    merged
}

/// Scans RawMIDI devices via `amidi -l`.
#[must_use]
pub fn scan_alsa_rawmidi() -> Vec<AlsaRawMidiEntry> {
    Command::new("amidi")
        .arg("-l")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| parse_amidi_output(&s))
        .unwrap_or_default()
}

/// Matches a card or port against the registered [`DeviceDefinition`] list (`nanokontrol2`, `akai-midimix`)
/// by USB `vendor_id:product_id` first, then by port name pattern.
#[must_use]
pub fn match_registered_definition(
    vendor_id: Option<&str>,
    product_id: Option<&str>,
    names: &[&str],
) -> Option<DeviceDefinition> {
    if let (Some(v), Some(p)) = (vendor_id, product_id) {
        if let (Ok(vid), Ok(pid)) = (u16::from_str_radix(v, 16), u16::from_str_radix(p, 16)) {
            for def in get_all_device_definitions() {
                if def.usb_vendor_id == vid && def.usb_product_id == pid {
                    return Some(def);
                }
            }
        }
    }
    for name in names {
        if let Some(def) = identify_device(name) {
            return Some(def);
        }
        // Also check bare device name match (e.g. "nanoKONTROL2" or "MIDI Mix")
        let lower = name.to_ascii_lowercase();
        if lower.contains("nanokontrol2") {
            return identify_device("nanoKONTROL2:0");
        }
        if lower.contains("midi mix") || lower.contains("midimix") {
            return identify_device("MIDI Mix:0");
        }
    }
    None
}

/// Combines sysfs sound cards, ALSA Sequencer clients, and RawMIDI entries into unified
/// [`DiscoveredMidiDevice`] descriptors with stable [`HardwareFingerprint`]s.
#[must_use]
pub fn correlate_hardware_devices(
    cards: &[SysfsSoundCard],
    seq_clients: &[AlsaSeqClient],
    rawmidi_entries: &[AlsaRawMidiEntry],
) -> Vec<DiscoveredMidiDevice> {
    let mut discovered = Vec::new();
    let mut matched_client_ids = Vec::new();

    for card in cards {
        // Find matching ALSA sequencer client by explicit card_index or by card_id / product name
        let seq_client = seq_clients
            .iter()
            .find(|c| c.card_index == Some(card.card_index))
            .or_else(|| {
                seq_clients.iter().find(|c| {
                    c.client_id >= 16
                        && c.client_id < 128
                        && (c.name.eq_ignore_ascii_case(&card.card_id)
                            || card
                                .product
                                .as_deref()
                                .is_some_and(|p| c.name.eq_ignore_ascii_case(p)))
                })
            });

        let rawmidi = rawmidi_entries
            .iter()
            .find(|r| r.card_index == Some(card.card_index));

        if !card.has_midi_device && seq_client.is_none() && rawmidi.is_none() {
            continue;
        }

        if let Some(sc) = seq_client {
            matched_client_ids.push(sc.client_id);
        }

        let primary_port = seq_client.and_then(|c| c.ports.first());
        let alsa_seq_addr = match (seq_client, primary_port) {
            (Some(c), Some(p)) => Some(format!("{}:{}", c.client_id, p.port_id)),
            _ => None,
        };
        let alsa_port_name = primary_port.map(|p| p.name.clone());

        let mut candidate_names = vec![card.card_id.as_str()];
        if let Some(ref p) = card.product {
            candidate_names.push(p.as_str());
        }
        if let Some(ref pname) = alsa_port_name {
            candidate_names.push(pname.as_str());
        }

        let matched_def = match_registered_definition(
            card.vendor_id.as_deref(),
            card.product_id.as_deref(),
            &candidate_names,
        );

        let display_name = matched_def
            .as_ref()
            .map(|d| d.name.clone())
            .or_else(|| card.product.clone())
            .unwrap_or_else(|| card.card_id.clone());

        let manufacturer = matched_def
            .as_ref()
            .map(|d| d.manufacturer.clone())
            .or_else(|| card.manufacturer.clone())
            .unwrap_or_else(|| "Generic".to_string());

        let rawmidi_hw_id = rawmidi
            .map(|r| r.hw_id.clone())
            .or_else(|| card.has_midi_device.then(|| format!("hw:{},0,0", card.card_index)));

        discovered.push(DiscoveredMidiDevice {
            fingerprint: card.fingerprint(),
            definition_id: matched_def.map(|d| d.id),
            display_name,
            manufacturer,
            card_index: Some(card.card_index),
            alsa_client_id: seq_client.map(|c| c.client_id),
            alsa_seq_addr,
            alsa_port_name,
            rawmidi_hw_id,
            rawmidi_name: rawmidi.map(|r| r.name.clone()),
            by_id_symlink: card.by_id_symlink.clone(),
            by_path_symlink: card.by_path_symlink.clone(),
        });
    }

    // Also include any standalone hardware/virtual ALSA sequencer MIDI clients not backed by a sysfs card
    // (excluding System client 0, Midi Through client 14, and PipeWire system clients)
    for client in seq_clients {
        if matched_client_ids.contains(&client.client_id) {
            continue;
        }
        if client.client_id == 0
            || client.client_id == 14
            || client.name.starts_with("PipeWire-")
        {
            continue;
        }
        let Some(port) = client.ports.first() else {
            continue;
        };
        let matched_def =
            match_registered_definition(None, None, &[client.name.as_str(), port.name.as_str()]);
        let (vid, pid) = matched_def
            .as_ref()
            .map(|d| {
                (
                    format!("{:04x}", d.usb_vendor_id),
                    format!("{:04x}", d.usb_product_id),
                )
            })
            .unwrap_or_else(|| ("0000".to_string(), "0000".to_string()));

        discovered.push(DiscoveredMidiDevice {
            fingerprint: HardwareFingerprint::new(vid, pid, "", &client.name),
            definition_id: matched_def.as_ref().map(|d| d.id.clone()),
            display_name: matched_def
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| client.name.clone()),
            manufacturer: matched_def
                .map(|d| d.manufacturer)
                .unwrap_or_else(|| "ALSA Sequencer".to_string()),
            card_index: client.card_index,
            alsa_client_id: Some(client.client_id),
            alsa_seq_addr: Some(format!("{}:{}", client.client_id, port.port_id)),
            alsa_port_name: Some(port.name.clone()),
            rawmidi_hw_id: None,
            rawmidi_name: None,
            by_id_symlink: None,
            by_path_symlink: None,
        });
    }

    discovered
}

/// Performs a full read-only hardware discovery pass on the Linux host.
#[must_use]
pub fn discover_connected_midi_hardware() -> Vec<DiscoveredMidiDevice> {
    let cards = scan_sysfs_sound_cards();
    let seq_clients = scan_alsa_seq_clients();
    let rawmidi = scan_alsa_rawmidi();
    correlate_hardware_devices(&cards, &seq_clients, &rawmidi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_fingerprint_parsing_and_usb_replug_matching() {
        let nk2_fp1 = HardwareFingerprint::new("0944", "0117", "", "nanoKONTROL2");
        assert_eq!(nk2_fp1.usb_id(), "0944:0117");
        assert_eq!(nk2_fp1.canonical_key(), "0944:0117::nanoKONTROL2");

        let parsed = HardwareFingerprint::parse("0944:0117::nanoKONTROL2").unwrap();
        assert_eq!(parsed, nk2_fp1);

        // Simulated re-plug where ALSA card id gets suffixed or case-changed by kernel udev
        let nk2_fp2 = HardwareFingerprint::new("0944", "0117", "", "nanokontrol2");
        assert!(nk2_fp1.matches_replugged(&nk2_fp2));

        // Behringer UMC1820 with hardware USB serial number
        let umc_a = HardwareFingerprint::new("1397", "0503", "B8AAF8D3", "UMC1820");
        let umc_b = HardwareFingerprint::new("1397", "0503", "B8AAF8D3", "UMC1820_1");
        let umc_different_unit = HardwareFingerprint::new("1397", "0503", "99999999", "UMC1820");
        assert!(umc_a.matches_replugged(&umc_b));
        assert!(!umc_a.matches_replugged(&umc_different_unit));
    }

    #[test]
    fn parses_proc_asound_seq_clients_and_aconnect_and_amidi() {
        let sample_proc = r#"Client info
  cur  clients : 6
  peak clients : 18
  max  clients : 192

Client   0 : "System" [Kernel Legacy]
  Port   0 : "Timer" (Rwe-) [In/Out]
  Port   1 : "Announce" (R-e-) [In]
Client  14 : "Midi Through" [Kernel Legacy]
  Port   0 : "Midi Through Port-0" (RWe-) [In/Out]
Client  16 : "UMC1820" [Kernel Legacy]
  Port   0 : "UMC1820 MIDI 1" (RWeX) [In/Out]
Client  24 : "nanoKONTROL2" [Kernel Legacy]
  Port   0 : "nanoKONTROL2 _ CTRL" (RWeX) [In/Out]
"#;
        let clients = parse_proc_asound_seq_clients(sample_proc);
        assert_eq!(clients.len(), 4);
        let nk2_client = clients.iter().find(|c| c.client_id == 24).unwrap();
        assert_eq!(nk2_client.name, "nanoKONTROL2");
        assert_eq!(nk2_client.ports.len(), 1);
        assert_eq!(nk2_client.ports[0].port_id, 0);
        assert_eq!(nk2_client.ports[0].name, "nanoKONTROL2 _ CTRL");
        assert_eq!(nk2_client.ports[0].capabilities, "RWeX");

        let sample_aconnect = r#"client 16: 'UMC1820' [type=kernel,card=0]
    0 'UMC1820 MIDI 1  '
client 24: 'nanoKONTROL2' [type=kernel,card=2]
    0 'nanoKONTROL2 _ CTRL'
"#;
        let ac_clients = parse_aconnect_output(sample_aconnect);
        assert_eq!(ac_clients.len(), 2);
        assert_eq!(ac_clients[1].client_id, 24);
        assert_eq!(ac_clients[1].card_index, Some(2));
        assert_eq!(ac_clients[1].ports[0].name, "nanoKONTROL2 _ CTRL");

        let sample_amidi = r#"Dir Device    Name
IO  hw:0,0,0  UMC1820 MIDI 1
IO  hw:2,0,0  nanoKONTROL2 _ SLIDER/KNOB
"#;
        let rawmidi = parse_amidi_output(sample_amidi);
        assert_eq!(rawmidi.len(), 2);
        assert_eq!(rawmidi[1].hw_id, "hw:2,0,0");
        assert_eq!(rawmidi[1].card_index, Some(2));
        assert_eq!(rawmidi[1].name, "nanoKONTROL2 _ SLIDER/KNOB");
    }

    #[test]
    fn correlates_usb_replug_across_changing_alsa_client_and_bus_path() {
        // Initial connection on USB port 1-5.3, card 2, ALSA client 24:0
        let card_port_a = SysfsSoundCard {
            card_index: 2,
            card_id: "nanoKONTROL2".to_string(),
            vendor_id: Some("0944".to_string()),
            product_id: Some("0117".to_string()),
            serial: None,
            manufacturer: Some("KORG INC.".to_string()),
            product: Some("nanoKONTROL2".to_string()),
            by_id_symlink: Some("usb-KORG_INC._nanoKONTROL2-00".to_string()),
            by_path_symlink: Some("pci-0000:00:14.0-usb-0:5.3:1.0".to_string()),
            has_midi_device: true,
        };
        let seq_port_a = AlsaSeqClient {
            client_id: 24,
            name: "nanoKONTROL2".to_string(),
            client_type: "Kernel Legacy".to_string(),
            card_index: Some(2),
            ports: vec![AlsaSeqPort {
                port_id: 0,
                name: "nanoKONTROL2 _ CTRL".to_string(),
                capabilities: "RWeX".to_string(),
            }],
        };

        let dev_a = &correlate_hardware_devices(&[card_port_a], &[seq_port_a], &[])[0];
        assert_eq!(dev_a.fingerprint.canonical_key(), "0944:0117::nanoKONTROL2");
        assert_eq!(dev_a.definition_id.as_deref(), Some("nanokontrol2"));
        assert_eq!(dev_a.alsa_seq_addr.as_deref(), Some("24:0"));

        // Re-plugged into different physical USB port 1-2.1 -> card 3, ALSA client 28:0
        let card_port_b = SysfsSoundCard {
            card_index: 3,
            card_id: "nanoKONTROL2".to_string(),
            vendor_id: Some("0944".to_string()),
            product_id: Some("0117".to_string()),
            serial: None,
            manufacturer: Some("KORG INC.".to_string()),
            product: Some("nanoKONTROL2".to_string()),
            by_id_symlink: Some("usb-KORG_INC._nanoKONTROL2-00".to_string()),
            by_path_symlink: Some("pci-0000:00:14.0-usb-0:2.1:1.0".to_string()),
            has_midi_device: true,
        };
        let seq_port_b = AlsaSeqClient {
            client_id: 28,
            name: "nanoKONTROL2".to_string(),
            client_type: "Kernel Legacy".to_string(),
            card_index: Some(3),
            ports: vec![AlsaSeqPort {
                port_id: 0,
                name: "nanoKONTROL2 _ CTRL".to_string(),
                capabilities: "RWeX".to_string(),
            }],
        };

        let dev_b = &correlate_hardware_devices(&[card_port_b], &[seq_port_b], &[])[0];
        assert_eq!(dev_b.alsa_seq_addr.as_deref(), Some("28:0"));
        assert_eq!(dev_b.rawmidi_hw_id.as_deref(), Some("hw:3,0,0"));
        // Stable hardware fingerprint is identical across physical USB ports!
        assert_eq!(dev_a.fingerprint, dev_b.fingerprint);
        assert!(dev_a.fingerprint.matches_replugged(&dev_b.fingerprint));
    }

    #[test]
    fn live_readonly_host_hardware_inspection_detects_connected_nanokontrol2_if_present() {
        let devices = discover_connected_midi_hardware();
        // If Korg nanoKONTROL2 is connected on this Linux host (card2 / 0944:0117), verify its fields
        if let Some(nk2) = devices
            .iter()
            .find(|d| d.fingerprint.usb_id() == "0944:0117")
        {
            assert_eq!(nk2.definition_id.as_deref(), Some("nanokontrol2"));
            assert_eq!(nk2.fingerprint.canonical_key(), "0944:0117::nanoKONTROL2");
            assert_eq!(nk2.manufacturer, "Korg");
            assert!(nk2.alsa_seq_addr.is_some());
            assert!(nk2.rawmidi_hw_id.is_some());
            assert_eq!(
                nk2.by_id_symlink.as_deref(),
                Some("usb-KORG_INC._nanoKONTROL2-00")
            );
        }
    }
}
