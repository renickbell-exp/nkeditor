//! Hardware LED Controller (`src/led_controller.rs`).
//!
//! Manages physical button LEDs across multiple connected MIDI controllers simultaneously,
//! honoring each controller's hardware [`LedProtocol`]:
//! - **Korg nanoKONTROL2 (`nanokontrol2`)**: 28 CC-driven LEDs on MIDI Channel 1
//!   (`0xB0`, `CC 32..39` Solo, `CC 48..55` Mute, `CC 64..71` Rec, and `CC 41, 42, 45, 46`
//!   Transport Play/Stop/Record/Cycle; `127` = ON, `0` = OFF).
//! - **Akai MIDI Mix (`akai-midimix`)**: 24 Note-driven LEDs on MIDI Channel 1
//!   (`0x90`, Notes `1..24` for Mute, Solo, and Rec Arm across Tracks 1..8; `127` = ON, `0` = OFF).
//!
//! Supports three operating modes per device ([`LedOperatingMode`]):
//! 1. [`LedOperatingMode::Internal`]: LEDs automatically reflect the local button toggle/momentary
//!    states from [`DeviceMappingPipeline`].
//! 2. [`LedOperatingMode::External`]: LEDs are driven by incoming DAW/synth MIDI packets or
//!    OSC `/nkeditor/led/set` feedback.
//! 3. [`LedOperatingMode::Op8Cockpit`]: Solo, Mute, Rec/RecArm, and Transport LEDs reflect live
//!    `8opPerc` percussion voice lane states, timed timbre reroll flashes, and bar/beat transport
//!    indicators ([`Op8CockpitLedSnapshot`]).
//!
//! Includes **dirty-flag diff coalescing** so redundant or oscillating state updates within a frame
//! only transmit changed LED states to hardware, plus **automatic full-panel resynchronization**
//! upon hot-plug reconnection or preset load.

use crate::devices::{
    akai_midimix_definition, get_device_definition, nanokontrol2_definition, DeviceDefinition,
    LedDef, MidiMessageType,
};
use crate::mapping_engine::{ButtonStateChangeEvent, DeviceMappingPipeline};
use crate::midi_io::{HotplugEvent, RawMidiPacket};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Default number of animation ticks a Rec/RecArm LED flashes when a timbre reroll is triggered.
pub const DEFAULT_REROLL_FLASH_TICKS: u8 = 4;

// =============================================================================
// 1. LED Operating Modes & 8opPerc Cockpit Telemetry State
// =============================================================================

/// Operating mode governing how a controller's physical LEDs are driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LedOperatingMode {
    /// LEDs mirror the internal button toggle/momentary state from [`DeviceMappingPipeline`].
    #[default]
    Internal,
    /// LEDs are controlled by external MIDI/OSC feedback from a DAW, synth, or external script.
    External,
    /// LEDs reflect the live `8opPerc` performance cockpit (per-lane Solo/Mute, timbre reroll
    /// Rec flashes, Play/Stop transport, Record WAV capture, and Cycle bar/downbeat indicator).
    Op8Cockpit,
}

/// Live LED state for one of the 8 `8opPerc` percussion voice lanes (`0..8` -> `track1..track8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Op8VoiceLedState {
    /// Whether this percussion voice lane is muted (drives `trackN.mute` LED).
    pub muted: bool,
    /// Whether this percussion voice lane is soloed (drives `trackN.solo` LED).
    pub soloed: bool,
    /// Remaining animation ticks for the timbre reroll flash on `trackN.rec` / `trackN.rec_arm`.
    /// When `> 0`, the Rec LED illuminates and counts down on each call to `tick_animations()`.
    pub reroll_flash_ticks: u8,
}

/// Complete `8opPerc` performance cockpit LED state across all 8 percussion lanes and transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op8CockpitLedSnapshot {
    /// 8 percussion voice lanes (`kick`, `snare`, `hat_closed`, `hat_open`, `clap`, `tom`, `rim`, `cymbal`).
    pub lanes: [Op8VoiceLedState; 8],
    /// Whether the Konduktiva / `8opPerc` transport is actively playing (`transport.play` LED).
    pub playing: bool,
    /// Whether live WAV stem/mix capture is active (`transport.record` LED).
    pub recording: bool,
    /// Current 0-indexed beat within the bar (`0..3` in 4/4). Beat `0` (downbeat) or a full
    /// 8-voice timbre set advance illuminates the `transport.cycle` LED.
    pub beat_in_bar: u8,
    /// Whether `transport.cycle` should illuminate on the bar downbeat (`beat_in_bar == 0` while `playing`).
    pub beat_pulse_enabled: bool,
    /// Remaining animation ticks for a full 8-voice timbre set change flash on `transport.cycle`.
    pub cycle_flash_ticks: u8,
}

impl Default for Op8CockpitLedSnapshot {
    fn default() -> Self {
        Self {
            lanes: [Op8VoiceLedState::default(); 8],
            playing: false,
            recording: false,
            beat_in_bar: 0,
            beat_pulse_enabled: true,
            cycle_flash_ticks: 0,
        }
    }
}

impl Op8CockpitLedSnapshot {
    /// Triggers a timed timbre reroll LED flash on a single percussion voice lane (`0..8`).
    pub fn trigger_lane_reroll_flash(&mut self, lane_index: usize, ticks: u8) {
        if let Some(lane) = self.lanes.get_mut(lane_index) {
            lane.reroll_flash_ticks = ticks.max(1);
        }
    }

    /// Triggers a timed timbre reroll LED flash across all 8 percussion voice lanes and the
    /// `transport.cycle` indicator (used when advancing the 4,096-entry 8-voice timbre reservoir).
    pub fn trigger_all_lanes_reroll_flash(&mut self, ticks: u8) {
        let t = ticks.max(1);
        for lane in &mut self.lanes {
            lane.reroll_flash_ticks = t;
        }
        self.cycle_flash_ticks = t;
    }

    /// Advances all flash timers by one tick, returning `true` if any LED flash state changed.
    pub fn advance_flash_timers(&mut self) -> bool {
        let mut changed = false;
        for lane in &mut self.lanes {
            if lane.reroll_flash_ticks > 0 {
                lane.reroll_flash_ticks -= 1;
                if lane.reroll_flash_ticks == 0 {
                    changed = true;
                }
            }
        }
        if self.cycle_flash_ticks > 0 {
            self.cycle_flash_ticks -= 1;
            if self.cycle_flash_ticks == 0 {
                changed = true;
            }
        }
        changed
    }

    /// Computes whether a specific dot-notated `button_name` should be illuminated for this snapshot.
    #[must_use]
    pub fn desired_led_for_button(&self, button_name: &str) -> Option<bool> {
        if let Some(rest) = button_name.strip_prefix("track") {
            let (track_str, control_name) = rest.split_once('.')?;
            let track_num: usize = track_str.parse().ok()?;
            if !(1..=8).contains(&track_num) {
                return None;
            }
            let lane = &self.lanes[track_num - 1];
            match control_name {
                "mute" => Some(lane.muted),
                "solo" => Some(lane.soloed),
                "rec" | "rec_arm" => Some(lane.reroll_flash_ticks > 0),
                _ => None,
            }
        } else if let Some(transport_control) = button_name.strip_prefix("transport.") {
            match transport_control {
                "play" => Some(self.playing),
                "stop" => Some(!self.playing),
                "record" => Some(self.recording),
                "cycle" => {
                    let downbeat_on =
                        self.playing && self.beat_pulse_enabled && self.beat_in_bar == 0;
                    Some(downbeat_on || self.cycle_flash_ticks > 0)
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

// =============================================================================
// 2. Per-LED Dirty-Flag Tracking Entry
// =============================================================================

/// Internal state of a single hardware LED managed by [`DeviceLedController`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackedLedEntry {
    /// Dot-notated button name (e.g. `"track1.solo"`, `"transport.play"`).
    pub button_name: String,
    /// Hardware MIDI CC or Note number (`0..=127`).
    pub midi_number: u8,
    /// Desired logical state (`true` = ON, `false` = OFF).
    pub desired_state: bool,
    /// Last state actually transmitted to hardware (`None` if never transmitted or invalidated by resync).
    pub last_transmitted_state: Option<bool>,
    /// Dirty flag indicating `last_transmitted_state != Some(desired_state)`.
    pub dirty: bool,
}

impl TrackedLedEntry {
    #[must_use]
    pub fn new(def: &LedDef) -> Self {
        Self {
            button_name: def.button_name.clone(),
            midi_number: def.midi_number,
            desired_state: false,
            last_transmitted_state: Some(false),
            dirty: false,
        }
    }

    /// Updates `desired_state` and recomputes `dirty` relative to `last_transmitted_state`.
    /// Returns `true` if the update was coalesced (no change in `desired_state`).
    pub fn set_desired(&mut self, is_on: bool) -> bool {
        let was_same_desired = self.desired_state == is_on;
        self.desired_state = is_on;
        self.dirty = self.last_transmitted_state != Some(self.desired_state);
        was_same_desired
    }

    /// Forces this LED to be marked dirty for full-panel resynchronization.
    pub fn mark_dirty_for_resync(&mut self) {
        self.last_transmitted_state = None;
        self.dirty = true;
    }
}

// =============================================================================
// 3. Per-Device Hardware LED Controller
// =============================================================================

/// Protocol-aware Hardware LED Controller for a single MIDI controller instance
/// (e.g. Korg `nanoKONTROL2` with 28 CC LEDs or Akai `MIDI Mix` with 24 Note LEDs).
#[derive(Debug, Clone)]
pub struct DeviceLedController {
    /// Unique device key (e.g. `"0944:0117::nanoKONTROL2"`).
    pub device_key: String,
    /// Hardware specification containing `led_protocol` and `leds`.
    pub device_def: DeviceDefinition,
    /// Active LED operating mode (`Internal`, `External`, or `Op8Cockpit`).
    pub mode: LedOperatingMode,
    /// Ordered LED entries preserving `device_def.leds` panel order.
    pub leds: Vec<TrackedLedEntry>,
    /// Fast index from `button_name` (`"track1.solo"`) -> index in `leds`.
    name_to_index: BTreeMap<String, usize>,
    /// Fast index from `midi_number` (`32`) -> index in `leds`.
    number_to_index: BTreeMap<u8, usize>,
    /// Live `8opPerc` cockpit state used when `mode == LedOperatingMode::Op8Cockpit`.
    pub op8_snapshot: Op8CockpitLedSnapshot,
    /// Total MIDI LED packets emitted by `flush_dirty_packets()`.
    pub total_packets_transmitted: u64,
    /// Total redundant state updates suppressed by dirty-flag diff coalescing.
    pub coalesced_updates_skipped: u64,
}

impl DeviceLedController {
    /// Creates a new [`DeviceLedController`] for the given `device_key` and [`DeviceDefinition`].
    #[must_use]
    pub fn new(device_key: impl Into<String>, device_def: DeviceDefinition) -> Self {
        let mut leds = Vec::with_capacity(device_def.leds.len());
        let mut name_to_index = BTreeMap::new();
        let mut number_to_index = BTreeMap::new();

        for (idx, led_def) in device_def.leds.iter().enumerate() {
            leds.push(TrackedLedEntry::new(led_def));
            name_to_index.insert(led_def.button_name.clone(), idx);
            number_to_index.insert(led_def.midi_number, idx);
        }

        Self {
            device_key: device_key.into(),
            device_def,
            mode: LedOperatingMode::Internal,
            leds,
            name_to_index,
            number_to_index,
            op8_snapshot: Op8CockpitLedSnapshot::default(),
            total_packets_transmitted: 0,
            coalesced_updates_skipped: 0,
        }
    }

    /// Creates a 28-LED CC controller for a Korg `nanoKONTROL2`.
    #[must_use]
    pub fn new_nanokontrol2(device_key: impl Into<String>) -> Self {
        Self::new(device_key, nanokontrol2_definition())
    }

    /// Creates a 24-LED Note controller for an Akai `MIDI Mix`.
    #[must_use]
    pub fn new_akai_midimix(device_key: impl Into<String>) -> Self {
        Self::new(device_key, akai_midimix_definition())
    }

    /// Returns the total number of controllable hardware LEDs on this device
    /// (`28` for `nanoKONTROL2`, `24` for `MIDI Mix`).
    #[must_use]
    pub fn led_count(&self) -> usize {
        self.leds.len()
    }

    /// Returns the number of LEDs currently marked dirty (pending hardware transmission).
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.leds.iter().filter(|l| l.dirty).count()
    }

    /// Switches the operating mode (`Internal`, `External`, or `Op8Cockpit`).
    /// When switching into `Op8Cockpit` mode, immediately applies the stored `op8_snapshot`.
    pub fn set_mode(&mut self, mode: LedOperatingMode) {
        self.mode = mode;
        if mode == LedOperatingMode::Op8Cockpit {
            let snap = self.op8_snapshot.clone();
            self.apply_op8_cockpit_snapshot(&snap);
        }
    }

    /// Queries the current desired state (`true` = ON, `false` = OFF) of an LED by `button_name`.
    #[must_use]
    pub fn led_state_by_name(&self, button_name: &str) -> bool {
        self.name_to_index
            .get(button_name)
            .and_then(|&idx| self.leds.get(idx))
            .is_some_and(|e| e.desired_state)
    }

    /// Queries the current desired state (`true` = ON, `false` = OFF) of an LED by its MIDI CC/Note number.
    #[must_use]
    pub fn led_state_by_number(&self, midi_number: u8) -> bool {
        self.number_to_index
            .get(&midi_number)
            .and_then(|&idx| self.leds.get(idx))
            .is_some_and(|e| e.desired_state)
    }

    /// Returns a `BTreeMap<String, bool>` snapshot of all LED states (`button_name -> is_on`),
    /// suitable for persisting in [`crate::midi_io::ManagedDeviceSession::led_states`].
    #[must_use]
    pub fn export_led_state_map(&self) -> BTreeMap<String, bool> {
        self.leds
            .iter()
            .map(|e| (e.button_name.clone(), e.desired_state))
            .collect()
    }

    /// Sets the desired state of an LED by `button_name` (e.g. `"track1.solo"`), applying
    /// dirty-flag diff coalescing. Returns `true` if `button_name` exists on this device.
    pub fn set_led_by_name(&mut self, button_name: &str, is_on: bool) -> bool {
        let Some(&idx) = self.name_to_index.get(button_name) else {
            return false;
        };
        let entry = &mut self.leds[idx];
        let was_coalesced = entry.set_desired(is_on);
        if was_coalesced || !entry.dirty {
            self.coalesced_updates_skipped += 1;
        }
        true
    }

    /// Sets the desired state of an LED by its hardware MIDI CC or Note number (`0..=127`),
    /// applying dirty-flag diff coalescing. Returns `true` if the MIDI number maps to an LED.
    pub fn set_led_by_number(&mut self, midi_number: u8, is_on: bool) -> bool {
        let Some(&idx) = self.number_to_index.get(&midi_number) else {
            return false;
        };
        let entry = &mut self.leds[idx];
        let was_coalesced = entry.set_desired(is_on);
        if was_coalesced || !entry.dirty {
            self.coalesced_updates_skipped += 1;
        }
        true
    }

    // -------------------------------------------------------------------------
    // Mode 1: Internal Mode (Button Toggle / Momentary State Sync)
    // -------------------------------------------------------------------------

    /// Processes a [`ButtonStateChangeEvent`] emitted by [`DeviceMappingPipeline`].
    ///
    /// Only updates hardware LEDs when `self.mode == LedOperatingMode::Internal`.
    pub fn handle_internal_button_event(&mut self, event: &ButtonStateChangeEvent) -> bool {
        if self.mode != LedOperatingMode::Internal {
            return false;
        }
        if !self.set_led_by_name(&event.control_path, event.is_on) {
            return self.set_led_by_number(event.input_number, event.is_on);
        }
        true
    }

    /// Synchronizes all LED desired states from a [`DeviceMappingPipeline`]'s button states.
    pub fn sync_from_pipeline(&mut self, pipeline: &DeviceMappingPipeline) {
        let names: Vec<String> = self.leds.iter().map(|l| l.button_name.clone()).collect();
        for name in names {
            let is_on = pipeline.button_state(&name);
            let _ = self.set_led_by_name(&name, is_on);
        }
    }

    // -------------------------------------------------------------------------
    // Mode 2: External Mode (DAW / Synth MIDI or OSC Feedback)
    // -------------------------------------------------------------------------

    /// Processes an incoming external MIDI packet (from a DAW, hardware synth, or loopback)
    /// when `self.mode == LedOperatingMode::External`.
    pub fn handle_external_midi_packet(&mut self, packet: RawMidiPacket) -> bool {
        if self.mode != LedOperatingMode::External {
            return false;
        }
        let Some((msg_type, ch, midi_number, value)) = packet.decode() else {
            return false;
        };
        if msg_type != self.device_def.led_protocol.message_type
            || ch != self.device_def.led_protocol.channel
        {
            return false;
        }
        let is_on = value >= 64 || value == self.device_def.led_protocol.on_value;
        self.set_led_by_number(midi_number, is_on)
    }

    /// Processes an external OSC `/nkeditor/led/set <button_id_or_number> <on_off>` command.
    /// Accepts either a dot-notated button name (`"track1.solo"`) or numeric string (`"32"`).
    pub fn handle_external_osc_led(&mut self, identifier: &str, is_on: bool) -> bool {
        if self.mode != LedOperatingMode::External {
            return false;
        }
        if self.set_led_by_name(identifier, is_on) {
            return true;
        }
        if let Ok(num) = identifier.trim().parse::<u8>() {
            return self.set_led_by_number(num, is_on);
        }
        false
    }

    // -------------------------------------------------------------------------
    // Mode 3: 8opPerc Cockpit Mode (Voice Lanes, Reroll Flashes & Bar Beats)
    // -------------------------------------------------------------------------

    /// Applies a complete [`Op8CockpitLedSnapshot`] to all track Solo/Mute/Rec LEDs and
    /// transport Play/Stop/Record/Cycle LEDs when in [`LedOperatingMode::Op8Cockpit`].
    pub fn apply_op8_cockpit_snapshot(&mut self, snapshot: &Op8CockpitLedSnapshot) {
        self.op8_snapshot = snapshot.clone();
        if self.mode != LedOperatingMode::Op8Cockpit {
            return;
        }
        let names: Vec<String> = self.leds.iter().map(|l| l.button_name.clone()).collect();
        for name in names {
            if let Some(desired) = self.op8_snapshot.desired_led_for_button(&name) {
                let _ = self.set_led_by_name(&name, desired);
            }
        }
    }

    /// Triggers a timbre reroll LED flash on `track{lane+1}.rec` (or `rec_arm`) in `Op8Cockpit` mode.
    pub fn trigger_op8_lane_reroll(&mut self, lane_index: usize, flash_ticks: u8) {
        self.op8_snapshot
            .trigger_lane_reroll_flash(lane_index, flash_ticks);
        if self.mode == LedOperatingMode::Op8Cockpit {
            let snap = self.op8_snapshot.clone();
            self.apply_op8_cockpit_snapshot(&snap);
        }
    }

    /// Triggers a full 8-voice timbre set reroll flash across all 8 Rec LEDs and `transport.cycle`.
    pub fn trigger_op8_all_lanes_reroll(&mut self, flash_ticks: u8) {
        self.op8_snapshot.trigger_all_lanes_reroll_flash(flash_ticks);
        if self.mode == LedOperatingMode::Op8Cockpit {
            let snap = self.op8_snapshot.clone();
            self.apply_op8_cockpit_snapshot(&snap);
        }
    }

    /// Updates the current `8opPerc` bar beat (`0..3`) and playing state, illuminating
    /// `transport.cycle` on the bar downbeat (`beat_in_bar == 0`).
    pub fn update_op8_transport_beat(&mut self, playing: bool, beat_in_bar: u8, recording: bool) {
        self.op8_snapshot.playing = playing;
        self.op8_snapshot.beat_in_bar = beat_in_bar;
        self.op8_snapshot.recording = recording;
        if self.mode == LedOperatingMode::Op8Cockpit {
            let snap = self.op8_snapshot.clone();
            self.apply_op8_cockpit_snapshot(&snap);
        }
    }

    /// Advances `Op8Cockpit` flash timers by one tick and updates any expiring Rec/Cycle LEDs.
    pub fn tick_op8_animations(&mut self) {
        if self.op8_snapshot.advance_flash_timers() && self.mode == LedOperatingMode::Op8Cockpit {
            let snap = self.op8_snapshot.clone();
            self.apply_op8_cockpit_snapshot(&snap);
        }
    }

    // -------------------------------------------------------------------------
    // Dirty-Flag Diff Coalescing & Full-Panel Resynchronization
    // -------------------------------------------------------------------------

    /// Encodes a single LED state into its device-specific [`RawMidiPacket`] (`CC` on Ch 1 for
    /// Korg `nanoKONTROL2`, `Note On` on Ch 1 for Akai `MIDI Mix`).
    #[must_use]
    pub const fn encode_led_packet(&self, midi_number: u8, is_on: bool) -> RawMidiPacket {
        let protocol = self.device_def.led_protocol;
        let value = if is_on {
            protocol.on_value
        } else {
            protocol.off_value
        };
        match protocol.message_type {
            MidiMessageType::Cc => RawMidiPacket::control_change(protocol.channel, midi_number, value),
            MidiMessageType::Note => RawMidiPacket::note_on(protocol.channel, midi_number, value),
        }
    }

    /// Flushes only the LEDs whose `dirty` flag is set (`last_transmitted_state != Some(desired_state)`),
    /// clearing their `dirty` flags and returning the minimal sequence of [`RawMidiPacket`]s.
    pub fn flush_dirty_packets(&mut self) -> Vec<RawMidiPacket> {
        let protocol = self.device_def.led_protocol;
        let mut packets = Vec::new();

        for entry in &mut self.leds {
            if entry.dirty {
                let value = if entry.desired_state {
                    protocol.on_value
                } else {
                    protocol.off_value
                };
                let pkt = match protocol.message_type {
                    MidiMessageType::Cc => {
                        RawMidiPacket::control_change(protocol.channel, entry.midi_number, value)
                    }
                    MidiMessageType::Note => {
                        RawMidiPacket::note_on(protocol.channel, entry.midi_number, value)
                    }
                };
                entry.last_transmitted_state = Some(entry.desired_state);
                entry.dirty = false;
                packets.push(pkt);
            }
        }

        self.total_packets_transmitted += packets.len() as u64;
        packets
    }

    /// Marks every LED on the controller as dirty and immediately flushes all 28 (nanoKONTROL2)
    /// or 24 (Akai MIDImix) packets to guarantee complete hardware panel synchronization.
    pub fn resync_full_panel(&mut self) -> Vec<RawMidiPacket> {
        for entry in &mut self.leds {
            entry.mark_dirty_for_resync();
        }
        self.flush_dirty_packets()
    }

    /// Triggered upon USB hot-plug reconnection: restores desired states from `pipeline` (if in
    /// `Internal` mode) or `op8_snapshot` (if in `Op8Cockpit` mode) and transmits a full-panel
    /// LED resynchronization burst.
    pub fn on_hotplug_reconnect(
        &mut self,
        pipeline: Option<&DeviceMappingPipeline>,
    ) -> Vec<RawMidiPacket> {
        match self.mode {
            LedOperatingMode::Internal => {
                if let Some(p) = pipeline {
                    self.sync_from_pipeline(p);
                }
            }
            LedOperatingMode::External => {}
            LedOperatingMode::Op8Cockpit => {
                let snap = self.op8_snapshot.clone();
                self.apply_op8_cockpit_snapshot(&snap);
            }
        }
        self.resync_full_panel()
    }

    /// Triggered when a new preset is loaded: synchronizes button LED states from `pipeline`
    /// (in `Internal` mode) and performs a full-panel LED resynchronization.
    pub fn on_preset_loaded(&mut self, pipeline: &DeviceMappingPipeline) -> Vec<RawMidiPacket> {
        if self.mode == LedOperatingMode::Internal {
            self.sync_from_pipeline(pipeline);
        }
        self.resync_full_panel()
    }

    /// Turns all hardware LEDs off (`desired_state = false`) and returns the full-panel
    /// shutdown packet burst so no LEDs remain lit after disconnect/exit.
    pub fn all_off(&mut self) -> Vec<RawMidiPacket> {
        for entry in &mut self.leds {
            entry.desired_state = false;
            entry.mark_dirty_for_resync();
        }
        self.flush_dirty_packets()
    }
}

// =============================================================================
// 4. Simultaneous Multi-Device LED Manager
// =============================================================================

/// Coordinates [`DeviceLedController`] instances across all connected and remembered
/// hardware MIDI controllers.
#[derive(Debug, Clone, Default)]
pub struct MultiDeviceLedManager {
    /// Per-device LED controllers keyed by `device_key` (e.g. `HardwareFingerprint::canonical_key()`).
    pub controllers: BTreeMap<String, DeviceLedController>,
}

impl MultiDeviceLedManager {
    /// Creates an empty [`MultiDeviceLedManager`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or returns an existing [`DeviceLedController`] for `device_key` and `definition_id`.
    pub fn ensure_controller(
        &mut self,
        device_key: &str,
        definition_id: &str,
    ) -> Option<&mut DeviceLedController> {
        if !self.controllers.contains_key(device_key) {
            let def = get_device_definition(definition_id)?;
            self.controllers.insert(
                device_key.to_string(),
                DeviceLedController::new(device_key, def),
            );
        }
        self.controllers.get_mut(device_key)
    }

    /// Flushes dirty LED packets across all managed controllers, returning `(device_key, packet)` pairs.
    pub fn flush_all_dirty(&mut self) -> Vec<(String, RawMidiPacket)> {
        let mut out = Vec::new();
        for (key, ctrl) in &mut self.controllers {
            for pkt in ctrl.flush_dirty_packets() {
                out.push((key.clone(), pkt));
            }
        }
        out
    }

    /// Handles a [`HotplugEvent`] from [`crate::midi_io::MultiDeviceManager`], automatically
    /// performing a full-panel LED resynchronization when a controller connects or reconnects.
    pub fn handle_hotplug_event(
        &mut self,
        event: &HotplugEvent,
        pipeline: Option<&DeviceMappingPipeline>,
    ) -> Vec<(String, RawMidiPacket)> {
        match event {
            HotplugEvent::ConnectedNew { fingerprint, .. }
            | HotplugEvent::ReconnectedRemembered { fingerprint, .. } => {
                let key = fingerprint.canonical_key();
                if let Some(ctrl) = self.controllers.get_mut(&key) {
                    return ctrl
                        .on_hotplug_reconnect(pipeline)
                        .into_iter()
                        .map(|pkt| (key.clone(), pkt))
                        .collect();
                }
                Vec::new()
            }
            HotplugEvent::Disconnected { .. } => Vec::new(),
        }
    }
}

// =============================================================================
// Unit & Integration Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::HardwareFingerprint;
    use crate::mapping_engine::DeviceMappingConfig;

    #[test]
    fn test_device_specific_led_protocols_nanokontrol2_28_cc_vs_midimix_24_note() {
        let mut nk2_leds = DeviceLedController::new_nanokontrol2("0944:0117::nanoKONTROL2");
        let mut midimix_leds = DeviceLedController::new_akai_midimix("09e8:0031::MIDI Mix");

        assert_eq!(nk2_leds.led_count(), 28);
        assert_eq!(midimix_leds.led_count(), 24);

        // Full panel resync on nanoKONTROL2 emits 28 CC messages on Channel 1 (status 0xB0)
        assert!(nk2_leds.set_led_by_name("track1.solo", true));
        assert!(nk2_leds.set_led_by_name("transport.play", true));
        let nk2_burst = nk2_leds.resync_full_panel();
        assert_eq!(nk2_burst.len(), 28);
        assert!(nk2_burst.iter().all(|p| p.status == 0xB0));
        // Verify track1.solo (CC 32) is ON (127) and transport.play (CC 41) is ON (127)
        assert!(nk2_burst.contains(&RawMidiPacket::control_change(1, 32, 127)));
        assert!(nk2_burst.contains(&RawMidiPacket::control_change(1, 41, 127)));
        assert!(nk2_burst.contains(&RawMidiPacket::control_change(1, 48, 0)));

        // Full panel resync on Akai MIDI Mix emits 24 Note On messages on Channel 1 (status 0x90)
        assert!(midimix_leds.set_led_by_name("track1.mute", true)); // Note 1
        assert!(midimix_leds.set_led_by_name("track8.rec_arm", true)); // Note 24
        let midimix_burst = midimix_leds.resync_full_panel();
        assert_eq!(midimix_burst.len(), 24);
        assert!(midimix_burst.iter().all(|p| p.status == 0x90));
        assert!(midimix_burst.contains(&RawMidiPacket::note_on(1, 1, 127)));
        assert!(midimix_burst.contains(&RawMidiPacket::note_on(1, 24, 127)));
        assert!(midimix_burst.contains(&RawMidiPacket::note_on(1, 2, 0)));
    }

    #[test]
    fn test_dirty_flag_diff_coalescing_transmits_only_changed_leds() {
        let mut nk2 = DeviceLedController::new_nanokontrol2("nk2");

        // Initially all LEDs are clean (last_transmitted_state == Some(false), desired == false)
        assert_eq!(nk2.dirty_count(), 0);
        assert!(nk2.flush_dirty_packets().is_empty());

        // Set track1.solo (CC 32) to true 5 times in a row -> only 1 dirty LED
        for _ in 0..5 {
            assert!(nk2.set_led_by_name("track1.solo", true));
        }
        assert_eq!(nk2.dirty_count(), 1);
        assert_eq!(nk2.coalesced_updates_skipped, 4);

        // Toggle track2.mute (CC 49) false -> true -> false before flushing -> net change is 0!
        assert!(nk2.set_led_by_name("track2.mute", true));
        assert_eq!(nk2.dirty_count(), 2);
        assert!(nk2.set_led_by_name("track2.mute", false));
        assert_eq!(
            nk2.dirty_count(),
            1,
            "Oscillating false -> true -> false before flush must coalesce to clean"
        );

        // Flush transmits ONLY the 1 actually changed LED (track1.solo CC 32 = 127)
        let flushed = nk2.flush_dirty_packets();
        assert_eq!(flushed, vec![RawMidiPacket::control_change(1, 32, 127)]);
        assert_eq!(nk2.dirty_count(), 0);
        assert_eq!(nk2.total_packets_transmitted, 1);

        // Subsequent flush without changes emits 0 packets
        assert!(nk2.flush_dirty_packets().is_empty());
    }

    #[test]
    fn test_three_operating_modes_internal_external_and_op8_cockpit() {
        let mut nk2_leds = DeviceLedController::new_nanokontrol2("nk2");
        let mut pipeline =
            DeviceMappingPipeline::new("nk2", DeviceMappingConfig::default_nanokontrol2());

        // --- Mode 1: INTERNAL ---
        assert_eq!(nk2_leds.mode, LedOperatingMode::Internal);
        // Press track1.solo (CC 32, Toggle) -> emits ButtonStateChangeEvent -> lights CC 32 LED
        if let crate::mapping_engine::MappingEvaluation::Emitted {
            button_event: Some(ev),
            ..
        } = pipeline.evaluate_message(MidiMessageType::Cc, 1, 32, 127)
        {
            assert!(nk2_leds.handle_internal_button_event(&ev));
        }
        let internal_pkts = nk2_leds.flush_dirty_packets();
        assert_eq!(
            internal_pkts,
            vec![RawMidiPacket::control_change(1, 32, 127)]
        );

        // External MIDI feedback is ignored while in Internal mode
        assert!(!nk2_leds.handle_external_midi_packet(RawMidiPacket::control_change(1, 48, 127)));
        assert_eq!(nk2_leds.dirty_count(), 0);

        // --- Mode 2: EXTERNAL ---
        nk2_leds.set_mode(LedOperatingMode::External);
        // Local button press is ignored in External mode
        if let crate::mapping_engine::MappingEvaluation::Emitted {
            button_event: Some(ev),
            ..
        } = pipeline.evaluate_message(MidiMessageType::Cc, 1, 49, 127)
        {
            assert!(!nk2_leds.handle_internal_button_event(&ev));
        }
        assert_eq!(nk2_leds.dirty_count(), 0);

        // Incoming DAW CC 48 = 127 and OSC "/nkeditor/led/set transport.record true" drive LEDs
        assert!(nk2_leds.handle_external_midi_packet(RawMidiPacket::control_change(1, 48, 127)));
        assert!(nk2_leds.handle_external_osc_led("transport.record", true));
        let external_pkts = nk2_leds.flush_dirty_packets();
        assert_eq!(
            external_pkts,
            vec![
                RawMidiPacket::control_change(1, 48, 127),
                RawMidiPacket::control_change(1, 45, 127),
            ]
        );

        // --- Mode 3: 8opPerc COCKPIT ---
        nk2_leds.set_mode(LedOperatingMode::Op8Cockpit);
        let mut snap = Op8CockpitLedSnapshot::default();
        snap.playing = true;
        snap.recording = true;
        snap.beat_in_bar = 0; // Downbeat -> Cycle LED ON
        snap.lanes[0].soloed = true; // Kick (track 1) soloed -> CC 32 ON
        snap.lanes[2].muted = true; // Hat Closed (track 3) muted -> CC 50 ON
        nk2_leds.apply_op8_cockpit_snapshot(&snap);

        // Trigger a 2-tick timbre reroll flash on Snare (lane 1 -> track2.rec CC 65)
        nk2_leds.trigger_op8_lane_reroll(1, 2);
        let op8_pkts = nk2_leds.flush_dirty_packets();
        assert!(op8_pkts.contains(&RawMidiPacket::control_change(1, 50, 127))); // track3.mute ON
        assert!(op8_pkts.contains(&RawMidiPacket::control_change(1, 65, 127))); // track2.rec flash ON
        assert!(op8_pkts.contains(&RawMidiPacket::control_change(1, 41, 127))); // transport.play ON
        assert!(op8_pkts.contains(&RawMidiPacket::control_change(1, 46, 127))); // transport.cycle downbeat ON

        // Advance beat to beat 1 (off downbeat) -> Cycle LED turns OFF
        nk2_leds.update_op8_transport_beat(true, 1, true);
        let beat1_pkts = nk2_leds.flush_dirty_packets();
        assert_eq!(beat1_pkts, vec![RawMidiPacket::control_change(1, 46, 0)]);

        // Tick 1 of snare reroll flash -> still > 0 (1 tick left)
        nk2_leds.tick_op8_animations();
        assert!(nk2_leds.flush_dirty_packets().is_empty());

        // Tick 2 of snare reroll flash -> expires to 0 -> track2.rec (CC 65) automatically turns OFF!
        nk2_leds.tick_op8_animations();
        let flash_off_pkts = nk2_leds.flush_dirty_packets();
        assert_eq!(
            flash_off_pkts,
            vec![RawMidiPacket::control_change(1, 65, 0)]
        );
    }

    #[test]
    fn test_hotplug_reconnection_and_preset_load_full_panel_resync() {
        let mut manager = MultiDeviceLedManager::new();
        let fp = HardwareFingerprint::new("0944", "0117", "", "nanoKONTROL2");
        let key = fp.canonical_key();

        let ctrl = manager
            .ensure_controller(&key, "nanokontrol2")
            .expect("nanokontrol2 registered");
        ctrl.set_led_by_name("track1.solo", true);
        ctrl.set_led_by_name("track4.mute", true);
        let initial_diff = ctrl.flush_dirty_packets();
        assert_eq!(initial_diff.len(), 2);

        // Simulate hot-plug reconnection on a different physical USB port (ALSA 24:0 -> 32:0)
        let reconnect_event = HotplugEvent::ReconnectedRemembered {
            fingerprint: fp,
            previous_alsa_seq_addr: Some("24:0".to_string()),
            new_alsa_seq_addr: Some("32:0".to_string()),
            previous_by_path: Some("usb-0:5.3:1.0".to_string()),
            new_by_path: Some("usb-0:2.1:1.0".to_string()),
            restored_preset: "8opperc-live".to_string(),
            led_sync_packet_count: 28,
        };

        let resync_packets = manager.handle_hotplug_event(&reconnect_event, None);
        assert_eq!(
            resync_packets.len(),
            28,
            "Hot-plug reconnect must transmit all 28 nanoKONTROL2 LED packets"
        );
        assert!(resync_packets.contains(&(
            key.clone(),
            RawMidiPacket::control_change(1, 32, 127)
        )));
        assert!(resync_packets.contains(&(
            key.clone(),
            RawMidiPacket::control_change(1, 51, 127)
        )));

        // Verify all_off turns off all 28 LEDs cleanly
        let off_pkts = manager.controllers.get_mut(&key).unwrap().all_off();
        assert_eq!(off_pkts.len(), 28);
        assert!(off_pkts.iter().all(|p| p.data2 == 0));
    }
}
