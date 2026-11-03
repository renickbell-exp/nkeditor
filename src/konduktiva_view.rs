//! Konduktiva Live 8-Lane Rhythm Activity & Step Visualizer (`src/konduktiva_view.rs`).
//!
//! Connects Konduktiva `MusicalEnvironment` players and `QuantizedMap` rhythm-map ladders
//! (`js/konduktiva-bridge.js`, `/home/renick/claude/konduktiva`, and
//! `/home/renick/claude/8operc-konduktiva/src/pulsed-core-rhythm.js`) to the Rust `egui` cockpit:
//! 1. **Per-Player `QuantizedMap` IOI Rhythm Rows ([`KonduktivaLaneRhythmRow`])**:
//!    Models each lane's loop `keyspan` (`4.0`, `8.0`, or `16.0` beats), `onset_beats`,
//!    inter-onset intervals (`ioi_values`), `mask_flags` (`maskMap`), `velocities` (`velocityMap`),
//!    sparse-to-dense ladder rung position (`selected_rung_index` / `total_rungs`),
//!    `native_rate`, `effective_density`, `target_rate`, and per-hit `variation_depth`.
//! 2. **Real-Time Note Trigger Stream ([`KonduktivaNoteTrigger`])**:
//!    Captures live note onset triggers (`beat`, `step_index`, `ioi`, `note`, `velocity`,
//!    `gate_ms`, `timbre`, `variation_depth`) streamed over UDP OSC from `KonduktivaBridge`.
//! 3. **Interactive 8-Lane Rhythm Activity & Step Visualizer (`render_konduktiva_rhythm_visualizer`)**:
//!    Renders an 8-row custom vector step & IOI timeline in `egui` so the performer can see
//!    upcoming rhythm density changes, ladder rung shifts, and voice triggers alongside
//!    physical controller movements.

use crate::op8_model::{
    Op8PerformanceModel, DEFAULT_BOOT_PATCHES, FAMILY_ORDER, KONDUKTIVA_FAMILY_ORDER,
    MAX_DENSITY_SCALE, MIN_DENSITY_SCALE,
};
use crate::theme::ThemeSettings;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Canonical per-family variation depth ranges `(low, high)` from
/// `8operc-konduktiva/src/pulsed-core-rhythm.js` (`VARIATION_DEPTH_RANGES`).
pub const VARIATION_DEPTH_RANGES: [(f64, f64); 8] = [
    (0.45, 0.95), // kicks
    (0.82, 1.45), // snares
    (0.72, 1.55), // hats
    (0.82, 1.50), // toms
    (0.78, 1.50), // bells
    (0.70, 1.28), // metals
    (0.75, 1.48), // noise
    (0.72, 1.50), // fx
];

/// Computes the deterministic per-hit variation depth at `beat` for lane `lane_index` (`0..8`),
/// matching `variationDepths` in `pulsed-core-rhythm.js`.
#[must_use]
pub fn compute_variation_depth(
    lane_index: usize,
    beat: f64,
    keyspan: f64,
    step_index: usize,
) -> f64 {
    let idx = lane_index.min(7);
    let (low, high) = VARIATION_DEPTH_RANGES[idx];
    let spread = high - low;
    let fam = KONDUKTIVA_FAMILY_ORDER[idx];
    let char_sum: u32 = fam.bytes().map(u32::from).sum();
    let family_phase = f64::from(char_sum) * 0.173;
    let phase = (beat / keyspan.max(1.0)) * std::f64::consts::TAU + family_phase;
    let contour =
        0.5 + 0.32 * phase.sin() + 0.18 * (phase * 2.618 + (step_index as f64) * 0.71).sin();
    (low + spread * contour).clamp(low, high)
}

// =============================================================================
// 1. Per-Player IOI Rhythm Row & Note Trigger Telemetry Structs
// =============================================================================

/// Real-time note trigger event fired by a Konduktiva `Player` (`action = '8operc'`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KonduktivaNoteTrigger {
    /// 0-based voice lane index (`0..8`).
    pub lane_index: usize,
    /// Canonical `nkEditor4` family (`"kick"`, `"snare"`, etc.).
    pub family: String,
    /// Beat timestamp when the note fired.
    pub beat: f64,
    /// Step index inside the active `QuantizedMap` rhythm row.
    pub step_index: usize,
    /// Inter-onset interval (`next_beat - current_beat`) in beats.
    pub ioi: f64,
    /// Triggered MIDI note (`0..=127`).
    pub note: u8,
    /// Triggered MIDI velocity (`0..=127`).
    pub velocity: u8,
    /// Note gate duration in milliseconds.
    pub gate_ms: u32,
    /// Active FM timbre preset name used for this hit.
    pub timbre: String,
    /// Evaluated per-hit variation depth (`0.45 ..= 1.55`).
    pub variation_depth: f64,
}

/// A single rung inside a Konduktiva sparse-to-dense `QuantizedMap` rhythm ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantizedRhythmRung {
    /// Loop span in beats (`4.0`, `8.0`, or `16.0`).
    pub keyspan: f64,
    /// Average event rate (`onsets / keyspan` hits per beat).
    pub rate: f64,
    /// Sorted onset beat positions within `[0.0, keyspan)`.
    pub onset_beats: Vec<f64>,
    /// Inter-onset intervals (`next_onset - current_onset`) in beats.
    pub ioi_values: Vec<f64>,
}

/// Complete real-time Konduktiva rhythm & density telemetry for a single percussion lane (`0..8`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KonduktivaLaneRhythmRow {
    /// 0-based lane index (`0..8`).
    pub lane_index: usize,
    /// Canonical `nkEditor4` family (`"kick".."cymbal"`).
    pub family: String,
    /// Canonical `8operc-konduktiva` family (`"kicks".."fx"`).
    pub konduktiva_family: String,
    /// Loop span in beats (`4.0`, `8.0`, or `16.0`).
    pub keyspan: f64,
    /// Sorted onset beat positions in the currently selected ladder rung.
    pub onset_beats: Vec<f64>,
    /// Inter-onset intervals (`ioi`) for each onset in the active rung.
    pub ioi_values: Vec<f64>,
    /// Per-step mask flags (`true` = masked/muted, `false` = active onset).
    pub mask_flags: Vec<bool>,
    /// Per-step MIDI velocities (`0..=127`).
    pub velocities: Vec<u8>,
    /// Currently selected rung index (`0..total_rungs`) in the `QuantizedMap` ladder.
    pub selected_rung_index: usize,
    /// Total rungs in the sparse-to-dense `QuantizedMap` ladder (default `5`).
    pub total_rungs: usize,
    /// Authored native event rate (hits per beat at `1.0x` density).
    pub native_rate: f64,
    /// Effective density multiplier (`lane_density * global_density`, `0.1 ..= 25.0`).
    pub effective_density: f64,
    /// Target event rate (`native_rate * effective_density`).
    pub target_rate: f64,
    /// Current per-hit variation depth (`0.45 ..= 1.55`).
    pub variation_depth: f64,
    /// Active FM timbre preset name.
    pub active_timbre: String,
    /// All 5 sparse-to-dense rungs in this lane's `QuantizedMap` ladder.
    pub ladder_rungs: Vec<QuantizedRhythmRung>,
    /// Most recent note trigger fired on this lane.
    pub last_trigger: Option<KonduktivaNoteTrigger>,
}

impl KonduktivaLaneRhythmRow {
    /// Constructs a new [`KonduktivaLaneRhythmRow`] for `lane_index` (`0..8`) over `keyspan` beats.
    #[must_use]
    pub fn new(lane_index: usize, keyspan: f64) -> Self {
        let idx = lane_index.min(7);
        let span = keyspan.max(1.0);
        let ladder_rungs = build_rhythm_map_ladder(idx, span);
        let native_rung = &ladder_rungs[2];
        let native_rate = native_rung.rate;
        let onset_beats = native_rung.onset_beats.clone();
        let ioi_values = native_rung.ioi_values.clone();
        let step_count = onset_beats.len();

        Self {
            lane_index: idx,
            family: FAMILY_ORDER[idx].to_string(),
            konduktiva_family: KONDUKTIVA_FAMILY_ORDER[idx].to_string(),
            keyspan: span,
            onset_beats,
            ioi_values,
            mask_flags: vec![false; step_count],
            velocities: (0..step_count)
                .map(|s| if s == 0 { 118 } else { 96 })
                .collect(),
            selected_rung_index: 2,
            total_rungs: ladder_rungs.len(),
            native_rate,
            effective_density: 1.0,
            target_rate: native_rate,
            variation_depth: compute_variation_depth(idx, 0.0, span, 0),
            active_timbre: DEFAULT_BOOT_PATCHES[idx].to_string(),
            ladder_rungs,
            last_trigger: None,
        }
    }

    /// Re-evaluates which `QuantizedMap` ladder rung is active given `effective_density`,
    /// `is_effectively_audible`, `active_timbre`, and `current_beat`.
    pub fn update_from_density(
        &mut self,
        effective_density: f64,
        is_effectively_audible: bool,
        active_timbre: &str,
        current_beat: f64,
    ) {
        let clamped = effective_density.clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);
        self.effective_density = clamped;
        self.target_rate = (self.native_rate * clamped).max(0.05);
        self.active_timbre = active_timbre.to_string();

        // Find the rung whose rate is closest to target_rate
        let mut best_idx = 0usize;
        let mut best_dist = f64::INFINITY;
        for (i, rung) in self.ladder_rungs.iter().enumerate() {
            let dist = (rung.rate - self.target_rate).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }

        self.selected_rung_index = best_idx;
        if let Some(rung) = self.ladder_rungs.get(best_idx) {
            self.onset_beats = rung.onset_beats.clone();
            self.ioi_values = rung.ioi_values.clone();
            let n = self.onset_beats.len();
            self.mask_flags = vec![!is_effectively_audible; n];
            self.velocities = (0..n).map(|s| if s == 0 { 118 } else { 96 }).collect();
        }
        self.variation_depth =
            compute_variation_depth(self.lane_index, current_beat, self.keyspan, 0);
    }

    /// Returns the index of the onset step currently sounding at `current_beat` (if unmasked).
    #[must_use]
    pub fn active_step_at_beat(&self, current_beat: f64) -> Option<usize> {
        if self.onset_beats.is_empty() {
            return None;
        }
        let local = current_beat.rem_euclid(self.keyspan.max(1.0));
        let mut candidate = None;
        for (idx, &onset) in self.onset_beats.iter().enumerate() {
            if onset <= local + 1e-4 {
                candidate = Some(idx);
            }
        }
        candidate.filter(|&i| !self.mask_flags.get(i).copied().unwrap_or(true))
    }
}

/// Builds the canonical 5-rung sparse-to-dense `QuantizedMap` rhythm ladder for `lane_index`
/// over `keyspan` beats, matching `buildRhythmMapLadderData` in `js/konduktiva-bridge.js`.
#[must_use]
pub fn build_rhythm_map_ladder(lane_index: usize, keyspan: f64) -> Vec<QuantizedRhythmRung> {
    let idx = lane_index.min(7);
    let span = keyspan.max(1.0);
    let scale = span / 4.0;

    let base_onsets: &[f64] = match idx {
        0 => &[0.0, 1.5, 2.0, 3.25],
        1 => &[1.0, 3.0],
        2 => &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5],
        3 => &[0.75, 2.75],
        4 => &[1.0, 2.5, 3.0],
        5 => &[0.5, 1.75, 3.5],
        6 => &[0.25, 1.25, 2.25, 3.25],
        _ => &[0.0, 2.0],
    };

    let native_keys: Vec<f64> = base_onsets.iter().map(|b| b * scale).collect();

    let rung_0: Vec<f64> = native_keys
        .iter()
        .enumerate()
        .filter_map(|(i, &k)| if i % 2 == 0 { Some(k) } else { None })
        .collect();
    let light_len = ((native_keys.len() as f64) * 0.75).ceil() as usize;
    let rung_1: Vec<f64> = native_keys
        .iter()
        .take(light_len.max(1))
        .copied()
        .collect();
    let rung_2: Vec<f64> = native_keys.clone();
    let rung_3: Vec<f64> = subdivide_onsets(&native_keys, span, 0.5);
    let rung_4: Vec<f64> = subdivide_onsets(&native_keys, span, 0.25);

    [rung_0, rung_1, rung_2, rung_3, rung_4]
        .into_iter()
        .map(|keys| make_rung(span, keys))
        .collect()
}

fn subdivide_onsets(keys: &[f64], span: f64, step: f64) -> Vec<f64> {
    let mut all = keys.to_vec();
    for &k in keys {
        let next = (k + step).rem_euclid(span);
        if !all.iter().any(|&existing| (existing - next).abs() < 1e-4) {
            all.push(next);
        }
    }
    all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    all
}

fn make_rung(span: f64, mut onset_beats: Vec<f64>) -> QuantizedRhythmRung {
    onset_beats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = onset_beats.len();
    let ioi_values: Vec<f64> = (0..n)
        .map(|i| {
            let next = if i + 1 < n {
                onset_beats[i + 1]
            } else {
                onset_beats[0] + span
            };
            ((next - onset_beats[i]) * 10000.0).round() / 10000.0
        })
        .collect();
    let rate = (n as f64) / span;
    QuantizedRhythmRung {
        keyspan: span,
        rate,
        onset_beats,
        ioi_values,
    }
}

// =============================================================================
// 2. Complete 8-Lane Konduktiva Telemetry State
// =============================================================================

/// Complete Konduktiva rhythm-map ladder and note-trigger telemetry state for all 8 voice lanes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KonduktivaTelemetryState {
    /// Per-lane IOI rhythm rows (`0..8`).
    pub lanes: [KonduktivaLaneRhythmRow; 8],
    /// Current loop `keyspan` in beats (`4.0`, `8.0`, or `16.0`).
    pub keyspan: f64,
    /// Active musical section label (e.g. `"drive"`, `"intro"`, `"peak"`).
    pub active_section_name: String,
    /// Current playhead beat cursor.
    pub current_beat: f64,
    /// Ring buffer of recent note triggers streamed from Konduktiva.
    pub trigger_history: Vec<KonduktivaNoteTrigger>,
    /// Maximum triggers kept in `trigger_history`.
    pub max_trigger_history: usize,
}

impl Default for KonduktivaTelemetryState {
    fn default() -> Self {
        Self::new(4.0)
    }
}

impl KonduktivaTelemetryState {
    /// Creates a new 8-lane [`KonduktivaTelemetryState`] with loop `keyspan` beats.
    #[must_use]
    pub fn new(keyspan: f64) -> Self {
        let span = keyspan.max(1.0);
        Self {
            lanes: std::array::from_fn(|i| KonduktivaLaneRhythmRow::new(i, span)),
            keyspan: span,
            active_section_name: "drive".to_string(),
            current_beat: 0.0,
            trigger_history: Vec::new(),
            max_trigger_history: 64,
        }
    }

    /// Synchronizes all 8 lanes' `QuantizedMap` ladder rungs, IOI rows, and mask maps
    /// with the current [`Op8PerformanceModel`] state.
    pub fn sync_with_op8_model(&mut self, model: &Op8PerformanceModel) {
        self.current_beat = model.transport.current_beat;
        let audible_mask = model.audible_mask();
        for i in 0..8 {
            let eff_density =
                model.lanes[i].density_multiplier * model.global_density_multiplier;
            self.lanes[i].update_from_density(
                eff_density,
                audible_mask[i],
                &model.lanes[i].active_timbre,
                self.current_beat,
            );
        }
    }

    /// Applies a JSON-encoded rhythm row update received over UDP OSC
    /// (`/nkeditor/konduktiva/lane/<idx>/rhythm`) from `js/konduktiva-bridge.js`.
    pub fn apply_lane_rhythm_json(&mut self, json_str: &str) -> Result<usize, String> {
        let val: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
        let obj = val
            .as_object()
            .ok_or_else(|| "Rhythm row JSON must be an object".to_string())?;

        let lane_idx = obj
            .get("laneIndex")
            .or_else(|| obj.get("lane_index"))
            .and_then(Value::as_u64)
            .ok_or_else(|| "Missing laneIndex".to_string())? as usize;
        if lane_idx >= 8 {
            return Err(format!("Invalid laneIndex: {lane_idx}"));
        }

        let row = &mut self.lanes[lane_idx];
        if let Some(span) = obj.get("keyspan").and_then(Value::as_f64) {
            row.keyspan = span.max(1.0);
        }
        if let Some(onsets) = obj
            .get("onsetBeats")
            .or_else(|| obj.get("onset_beats"))
            .and_then(Value::as_array)
        {
            row.onset_beats = onsets.iter().filter_map(Value::as_f64).collect();
        }
        if let Some(iois) = obj
            .get("ioiValues")
            .or_else(|| obj.get("ioi_values"))
            .and_then(Value::as_array)
        {
            row.ioi_values = iois.iter().filter_map(Value::as_f64).collect();
        }
        if let Some(masks) = obj
            .get("maskFlags")
            .or_else(|| obj.get("mask_flags"))
            .and_then(Value::as_array)
        {
            row.mask_flags = masks.iter().filter_map(Value::as_bool).collect();
        }
        if let Some(vels) = obj.get("velocities").and_then(Value::as_array) {
            row.velocities = vels
                .iter()
                .filter_map(Value::as_u64)
                .map(|v| (v as u8).min(127))
                .collect();
        }
        if let Some(rung) = obj
            .get("selectedRungIndex")
            .or_else(|| obj.get("selected_rung_index"))
            .and_then(Value::as_u64)
        {
            row.selected_rung_index = rung as usize;
        }
        if let Some(total) = obj
            .get("totalRungs")
            .or_else(|| obj.get("total_rungs"))
            .and_then(Value::as_u64)
        {
            row.total_rungs = (total as usize).max(1);
        }
        if let Some(eff) = obj
            .get("effectiveDensity")
            .or_else(|| obj.get("effective_density"))
            .and_then(Value::as_f64)
        {
            row.effective_density = eff;
        }
        if let Some(tr) = obj
            .get("targetRate")
            .or_else(|| obj.get("target_rate"))
            .and_then(Value::as_f64)
        {
            row.target_rate = tr;
        }
        if let Some(vd) = obj
            .get("variationDepth")
            .or_else(|| obj.get("variation_depth"))
            .and_then(Value::as_f64)
        {
            row.variation_depth = vd;
        }
        if let Some(timbre) = obj
            .get("activeTimbre")
            .or_else(|| obj.get("active_timbre"))
            .and_then(Value::as_str)
        {
            row.active_timbre = timbre.to_string();
        }

        Ok(lane_idx)
    }

    /// Records a real-time note trigger event (`/nkeditor/konduktiva/lane/<idx>/trigger`).
    pub fn record_note_trigger(&mut self, trigger: KonduktivaNoteTrigger) {
        let idx = trigger.lane_index.min(7);
        self.current_beat = trigger.beat;
        self.lanes[idx].last_trigger = Some(trigger.clone());
        self.trigger_history.push(trigger);
        if self.trigger_history.len() > self.max_trigger_history {
            self.trigger_history.remove(0);
        }
    }
}

// =============================================================================
// 3. Interactive egui 8-Lane Rhythm Activity & Step Visualizer
// =============================================================================

/// Renders the live 8-Lane Konduktiva Rhythm Activity & IOI Step Visualizer in `egui`.
pub fn render_konduktiva_rhythm_visualizer(
    ui: &mut Ui,
    telemetry: &mut KonduktivaTelemetryState,
    model: &Op8PerformanceModel,
    theme_settings: &ThemeSettings,
) {
    let palette = theme_settings.resolved_palette();
    let panel_fill = theme_settings.build_visuals().panel_fill;

    egui::Frame::none()
        .fill(panel_fill)
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // 1. Header Bar
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("KONDUKTIVA 8-LANE IOI RHYTHM & STEP VISUALIZER")
                        .strong()
                        .color(palette.accent),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Section: {}  |  Keyspan: {:.0}b  |  Beat: {:.2}  |  Triggers: {}",
                        telemetry.active_section_name,
                        telemetry.keyspan,
                        telemetry.current_beat,
                        telemetry.trigger_history.len()
                    ))
                    .monospace()
                    .small(),
                );
                if ui.small_button("Sync from Rig").clicked() {
                    telemetry.sync_with_op8_model(model);
                }
            });

            ui.add_space(6.0);

            // 2. 8-Row Percussion IOI Step Matrix
            for lane_idx in 0..8 {
                let row = &telemetry.lanes[lane_idx];
                let active_step = row.active_step_at_beat(telemetry.current_beat);

                ui.horizontal(|ui| {
                    // Left Column: Family + Ladder Rung + Rate + Density
                    let left_label = format!(
                        "{:<10} Rung {}/{}  {:>4.2}x  ({:>4.2}h/b)",
                        row.family.to_ascii_uppercase(),
                        row.selected_rung_index + 1,
                        row.total_rungs,
                        row.effective_density,
                        row.target_rate
                    );
                    ui.label(RichText::new(left_label).monospace().small());

                    // Center Column: Custom Vector IOI Step Strip
                    let desired_size = Vec2::new(280.0, 18.0);
                    let (rect, _resp) = ui.allocate_exact_size(desired_size, Sense::hover());
                    if ui.is_rect_visible(rect) {
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 0.0, Color32::from_gray(22));
                        painter.rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(1.0, Color32::from_gray(60)),
                        );

                        let span = row.keyspan.max(1.0) as f32;
                        // Draw beat gridlines
                        let beat_count = span.round() as usize;
                        for b in 1..beat_count {
                            let x = rect.left() + (b as f32 / span) * rect.width();
                            painter.line_segment(
                                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                                Stroke::new(1.0, Color32::from_gray(45)),
                            );
                        }

                        // Draw each IOI block
                        for (s_idx, &onset) in row.onset_beats.iter().enumerate() {
                            let ioi = row.ioi_values.get(s_idx).copied().unwrap_or(0.5);
                            let masked = row.mask_flags.get(s_idx).copied().unwrap_or(false);
                            let start_frac = (onset as f32 / span).clamp(0.0, 1.0);
                            let width_frac =
                                ((ioi as f32 / span) * 0.88).clamp(0.015, 1.0 - start_frac);

                            let x0 = rect.left() + start_frac * rect.width();
                            let x1 = x0 + width_frac * rect.width();
                            let step_rect = Rect::from_min_max(
                                Pos2::new(x0 + 1.0, rect.top() + 2.0),
                                Pos2::new((x1 - 1.0).max(x0 + 2.0), rect.bottom() - 2.0),
                            );

                            let is_current = active_step == Some(s_idx);
                            let fill = if masked {
                                Color32::from_gray(40)
                            } else if is_current {
                                palette.accent_strong
                            } else {
                                palette.accent
                            };
                            painter.rect_filled(step_rect, 0.0, fill);
                        }

                        // Draw playhead cursor
                        let playhead_frac =
                            (telemetry.current_beat.rem_euclid(row.keyspan.max(1.0)) as f32 / span)
                                .clamp(0.0, 1.0);
                        let px = rect.left() + playhead_frac * rect.width();
                        painter.line_segment(
                            [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
                            Stroke::new(1.5, Color32::WHITE),
                        );
                    }

                    // Right Column: Last Note Trigger Readout
                    let trig_text = if let Some(ref trig) = row.last_trigger {
                        format!(
                            "N{} v{:>3} IOI {:.2}b [{}]",
                            trig.note, trig.velocity, trig.ioi, trig.timbre
                        )
                    } else {
                        format!("var:{:.2} [{}]", row.variation_depth, row.active_timbre)
                    };
                    ui.label(RichText::new(trig_text).monospace().small().weak());
                });
            }
        });
}

// =============================================================================
// 4. Automated Unit & Headless egui Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhythm_ladder_shifts_rungs_and_ioi_rows_as_density_changes() {
        let mut model = Op8PerformanceModel::new();
        let mut telemetry = KonduktivaTelemetryState::new(4.0);

        // At default 1.0x density, lane 0 (kick) selects native rung 2 (4 onsets in 4 beats -> rate 1.0)
        telemetry.sync_with_op8_model(&model);
        assert_eq!(telemetry.lanes[0].selected_rung_index, 2);
        assert_eq!(telemetry.lanes[0].onset_beats.len(), 4);

        // Drop lane 0 density to 0.2x -> selects sparse rung 0 (2 onsets in 4 beats)
        model.lanes[0].density_multiplier = 0.2;
        telemetry.sync_with_op8_model(&model);
        assert_eq!(telemetry.lanes[0].selected_rung_index, 0);
        assert_eq!(telemetry.lanes[0].onset_beats.len(), 2);

        // Boost lane 0 density to 3.5x -> selects dense rung 4
        model.lanes[0].density_multiplier = 3.5;
        telemetry.sync_with_op8_model(&model);
        assert_eq!(telemetry.lanes[0].selected_rung_index, 4);
        assert!(telemetry.lanes[0].onset_beats.len() > 4);

        // Mute lane 0 -> mask_flags become all true and active_step_at_beat returns None
        model.lanes[0].muted = true;
        telemetry.sync_with_op8_model(&model);
        assert!(telemetry.lanes[0].mask_flags.iter().all(|&m| m));
        assert_eq!(telemetry.lanes[0].active_step_at_beat(0.0), None);
    }

    #[test]
    fn ingests_osc_json_rhythm_rows_and_note_triggers_from_konduktiva_bridge() {
        let mut telemetry = KonduktivaTelemetryState::new(4.0);
        let json_payload = r#"{
            "laneIndex": 2,
            "family": "hat_closed",
            "konduktivaFamily": "hats",
            "keyspan": 4,
            "onsetBeats": [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5],
            "ioiValues": [0.25, 0.25, 0.25, 0.25, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
            "maskFlags": [false, false, false, false, false, false, false, false, false, false],
            "velocities": [120, 90, 100, 90, 115, 95, 115, 95, 115, 95],
            "selectedRungIndex": 4,
            "totalRungs": 5,
            "nativeRate": 2.0,
            "effectiveDensity": 2.5,
            "targetRate": 5.0,
            "variationDepth": 1.18,
            "activeTimbre": "hat-razor-fm"
        }"#;

        let idx = telemetry
            .apply_lane_rhythm_json(json_payload)
            .expect("apply rhythm json");
        assert_eq!(idx, 2);
        assert_eq!(telemetry.lanes[2].onset_beats.len(), 10);
        assert_eq!(telemetry.lanes[2].selected_rung_index, 4);
        assert_eq!(telemetry.lanes[2].active_timbre, "hat-razor-fm");
        assert_eq!(telemetry.lanes[2].active_step_at_beat(0.26), Some(1));

        telemetry.record_note_trigger(KonduktivaNoteTrigger {
            lane_index: 2,
            family: "hat_closed".to_string(),
            beat: 0.25,
            step_index: 1,
            ioi: 0.25,
            note: 38,
            velocity: 90,
            gate_ms: 120,
            timbre: "hat-razor-fm".to_string(),
            variation_depth: 1.18,
        });
        assert_eq!(telemetry.trigger_history.len(), 1);
        assert_eq!(
            telemetry.lanes[2]
                .last_trigger
                .as_ref()
                .expect("last trigger")
                .note,
            38
        );
    }

    #[test]
    fn headless_egui_renders_konduktiva_8_lane_rhythm_visualizer_without_panic() {
        let ctx = egui::Context::default();
        let model = Op8PerformanceModel::new();
        let mut telemetry = KonduktivaTelemetryState::new(4.0);
        telemetry.sync_with_op8_model(&model);
        let theme = ThemeSettings::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_konduktiva_rhythm_visualizer(ui, &mut telemetry, &model, &theme);
            });
        });
    }
}
