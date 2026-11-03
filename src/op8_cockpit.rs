//! Interactive `8opPerc` Performance Cockpit View (`src/op8_cockpit.rs`).
//!
//! Renders the 8-column **Percussion Voice Matrix** aligned 1:1 with the 8 physical tracks
//! of the Korg `nanoKONTROL2`, styled with `yaae-theme` and `twoBowls` crisp zero-rounding
//! borders (`egui::Rounding::ZERO`) and [`crate::theme::AccentPalette`] highlights.
//!
//! For each of the 8 voice lanes (`0..8`), displays and controls:
//! - Family name (`KICK`, `SNARE`, `HAT_CLOSED`, `HAT_OPEN`, `CLAP`, `TOM`, `RIM`, `CYMBAL`),
//!   Konduktiva live label (`kicks`, `snares`, `hats`, `toms`, `percussion`, `alt-toms`,
//!   `noise`, `fx`), and Solo/Mute Audibility badge (`AUDIBLE` vs `SILENCED`)
//! - Active timbre patch badge (`active_timbre`)
//! - Queued next-bar timbre indicator (`queued_timbre`)
//! - Interactive density ladder meter (`0.1x` to `5.0x` + effective `global * lane` density)
//! - Level / decay macro slider (`0..=127` / normalized `%`)
//! - Hardware-synchronized **Solo (`S`, `CC 32..39`)**, **Mute (`M`, `CC 48..55`)**, and
//!   **Rec (`R`, `CC 64..71`)** LED buttons
//! - **Timbre Lock** toggle button (preventing global timbre set changes from overwriting
//!   locked lanes)
//! - One-click **Timbre Reroll** button
//!
//! All cockpit interactions are wired bidirectionally to [`OscServer`] and
//! [`MultiDeviceLedManager`] so GUI clicks immediately update both the Konduktiva live rig
//! over UDP OSC and the physical Korg `nanoKONTROL2` LEDs.

use crate::led_controller::{
    LedOperatingMode, MultiDeviceLedManager, Op8VoiceLedState, DEFAULT_REROLL_FLASH_TICKS,
};
use crate::op8_model::{
    FormDirection, Op8PerformanceAction, Op8PerformanceModel, Op8PresetSnapshot, MAX_DENSITY_SCALE,
    MIN_DENSITY_SCALE, NK2_DENSITY_KNOB_CCS, NK2_GAIN_SLIDER_CCS, NK2_MUTE_BUTTON_CCS,
    NK2_REC_REROLL_CCS, NK2_SOLO_BUTTON_CCS,
};
use crate::osc_server::OscServer;
use crate::theme::{AccentPalette, ThemeSettings};
use eframe::egui::{self, Color32, FontId, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use rosc::{OscMessage, OscType};
use serde::{Deserialize, Serialize};

// =============================================================================
// 1. Cockpit View State & Bidirectional OSC / LED Controller Wiring
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
        Self {
            model: Op8PerformanceModel::new(),
            active_device_key: active_device_key.into(),
            selected_lane: 0,
            snapshot_label_draft: String::new(),
            action_log: Vec::new(),
        }
    }

    fn push_log(&mut self, entry: String) {
        self.action_log.push(entry);
        if self.action_log.len() > 64 {
            self.action_log.remove(0);
        }
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
                // Keep Rec LED lit if a next-bar timbre is queued or a flash timer is active
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

    /// Commits all queued next-bar timbres into active timbres (simulating a bar downbeat).
    pub fn commit_queued_timbres(
        &mut self,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> usize {
        let committed = self.model.commit_all_queued_timbres();
        if let Some(ctrl) = led_manager.controllers.get_mut(&self.active_device_key) {
            for i in 0..8 {
                ctrl.op8_snapshot.lanes[i].reroll_flash_ticks = 0;
            }
        }
        self.sync_leds(led_manager);
        self.sync_and_broadcast_osc_state(osc_server);
        committed
    }

    /// Navigates the 4,096-entry form bank (`Previous` or `Next`) and broadcasts `/nkeditor/op8/form`.
    pub fn navigate_form(
        &mut self,
        direction: FormDirection,
        osc_server: &mut Option<OscServer>,
    ) -> (usize, String) {
        let (idx, name) = self.model.form_bank.step(direction);
        let dir_str = match direction {
            FormDirection::Previous => "previous",
            FormDirection::Next => "next",
        };
        self.push_log(format!("form[{dir_str}] -> {name}"));
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/op8/form".to_string(),
                args: vec![
                    OscType::String(dir_str.to_string()),
                    OscType::String(name.clone()),
                ],
            });
        }
        self.sync_and_broadcast_osc_state(osc_server);
        (idx, name)
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
// 2. Interactive egui Rendering (Percussion Voice Matrix & Cockpit Header)
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
            // 1. Global Cockpit Header Bar (Form Bank, Timbre Reservoir, Global Density/Feel, Transport)
            render_cockpit_header_bar(ui, cockpit, &palette, text_color, led_manager, osc_server);

            ui.add_space(8.0);

            // 2. 8-Column Percussion Voice Matrix (Aligned 1:1 with Korg nanoKONTROL2 Tracks 1..8)
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

                // Transport Controls (Play CC 41, Stop CC 42, WAV Rec CC 45)
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

                // Form Bank Navigation (Track Left CC 58 / Track Right CC 59)
                if ui
                    .add(
                        egui::Button::new(RichText::new("<< PREV FORM [CC58]").monospace())
                            .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.navigate_form(FormDirection::Previous, osc_server);
                }
                ui.label(
                    RichText::new(format!(
                        "FORM: {} ({}/{})",
                        cockpit.model.form_bank.active_form_name,
                        cockpit.model.form_bank.active_index + 1,
                        cockpit.model.form_bank.total_entries
                    ))
                    .monospace()
                    .strong(),
                );
                if ui
                    .add(
                        egui::Button::new(RichText::new("NEXT FORM [CC59] >>").monospace())
                            .rounding(Rounding::ZERO),
                    )
                    .clicked()
                {
                    cockpit.navigate_form(FormDirection::Next, osc_server);
                }

                ui.separator();

                // Full Timbre Set Advance (Cycle CC 46) & Commit Queued Timbres
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

                // Atomic Preset Pool Capture (Marker Set CC 60)
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

            ui.add_space(4.0);

            // Second Header Row: Global Density (0.1x..5.0x) & Global Feel Scale (0.0..1.0, CC 64 = 0.5)
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "GLOBAL DENSITY: {:.2}x [CC61/62]",
                        cockpit.model.global_density_multiplier
                    ))
                    .monospace(),
                );
                let mut g_dens = cockpit.model.global_density_multiplier;
                if ui
                    .add(
                        egui::Slider::new(&mut g_dens, MIN_DENSITY_SCALE..=MAX_DENSITY_SCALE)
                            .logarithmic(true)
                            .show_value(false),
                    )
                    .changed()
                {
                    cockpit.model.set_global_density_multiplier(g_dens);
                    cockpit.sync_and_broadcast_osc_state(osc_server);
                }

                ui.separator();

                ui.label(
                    RichText::new(format!(
                        "FEEL SCALE: {:.2} (CC={} | SWING={:.3} HUM={:.3}) [CC43/44]",
                        cockpit.model.feel.scale,
                        cockpit.model.feel.feel_cc,
                        cockpit.model.feel.effective_swing,
                        cockpit.model.feel.effective_humanize
                    ))
                    .monospace(),
                );
                let mut f_scale = cockpit.model.feel.scale;
                if ui
                    .add(egui::Slider::new(&mut f_scale, 0.0..=1.0).show_value(false))
                    .changed()
                {
                    cockpit.model.feel.set_scale(f_scale);
                    cockpit.sync_and_broadcast_osc_state(osc_server);
                }
            });
        });
}

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

                // Custom vector density ladder bar (0.1x .. 5.0x)
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
// 3. Unit & Headless egui + Bidirectional OSC / LED Integration Tests
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
        // Reroll lane 0 (kick) and commit it so kick has "kick-var-0001"
        cockpit.reroll_lane_timbre(0, &mut led_manager, &mut osc_server);
        cockpit.commit_queued_timbres(&mut led_manager, &mut osc_server);
        assert_eq!(cockpit.model.lanes[0].active_timbre, "kick-var-0001");

        // Lock lane 0 (kick) and lane 4 (clap)
        assert_eq!(cockpit.toggle_lane_timbre_lock(0), Some(true));
        assert_eq!(cockpit.toggle_lane_timbre_lock(4), Some(true));

        // Trigger global timbre set advance (Cycle CC 46)
        let (set_idx, timbres) = cockpit.advance_full_timbre_set(&mut led_manager, &mut osc_server);
        assert_eq!(set_idx, 1);

        // Locked lanes 0 and 4 must keep their existing timbres ("kick-var-0001" and "clap")
        assert_eq!(timbres[0], "kick-var-0001");
        assert_eq!(cockpit.model.lanes[0].queued_timbre, None);
        assert_eq!(timbres[4], "clap");
        assert_eq!(cockpit.model.lanes[4].queued_timbre, None);

        // Unlocked lanes (e.g. lane 1 snare, lane 7 cymbal) advance to set 0002
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

        // 1. Toggle Solo on Lane 1 (Snare -> CC 33 LED ON)
        cockpit.toggle_lane_solo(1, &mut led_manager, &mut osc_server);
        let ctrl = led_manager.controllers.get(dev_key).unwrap();
        assert!(ctrl.led_state_by_name("track2.solo"));
        assert!(ctrl.led_state_by_number(33));

        // 2. Toggle Mute on Lane 2 (Hat Closed -> CC 50 LED ON)
        cockpit.toggle_lane_mute(2, &mut led_manager, &mut osc_server);
        let ctrl = led_manager.controllers.get(dev_key).unwrap();
        assert!(ctrl.led_state_by_name("track3.mute"));
        assert!(ctrl.led_state_by_number(50));

        // 3. Reroll Timbre on Lane 0 (Kick -> CC 64 Rec LED ON)
        cockpit.reroll_lane_timbre(0, &mut led_manager, &mut osc_server);
        let ctrl = led_manager.controllers.get(dev_key).unwrap();
        assert!(ctrl.led_state_by_name("track1.rec"));
        assert!(ctrl.led_state_by_number(64));

        // 4. Adjust Density on Lane 0 -> emits /nkeditor/op8/lane/0/density
        cockpit.set_lane_density(0, 3.25, &mut led_manager, &mut osc_server);

        // Collect emitted OSC packets from client_sock
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
            "8opPerc cockpit should emit vector shapes for all 8 voice lanes"
        );
    }
}
