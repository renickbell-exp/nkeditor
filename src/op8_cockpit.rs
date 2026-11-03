//! Interactive `8opPerc` Performance Cockpit View & Form Navigation Deck (`src/op8_cockpit.rs`).
//!
//! Renders two coordinated performance regions styled with `yaae-theme` and `twoBowls` crisp
//! zero-rounding borders (`egui::Rounding::ZERO`) and [`crate::theme::AccentPalette`] highlights:
//!
//! 1. **Form Bank & Performance Navigation Deck**:
//!    - **Searchable Form Bank Browser**: Displays the active form (e.g. `gqom_drive`),
//!      4 hydrated neighbor forms (`FORM_BANK_PREFETCH`), canonical `8operc-konduktiva` tags
//!      and descriptors (`motif`, `phase`, `section_role`, `density_curve`), search filter
//!      across all 4,096 forms, and one-click `Previous [CC58]`, `Next [CC59]`, and `Queue Form`
//!      controls synced with `nanoKONTROL2` `Track Left/Right` (`CC 58/59`).
//!    - **Section & Bar Timeline Progress Bar**: Shows current `BPM`, bar/beat phase
//!      (`Beat 1..4` cells + continuous bar and 16-bar section progress bars), and pending
//!      next-bar timbre/form latch indicators.
//!    - **Interactive Global Density & Rhythmic Feel (Swing / Humanize) XY Control & Readout**:
//!      2D vector XY pad mapping X -> Global Density (`0.1x ..= 5.0x`, `CC 61/62`) and
//!      Y -> Global Feel Scale (`0.0 ..= 1.0` with exact midpoint `CC 64 = 0.5`, `CC 43/44`),
//!      paired with tabular Swing and Humanize readouts.
//!
//! 2. **8-Column Percussion Voice Matrix**:
//!    - Aligned 1:1 with the 8 physical tracks of the Korg `nanoKONTROL2` (`T1 KICK` .. `T8 CYMBAL`),
//!      displaying each lane's family name, active timbre badge, queued next-bar timbre indicator,
//!      interactive density ladder meter (`0.1x` to `5.0x`), level/decay macro slider,
//!      Solo/Mute/Rec LED buttons, Timbre Lock toggle, and one-click Timbre Reroll button.

use crate::led_controller::{
    LedOperatingMode, MultiDeviceLedManager, Op8VoiceLedState, DEFAULT_REROLL_FLASH_TICKS,
};
use crate::op8_model::{
    cc_to_density, cc_to_feel_scale, density_to_cc, feel_scale_to_cc, FormDirection,
    Op8PerformanceAction, Op8PerformanceModel, Op8PresetSnapshot, FORM_BANK_SIZE,
    MAX_DENSITY_SCALE, MIN_DENSITY_SCALE, NK2_DENSITY_KNOB_CCS, NK2_GAIN_SLIDER_CCS,
    NK2_MUTE_BUTTON_CCS, NK2_REC_REROLL_CCS, NK2_SOLO_BUTTON_CCS,
};
use crate::osc_server::OscServer;
use crate::theme::{AccentPalette, ThemeSettings};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use rosc::{OscMessage, OscType};
use serde::{Deserialize, Serialize};

// =============================================================================
// 1. Canonical 8operc-konduktiva Form Bank Catalogue & Descriptors
// =============================================================================

/// Canonical base form archetypes from `8operc-konduktiva/src/pulsed-form-change-factory.js`.
pub const PULSED_FORM_ARCHETYPES: [&str; 10] = [
    "gqom_drive",
    "foundry",
    "tidal",
    "half_light",
    "orbit",
    "fracture",
    "footwork_drive",
    "juke_drive",
    "kuduro_drive",
    "singeli_drive",
];

/// Canonical motifs per form archetype from `pulsed-form-change-factory.js`.
pub const PULSED_FORM_MOTIFS: [[&str; 4]; 10] = [
    ["pressure", "interlock", "grind", "township"],
    ["press", "furnace", "relay", "vent"],
    ["undertow", "current", "wake", "shore"],
    ["shadow", "weight", "halo", "afterimage"],
    ["signal", "weave", "apogee", "eclipse"],
    ["fault", "shear", "split", "resolve"],
    ["battle", "swerve", "strobe", "rush"],
    ["jack", "swerve", "cut", "lock"],
    ["motion", "heat", "turn", "lattice"],
    ["torrent", "flash", "flare", "breach"],
];

/// Canonical name phases from `pulsed-form-change-factory.js`.
pub const PULSED_NAME_PHASES: [&str; 5] = ["arc", "return", "lift", "turn", "release"];

/// Canonical section roles from `pulsed-form-change-factory.js`.
pub const PULSED_SECTION_ROLES: [&str; 5] = ["anchor", "lift", "pressure", "break", "return"];

/// Canonical density curves from `pulsed-form-change-factory.js`.
pub const PULSED_DENSITY_CURVES: [&str; 5] = ["terraced", "waves", "arc", "rise", "spike"];

/// Rich metadata descriptor for a single entry in the 4,096-form catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormBankEntryDescriptor {
    /// 0-based index in the 4,096-entry form reservoir (`0..4096`).
    pub index: usize,
    /// Canonical form slot ID (e.g. `"form-0001"`).
    pub id: String,
    /// Base form archetype (e.g. `"gqom_drive"`, `"footwork_drive"`).
    pub base_form: String,
    /// Full descriptive name (e.g. `"gqom_drive-pressure-arc-0001"`).
    pub display_name: String,
    /// Structural motif tag (`"pressure"`, `"interlock"`, etc.).
    pub motif: String,
    /// Structural phase tag (`"arc"`, `"return"`, `"lift"`, etc.).
    pub phase: String,
    /// Section role (`"anchor"`, `"lift"`, `"pressure"`, `"break"`, `"return"`).
    pub section_role: String,
    /// Density curve shape (`"terraced"`, `"waves"`, `"arc"`, `"rise"`, `"spike"`).
    pub density_curve: String,
    /// Searchable tags combining archetype, motif, phase, section role, and density curve.
    pub tags: Vec<String>,
}

impl FormBankEntryDescriptor {
    /// Case-insensitive search match against form index, ID, archetype, display name, or tags.
    #[must_use]
    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return true;
        }
        self.id.to_ascii_lowercase().contains(&q)
            || self.base_form.to_ascii_lowercase().contains(&q)
            || self.display_name.to_ascii_lowercase().contains(&q)
            || self.tags.iter().any(|t| t.to_ascii_lowercase().contains(&q))
    }
}

/// Deterministically generates the [`FormBankEntryDescriptor`] for `index` (`0..4096`).
#[must_use]
pub fn describe_form_entry(index: usize) -> FormBankEntryDescriptor {
    let idx = index % FORM_BANK_SIZE;
    let arch_idx = idx % PULSED_FORM_ARCHETYPES.len();
    let base_form = PULSED_FORM_ARCHETYPES[arch_idx];
    let motif = PULSED_FORM_MOTIFS[arch_idx][(idx / 10) % 4];
    let phase = PULSED_NAME_PHASES[(idx / 40) % PULSED_NAME_PHASES.len()];
    let section_role = PULSED_SECTION_ROLES[(idx / 8) % PULSED_SECTION_ROLES.len()];
    let density_curve = PULSED_DENSITY_CURVES[(idx / 16) % PULSED_DENSITY_CURVES.len()];
    let id = format!("form-{:04}", idx + 1);
    let display_name = if idx == 0 {
        "gqom_drive".to_string()
    } else {
        format!("{base_form}-{motif}-{phase}-{:04}", idx + 1)
    };
    let tags = vec![
        base_form.to_string(),
        motif.to_string(),
        phase.to_string(),
        section_role.to_string(),
        density_curve.to_string(),
    ];
    FormBankEntryDescriptor {
        index: idx,
        id,
        base_form: base_form.to_string(),
        display_name,
        motif: motif.to_string(),
        phase: phase.to_string(),
        section_role: section_role.to_string(),
        density_curve: density_curve.to_string(),
        tags,
    }
}

// =============================================================================
// 2. Cockpit View State & Bidirectional OSC / LED Controller Wiring
// =============================================================================

/// Persistent state for the `8opPerc` Performance Cockpit view in `nkEditor4`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op8CockpitViewState {
    /// Complete `8opPerc` performance domain model (8 lanes, form bank, timbre bank, feel, transport).
    pub model: Op8PerformanceModel,
    /// Active hardware device key (e.g. `"0944:0117:noserial:nanoKONTROL2"`) whose LEDs mirror cockpit state.
    pub active_device_key: String,
    /// Currently focused voice lane index (`0..8`) for keyboard/inspector focus.
    pub selected_lane: usize,
    /// Draft label for the next atomic 8-voice preset pool snapshot (`Marker Set CC 60`).
    pub snapshot_label_draft: String,
    /// Search query string for filtering the 4,096-entry Form Bank Browser.
    pub form_search_query: String,
    /// Optional form index queued to latch at the next bar downbeat.
    pub queued_form_index: Option<usize>,
    /// Number of bars per structural section in the Section & Bar Timeline (default `16`).
    pub section_total_bars: u32,
    /// Recent cockpit interaction log for telemetry inspection.
    pub action_log: Vec<String>,
}

impl Default for Op8CockpitViewState {
    fn default() -> Self {
        Self::new("0944:0117:noserial:nanoKONTROL2")
    }
}

impl Op8CockpitViewState {
    /// Creates a new `Op8CockpitViewState` bound to `active_device_key`.
    pub fn new(active_device_key: impl Into<String>) -> Self {
        let mut model = Op8PerformanceModel::new();
        model.form_bank.active_form_name = "gqom_drive".to_string();
        Self {
            model,
            active_device_key: active_device_key.into(),
            selected_lane: 0,
            snapshot_label_draft: String::new(),
            form_search_query: String::new(),
            queued_form_index: None,
            section_total_bars: 16,
            action_log: Vec::new(),
        }
    }

    fn push_log(&mut self, entry: String) {
        self.action_log.push(entry);
        if self.action_log.len() > 64 {
            self.action_log.remove(0);
        }
    }

    /// Returns the rich [`FormBankEntryDescriptor`] for the currently active form.
    #[must_use]
    pub fn active_form_descriptor(&self) -> FormBankEntryDescriptor {
        let mut desc = describe_form_entry(self.model.form_bank.active_index);
        if !self.model.form_bank.active_form_name.is_empty() {
            desc.display_name = self.model.form_bank.active_form_name.clone();
        }
        desc
    }

    /// Returns the descriptors of the 4 hydrated neighbor forms (`FORM_BANK_PREFETCH = 4`).
    #[must_use]
    pub fn hydrated_neighbor_forms(&self) -> Vec<FormBankEntryDescriptor> {
        self.model
            .form_bank
            .prefetched_indices()
            .into_iter()
            .map(describe_form_entry)
            .collect()
    }

    /// Searches the 4,096-entry form bank using `self.form_search_query`, returning up to
    /// `max_results` matching [`FormBankEntryDescriptor`] entries.
    #[must_use]
    pub fn search_forms(&self, max_results: usize) -> Vec<FormBankEntryDescriptor> {
        let limit = max_results.max(1);
        let mut out = Vec::with_capacity(limit);
        for idx in 0..self.model.form_bank.total_entries {
            let desc = describe_form_entry(idx);
            if desc.matches_query(&self.form_search_query) {
                out.push(desc);
                if out.len() >= limit {
                    break;
                }
            }
        }
        out
    }

    /// Returns the number of voice lanes that currently have a pending `queued_timbre` waiting
    /// for the next bar downbeat.
    #[must_use]
    pub fn pending_timbre_lane_count(&self) -> usize {
        self.model
            .lanes
            .iter()
            .filter(|l| l.queued_timbre.is_some())
            .count()
    }

    /// Computes `(current_bar_1_based, beat_in_bar_0_based, bar_progress_0_to_1, section_progress_0_to_1, section_role)`.
    #[must_use]
    pub fn timeline_progress(&self) -> (u32, f64, f32, f32, String) {
        let beats_per_bar = f64::from(self.model.transport.beats_per_bar.max(1));
        let cur_beat = self.model.transport.current_beat.max(0.0);
        let total_bars_elapsed = (cur_beat / beats_per_bar).floor() as u32;
        let sec_bars = self.section_total_bars.max(1);
        let bar_in_section = (total_bars_elapsed % sec_bars) + 1;
        let beat_in_bar = cur_beat % beats_per_bar;
        let bar_progress = (beat_in_bar / beats_per_bar).clamp(0.0, 1.0) as f32;
        let section_beats = f64::from(sec_bars) * beats_per_bar;
        let section_progress = ((cur_beat % section_beats) / section_beats).clamp(0.0, 1.0) as f32;
        let role_idx = ((total_bars_elapsed / 4) as usize) % PULSED_SECTION_ROLES.len();
        (
            bar_in_section,
            beat_in_bar,
            bar_progress,
            section_progress,
            PULSED_SECTION_ROLES[role_idx].to_string(),
        )
    }

    /// Builds an [`crate::led_controller::Op8CockpitLedSnapshot`] from the current `self.model`
    /// state and applies it to the active device's [`crate::led_controller::DeviceLedController`]
    /// in [`LedOperatingMode::Op8Cockpit`].
    pub fn sync_leds(&self, led_manager: &mut MultiDeviceLedManager) {
        let Some(ctrl) = led_manager.controllers.get_mut(&self.active_device_key) else {
            return;
        };
        if ctrl.mode != LedOperatingMode::Op8Cockpit {
            ctrl.set_mode(LedOperatingMode::Op8Cockpit);
        }
        let mut snap = ctrl.op8_snapshot.clone();
        snap.playing = self.model.transport.playing;
        snap.recording = self.model.transport.recording_wav;
        snap.beat_in_bar =
            (self.model.transport.current_beat as u8) % self.model.transport.beats_per_bar.max(1);
        for i in 0..8 {
            let lane = &self.model.lanes[i];
            snap.lanes[i] = Op8VoiceLedState {
                soloed: lane.soloed,
                muted: lane.muted,
                reroll_flash_ticks: if lane.queued_timbre.is_some() {
                    snap.lanes[i].reroll_flash_ticks.max(1)
                } else {
                    snap.lanes[i].reroll_flash_ticks
                },
            };
        }
        ctrl.apply_op8_cockpit_snapshot(&snap);
    }

    /// Broadcasts `/nkeditor/op8/state` via [`OscServer`].
    pub fn sync_and_broadcast_osc_state(&self, osc_server: &mut Option<OscServer>) {
        let rig_state = self.model.to_osc_rig_state();
        if let Some(server) = osc_server {
            server.broadcast_op8_state(&rig_state);
        }
    }

    // -------------------------------------------------------------------------
    // Interactive Cockpit Mutations (Bidirectionally Wired to OSC & LEDs)
    // -------------------------------------------------------------------------

    /// Sets per-lane density multiplier (`0.1 ..= 5.0`), updates `OscServer`, and emits
    /// `/nkeditor/op8/lane/<index>/density` + `/nkeditor/op8/state`.
    pub fn set_lane_density(
        &mut self,
        lane_idx: usize,
        density: f64,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> Option<f64> {
        let lane = self.model.lanes.get_mut(lane_idx)?;
        let applied = lane.set_density_multiplier(density);
        let fam = lane.family.clone();
        self.push_log(format!("lane[{lane_idx}:{fam}] density -> {applied:.2}x"));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: format!("/nkeditor/op8/lane/{lane_idx}/density"),
                args: vec![OscType::Float(applied as f32)],
            });
        }
        self.sync_leds(led_manager);
        self.sync_and_broadcast_osc_state(osc_server);
        Some(applied)
    }

    /// Sets per-lane macro/gain slider (`0..=127`), updates `OscServer`, and broadcasts state.
    pub fn set_lane_gain_cc(
        &mut self,
        lane_idx: usize,
        gain_cc: u8,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> Option<u8> {
        let lane = self.model.lanes.get_mut(lane_idx)?;
        lane.set_gain_from_cc(gain_cc);
        let applied = lane.gain_cc;
        let fam = lane.family.clone();
        self.push_log(format!("lane[{lane_idx}:{fam}] gain_cc -> {applied}"));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/control/set".to_string(),
                args: vec![
                    OscType::String(format!("track{}.slider", lane_idx + 1)),
                    OscType::Int(i32::from(applied)),
                ],
            });
        }
        self.sync_leds(led_manager);
        self.sync_and_broadcast_osc_state(osc_server);
        Some(applied)
    }

    /// Toggles per-lane Solo state (`trackN.solo`), updates hardware Solo LED (`CC 32..39`),
    /// and broadcasts `/nkeditor/op8/lane/<index>/solo`.
    pub fn toggle_lane_solo(
        &mut self,
        lane_idx: usize,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> Option<bool> {
        let lane = self.model.lanes.get_mut(lane_idx)?;
        lane.soloed = !lane.soloed;
        let soloed = lane.soloed;
        let fam = lane.family.clone();
        self.push_log(format!("lane[{lane_idx}:{fam}] solo -> {soloed}"));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: format!("/nkeditor/op8/lane/{lane_idx}/solo"),
                args: vec![OscType::Int(i32::from(soloed))],
            });
        }
        self.sync_leds(led_manager);
        self.sync_and_broadcast_osc_state(osc_server);
        Some(soloed)
    }

    /// Toggles per-lane Mute state (`trackN.mute`), updates hardware Mute LED (`CC 48..55`),
    /// and broadcasts `/nkeditor/op8/lane/<index>/mute`.
    pub fn toggle_lane_mute(
        &mut self,
        lane_idx: usize,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> Option<bool> {
        let lane = self.model.lanes.get_mut(lane_idx)?;
        lane.muted = !lane.muted;
        let muted = lane.muted;
        let fam = lane.family.clone();
        self.push_log(format!("lane[{lane_idx}:{fam}] mute -> {muted}"));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: format!("/nkeditor/op8/lane/{lane_idx}/mute"),
                args: vec![OscType::Int(i32::from(muted))],
            });
        }
        self.sync_leds(led_manager);
        self.sync_and_broadcast_osc_state(osc_server);
        Some(muted)
    }

    /// Toggles per-lane Timbre Lock (`timbre_locked`), preventing global timbre set changes
    /// (`Cycle CC 46` or `advance_full_timbre_set`) from overwriting this lane's timbre.
    pub fn toggle_lane_timbre_lock(&mut self, lane_idx: usize) -> Option<bool> {
        let lane = self.model.lanes.get_mut(lane_idx)?;
        lane.timbre_locked = !lane.timbre_locked;
        let locked = lane.timbre_locked;
        let fam = lane.family.clone();
        self.push_log(format!("lane[{lane_idx}:{fam}] timbre_locked -> {locked}"));
        Some(locked)
    }

    /// Rerolls a single voice lane's timbre (`trackN.rec`, `CC 64..71`), flashes the physical
    /// `Rec` LED on the Korg `nanoKONTROL2`, and broadcasts `/nkeditor/op8/timbre`.
    pub fn reroll_lane_timbre(
        &mut self,
        lane_idx: usize,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> Option<String> {
        let (active, queued) = self.model.reroll_lane_timbre(lane_idx)?;
        let chosen = queued.unwrap_or(active);
        let fam = self.model.lanes[lane_idx].family.clone();
        self.push_log(format!("lane[{lane_idx}:{fam}] reroll -> {chosen}"));

        if let Some(ctrl) = led_manager.controllers.get_mut(&self.active_device_key) {
            ctrl.set_mode(LedOperatingMode::Op8Cockpit);
            ctrl.trigger_op8_lane_reroll(lane_idx, DEFAULT_REROLL_FLASH_TICKS);
        }
        self.sync_leds(led_manager);

        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/op8/timbre".to_string(),
                args: vec![
                    OscType::String(lane_idx.to_string()),
                    OscType::String(chosen.clone()),
                ],
            });
        }
        self.sync_and_broadcast_osc_state(osc_server);
        Some(chosen)
    }

    /// Advances to the next full 8-voice timbre set (`Cycle CC 46`), skipping any lanes whose
    /// `timbre_locked == true`, flashes hardware `Rec` + `Cycle` LEDs, and broadcasts `/nkeditor/op8/timbre all`.
    pub fn advance_full_timbre_set(
        &mut self,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> (usize, [String; 8]) {
        let (set_idx, timbres) = self.model.advance_full_timbre_set();
        self.push_log(format!("timbre_set -> #{}", set_idx + 1));

        if let Some(ctrl) = led_manager.controllers.get_mut(&self.active_device_key) {
            ctrl.set_mode(LedOperatingMode::Op8Cockpit);
            ctrl.trigger_op8_all_lanes_reroll(DEFAULT_REROLL_FLASH_TICKS);
        }
        self.sync_leds(led_manager);

        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/op8/timbre".to_string(),
                args: vec![OscType::String("all".to_string())],
            });
        }
        self.sync_and_broadcast_osc_state(osc_server);
        (set_idx, timbres)
    }

    /// Commits all queued next-bar timbres and any queued form change (simulating a bar downbeat).
    pub fn commit_queued_timbres(
        &mut self,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> usize {
        let committed = self.model.commit_all_queued_timbres();
        if let Some(next_form_idx) = self.queued_form_index.take() {
            let desc = describe_form_entry(next_form_idx);
            self.model.form_bank.select_index(desc.index);
            self.model.form_bank.active_form_name = desc.display_name;
        }
        if let Some(ctrl) = led_manager.controllers.get_mut(&self.active_device_key) {
            for i in 0..8 {
                ctrl.op8_snapshot.lanes[i].reroll_flash_ticks = 0;
            }
        }
        self.sync_leds(led_manager);
        self.sync_and_broadcast_osc_state(osc_server);
        committed
    }

    /// Navigates the 4,096-entry form bank (`Previous` or `Next`, synced with `CC 58/59`) and
    /// broadcasts `/nkeditor/op8/form`.
    pub fn navigate_form(
        &mut self,
        direction: FormDirection,
        osc_server: &mut Option<OscServer>,
    ) -> (usize, String) {
        let (idx, _) = self.model.form_bank.step(direction);
        let desc = describe_form_entry(idx);
        self.model.form_bank.active_form_name = desc.display_name.clone();
        let dir_str = match direction {
            FormDirection::Previous => "previous",
            FormDirection::Next => "next",
        };
        self.push_log(format!("form[{dir_str}] -> {}", desc.display_name));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/op8/form".to_string(),
                args: vec![
                    OscType::String(dir_str.to_string()),
                    OscType::String(desc.display_name.clone()),
                ],
            });
        }
        self.sync_and_broadcast_osc_state(osc_server);
        (idx, desc.display_name)
    }

    /// Queues a specific form index (`0..4096`) to latch on the next bar downbeat and broadcasts
    /// `/nkeditor/op8/form`.
    pub fn queue_form_for_next_bar(
        &mut self,
        form_index: usize,
        osc_server: &mut Option<OscServer>,
    ) -> FormBankEntryDescriptor {
        let desc = describe_form_entry(form_index);
        self.queued_form_index = Some(desc.index);
        self.push_log(format!("queued_form -> {} (#{})", desc.display_name, desc.index + 1));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/op8/form".to_string(),
                args: vec![OscType::String(desc.display_name.clone())],
            });
        }
        self.sync_and_broadcast_osc_state(osc_server);
        desc
    }

    /// Updates both Global Density (`0.1 ..= 5.0`) and Global Feel Scale (`0.0 ..= 1.0`,
    /// exact midpoint `CC 64 = 0.5`) from the interactive 2D XY Pad and broadcasts OSC state.
    pub fn set_global_density_and_feel_xy(
        &mut self,
        density: f64,
        feel_scale: f64,
        osc_server: &mut Option<OscServer>,
    ) -> (f64, f64) {
        let applied_d = self.model.set_global_density_multiplier(density);
        let applied_f = self.model.feel.set_scale(feel_scale);
        self.push_log(format!(
            "xy_pad -> density={applied_d:.2}x feel={applied_f:.2} (CC={})",
            self.model.feel.feel_cc
        ));
        self.sync_and_broadcast_osc_state(osc_server);
        (applied_d, applied_f)
    }

    /// Advances the transport beat clock by `delta_beats` (`> 0.0`). Whenever a bar boundary
    /// (`beats_per_bar`) is crossed, automatically commits any pending next-bar timbres and
    /// queued form changes. Returns `true` if a bar boundary was crossed.
    pub fn advance_beat_phase(
        &mut self,
        delta_beats: f64,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> bool {
        if delta_beats <= 0.0 {
            return false;
        }
        let bar = f64::from(self.model.transport.beats_per_bar.max(1));
        let prev_bar = (self.model.transport.current_beat / bar).floor() as u64;
        self.model.transport.current_beat += delta_beats;
        let next_bar = (self.model.transport.current_beat / bar).floor() as u64;
        let crossed_bar = next_bar > prev_bar;
        if crossed_bar {
            self.commit_queued_timbres(led_manager, osc_server);
        } else {
            self.sync_leds(led_manager);
            self.sync_and_broadcast_osc_state(osc_server);
        }
        crossed_bar
    }

    /// Captures an atomic 8-voice preset snapshot (`Marker Set CC 60`) and broadcasts `/nkeditor/op8/snapshot/save`.
    pub fn capture_preset_snapshot(
        &mut self,
        label: Option<&str>,
        osc_server: &mut Option<OscServer>,
    ) -> Op8PresetSnapshot {
        let snap = self.model.capture_preset_snapshot(label);
        self.push_log(format!(
            "snapshot saved -> {} (bank slot {})",
            snap.label, snap.pool_bank_index
        ));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/op8/snapshot/save".to_string(),
                args: vec![OscType::String(snap.label.clone())],
            });
        }
        self.sync_and_broadcast_osc_state(osc_server);
        snap
    }

    /// Processes a physical Korg `nanoKONTROL2` CC event (`cc`, `value`), updates the cockpit,
    /// hardware LEDs, and broadcasts OSC telemetry.
    pub fn handle_physical_nanokontrol2_cc(
        &mut self,
        cc: u8,
        value: u8,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> Option<Op8PerformanceAction> {
        let action = self.model.apply_nanokontrol2_cc(cc, value)?;
        if let Some(ctrl) = led_manager.controllers.get_mut(&self.active_device_key) {
            ctrl.set_mode(LedOperatingMode::Op8Cockpit);
            match &action {
                Op8PerformanceAction::RerollLaneTimbre { lane, .. } => {
                    ctrl.trigger_op8_lane_reroll(*lane, DEFAULT_REROLL_FLASH_TICKS);
                }
                Op8PerformanceAction::AdvanceTimbreSet { .. } => {
                    ctrl.trigger_op8_all_lanes_reroll(DEFAULT_REROLL_FLASH_TICKS);
                }
                _ => {}
            }
        }
        self.sync_leds(led_manager);
        self.sync_and_broadcast_osc_state(osc_server);
        Some(action)
    }
}

// =============================================================================
// 3. Interactive egui Rendering (Form Deck + Percussion Voice Matrix)
// =============================================================================

/// Renders the complete interactive `8opPerc` Performance Cockpit inside `ui`.
pub fn render_op8_cockpit(
    ui: &mut Ui,
    cockpit: &mut Op8CockpitViewState,
    theme_settings: &ThemeSettings,
    led_manager: &mut MultiDeviceLedManager,
    osc_server: &mut Option<OscServer>,
) {
    let palette = theme_settings.resolved_palette();
    let visuals = theme_settings.build_visuals();
    let bg_color = visuals.panel_fill;
    let text_color = ui.visuals().text_color();
    let border_stroke = Stroke::new(1.0, palette.accent_weak);

    egui::Frame::none()
        .fill(bg_color)
        .stroke(border_stroke)
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // 1. Global Cockpit Header Bar
            render_cockpit_header_bar(ui, cockpit, &palette, text_color, led_manager, osc_server);

            ui.add_space(6.0);

            // 2. Form Bank & Performance Navigation Deck (Browser + Timeline + XY Pad)
            render_form_and_navigation_deck(
                ui,
                cockpit,
                &palette,
                text_color,
                led_manager,
                osc_server,
            );

            ui.add_space(8.0);

            // 3. 8-Column Percussion Voice Matrix (Aligned 1:1 with Korg nanoKONTROL2 Tracks 1..8)
            ui.columns(8, |columns| {
                for (lane_idx, col_ui) in columns.iter_mut().enumerate() {
                    render_voice_lane_column(
                        col_ui,
                        lane_idx,
                        cockpit,
                        &palette,
                        text_color,
                        led_manager,
                        osc_server,
                    );
                }
            });
        });
}

fn render_cockpit_header_bar(
    ui: &mut Ui,
    cockpit: &mut Op8CockpitViewState,
    palette: &AccentPalette,
    text_color: Color32,
    led_manager: &mut MultiDeviceLedManager,
    osc_server: &mut Option<OscServer>,
) {
    egui::Frame::none()
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("8OPPERC PERFORMANCE COCKPIT")
                        .font(FontId::monospace(13.0))
                        .strong()
                        .color(palette.accent_strong),
                );
                ui.separator();

                let play_label = if cockpit.model.transport.playing {
                    "PLAYING [CC41]"
                } else {
                    "PLAY [CC41]"
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(play_label).monospace().color(
                            if cockpit.model.transport.playing {
                                palette.accent_strong
                            } else {
                                text_color
                            },
                        ))
                        .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.model.transport.playing = true;
                    cockpit.sync_leds(led_manager);
                    cockpit.sync_and_broadcast_osc_state(osc_server);
                }

                if ui
                    .add(
                        egui::Button::new(RichText::new("STOP [CC42]").monospace())
                            .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.model.transport.playing = false;
                    cockpit.sync_leds(led_manager);
                    cockpit.sync_and_broadcast_osc_state(osc_server);
                }

                let rec_label = if cockpit.model.transport.recording_wav {
                    "WAV REC: ON [CC45]"
                } else {
                    "WAV REC: OFF [CC45]"
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(rec_label).monospace().color(
                            if cockpit.model.transport.recording_wav {
                                palette.strip_record_text
                            } else {
                                text_color
                            },
                        ))
                        .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.model.transport.recording_wav = !cockpit.model.transport.recording_wav;
                    cockpit.sync_leds(led_manager);
                    cockpit.sync_and_broadcast_osc_state(osc_server);
                }

                ui.separator();

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "CYCLE TIMBRE SET #{}/{} [CC46]",
                                cockpit.model.timbre_bank.active_set_index + 1,
                                cockpit.model.timbre_bank.total_entries
                            ))
                            .monospace()
                            .color(palette.strip_solo_text),
                        )
                        .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.advance_full_timbre_set(led_manager, osc_server);
                }

                if ui
                    .add(
                        egui::Button::new(RichText::new("COMMIT NEXT-BAR").monospace())
                            .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.commit_queued_timbres(led_manager, osc_server);
                }

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "SAVE SNAPSHOT [CC60] ({})",
                                cockpit.model.timbre_bank.captured_pool.len()
                            ))
                            .monospace(),
                        )
                        .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    let draft = cockpit.snapshot_label_draft.clone();
                    let label = if draft.trim().is_empty() {
                        None
                    } else {
                        Some(draft.as_str())
                    };
                    cockpit.capture_preset_snapshot(label, osc_server);
                }
            });
        });
}

// =============================================================================
// 4. Epoch 11: Form Bank & Performance Navigation Deck
// =============================================================================

fn render_form_and_navigation_deck(
    ui: &mut Ui,
    cockpit: &mut Op8CockpitViewState,
    palette: &AccentPalette,
    text_color: Color32,
    led_manager: &mut MultiDeviceLedManager,
    osc_server: &mut Option<OscServer>,
) {
    ui.columns(3, |cols| {
        // Column 1: Searchable Form Bank Browser (CC 58 / CC 59)
        render_form_bank_browser(&mut cols[0], cockpit, palette, text_color, osc_server);

        // Column 2: Section & Bar Timeline Progress Bar + Pending Latch Indicators
        render_section_and_bar_timeline(
            &mut cols[1],
            cockpit,
            palette,
            text_color,
            led_manager,
            osc_server,
        );

        // Column 3: Interactive Global Density & Rhythmic Feel (Swing / Humanize) XY Pad
        render_global_density_feel_xy_panel(&mut cols[2], cockpit, palette, text_color, osc_server);
    });
}

fn render_form_bank_browser(
    ui: &mut Ui,
    cockpit: &mut Op8CockpitViewState,
    palette: &AccentPalette,
    text_color: Color32,
    osc_server: &mut Option<OscServer>,
) {
    let active_desc = cockpit.active_form_descriptor();
    let neighbors = cockpit.hydrated_neighbor_forms();

    egui::Frame::none()
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("FORM BANK BROWSER (4,096 FORMS | CC58/59)")
                        .font(FontId::monospace(11.0))
                        .strong()
                        .color(palette.accent_strong),
                );

                // Active form badge & tags
                ui.label(
                    RichText::new(format!(
                        "ACTIVE: {} (#{}/{})",
                        active_desc.display_name,
                        active_desc.index + 1,
                        cockpit.model.form_bank.total_entries
                    ))
                    .font(FontId::monospace(10.5))
                    .strong()
                    .color(text_color),
                );
                ui.label(
                    RichText::new(format!(
                        "TAGS: [{}] [{}] [{}] [{}]",
                        active_desc.base_form,
                        active_desc.motif,
                        active_desc.density_curve,
                        active_desc.section_role
                    ))
                    .font(FontId::monospace(9.5))
                    .color(palette.accent_weak),
                );

                // Prev / Next / Queue Next controls synced with Track L/R (CC 58 / 59)
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("<< PREV [CC58]").monospace())
                                .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.navigate_form(FormDirection::Previous, osc_server);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("NEXT [CC59] >>").monospace())
                                .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.navigate_form(FormDirection::Next, osc_server);
                    }
                    let next_idx = (active_desc.index + 1) % cockpit.model.form_bank.total_entries;
                    if ui
                        .add(
                            egui::Button::new(RichText::new("QUEUE NEXT").monospace())
                                .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.queue_form_for_next_bar(next_idx, osc_server);
                    }
                });

                // Hydrated Neighbor Forms (4-form prefetch window)
                ui.label(
                    RichText::new("HYDRATED NEIGHBORS (PREFETCH=4):")
                        .font(FontId::monospace(9.5))
                        .color(palette.accent_weak),
                );
                ui.horizontal_wrapped(|ui| {
                    for nb in &neighbors {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("{}:{}", nb.id, nb.base_form))
                                        .font(FontId::monospace(9.0)),
                                )
                                .rounding(Rounding::ZERO),
                            )
                            .clicked()
                        {
                            cockpit.queue_form_for_next_bar(nb.index, osc_server);
                        }
                    }
                });

                // Search filter box & top matching entries
                ui.horizontal(|ui| {
                    ui.label(RichText::new("SEARCH:").font(FontId::monospace(9.5)));
                    ui.text_edit_singleline(&mut cockpit.form_search_query);
                });

                let matches = cockpit.search_forms(3);
                for m in matches {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{} [{}]", m.display_name, m.density_curve))
                                .font(FontId::monospace(9.0)),
                        );
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("QUEUE").font(FontId::monospace(9.0)),
                                )
                                .rounding(Rounding::ZERO),
                            )
                            .clicked()
                        {
                            cockpit.queue_form_for_next_bar(m.index, osc_server);
                        }
                    });
                }
            });
        });
}

fn render_section_and_bar_timeline(
    ui: &mut Ui,
    cockpit: &mut Op8CockpitViewState,
    palette: &AccentPalette,
    text_color: Color32,
    led_manager: &mut MultiDeviceLedManager,
    osc_server: &mut Option<OscServer>,
) {
    let (bar_in_sec, beat_in_bar, bar_prog, sec_prog, sec_role) = cockpit.timeline_progress();
    let pending_timbres = cockpit.pending_timbre_lane_count();
    let queued_form = cockpit.queued_form_index.map(describe_form_entry);

    egui::Frame::none()
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("SECTION & BAR TIMELINE")
                        .font(FontId::monospace(11.0))
                        .strong()
                        .color(palette.accent_strong),
                );

                ui.label(
                    RichText::new(format!(
                        "BPM: {:.1} | SECTION: {} (BAR {}/{}) | BEAT: {:.2}/{}",
                        cockpit.model.transport.bpm,
                        sec_role.to_ascii_uppercase(),
                        bar_in_sec,
                        cockpit.section_total_bars,
                        beat_in_bar + 1.0,
                        cockpit.model.transport.beats_per_bar
                    ))
                    .font(FontId::monospace(10.0))
                    .color(text_color),
                );

                // 4-Beat Step Indicator Cells
                let active_beat_cell = (beat_in_bar.floor() as u8)
                    % cockpit.model.transport.beats_per_bar.max(1);
                ui.horizontal(|ui| {
                    for b in 0..cockpit.model.transport.beats_per_bar.max(1) {
                        let is_now = b == active_beat_cell;
                        egui::Frame::none()
                            .fill(if is_now {
                                palette.accent
                            } else {
                                Color32::TRANSPARENT
                            })
                            .stroke(Stroke::new(1.0, palette.accent_weak))
                            .rounding(Rounding::ZERO)
                            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("BEAT {}", b + 1))
                                        .font(FontId::monospace(9.5))
                                        .strong(),
                                );
                            });
                    }
                });

                // Bar Phase Progress Bar
                ui.label(
                    RichText::new(format!("BAR PHASE ({:.0}%):", bar_prog * 100.0))
                        .font(FontId::monospace(9.5)),
                );
                let (bar_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width().max(80.0), 10.0),
                    Sense::hover(),
                );
                if ui.is_rect_visible(bar_rect) {
                    let painter = ui.painter();
                    painter.rect_stroke(
                        bar_rect,
                        Rounding::ZERO,
                        Stroke::new(1.0, palette.accent_weak),
                    );
                    let mut fill = bar_rect;
                    fill.set_width(bar_rect.width() * bar_prog);
                    painter.rect_filled(fill, Rounding::ZERO, palette.accent_strong);
                }

                // Section Progress Bar
                ui.label(
                    RichText::new(format!("SECTION PROGRESS ({:.0}%):", sec_prog * 100.0))
                        .font(FontId::monospace(9.5)),
                );
                let (sec_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width().max(80.0), 10.0),
                    Sense::hover(),
                );
                if ui.is_rect_visible(sec_rect) {
                    let painter = ui.painter();
                    painter.rect_stroke(
                        sec_rect,
                        Rounding::ZERO,
                        Stroke::new(1.0, palette.accent_weak),
                    );
                    let mut fill = sec_rect;
                    fill.set_width(sec_rect.width() * sec_prog);
                    painter.rect_filled(fill, Rounding::ZERO, palette.strip_solo_text);
                }

                // Pending Next-Bar Timbre & Form Latch Indicators
                let timbre_latch_text = if pending_timbres > 0 {
                    format!("PENDING TIMBRE LATCH: {pending_timbres} LANE(S) AT NEXT BAR")
                } else {
                    "PENDING TIMBRE LATCH: NONE".to_string()
                };
                ui.label(
                    RichText::new(timbre_latch_text)
                        .font(FontId::monospace(9.5))
                        .color(if pending_timbres > 0 {
                            palette.strip_record_text
                        } else {
                            palette.accent_weak
                        }),
                );

                let form_latch_text = match &queued_form {
                    Some(qf) => format!("PENDING FORM LATCH: {} AT NEXT BAR", qf.display_name),
                    None => "PENDING FORM LATCH: NONE".to_string(),
                };
                ui.label(
                    RichText::new(form_latch_text)
                        .font(FontId::monospace(9.5))
                        .color(if queued_form.is_some() {
                            palette.strip_solo_text
                        } else {
                            palette.accent_weak
                        }),
                );

                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("+1 BEAT").monospace())
                                .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.advance_beat_phase(1.0, led_manager, osc_server);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("NEXT BAR DOWNBEAT").monospace())
                                .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        let bar = f64::from(cockpit.model.transport.beats_per_bar.max(1));
                        let rem = bar - (cockpit.model.transport.current_beat % bar);
                        cockpit.advance_beat_phase(rem, led_manager, osc_server);
                    }
                });
            });
        });
}

fn render_global_density_feel_xy_panel(
    ui: &mut Ui,
    cockpit: &mut Op8CockpitViewState,
    palette: &AccentPalette,
    text_color: Color32,
    osc_server: &mut Option<OscServer>,
) {
    egui::Frame::none()
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("GLOBAL DENSITY & FEEL XY PAD")
                        .font(FontId::monospace(11.0))
                        .strong()
                        .color(palette.accent_strong),
                );

                // Interactive 2D Vector XY Surface
                let pad_size = Vec2::new(ui.available_width().max(120.0), 96.0);
                let (pad_rect, response) = ui.allocate_exact_size(pad_size, Sense::click_and_drag());

                if response.clicked() || response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let norm_x = ((pos.x - pad_rect.left()) / pad_rect.width()).clamp(0.0, 1.0);
                        // Y-axis: top = 1.0 feel, bottom = 0.0 feel, exact center = CC 64 = 0.5
                        let norm_y =
                            (1.0 - (pos.y - pad_rect.top()) / pad_rect.height()).clamp(0.0, 1.0);
                        let cc_x = (norm_x * 127.0).round().clamp(0.0, 127.0) as u8;
                        let cc_y = feel_scale_to_cc(f64::from(norm_y));
                        let density = cc_to_density(cc_x);
                        let feel_scale = cc_to_feel_scale(cc_y);
                        cockpit.set_global_density_and_feel_xy(density, feel_scale, osc_server);
                    }
                }

                if ui.is_rect_visible(pad_rect) {
                    let painter = ui.painter();
                    painter.rect_stroke(
                        pad_rect,
                        Rounding::ZERO,
                        Stroke::new(1.0, palette.accent_weak),
                    );

                    // Nominal 1.0x density vertical guide & CC 64 = 0.5 feel horizontal guide
                    let nominal_x_ratio = f32::from(density_to_cc(1.0)) / 127.0;
                    let guide_x = pad_rect.left() + pad_rect.width() * nominal_x_ratio;
                    let guide_y = pad_rect.center().y;
                    painter.line_segment(
                        [
                            Pos2::new(guide_x, pad_rect.top()),
                            Pos2::new(guide_x, pad_rect.bottom()),
                        ],
                        Stroke::new(1.0, palette.accent_weak),
                    );
                    painter.line_segment(
                        [
                            Pos2::new(pad_rect.left(), guide_y),
                            Pos2::new(pad_rect.right(), guide_y),
                        ],
                        Stroke::new(1.0, palette.accent_weak),
                    );

                    // Current XY cursor position
                    let cur_x_ratio =
                        (f32::from(cockpit.model.global_density_cc) / 127.0).clamp(0.0, 1.0);
                    let cur_y_ratio = (cockpit.model.feel.scale as f32).clamp(0.0, 1.0);
                    let cursor_pos = Pos2::new(
                        pad_rect.left() + pad_rect.width() * cur_x_ratio,
                        pad_rect.bottom() - pad_rect.height() * cur_y_ratio,
                    );
                    let cursor_rect = Rect::from_center_size(cursor_pos, Vec2::splat(8.0));
                    painter.rect_filled(cursor_rect, Rounding::ZERO, palette.accent_strong);
                }

                // Tabular Readouts for X (Global Density) and Y (Feel / Swing / Humanize)
                ui.label(
                    RichText::new(format!(
                        "X DENSITY: {:.2}x (CC={}) | Y FEEL: {:.2} (CC={})",
                        cockpit.model.global_density_multiplier,
                        cockpit.model.global_density_cc,
                        cockpit.model.feel.scale,
                        cockpit.model.feel.feel_cc
                    ))
                    .font(FontId::monospace(9.5))
                    .color(text_color),
                );
                ui.label(
                    RichText::new(format!(
                        "SWING: {:.3} (AUTH {:.2}) | HUMANIZE: {:.3} (AUTH {:.2})",
                        cockpit.model.feel.effective_swing,
                        cockpit.model.feel.authored_swing,
                        cockpit.model.feel.effective_humanize,
                        cockpit.model.feel.authored_humanize
                    ))
                    .font(FontId::monospace(9.5))
                    .color(palette.strip_solo_text),
                );

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("RESET CENTER (1.00x | CC64=0.50)").monospace(),
                        )
                        .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.set_global_density_and_feel_xy(1.0, 0.5, osc_server);
                }
            });
        });
}

// =============================================================================
// 5. 8-Column Percussion Voice Matrix Renderer
// =============================================================================

fn render_voice_lane_column(
    ui: &mut Ui,
    lane_idx: usize,
    cockpit: &mut Op8CockpitViewState,
    palette: &AccentPalette,
    text_color: Color32,
    led_manager: &mut MultiDeviceLedManager,
    osc_server: &mut Option<OscServer>,
) {
    let is_audible = cockpit.model.is_lane_audible(lane_idx);
    let is_selected = cockpit.selected_lane == lane_idx;
    let lane_snapshot = cockpit.model.lanes[lane_idx].clone();
    let effective_density = cockpit.model.effective_lane_density(lane_idx);

    let border_color = if is_selected {
        palette.strip_selection_border
    } else if is_audible {
        palette.accent_weak
    } else {
        palette.strip_mute_text
    };

    egui::Frame::none()
        .stroke(Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_color))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                // 1. Track & Family Header + Audibility Badge
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            is_selected,
                            RichText::new(format!(
                                "T{} {}",
                                lane_idx + 1,
                                lane_snapshot.family.to_ascii_uppercase()
                            ))
                            .font(FontId::monospace(11.5))
                            .strong(),
                        )
                        .clicked()
                    {
                        cockpit.selected_lane = lane_idx;
                    }
                });

                ui.label(
                    RichText::new(format!(
                        "[{}] {}",
                        lane_snapshot.live_label,
                        if is_audible { "AUDIBLE" } else { "SILENCED" }
                    ))
                    .font(FontId::monospace(10.0))
                    .color(if is_audible {
                        palette.accent_strong
                    } else {
                        palette.strip_mute_text
                    }),
                );

                ui.separator();

                // 2. Active Timbre Patch Badge
                ui.label(
                    RichText::new("ACTIVE TIMBRE:")
                        .font(FontId::monospace(9.5))
                        .color(palette.accent_weak),
                );
                egui::Frame::none()
                    .stroke(Stroke::new(1.0, palette.accent))
                    .rounding(Rounding::ZERO)
                    .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&lane_snapshot.active_timbre)
                                .font(FontId::monospace(10.5))
                                .strong()
                                .color(text_color),
                        );
                    });

                // 3. Queued Next-Bar Timbre Indicator
                let queued_text = lane_snapshot
                    .queued_timbre
                    .as_deref()
                    .unwrap_or("(none queued)");
                let has_queued = lane_snapshot.queued_timbre.is_some();
                ui.label(
                    RichText::new(format!("NEXT BAR: {queued_text}"))
                        .font(FontId::monospace(9.5))
                        .color(if has_queued {
                            palette.strip_solo_text
                        } else {
                            palette.accent_weak
                        }),
                );

                ui.add_space(4.0);

                // 4. Interactive Density Ladder Meter (0.1x to 5.0x, CC 16..23)
                ui.label(
                    RichText::new(format!(
                        "DENS [CC{}]: {:.2}x (EFF {:.2}x)",
                        NK2_DENSITY_KNOB_CCS[lane_idx],
                        lane_snapshot.density_multiplier,
                        effective_density
                    ))
                    .font(FontId::monospace(9.5)),
                );

                let (meter_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width().max(48.0), 10.0),
                    Sense::hover(),
                );
                if ui.is_rect_visible(meter_rect) {
                    let painter = ui.painter();
                    painter.rect_stroke(
                        meter_rect,
                        Rounding::ZERO,
                        Stroke::new(1.0, palette.accent_weak),
                    );
                    let fill_ratio = (f32::from(lane_snapshot.density_cc) / 127.0).clamp(0.0, 1.0);
                    let mut fill_rect = meter_rect;
                    fill_rect.set_width(meter_rect.width() * fill_ratio);
                    painter.rect_filled(fill_rect, Rounding::ZERO, palette.accent);
                }

                let mut density_val = lane_snapshot.density_multiplier;
                if ui
                    .add(
                        egui::Slider::new(&mut density_val, MIN_DENSITY_SCALE..=MAX_DENSITY_SCALE)
                            .logarithmic(true)
                            .show_value(false),
                    )
                    .changed()
                {
                    cockpit.set_lane_density(lane_idx, density_val, led_manager, osc_server);
                }

                ui.add_space(4.0);

                // 5. Level / Decay Macro Slider (CC 0..7, 0..=127)
                ui.label(
                    RichText::new(format!(
                        "MACRO [CC{}]: {:3}/127 ({:.0}%)",
                        NK2_GAIN_SLIDER_CCS[lane_idx],
                        lane_snapshot.gain_cc,
                        lane_snapshot.gain_normalized * 100.0
                    ))
                    .font(FontId::monospace(9.5)),
                );
                let mut gain_cc = lane_snapshot.gain_cc;
                if ui
                    .add(egui::Slider::new(&mut gain_cc, 0..=127).show_value(false))
                    .changed()
                {
                    cockpit.set_lane_gain_cc(lane_idx, gain_cc, led_manager, osc_server);
                }

                ui.add_space(4.0);

                // 6. Solo / Mute / Rec LED Buttons (CC 32..39, CC 48..55, CC 64..71)
                ui.horizontal(|ui| {
                    let s_label = format!("S[{}]", NK2_SOLO_BUTTON_CCS[lane_idx]);
                    if ui
                        .add(
                            egui::Button::new(RichText::new(s_label).monospace().color(
                                if lane_snapshot.soloed {
                                    palette.strip_solo_text
                                } else {
                                    text_color
                                },
                            ))
                            .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.toggle_lane_solo(lane_idx, led_manager, osc_server);
                    }

                    let m_label = format!("M[{}]", NK2_MUTE_BUTTON_CCS[lane_idx]);
                    if ui
                        .add(
                            egui::Button::new(RichText::new(m_label).monospace().color(
                                if lane_snapshot.muted {
                                    palette.strip_mute_text
                                } else {
                                    text_color
                                },
                            ))
                            .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.toggle_lane_mute(lane_idx, led_manager, osc_server);
                    }

                    let r_label = format!("R[{}]", NK2_REC_REROLL_CCS[lane_idx]);
                    if ui
                        .add(
                            egui::Button::new(RichText::new(r_label).monospace().color(
                                if has_queued {
                                    palette.strip_record_text
                                } else {
                                    text_color
                                },
                            ))
                            .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.reroll_lane_timbre(lane_idx, led_manager, osc_server);
                    }
                });

                ui.add_space(4.0);

                // 7. Timbre Lock Toggle & One-Click Timbre Reroll Button
                ui.horizontal(|ui| {
                    let lock_label = if lane_snapshot.timbre_locked {
                        "LOCKED"
                    } else {
                        "UNLOCKED"
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(lock_label).monospace().color(
                                if lane_snapshot.timbre_locked {
                                    palette.strip_solo_text
                                } else {
                                    text_color
                                },
                            ))
                            .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.toggle_lane_timbre_lock(lane_idx);
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(format!("REROLL (#{})", lane_snapshot.reroll_count))
                                    .monospace(),
                            )
                            .rounding(Rounding::ZERO),
                        )
                        .clicked()
                    {
                        cockpit.reroll_lane_timbre(lane_idx, led_manager, osc_server);
                    }
                });
            });
        });
}

// =============================================================================
// 6. Unit & Headless egui + Bidirectional OSC / LED Integration Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rosc::decoder;
    use std::net::UdpSocket;
    use std::time::Duration;

    #[test]
    fn test_timbre_lock_prevents_global_set_from_overwriting_locked_lanes() {
        let mut led_manager = MultiDeviceLedManager::new();
        let dev_key = "0944:0117:noserial:nanoKONTROL2";
        led_manager.ensure_controller(dev_key, "nanokontrol2");
        let mut osc_server = None;

        let mut cockpit = Op8CockpitViewState::new(dev_key);
        cockpit.reroll_lane_timbre(0, &mut led_manager, &mut osc_server);
        cockpit.commit_queued_timbres(&mut led_manager, &mut osc_server);
        assert_eq!(cockpit.model.lanes[0].active_timbre, "kick-var-0001");

        assert_eq!(cockpit.toggle_lane_timbre_lock(0), Some(true));
        assert_eq!(cockpit.toggle_lane_timbre_lock(4), Some(true));

        let (set_idx, timbres) = cockpit.advance_full_timbre_set(&mut led_manager, &mut osc_server);
        assert_eq!(set_idx, 1);

        assert_eq!(timbres[0], "kick-var-0001");
        assert_eq!(cockpit.model.lanes[0].queued_timbre, None);
        assert_eq!(timbres[4], "clap");
        assert_eq!(cockpit.model.lanes[4].queued_timbre, None);

        assert_eq!(timbres[1], "snare-set-0002");
        assert_eq!(timbres[7], "cymbal-set-0002");
    }

    #[test]
    fn test_cockpit_interactions_drive_hardware_leds_and_udp_osc_telemetry() {
        let client_sock = UdpSocket::bind("127.0.0.1:0").expect("bind client UDP");
        client_sock
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        let client_addr = client_sock.local_addr().unwrap();

        let server = OscServer::bind("127.0.0.1:0", vec![client_addr]).expect("bind OscServer");
        let mut osc_server = Some(server);

        let dev_key = "0944:0117:noserial:nanoKONTROL2";
        let mut led_manager = MultiDeviceLedManager::new();
        led_manager.ensure_controller(dev_key, "nanokontrol2");

        let mut cockpit = Op8CockpitViewState::new(dev_key);

        cockpit.toggle_lane_solo(1, &mut led_manager, &mut osc_server);
        let ctrl = led_manager.controllers.get(dev_key).unwrap();
        assert!(ctrl.led_state_by_name("track2.solo"));
        assert!(ctrl.led_state_by_number(33));

        cockpit.toggle_lane_mute(2, &mut led_manager, &mut osc_server);
        let ctrl = led_manager.controllers.get(dev_key).unwrap();
        assert!(ctrl.led_state_by_name("track3.mute"));
        assert!(ctrl.led_state_by_number(50));

        cockpit.reroll_lane_timbre(0, &mut led_manager, &mut osc_server);
        let ctrl = led_manager.controllers.get(dev_key).unwrap();
        assert!(ctrl.led_state_by_name("track1.rec"));
        assert!(ctrl.led_state_by_number(64));

        cockpit.set_lane_density(0, 3.25, &mut led_manager, &mut osc_server);

        let mut addrs = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok((len, _)) = client_sock.recv_from(&mut buf) {
            if let Ok((_, rosc::OscPacket::Message(msg))) = decoder::decode_udp(&buf[..len]) {
                addrs.push(msg.addr);
            }
        }
        assert!(addrs.contains(&"/nkeditor/op8/lane/1/solo".to_string()));
        assert!(addrs.contains(&"/nkeditor/op8/lane/2/mute".to_string()));
        assert!(addrs.contains(&"/nkeditor/op8/timbre".to_string()));
        assert!(addrs.contains(&"/nkeditor/op8/lane/0/density".to_string()));
        assert!(addrs.contains(&"/nkeditor/op8/state".to_string()));
    }

    #[test]
    fn test_form_bank_browser_search_timeline_latch_and_xy_pad() {
        let mut led_manager = MultiDeviceLedManager::new();
        let dev_key = "0944:0117:noserial:nanoKONTROL2";
        led_manager.ensure_controller(dev_key, "nanokontrol2");
        let mut osc_server = None;

        let mut cockpit = Op8CockpitViewState::new(dev_key);
        // Active form starts at gqom_drive (index 0)
        let active = cockpit.active_form_descriptor();
        assert_eq!(active.index, 0);
        assert_eq!(active.display_name, "gqom_drive");
        assert_eq!(active.base_form, "gqom_drive");

        // Hydrated neighbor forms should return the next 4 forms (indices 1..=4)
        let neighbors = cockpit.hydrated_neighbor_forms();
        assert_eq!(neighbors.len(), 4);
        assert_eq!(neighbors[0].index, 1);
        assert_eq!(neighbors[3].index, 4);

        // Search forms by archetype tag "footwork_drive"
        cockpit.form_search_query = "footwork_drive".to_string();
        let results = cockpit.search_forms(10);
        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|r| r.base_form == "footwork_drive"));

        // Queue form #6 (footwork_drive) and reroll lane 2 timbre for next bar downbeat
        let queued = cockpit.queue_form_for_next_bar(results[0].index, &mut osc_server);
        cockpit.reroll_lane_timbre(2, &mut led_manager, &mut osc_server);
        assert_eq!(cockpit.queued_form_index, Some(queued.index));
        assert_eq!(cockpit.pending_timbre_lane_count(), 1);

        // Advance beat by 2.0 beats (mid-bar: bar 1, beat 2.0) -> still pending
        let crossed = cockpit.advance_beat_phase(2.0, &mut led_manager, &mut osc_server);
        assert!(!crossed);
        assert_eq!(cockpit.queued_form_index, Some(queued.index));
        assert_eq!(cockpit.pending_timbre_lane_count(), 1);

        // Advance beat by another 2.0 beats (reaches beat 4.0 = Bar 2 downbeat) -> latches both!
        let crossed = cockpit.advance_beat_phase(2.0, &mut led_manager, &mut osc_server);
        assert!(crossed);
        assert_eq!(cockpit.queued_form_index, None);
        assert_eq!(cockpit.pending_timbre_lane_count(), 0);
        assert_eq!(cockpit.model.form_bank.active_index, queued.index);
        assert_eq!(cockpit.model.lanes[2].active_timbre, "hat_closed-var-0001");

        // Interactive XY Pad: set Global Density = 2.5x, Feel Scale = 0.5 (exact CC 64)
        let (d, f) = cockpit.set_global_density_and_feel_xy(2.5, 0.5, &mut osc_server);
        assert!((d - 2.5).abs() < 1e-6);
        assert_eq!(f, 0.5);
        assert_eq!(cockpit.model.feel.feel_cc, 64);
    }

    #[test]
    fn test_headless_egui_renders_8_column_op8_cockpit_matrix() {
        let ctx = egui::Context::default();
        let theme_settings = ThemeSettings::default();
        theme_settings.apply_to_ctx(&ctx);

        let dev_key = "0944:0117:noserial:nanoKONTROL2";
        let mut led_manager = MultiDeviceLedManager::new();
        led_manager.ensure_controller(dev_key, "nanokontrol2");
        let mut osc_server = None;
        let mut cockpit = Op8CockpitViewState::new(dev_key);
        cockpit.reroll_lane_timbre(3, &mut led_manager, &mut osc_server);
        cockpit.toggle_lane_timbre_lock(2);
        cockpit.queue_form_for_next_bar(6, &mut osc_server);

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1440.0, 900.0),
            )),
            ..Default::default()
        };

        let full_output = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_op8_cockpit(
                    ui,
                    &mut cockpit,
                    &theme_settings,
                    &mut led_manager,
                    &mut osc_server,
                );
            });
        });

        assert!(
            !full_output.shapes.is_empty(),
            "8opPerc cockpit should emit vector shapes for form deck, timeline, XY pad, and 8 voice lanes"
        );
    }
}
