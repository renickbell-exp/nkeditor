//! Advanced Multi-Device Matrix Routing, Per-Port CC Rate-Limiting/Coalescing,
//! and Emergency Hardware & Software PANIC Engine (`src/multi_device_router.rs`).
//!
//! Fulfils Epoch 18 of the 20-epoch `nkEditor4` migration plan:
//! 1. **Composite Performance Rig Matrix Routing ([`MultiDeviceMatrixRouter`])**:
//!    - Links multiple simultaneous physical MIDI controllers into composite rigs:
//!      - Controller A (`nanoKONTROL2`) -> `8opPerc` Voices 1–8 Density & Mute/Solo
//!      - Controller B (`nanoKONTROL2 #2` or `Akai MIDImix`) -> Layer Page 2 Timbre Morph
//!        or Nord Drum 2 6-pad synthesis (`Ch 1..=6` + Global `Ch 10`)
//!    - Supports dedicated per-controller output ports or merging multiple controllers
//!      into a single virtual output port ([`MergeMode::SingleVirtualOutput`]) with
//!      optional per-controller channel offset to prevent CC collisions.
//! 2. **Per-Port MIDI CC Rate-Limiting & Latest-Value Coalescing ([`PortRateLimiter`])**:
//!    - Prevents USB and 5-pin DIN MIDI buffer overflows during rapid fader sweeps or
//!      beat-synchronized scene morphs by enforcing a configurable microsecond interval
//!      (`min_cc_interval_us`) per `(port, channel, cc)` and coalescing intermediate
//!      continuous values into the newest value while letting `Note` and `PANIC` packets
//!      bypass coalescing immediately.
//! 3. **Immediate Emergency PANIC ([`MultiDeviceMatrixRouter::execute_emergency_panic`])**:
//!    - Triggered via GUI button, keyboard shortcut (`Ctrl+Shift+P`), or
//!      `/nkeditor/panic` OSC endpoint ([`crate::osc_server::OscCommand::Panic`]).
//!    - Purges pending coalesced queues, transmits `CC 64 = 0` (Sustain Off),
//!      `CC 120 = 0` (All Sound Off), `CC 121 = 0` (Reset All Controllers), and
//!      `CC 123 = 0` (All Notes Off) across configured MIDI channels, mutes all 8
//!      `8opPerc` voice lanes, stops transport, and forces a full hardware LED resync.

use crate::devices::MidiMessageType;
use crate::layer_manager::{ControllerPage, MultiPageLayerManager};
use crate::led_controller::{LedOperatingMode, MultiDeviceLedManager};
use crate::mapping_engine::{MultiDeviceMappingEngine, ProcessedMidiMessage};
use crate::midi_io::RawMidiPacket;
use crate::op8_model::{Op8PerformanceAction, Op8PerformanceModel};
use crate::osc_server::Op8RigState;
use crate::theme::ThemeSettings;
use egui::{Align, Color32, Layout, RichText, ScrollArea, Stroke, Ui};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Standard MIDI CC 64: Damper / Sustain Pedal (`0` = Off).
pub const MIDI_CC_SUSTAIN_PEDAL: u8 = 64;
/// Standard MIDI CC 120: All Sound Off (`0`).
pub const MIDI_CC_ALL_SOUND_OFF: u8 = 120;
/// Standard MIDI CC 121: Reset All Controllers (`0`).
pub const MIDI_CC_RESET_ALL_CONTROLLERS: u8 = 121;
/// Standard MIDI CC 123: All Notes Off (`0`).
pub const MIDI_CC_ALL_NOTES_OFF: u8 = 123;

/// Default per-(channel, CC) minimum interval in microseconds (`2,000 us` = `2.0 ms`,
/// capping any single CC number at 500 Hz while coalescing intermediate values).
pub const DEFAULT_CC_COALESCE_INTERVAL_US: u64 = 2_000;

/// Default merged virtual output port name.
pub const DEFAULT_MERGED_VIRTUAL_PORT: &str = "nkEditor4 Composite Virtual Out";

// =============================================================================
// 1. Composite Rig Roles, Merge Modes & Factory Templates
// =============================================================================

/// Functional role assigned to a physical MIDI controller inside a composite performance rig.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerRole {
    /// Drives `8opPerc` Voices 1–8 Density (Knobs), Level/Macro (Sliders), Solo/Mute/Rec, and Transport.
    Op8PrimaryVoices,
    /// Pinned to a specific [`ControllerPage`] (e.g. Page 2 Timbre Morph & Decay) so the performer
    /// can manipulate Page 1 on Controller A and Page 2 on Controller B simultaneously without page switching.
    LayerPagePinned(ControllerPage),
    /// Drives Clavia Nord Drum 2 6-channel pad synthesis (`Ch 1..=6`) and Global FX (`Ch 10`).
    NordDrum2SynthSurface,
    /// Translates Korg `nanoKONTROL2` hardware events into factory Akai `MIDImix` Notes and CCs.
    MidimixEmulationBridge,
    /// Evaluates through the standard per-device [`crate::mapping_engine::DeviceMappingPipeline`].
    StandardRemapper,
}

impl ControllerRole {
    /// Human-readable title for the routing matrix UI.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Op8PrimaryVoices => "8opPerc Primary Voices 1-8 (Density & Mute/Solo)",
            Self::LayerPagePinned(ControllerPage::VoiceDensityAndLevel) => {
                "Pinned Layer Page 1: Voice Density & Level"
            }
            Self::LayerPagePinned(ControllerPage::TimbreMorphAndDecay) => {
                "Pinned Layer Page 2: Timbre Morph & Decay"
            }
            Self::LayerPagePinned(ControllerPage::PitchSpectraAndNoiseFilter) => {
                "Pinned Layer Page 3: Pitch/Spectra & Filter"
            }
            Self::LayerPagePinned(ControllerPage::GlobalFeelAndMasterFx) => {
                "Pinned Layer Page 4: Global Feel & Master FX"
            }
            Self::NordDrum2SynthSurface => "Nord Drum 2 6-Pad Synth Surface (Ch 1-6 + Ch 10 FX)",
            Self::MidimixEmulationBridge => "Akai MIDImix Emulation Bridge",
            Self::StandardRemapper => "Standard MIDI Remapper Pipeline",
        }
    }

    /// All selectable controller roles in the GUI matrix.
    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::Op8PrimaryVoices,
            Self::LayerPagePinned(ControllerPage::VoiceDensityAndLevel),
            Self::LayerPagePinned(ControllerPage::TimbreMorphAndDecay),
            Self::LayerPagePinned(ControllerPage::PitchSpectraAndNoiseFilter),
            Self::LayerPagePinned(ControllerPage::GlobalFeelAndMasterFx),
            Self::NordDrum2SynthSurface,
            Self::MidimixEmulationBridge,
            Self::StandardRemapper,
        ]
    }
}

/// Output port topology for the multi-device matrix router.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeMode {
    /// Each physical controller routes to its own dedicated virtual output port.
    DedicatedPerController,
    /// All enabled controllers merge into a single shared virtual output port.
    SingleVirtualOutput {
        /// Shared virtual output port identifier.
        virtual_port_name: String,
    },
}

impl Default for MergeMode {
    fn default() -> Self {
        Self::DedicatedPerController
    }
}

/// Routing configuration for a single linked physical controller in the matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerLinkEntry {
    /// Canonical hardware fingerprint or device key (e.g. `"nanokontrol2-a"`).
    pub device_key: String,
    /// Human-readable label in the matrix (e.g. `"Controller A (Korg nanoKONTROL2)"`).
    pub display_name: String,
    /// Hardware definition ID (`"nanokontrol2"` or `"akai-midimix"`).
    pub definition_id: String,
    /// Assigned functional role in the composite rig.
    pub role: ControllerRole,
    /// Dedicated output port name used when [`MergeMode::DedicatedPerController`] is active.
    pub dedicated_output_port: String,
    /// Optional MIDI channel offset (`0..=15`) applied to outgoing MIDI messages when merging
    /// multiple controllers into one port so identical CCs do not collide.
    pub output_channel_offset: u8,
    /// Whether this controller link is active.
    pub enabled: bool,
}

/// Built-in composite multi-controller rig templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositeRigPreset {
    /// Controller A (`nanoKONTROL2 #1`) -> `8opPerc` Voices 1–8 Density & Mute/Solo;
    /// Controller B (`nanoKONTROL2 #2`) -> Pinned `Page 2: Timbre Morph & Decay`.
    DualNanoKontrol2SplitRig,
    /// Controller A (`nanoKONTROL2`) -> `8opPerc` Voices 1–8 Density & Mute/Solo;
    /// Controller B (`Akai MIDImix`) -> Pinned `Page 2: Timbre Morph & Decay`.
    NanoKontrol2PlusMidiMixHybrid,
    /// Controller A (`nanoKONTROL2`) -> `8opPerc` Voices 1–8 Density & Mute/Solo;
    /// Controller B (`Akai MIDImix`) -> Nord Drum 2 6-Pad Synth Surface (`Ch 1..=6` + `Ch 10`).
    NanoKontrol2PlusNordDrum2Rig,
    /// Merges Controller A (`Ch 1`) and Controller B (`Ch 2` via `+1` channel offset)
    /// into a single virtual output port (`"nkEditor4 Composite Virtual Out"`).
    MergedVirtualOutputRig,
}

impl CompositeRigPreset {
    /// All 4 built-in composite rig templates.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [
            Self::DualNanoKontrol2SplitRig,
            Self::NanoKontrol2PlusMidiMixHybrid,
            Self::NanoKontrol2PlusNordDrum2Rig,
            Self::MergedVirtualOutputRig,
        ]
    }

    /// Human-readable title for UI buttons and command palette.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::DualNanoKontrol2SplitRig => {
                "Dual nanoKONTROL2 Split (A: 8opPerc Voices 1-8 | B: Page 2 Timbre Morph)"
            }
            Self::NanoKontrol2PlusMidiMixHybrid => {
                "nanoKONTROL2 + Akai MIDImix Hybrid (A: 8opPerc Voices | B: Page 2 Timbre Morph)"
            }
            Self::NanoKontrol2PlusNordDrum2Rig => {
                "nanoKONTROL2 + Nord Drum 2 Rig (A: 8opPerc Voices | B: Nord Drum 2 Ch 1-6 + FX)"
            }
            Self::MergedVirtualOutputRig => {
                "Merged Single Virtual Output (Controller A Ch 1 + Controller B Ch 2 -> 1 Port)"
            }
        }
    }
}

// =============================================================================
// 2. Per-Port MIDI CC Rate-Limiter & Latest-Value Coalescer
// =============================================================================

/// Priority class for outgoing MIDI packets routed through [`PortRateLimiter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketPriority {
    /// Continuous CC updates subject to per-`(port, channel, cc)` rate limiting and coalescing.
    CoalesceableCc,
    /// Note On/Off, transport triggers, and emergency PANIC CCs that bypass rate limiting immediately.
    ImmediatePriority,
}

/// Outgoing MIDI packet emitted by the [`MultiDeviceMatrixRouter`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedOutputPacket {
    /// Destination virtual or physical output port name.
    pub output_port: String,
    /// Source physical controller device key (or `"panic-engine"` for emergency panic packets).
    pub source_device_key: String,
    /// Control identifier or summary label (e.g. `"track1.knob"`, `"page2.track1.knob"`, `"panic.all_notes_off"`).
    pub label: String,
    /// MIDI message type (`Cc` or `Note`).
    pub message_type: MidiMessageType,
    /// 1-indexed MIDI channel (`1..=16`).
    pub channel: u8,
    /// MIDI CC or Note number (`0..=127`).
    pub number: u8,
    /// MIDI data byte 2 value (`0..=127`).
    pub value: u8,
    /// Raw 3-byte MIDI wire representation `[status, data1, data2]`.
    pub raw_bytes: [u8; 3],
    /// Priority classification (`CoalesceableCc` vs. `ImmediatePriority`).
    pub priority: PacketPriority,
    /// Timestamp in microseconds when this packet was emitted (`us`).
    pub emitted_at_us: u64,
}

impl RoutedOutputPacket {
    /// Creates an immediate-priority MIDI CC packet (used for emergency PANIC CCs).
    #[must_use]
    pub fn immediate_cc(
        output_port: impl Into<String>,
        source_device_key: impl Into<String>,
        label: impl Into<String>,
        channel: u8,
        cc_number: u8,
        value: u8,
        emitted_at_us: u64,
    ) -> Self {
        let ch = channel.clamp(1, 16);
        let status = 0xB0 | ((ch - 1) & 0x0F);
        let num = cc_number & 0x7F;
        let val = value & 0x7F;
        Self {
            output_port: output_port.into(),
            source_device_key: source_device_key.into(),
            label: label.into(),
            message_type: MidiMessageType::Cc,
            channel: ch,
            number: num,
            value: val,
            raw_bytes: [status, num, val],
            priority: PacketPriority::ImmediatePriority,
            emitted_at_us,
        }
    }
}

/// Composite key `(output_port, channel, cc_number)` used for per-port CC rate-limiting and coalescing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PortCcKey {
    pub output_port: String,
    pub channel: u8,
    pub cc_number: u8,
}

/// Per-port MIDI CC rate-limiter and latest-value coalescer preventing USB/DIN buffer overflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRateLimiter {
    /// Minimum interval in microseconds between consecutive emissions of the same `(port, channel, cc)`.
    /// Set to `0` to disable rate-limiting.
    pub min_cc_interval_us: u64,
    /// Last emission timestamp (`us`) for each `(port, channel, cc)`.
    pub last_emitted_us: BTreeMap<PortCcKey, u64>,
    /// Latest coalesced packet waiting for its rate-limit window (`last_emitted_us + min_cc_interval_us`) to expire.
    pub pending_coalesced: BTreeMap<PortCcKey, RoutedOutputPacket>,
    /// Total packets emitted immediately or via flush.
    pub total_emitted_packets: u64,
    /// Total redundant intermediate CC updates absorbed/coalesced before transmission.
    pub total_coalesced_updates: u64,
}

impl Default for PortRateLimiter {
    fn default() -> Self {
        Self::new(DEFAULT_CC_COALESCE_INTERVAL_US)
    }
}

impl PortRateLimiter {
    /// Creates a new [`PortRateLimiter`] with the specified `min_cc_interval_us` in microseconds.
    #[must_use]
    pub fn new(min_cc_interval_us: u64) -> Self {
        Self {
            min_cc_interval_us,
            last_emitted_us: BTreeMap::new(),
            pending_coalesced: BTreeMap::new(),
            total_emitted_packets: 0,
            total_coalesced_updates: 0,
        }
    }

    /// Submits an outgoing [`RoutedOutputPacket`] at `now_us`.
    /// - If `packet.priority == PacketPriority::ImmediatePriority` or `packet.message_type == MidiMessageType::Note`
    ///   or `self.min_cc_interval_us == 0`, the packet is emitted immediately (`Some(packet)`).
    /// - Otherwise, if `(output_port, channel, cc)` was emitted less than `min_cc_interval_us` ago,
    ///   the packet is coalesced into `pending_coalesced` (overwriting any older pending value for that key)
    ///   and returns `None` until [`Self::flush_due_packets`] is called at or after the due timestamp.
    pub fn submit_packet(
        &mut self,
        mut packet: RoutedOutputPacket,
        now_us: u64,
    ) -> Option<RoutedOutputPacket> {
        packet.emitted_at_us = now_us;

        if self.min_cc_interval_us == 0
            || packet.priority == PacketPriority::ImmediatePriority
            || packet.message_type == MidiMessageType::Note
        {
            self.total_emitted_packets += 1;
            return Some(packet);
        }

        let key = PortCcKey {
            output_port: packet.output_port.clone(),
            channel: packet.channel,
            cc_number: packet.number,
        };

        if let Some(&last_us) = self.last_emitted_us.get(&key) {
            if now_us.saturating_sub(last_us) < self.min_cc_interval_us {
                // Inside rate-limit window: coalesce into the latest value for this key.
                self.total_coalesced_updates += 1;
                self.pending_coalesced.insert(key, packet);
                return None;
            }
        }

        // Window has elapsed (or first time seeing this CC): emit immediately and record timestamp.
        self.pending_coalesced.remove(&key);
        self.last_emitted_us.insert(key, now_us);
        self.total_emitted_packets += 1;
        Some(packet)
    }

    /// Flushes all pending coalesced CC packets whose rate-limit window (`last_us + min_cc_interval_us`)
    /// has expired as of `now_us`.
    pub fn flush_due_packets(&mut self, now_us: u64) -> Vec<RoutedOutputPacket> {
        let mut due_keys = Vec::new();
        for key in self.pending_coalesced.keys() {
            let last_us = self.last_emitted_us.get(key).copied().unwrap_or(0);
            if now_us.saturating_sub(last_us) >= self.min_cc_interval_us {
                due_keys.push(key.clone());
            }
        }

        let mut flushed = Vec::with_capacity(due_keys.len());
        for key in due_keys {
            if let Some(mut pkt) = self.pending_coalesced.remove(&key) {
                pkt.emitted_at_us = now_us;
                self.last_emitted_us.insert(key, now_us);
                self.total_emitted_packets += 1;
                flushed.push(pkt);
            }
        }
        flushed
    }

    /// Purges all pending coalesced CC packets immediately (used during emergency PANIC)
    /// and returns the number of dropped packets.
    pub fn purge_pending(&mut self) -> usize {
        let count = self.pending_coalesced.len();
        self.pending_coalesced.clear();
        count
    }
}

// =============================================================================
// 3. Multi-Device Matrix Router & Emergency Hardware/Software PANIC
// =============================================================================

/// Summary report returned by [`MultiDeviceMatrixRouter::execute_emergency_panic`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanicReport {
    /// Timestamp (`us`) when emergency PANIC was executed.
    pub timestamp_us: u64,
    /// Number of pending coalesced CC packets purged from the rate limiter.
    pub purged_coalesced_packets: usize,
    /// All immediate-priority MIDI panic packets (`CC 64`, `CC 120`, `CC 121`, `CC 123`) emitted across output ports.
    pub midi_panic_packets: Vec<RoutedOutputPacket>,
    /// All hardware LED update packets emitted to resync connected controllers.
    pub led_resync_messages: Vec<(String, Vec<RawMidiPacket>)>,
    /// Number of `8opPerc` voice lanes muted (`8`).
    pub muted_op8_lanes: usize,
}

/// Result of routing a single incoming physical MIDI packet through [`MultiDeviceMatrixRouter`].
#[derive(Debug, Clone, Default)]
pub struct MatrixRouteOutcome {
    /// Role that processed the incoming packet.
    pub role: Option<ControllerRole>,
    /// High-level `8opPerc` action triggered (if routed to `Op8PrimaryVoices`).
    pub op8_action: Option<Op8PerformanceAction>,
    /// Outgoing MIDI packets emitted immediately (after rate-limiting/coalescing).
    pub emitted_packets: Vec<RoutedOutputPacket>,
    /// Whether an outgoing CC packet was absorbed into the rate-limiter coalescing buffer.
    pub was_coalesced: bool,
}

/// Advanced Multi-Device Matrix Router, Per-Port CC Rate-Limiter, and Emergency PANIC Manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiDeviceMatrixRouter {
    /// Active composite rig preset template.
    pub active_preset: CompositeRigPreset,
    /// Output port merge mode (`DedicatedPerController` vs. `SingleVirtualOutput`).
    pub merge_mode: MergeMode,
    /// Linked controller entries keyed by canonical `device_key`.
    pub controller_links: BTreeMap<String, ControllerLinkEntry>,
    /// Per-port MIDI CC rate-limiter and coalescer.
    pub rate_limiter: PortRateLimiter,
    /// MIDI channels (`1..=16`) on which Emergency PANIC transmits `CC 64`, `CC 120`, `CC 121`, and `CC 123`.
    /// Defaults to `[1, 2, 3, 4, 5, 6, 10]` (covering primary performance channels, Nord Drum 2 pads `1..=6`, and global FX `10`).
    pub panic_channels: Vec<u8>,
    /// Total emergency PANIC executions during the session.
    pub panic_count: u64,
    /// Most recent [`PanicReport`] for UI inspection.
    pub last_panic_report: Option<PanicReport>,
}

impl Default for MultiDeviceMatrixRouter {
    fn default() -> Self {
        let mut router = Self {
            active_preset: CompositeRigPreset::DualNanoKontrol2SplitRig,
            merge_mode: MergeMode::DedicatedPerController,
            controller_links: BTreeMap::new(),
            rate_limiter: PortRateLimiter::default(),
            panic_channels: vec![1, 2, 3, 4, 5, 6, 10],
            panic_count: 0,
            last_panic_report: None,
        };
        router.apply_composite_preset(CompositeRigPreset::DualNanoKontrol2SplitRig);
        router
    }
}

impl MultiDeviceMatrixRouter {
    /// Applies one of the 4 built-in [`CompositeRigPreset`] configurations.
    pub fn apply_composite_preset(&mut self, preset: CompositeRigPreset) {
        self.active_preset = preset;
        self.controller_links.clear();

        match preset {
            CompositeRigPreset::DualNanoKontrol2SplitRig => {
                self.merge_mode = MergeMode::DedicatedPerController;
                self.link_controller(ControllerLinkEntry {
                    device_key: "nanokontrol2-a".to_string(),
                    display_name: "Controller A (Korg nanoKONTROL2 #1)".to_string(),
                    definition_id: "nanokontrol2".to_string(),
                    role: ControllerRole::Op8PrimaryVoices,
                    dedicated_output_port: "nkEditor4 Out A (8opPerc Voices)".to_string(),
                    output_channel_offset: 0,
                    enabled: true,
                });
                self.link_controller(ControllerLinkEntry {
                    device_key: "nanokontrol2-b".to_string(),
                    display_name: "Controller B (Korg nanoKONTROL2 #2)".to_string(),
                    definition_id: "nanokontrol2".to_string(),
                    role: ControllerRole::LayerPagePinned(ControllerPage::TimbreMorphAndDecay),
                    dedicated_output_port: "nkEditor4 Out B (Page 2 Timbre Morph)".to_string(),
                    output_channel_offset: 0,
                    enabled: true,
                });
            }
            CompositeRigPreset::NanoKontrol2PlusMidiMixHybrid => {
                self.merge_mode = MergeMode::DedicatedPerController;
                self.link_controller(ControllerLinkEntry {
                    device_key: "nanokontrol2".to_string(),
                    display_name: "Controller A (Korg nanoKONTROL2)".to_string(),
                    definition_id: "nanokontrol2".to_string(),
                    role: ControllerRole::Op8PrimaryVoices,
                    dedicated_output_port: "nkEditor4 Out A (8opPerc Voices)".to_string(),
                    output_channel_offset: 0,
                    enabled: true,
                });
                self.link_controller(ControllerLinkEntry {
                    device_key: "akai-midimix".to_string(),
                    display_name: "Controller B (Akai MIDImix)".to_string(),
                    definition_id: "akai-midimix".to_string(),
                    role: ControllerRole::LayerPagePinned(ControllerPage::TimbreMorphAndDecay),
                    dedicated_output_port: "nkEditor4 Out B (MIDImix Page 2 Morph)".to_string(),
                    output_channel_offset: 0,
                    enabled: true,
                });
            }
            CompositeRigPreset::NanoKontrol2PlusNordDrum2Rig => {
                self.merge_mode = MergeMode::DedicatedPerController;
                self.link_controller(ControllerLinkEntry {
                    device_key: "nanokontrol2".to_string(),
                    display_name: "Controller A (Korg nanoKONTROL2)".to_string(),
                    definition_id: "nanokontrol2".to_string(),
                    role: ControllerRole::Op8PrimaryVoices,
                    dedicated_output_port: "nkEditor4 Out A (8opPerc Voices)".to_string(),
                    output_channel_offset: 0,
                    enabled: true,
                });
                self.link_controller(ControllerLinkEntry {
                    device_key: "akai-midimix".to_string(),
                    display_name: "Controller B (Nord Drum 2 Synth Surface)".to_string(),
                    definition_id: "akai-midimix".to_string(),
                    role: ControllerRole::NordDrum2SynthSurface,
                    dedicated_output_port: "nkEditor4 Out B (Nord Drum 2 Ch 1-6 + Ch 10)".to_string(),
                    output_channel_offset: 0,
                    enabled: true,
                });
            }
            CompositeRigPreset::MergedVirtualOutputRig => {
                self.merge_mode = MergeMode::SingleVirtualOutput {
                    virtual_port_name: DEFAULT_MERGED_VIRTUAL_PORT.to_string(),
                };
                self.link_controller(ControllerLinkEntry {
                    device_key: "nanokontrol2-a".to_string(),
                    display_name: "Controller A (nanoKONTROL2 Ch 1)".to_string(),
                    definition_id: "nanokontrol2".to_string(),
                    role: ControllerRole::StandardRemapper,
                    dedicated_output_port: "nkEditor4 Out A".to_string(),
                    output_channel_offset: 0,
                    enabled: true,
                });
                self.link_controller(ControllerLinkEntry {
                    device_key: "nanokontrol2-b".to_string(),
                    display_name: "Controller B (nanoKONTROL2 Ch 2 Offset +1)".to_string(),
                    definition_id: "nanokontrol2".to_string(),
                    role: ControllerRole::StandardRemapper,
                    dedicated_output_port: "nkEditor4 Out B".to_string(),
                    output_channel_offset: 1,
                    enabled: true,
                });
            }
        }
    }

    /// Registers or updates a [`ControllerLinkEntry`] in the routing matrix.
    pub fn link_controller(&mut self, entry: ControllerLinkEntry) {
        self.controller_links
            .insert(entry.device_key.clone(), entry);
    }

    /// Resolves the destination virtual/physical output port name for `link`.
    #[must_use]
    pub fn resolve_output_port(&self, link: &ControllerLinkEntry) -> String {
        match &self.merge_mode {
            MergeMode::DedicatedPerController => link.dedicated_output_port.clone(),
            MergeMode::SingleVirtualOutput { virtual_port_name } => virtual_port_name.clone(),
        }
    }

    /// Returns all distinct active output port names configured in the matrix.
    #[must_use]
    pub fn active_output_ports(&self) -> Vec<String> {
        let mut ports = Vec::new();
        for link in self.controller_links.values().filter(|l| l.enabled) {
            let port = self.resolve_output_port(link);
            if !ports.contains(&port) {
                ports.push(port);
            }
        }
        if ports.is_empty() {
            ports.push(DEFAULT_MERGED_VIRTUAL_PORT.to_string());
        }
        ports
    }

    /// Routes an incoming physical [`RawMidiPacket`] from `device_key` at `now_us` through
    /// the configured [`ControllerRole`], output port merger, and [`PortRateLimiter`].
    pub fn route_incoming_midi(
        &mut self,
        device_key: &str,
        raw: RawMidiPacket,
        now_us: u64,
        mapping_engine: &mut MultiDeviceMappingEngine,
        layer_manager: &mut MultiPageLayerManager,
        op8_model: &mut Op8PerformanceModel,
    ) -> MatrixRouteOutcome {
        let Some(link) = self.controller_links.get(device_key).cloned() else {
            return MatrixRouteOutcome::default();
        };
        if !link.enabled {
            return MatrixRouteOutcome::default();
        }

        let output_port = self.resolve_output_port(&link);
        let mut outcome = MatrixRouteOutcome {
            role: Some(link.role),
            op8_action: None,
            emitted_packets: Vec::new(),
            was_coalesced: false,
        };

        let Some((msg_type, in_ch, number, value)) = raw.decode() else {
            return outcome;
        };

        match link.role {
            ControllerRole::Op8PrimaryVoices => {
                if msg_type == MidiMessageType::Cc {
                    outcome.op8_action = op8_model.apply_nanokontrol2_cc(number, value);
                }
                let out_ch = ((in_ch - 1 + link.output_channel_offset) % 16) + 1;
                let status = match msg_type {
                    MidiMessageType::Cc => 0xB0 | ((out_ch - 1) & 0x0F),
                    MidiMessageType::Note => 0x90 | ((out_ch - 1) & 0x0F),
                };
                let pkt = RoutedOutputPacket {
                    output_port,
                    source_device_key: device_key.to_string(),
                    label: format!("op8.primary.cc{number}"),
                    message_type: msg_type,
                    channel: out_ch,
                    number,
                    value,
                    raw_bytes: [status, number & 0x7F, value & 0x7F],
                    priority: if msg_type == MidiMessageType::Note {
                        PacketPriority::ImmediatePriority
                    } else {
                        PacketPriority::CoalesceableCc
                    },
                    emitted_at_us: now_us,
                };
                if let Some(emitted) = self.rate_limiter.submit_packet(pkt, now_us) {
                    outcome.emitted_packets.push(emitted);
                } else {
                    outcome.was_coalesced = true;
                }
            }
            ControllerRole::LayerPagePinned(pinned_page) => {
                // Evaluate using the pinned page's PageParameterBank without changing layer_manager.active_page!
                let bank = &mut layer_manager.pages[pinned_page.index()];
                if msg_type == MidiMessageType::Cc {
                    if let Some((slot_idx, is_knob)) = Self::decode_nk2_or_midimix_slot(number) {
                        let control_path = if is_knob {
                            format!("track{}.knob", slot_idx + 1)
                        } else {
                            format!("track{}.slider", slot_idx + 1)
                        };
                        bank.continuous_targets.insert(control_path.clone(), value);
                        let (target_cc, base_ch) = bank
                            .config
                            .entries
                            .get(&control_path)
                            .map(|e| (e.output_number, e.output_channel))
                            .unwrap_or((number, in_ch));
                        let out_ch = ((base_ch - 1 + link.output_channel_offset) % 16) + 1;
                        let status = 0xB0 | ((out_ch - 1) & 0x0F);
                        let pkt = RoutedOutputPacket {
                            output_port,
                            source_device_key: device_key.to_string(),
                            label: format!("{}.{}", pinned_page.id(), control_path),
                            message_type: MidiMessageType::Cc,
                            channel: out_ch,
                            number: target_cc,
                            value,
                            raw_bytes: [status, target_cc & 0x7F, value & 0x7F],
                            priority: PacketPriority::CoalesceableCc,
                            emitted_at_us: now_us,
                        };
                        if let Some(emitted) = self.rate_limiter.submit_packet(pkt, now_us) {
                            outcome.emitted_packets.push(emitted);
                        } else {
                            outcome.was_coalesced = true;
                        }
                    }
                }
            }
            ControllerRole::NordDrum2SynthSurface => {
                // Map Track 1..6 controls to Nord Drum 2 Channels 1..=6 (and Track 7..8 to Global FX Ch 10)
                let (slot_idx, is_knob) =
                    Self::decode_nk2_or_midimix_slot(number).unwrap_or((0, true));
                let nd2_channel = if slot_idx < 6 {
                    (slot_idx as u8) + 1
                } else {
                    10
                };
                // Knobs control Timbre (CC 52) / Global FX (CC 14), Sliders control Decay (CC 49) / FX (CC 47)
                let nd2_cc = if slot_idx < 6 {
                    if is_knob { 52 } else { 49 }
                } else if is_knob {
                    14
                } else {
                    47
                };
                let status = 0xB0 | ((nd2_channel - 1) & 0x0F);
                let pkt = RoutedOutputPacket {
                    output_port,
                    source_device_key: device_key.to_string(),
                    label: format!("nord_drum_2.ch{nd2_channel}.cc{nd2_cc}"),
                    message_type: MidiMessageType::Cc,
                    channel: nd2_channel,
                    number: nd2_cc,
                    value,
                    raw_bytes: [status, nd2_cc & 0x7F, value & 0x7F],
                    priority: PacketPriority::CoalesceableCc,
                    emitted_at_us: now_us,
                };
                if let Some(emitted) = self.rate_limiter.submit_packet(pkt, now_us) {
                    outcome.emitted_packets.push(emitted);
                } else {
                    outcome.was_coalesced = true;
                }
            }
            ControllerRole::MidimixEmulationBridge => {
                for emulated in op8_model.translate_nanokontrol2_to_midimix(number, value) {
                    let out_ch = ((emulated.channel - 1 + link.output_channel_offset) % 16) + 1;
                    let pkt = RoutedOutputPacket {
                        output_port: output_port.clone(),
                        source_device_key: device_key.to_string(),
                        label: format!("midimix_emu.n{}", emulated.number),
                        message_type: emulated.message_type,
                        channel: out_ch,
                        number: emulated.number,
                        value: emulated.value,
                        raw_bytes: emulated.raw_bytes,
                        priority: if emulated.message_type == MidiMessageType::Note {
                            PacketPriority::ImmediatePriority
                        } else {
                            PacketPriority::CoalesceableCc
                        },
                        emitted_at_us: now_us,
                    };
                    if let Some(emitted) = self.rate_limiter.submit_packet(pkt, now_us) {
                        outcome.emitted_packets.push(emitted);
                    } else {
                        outcome.was_coalesced = true;
                    }
                }
            }
            ControllerRole::StandardRemapper => {
                if let Some(processed) = mapping_engine
                    .evaluate_for_device(device_key, msg_type, in_ch, number, value)
                    .into_emitted()
                {
                    let pkt = self.processed_to_routed_packet(
                        &output_port,
                        &link,
                        &processed,
                        now_us,
                    );
                    if let Some(emitted) = self.rate_limiter.submit_packet(pkt, now_us) {
                        outcome.emitted_packets.push(emitted);
                    } else {
                        outcome.was_coalesced = true;
                    }
                }
            }
        }

        outcome
    }

    /// Executes an immediate emergency **PANIC** at `now_us`:
    /// 1. Purges all pending coalesced CC queues across all ports.
    /// 2. Emits `CC 64 = 0` (Sustain Off), `CC 120 = 0` (All Sound Off),
    ///    `CC 121 = 0` (Reset All Controllers), and `CC 123 = 0` (All Notes Off)
    ///    across all configured `panic_channels` on every active output port.
    /// 3. Mutes all 8 `8opPerc` voice lanes (`muted = true`, `soloed = false`, `playing = false`)
    ///    in both [`Op8PerformanceModel`] and [`Op8RigState`].
    /// 4. Forces a full hardware LED panel resync across all connected controllers in [`MultiDeviceLedManager`].
    pub fn execute_emergency_panic(
        &mut self,
        now_us: u64,
        op8_model: &mut Op8PerformanceModel,
        op8_rig_state: &mut Op8RigState,
        led_manager: &mut MultiDeviceLedManager,
    ) -> PanicReport {
        // 1. Purge pending coalesced CC updates
        let purged_coalesced_packets = self.rate_limiter.purge_pending();

        // 2. Emit immediate-priority MIDI panic CCs across all active output ports and panic channels
        let ports = self.active_output_ports();
        let mut midi_panic_packets = Vec::new();
        for port in &ports {
            for &ch in &self.panic_channels {
                for (cc, label) in [
                    (MIDI_CC_SUSTAIN_PEDAL, "panic.sustain_off"),
                    (MIDI_CC_ALL_SOUND_OFF, "panic.all_sound_off"),
                    (MIDI_CC_RESET_ALL_CONTROLLERS, "panic.reset_all_controllers"),
                    (MIDI_CC_ALL_NOTES_OFF, "panic.all_notes_off"),
                ] {
                    let pkt = RoutedOutputPacket::immediate_cc(
                        port.clone(),
                        "panic-engine",
                        label,
                        ch,
                        cc,
                        0,
                        now_us,
                    );
                    if let Some(emitted) = self.rate_limiter.submit_packet(pkt, now_us) {
                        midi_panic_packets.push(emitted);
                    }
                }
            }
        }

        // 3. Mute all 8 8opPerc voice lanes, clear solo, and stop transport
        op8_model.transport.playing = false;
        op8_model.transport.recording_wav = false;
        for lane in &mut op8_model.lanes {
            lane.muted = true;
            lane.soloed = false;
        }

        op8_rig_state.playing = false;
        op8_rig_state.recording = false;
        for lane in &mut op8_rig_state.lanes {
            lane.muted = true;
            lane.soloed = false;
        }

        // 4. Update Op8Cockpit LED snapshot and force a full hardware LED resync on every controller
        let mut led_resync_messages = Vec::new();
        for (dev_key, ctrl) in &mut led_manager.controllers {
            ctrl.set_mode(LedOperatingMode::Op8Cockpit);
            let mut snap = ctrl.op8_snapshot.clone();
            snap.playing = false;
            snap.recording = false;
            for lane in &mut snap.lanes {
                lane.muted = true;
                lane.soloed = false;
                lane.reroll_flash_ticks = 0;
            }
            ctrl.apply_op8_cockpit_snapshot(&snap);
            let msgs = ctrl.resync_full_panel();
            led_resync_messages.push((dev_key.clone(), msgs));
        }

        self.panic_count += 1;
        let report = PanicReport {
            timestamp_us: now_us,
            purged_coalesced_packets,
            midi_panic_packets,
            led_resync_messages,
            muted_op8_lanes: 8,
        };
        self.last_panic_report = Some(report.clone());
        report
    }

    fn processed_to_routed_packet(
        &self,
        output_port: &str,
        link: &ControllerLinkEntry,
        processed: &ProcessedMidiMessage,
        now_us: u64,
    ) -> RoutedOutputPacket {
        let out_ch = ((processed.output_channel - 1 + link.output_channel_offset) % 16) + 1;
        let status = match processed.output_message_type {
            MidiMessageType::Cc => 0xB0 | ((out_ch - 1) & 0x0F),
            MidiMessageType::Note => {
                if processed.output_value > 0 {
                    0x90 | ((out_ch - 1) & 0x0F)
                } else {
                    0x80 | ((out_ch - 1) & 0x0F)
                }
            }
        };
        RoutedOutputPacket {
            output_port: output_port.to_string(),
            source_device_key: processed.device_key.clone(),
            label: processed.control_path.clone(),
            message_type: processed.output_message_type,
            channel: out_ch,
            number: processed.output_number,
            value: processed.output_value,
            raw_bytes: [
                status,
                processed.output_number & 0x7F,
                processed.output_value & 0x7F,
            ],
            priority: if processed.output_message_type == MidiMessageType::Note {
                PacketPriority::ImmediatePriority
            } else {
                PacketPriority::CoalesceableCc
            },
            emitted_at_us: now_us,
        }
    }

    /// Maps either a Korg `nanoKONTROL2` Knob (`16..=23`) / Slider (`0..=7`) or an
    /// Akai `MIDImix` Top Knob (`16, 20, 24, 28, 46, 50, 54, 58`) / Fader (`19, 23, 27, 31, 49, 53, 57, 61`)
    /// to `(slot_index 0..8, is_knob)`.
    fn decode_nk2_or_midimix_slot(cc_number: u8) -> Option<(usize, bool)> {
        if (16..=23).contains(&cc_number) {
            return Some(((cc_number - 16) as usize, true));
        }
        if (0..=7).contains(&cc_number) {
            return Some((cc_number as usize, false));
        }
        const MIDIMIX_ROW1_KNOBS: [u8; 8] = [16, 20, 24, 28, 46, 50, 54, 58];
        if let Some(idx) = MIDIMIX_ROW1_KNOBS.iter().position(|&c| c == cc_number) {
            return Some((idx, true));
        }
        const MIDIMIX_FADERS: [u8; 8] = [19, 23, 27, 31, 49, 53, 57, 61];
        if let Some(idx) = MIDIMIX_FADERS.iter().position(|&c| c == cc_number) {
            return Some((idx, false));
        }
        None
    }
}

// =============================================================================
// 4. Interactive egui Multi-Device Matrix Router & Emergency Panic Panel
// =============================================================================

/// Renders the Multi-Device Matrix Router, Per-Port CC Rate-Limiter controls, and
/// Emergency PANIC button in `egui`.
pub fn render_multi_device_router_panel(
    ui: &mut Ui,
    router: &mut MultiDeviceMatrixRouter,
    op8_model: &mut Op8PerformanceModel,
    op8_rig_state: &mut Op8RigState,
    led_manager: &mut MultiDeviceLedManager,
    theme_settings: &ThemeSettings,
    now_us: u64,
) -> Option<PanicReport> {
    let palette = theme_settings.resolved_palette();
    let panel_fill = theme_settings.build_visuals().panel_fill;
    let mut triggered_panic = None;

    // Check keyboard shortcut (Ctrl+Shift+P) for Emergency Panic
    let shortcut_panic =
        ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::P));
    if shortcut_panic {
        triggered_panic = Some(router.execute_emergency_panic(
            now_us,
            op8_model,
            op8_rig_state,
            led_manager,
        ));
    }

    egui::Frame::none()
        .fill(panel_fill)
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            // Top Header & Emergency PANIC Button
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("MULTI-DEVICE MATRIX ROUTER & HARDWARE SAFETY")
                        .strong()
                        .size(14.0)
                        .color(palette.accent),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Emitted: {}  |  Coalesced: {}  |  Pending Queue: {}",
                        router.rate_limiter.total_emitted_packets,
                        router.rate_limiter.total_coalesced_updates,
                        router.rate_limiter.pending_coalesced.len()
                    ))
                    .monospace()
                    .size(11.5),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let panic_btn = egui::Button::new(
                        RichText::new("!! EMERGENCY PANIC (CC 123 / MUTE ALL) !!")
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(195, 35, 35));
                    if ui
                        .add(panic_btn)
                        .on_hover_text(
                            "Send All Notes Off (CC 123), Sustain Off (CC 64), mute all 8opPerc lanes, and resync hardware LEDs (Ctrl+Shift+P / /nkeditor/panic)",
                        )
                        .clicked()
                    {
                        triggered_panic = Some(router.execute_emergency_panic(
                            now_us,
                            op8_model,
                            op8_rig_state,
                            led_manager,
                        ));
                    }
                });
            });

            ui.add_space(6.0);

            // Composite Rig Preset Selector Bar
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Composite Rig Preset:").strong());
                for preset in CompositeRigPreset::all() {
                    let selected = router.active_preset == preset;
                    if ui.selectable_label(selected, preset.title()).clicked() {
                        router.apply_composite_preset(preset);
                    }
                }
            });

            ui.add_space(4.0);

            // Merge Mode & Per-Port Rate Limiter Controls
            ui.horizontal_wrapped(|ui| {
                let is_merged = matches!(router.merge_mode, MergeMode::SingleVirtualOutput { .. });
                if ui
                    .selectable_label(!is_merged, "Dedicated Output Per Controller")
                    .clicked()
                {
                    router.merge_mode = MergeMode::DedicatedPerController;
                }
                if ui
                    .selectable_label(is_merged, "Merge Into Single Virtual Output Port")
                    .clicked()
                {
                    router.merge_mode = MergeMode::SingleVirtualOutput {
                        virtual_port_name: DEFAULT_MERGED_VIRTUAL_PORT.to_string(),
                    };
                }

                ui.separator();
                ui.label("CC Coalesce Interval (us):");
                let mut interval_us = router.rate_limiter.min_cc_interval_us as u32;
                if ui
                    .add(egui::Slider::new(&mut interval_us, 0..=10_000).suffix(" us"))
                    .changed()
                {
                    router.rate_limiter.min_cc_interval_us = u64::from(interval_us);
                }
            });

            ui.add_space(6.0);

            // Controller Routing Matrix Table
            ScrollArea::vertical()
                .max_height(150.0)
                .id_salt("multi_device_router_matrix_scroll")
                .show(ui, |ui| {
                    for link in router.controller_links.values_mut() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut link.enabled, "");
                            ui.label(RichText::new(&link.display_name).strong().monospace());
                            ui.label(
                                RichText::new(format!("[{}]", link.device_key))
                                    .monospace()
                                    .size(11.0),
                            );
                            ui.separator();
                            egui::ComboBox::from_id_salt(format!("role_{}", link.device_key))
                                .selected_text(link.role.title())
                                .show_ui(ui, |ui| {
                                    for role in ControllerRole::all() {
                                        ui.selectable_value(&mut link.role, role, role.title());
                                    }
                                });
                            ui.separator();
                            ui.label(
                                RichText::new(format!("Ch Offset: +{}", link.output_channel_offset))
                                    .monospace()
                                    .size(11.0)
                                    .color(palette.accent),
                            );
                        });
                    }
                });

            if let Some(ref rep) = router.last_panic_report {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Last PANIC @ {} us: {} MIDI Reset Packets sent | {} Coalesced Packets Purged | {} 8opPerc Lanes Muted",
                        rep.timestamp_us,
                        rep.midi_panic_packets.len(),
                        rep.purged_coalesced_packets,
                        rep.muted_op8_lanes
                    ))
                    .monospace()
                    .size(11.0)
                    .color(Color32::from_rgb(235, 110, 80)),
                );
            }
        });

    triggered_panic
}

// =============================================================================
// 5. Automated Unit & Headless egui Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping_engine::DeviceMappingConfig;
    use crate::osc_server::{parse_osc_message, OscCommand};
    use rosc::OscMessage;

    #[test]
    fn composite_rig_links_controller_a_to_op8_voices_and_controller_b_to_page2_or_nord_drum_2_or_merged_port() {
        let mut router = MultiDeviceMatrixRouter::default();
        let mut mapping_engine = MultiDeviceMappingEngine::new();
        mapping_engine.register_device_pipeline(
            "nanokontrol2-a",
            DeviceMappingConfig::default_nanokontrol2(),
        );
        mapping_engine.register_device_pipeline(
            "nanokontrol2-b",
            DeviceMappingConfig::default_nanokontrol2(),
        );
        let mut layer_manager = MultiPageLayerManager::default();
        let mut op8_model = Op8PerformanceModel::default();

        // 1. Default DualNanoKontrol2SplitRig:
        //    Controller A (CC 16 = Track 1 Knob) -> Op8PrimaryVoices (updates Lane 0 density)
        //    Controller B (CC 16 = Track 1 Knob) -> LayerPagePinned(TimbreMorphAndDecay) (emits Page 2 CC 24 without changing LayerManager's active page!)
        assert_eq!(
            layer_manager.active_page,
            ControllerPage::VoiceDensityAndLevel
        );

        let out_a = router.route_incoming_midi(
            "nanokontrol2-a",
            RawMidiPacket::control_change(1, 16, 127),
            1_000,
            &mut mapping_engine,
            &mut layer_manager,
            &mut op8_model,
        );
        assert_eq!(out_a.role, Some(ControllerRole::Op8PrimaryVoices));
        assert!(out_a.op8_action.is_some());
        assert!((op8_model.lanes[0].density_multiplier - 5.0).abs() < 1e-6);
        assert_eq!(out_a.emitted_packets.len(), 1);
        assert_eq!(
            out_a.emitted_packets[0].output_port,
            "nkEditor4 Out A (8opPerc Voices)"
        );

        let out_b = router.route_incoming_midi(
            "nanokontrol2-b",
            RawMidiPacket::control_change(1, 16, 90),
            2_000,
            &mut mapping_engine,
            &mut layer_manager,
            &mut op8_model,
        );
        assert_eq!(
            out_b.role,
            Some(ControllerRole::LayerPagePinned(
                ControllerPage::TimbreMorphAndDecay
            ))
        );
        // Active page on primary layer_manager remains VoiceDensityAndLevel!
        assert_eq!(
            layer_manager.active_page,
            ControllerPage::VoiceDensityAndLevel
        );
        assert_eq!(out_b.emitted_packets.len(), 1);
        assert_eq!(out_b.emitted_packets[0].number, 24); // Page 2 Timbre Morph CC for lane 0
        assert_eq!(
            out_b.emitted_packets[0].output_port,
            "nkEditor4 Out B (Page 2 Timbre Morph)"
        );

        // 2. Switch to MergedVirtualOutputRig: both Controller A and Controller B route to DEFAULT_MERGED_VIRTUAL_PORT,
        //    with Controller A on Channel 1 and Controller B offset to Channel 2.
        router.apply_composite_preset(CompositeRigPreset::MergedVirtualOutputRig);
        let merged_a = router.route_incoming_midi(
            "nanokontrol2-a",
            RawMidiPacket::control_change(1, 16, 64),
            10_000,
            &mut mapping_engine,
            &mut layer_manager,
            &mut op8_model,
        );
        let merged_b = router.route_incoming_midi(
            "nanokontrol2-b",
            RawMidiPacket::control_change(1, 16, 64),
            10_000,
            &mut mapping_engine,
            &mut layer_manager,
            &mut op8_model,
        );
        assert_eq!(
            merged_a.emitted_packets[0].output_port,
            DEFAULT_MERGED_VIRTUAL_PORT
        );
        assert_eq!(
            merged_b.emitted_packets[0].output_port,
            DEFAULT_MERGED_VIRTUAL_PORT
        );
        assert_eq!(merged_a.emitted_packets[0].channel, 1);
        assert_eq!(merged_b.emitted_packets[0].channel, 2);
    }

    #[test]
    fn per_port_cc_rate_limiter_coalesces_rapid_sweeps_and_flushes_latest_value() {
        let mut limiter = PortRateLimiter::new(2_000); // 2,000 us = 2ms window

        // Packet 1 at t = 1,000 us -> emitted immediately
        let p1 = RoutedOutputPacket {
            output_port: "PortA".to_string(),
            source_device_key: "nk2".to_string(),
            label: "track1.knob".to_string(),
            message_type: MidiMessageType::Cc,
            channel: 1,
            number: 16,
            value: 10,
            raw_bytes: [0xB0, 16, 10],
            priority: PacketPriority::CoalesceableCc,
            emitted_at_us: 1_000,
        };
        assert!(limiter.submit_packet(p1.clone(), 1_000).is_some());

        // Packets 2, 3, 4 within 2,000 us window (at 1,200 us, 1,500 us, 1,900 us) -> coalesced!
        for (t, val) in [(1_200, 30), (1_500, 75), (1_900, 118)] {
            let mut p = p1.clone();
            p.value = val;
            p.raw_bytes[2] = val;
            assert!(
                limiter.submit_packet(p, t).is_none(),
                "Update at t={t} should be coalesced"
            );
        }
        assert_eq!(limiter.total_coalesced_updates, 3);
        assert_eq!(limiter.pending_coalesced.len(), 1);

        // Flush at t = 2,500 us (still < 1,000 + 2,000 = 3,000 us) -> nothing due yet
        assert!(limiter.flush_due_packets(2_500).is_empty());

        // Flush at t = 3,000 us -> emits ONLY the latest coalesced value (118)!
        let flushed = limiter.flush_due_packets(3_000);
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].value, 118);
        assert_eq!(limiter.pending_coalesced.len(), 0);
    }

    #[test]
    fn emergency_panic_sends_cc123_and_cc64_mutes_all_op8_lanes_resyncs_leds_and_parses_osc() {
        // Verify `/nkeditor/panic` OSC parsing
        let osc_cmd = parse_osc_message(&OscMessage {
            addr: "/nkeditor/panic".to_string(),
            args: vec![],
        })
        .expect("valid /nkeditor/panic command");
        assert_eq!(osc_cmd, OscCommand::Panic);

        let mut router = MultiDeviceMatrixRouter::default();
        let mut op8_model = Op8PerformanceModel::default();
        let mut op8_rig_state = Op8RigState::default();
        let mut led_manager = MultiDeviceLedManager::new();
        led_manager.ensure_controller("nanokontrol2-a", "nanokontrol2");
        led_manager.ensure_controller("akai-midimix", "akai-midimix");

        // Start playing with unmuted, soloed lanes and a pending coalesced CC packet
        op8_model.transport.playing = true;
        op8_model.lanes[0].muted = false;
        op8_model.lanes[0].soloed = true;
        op8_rig_state.playing = true;
        op8_rig_state.lanes[0].muted = false;
        op8_rig_state.lanes[0].soloed = true;

        // Queue a coalesced packet
        let pkt = RoutedOutputPacket {
            output_port: "nkEditor4 Out A (8opPerc Voices)".to_string(),
            source_device_key: "nanokontrol2-a".to_string(),
            label: "track1.knob".to_string(),
            message_type: MidiMessageType::Cc,
            channel: 1,
            number: 16,
            value: 50,
            raw_bytes: [0xB0, 16, 50],
            priority: PacketPriority::CoalesceableCc,
            emitted_at_us: 100,
        };
        let _ = router.rate_limiter.submit_packet(pkt.clone(), 100);
        let _ = router.rate_limiter.submit_packet(pkt, 200);
        assert_eq!(router.rate_limiter.pending_coalesced.len(), 1);

        // Execute emergency PANIC
        let report = router.execute_emergency_panic(
            5_000,
            &mut op8_model,
            &mut op8_rig_state,
            &mut led_manager,
        );

        assert_eq!(report.purged_coalesced_packets, 1);
        assert_eq!(router.rate_limiter.pending_coalesced.len(), 0);
        assert!(!op8_model.transport.playing);
        assert!(!op8_rig_state.playing);
        assert!(op8_model.lanes.iter().all(|l| l.muted && !l.soloed));
        assert!(op8_rig_state.lanes.iter().all(|l| l.muted && !l.soloed));

        // Verify CC 64 (Sustain Off = 0) and CC 123 (All Notes Off = 0) were emitted
        assert!(report
            .midi_panic_packets
            .iter()
            .any(|p| p.number == MIDI_CC_SUSTAIN_PEDAL && p.value == 0));
        assert!(report
            .midi_panic_packets
            .iter()
            .any(|p| p.number == MIDI_CC_ALL_NOTES_OFF && p.value == 0));

        // Verify hardware LEDs were resynced on both connected controllers (28 LEDs on nanoKONTROL2, 24 on MIDImix)
        assert_eq!(report.led_resync_messages.len(), 2);
        assert_eq!(report.led_resync_messages[0].1.len(), 24);
        assert_eq!(report.led_resync_messages[1].1.len(), 28);
    }

    #[test]
    fn headless_egui_renders_multi_device_router_panel_without_panic() {
        let ctx = egui::Context::default();
        let mut router = MultiDeviceMatrixRouter::default();
        let mut op8_model = Op8PerformanceModel::default();
        let mut op8_rig_state = Op8RigState::default();
        let mut led_manager = MultiDeviceLedManager::new();
        led_manager.ensure_controller("nanokontrol2-a", "nanokontrol2");
        let theme_settings = ThemeSettings::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_multi_device_router_panel(
                    ui,
                    &mut router,
                    &mut op8_model,
                    &mut op8_rig_state,
                    &mut led_manager,
                    &theme_settings,
                    10_000,
                );
            });
        });
    }
}
