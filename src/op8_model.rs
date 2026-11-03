//! `8opPerc` Performance Domain Model, Native `nanoKONTROL2` Mapper & `MIDImix` Emulation (`src/op8_model.rs`).
//!
//! Models the complete 8-voice percussion live rig from `8operc-konduktiva` (`pulsed-live-rig.js`,
//! `live-density.js`, `families.js`, `live-timbre-layout.js`) and `docs/NKEDITOR-RUST-PORT-SPEC.md`:
//! - All 8 percussion voice lanes ([`FAMILY_ORDER`] / [`KONDUKTIVA_FAMILY_ORDER`])
//! - Per-lane density multipliers (`0.1 ..= 5.0`, exponential ladder `0.1 * 50^(cc / 127)`)
//! - Per-lane macro/gain sliders (`0..=127` / normalized `0.0..=1.0`)
//! - Per-lane Mute and new Solo states (including solo-priority audibility calculation)
//! - Per-lane active and queued timbre names (for bar-boundary patch preloading)
//! - Global density multiplier (`0.1 ..= 5.0`)
//! - Global rhythmic feel scale (`swing` `0.0..=0.5` and `humanize` `0.0..=1.0`, scaled by
//!   `0.0..=1.0` with exact midpoint `CC 64 = 0.5`)
//! - 4,096-entry form bank state (`0..4096`, with navigation history & 4-form prefetch window)
//! - 4,096-entry timbre bank state (`0..4096`, with refill threshold `1024` & FNV-1a distributed
//!   preset pool capture)
//! - **Native Korg `nanoKONTROL2` Performance Mapping** (`Knobs CC 16..23`, `Sliders CC 0..7`,
//!   `Solo CC 32..39`, `Mute CC 48..55`, `Rec CC 64..71`, `Track L/R CC 58/59`, `Cycle CC 46`,
//!   `Marker Set CC 60`, `Marker L/R CC 61/62`, `Rewind/Forward CC 43/44`, `Play/Stop/Rec CC 41/42/45`)
//! - **`MIDImix` Emulation Mode** translating `nanoKONTROL2` CCs into factory Akai `MIDI Mix`
//!   Notes and CCs (including the 32-CC `Send All` burst on `Marker Set CC 60`).

use crate::devices::MidiMessageType;
use crate::osc_server::{Op8RigState, Op8VoiceLaneState};
use serde::{Deserialize, Serialize};

// =============================================================================
// 1. Canonical Constants & Family Tables
// =============================================================================

/// Minimum density multiplier (`0.1x`, matching `MIN_DENSITY_SCALE` in `live-density.js`).
pub const MIN_DENSITY_SCALE: f64 = 0.1;

/// Maximum density multiplier (`5.0x`, matching `MAX_DENSITY_SCALE` in `live-density.js`).
pub const MAX_DENSITY_SCALE: f64 = 5.0;

/// Default density quantization grid in beats (`1/32` note = `0.125` beats).
pub const DEFAULT_DENSITY_QUANTIZE_BEATS: f64 = 0.125;

/// Canonical 4,096-entry form reservoir size (`FORM_BANK_SIZE` in `pulsed-live-rig.js`).
pub const FORM_BANK_SIZE: usize = 4096;

/// Canonical nearby form prefetch window (`FORM_BANK_PREFETCH` in `pulsed-live-rig.js`).
pub const FORM_BANK_PREFETCH: usize = 4;

/// Canonical 4,096-entry timbre set reservoir size (`TIMBRE_BANK_SIZE` in `pulsed-live-rig.js`).
pub const TIMBRE_BANK_SIZE: usize = 4096;

/// Threshold at which the timbre bank triggers background refill (`TIMBRE_BANK_REFILL_AT`).
pub const TIMBRE_BANK_REFILL_AT: usize = 1024;

/// Canonical 8-voice percussion lane names in `nkEditor4` (`FAMILY_ORDER`).
pub const FAMILY_ORDER: [&str; 8] = [
    "kick",
    "snare",
    "hat_closed",
    "hat_open",
    "clap",
    "tom",
    "rim",
    "cymbal",
];

/// Canonical 8-family names in `8operc-konduktiva/src/families.js`.
pub const KONDUKTIVA_FAMILY_ORDER: [&str; 8] = [
    "kicks", "snares", "hats", "toms", "bells", "metals", "noise", "fx",
];

/// Live player labels used by `8operc-konduktiva/src/live-timbre-layout.js`
/// (`bells -> percussion`, `metals -> alt-toms`).
pub const KONDUKTIVA_LIVE_LABELS: [&str; 8] = [
    "kicks",
    "snares",
    "hats",
    "toms",
    "percussion",
    "alt-toms",
    "noise",
    "fx",
];

/// Default boot patch names for each of the 8 voice lanes.
pub const DEFAULT_BOOT_PATCHES: [&str; 8] = [
    "kick-sub",
    "snare",
    "closed-hat",
    "open-hat",
    "clap",
    "tom",
    "rim",
    "cymbal",
];

// =============================================================================
// 2. Korg nanoKONTROL2 & Akai MIDImix Control Constants
// =============================================================================

/// Korg `nanoKONTROL2` Track Knobs 1..8 (`CC 16..=23`) -> Per-lane density (`0.1..=5.0`).
pub const NK2_DENSITY_KNOB_CCS: [u8; 8] = [16, 17, 18, 19, 20, 21, 22, 23];

/// Korg `nanoKONTROL2` Track Sliders 1..8 (`CC 0..=7`) -> Per-lane macro/gain (`0..=127`).
pub const NK2_GAIN_SLIDER_CCS: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

/// Korg `nanoKONTROL2` Track Solo Buttons 1..8 (`CC 32..=39`) -> Per-lane solo toggle.
pub const NK2_SOLO_BUTTON_CCS: [u8; 8] = [32, 33, 34, 35, 36, 37, 38, 39];

/// Korg `nanoKONTROL2` Track Mute Buttons 1..8 (`CC 48..=55`) -> Per-lane mute toggle.
pub const NK2_MUTE_BUTTON_CCS: [u8; 8] = [48, 49, 50, 51, 52, 53, 54, 55];

/// Korg `nanoKONTROL2` Track Rec Buttons 1..8 (`CC 64..=71`) -> Single-lane timbre reroll.
pub const NK2_REC_REROLL_CCS: [u8; 8] = [64, 65, 66, 67, 68, 69, 70, 71];

/// Korg `nanoKONTROL2` Transport CC numbers.
pub const NK2_CC_PLAY: u8 = 41;
pub const NK2_CC_STOP: u8 = 42;
pub const NK2_CC_REWIND: u8 = 43;
pub const NK2_CC_FORWARD: u8 = 44;
pub const NK2_CC_RECORD: u8 = 45;
pub const NK2_CC_CYCLE: u8 = 46;
pub const NK2_CC_TRACK_LEFT: u8 = 58;
pub const NK2_CC_TRACK_RIGHT: u8 = 59;
pub const NK2_CC_MARKER_SET: u8 = 60;
pub const NK2_CC_MARKER_LEFT: u8 = 61;
pub const NK2_CC_MARKER_RIGHT: u8 = 62;

/// Factory Akai `MIDI Mix` Mute button notes (`Note 1, 4, 7, 10, 13, 16, 19, 22`).
pub const MIDIMIX_MUTE_NOTES: [u8; 8] = [1, 4, 7, 10, 13, 16, 19, 22];

/// Factory Akai `MIDI Mix` Solo button notes (`Note 2, 5, 8, 11, 14, 17, 20, 23`).
pub const MIDIMIX_SOLO_NOTES: [u8; 8] = [2, 5, 8, 11, 14, 17, 20, 23];

/// Factory Akai `MIDI Mix` Rec Arm button notes (`Note 3, 6, 9, 12, 15, 18, 21, 24`).
pub const MIDIMIX_REC_ARM_NOTES: [u8; 8] = [3, 6, 9, 12, 15, 18, 21, 24];

/// Factory Akai `MIDI Mix` Row 2 voice density CCs (`CC 17, 21, 25, 29, 47, 51, 55, 59`).
pub const MIDIMIX_VOICE_DENSITY_CCS: [u8; 8] = [17, 21, 25, 29, 47, 51, 55, 59];

/// Factory Akai `MIDI Mix` Channel Strip Fader CCs (`CC 19, 23, 27, 31, 49, 53, 57, 61`).
pub const MIDIMIX_FADER_CCS: [u8; 8] = [19, 23, 27, 31, 49, 53, 57, 61];

/// Factory Akai `MIDI Mix` Global Feel controller (`CC 58`).
pub const MIDIMIX_FEEL_CC: u8 = 58;

/// Factory Akai `MIDI Mix` Global Density controller (`CC 61`).
pub const MIDIMIX_GLOBAL_DENSITY_CC: u8 = 61;

/// Factory Akai `MIDI Mix` Bank Left (`Note 25`), Bank Right (`Note 26`), and Solo Master (`Note 27`).
pub const MIDIMIX_BANK_LEFT_NOTE: u8 = 25;
pub const MIDIMIX_BANK_RIGHT_NOTE: u8 = 26;
pub const MIDIMIX_TIMBRE_SET_NOTE: u8 = 27;

/// Factory Akai `MIDI Mix` 32-CC `Send All` burst controller numbers (`CC 16..=31` and `CC 46..=61`).
pub const MIDIMIX_SEND_ALL_CCS: [u8; 32] = [
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 46, 47, 48, 49, 50, 51, 52, 53,
    54, 55, 56, 57, 58, 59, 60, 61,
];

// =============================================================================
// 3. Mathematical Scaling Functions (Density & Rhythmic Feel)
// =============================================================================

/// Maps a 7-bit MIDI CC value (`0..=127`) onto the exponential `8opPerc` density ladder
/// (`0.1 ..= 5.0`), matching `mapMidiCCToDensity(cc)` in `live-density.js`:
/// `0.1 * (5.0 / 0.1)^(cc / 127)`.
pub fn cc_to_density(cc: u8) -> f64 {
    let clamped = cc.min(127) as f64;
    MIN_DENSITY_SCALE * (MAX_DENSITY_SCALE / MIN_DENSITY_SCALE).powf(clamped / 127.0)
}

/// Inverse of [`cc_to_density`]: converts a density multiplier (`0.1 ..= 5.0`) back to the
/// nearest 7-bit MIDI CC value (`0..=127`).
pub fn density_to_cc(density: f64) -> u8 {
    let clamped = density.clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);
    let ratio = clamped / MIN_DENSITY_SCALE;
    let max_ratio = MAX_DENSITY_SCALE / MIN_DENSITY_SCALE;
    let normalized = ratio.ln() / max_ratio.ln();
    (normalized * 127.0).round().clamp(0.0, 127.0) as u8
}

/// Maps a 7-bit MIDI CC value (`0..=127`) onto a normalized feel scale (`0.0 ..= 1.0`) with an
/// **exact** midpoint at `CC 64 = 0.5`:
/// - `CC 0   -> 0.0`
/// - `CC 64  -> 0.5`
/// - `CC 127 -> 1.0`
pub fn cc_to_feel_scale(cc: u8) -> f64 {
    let c = cc.min(127);
    if c <= 64 {
        (c as f64) / 128.0
    } else {
        0.5 + ((c - 64) as f64) / 126.0
    }
}

/// Inverse of [`cc_to_feel_scale`]: converts a feel scale (`0.0 ..= 1.0`) to `0..=127` with
/// `0.5 -> CC 64`.
pub fn feel_scale_to_cc(scale: f64) -> u8 {
    let s = scale.clamp(0.0, 1.0);
    if s <= 0.5 {
        (s * 128.0).round().clamp(0.0, 64.0) as u8
    } else {
        (64.0 + (s - 0.5) * 126.0).round().clamp(64.0, 127.0) as u8
    }
}

/// Inverted feel scale curve used by legacy `pulsed-live-rig.js` (`mapMidiCCToFeelScale`),
/// where `CC 0 = 1.0` (untouched authored feel), `CC 64 = 0.5` (exact midpoint), and `CC 127 = 0.0`.
pub fn cc_to_midimix_feel_scale(cc: u8) -> f64 {
    let c = cc.min(127);
    if c <= 64 {
        1.0 - (c as f64) / 128.0
    } else {
        (127.0 - c as f64) / 126.0
    }
}

/// Computes the deterministic FNV-1a 32-bit slot index within a bank of `length` entries,
/// matching `timbrePoolIndex(id, length)` and `alternateTimbreName(value, pool)` in
/// `8operc-konduktiva`.
pub fn fnv1a_pool_index(text: &str, length: usize) -> usize {
    if length == 0 {
        return 0;
    }
    let mut hash: u32 = 2_166_136_261;
    for byte in text.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    (hash as usize) % length
}

/// Resolves a lane identifier (`"0".."7"`, `"track1".."track8"`, `"kick"`, `"kicks"`,
/// `"bells"`, `"percussion"`, `"metals"`, `"alt-toms"`, etc.) to a 0-based lane index (`0..8`).
pub fn resolve_lane_index(ident: &str) -> Option<usize> {
    let norm = ident.trim().to_ascii_lowercase();
    if let Ok(idx) = norm.parse::<usize>() {
        if idx < 8 {
            return Some(idx);
        }
    }
    if let Some(rest) = norm.strip_prefix("track") {
        if let Ok(track_num) = rest.parse::<usize>() {
            if (1..=8).contains(&track_num) {
                return Some(track_num - 1);
            }
        }
    }
    for i in 0..8 {
        if FAMILY_ORDER[i] == norm
            || KONDUKTIVA_FAMILY_ORDER[i] == norm
            || KONDUKTIVA_LIVE_LABELS[i] == norm
        {
            return Some(i);
        }
    }
    match norm.as_str() {
        "closed-hat" | "closed_hat" => Some(2),
        "open-hat" | "open_hat" => Some(3),
        _ => None,
    }
}

// =============================================================================
// 4. Domain Model Structs
// =============================================================================

/// State of a single percussion voice lane (`0..8`) in the `8opPerc` performance model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8VoiceLane {
    /// 0-based lane index (`0..8`).
    pub index: usize,
    /// Canonical `nkEditor4` family identifier (`"kick"`, `"snare"`, `"hat_closed"`, etc.).
    pub family: String,
    /// Canonical `8operc-konduktiva` family identifier (`"kicks"`, `"snares"`, `"hats"`, etc.).
    pub konduktiva_family: String,
    /// Live player label (`"percussion"` for lane 4, `"alt-toms"` for lane 5, etc.).
    pub live_label: String,
    /// Per-lane density multiplier (`0.1 ..= 5.0`, default `1.0`).
    pub density_multiplier: f64,
    /// Last 7-bit CC value driving this lane's density knob (`0..=127`).
    pub density_cc: u8,
    /// Per-lane macro/gain slider 7-bit CC value (`0..=127`, default `100`).
    pub gain_cc: u8,
    /// Normalized per-lane macro/gain level (`0.0 ..= 1.0`).
    pub gain_normalized: f64,
    /// Whether this lane is muted (`trackN.mute`).
    pub muted: bool,
    /// Whether this lane is soloed (`trackN.solo`).
    pub soloed: bool,
    /// Whether this lane's timbre is locked (preventing global timbre set changes from overwriting it).
    #[serde(default)]
    pub timbre_locked: bool,
    /// Currently sounding FM timbre preset name.
    pub active_timbre: String,
    /// Preloaded/queued FM timbre preset scheduled for the next bar boundary (`None` if already active).
    pub queued_timbre: Option<String>,
    /// Number of single-lane or full-set timbre rerolls applied to this lane.
    pub reroll_count: u32,
}

impl Op8VoiceLane {
    /// Creates a new voice lane with default live performance parameters for `index` (`0..8`).
    pub fn new(index: usize) -> Self {
        let idx = index.min(7);
        let default_density_cc = density_to_cc(1.0);
        let default_gain_cc = 100u8;
        Self {
            index: idx,
            family: FAMILY_ORDER[idx].to_string(),
            konduktiva_family: KONDUKTIVA_FAMILY_ORDER[idx].to_string(),
            live_label: KONDUKTIVA_LIVE_LABELS[idx].to_string(),
            density_multiplier: 1.0,
            density_cc: default_density_cc,
            gain_cc: default_gain_cc,
            gain_normalized: f64::from(default_gain_cc) / 127.0,
            muted: false,
            soloed: false,
            timbre_locked: false,
            active_timbre: DEFAULT_BOOT_PATCHES[idx].to_string(),
            queued_timbre: None,
            reroll_count: 0,
        }
    }

    /// Updates this lane's density from a 7-bit MIDI CC value (`0..=127`).
    pub fn set_density_from_cc(&mut self, cc: u8) -> f64 {
        let clamped = cc.min(127);
        self.density_cc = clamped;
        self.density_multiplier = cc_to_density(clamped);
        self.density_multiplier
    }

    /// Updates this lane's density multiplier directly (`0.1 ..= 5.0`).
    pub fn set_density_multiplier(&mut self, density: f64) -> f64 {
        let clamped = density.clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);
        self.density_multiplier = clamped;
        self.density_cc = density_to_cc(clamped);
        self.density_multiplier
    }

    /// Updates this lane's macro/gain slider from a 7-bit MIDI CC value (`0..=127`).
    pub fn set_gain_from_cc(&mut self, cc: u8) -> f64 {
        let clamped = cc.min(127);
        self.gain_cc = clamped;
        self.gain_normalized = f64::from(clamped) / 127.0;
        self.gain_normalized
    }

    /// Queues a new timbre for the next bar boundary (or applies immediately if `immediate == true`).
    pub fn schedule_timbre(&mut self, timbre_name: impl Into<String>, immediate: bool) {
        let name = timbre_name.into();
        self.reroll_count += 1;
        if immediate {
            self.active_timbre = name;
            self.queued_timbre = None;
        } else {
            self.queued_timbre = Some(name);
        }
    }

    /// Commits any queued timbre into `active_timbre` (called at the downbeat/bar boundary).
    pub fn commit_queued_timbre(&mut self) -> bool {
        if let Some(next) = self.queued_timbre.take() {
            self.active_timbre = next;
            true
        } else {
            false
        }
    }
}

/// Global rhythmic feel state (`swing` and `humanize` scaled by `scale` in `0.0 ..= 1.0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8RhythmicFeel {
    /// Global feel scale (`0.0 ..= 1.0`, exact midpoint `CC 64 = 0.5`).
    pub scale: f64,
    /// Last 7-bit MIDI CC value (`0..=127`, default `64` -> `0.5`).
    pub feel_cc: u8,
    /// Authored template swing (`0.0 ..= 0.5`, default `0.24`).
    pub authored_swing: f64,
    /// Authored template humanize (`0.0 ..= 1.0`, default `0.36`).
    pub authored_humanize: f64,
    /// Effective swing applied to the score (`authored_swing * scale`).
    pub effective_swing: f64,
    /// Effective humanize applied to the score (`authored_humanize * scale`).
    pub effective_humanize: f64,
}

impl Default for Op8RhythmicFeel {
    fn default() -> Self {
        let authored_swing = 0.24;
        let authored_humanize = 0.36;
        let scale = 0.5;
        Self {
            scale,
            feel_cc: 64,
            authored_swing,
            authored_humanize,
            effective_swing: authored_swing * scale,
            effective_humanize: authored_humanize * scale,
        }
    }
}

impl Op8RhythmicFeel {
    /// Updates the rhythmic feel scale from a 7-bit MIDI CC value (`0..=127`), guaranteeing
    /// `CC 64 == 0.5`.
    pub fn set_from_cc(&mut self, cc: u8) -> f64 {
        let clamped = cc.min(127);
        self.feel_cc = clamped;
        self.scale = cc_to_feel_scale(clamped);
        self.recompute_effective();
        self.scale
    }

    /// Updates the rhythmic feel scale directly (`0.0 ..= 1.0`).
    pub fn set_scale(&mut self, scale: f64) -> f64 {
        let clamped = scale.clamp(0.0, 1.0);
        self.scale = clamped;
        self.feel_cc = feel_scale_to_cc(clamped);
        self.recompute_effective();
        self.scale
    }

    /// Recomputes `effective_swing` and `effective_humanize` from `authored_* * scale`.
    pub fn recompute_effective(&mut self) {
        self.effective_swing = (self.authored_swing * self.scale).clamp(0.0, 0.5);
        self.effective_humanize = (self.authored_humanize * self.scale).clamp(0.0, 1.0);
    }
}

/// Direction for navigating the 4,096-entry form bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormDirection {
    Previous,
    Next,
}

/// State of the 4,096-entry musical form bank (`FORM_BANK_SIZE = 4096`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8FormBankState {
    /// Total forms in the pregenerated catalogue (`4096`).
    pub total_entries: usize,
    /// Currently active form index (`0..total_entries`).
    pub active_index: usize,
    /// Human-readable active form name (e.g. `"form-0001"`).
    pub active_form_name: String,
    /// Number of nearby forms kept hydrated in memory (`4`).
    pub prefetch_window: usize,
    /// Visited form index history for deterministic back/forward navigation.
    pub history: Vec<usize>,
    /// Current cursor inside `history`.
    pub history_cursor: usize,
}

impl Default for Op8FormBankState {
    fn default() -> Self {
        Self {
            total_entries: FORM_BANK_SIZE,
            active_index: 0,
            active_form_name: "form-0001".to_string(),
            prefetch_window: FORM_BANK_PREFETCH,
            history: vec![0],
            history_cursor: 0,
        }
    }
}

impl Op8FormBankState {
    /// Steps to the previous or next form in the 4,096-entry catalogue.
    pub fn step(&mut self, direction: FormDirection) -> (usize, String) {
        let total = self.total_entries.max(1);
        match direction {
            FormDirection::Next => {
                self.active_index = (self.active_index + 1) % total;
            }
            FormDirection::Previous => {
                self.active_index = (self.active_index + total - 1) % total;
            }
        }
        self.active_form_name = format!("form-{:04}", self.active_index + 1);
        self.history.push(self.active_index);
        self.history_cursor = self.history.len() - 1;
        (self.active_index, self.active_form_name.clone())
    }

    /// Selects a specific form by index (`0..total_entries`).
    pub fn select_index(&mut self, index: usize) -> (usize, String) {
        let total = self.total_entries.max(1);
        self.active_index = index % total;
        self.active_form_name = format!("form-{:04}", self.active_index + 1);
        self.history.push(self.active_index);
        self.history_cursor = self.history.len() - 1;
        (self.active_index, self.active_form_name.clone())
    }

    /// Returns the indices of the next `prefetch_window` forms to keep prewarmed.
    pub fn prefetched_indices(&self) -> Vec<usize> {
        let total = self.total_entries.max(1);
        (1..=self.prefetch_window)
            .map(|offset| (self.active_index + offset) % total)
            .collect()
    }
}

/// Atomic 8-voice timbre preset snapshot captured into the preset pool (`Marker Set CC 60` or `Send All`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op8PresetSnapshot {
    /// Unique snapshot identifier.
    pub id: String,
    /// Optional human-readable label.
    pub label: String,
    /// Pipe-delimited canonical signature of all 8 active timbres (`timbreSetSignature`).
    pub signature: String,
    /// Active timbre name for each of the 8 lanes (`FAMILY_ORDER`).
    pub timbres: [String; 8],
    /// Deterministic FNV-1a index (`0..4096`) where this snapshot is injected into the timbre bank.
    pub pool_bank_index: usize,
}

/// State of the 4,096-entry timbre set bank (`TIMBRE_BANK_SIZE = 4096`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8TimbreBankState {
    /// Total 8-voice timbre sets in the reservoir (`4096`).
    pub total_entries: usize,
    /// Remaining sets threshold that triggers background refill (`1024`).
    pub refill_threshold: usize,
    /// Currently selected timbre set index (`0..4096`).
    pub active_set_index: usize,
    /// Number of unconsumed sets remaining before refill.
    pub remaining_sets: usize,
    /// Captured 8-voice preset snapshots (`midimix-random-preset-pool.jsonl` equivalents).
    pub captured_pool: Vec<Op8PresetSnapshot>,
}

impl Default for Op8TimbreBankState {
    fn default() -> Self {
        Self {
            total_entries: TIMBRE_BANK_SIZE,
            refill_threshold: TIMBRE_BANK_REFILL_AT,
            active_set_index: 0,
            remaining_sets: TIMBRE_BANK_SIZE,
            captured_pool: Vec::new(),
        }
    }
}

impl Op8TimbreBankState {
    /// Returns `true` if `remaining_sets <= refill_threshold` (`1024`).
    pub fn needs_refill(&self) -> bool {
        self.remaining_sets <= self.refill_threshold
    }

    /// Refills the timbre bank back to `total_entries` (`4096`).
    pub fn refill(&mut self) {
        self.remaining_sets = self.total_entries;
    }
}

/// Konduktiva live transport and WAV stem/mix capture state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8TransportState {
    /// Whether the Konduktiva scheduler transport is playing.
    pub playing: bool,
    /// Whether `pw-record` WAV mix capture is active.
    pub recording_wav: bool,
    /// Current tempo in beats per minute.
    pub bpm: f64,
    /// Current beat position.
    pub current_beat: f64,
    /// Beats per bar (default `4`).
    pub beats_per_bar: u8,
}

impl Default for Op8TransportState {
    fn default() -> Self {
        Self {
            playing: false,
            recording_wav: false,
            bpm: 132.0,
            current_beat: 0.0,
            beats_per_bar: 4,
        }
    }
}

// =============================================================================
// 5. Complete 8opPerc Performance Model & Native nanoKONTROL2 Mapper
// =============================================================================

/// High-level semantic action produced when a physical `nanoKONTROL2` control is moved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op8PerformanceAction {
    /// Track Knob 1..8 (`CC 16..23`) changed per-lane density (`0.1 ..= 5.0`).
    SetLaneDensity {
        lane: usize,
        family: String,
        cc_value: u8,
        density: f64,
        effective_density: f64,
    },
    /// Track Slider 1..8 (`CC 0..7`) changed per-lane macro/gain (`0..=127`).
    SetLaneGain {
        lane: usize,
        family: String,
        cc_value: u8,
        gain_normalized: f64,
    },
    /// Track Solo Button 1..8 (`CC 32..39`) toggled lane solo state.
    ToggleLaneSolo {
        lane: usize,
        family: String,
        soloed: bool,
        audible_mask: [bool; 8],
    },
    /// Track Mute Button 1..8 (`CC 48..55`) toggled lane mute state.
    ToggleLaneMute {
        lane: usize,
        family: String,
        muted: bool,
        audible_mask: [bool; 8],
    },
    /// Track Rec Button 1..8 (`CC 64..71`) rerolled single-lane timbre.
    RerollLaneTimbre {
        lane: usize,
        family: String,
        active_timbre: String,
        queued_timbre: Option<String>,
        reroll_count: u32,
    },
    /// Track Left / Right (`CC 58 / 59`) navigated the 4,096-entry form bank.
    NavigateForm {
        direction: FormDirection,
        active_index: usize,
        active_form_name: String,
    },
    /// Cycle (`CC 46`) advanced to the next full 8-voice timbre set in the 4,096-entry reservoir.
    AdvanceTimbreSet {
        active_set_index: usize,
        timbres: [String; 8],
    },
    /// Marker Set (`CC 60`) atomically captured the current 8-voice timbre snapshot into the preset pool.
    CapturePresetPoolSnapshot(Op8PresetSnapshot),
    /// Marker Left / Right (`CC 61 / 62`) stepped the global density multiplier (`0.1 ..= 5.0`).
    StepGlobalDensity {
        delta_cc: i16,
        cc_value: u8,
        global_density: f64,
    },
    /// Rewind / Forward (`CC 43 / 44`) stepped the global rhythmic feel scale (`0.0 ..= 1.0`).
    StepGlobalFeel {
        delta_cc: i16,
        cc_value: u8,
        feel_scale: f64,
    },
    /// Play (`CC 41`) started transport playback.
    TransportPlay { playing: bool },
    /// Stop (`CC 42`) stopped transport playback.
    TransportStop { playing: bool },
    /// Record (`CC 45`) toggled live WAV stem/mix capture indicator.
    TransportRecord { recording_wav: bool },
}

/// Complete `8opPerc` performance domain model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8PerformanceModel {
    /// The 8 percussion voice lanes (`FAMILY_ORDER`).
    pub lanes: [Op8VoiceLane; 8],
    /// Global rhythmic density multiplier (`0.1 ..= 5.0`, default `1.0`).
    pub global_density_multiplier: f64,
    /// Last 7-bit CC value for global density (`0..=127`).
    pub global_density_cc: u8,
    /// Global rhythmic feel state (`swing` & `humanize`, `CC 64 = 0.5`).
    pub feel: Op8RhythmicFeel,
    /// 4,096-entry musical form bank state.
    pub form_bank: Op8FormBankState,
    /// 4,096-entry 8-voice timbre set reservoir state.
    pub timbre_bank: Op8TimbreBankState,
    /// Transport and WAV recording state.
    pub transport: Op8TransportState,
    /// Step size (in CC units) for Marker L/R global density and Rewind/Forward global feel buttons.
    pub step_cc_delta: u8,
    /// Whether single-lane or full-set timbre rerolls queue for next bar (`true`) or commit immediately (`false`).
    pub queue_timbres_for_next_bar: bool,
}

impl Default for Op8PerformanceModel {
    fn default() -> Self {
        Self::new()
    }
}

impl Op8PerformanceModel {
    /// Creates a new `Op8PerformanceModel` initialized with canonical defaults.
    pub fn new() -> Self {
        Self {
            lanes: std::array::from_fn(Op8VoiceLane::new),
            global_density_multiplier: 1.0,
            global_density_cc: density_to_cc(1.0),
            feel: Op8RhythmicFeel::default(),
            form_bank: Op8FormBankState::default(),
            timbre_bank: Op8TimbreBankState::default(),
            transport: Op8TransportState::default(),
            step_cc_delta: 8,
            queue_timbres_for_next_bar: true,
        }
    }

    /// Returns `true` if at least one voice lane is currently soloed.
    pub fn any_lane_soloed(&self) -> bool {
        self.lanes.iter().any(|l| l.soloed)
    }

    /// Returns whether lane `index` (`0..8`) is currently audible, honoring both Solo priority
    /// and Mute state:
    /// - If any lane is soloed, a lane is audible iff `lane.soloed && !lane.muted`.
    /// - Otherwise, a lane is audible iff `!lane.muted`.
    pub fn is_lane_audible(&self, index: usize) -> bool {
        let Some(lane) = self.lanes.get(index) else {
            return false;
        };
        if lane.muted {
            return false;
        }
        if self.any_lane_soloed() {
            lane.soloed
        } else {
            true
        }
    }

    /// Returns the 8-element audibility mask across all percussion lanes.
    pub fn audible_mask(&self) -> [bool; 8] {
        std::array::from_fn(|i| self.is_lane_audible(i))
    }

    /// Computes the effective density multiplier for lane `index` (`global_density * lane_density`),
    /// matching `live-density.js` (`this.currentScale * state.currentScale`).
    pub fn effective_lane_density(&self, index: usize) -> f64 {
        let lane_scale = self
            .lanes
            .get(index)
            .map(|l| l.density_multiplier)
            .unwrap_or(1.0);
        self.global_density_multiplier * lane_scale
    }

    /// Sets the global density multiplier from a 7-bit MIDI CC value (`0..=127`).
    pub fn set_global_density_from_cc(&mut self, cc: u8) -> f64 {
        let clamped = cc.min(127);
        self.global_density_cc = clamped;
        self.global_density_multiplier = cc_to_density(clamped);
        self.global_density_multiplier
    }

    /// Sets the global density multiplier directly (`0.1 ..= 5.0`).
    pub fn set_global_density_multiplier(&mut self, density: f64) -> f64 {
        let clamped = density.clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);
        self.global_density_multiplier = clamped;
        self.global_density_cc = density_to_cc(clamped);
        self.global_density_multiplier
    }

    /// Rerolls the timbre of a single lane (`0..8`).
    pub fn reroll_lane_timbre(&mut self, lane_idx: usize) -> Option<(String, Option<String>)> {
        let lane = self.lanes.get_mut(lane_idx)?;
        let next_num = lane.reroll_count + 1;
        let next_name = format!("{}-var-{:04}", lane.family, next_num);
        lane.schedule_timbre(next_name, !self.queue_timbres_for_next_bar);
        Some((lane.active_timbre.clone(), lane.queued_timbre.clone()))
    }

    /// Advances to the next full 8-voice timbre set in the 4,096-entry reservoir (`Cycle CC 46`).
    pub fn advance_full_timbre_set(&mut self) -> (usize, [String; 8]) {
        let total = self.timbre_bank.total_entries.max(1);
        self.timbre_bank.active_set_index = (self.timbre_bank.active_set_index + 1) % total;
        if self.timbre_bank.remaining_sets > 0 {
            self.timbre_bank.remaining_sets -= 1;
        }
        if self.timbre_bank.needs_refill() {
            self.timbre_bank.refill();
        }
        let set_num = self.timbre_bank.active_set_index + 1;
        for lane in &mut self.lanes {
            if lane.timbre_locked {
                continue;
            }
            let next_name = format!("{}-set-{:04}", lane.family, set_num);
            lane.schedule_timbre(next_name, !self.queue_timbres_for_next_bar);
        }
        let timbres = std::array::from_fn(|i| {
            self.lanes[i]
                .queued_timbre
                .clone()
                .unwrap_or_else(|| self.lanes[i].active_timbre.clone())
        });
        (self.timbre_bank.active_set_index, timbres)
    }

    /// Commits all queued timbres across all 8 lanes (e.g., at the next bar downbeat).
    pub fn commit_all_queued_timbres(&mut self) -> usize {
        let mut count = 0;
        for lane in &mut self.lanes {
            if lane.commit_queued_timbre() {
                count += 1;
            }
        }
        count
    }

    /// Atomically captures the current 8-voice timbre state into the preset pool (`Marker Set CC 60`),
    /// computing its deterministic FNV-1a index within the 4,096-entry timbre bank.
    pub fn capture_preset_snapshot(&mut self, label: Option<&str>) -> Op8PresetSnapshot {
        let serial = self.timbre_bank.captured_pool.len() + 1;
        let id = format!("preset-{serial:04}");
        let label_str = label
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| id.clone());
        let timbres: [String; 8] = std::array::from_fn(|i| {
            self.lanes[i]
                .queued_timbre
                .clone()
                .unwrap_or_else(|| self.lanes[i].active_timbre.clone())
        });
        let signature = timbres.join("|");
        let pool_bank_index =
            fnv1a_pool_index(&format!("{id}:{signature}"), self.timbre_bank.total_entries);
        let snapshot = Op8PresetSnapshot {
            id,
            label: label_str,
            signature,
            timbres,
            pool_bank_index,
        };
        self.timbre_bank.captured_pool.push(snapshot.clone());
        snapshot
    }

    /// Processes a native Korg `nanoKONTROL2` CC message (`cc`, `value`) and updates the
    /// `8opPerc` performance model.
    ///
    /// Note that momentary/toggle button CCs only trigger actions on button press (`value > 0`),
    /// while continuous Knobs (`CC 16..23`) and Sliders (`CC 0..7`) update on every value `0..=127`.
    pub fn apply_nanokontrol2_cc(&mut self, cc: u8, value: u8) -> Option<Op8PerformanceAction> {
        let val = value.min(127);

        // 1. Track Knobs 1..8 (CC 16..=23) -> Per-lane voice density (0.1 ..= 5.0)
        if (16..=23).contains(&cc) {
            let lane = (cc - 16) as usize;
            let density = self.lanes[lane].set_density_from_cc(val);
            let effective_density = self.effective_lane_density(lane);
            return Some(Op8PerformanceAction::SetLaneDensity {
                lane,
                family: self.lanes[lane].family.clone(),
                cc_value: val,
                density,
                effective_density,
            });
        }

        // 2. Track Sliders 1..8 (CC 0..=7) -> Per-lane voice gain/macro (0..=127)
        if cc <= 7 {
            let lane = cc as usize;
            let gain_normalized = self.lanes[lane].set_gain_from_cc(val);
            return Some(Op8PerformanceAction::SetLaneGain {
                lane,
                family: self.lanes[lane].family.clone(),
                cc_value: val,
                gain_normalized,
            });
        }

        // All remaining controls are buttons; ignore button release (value == 0).
        if val == 0 {
            return None;
        }

        // 3. Track Solo Buttons 1..8 (CC 32..=39) -> Toggle voice solo
        if (32..=39).contains(&cc) {
            let lane = (cc - 32) as usize;
            self.lanes[lane].soloed = !self.lanes[lane].soloed;
            let soloed = self.lanes[lane].soloed;
            let audible_mask = self.audible_mask();
            return Some(Op8PerformanceAction::ToggleLaneSolo {
                lane,
                family: self.lanes[lane].family.clone(),
                soloed,
                audible_mask,
            });
        }

        // 4. Track Mute Buttons 1..8 (CC 48..=55) -> Toggle voice mute
        if (48..=55).contains(&cc) {
            let lane = (cc - 48) as usize;
            self.lanes[lane].muted = !self.lanes[lane].muted;
            let muted = self.lanes[lane].muted;
            let audible_mask = self.audible_mask();
            return Some(Op8PerformanceAction::ToggleLaneMute {
                lane,
                family: self.lanes[lane].family.clone(),
                muted,
                audible_mask,
            });
        }

        // 5. Track Rec Buttons 1..8 (CC 64..=71) -> Single-lane timbre reroll
        if (64..=71).contains(&cc) {
            let lane = (cc - 64) as usize;
            let (active_timbre, queued_timbre) = self.reroll_lane_timbre(lane)?;
            return Some(Op8PerformanceAction::RerollLaneTimbre {
                lane,
                family: self.lanes[lane].family.clone(),
                active_timbre,
                queued_timbre,
                reroll_count: self.lanes[lane].reroll_count,
            });
        }

        // 6. Transport & Bank Navigation Buttons
        match cc {
            NK2_CC_TRACK_LEFT => {
                let (active_index, active_form_name) =
                    self.form_bank.step(FormDirection::Previous);
                Some(Op8PerformanceAction::NavigateForm {
                    direction: FormDirection::Previous,
                    active_index,
                    active_form_name,
                })
            }
            NK2_CC_TRACK_RIGHT => {
                let (active_index, active_form_name) = self.form_bank.step(FormDirection::Next);
                Some(Op8PerformanceAction::NavigateForm {
                    direction: FormDirection::Next,
                    active_index,
                    active_form_name,
                })
            }
            NK2_CC_CYCLE => {
                let (active_set_index, timbres) = self.advance_full_timbre_set();
                Some(Op8PerformanceAction::AdvanceTimbreSet {
                    active_set_index,
                    timbres,
                })
            }
            NK2_CC_MARKER_SET => {
                let snapshot = self.capture_preset_snapshot(None);
                Some(Op8PerformanceAction::CapturePresetPoolSnapshot(snapshot))
            }
            NK2_CC_MARKER_LEFT => {
                let delta = -(i16::from(self.step_cc_delta));
                let next_cc = (i16::from(self.global_density_cc) + delta).clamp(0, 127) as u8;
                let global_density = self.set_global_density_from_cc(next_cc);
                Some(Op8PerformanceAction::StepGlobalDensity {
                    delta_cc: delta,
                    cc_value: next_cc,
                    global_density,
                })
            }
            NK2_CC_MARKER_RIGHT => {
                let delta = i16::from(self.step_cc_delta);
                let next_cc = (i16::from(self.global_density_cc) + delta).clamp(0, 127) as u8;
                let global_density = self.set_global_density_from_cc(next_cc);
                Some(Op8PerformanceAction::StepGlobalDensity {
                    delta_cc: delta,
                    cc_value: next_cc,
                    global_density,
                })
            }
            NK2_CC_REWIND => {
                let delta = -(i16::from(self.step_cc_delta));
                let next_cc = (i16::from(self.feel.feel_cc) + delta).clamp(0, 127) as u8;
                let feel_scale = self.feel.set_from_cc(next_cc);
                Some(Op8PerformanceAction::StepGlobalFeel {
                    delta_cc: delta,
                    cc_value: next_cc,
                    feel_scale,
                })
            }
            NK2_CC_FORWARD => {
                let delta = i16::from(self.step_cc_delta);
                let next_cc = (i16::from(self.feel.feel_cc) + delta).clamp(0, 127) as u8;
                let feel_scale = self.feel.set_from_cc(next_cc);
                Some(Op8PerformanceAction::StepGlobalFeel {
                    delta_cc: delta,
                    cc_value: next_cc,
                    feel_scale,
                })
            }
            NK2_CC_PLAY => {
                self.transport.playing = true;
                Some(Op8PerformanceAction::TransportPlay { playing: true })
            }
            NK2_CC_STOP => {
                self.transport.playing = false;
                Some(Op8PerformanceAction::TransportStop { playing: false })
            }
            NK2_CC_RECORD => {
                self.transport.recording_wav = !self.transport.recording_wav;
                Some(Op8PerformanceAction::TransportRecord {
                    recording_wav: self.transport.recording_wav,
                })
            }
            _ => None,
        }
    }

    // =========================================================================
    // 6. MIDImix Emulation Mode Translator
    // =========================================================================

    /// Translates a Korg `nanoKONTROL2` CC event (`cc`, `value`) into the equivalent factory
    /// Akai `MIDI Mix` Note/CC messages on MIDI Channel 1 so unmodified legacy `MIDImix` scripts
    /// (`pulsed-live-rig.js`) work seamlessly with a physical `nanoKONTROL2`.
    pub fn translate_nanokontrol2_to_midimix(&self, cc: u8, value: u8) -> Vec<EmulatedMidiMessage> {
        let val = value.min(127);

        // Track Knobs 1..8 (CC 16..=23) -> MIDImix Row 2 Voice Density CCs (17, 21, 25, 29, 47, 51, 55, 59)
        if (16..=23).contains(&cc) {
            let lane = (cc - 16) as usize;
            return vec![EmulatedMidiMessage::cc(
                1,
                MIDIMIX_VOICE_DENSITY_CCS[lane],
                val,
            )];
        }

        // Track Sliders 1..8 (CC 0..=7) -> MIDImix Channel Strip Fader CCs (19, 23, 27, 31, 49, 53, 57, 61)
        if cc <= 7 {
            let lane = cc as usize;
            return vec![EmulatedMidiMessage::cc(1, MIDIMIX_FADER_CCS[lane], val)];
        }

        // Track Mute Buttons 1..8 (CC 48..=55) -> MIDImix Mute Notes (1, 4, 7, 10, 13, 16, 19, 22)
        if (48..=55).contains(&cc) {
            let lane = (cc - 48) as usize;
            return vec![EmulatedMidiMessage::note(1, MIDIMIX_MUTE_NOTES[lane], val)];
        }

        // Track Solo Buttons 1..8 (CC 32..=39) -> MIDImix Solo Notes (2, 5, 8, 11, 14, 17, 20, 23)
        if (32..=39).contains(&cc) {
            let lane = (cc - 32) as usize;
            return vec![EmulatedMidiMessage::note(1, MIDIMIX_SOLO_NOTES[lane], val)];
        }

        // Track Rec Buttons 1..8 (CC 64..=71) -> MIDImix Rec Arm Notes (3, 6, 9, 12, 15, 18, 21, 24)
        if (64..=71).contains(&cc) {
            let lane = (cc - 64) as usize;
            return vec![EmulatedMidiMessage::note(
                1,
                MIDIMIX_REC_ARM_NOTES[lane],
                val,
            )];
        }

        match cc {
            // Track Left (CC 58) -> MIDImix Bank Left (Note 25)
            NK2_CC_TRACK_LEFT => vec![EmulatedMidiMessage::note(1, MIDIMIX_BANK_LEFT_NOTE, val)],
            // Track Right (CC 59) -> MIDImix Bank Right (Note 26)
            NK2_CC_TRACK_RIGHT => vec![EmulatedMidiMessage::note(1, MIDIMIX_BANK_RIGHT_NOTE, val)],
            // Cycle (CC 46) -> MIDImix Solo Master / Timbre Set (Note 27)
            NK2_CC_CYCLE => vec![EmulatedMidiMessage::note(1, MIDIMIX_TIMBRE_SET_NOTE, val)],
            // Marker Set (CC 60, on press) -> Emits the 32-CC MIDImix Send All burst (CC 16..31, 46..61)
            NK2_CC_MARKER_SET if val > 0 => MIDIMIX_SEND_ALL_CCS
                .iter()
                .map(|&ctrl| {
                    let ctrl_val = if let Some(lane) =
                        MIDIMIX_VOICE_DENSITY_CCS.iter().position(|&c| c == ctrl)
                    {
                        self.lanes[lane].density_cc
                    } else if ctrl == MIDIMIX_GLOBAL_DENSITY_CC {
                        self.global_density_cc
                    } else if ctrl == MIDIMIX_FEEL_CC {
                        self.feel.feel_cc
                    } else {
                        64
                    };
                    EmulatedMidiMessage::cc(1, ctrl, ctrl_val)
                })
                .collect(),
            // Marker Left / Right (CC 61 / 62) -> MIDImix Global Density (CC 61)
            NK2_CC_MARKER_LEFT | NK2_CC_MARKER_RIGHT if val > 0 => vec![EmulatedMidiMessage::cc(
                1,
                MIDIMIX_GLOBAL_DENSITY_CC,
                self.global_density_cc,
            )],
            // Rewind / Forward (CC 43 / 44) -> MIDImix Global Feel (CC 58)
            NK2_CC_REWIND | NK2_CC_FORWARD if val > 0 => {
                vec![EmulatedMidiMessage::cc(1, MIDIMIX_FEEL_CC, self.feel.feel_cc)]
            }
            _ => Vec::new(),
        }
    }

    // =========================================================================
    // 7. Conversion To/From OSC Op8RigState
    // =========================================================================

    /// Exports this domain model to an [`Op8RigState`] for OSC broadcast.
    pub fn to_osc_rig_state(&self) -> Op8RigState {
        let lanes = std::array::from_fn(|i| {
            let l = &self.lanes[i];
            Op8VoiceLaneState {
                index: i,
                family: l.family.clone(),
                timbre_name: l
                    .queued_timbre
                    .clone()
                    .unwrap_or_else(|| l.active_timbre.clone()),
                density: l.density_multiplier as f32,
                level: l.gain_cc,
                muted: l.muted,
                soloed: l.soloed,
                reroll_count: l.reroll_count,
            }
        });
        Op8RigState {
            active_form_index: self.form_bank.active_index,
            active_form_name: self.form_bank.active_form_name.clone(),
            total_forms: self.form_bank.total_entries,
            active_timbre_set_index: self.timbre_bank.active_set_index,
            total_timbre_sets: self.timbre_bank.total_entries,
            bpm: self.transport.bpm as f32,
            global_density: self.global_density_multiplier as f32,
            feel_scale: self.feel.scale as f32,
            playing: self.transport.playing,
            recording: self.transport.recording_wav,
            beat_in_bar: (self.transport.current_beat as u8)
                % self.transport.beats_per_bar.max(1),
            saved_snapshots: self
                .timbre_bank
                .captured_pool
                .iter()
                .map(|s| s.label.clone())
                .collect(),
            lanes,
        }
    }

    /// Synchronizes this domain model from an incoming [`Op8RigState`].
    pub fn sync_from_osc_rig_state(&mut self, state: &Op8RigState) {
        self.form_bank.total_entries = state.total_forms.max(1);
        self.form_bank.active_index = state.active_form_index % self.form_bank.total_entries;
        self.form_bank.active_form_name = state.active_form_name.clone();
        self.timbre_bank.total_entries = state.total_timbre_sets.max(1);
        self.timbre_bank.active_set_index =
            state.active_timbre_set_index % self.timbre_bank.total_entries;
        self.transport.bpm = f64::from(state.bpm);
        self.set_global_density_multiplier(f64::from(state.global_density));
        self.feel.set_scale(f64::from(state.feel_scale));
        self.transport.playing = state.playing;
        self.transport.recording_wav = state.recording;
        for (i, src) in state.lanes.iter().enumerate() {
            let dst = &mut self.lanes[i];
            dst.set_density_multiplier(f64::from(src.density));
            dst.set_gain_from_cc(src.level);
            dst.muted = src.muted;
            dst.soloed = src.soloed;
            dst.active_timbre = src.timbre_name.clone();
            dst.queued_timbre = None;
            dst.reroll_count = src.reroll_count;
        }
    }
}

/// Emulated MIDI message emitted by [`Op8PerformanceModel::translate_nanokontrol2_to_midimix`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmulatedMidiMessage {
    /// 1-indexed MIDI channel (`1..=16`).
    pub channel: u8,
    /// Message type (`Cc` or `Note`).
    pub message_type: MidiMessageType,
    /// Controller or Note number (`0..=127`).
    pub number: u8,
    /// Value or velocity (`0..=127`).
    pub value: u8,
    /// Raw 3-byte MIDI packet (`[status, data1, data2]`).
    pub raw_bytes: [u8; 3],
}

impl EmulatedMidiMessage {
    /// Constructs a Control Change (`0xB0`) message.
    pub fn cc(channel: u8, controller: u8, value: u8) -> Self {
        let ch = channel.clamp(1, 16);
        let c = controller.min(127);
        let v = value.min(127);
        Self {
            channel: ch,
            message_type: MidiMessageType::Cc,
            number: c,
            value: v,
            raw_bytes: [0xB0 | (ch - 1), c, v],
        }
    }

    /// Constructs a Note On (`0x90` when `velocity > 0`) or Note Off (`0x80` when `velocity == 0`) message.
    pub fn note(channel: u8, note: u8, velocity: u8) -> Self {
        let ch = channel.clamp(1, 16);
        let n = note.min(127);
        let v = velocity.min(127);
        let status = if v > 0 {
            0x90 | (ch - 1)
        } else {
            0x80 | (ch - 1)
        };
        Self {
            channel: ch,
            message_type: MidiMessageType::Note,
            number: n,
            value: v,
            raw_bytes: [status, n, v],
        }
    }
}

// =============================================================================
// 8. Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_density_ladder_endpoints_and_roundtrip() {
        let min_d = cc_to_density(0);
        let max_d = cc_to_density(127);
        assert!((min_d - 0.1).abs() < 1e-12);
        assert!((max_d - 5.0).abs() < 1e-12);

        for cc in [0u8, 1, 32, 64, 72, 96, 126, 127] {
            let d = cc_to_density(cc);
            assert!(d >= 0.1 && d <= 5.0);
            assert_eq!(density_to_cc(d), cc);
        }
    }

    #[test]
    fn test_feel_scale_exact_midpoint_at_cc_64() {
        // Both positive-slope and MIDImix inverted feel curves must hit exact 0.5 at CC 64
        assert_eq!(cc_to_feel_scale(0), 0.0);
        assert_eq!(cc_to_feel_scale(64), 0.5);
        assert_eq!(cc_to_feel_scale(127), 1.0);

        assert_eq!(cc_to_midimix_feel_scale(0), 1.0);
        assert_eq!(cc_to_midimix_feel_scale(64), 0.5);
        assert_eq!(cc_to_midimix_feel_scale(127), 0.0);

        assert_eq!(feel_scale_to_cc(0.0), 0);
        assert_eq!(feel_scale_to_cc(0.5), 64);
        assert_eq!(feel_scale_to_cc(1.0), 127);
    }

    #[test]
    fn test_voice_lanes_mute_and_solo_priority() {
        let mut model = Op8PerformanceModel::new();
        assert_eq!(model.audible_mask(), [true; 8]);

        // Mute lane 0 (kick) via CC 48
        let act = model.apply_nanokontrol2_cc(48, 127).unwrap();
        assert!(matches!(
            act,
            Op8PerformanceAction::ToggleLaneMute {
                lane: 0,
                muted: true,
                ..
            }
        ));
        assert!(!model.is_lane_audible(0));
        assert!(model.is_lane_audible(1));

        // Solo lane 1 (snare) via CC 33 -> only lane 1 should be audible
        let act = model.apply_nanokontrol2_cc(33, 127).unwrap();
        assert!(matches!(
            act,
            Op8PerformanceAction::ToggleLaneSolo {
                lane: 1,
                soloed: true,
                ..
            }
        ));
        assert_eq!(
            model.audible_mask(),
            [false, true, false, false, false, false, false, false]
        );

        // Solo lane 2 (hat_closed) via CC 34 -> lanes 1 and 2 are audible
        model.apply_nanokontrol2_cc(34, 127);
        assert_eq!(
            model.audible_mask(),
            [false, true, true, false, false, false, false, false]
        );
    }

    #[test]
    fn test_native_nanokontrol2_performance_mapping_all_controls() {
        let mut model = Op8PerformanceModel::new();

        // 1. Knobs CC 16..23 -> voice density
        let act = model.apply_nanokontrol2_cc(16, 127).unwrap();
        if let Op8PerformanceAction::SetLaneDensity { lane, density, .. } = act {
            assert_eq!(lane, 0);
            assert!((density - 5.0).abs() < 1e-9);
        } else {
            panic!("expected SetLaneDensity");
        }

        // 2. Sliders CC 0..7 -> voice gain/macro
        let act = model.apply_nanokontrol2_cc(3, 64).unwrap();
        if let Op8PerformanceAction::SetLaneGain {
            lane, cc_value, ..
        } = act
        {
            assert_eq!(lane, 3);
            assert_eq!(cc_value, 64);
        } else {
            panic!("expected SetLaneGain");
        }

        // 3. Rec CC 64..71 -> single-lane timbre reroll (queued for next bar)
        let act = model.apply_nanokontrol2_cc(64, 127).unwrap();
        if let Op8PerformanceAction::RerollLaneTimbre {
            lane,
            queued_timbre,
            reroll_count,
            ..
        } = act
        {
            assert_eq!(lane, 0);
            assert_eq!(queued_timbre.as_deref(), Some("kick-var-0001"));
            assert_eq!(reroll_count, 1);
        } else {
            panic!("expected RerollLaneTimbre");
        }
        assert_eq!(model.commit_all_queued_timbres(), 1);
        assert_eq!(model.lanes[0].active_timbre, "kick-var-0001");

        // 4. Track Left / Right CC 58 / 59 -> 4,096-entry form bank navigation
        let act = model.apply_nanokontrol2_cc(58, 127).unwrap();
        assert_eq!(
            act,
            Op8PerformanceAction::NavigateForm {
                direction: FormDirection::Previous,
                active_index: 4095,
                active_form_name: "form-4096".to_string(),
            }
        );
        let act = model.apply_nanokontrol2_cc(59, 127).unwrap();
        assert_eq!(
            act,
            Op8PerformanceAction::NavigateForm {
                direction: FormDirection::Next,
                active_index: 0,
                active_form_name: "form-0001".to_string(),
            }
        );

        // 5. Cycle CC 46 -> full 8-voice timbre set advance
        let act = model.apply_nanokontrol2_cc(46, 127).unwrap();
        if let Op8PerformanceAction::AdvanceTimbreSet {
            active_set_index,
            timbres,
        } = act
        {
            assert_eq!(active_set_index, 1);
            assert_eq!(timbres[0], "kick-set-0002");
            assert_eq!(timbres[7], "cymbal-set-0002");
        } else {
            panic!("expected AdvanceTimbreSet");
        }

        // 6. Marker Set CC 60 -> atomic preset pool snapshot capture
        let act = model.apply_nanokontrol2_cc(60, 127).unwrap();
        if let Op8PerformanceAction::CapturePresetPoolSnapshot(snap) = act {
            assert_eq!(snap.id, "preset-0001");
            assert!(snap.pool_bank_index < 4096);
            assert_eq!(model.timbre_bank.captured_pool.len(), 1);
        } else {
            panic!("expected CapturePresetPoolSnapshot");
        }

        // 7. Marker Left / Right CC 61 / 62 -> global density step
        let prev_cc = model.global_density_cc;
        model.apply_nanokontrol2_cc(62, 127).unwrap();
        assert_eq!(model.global_density_cc, prev_cc + 8);

        // 8. Rewind / Forward CC 43 / 44 -> global feel step around CC 64
        assert_eq!(model.feel.feel_cc, 64);
        model.apply_nanokontrol2_cc(43, 127).unwrap();
        assert_eq!(model.feel.feel_cc, 56);
        model.apply_nanokontrol2_cc(44, 127).unwrap();
        assert_eq!(model.feel.feel_cc, 64);
        assert_eq!(model.feel.scale, 0.5);

        // 9. Play / Stop / Rec CC 41 / 42 / 45 -> transport
        model.apply_nanokontrol2_cc(41, 127).unwrap();
        assert!(model.transport.playing);
        model.apply_nanokontrol2_cc(45, 127).unwrap();
        assert!(model.transport.recording_wav);
        model.apply_nanokontrol2_cc(42, 127).unwrap();
        assert!(!model.transport.playing);
    }

    #[test]
    fn test_midimix_emulation_translation() {
        let model = Op8PerformanceModel::new();

        // Knob 1 (CC 16) -> MIDImix CC 17
        let msgs = model.translate_nanokontrol2_to_midimix(16, 100);
        assert_eq!(msgs, vec![EmulatedMidiMessage::cc(1, 17, 100)]);

        // Mute 1 (CC 48) -> MIDImix Note 1
        let msgs = model.translate_nanokontrol2_to_midimix(48, 127);
        assert_eq!(msgs, vec![EmulatedMidiMessage::note(1, 1, 127)]);

        // Rec 8 (CC 71) -> MIDImix Note 24
        let msgs = model.translate_nanokontrol2_to_midimix(71, 127);
        assert_eq!(msgs, vec![EmulatedMidiMessage::note(1, 24, 127)]);

        // Track Left / Right (CC 58 / 59) -> MIDImix Bank Left / Right (Note 25 / 26)
        assert_eq!(
            model.translate_nanokontrol2_to_midimix(58, 127),
            vec![EmulatedMidiMessage::note(1, 25, 127)]
        );
        assert_eq!(
            model.translate_nanokontrol2_to_midimix(59, 127),
            vec![EmulatedMidiMessage::note(1, 26, 127)]
        );

        // Cycle (CC 46) -> MIDImix Solo Master / Timbre Set (Note 27)
        assert_eq!(
            model.translate_nanokontrol2_to_midimix(46, 127),
            vec![EmulatedMidiMessage::note(1, 27, 127)]
        );

        // Marker Set (CC 60) -> Full 32-CC MIDImix Send All burst
        let burst = model.translate_nanokontrol2_to_midimix(60, 127);
        assert_eq!(burst.len(), 32);
        assert_eq!(burst[0].number, 16);
        assert_eq!(burst[15].number, 31);
        assert_eq!(burst[16].number, 46);
        assert_eq!(burst[31].number, 61);
    }
}
