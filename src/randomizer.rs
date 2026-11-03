//! Intelligent Randomization & Mutation Engine (`src/randomizer.rs`).
//!
//! Provides musical parameter randomization and bounded Gaussian drift for both
//! hardware controller mappings ([`DeviceMappingPipeline`]) and the `8opPerc`
//! performance model ([`Op8PerformanceModel`]):
//! - **Global vs. Selected-Track/Lane Randomization ([`RandomizationScope`])**:
//!   Target all 8 tracks/lanes, a single selected track (`1..=8`), or a custom subset of lanes.
//! - **Per-Control Lock Shields (`locked_controls` & `locked_lanes`)**:
//!   Any shielded control path (e.g. `"track1.knob"`) or locked voice lane (e.g. `kick`, `snare`)
//!   is strictly protected from randomization and drift.
//! - **Bounded Gaussian Drift (`1%` to `100%`) & Uniform Randomization ([`MutationMode`])**:
//!   Uses a deterministic `SplitMix64` PRNG and Box-Muller transform to apply smooth,
//!   repeatable Gaussian perturbations within each control's `[min_value, max_value]` range.
//! - **Musical Randomization Presets ([`MusicalRandomPreset`])**:
//!   `SubtleHumanize` (`5%`), `PercussiveGrooveShift` (`22%`, locks Kick/Snare),
//!   `HarmonicTimbreExplore` (`45%` + timbre reroll), and `PolyrhythmicChaos` (`85%`).

use crate::mapping_engine::DeviceMappingPipeline;
use crate::op8_model::{
    cc_to_density, density_to_cc, Op8PerformanceModel, FAMILY_ORDER, MAX_DENSITY_SCALE,
    MIN_DENSITY_SCALE,
};
use crate::theme::ThemeSettings;
use egui::{RichText, Stroke, Ui};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// =============================================================================
// 1. Deterministic SplitMix64 PRNG & Box-Muller Gaussian Generator
// =============================================================================

/// Deterministic 64-bit `SplitMix64` pseudo-random number generator with Box-Muller
/// standard normal $\mathcal{N}(0, 1)$ sampling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitMix64Rng {
    state: u64,
}

impl Default for SplitMix64Rng {
    fn default() -> Self {
        Self::new(0x80F8_2026_CAFE_BEEF)
    }
}

impl SplitMix64Rng {
    /// Creates a new deterministic PRNG seeded with `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Generates the next 64-bit unsigned integer.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Generates a uniform `f64` in `(0.0, 1.0)`.
    pub fn next_f64_open(&mut self) -> f64 {
        let bits = (self.next_u64() >> 11) | 1;
        (bits as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    /// Samples a standard normal $\mathcal{N}(0, 1)$ variate using the Box-Muller transform,
    /// clamped to `[-3.0, 3.0]` for bounded musical drift.
    pub fn next_gaussian_clamped(&mut self) -> f64 {
        let u1 = self.next_f64_open().max(1e-12);
        let u2 = self.next_f64_open();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = std::f64::consts::TAU * u2;
        (r * theta.cos()).clamp(-3.0, 3.0)
    }
}

// =============================================================================
// 2. Scope, Mutation Mode & Musical Presets
// =============================================================================

/// Target scope for a randomization or mutation pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RandomizationScope {
    /// Mutates all unlocked controls across all 8 tracks / voice lanes.
    #[default]
    Global,
    /// Mutates only controls belonging to a single 1-indexed track (`1..=8`, e.g. `"track3.*"`).
    SelectedTrack(usize),
    /// Mutates only the specified 0-indexed voice lanes (`0..8`).
    SelectedLanes(Vec<usize>),
}

impl RandomizationScope {
    /// Returns `true` if `control_path` (e.g. `"track2.knob"`) falls within this scope.
    #[must_use]
    pub fn matches_control_path(&self, control_path: &str) -> bool {
        match self {
            Self::Global => true,
            Self::SelectedTrack(track_num) => {
                let prefix = format!("track{track_num}.");
                control_path.starts_with(&prefix)
            }
            Self::SelectedLanes(lanes) => lanes.iter().any(|&lane_idx| {
                let prefix = format!("track{}.", lane_idx + 1);
                control_path.starts_with(&prefix)
            }),
        }
    }

    /// Returns `true` if 0-based `lane_index` (`0..8`) falls within this scope.
    #[must_use]
    pub fn matches_lane(&self, lane_index: usize) -> bool {
        match self {
            Self::Global => true,
            Self::SelectedTrack(track_num) => *track_num == lane_index + 1,
            Self::SelectedLanes(lanes) => lanes.contains(&lane_index),
        }
    }
}

/// Algorithm used to compute new parameter values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MutationMode {
    /// Bounded Gaussian drift around the current parameter value (`1% ..= 100%`).
    #[default]
    GaussianDrift,
    /// Full uniform random draw within each control's `[min_value, max_value]` range.
    UniformRandom,
}

/// Curated musical randomization presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MusicalRandomPreset {
    /// `5%` Gaussian drift; shields Kick (`lane 0`) and Snare (`lane 1`) for tight micro-variation.
    SubtleHumanize,
    /// `22%` Gaussian drift on upper percussion (`lanes 2..=7`); locks Kick and Snare.
    PercussiveGrooveShift,
    /// `45%` Gaussian drift + timbre rerolls on unlocked `8opPerc` lanes.
    HarmonicTimbreExplore,
    /// `85%` wide mutation across all unlocked tracks and voice lanes.
    PolyrhythmicChaos,
}

impl MusicalRandomPreset {
    /// All 4 built-in musical randomization presets.
    pub const ALL: [Self; 4] = [
        Self::SubtleHumanize,
        Self::PercussiveGrooveShift,
        Self::HarmonicTimbreExplore,
        Self::PolyrhythmicChaos,
    ];

    /// Human-readable title for UI buttons and command palette.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::SubtleHumanize => "Subtle Humanize (5%)",
            Self::PercussiveGrooveShift => "Percussive Groove Shift (22%)",
            Self::HarmonicTimbreExplore => "Harmonic Timbre Explore (45%)",
            Self::PolyrhythmicChaos => "Polyrhythmic Chaos (85%)",
        }
    }
}

// =============================================================================
// 3. Randomization Engine & Reversible Batch Result
// =============================================================================

/// Reversible record of a randomization pass, suitable for pushing directly onto
/// [`crate::history::CommandHistoryStack`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomizationResult {
    /// Human-readable description of the randomization pass.
    pub label: String,
    /// Map of `control_path -> (before_value, after_value)` for every mutated control.
    pub control_deltas: BTreeMap<String, (u8, u8)>,
    /// 0-based indices of `8opPerc` voice lanes that were mutated.
    pub mutated_lanes: Vec<usize>,
}

/// Stateful Randomization & Mutation Engine with per-control lock shields and bounded Gaussian drift.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RandomizerEngine {
    /// Deterministic PRNG for repeatable mutations.
    pub rng: SplitMix64Rng,
    /// Active target scope (`Global`, `SelectedTrack(1..=8)`, or `SelectedLanes`).
    pub scope: RandomizationScope,
    /// Active mutation mode (`GaussianDrift` or `UniformRandom`).
    pub mode: MutationMode,
    /// Drift intensity percentage (`1.0 ..= 100.0`, default `15.0`).
    pub drift_percent: f64,
    /// Whether timbre rerolls should also be triggered on unlocked `8opPerc` lanes.
    pub reroll_unlocked_timbres: bool,
    /// Per-control lock shields (`control_path`, e.g. `"track1.knob"`).
    pub locked_controls: BTreeSet<String>,
    /// Per-lane lock shields (`0..8`, e.g. `locked_lanes[0] = true` shields `kick`).
    pub locked_lanes: [bool; 8],
}

impl Default for RandomizerEngine {
    fn default() -> Self {
        Self::new(0x80F8_2026_CAFE_BEEF)
    }
}

impl RandomizerEngine {
    /// Creates a new [`RandomizerEngine`] with the given deterministic `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SplitMix64Rng::new(seed),
            scope: RandomizationScope::Global,
            mode: MutationMode::GaussianDrift,
            drift_percent: 15.0,
            reroll_unlocked_timbres: false,
            locked_controls: BTreeSet::new(),
            locked_lanes: [false; 8],
        }
    }

    /// Clamps and sets the Gaussian drift percentage (`1.0 ..= 100.0`).
    pub fn set_drift_percent(&mut self, percent: f64) {
        self.drift_percent = percent.clamp(1.0, 100.0);
    }

    /// Shields or unshields a specific `control_path` (e.g. `"track1.knob"`).
    pub fn set_control_lock(&mut self, control_path: impl Into<String>, locked: bool) {
        let path = control_path.into();
        if locked {
            self.locked_controls.insert(path);
        } else {
            self.locked_controls.remove(&path);
        }
    }

    /// Returns `true` if `control_path` is protected by either `locked_controls` or `locked_lanes`.
    #[must_use]
    pub fn is_control_shielded(&self, control_path: &str) -> bool {
        if self.locked_controls.contains(control_path) {
            return true;
        }
        for i in 0..8 {
            if self.locked_lanes[i] {
                let prefix = format!("track{}.", i + 1);
                if control_path.starts_with(&prefix) {
                    return true;
                }
            }
        }
        false
    }

    /// Configures the engine from a [`MusicalRandomPreset`].
    pub fn apply_musical_preset(&mut self, preset: MusicalRandomPreset) {
        self.mode = MutationMode::GaussianDrift;
        match preset {
            MusicalRandomPreset::SubtleHumanize => {
                self.set_drift_percent(5.0);
                self.reroll_unlocked_timbres = false;
                self.locked_lanes[0] = true;
                self.locked_lanes[1] = true;
            }
            MusicalRandomPreset::PercussiveGrooveShift => {
                self.set_drift_percent(22.0);
                self.reroll_unlocked_timbres = false;
                self.locked_lanes = [true, true, false, false, false, false, false, false];
            }
            MusicalRandomPreset::HarmonicTimbreExplore => {
                self.set_drift_percent(45.0);
                self.reroll_unlocked_timbres = true;
                self.locked_lanes = [true, false, false, false, false, false, false, false];
            }
            MusicalRandomPreset::PolyrhythmicChaos => {
                self.set_drift_percent(85.0);
                self.reroll_unlocked_timbres = true;
                self.locked_lanes = [false; 8];
            }
        }
    }

    /// Computes a new 7-bit parameter value within `[min_val, max_val]` according to
    /// `self.mode` and `self.drift_percent`.
    pub fn sample_mutated_value(&mut self, current: u8, min_val: u8, max_val: u8) -> u8 {
        let low = min_val.min(max_val).min(127);
        let high = min_val.max(max_val).min(127);
        if low == high {
            return low;
        }
        let span = f64::from(high - low);

        match self.mode {
            MutationMode::UniformRandom => {
                let u = self.rng.next_f64_open();
                (f64::from(low) + u * span).round().clamp(f64::from(low), f64::from(high)) as u8
            }
            MutationMode::GaussianDrift => {
                let pct = self.drift_percent.clamp(1.0, 100.0) / 100.0;
                let z = self.rng.next_gaussian_clamped();
                // Guarantee at least a 1-unit nudge when z is non-zero and drift_percent >= 5%
                let raw_delta = (z / 1.5) * pct * span;
                let delta = if raw_delta.abs() < 1.0 && pct >= 0.05 {
                    if z >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                } else {
                    raw_delta
                };
                (f64::from(current) + delta)
                    .round()
                    .clamp(f64::from(low), f64::from(high)) as u8
            }
        }
    }

    /// Mutates all unlocked continuous controls in `pipeline` that match `self.scope`,
    /// updating their software target values (and engaging Soft Takeover) while returning a
    /// reversible [`RandomizationResult`].
    pub fn mutate_pipeline(&mut self, pipeline: &mut DeviceMappingPipeline) -> RandomizationResult {
        let paths: Vec<(String, u8, u8)> = pipeline
            .config
            .entries
            .values()
            .filter(|e| {
                !e.is_button
                    && self.scope.matches_control_path(&e.control_path)
                    && !self.is_control_shielded(&e.control_path)
            })
            .map(|e| (e.control_path.clone(), e.min_value, e.max_value))
            .collect();

        let mut control_deltas = BTreeMap::new();
        for (path, min_val, max_val) in paths {
            let before = pipeline
                .soft_takeover_status(&path)
                .map(|s| s.target_value)
                .unwrap_or(64);
            let after = self.sample_mutated_value(before, min_val, max_val);
            pipeline.set_target_value(&path, after);
            control_deltas.insert(path, (before, after));
        }

        RandomizationResult {
            label: format!(
                "{:?} ({:.0}%)",
                self.mode,
                self.drift_percent.clamp(1.0, 100.0)
            ),
            control_deltas,
            mutated_lanes: Vec::new(),
        }
    }

    /// Mutates all unlocked `8opPerc` voice lanes in `model` that match `self.scope`,
    /// respecting both `self.locked_lanes` and `model.lanes[i].timbre_locked`.
    pub fn mutate_op8_model(&mut self, model: &mut Op8PerformanceModel) -> RandomizationResult {
        let mut control_deltas = BTreeMap::new();
        let mut mutated_lanes = Vec::new();

        for i in 0..8 {
            if !self.scope.matches_lane(i) {
                continue;
            }
            if self.locked_lanes[i] || model.lanes[i].timbre_locked {
                continue;
            }

            let knob_path = format!("track{}.knob", i + 1);
            if !self.locked_controls.contains(&knob_path) {
                let before_cc = model.lanes[i].density_cc;
                let after_cc = self.sample_mutated_value(before_cc, 0, 127);
                model.lanes[i].density_cc = after_cc;
                model.lanes[i].density_multiplier =
                    cc_to_density(after_cc).clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);
                let _ = density_to_cc(model.lanes[i].density_multiplier);
                control_deltas.insert(knob_path, (before_cc, after_cc));
            }

            let slider_path = format!("track{}.slider", i + 1);
            if !self.locked_controls.contains(&slider_path) {
                let before_gain = model.lanes[i].gain_cc;
                let after_gain = self.sample_mutated_value(before_gain, 20, 127);
                model.lanes[i].gain_cc = after_gain;
                model.lanes[i].gain_normalized = f64::from(after_gain) / 127.0;
                control_deltas.insert(slider_path, (before_gain, after_gain));
            }

            if self.reroll_unlocked_timbres {
                let next_count = model.lanes[i].reroll_count + 1;
                model.lanes[i].reroll_count = next_count;
                let next_timbre = format!("{}-rand-{next_count:04}", FAMILY_ORDER[i]);
                if model.queue_timbres_for_next_bar {
                    model.lanes[i].queued_timbre = Some(next_timbre);
                } else {
                    model.lanes[i].active_timbre = next_timbre;
                }
            }

            mutated_lanes.push(i);
        }

        RandomizationResult {
            label: format!(
                "8opPerc {:?} ({:.0}%)",
                self.mode,
                self.drift_percent.clamp(1.0, 100.0)
            ),
            control_deltas,
            mutated_lanes,
        }
    }
}

// =============================================================================
// 4. Interactive egui Randomizer & Lock Shield Panel
// =============================================================================

/// Renders the Intelligent Randomization & Mutation Engine panel in `egui`.
pub fn render_randomizer_panel(
    ui: &mut Ui,
    randomizer: &mut RandomizerEngine,
    pipeline: &mut DeviceMappingPipeline,
    op8_model: &mut Op8PerformanceModel,
    theme_settings: &ThemeSettings,
) -> Option<RandomizationResult> {
    let palette = theme_settings.resolved_palette();
    let panel_fill = theme_settings.build_visuals().panel_fill;
    let mut result_out: Option<RandomizationResult> = None;

    egui::Frame::none()
        .fill(panel_fill)
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("INTELLIGENT RANDOMIZER & GAUSSIAN DRIFT")
                        .strong()
                        .color(palette.accent),
                );
                ui.separator();
                for preset in MusicalRandomPreset::ALL {
                    if ui.small_button(preset.title()).clicked() {
                        randomizer.apply_musical_preset(preset);
                    }
                }
            });

            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Scope:").small());
                if ui
                    .selectable_label(randomizer.scope == RandomizationScope::Global, "Global")
                    .clicked()
                {
                    randomizer.scope = RandomizationScope::Global;
                }
                for t in 1..=8 {
                    let is_sel = randomizer.scope == RandomizationScope::SelectedTrack(t);
                    if ui.selectable_label(is_sel, format!("Trk {t}")).clicked() {
                        randomizer.scope = RandomizationScope::SelectedTrack(t);
                    }
                }

                ui.separator();
                let mut drift = randomizer.drift_percent as f32;
                if ui
                    .add(
                        egui::Slider::new(&mut drift, 1.0..=100.0)
                            .suffix("%")
                            .text("Gaussian Drift"),
                    )
                    .changed()
                {
                    randomizer.set_drift_percent(f64::from(drift));
                }

                if ui.button("🎲 Mutate Rig").clicked() {
                    let _ = randomizer.mutate_pipeline(pipeline);
                    result_out = Some(randomizer.mutate_op8_model(op8_model));
                }
            });

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("LOCK SHIELDS:").small().weak());
                for (i, fam) in FAMILY_ORDER.iter().enumerate() {
                    let shielded = randomizer.locked_lanes[i] || op8_model.lanes[i].timbre_locked;
                    let lbl = if shielded {
                        format!("🛡 {}", fam.to_ascii_uppercase())
                    } else {
                        fam.to_ascii_uppercase()
                    };
                    if ui.selectable_label(shielded, RichText::new(lbl).small()).clicked() {
                        let next = !shielded;
                        randomizer.locked_lanes[i] = next;
                        op8_model.lanes[i].timbre_locked = next;
                    }
                }
            });
        });

    result_out
}
