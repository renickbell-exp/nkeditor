//! 8opPerc Timbre Snapshot & Scene Morphing Manager (`src/snapshot_pool.rs`).
//!
//! Manages durable 8-voice percussion snapshots and continuous scene morphing:
//! 1. **JSONL Preset Pools**: Reads and writes both `nanokontrol2-random-preset-pool.jsonl`
//!    and legacy `midimix-random-preset-pool.jsonl` (supporting both 8-element array and
//!    family-keyed object timbre representations, including legacy `KONDUKTIVA_FAMILY_ORDER` keys).
//! 2. **Named Timbre-State JSON Files**: Loads and saves full 8-voice scene JSON documents
//!    containing timbres, per-lane densities, per-lane macro/gains, mutes/solos, global density,
//!    rhythmic feel scale, ratings (`0..=5`), and favorites.
//! 3. **Rehearsal Browsing, Rating/Favorites & Selective Voice-Lane Locking**:
//!    Allows locking specific voice lanes (e.g., keeping `kick` and `snare` while loading
//!    `hat_closed`, `clap`, `tom`, etc. from another snapshot).
//! 4. **Beat-Synchronized Scene Morphing Engine (`SceneMorphEngine`)**:
//!    Smoothly interpolates continuous per-lane density multipliers (`0.1..=5.0`), macro/gain
//!    sliders (`0..=127`), global density (`0.1..=5.0`), and rhythmic feel (`0.0..=1.0`)
//!    between Scene A and Scene B over a configurable number of beats (`duration_beats`).

use crate::op8_model::{
    density_to_cc, feel_scale_to_cc, fnv1a_pool_index, resolve_lane_index, Op8PerformanceModel,
    DEFAULT_BOOT_PATCHES, FAMILY_ORDER, MAX_DENSITY_SCALE, MIN_DENSITY_SCALE, TIMBRE_BANK_SIZE,
};
use crate::theme::ThemeSettings;
use egui::{Color32, RichText, ScrollArea, Stroke, Ui};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Default filename for the Korg `nanoKONTROL2` JSONL random preset pool.
pub const DEFAULT_NK2_POOL_FILENAME: &str = "nanokontrol2-random-preset-pool.jsonl";

/// Legacy filename for the Akai `MIDI Mix` JSONL random preset pool.
pub const LEGACY_MIDIMIX_POOL_FILENAME: &str = "midimix-random-preset-pool.jsonl";

/// Computes the canonical pipe-delimited signature for an 8-voice timbre array.
#[must_use]
pub fn timbre_set_signature(timbres: &[String; 8]) -> String {
    timbres.join("|")
}

// =============================================================================
// 1. Durable 8-Voice Snapshot & Scene Record
// =============================================================================

/// Complete 8-voice timbre snapshot and continuous performance scene record.
///
/// Compatible with both `nanokontrol2-random-preset-pool.jsonl` /
/// `midimix-random-preset-pool.jsonl` lines and standalone named `.json` scene files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotSceneRecord {
    /// Unique snapshot identifier (e.g. `"snap-0001"` or `"scene-gqom-heavy"`).
    pub id: String,
    /// Human-readable label or scene name.
    pub label: String,
    /// Originating hardware controller (`"korg-nanokontrol2"` or `"akai-midimix"`).
    pub controller: String,
    /// ISO-8601 timestamp when captured or saved.
    pub saved_at: String,
    /// Pipe-delimited 8-voice timbre signature.
    pub signature: String,
    /// Deterministic FNV-1a index (`0..4096`) into the timbre reservoir.
    pub pool_bank_index: usize,
    /// Rehearsal star rating (`0..=5`, where `0` is unrated).
    #[serde(default)]
    pub rating: u8,
    /// Whether this snapshot is starred as a favorite during rehearsal.
    #[serde(default)]
    pub favorite: bool,
    /// Searchable tags (e.g. `["rehearsal", "heavy", "metallic"]`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Active FM timbre preset name for each of the 8 voice lanes (`FAMILY_ORDER`).
    pub timbres: [String; 8],
    /// Per-lane density multiplier (`0.1 ..= 5.0`) for each of the 8 voice lanes.
    pub densities: [f64; 8],
    /// Per-lane macro/gain CC (`0..=127`) for each of the 8 voice lanes.
    pub gains: [u8; 8],
    /// Per-lane mute state (`true` = muted).
    #[serde(default)]
    pub mutes: [bool; 8],
    /// Per-lane solo state (`true` = soloed).
    #[serde(default)]
    pub solos: [bool; 8],
    /// Global density multiplier (`0.1 ..= 5.0`, default `1.0`).
    pub global_density: f64,
    /// Global rhythmic feel scale (`0.0 ..= 1.0`, default `0.5` at `CC 64`).
    pub feel_scale: f64,
    /// Associated musical form name (e.g. `"form-0001"`).
    #[serde(default)]
    pub active_form_name: String,
}

impl SnapshotSceneRecord {
    /// Captures a new [`SnapshotSceneRecord`] from the current live [`Op8PerformanceModel`].
    #[must_use]
    pub fn from_op8_model(
        id: impl Into<String>,
        label: impl Into<String>,
        controller: impl Into<String>,
        model: &Op8PerformanceModel,
    ) -> Self {
        let timbres = std::array::from_fn(|i| model.lanes[i].active_timbre.clone());
        let densities = std::array::from_fn(|i| model.lanes[i].density_multiplier);
        let gains = std::array::from_fn(|i| model.lanes[i].gain_cc);
        let mutes = std::array::from_fn(|i| model.lanes[i].muted);
        let solos = std::array::from_fn(|i| model.lanes[i].soloed);
        let signature = timbre_set_signature(&timbres);
        let pool_bank_index = fnv1a_pool_index(&signature, TIMBRE_BANK_SIZE);

        Self {
            id: id.into(),
            label: label.into(),
            controller: controller.into(),
            saved_at: "2026-11-04T00:00:00Z".to_string(),
            signature,
            pool_bank_index,
            rating: 0,
            favorite: false,
            tags: Vec::new(),
            timbres,
            densities,
            gains,
            mutes,
            solos,
            global_density: model.global_density_multiplier,
            feel_scale: model.feel.scale,
            active_form_name: model.form_bank.active_form_name.clone(),
        }
    }

    /// Parses a single JSON object (from either `nanokontrol2-random-preset-pool.jsonl`,
    /// legacy `midimix-random-preset-pool.jsonl`, or a named timbre-state `.json` file)
    /// into a normalized [`SnapshotSceneRecord`].
    pub fn from_json_value(val: &Value, fallback_index: usize) -> Result<Self, String> {
        let obj = val
            .as_object()
            .ok_or_else(|| "Snapshot JSON must be an object".to_string())?;

        let id = obj
            .get("id")
            .or_else(|| obj.get("setId"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("snap-{:04}", fallback_index + 1));

        let label = obj
            .get("label")
            .or_else(|| obj.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| id.clone());

        let controller = obj
            .get("controller")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "korg-nanokontrol2".to_string());

        let saved_at = obj
            .get("saved_at")
            .or_else(|| obj.get("savedAt"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "2026-11-04T00:00:00Z".to_string());

        let timbres = parse_timbres_field(obj.get("timbres"))?;
        let signature = obj
            .get("signature")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| timbre_set_signature(&timbres));

        let pool_bank_index = obj
            .get("pool_bank_index")
            .or_else(|| obj.get("poolBankIndex"))
            .and_then(Value::as_u64)
            .map(|n| (n as usize) % TIMBRE_BANK_SIZE)
            .unwrap_or_else(|| fnv1a_pool_index(&signature, TIMBRE_BANK_SIZE));

        let rating = obj
            .get("rating")
            .and_then(Value::as_u64)
            .map(|r| (r as u8).min(5))
            .unwrap_or(0);

        let favorite = obj
            .get("favorite")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let tags = obj
            .get("tags")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let densities = parse_f64_lane_array(obj.get("densities"), 1.0, MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);
        let gains = parse_u8_lane_array(obj.get("gains"), 100);
        let mutes = parse_bool_lane_array(obj.get("mutes"), false);
        let solos = parse_bool_lane_array(obj.get("solos"), false);

        let global_density = obj
            .get("global_density")
            .or_else(|| obj.get("globalDensity"))
            .and_then(Value::as_f64)
            .unwrap_or(1.0)
            .clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);

        let feel_scale = obj
            .get("feel_scale")
            .or_else(|| obj.get("feelScale"))
            .and_then(Value::as_f64)
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        let active_form_name = obj
            .get("active_form_name")
            .or_else(|| obj.get("activeFormName"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "form-0001".to_string());

        Ok(Self {
            id,
            label,
            controller,
            saved_at,
            signature,
            pool_bank_index,
            rating,
            favorite,
            tags,
            timbres,
            densities,
            gains,
            mutes,
            solos,
            global_density,
            feel_scale,
            active_form_name,
        })
    }

    /// Serializes this snapshot as a compact single-line JSONL record compatible with both
    /// `nanokontrol2-random-preset-pool.jsonl` and `midimix-random-preset-pool.jsonl`.
    #[must_use]
    pub fn to_jsonl_line(&self) -> String {
        let mut timbres_map = serde_json::Map::new();
        for (i, fam) in FAMILY_ORDER.iter().enumerate() {
            timbres_map.insert((*fam).to_string(), Value::String(self.timbres[i].clone()));
        }
        let record = serde_json::json!({
            "id": self.id,
            "label": self.label,
            "controller": self.controller,
            "savedAt": self.saved_at,
            "signature": self.signature,
            "poolBankIndex": self.pool_bank_index,
            "rating": self.rating,
            "favorite": self.favorite,
            "tags": self.tags,
            "timbres": timbres_map,
            "densities": self.densities,
            "gains": self.gains,
            "mutes": self.mutes,
            "solos": self.solos,
            "globalDensity": self.global_density,
            "feelScale": self.feel_scale,
            "activeFormName": self.active_form_name,
        });
        serde_json::to_string(&record).unwrap_or_default()
    }

    /// Serializes this snapshot as a pretty-printed named timbre-state `.json` file.
    pub fn to_pretty_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

fn parse_timbres_field(field: Option<&Value>) -> Result<[String; 8], String> {
    let mut out: [String; 8] = std::array::from_fn(|i| DEFAULT_BOOT_PATCHES[i].to_string());
    let Some(val) = field else {
        return Ok(out);
    };

    if let Some(arr) = val.as_array() {
        for (i, item) in arr.iter().take(8).enumerate() {
            if let Some(s) = item.as_str() {
                out[i] = s.to_string();
            }
        }
        return Ok(out);
    }

    if let Some(map) = val.as_object() {
        for (k, v) in map {
            if let (Some(idx), Some(s)) = (resolve_lane_index(k), v.as_str()) {
                out[idx] = s.to_string();
            }
        }
        return Ok(out);
    }

    Err("Invalid 'timbres' field in snapshot JSON".to_string())
}

fn parse_f64_lane_array(field: Option<&Value>, default_val: f64, min: f64, max: f64) -> [f64; 8] {
    let mut out = [default_val; 8];
    if let Some(arr) = field.and_then(Value::as_array) {
        for (i, item) in arr.iter().take(8).enumerate() {
            if let Some(n) = item.as_f64() {
                out[i] = n.clamp(min, max);
            }
        }
    } else if let Some(map) = field.and_then(Value::as_object) {
        for (k, v) in map {
            if let (Some(idx), Some(n)) = (resolve_lane_index(k), v.as_f64()) {
                out[idx] = n.clamp(min, max);
            }
        }
    }
    out
}

fn parse_u8_lane_array(field: Option<&Value>, default_val: u8) -> [u8; 8] {
    let mut out = [default_val; 8];
    if let Some(arr) = field.and_then(Value::as_array) {
        for (i, item) in arr.iter().take(8).enumerate() {
            if let Some(n) = item.as_u64() {
                out[i] = (n as u8).min(127);
            }
        }
    }
    out
}

fn parse_bool_lane_array(field: Option<&Value>, default_val: bool) -> [bool; 8] {
    let mut out = [default_val; 8];
    if let Some(arr) = field.and_then(Value::as_array) {
        for (i, item) in arr.iter().take(8).enumerate() {
            if let Some(b) = item.as_bool() {
                out[i] = b;
            }
        }
    }
    out
}

// =============================================================================
// 2. Beat-Synchronized Scene Morphing Engine (`SceneMorphEngine`)
// =============================================================================

/// Interpolated continuous and discrete state produced at morph position `t in [0.0, 1.0]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MorphedSceneFrame {
    /// Normalized morph progress (`0.0` = 100% Scene A, `1.0` = 100% Scene B).
    pub progress: f64,
    /// Interpolated per-lane density multipliers (`0.1 ..= 5.0`).
    pub densities: [f64; 8],
    /// Interpolated per-lane macro/gain CC values (`0..=127`).
    pub gains: [u8; 8],
    /// Interpolated global density multiplier (`0.1 ..= 5.0`).
    pub global_density: f64,
    /// Interpolated global rhythmic feel scale (`0.0 ..= 1.0`).
    pub feel_scale: f64,
    /// Effective timbre per lane (respecting `timbre_switch_point` and `lane_locks`).
    pub timbres: [String; 8],
}

/// Smoothly morphs continuous density, macro/gain, global density, and rhythmic feel
/// between two scenes (`scene_a_id` and `scene_b_id`) over a configurable number of beats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneMorphEngine {
    /// Source Scene A snapshot ID.
    pub scene_a_id: String,
    /// Target Scene B snapshot ID.
    pub scene_b_id: String,
    /// Morph duration in musical beats (e.g. `4.0`, `8.0`, `16.0`, `32.0`, minimum `0.25`).
    pub duration_beats: f64,
    /// Normalized morph progress (`0.0 ..= 1.0`).
    pub progress: f64,
    /// Point in `[0.0, 1.0]` at which unlocked discrete timbres switch from Scene A to Scene B (`0.5` default).
    pub timbre_switch_point: f64,
    /// Whether beat-driven automatic morphing is currently running.
    pub active: bool,
    /// Beat timestamp when the current morph was started (`None` if manual).
    pub start_beat: Option<f64>,
}

impl Default for SceneMorphEngine {
    fn default() -> Self {
        Self {
            scene_a_id: "scene-boot-default".to_string(),
            scene_b_id: "scene-gqom-overdrive".to_string(),
            duration_beats: 8.0,
            progress: 0.0,
            timbre_switch_point: 0.5,
            active: false,
            start_beat: None,
        }
    }
}

impl SceneMorphEngine {
    /// Starts an automated scene morph from `progress = 0.0` to `1.0` over `duration_beats`.
    pub fn start_morph(&mut self, now_beat: f64, duration_beats: f64) {
        self.duration_beats = duration_beats.max(0.25);
        self.progress = 0.0;
        self.start_beat = Some(now_beat);
        self.active = true;
    }

    /// Advances the morph by `delta_beats` and returns the new `progress` (`0.0 ..= 1.0`).
    pub fn advance_beats(&mut self, delta_beats: f64) -> f64 {
        if !self.active {
            return self.progress;
        }
        let step = delta_beats.max(0.0) / self.duration_beats.max(0.25);
        self.progress = (self.progress + step).clamp(0.0, 1.0);
        if self.progress >= 1.0 {
            self.active = false;
        }
        self.progress
    }

    /// Updates morph progress at absolute beat `current_beat` given `start_beat`.
    pub fn update_at_beat(&mut self, current_beat: f64) -> f64 {
        if !self.active {
            return self.progress;
        }
        let start = *self.start_beat.get_or_insert(current_beat);
        let elapsed = (current_beat - start).max(0.0);
        self.progress = (elapsed / self.duration_beats.max(0.25)).clamp(0.0, 1.0);
        if self.progress >= 1.0 {
            self.active = false;
        }
        self.progress
    }

    /// Evaluates the interpolated [`MorphedSceneFrame`] between `scene_a` and `scene_b`
    /// at the current `self.progress`, respecting `lane_locks` (`true` keeps Scene A / locked state).
    #[must_use]
    pub fn evaluate_frame(
        &self,
        scene_a: &SnapshotSceneRecord,
        scene_b: &SnapshotSceneRecord,
        lane_locks: &[bool; 8],
    ) -> MorphedSceneFrame {
        let t = self.progress.clamp(0.0, 1.0);
        let mut densities = [1.0; 8];
        let mut gains = [100u8; 8];
        let mut timbres: [String; 8] = std::array::from_fn(|i| scene_a.timbres[i].clone());

        for i in 0..8 {
            if lane_locks[i] {
                densities[i] = scene_a.densities[i];
                gains[i] = scene_a.gains[i];
                timbres[i] = scene_a.timbres[i].clone();
            } else {
                let da = scene_a.densities[i];
                let db = scene_b.densities[i];
                densities[i] = (da + (db - da) * t).clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);

                let ga = f64::from(scene_a.gains[i]);
                let gb = f64::from(scene_b.gains[i]);
                gains[i] = (ga + (gb - ga) * t).round().clamp(0.0, 127.0) as u8;

                timbres[i] = if t >= self.timbre_switch_point {
                    scene_b.timbres[i].clone()
                } else {
                    scene_a.timbres[i].clone()
                };
            }
        }

        let global_density = (scene_a.global_density
            + (scene_b.global_density - scene_a.global_density) * t)
            .clamp(MIN_DENSITY_SCALE, MAX_DENSITY_SCALE);
        let feel_scale =
            (scene_a.feel_scale + (scene_b.feel_scale - scene_a.feel_scale) * t).clamp(0.0, 1.0);

        MorphedSceneFrame {
            progress: t,
            densities,
            gains,
            global_density,
            feel_scale,
            timbres,
        }
    }
}

// =============================================================================
// 3. Snapshot Pool Manager (`SnapshotPoolManager`)
// =============================================================================

/// Complete manager for 8opPerc timbre snapshots, JSONL preset pools, named JSON scenes,
/// per-lane lock masks, and beat-synchronized scene morphing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPoolManager {
    /// All loaded or captured 8-voice snapshots.
    pub snapshots: Vec<SnapshotSceneRecord>,
    /// Currently selected snapshot ID in the browser.
    pub active_snapshot_id: String,
    /// Per-lane lock mask (`true` = locked; prevents snapshot recall or scene morph from overwriting that lane).
    pub lane_locks: [bool; 8],
    /// Whether recalling a snapshot queues timbres for the next bar boundary (`true`) or switches immediately (`false`).
    pub queue_timbres_on_recall: bool,
    /// Search filter text.
    pub search_query: String,
    /// Filter to show only favorite snapshots.
    pub favorites_only: bool,
    /// Minimum star rating filter (`0..=5`).
    pub min_rating: u8,
    /// Active beat-synchronized scene morph engine.
    pub morph_engine: SceneMorphEngine,
}

impl Default for SnapshotPoolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotPoolManager {
    /// Creates a new [`SnapshotPoolManager`] seeded with 4 curated factory performance scenes
    /// so performers can immediately audition and morph scenes out of the box.
    #[must_use]
    pub fn new() -> Self {
        let s1 = SnapshotSceneRecord {
            id: "scene-boot-default".to_string(),
            label: "Boot Default Kit".to_string(),
            controller: "korg-nanokontrol2".to_string(),
            saved_at: "2026-11-04T00:00:00Z".to_string(),
            signature: timbre_set_signature(&std::array::from_fn(|i| {
                DEFAULT_BOOT_PATCHES[i].to_string()
            })),
            pool_bank_index: 0,
            rating: 4,
            favorite: true,
            tags: vec!["factory".into(), "boot".into(), "balanced".into()],
            timbres: std::array::from_fn(|i| DEFAULT_BOOT_PATCHES[i].to_string()),
            densities: [1.0; 8],
            gains: [100; 8],
            mutes: [false; 8],
            solos: [false; 8],
            global_density: 1.0,
            feel_scale: 0.5,
            active_form_name: "form-0001".to_string(),
        };

        let gqom_timbres: [String; 8] = [
            "kick-sub-anvil".into(),
            "snare-rim-crack".into(),
            "hat-closed-tick".into(),
            "hat-open-sizzle".into(),
            "clap-broken-glass".into(),
            "tom-low-log".into(),
            "rim-wood-clave".into(),
            "shaker-dust-grain".into(),
        ];
        let s2 = SnapshotSceneRecord {
            id: "scene-gqom-overdrive".to_string(),
            label: "Gqom Overdrive Scene".to_string(),
            controller: "korg-nanokontrol2".to_string(),
            saved_at: "2026-11-04T00:00:00Z".to_string(),
            signature: timbre_set_signature(&gqom_timbres),
            pool_bank_index: fnv1a_pool_index(&timbre_set_signature(&gqom_timbres), TIMBRE_BANK_SIZE),
            rating: 5,
            favorite: true,
            tags: vec!["factory".into(), "gqom".into(), "dense".into()],
            timbres: gqom_timbres,
            densities: [2.4, 1.8, 3.2, 1.5, 2.0, 2.8, 1.6, 3.5],
            gains: [120, 112, 96, 92, 110, 116, 98, 90],
            mutes: [false; 8],
            solos: [false; 8],
            global_density: 2.2,
            feel_scale: 0.75,
            active_form_name: "form-0002".to_string(),
        };

        let minimal_timbres: [String; 8] = [
            "kick-warm-pulse".into(),
            "snare-brush-ghost".into(),
            "hat-closed-pin".into(),
            "hat-open-air".into(),
            "clap-soft-palm".into(),
            "tom-glass-drop".into(),
            "rim-click-micro".into(),
            "shaker-silk".into(),
        ];
        let s3 = SnapshotSceneRecord {
            id: "scene-minimal-micro".to_string(),
            label: "Minimal Micro-Pulse".to_string(),
            controller: "akai-midimix".to_string(),
            saved_at: "2026-11-04T00:00:00Z".to_string(),
            signature: timbre_set_signature(&minimal_timbres),
            pool_bank_index: fnv1a_pool_index(
                &timbre_set_signature(&minimal_timbres),
                TIMBRE_BANK_SIZE,
            ),
            rating: 3,
            favorite: false,
            tags: vec!["factory".into(), "minimal".into(), "sparse".into()],
            timbres: minimal_timbres,
            densities: [0.5, 0.4, 0.8, 0.3, 0.25, 0.6, 0.5, 0.7],
            gains: [95, 84, 78, 72, 80, 88, 76, 70],
            mutes: [false, false, false, true, false, false, false, false],
            solos: [false; 8],
            global_density: 0.6,
            feel_scale: 0.35,
            active_form_name: "form-0003".to_string(),
        };

        Self {
            snapshots: vec![s1, s2, s3],
            active_snapshot_id: "scene-boot-default".to_string(),
            lane_locks: [false; 8],
            queue_timbres_on_recall: false,
            search_query: String::new(),
            favorites_only: false,
            min_rating: 0,
            morph_engine: SceneMorphEngine::default(),
        }
    }

    /// Looks up a snapshot by ID or case-insensitive label.
    #[must_use]
    pub fn get_snapshot(&self, id_or_label: &str) -> Option<&SnapshotSceneRecord> {
        self.snapshots.iter().find(|s| {
            s.id == id_or_label || s.label.eq_ignore_ascii_case(id_or_label)
        })
    }

    /// Looks up a mutable snapshot by ID or case-insensitive label.
    pub fn get_snapshot_mut(&mut self, id_or_label: &str) -> Option<&mut SnapshotSceneRecord> {
        self.snapshots.iter_mut().find(|s| {
            s.id == id_or_label || s.label.eq_ignore_ascii_case(id_or_label)
        })
    }

    /// Sets the rehearsal star rating (`0..=5`) for a snapshot.
    pub fn set_rating(&mut self, id_or_label: &str, rating: u8) -> Option<u8> {
        let snap = self.get_snapshot_mut(id_or_label)?;
        snap.rating = rating.min(5);
        Some(snap.rating)
    }

    /// Toggles the `favorite` flag on a snapshot.
    pub fn toggle_favorite(&mut self, id_or_label: &str) -> Option<bool> {
        let snap = self.get_snapshot_mut(id_or_label)?;
        snap.favorite = !snap.favorite;
        Some(snap.favorite)
    }

    /// Sets or toggles the lock state for a specific voice lane (`0..8`) and syncs it with
    /// `Op8PerformanceModel` when applied.
    pub fn set_lane_lock(&mut self, lane_index: usize, locked: bool) {
        if lane_index < 8 {
            self.lane_locks[lane_index] = locked;
        }
    }

    /// Captures the current live [`Op8PerformanceModel`] state as a new [`SnapshotSceneRecord`]
    /// and adds it to the pool.
    pub fn capture_from_model(
        &mut self,
        label: Option<String>,
        controller: &str,
        model: &Op8PerformanceModel,
    ) -> SnapshotSceneRecord {
        let idx = self.snapshots.len() + 1;
        let id = format!("snap-{idx:04}");
        let lbl = label.unwrap_or_else(|| format!("Snapshot {idx:04}"));
        let record = SnapshotSceneRecord::from_op8_model(id.clone(), lbl, controller, model);
        self.active_snapshot_id = id;
        self.snapshots.push(record.clone());
        record
    }

    /// Parses and imports all lines from a JSONL preset pool string
    /// (`nanokontrol2-random-preset-pool.jsonl` or `midimix-random-preset-pool.jsonl`).
    pub fn import_jsonl_str(&mut self, jsonl_content: &str) -> Result<usize, String> {
        let mut imported = 0usize;
        for (line_idx, raw_line) in jsonl_content.lines().enumerate() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let val: Value = serde_json::from_str(trimmed).map_err(|e| {
                format!("Invalid JSONL on line {}: {e}", line_idx + 1)
            })?;
            let record = SnapshotSceneRecord::from_json_value(&val, self.snapshots.len())?;
            if let Some(existing) = self
                .snapshots
                .iter_mut()
                .find(|s| s.id == record.id || s.signature == record.signature)
            {
                *existing = record;
            } else {
                self.snapshots.push(record);
            }
            imported += 1;
        }
        Ok(imported)
    }

    /// Exports all snapshots in the pool as a multi-line JSONL string.
    #[must_use]
    pub fn export_jsonl_str(&self) -> String {
        let mut out = String::new();
        for snap in &self.snapshots {
            out.push_str(&snap.to_jsonl_line());
            out.push('\n');
        }
        out
    }

    /// Appends a single [`SnapshotSceneRecord`] to a JSONL pool file on disk.
    pub fn append_snapshot_to_jsonl_file(
        &self,
        path: &Path,
        snapshot: &SnapshotSceneRecord,
    ) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{}", snapshot.to_jsonl_line()).map_err(|e| e.to_string())
    }

    /// Loads snapshots from a JSONL pool file on disk (`nanokontrol2-random-preset-pool.jsonl`
    /// or `midimix-random-preset-pool.jsonl`).
    pub fn load_jsonl_file(&mut self, path: &Path) -> Result<usize, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.import_jsonl_str(&content)
    }

    /// Saves a named timbre-state `.json` file to disk.
    pub fn save_named_scene_json_file(
        &self,
        path: &Path,
        id_or_label: &str,
    ) -> Result<(), String> {
        let snap = self
            .get_snapshot(id_or_label)
            .ok_or_else(|| format!("Snapshot not found: {id_or_label}"))?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        let pretty = snap.to_pretty_json()?;
        fs::write(path, pretty).map_err(|e| e.to_string())
    }

    /// Loads a named timbre-state `.json` file from disk and registers it in the pool.
    pub fn load_named_scene_json_file(
        &mut self,
        path: &Path,
    ) -> Result<SnapshotSceneRecord, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let val: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let record = SnapshotSceneRecord::from_json_value(&val, self.snapshots.len())?;
        if let Some(existing) = self.snapshots.iter_mut().find(|s| s.id == record.id) {
            *existing = record.clone();
        } else {
            self.snapshots.push(record.clone());
        }
        Ok(record)
    }

    /// Returns all snapshots matching the current search query, favorites toggle, and minimum rating.
    #[must_use]
    pub fn filtered_snapshots(&self) -> Vec<&SnapshotSceneRecord> {
        let q = self.search_query.trim().to_ascii_lowercase();
        self.snapshots
            .iter()
            .filter(|s| {
                if self.favorites_only && !s.favorite {
                    return false;
                }
                if s.rating < self.min_rating {
                    return false;
                }
                if !q.is_empty() {
                    let match_label = s.label.to_ascii_lowercase().contains(&q);
                    let match_id = s.id.to_ascii_lowercase().contains(&q);
                    let match_timbre = s
                        .timbres
                        .iter()
                        .any(|t| t.to_ascii_lowercase().contains(&q));
                    let match_tag = s
                        .tags
                        .iter()
                        .any(|t| t.to_ascii_lowercase().contains(&q));
                    if !(match_label || match_id || match_timbre || match_tag) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Applies a snapshot (`id_or_label`) to the live [`Op8PerformanceModel`], respecting both
    /// `self.lane_locks` and `model.lanes[i].timbre_locked` so locked voice lanes keep their
    /// existing timbres, densities, and gains!
    pub fn recall_snapshot_into_model(
        &mut self,
        id_or_label: &str,
        model: &mut Op8PerformanceModel,
    ) -> Option< Vec<usize> > {
        let snap = self.get_snapshot(id_or_label)?.clone();
        self.active_snapshot_id = snap.id.clone();

        let mut updated_lanes = Vec::new();
        for i in 0..8 {
            let is_locked = self.lane_locks[i] || model.lanes[i].timbre_locked;
            model.lanes[i].timbre_locked = is_locked;
            if is_locked {
                continue;
            }
            if self.queue_timbres_on_recall {
                model.lanes[i].queued_timbre = Some(snap.timbres[i].clone());
            } else {
                model.lanes[i].active_timbre = snap.timbres[i].clone();
                model.lanes[i].queued_timbre = None;
            }
            model.lanes[i].density_multiplier = snap.densities[i];
            model.lanes[i].density_cc = density_to_cc(snap.densities[i]);
            model.lanes[i].gain_cc = snap.gains[i];
            model.lanes[i].gain_normalized = f64::from(snap.gains[i]) / 127.0;
            model.lanes[i].muted = snap.mutes[i];
            model.lanes[i].soloed = snap.solos[i];
            updated_lanes.push(i);
        }

        model.global_density_multiplier = snap.global_density;
        model.global_density_cc = density_to_cc(snap.global_density);
        model.feel.set_scale(snap.feel_scale);

        Some(updated_lanes)
    }

    /// Evaluates the current [`SceneMorphEngine`] state and applies the interpolated
    /// continuous density/macro/feel values (and unlocked timbres) to `model`.
    pub fn apply_current_morph_to_model(
        &self,
        model: &mut Op8PerformanceModel,
    ) -> Option<MorphedSceneFrame> {
        let scene_a = self.get_snapshot(&self.morph_engine.scene_a_id)?;
        let scene_b = self.get_snapshot(&self.morph_engine.scene_b_id)?;
        let effective_locks: [bool; 8] =
            std::array::from_fn(|i| self.lane_locks[i] || model.lanes[i].timbre_locked);

        let frame = self
            .morph_engine
            .evaluate_frame(scene_a, scene_b, &effective_locks);

        for i in 0..8 {
            if effective_locks[i] {
                continue;
            }
            model.lanes[i].density_multiplier = frame.densities[i];
            model.lanes[i].density_cc = density_to_cc(frame.densities[i]);
            model.lanes[i].gain_cc = frame.gains[i];
            model.lanes[i].gain_normalized = f64::from(frame.gains[i]) / 127.0;
            if self.queue_timbres_on_recall {
                if model.lanes[i].active_timbre != frame.timbres[i] {
                    model.lanes[i].queued_timbre = Some(frame.timbres[i].clone());
                }
            } else {
                model.lanes[i].active_timbre = frame.timbres[i].clone();
                model.lanes[i].queued_timbre = None;
            }
        }

        model.global_density_multiplier = frame.global_density;
        model.global_density_cc = density_to_cc(frame.global_density);
        model.feel.set_scale(frame.feel_scale);
        let _ = feel_scale_to_cc(frame.feel_scale);

        Some(frame)
    }
}

// =============================================================================
// 4. Interactive egui Snapshot Pool & Scene Morphing Panel
// =============================================================================

/// Renders the 8opPerc Timbre Snapshot Browser, 8-Lane Lock Matrix, and Beat-Synchronized
/// Scene Morphing Deck inside the `egui` cockpit.
pub fn render_snapshot_pool_panel(
    ui: &mut Ui,
    pool: &mut SnapshotPoolManager,
    model: &mut Op8PerformanceModel,
    theme_settings: &ThemeSettings,
) {
    let palette = theme_settings.resolved_palette();
    let panel_fill = theme_settings.build_visuals().panel_fill;

    egui::Frame::none()
        .fill(panel_fill)
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // 1. Header + Capture Snapshot + Voice Lane Lock Strip
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("8OPPERC SNAPSHOT POOL & SCENE MORPHER")
                        .strong()
                        .color(palette.accent),
                );
                if ui.button("📷 Capture Snapshot (CC 60)").clicked() {
                    let _ = pool.capture_from_model(None, "korg-nanokontrol2", model);
                }
                ui.checkbox(
                    &mut pool.queue_timbres_on_recall,
                    RichText::new("Queue Timbres for Next Bar").small(),
                );
            });

            ui.add_space(4.0);

            // 8-Voice Lock Matrix
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("VOICE LOCKS:").small().weak());
                for (i, fam) in FAMILY_ORDER.iter().enumerate() {
                    let locked = pool.lane_locks[i] || model.lanes[i].timbre_locked;
                    let text = if locked {
                        format!("🔒 {}", fam.to_ascii_uppercase())
                    } else {
                        fam.to_ascii_uppercase()
                    };
                    if ui.selectable_label(locked, RichText::new(text).small()).clicked() {
                        let next = !locked;
                        pool.lane_locks[i] = next;
                        model.lanes[i].timbre_locked = next;
                    }
                }
            });

            ui.separator();

            // 2. Filter Bar + Snapshot List
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Search:").small());
                ui.add(
                    egui::TextEdit::singleline(&mut pool.search_query)
                        .desired_width(140.0)
                        .hint_text("Scene, timbre, tag..."),
                );
                if ui
                    .selectable_label(pool.favorites_only, "★ Favorites")
                    .clicked()
                {
                    pool.favorites_only = !pool.favorites_only;
                }
                ui.label(RichText::new("Min Rating:").small());
                for stars in 0..=5u8 {
                    let lbl = if stars == 0 {
                        "Any".to_string()
                    } else {
                        format!("{stars}★+")
                    };
                    if ui.selectable_label(pool.min_rating == stars, lbl).clicked() {
                        pool.min_rating = stars;
                    }
                }
            });

            let rows: Vec<(String, String, String, u8, bool, [String; 8])> = pool
                .filtered_snapshots()
                .into_iter()
                .map(|s| {
                    (
                        s.id.clone(),
                        s.label.clone(),
                        s.controller.clone(),
                        s.rating,
                        s.favorite,
                        s.timbres.clone(),
                    )
                })
                .collect();

            ScrollArea::vertical()
                .id_salt("op8_snapshot_pool_scroll")
                .max_height(130.0)
                .show(ui, |ui| {
                    for (id, label, controller, rating, fav, timbres) in rows {
                        let is_active = pool.active_snapshot_id == id;
                        ui.horizontal(|ui| {
                            if ui.small_button(if fav { "★" } else { "☆" }).clicked() {
                                let _ = pool.toggle_favorite(&id);
                            }
                            let stars_str = "★".repeat(rating as usize)
                                + &"☆".repeat(5usize.saturating_sub(rating as usize));
                            if ui
                                .small_button(RichText::new(stars_str).monospace())
                                .clicked()
                            {
                                let next_rating = (rating + 1) % 6;
                                let _ = pool.set_rating(&id, next_rating);
                            }
                            if ui
                                .selectable_label(
                                    is_active,
                                    format!("{label} [{controller}] — {} / {}", timbres[0], timbres[1]),
                                )
                                .clicked()
                            {
                                let _ = pool.recall_snapshot_into_model(&id, model);
                            }
                            if ui.small_button("Scene A").clicked() {
                                pool.morph_engine.scene_a_id = id.clone();
                            }
                            if ui.small_button("Scene B").clicked() {
                                pool.morph_engine.scene_b_id = id.clone();
                            }
                        });
                    }
                });

            ui.separator();

            // 3. Scene Morphing Deck (Scene A <-> Scene B over configurable beats)
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "SCENE MORPH: [A] {}  ->  [B] {}",
                        pool.morph_engine.scene_a_id, pool.morph_engine.scene_b_id
                    ))
                    .strong()
                    .color(palette.accent_strong),
                );
                ui.label(RichText::new("Beats:").small());
                for &b in &[2.0, 4.0, 8.0, 16.0, 32.0] {
                    let selected = (pool.morph_engine.duration_beats - b).abs() < 1e-4;
                    if ui
                        .selectable_label(selected, format!("{}b", b as u32))
                        .clicked()
                    {
                        pool.morph_engine.duration_beats = b;
                    }
                }
                if ui
                    .button(if pool.morph_engine.active {
                        "⏹ Morphing..."
                    } else {
                        "▶ Start Beat Morph"
                    })
                    .clicked()
                {
                    let dur = pool.morph_engine.duration_beats;
                    pool.morph_engine
                        .start_morph(model.transport.current_beat, dur);
                }

                let mut pct = (pool.morph_engine.progress * 100.0) as f32;
                if ui
                    .add(
                        egui::Slider::new(&mut pct, 0.0..=100.0)
                            .suffix("%")
                            .text("Morph A/B"),
                    )
                    .changed()
                {
                    pool.morph_engine.progress = f64::from(pct / 100.0).clamp(0.0, 1.0);
                    let _ = pool.apply_current_morph_to_model(model);
                }
            });
            let _ = Color32::TRANSPARENT;
        });
}

// =============================================================================
// 5. Automated Unit & Headless egui Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_preset_pools_nanokontrol2_and_legacy_midimix_roundtrip_and_named_json() {
        let mut pool = SnapshotPoolManager::new();
        let initial_len = pool.snapshots.len();

        // Simulate a legacy midimix-random-preset-pool.jsonl line (using KONDUKTIVA_FAMILY_ORDER keys)
        let legacy_midimix_line = r#"{"id":"midimix-0042","label":"Legacy MidiMix Take","controller":"akai-midimix","savedAt":"2026-10-01T12:00:00Z","timbres":{"kicks":"kick-iron","snares":"snare-wire","hats":"hat-crisp","toms":"hat-wide","bells":"clap-bell","metals":"tom-deep","noise":"rim-clang","fx":"shaker-sand"},"globalDensity":1.75,"feelScale":0.6}"#;
        let nk2_line = r#"{"id":"nk2-0099","label":"nanoKONTROL2 Live Capture","controller":"korg-nanokontrol2","savedAt":"2026-11-04T01:00:00Z","rating":5,"favorite":true,"timbres":{"kick":"kick-laser","snare":"snare-snap","hat_closed":"hat-tight","hat_open":"hat-air","clap":"clap-whip","tom":"tom-sub","rim":"rim-ping","cymbal":"shaker-maraca"},"densities":[1.2,1.4,2.0,1.0,1.5,1.8,1.1,2.5],"gains":[110,105,95,90,100,108,92,88]}"#;

        let combined_jsonl = format!("{legacy_midimix_line}\n{nk2_line}\n");
        let count = pool
            .import_jsonl_str(&combined_jsonl)
            .expect("import jsonl lines");
        assert_eq!(count, 2);
        assert_eq!(pool.snapshots.len(), initial_len + 2);

        // Verify legacy midimix keys mapped accurately onto canonical 8-voice FAMILY_ORDER
        let mm = pool
            .get_snapshot("midimix-0042")
            .expect("legacy midimix snapshot");
        assert_eq!(mm.timbres[0], "kick-iron");
        assert_eq!(mm.timbres[4], "clap-bell"); // bells -> lane 4
        assert_eq!(mm.timbres[5], "tom-deep"); // metals -> lane 5
        assert_eq!(mm.timbres[6], "rim-clang"); // noise -> lane 6
        assert!((mm.global_density - 1.75).abs() < 1e-6);

        // Round-trip exported JSONL and named JSON scene
        let exported_jsonl = pool.export_jsonl_str();
        let mut pool2 = SnapshotPoolManager::new();
        pool2.snapshots.clear();
        pool2
            .import_jsonl_str(&exported_jsonl)
            .expect("re-import exported jsonl");
        assert_eq!(pool2.snapshots.len(), pool.snapshots.len());

        let nk2_snap = pool.get_snapshot("nk2-0099").expect("nk2-0099");
        let pretty_json = nk2_snap.to_pretty_json().expect("pretty json");
        let parsed_val: Value = serde_json::from_str(&pretty_json).expect("valid json");
        let reparsed = SnapshotSceneRecord::from_json_value(&parsed_val, 0).expect("from_json_value");
        assert_eq!(reparsed.timbres, nk2_snap.timbres);
        assert_eq!(reparsed.rating, 5);
        assert!(reparsed.favorite);
    }

    #[test]
    fn selective_voice_lane_locking_preserves_kick_and_snare_during_snapshot_recall() {
        let mut pool = SnapshotPoolManager::new();
        let mut model = Op8PerformanceModel::new();

        // Set custom kick (lane 0) and snare (lane 1) timbres and densities
        model.lanes[0].active_timbre = "kick-custom-locked".to_string();
        model.lanes[0].density_multiplier = 3.5;
        model.lanes[1].active_timbre = "snare-custom-locked".to_string();
        model.lanes[1].density_multiplier = 2.75;

        // Lock kick (lane 0) via pool.lane_locks and snare (lane 1) via model.lanes[1].timbre_locked
        pool.set_lane_lock(0, true);
        model.lanes[1].timbre_locked = true;

        // Recall "scene-gqom-overdrive"
        let updated = pool
            .recall_snapshot_into_model("scene-gqom-overdrive", &mut model)
            .expect("recall gqom scene");
        assert_eq!(updated, vec![2, 3, 4, 5, 6, 7]);

        // Locked kick & snare must remain untouched
        assert_eq!(model.lanes[0].active_timbre, "kick-custom-locked");
        assert!((model.lanes[0].density_multiplier - 3.5).abs() < 1e-6);
        assert_eq!(model.lanes[1].active_timbre, "snare-custom-locked");
        assert!((model.lanes[1].density_multiplier - 2.75).abs() < 1e-6);

        // Unlocked hat_closed (lane 2), clap (lane 4), tom (lane 5) must load from Gqom scene
        assert_eq!(model.lanes[2].active_timbre, "hat-closed-tick");
        assert!((model.lanes[2].density_multiplier - 3.2).abs() < 1e-6);
        assert_eq!(model.lanes[4].active_timbre, "clap-broken-glass");
        assert_eq!(model.lanes[5].active_timbre, "tom-low-log");
    }

    #[test]
    fn scene_morph_engine_interpolates_densities_gains_and_feel_over_configurable_beats() {
        let mut pool = SnapshotPoolManager::new();
        let mut model = Op8PerformanceModel::new();

        pool.morph_engine.scene_a_id = "scene-boot-default".to_string();
        pool.morph_engine.scene_b_id = "scene-gqom-overdrive".to_string();

        // Lock lane 0 (kick) so morphing leaves kick density at 1.0
        pool.set_lane_lock(0, true);

        // Start an 8-beat morph at beat 0.0
        pool.morph_engine.start_morph(0.0, 8.0);
        assert!(pool.morph_engine.active);

        // Advance 4 beats (50% midpoint: t = 0.5)
        let p_mid = pool.morph_engine.advance_beats(4.0);
        assert!((p_mid - 0.5).abs() < 1e-6);

        let frame_mid = pool
            .apply_current_morph_to_model(&mut model)
            .expect("frame at 50%");

        // Lane 0 is locked -> stays at 1.0 (Scene A)
        assert!((frame_mid.densities[0] - 1.0).abs() < 1e-6);
        assert_eq!(frame_mid.timbres[0], DEFAULT_BOOT_PATCHES[0]);

        // Lane 2 (hat_closed) morphs halfway between 1.0 and 3.2 -> 2.1
        assert!((frame_mid.densities[2] - 2.1).abs() < 1e-6);
        assert!((model.lanes[2].density_multiplier - 2.1).abs() < 1e-6);

        // Global density morphs halfway between 1.0 and 2.2 -> 1.6
        assert!((frame_mid.global_density - 1.6).abs() < 1e-6);
        // Global feel scale morphs halfway between 0.5 and 0.75 -> 0.625
        assert!((frame_mid.feel_scale - 0.625).abs() < 1e-6);

        // At t = 0.5 (>= timbre_switch_point 0.5), unlocked lanes switch to Scene B timbres
        assert_eq!(frame_mid.timbres[2], "hat-closed-tick");

        // Advance remaining 4 beats (100%: t = 1.0)
        let p_end = pool.morph_engine.advance_beats(4.0);
        assert!((p_end - 1.0).abs() < 1e-6);
        assert!(!pool.morph_engine.active);
        let frame_end = pool
            .apply_current_morph_to_model(&mut model)
            .expect("frame at 100%");
        assert!((frame_end.densities[2] - 3.2).abs() < 1e-6);
        assert!((frame_end.global_density - 2.2).abs() < 1e-6);
    }

    #[test]
    fn headless_egui_snapshot_pool_panel_renders_without_panic() {
        let ctx = egui::Context::default();
        let mut pool = SnapshotPoolManager::new();
        let mut model = Op8PerformanceModel::new();
        let theme = ThemeSettings::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_snapshot_pool_panel(ui, &mut pool, &mut model, &theme);
            });
        });
    }
}
