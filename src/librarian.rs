//! Preset Librarian & A/B Diff/Merge Engine (`src/librarian.rs`).
//!
//! Provides:
//! 1. **14 Built-in Factory Presets**:
//!    - **10 Original `nkEditor3` Factory Presets**:
//!      `Default`, `Synth Lead`, `Drums`, `DAW Transport`, `DJ Mixer`, `Nord Drum 2`
//!      (for Korg `nanoKONTROL2`); and `Default MidiMix`, `MidiMix Synth`, `MidiMix DJ`,
//!      `MidiMix Nord Drum 2` (for Akai `MIDI Mix`).
//!    - **4 New `nkEditor4` / `8opPerc` Factory Presets**:
//!      `8opPerc nanoKONTROL2 Live Rig`, `8opPerc MIDImix Compatibility`,
//!      `Konduktiva 8-Lane Macro Surface`, and `Dual-Controller Split Rig`.
//! 2. **JSON & Legacy Text (`mappings/default.txt`) Import/Export** via [`crate::config_parser`].
//! 3. **5 Quick Access Preset Slots**, real-time search, group/tag/device filtering, and favorites.
//! 4. **A/B Preset Diff & Selective Merge Engine and `egui` View**:
//!    Compares any two presets across mappings, MIDI channels, min/max ranges, response curves,
//!    button behaviors, labels, and saved control values, with per-control selective merge.

use crate::config_parser::{
    format_mapping_to_text, format_preset_json, import_preset_auto, ConfigParseError,
};
use crate::devices::{nord_drum_2, ButtonBehavior, MidiMessageType};
use crate::mapping_engine::{ControlMappingEntry, DeviceMappingConfig, ResponseCurve};
use crate::op8_model::FAMILY_ORDER;
use crate::theme::ThemeSettings;
use egui::{Color32, RichText, ScrollArea, Stroke, Ui};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Number of Quick Access preset slots in the Librarian header bar.
pub const QUICK_ACCESS_SLOT_COUNT: usize = 5;

// =============================================================================
// 1. Preset Metadata & Preset Structs
// =============================================================================

/// Descriptive metadata for a [`Preset`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetMetadata {
    /// Stable preset identifier (slug, e.g. `"factory-default"`, `"factory-8opperc-nk2-live"`).
    pub id: String,
    /// Human-readable preset title (e.g. `"Synth Lead"`, `"8opPerc nanoKONTROL2 Live Rig"`).
    pub name: String,
    /// Summary of what the preset controls and how it is laid out.
    pub description: String,
    /// Group / folder category (e.g. `"Factory"`, `"MidiMix Factory"`, `"Nord Drum 2"`, `"8opPerc"`, `"User"`).
    pub group: String,
    /// Primary hardware controller ID (`"nanokontrol2"` or `"akai-midimix"`).
    pub device_id: String,
    /// Searchable lowercase tags.
    pub tags: Vec<String>,
    /// Whether the user has starred this preset as a favorite.
    pub favorite: bool,
    /// `true` for the 14 built-in factory presets (protected against accidental deletion).
    pub is_factory: bool,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last modification timestamp.
    pub modified_at: String,
}

impl PresetMetadata {
    /// Creates metadata for a preset.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        group: impl Into<String>,
        device_id: impl Into<String>,
        tags: Vec<&str>,
        is_factory: bool,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            group: group.into(),
            device_id: device_id.into(),
            tags: tags.into_iter().map(str::to_string).collect(),
            favorite: false,
            is_factory,
            created_at: "2026-11-04T00:00:00Z".to_string(),
            modified_at: "2026-11-04T00:00:00Z".to_string(),
        }
    }
}

/// Complete controller preset including metadata, primary [`DeviceMappingConfig`],
/// optional secondary controller mapping (for dual-controller rigs), and saved control values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    /// Metadata (name, group, tags, favorite flag).
    pub metadata: PresetMetadata,
    /// Primary controller mapping configuration.
    pub mapping: DeviceMappingConfig,
    /// Optional secondary controller mapping configuration (used by `Dual-Controller Split Rig`).
    #[serde(default)]
    pub secondary_mapping: Option<DeviceMappingConfig>,
    /// Saved continuous control target values (`control_path -> 0..=127`).
    #[serde(default)]
    pub continuous_targets: BTreeMap<String, u8>,
    /// Saved logical button ON/OFF states (`control_path -> bool`).
    #[serde(default)]
    pub button_states: BTreeMap<String, bool>,
}

impl Preset {
    /// Constructs a [`Preset`] from a name, description, group, and [`DeviceMappingConfig`],
    /// initializing default control values (`64` for knobs, `100` for sliders/faders, `false` for buttons).
    #[must_use]
    pub fn from_mapping(
        name: impl Into<String>,
        description: impl Into<String>,
        group: impl Into<String>,
        mapping: DeviceMappingConfig,
    ) -> Self {
        let name_str = name.into();
        let slug = slugify_preset_name(&name_str);
        let device_id = mapping.device_id.clone();

        let mut continuous_targets = BTreeMap::new();
        let mut button_states = BTreeMap::new();
        for (path, entry) in &mapping.entries {
            if entry.is_button {
                button_states.insert(path.clone(), false);
            } else if path.contains("knob") {
                continuous_targets.insert(path.clone(), 64);
            } else {
                continuous_targets.insert(path.clone(), 100);
            }
        }

        Self {
            metadata: PresetMetadata::new(
                slug,
                name_str,
                description,
                group,
                device_id,
                vec!["user"],
                false,
            ),
            mapping,
            secondary_mapping: None,
            continuous_targets,
            button_states,
        }
    }
}

/// Converts a preset name into a URL-safe lowercase ID slug.
#[must_use]
pub fn slugify_preset_name(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "preset".to_string()
    } else {
        out
    }
}

/// Generates a unique duplicate preset name following `nkEditor3` rules:
/// - `"Synth Lead"` -> `"Synth Lead copy"`
/// - `"Synth Lead copy"` -> `"Synth Lead copy 2"`
/// - `"Synth Lead copy 2"` -> `"Synth Lead copy 3"`
#[must_use]
pub fn generate_duplicate_name(original_name: &str, existing_names: &[String]) -> String {
    let lower = original_name.to_ascii_lowercase();
    let (base_name, mut copy_num) = if let Some(idx) = lower.rfind(" copy") {
        let suffix = original_name[idx + 5..].trim();
        if suffix.is_empty() {
            (&original_name[..idx], 2usize)
        } else if let Ok(n) = suffix.parse::<usize>() {
            (&original_name[..idx], n + 1)
        } else {
            (original_name, 0usize)
        }
    } else {
        (original_name, 0usize)
    };

    let existing_lower: BTreeSet<String> = existing_names
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let mut candidate = if copy_num == 0 {
        format!("{base_name} copy")
    } else {
        format!("{base_name} copy {copy_num}")
    };

    while existing_lower.contains(&candidate.to_ascii_lowercase()) {
        copy_num = if copy_num == 0 { 2 } else { copy_num + 1 };
        candidate = format!("{base_name} copy {copy_num}");
    }

    candidate
}

// =============================================================================
// 2. All 14 Built-in Factory Presets (10 Original nkEditor3 + 4 New nkEditor4)
// =============================================================================

fn build_factory_preset(
    id: &str,
    name: &str,
    description: &str,
    group: &str,
    tags: Vec<&str>,
    mapping: DeviceMappingConfig,
) -> Preset {
    let mut preset = Preset::from_mapping(name, description, group, mapping);
    preset.metadata.id = id.to_string();
    preset.metadata.tags = tags.into_iter().map(str::to_string).collect();
    preset.metadata.is_factory = true;
    preset
}

/// 1. Original `Default` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_default_nk2() -> Preset {
    build_factory_preset(
        "factory-default",
        "Default",
        "Identity mapping - input CCs pass through unchanged. Use as a starting point for custom configurations.",
        "Factory",
        vec!["factory", "default", "identity", "nanokontrol2"],
        DeviceMappingConfig::default_nanokontrol2(),
    )
}

/// 2. Original `Synth Lead` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_synth_lead_nk2() -> Preset {
    let mut mapping = DeviceMappingConfig::default_nanokontrol2();
    let synth_knob_ccs: [u8; 8] = [74, 71, 73, 72, 91, 93, 1, 11];
    let synth_knob_labels: [&str; 8] = [
        "Filter",
        "Resonance",
        "Attack",
        "Release",
        "Reverb",
        "Chorus",
        "Mod Wheel",
        "Expression",
    ];
    let synth_slider_ccs: [u8; 8] = [16, 17, 18, 19, 20, 21, 22, 7];
    let synth_slider_labels: [&str; 8] = [
        "OSC 1", "OSC 2", "OSC 3", "Sub", "Noise", "Aux 1", "Aux 2", "Master",
    ];

    for i in 0..8 {
        let track_num = i + 1;
        if let Some(knob) = mapping.entries.get_mut(&format!("track{track_num}.knob")) {
            knob.output_number = synth_knob_ccs[i];
            knob.label = Some(synth_knob_labels[i].to_string());
        }
        if let Some(slider) = mapping.entries.get_mut(&format!("track{track_num}.slider")) {
            slider.output_number = synth_slider_ccs[i];
            slider.label = Some(synth_slider_labels[i].to_string());
        }
    }

    build_factory_preset(
        "factory-synth-lead",
        "Synth Lead",
        "Optimized for synthesizer lead patches. Knobs control filter cutoff, resonance, envelope, and modulation parameters.",
        "Factory",
        vec!["factory", "synth", "lead", "performance", "nanokontrol2"],
        mapping,
    )
}

/// 3. Original `Drums` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_drums_nk2() -> Preset {
    let mut mapping = DeviceMappingConfig::default_nanokontrol2();
    let drum_knob_ccs: [u8; 8] = [10, 42, 43, 44, 45, 46, 47, 48];
    let drum_knob_labels: [&str; 8] = [
        "Kick Tune",
        "Snare Tune",
        "HH Decay",
        "Tom Tune",
        "Perc Tune",
        "Cymbal",
        "FX 1",
        "FX 2",
    ];
    let drum_slider_labels: [&str; 8] = [
        "Kick", "Snare", "HiHat", "Tom 1", "Tom 2", "Perc", "FX", "Master",
    ];

    for i in 0..8 {
        let track_num = i + 1;
        if let Some(knob) = mapping.entries.get_mut(&format!("track{track_num}.knob")) {
            knob.output_number = drum_knob_ccs[i];
            knob.label = Some(drum_knob_labels[i].to_string());
        }
        if let Some(slider) = mapping.entries.get_mut(&format!("track{track_num}.slider")) {
            slider.label = Some(drum_slider_labels[i].to_string());
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{track_num}.solo")) {
            solo.behavior = ButtonBehavior::Momentary;
            solo.label = Some("Trigger".to_string());
        }
    }

    build_factory_preset(
        "factory-drums",
        "Drums",
        "Optimized for drum machines and samplers. Sliders control individual drum levels, knobs adjust tuning and decay parameters.",
        "Factory",
        vec!["factory", "drums", "percussion", "beats", "nanokontrol2"],
        mapping,
    )
}

/// 4. Original `DAW Transport` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_daw_transport_nk2() -> Preset {
    let mut mapping = DeviceMappingConfig::default_nanokontrol2();
    let transport_overrides: [(&str, u8, &str); 6] = [
        ("transport.play", 94, "Play"),
        ("transport.stop", 93, "Stop"),
        ("transport.record", 95, "Record"),
        ("transport.rewind", 91, "Rewind"),
        ("transport.forward", 92, "Fast Fwd"),
        ("transport.cycle", 86, "Loop"),
    ];
    for (path, cc, label) in transport_overrides {
        if let Some(entry) = mapping.entries.get_mut(path) {
            entry.output_number = cc;
            entry.label = Some(label.to_string());
        }
    }

    let daw_track_labels: [&str; 8] = [
        "Ch 1", "Ch 2", "Ch 3", "Ch 4", "Ch 5", "Ch 6", "Ch 7", "Master",
    ];
    for i in 0..8 {
        let track_num = i + 1;
        if let Some(slider) = mapping.entries.get_mut(&format!("track{track_num}.slider")) {
            slider.label = Some(daw_track_labels[i].to_string());
        }
        if let Some(knob) = mapping.entries.get_mut(&format!("track{track_num}.knob")) {
            knob.label = Some(format!("Pan {track_num}"));
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{track_num}.solo")) {
            solo.label = Some("Solo".to_string());
        }
        if let Some(mute) = mapping.entries.get_mut(&format!("track{track_num}.mute")) {
            mute.label = Some("Mute".to_string());
        }
        if let Some(rec) = mapping.entries.get_mut(&format!("track{track_num}.rec")) {
            rec.label = Some("Arm".to_string());
        }
    }

    build_factory_preset(
        "factory-daw-transport",
        "DAW Transport",
        "Focused on DAW control with standard transport mappings. Compatible with most DAWs using Mackie Control protocol.",
        "Factory",
        vec!["factory", "daw", "transport", "mixing", "nanokontrol2"],
        mapping,
    )
}

/// 5. Original `DJ Mixer` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_dj_mixer_nk2() -> Preset {
    let mut mapping = DeviceMappingConfig::default_nanokontrol2();
    let dj_knob_ccs: [u8; 8] = [22, 23, 24, 25, 26, 27, 28, 29];
    let dj_knob_labels: [&str; 8] = [
        "A Hi", "A Mid", "A Lo", "A Filter", "B Hi", "B Mid", "B Lo", "B Filter",
    ];
    let dj_slider_labels: [&str; 8] = [
        "Deck A", "A Gain", "X-Fade", "Master", "Deck B", "B Gain", "Cue Mix", "Cue Vol",
    ];

    for i in 0..8 {
        let track_num = i + 1;
        if let Some(knob) = mapping.entries.get_mut(&format!("track{track_num}.knob")) {
            knob.output_number = dj_knob_ccs[i];
            knob.label = Some(dj_knob_labels[i].to_string());
        }
        if let Some(slider) = mapping.entries.get_mut(&format!("track{track_num}.slider")) {
            slider.label = Some(dj_slider_labels[i].to_string());
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{track_num}.solo")) {
            solo.behavior = ButtonBehavior::Toggle;
            solo.label = Some("Cue".to_string());
        }
    }

    if let Some(m1) = mapping.entries.get_mut("track1.mute") {
        m1.behavior = ButtonBehavior::Toggle;
        m1.label = Some("A Play".to_string());
    }
    if let Some(m5) = mapping.entries.get_mut("track5.mute") {
        m5.behavior = ButtonBehavior::Toggle;
        m5.label = Some("B Play".to_string());
    }
    if let Some(r1) = mapping.entries.get_mut("track1.rec") {
        r1.label = Some("A Sync".to_string());
    }
    if let Some(r5) = mapping.entries.get_mut("track5.rec") {
        r5.label = Some("B Sync".to_string());
    }

    build_factory_preset(
        "factory-dj-mixer",
        "DJ Mixer",
        "DJ-style layout with channel faders, 3-band EQ on knobs, and cue/sync controls. Works well with DJ software.",
        "Factory",
        vec!["factory", "dj", "mixer", "live", "nanokontrol2"],
        mapping,
    )
}

/// 6. Original `Nord Drum 2` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_nord_drum_2_nk2() -> Preset {
    let mut mapping = DeviceMappingConfig::default_nanokontrol2();

    // Tracks 1..=6 -> Pads 1..=6 on channels 1..=6
    for i in 0..6 {
        let pad_num = i + 1;
        let ch = nord_drum_2::channels::PADS[i];
        if let Some(knob) = mapping.entries.get_mut(&format!("track{pad_num}.knob")) {
            knob.output_number = nord_drum_2::tone::SPECTRA;
            knob.output_channel = ch;
            knob.label = Some(format!("Pad{pad_num} Spectra"));
        }
        if let Some(slider) = mapping.entries.get_mut(&format!("track{pad_num}.slider")) {
            slider.output_number = nord_drum_2::tone::DECAY;
            slider.output_channel = ch;
            slider.label = Some(format!("Pad{pad_num} Decay"));
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{pad_num}.solo")) {
            solo.output_channel = ch;
            solo.behavior = ButtonBehavior::Momentary;
            solo.label = Some("Trigger".to_string());
        }
        if let Some(mute) = mapping.entries.get_mut(&format!("track{pad_num}.mute")) {
            mute.output_channel = ch;
            mute.behavior = ButtonBehavior::Toggle;
            mute.label = Some("Mute".to_string());
        }
        if let Some(rec) = mapping.entries.get_mut(&format!("track{pad_num}.rec")) {
            rec.output_channel = ch;
            rec.label = Some("Rec".to_string());
        }
    }

    // Track 7: Global FX - Echo (channel 10)
    let fx_ch = nord_drum_2::channels::GLOBAL_FX;
    if let Some(k7) = mapping.entries.get_mut("track7.knob") {
        k7.output_number = nord_drum_2::fx::ECHO_AMOUNT;
        k7.output_channel = fx_ch;
        k7.label = Some("Echo Amount".to_string());
    }
    if let Some(s7) = mapping.entries.get_mut("track7.slider") {
        s7.output_number = nord_drum_2::fx::ECHO_FEEDBACK;
        s7.output_channel = fx_ch;
        s7.label = Some("Echo Feedbk".to_string());
    }
    for (suffix, beh, lbl) in [
        ("solo", ButtonBehavior::Momentary, "Trigger"),
        ("mute", ButtonBehavior::Toggle, "Mute"),
        ("rec", ButtonBehavior::Momentary, "Rec"),
    ] {
        if let Some(btn) = mapping.entries.get_mut(&format!("track7.{suffix}")) {
            btn.output_channel = fx_ch;
            btn.behavior = beh;
            btn.label = Some(lbl.to_string());
        }
    }

    // Track 8: Global FX - Distortion & EQ (channel 10)
    if let Some(k8) = mapping.entries.get_mut("track8.knob") {
        k8.output_number = nord_drum_2::fx::DIST_AMOUNT;
        k8.output_channel = fx_ch;
        k8.label = Some("Dist Amount".to_string());
    }
    if let Some(s8) = mapping.entries.get_mut("track8.slider") {
        s8.output_number = nord_drum_2::fx::EQ_GAIN;
        s8.output_channel = fx_ch;
        s8.label = Some("EQ Gain".to_string());
    }
    for (suffix, beh, lbl) in [
        ("solo", ButtonBehavior::Momentary, "Trigger"),
        ("mute", ButtonBehavior::Toggle, "Mute"),
        ("rec", ButtonBehavior::Momentary, "Rec"),
    ] {
        if let Some(btn) = mapping.entries.get_mut(&format!("track8.{suffix}")) {
            btn.output_channel = fx_ch;
            btn.behavior = beh;
            btn.label = Some(lbl.to_string());
        }
    }

    build_factory_preset(
        "factory-nord-drum-2-nk2",
        "Nord Drum 2",
        "Nord Drum 2 Desktop control. Knobs: Tone Spectra, Sliders: Tone Decay. Tracks 1-6 = Pads, Tracks 7-8 = Global FX (ch10).",
        "Nord Drum 2",
        vec![
            "factory",
            "nord-drum-2",
            "percussion",
            "synthesizer",
            "nanokontrol2",
        ],
        mapping,
    )
}

/// 7. Original `Default MidiMix` preset for Akai `MIDI Mix`.
#[must_use]
pub fn factory_preset_default_midimix() -> Preset {
    build_factory_preset(
        "factory-default-midimix",
        "Default MidiMix",
        "Identity mapping for the Akai MidiMix. All controls pass through unchanged with correct CC/Note message types.",
        "MidiMix Factory",
        vec!["factory", "midimix", "default", "identity"],
        DeviceMappingConfig::default_akai_midimix(),
    )
}

/// 8. Original `MidiMix Synth` preset for Akai `MIDI Mix`.
#[must_use]
pub fn factory_preset_midimix_synth() -> Preset {
    let mut mapping = DeviceMappingConfig::default_akai_midimix();
    let synth_knob_ccs: [u8; 8] = [74, 71, 73, 72, 91, 93, 1, 11];
    let synth_knob_labels: [&str; 8] = [
        "Filter",
        "Resonance",
        "Attack",
        "Release",
        "Reverb",
        "Chorus",
        "Mod Wheel",
        "Expression",
    ];
    let synth_fader_labels: [&str; 8] = [
        "OSC 1",
        "OSC 2",
        "Sub",
        "Noise",
        "Filter Env",
        "Amp Env",
        "FX Send",
        "Master",
    ];

    for i in 0..8 {
        let track_num = i + 1;
        if let Some(k1) = mapping.entries.get_mut(&format!("track{track_num}.knob1")) {
            k1.output_number = synth_knob_ccs[i];
            k1.label = Some(synth_knob_labels[i].to_string());
        }
        if let Some(fader) = mapping.entries.get_mut(&format!("track{track_num}.fader")) {
            fader.label = Some(synth_fader_labels[i].to_string());
        }
    }

    build_factory_preset(
        "factory-midimix-synth",
        "MidiMix Synth",
        "Synth control layout for MidiMix. Knobs control filter cutoff, resonance, and envelope. Faders control oscillator and mix levels.",
        "MidiMix Factory",
        vec!["factory", "midimix", "synth", "performance"],
        mapping,
    )
}

/// 9. Original `MidiMix DJ` preset for Akai `MIDI Mix`.
#[must_use]
pub fn factory_preset_midimix_dj() -> Preset {
    let mut mapping = DeviceMappingConfig::default_akai_midimix();
    let eq_high_ccs: [u8; 8] = [22, 23, 24, 25, 26, 27, 28, 29];

    for i in 0..8 {
        let track_num = i + 1;
        if let Some(k1) = mapping.entries.get_mut(&format!("track{track_num}.knob1")) {
            k1.output_number = eq_high_ccs[i];
            k1.label = Some(format!("Ch{track_num} Hi"));
        }
        if let Some(fader) = mapping.entries.get_mut(&format!("track{track_num}.fader")) {
            fader.label = Some(format!("Ch{track_num} Vol"));
        }
        if let Some(mute) = mapping.entries.get_mut(&format!("track{track_num}.mute")) {
            mute.label = Some("Cue".to_string());
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{track_num}.solo")) {
            solo.label = Some("PFL".to_string());
        }
    }

    build_factory_preset(
        "factory-midimix-dj",
        "MidiMix DJ",
        "DJ-style layout for MidiMix. Knob row 1 as EQ High, faders as channel volumes, buttons as cue/sync controls.",
        "MidiMix Factory",
        vec!["factory", "midimix", "dj", "live"],
        mapping,
    )
}

/// 10. Original `MidiMix Nord Drum 2` preset for Akai `MIDI Mix`.
#[must_use]
pub fn factory_preset_midimix_nord_drum_2() -> Preset {
    let mut mapping = DeviceMappingConfig::default_akai_midimix();

    // Tracks 1..=6 -> Pads 1..=6 on channels 1..=6
    for i in 0..6 {
        let pad_num = i + 1;
        let ch = nord_drum_2::channels::PADS[i];
        if let Some(k1) = mapping.entries.get_mut(&format!("track{pad_num}.knob1")) {
            k1.output_number = nord_drum_2::tone::SPECTRA;
            k1.output_channel = ch;
            k1.label = Some(format!("Pad{pad_num} Spectra"));
        }
        if let Some(k2) = mapping.entries.get_mut(&format!("track{pad_num}.knob2")) {
            k2.output_number = nord_drum_2::noise::FILTER_FREQ;
            k2.output_channel = ch;
            k2.label = Some(format!("Pad{pad_num} NoiseFlt"));
        }
        if let Some(k3) = mapping.entries.get_mut(&format!("track{pad_num}.knob3")) {
            k3.output_number = nord_drum_2::tone::TIMBRE;
            k3.output_channel = ch;
            k3.label = Some(format!("Pad{pad_num} Timbre"));
        }
        if let Some(fader) = mapping.entries.get_mut(&format!("track{pad_num}.fader")) {
            fader.output_number = nord_drum_2::mix::LEVEL;
            fader.output_channel = ch;
            fader.label = Some(format!("Pad{pad_num} Level"));
        }
        if let Some(mute) = mapping.entries.get_mut(&format!("track{pad_num}.mute")) {
            mute.output_channel = ch;
            mute.behavior = ButtonBehavior::Toggle;
            mute.label = Some("Mute".to_string());
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{pad_num}.solo")) {
            solo.output_channel = ch;
            solo.behavior = ButtonBehavior::Momentary;
            solo.label = Some("Trigger".to_string());
        }
        if let Some(rec) = mapping.entries.get_mut(&format!("track{pad_num}.rec_arm")) {
            rec.output_channel = ch;
            rec.behavior = ButtonBehavior::Momentary;
            rec.label = Some("Rec".to_string());
        }
    }

    let fx_ch = nord_drum_2::channels::GLOBAL_FX;
    // Track 7: Global FX - Echo + EQ
    for (ctrl, cc, lbl) in [
        ("knob1", nord_drum_2::fx::ECHO_AMOUNT, "Echo Amt"),
        ("knob2", nord_drum_2::fx::ECHO_FEEDBACK, "Echo FB"),
        ("knob3", nord_drum_2::fx::EQ_FREQ, "EQ Freq"),
        ("fader", nord_drum_2::fx::EQ_GAIN, "EQ Gain"),
    ] {
        if let Some(entry) = mapping.entries.get_mut(&format!("track7.{ctrl}")) {
            entry.output_number = cc;
            entry.output_channel = fx_ch;
            entry.label = Some(lbl.to_string());
        }
    }
    // Track 8: Global FX - Distortion + Decay + Punch
    for (ctrl, cc, lbl) in [
        ("knob1", nord_drum_2::fx::DIST_AMOUNT, "Dist Amt"),
        ("knob2", nord_drum_2::fx::DIST_TYPE, "Dist Type"),
        ("knob3", nord_drum_2::tone::DECAY, "Tone Dcy"),
        ("fader", nord_drum_2::tone::PUNCH, "Punch"),
    ] {
        if let Some(entry) = mapping.entries.get_mut(&format!("track8.{ctrl}")) {
            entry.output_number = cc;
            entry.output_channel = fx_ch;
            entry.label = Some(lbl.to_string());
        }
    }
    for t in [7, 8] {
        for (ctrl, beh, lbl) in [
            ("mute", ButtonBehavior::Toggle, "Mute"),
            ("solo", ButtonBehavior::Momentary, "Trigger"),
            ("rec_arm", ButtonBehavior::Momentary, "Rec"),
        ] {
            if let Some(btn) = mapping.entries.get_mut(&format!("track{t}.{ctrl}")) {
                btn.output_channel = fx_ch;
                btn.behavior = beh;
                btn.label = Some(lbl.to_string());
            }
        }
    }

    build_factory_preset(
        "factory-midimix-nord-drum-2",
        "MidiMix Nord Drum 2",
        "Nord Drum 2 Desktop control for Akai MidiMix. 32 parameters: Knob1=Spectra, Knob2=Noise Filter, Knob3=Timbre, Faders=Level. Tracks 1-6=Pads, 7-8=Global FX (ch10).",
        "Nord Drum 2",
        vec![
            "factory",
            "nord-drum-2",
            "midimix",
            "percussion",
            "synthesizer",
        ],
        mapping,
    )
}

/// 11. New `8opPerc nanoKONTROL2 Live Rig` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_op8_nanokontrol2_live_rig() -> Preset {
    let mut mapping = DeviceMappingConfig::default_nanokontrol2();

    for (idx, family) in FAMILY_ORDER.iter().enumerate() {
        let track_num = idx + 1;
        let fam_upper = family.to_ascii_uppercase();
        if let Some(knob) = mapping.entries.get_mut(&format!("track{track_num}.knob")) {
            knob.label = Some(format!("{fam_upper} Density"));
        }
        if let Some(slider) = mapping.entries.get_mut(&format!("track{track_num}.slider")) {
            slider.label = Some(format!("{fam_upper} Gain/Macro"));
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{track_num}.solo")) {
            solo.behavior = ButtonBehavior::Toggle;
            solo.label = Some(format!("{fam_upper} Solo"));
        }
        if let Some(mute) = mapping.entries.get_mut(&format!("track{track_num}.mute")) {
            mute.behavior = ButtonBehavior::Toggle;
            mute.label = Some(format!("{fam_upper} Mute"));
        }
        if let Some(rec) = mapping.entries.get_mut(&format!("track{track_num}.rec")) {
            rec.behavior = ButtonBehavior::Momentary;
            rec.label = Some(format!("{fam_upper} Reroll"));
        }
    }

    let transport_labels: [(&str, &str); 11] = [
        ("transport.track_left", "Form Prev"),
        ("transport.track_right", "Form Next"),
        ("transport.cycle", "Next Timbre Set"),
        ("transport.marker_set", "Save Snapshot"),
        ("transport.marker_left", "Global Dens -"),
        ("transport.marker_right", "Global Dens +"),
        ("transport.rewind", "Feel Scale -"),
        ("transport.forward", "Feel Scale +"),
        ("transport.play", "Play Rig"),
        ("transport.stop", "Stop Rig"),
        ("transport.record", "Rec Quantize"),
    ];
    for (path, lbl) in transport_labels {
        if let Some(entry) = mapping.entries.get_mut(path) {
            entry.behavior = ButtonBehavior::Momentary;
            entry.label = Some(lbl.to_string());
        }
    }

    build_factory_preset(
        "factory-8opperc-nk2-live",
        "8opPerc nanoKONTROL2 Live Rig",
        "Native 8-lane 8opPerc performance mapping for Korg nanoKONTROL2: per-voice density ladder on knobs, level/macro on sliders, Solo/Mute/Timbre-Reroll on S/M/R buttons, and Form/Timbre-Set/Snapshot navigation on transport.",
        "8opPerc",
        vec![
            "factory",
            "8opperc",
            "nanokontrol2",
            "live",
            "konduktiva",
            "percussion",
        ],
        mapping,
    )
}

/// 12. New `8opPerc MIDImix Compatibility` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_op8_midimix_compatibility() -> Preset {
    let mapping = DeviceMappingConfig::nanokontrol2_to_midimix_emulation();
    build_factory_preset(
        "factory-8opperc-midimix-compat",
        "8opPerc MIDImix Compatibility",
        "Cross-message emulation mapping that translates Korg nanoKONTROL2 CC buttons and knobs into Akai MIDImix Note and CC messages (CC->Note for Mute/Solo/Rec/Bank/Cycle, CC->CC for Row 2 knobs and faders).",
        "8opPerc",
        vec![
            "factory",
            "8opperc",
            "midimix",
            "compatibility",
            "emulation",
            "nanokontrol2",
        ],
        mapping,
    )
}

/// 13. New `Konduktiva 8-Lane Macro Surface` preset for Korg `nanoKONTROL2`.
#[must_use]
pub fn factory_preset_konduktiva_macro_surface() -> Preset {
    let mut mapping = DeviceMappingConfig::default_nanokontrol2();

    for i in 0..8 {
        let track_num = i + 1;
        let ch = 2;
        if let Some(knob) = mapping.entries.get_mut(&format!("track{track_num}.knob")) {
            knob.output_number = 102 + (i as u8);
            knob.output_channel = ch;
            knob.curve = ResponseCurve::SCurve;
            knob.min_value = 8;
            knob.max_value = 120;
            knob.label = Some(format!("Env Macro {track_num}"));
        }
        if let Some(slider) = mapping.entries.get_mut(&format!("track{track_num}.slider")) {
            slider.output_number = 110 + (i as u8);
            slider.output_channel = ch;
            slider.curve = ResponseCurve::Logarithmic;
            slider.min_value = 0;
            slider.max_value = 127;
            slider.label = Some(format!("Stem Gain {track_num}"));
        }
        if let Some(solo) = mapping.entries.get_mut(&format!("track{track_num}.solo")) {
            solo.output_channel = ch;
            solo.label = Some(format!("Focus {track_num}"));
        }
        if let Some(mute) = mapping.entries.get_mut(&format!("track{track_num}.mute")) {
            mute.output_channel = ch;
            mute.label = Some(format!("Cut {track_num}"));
        }
        if let Some(rec) = mapping.entries.get_mut(&format!("track{track_num}.rec")) {
            rec.output_channel = ch;
            rec.label = Some(format!("Mutate {track_num}"));
        }
    }

    build_factory_preset(
        "factory-konduktiva-macro-surface",
        "Konduktiva 8-Lane Macro Surface",
        "Expressive 8-lane macro surface for Konduktiva MusicalEnvironment sessions on MIDI Channel 2 with S-Curve macro knobs (CC 102-109) and Logarithmic stem faders (CC 110-117).",
        "8opPerc",
        vec![
            "factory",
            "8opperc",
            "konduktiva",
            "macro",
            "scurve",
            "nanokontrol2",
        ],
        mapping,
    )
}

/// 14. New `Dual-Controller Split Rig` preset pairing Korg `nanoKONTROL2` + Akai `MIDI Mix`.
#[must_use]
pub fn factory_preset_dual_controller_split_rig() -> Preset {
    let mut primary = factory_preset_op8_nanokontrol2_live_rig().mapping;
    for entry in primary.entries.values_mut() {
        entry.output_channel = 1;
    }
    let secondary = factory_preset_midimix_nord_drum_2().mapping;

    let mut preset = build_factory_preset(
        "factory-dual-controller-split-rig",
        "Dual-Controller Split Rig",
        "Simultaneous dual-controller split rig: Korg nanoKONTROL2 drives 8opPerc rhythmic density, Solo/Mute/Reroll, and Form navigation on Ch 1 while Akai MIDImix drives 32 Nord Drum 2 synthesis & Global FX parameters on Ch 1-6 & Ch 10.",
        "8opPerc",
        vec![
            "factory",
            "8opperc",
            "dual-controller",
            "split-rig",
            "nanokontrol2",
            "midimix",
            "nord-drum-2",
        ],
        primary,
    );
    preset.secondary_mapping = Some(secondary);
    preset
}

/// Creates all **14 built-in factory presets** (10 original `nkEditor3` presets + 4 new `nkEditor4` / `8opPerc` presets).
#[must_use]
pub fn create_all_factory_presets() -> Vec<Preset> {
    vec![
        // 6 original Korg nanoKONTROL2 presets
        factory_preset_default_nk2(),
        factory_preset_synth_lead_nk2(),
        factory_preset_drums_nk2(),
        factory_preset_daw_transport_nk2(),
        factory_preset_dj_mixer_nk2(),
        factory_preset_nord_drum_2_nk2(),
        // 4 original Akai MIDImix presets
        factory_preset_default_midimix(),
        factory_preset_midimix_synth(),
        factory_preset_midimix_dj(),
        factory_preset_midimix_nord_drum_2(),
        // 4 new nkEditor4 / 8opPerc presets
        factory_preset_op8_nanokontrol2_live_rig(),
        factory_preset_op8_midimix_compatibility(),
        factory_preset_konduktiva_macro_surface(),
        factory_preset_dual_controller_split_rig(),
    ]
}

// =============================================================================
// 3. A/B Preset Diff & Selective Merge Engine
// =============================================================================

/// Identifies which property of a control mapping or state differs between Preset A and Preset B.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDiff {
    /// Property name (e.g. `"output_number"`, `"output_channel"`, `"range"`, `"curve"`, `"behavior"`, `"label"`, `"target_value"`).
    pub field_name: String,
    /// Formatted value in Preset A.
    pub value_a: String,
    /// Formatted value in Preset B.
    pub value_b: String,
}

/// Detailed diff for a single control path (`"track1.knob"`, `"transport.play"`, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlDiffEntry {
    /// Dot-notated control path.
    pub control_path: String,
    /// Individual property differences detected for this control.
    pub field_diffs: Vec<FieldDiff>,
}

/// Complete A/B comparison report between two presets (`preset_a` vs `preset_b`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetDiffReport {
    /// ID of Preset A (baseline).
    pub preset_a_id: String,
    /// Name of Preset A.
    pub preset_a_name: String,
    /// ID of Preset B (comparison / incoming source).
    pub preset_b_id: String,
    /// Name of Preset B.
    pub preset_b_name: String,
    /// Control paths present only in Preset A.
    pub only_in_a: Vec<String>,
    /// Control paths present only in Preset B.
    pub only_in_b: Vec<String>,
    /// Controls present in both presets that have one or more differing fields.
    pub modified_controls: Vec<ControlDiffEntry>,
    /// Count of controls that are 100% identical across both presets.
    pub identical_count: usize,
}

impl PresetDiffReport {
    /// Total number of controls that differ (modified + only in A + only in B).
    #[must_use]
    pub fn total_differences(&self) -> usize {
        self.modified_controls.len() + self.only_in_a.len() + self.only_in_b.len()
    }

    /// Returns `true` if the two presets have zero mapping or value differences.
    #[must_use]
    pub fn is_identical(&self) -> bool {
        self.total_differences() == 0
    }
}

/// Computes a control-by-control [`PresetDiffReport`] comparing `preset_a` and `preset_b`
/// across mappings, MIDI channels, ranges, curves, button behaviors, labels, and control values.
#[must_use]
pub fn diff_presets(preset_a: &Preset, preset_b: &Preset) -> PresetDiffReport {
    let mut all_paths: BTreeSet<String> = BTreeSet::new();
    for k in preset_a.mapping.entries.keys() {
        all_paths.insert(k.clone());
    }
    for k in preset_b.mapping.entries.keys() {
        all_paths.insert(k.clone());
    }

    let mut only_in_a = Vec::new();
    let mut only_in_b = Vec::new();
    let mut modified_controls = Vec::new();
    let mut identical_count = 0usize;

    for path in all_paths {
        match (
            preset_a.mapping.entries.get(&path),
            preset_b.mapping.entries.get(&path),
        ) {
            (Some(_), None) => only_in_a.push(path),
            (None, Some(_)) => only_in_b.push(path),
            (Some(entry_a), Some(entry_b)) => {
                let mut diffs = compare_control_entries(entry_a, entry_b);

                // Compare saved continuous target value or button state
                if entry_a.is_button {
                    let state_a = preset_a.button_states.get(&path).copied().unwrap_or(false);
                    let state_b = preset_b.button_states.get(&path).copied().unwrap_or(false);
                    if state_a != state_b {
                        diffs.push(FieldDiff {
                            field_name: "button_state".to_string(),
                            value_a: if state_a { "ON" } else { "OFF" }.to_string(),
                            value_b: if state_b { "ON" } else { "OFF" }.to_string(),
                        });
                    }
                } else {
                    let val_a = preset_a.continuous_targets.get(&path).copied().unwrap_or(64);
                    let val_b = preset_b.continuous_targets.get(&path).copied().unwrap_or(64);
                    if val_a != val_b {
                        diffs.push(FieldDiff {
                            field_name: "target_value".to_string(),
                            value_a: val_a.to_string(),
                            value_b: val_b.to_string(),
                        });
                    }
                }

                if diffs.is_empty() {
                    identical_count += 1;
                } else {
                    modified_controls.push(ControlDiffEntry {
                        control_path: path,
                        field_diffs: diffs,
                    });
                }
            }
            (None, None) => {}
        }
    }

    PresetDiffReport {
        preset_a_id: preset_a.metadata.id.clone(),
        preset_a_name: preset_a.metadata.name.clone(),
        preset_b_id: preset_b.metadata.id.clone(),
        preset_b_name: preset_b.metadata.name.clone(),
        only_in_a,
        only_in_b,
        modified_controls,
        identical_count,
    }
}

fn format_msg_num(msg_type: MidiMessageType, num: u8) -> String {
    match msg_type {
        MidiMessageType::Cc => format!("CC {num}"),
        MidiMessageType::Note => format!("Note {num}"),
    }
}

fn compare_control_entries(a: &ControlMappingEntry, b: &ControlMappingEntry) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();

    if a.output_message_type != b.output_message_type || a.output_number != b.output_number {
        diffs.push(FieldDiff {
            field_name: "mapping".to_string(),
            value_a: format_msg_num(a.output_message_type, a.output_number),
            value_b: format_msg_num(b.output_message_type, b.output_number),
        });
    }
    if a.output_channel != b.output_channel {
        diffs.push(FieldDiff {
            field_name: "channel".to_string(),
            value_a: format!("Ch {}", a.output_channel),
            value_b: format!("Ch {}", b.output_channel),
        });
    }
    if !a.is_button && (a.min_value != b.min_value || a.max_value != b.max_value) {
        diffs.push(FieldDiff {
            field_name: "range".to_string(),
            value_a: format!("{}..{}", a.min_value, a.max_value),
            value_b: format!("{}..{}", b.min_value, b.max_value),
        });
    }
    if !a.is_button && a.curve != b.curve {
        diffs.push(FieldDiff {
            field_name: "curve".to_string(),
            value_a: format!("{:?}", a.curve),
            value_b: format!("{:?}", b.curve),
        });
    }
    if a.invert != b.invert {
        diffs.push(FieldDiff {
            field_name: "invert".to_string(),
            value_a: a.invert.to_string(),
            value_b: b.invert.to_string(),
        });
    }
    if a.is_button && a.behavior != b.behavior {
        diffs.push(FieldDiff {
            field_name: "behavior".to_string(),
            value_a: format!("{:?}", a.behavior),
            value_b: format!("{:?}", b.behavior),
        });
    }
    if a.is_button && (a.on_value != b.on_value || a.off_value != b.off_value) {
        diffs.push(FieldDiff {
            field_name: "on_off".to_string(),
            value_a: format!("{}/{}", a.on_value, a.off_value),
            value_b: format!("{}/{}", b.on_value, b.off_value),
        });
    }
    if a.label != b.label {
        diffs.push(FieldDiff {
            field_name: "label".to_string(),
            value_a: a.label.clone().unwrap_or_else(|| "-".to_string()),
            value_b: b.label.clone().unwrap_or_else(|| "-".to_string()),
        });
    }

    diffs
}

/// Selectively merges a chosen set of `selected_control_paths` from `preset_b` onto a copy of
/// `preset_a`, returning a new merged [`Preset`].
#[must_use]
pub fn merge_selected_controls(
    preset_a: &Preset,
    preset_b: &Preset,
    selected_control_paths: &BTreeSet<String>,
    merged_name: impl Into<String>,
) -> Preset {
    let mut merged = preset_a.clone();
    let name = merged_name.into();
    merged.metadata.id = slugify_preset_name(&name);
    merged.metadata.name = name;
    merged.metadata.group = "User".to_string();
    merged.metadata.is_factory = false;
    if !merged.metadata.tags.iter().any(|t| t == "merged") {
        merged.metadata.tags.push("merged".to_string());
    }

    for path in selected_control_paths {
        if let Some(entry_b) = preset_b.mapping.entries.get(path) {
            merged.mapping.entries.insert(path.clone(), entry_b.clone());
        }
        if let Some(&val_b) = preset_b.continuous_targets.get(path) {
            merged.continuous_targets.insert(path.clone(), val_b);
        }
        if let Some(&btn_b) = preset_b.button_states.get(path) {
            merged.button_states.insert(path.clone(), btn_b);
        }
    }

    merged
}

// =============================================================================
// 4. Preset Librarian State & Operations
// =============================================================================

/// High-level action emitted by the [`PresetLibrarian`] `egui` view when the user loads,
/// merges, or exports a preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibrarianAction {
    /// User activated a preset (by ID) to be loaded into the active `DeviceMappingPipeline`.
    LoadPreset(String),
    /// User created a new merged preset via the A/B Diff & Selective Merge panel.
    CreatedMergedPreset(String),
}

/// Complete state of the `nkEditor4` Preset Librarian, including all 14 factory presets,
/// user presets, 5 Quick Access slots, search/tag/group/favorite filters, and A/B Diff state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetLibrarian {
    /// All presets stored in the librarian (factory + user).
    pub presets: Vec<Preset>,
    /// Currently loaded preset ID.
    pub active_preset_id: String,
    /// 5 Quick Access preset slots (`[Option<preset_id>; 5]`).
    pub quick_access_slots: [Option<String>; QUICK_ACCESS_SLOT_COUNT],
    /// Case-insensitive search query string.
    pub search_query: String,
    /// Optional group filter (e.g. `Some("8opPerc")`, `Some("Nord Drum 2")`, or `None` for All).
    pub selected_group: Option<String>,
    /// Optional tag filter (e.g. `Some("synth")`, or `None` for All).
    pub selected_tag: Option<String>,
    /// Optional device ID filter (`Some("nanokontrol2")`, `Some("akai-midimix")`, or `None` for All).
    pub selected_device_id: Option<String>,
    /// When `true`, only starred favorite presets are shown.
    pub favorites_only: bool,
    /// Preset A ID for the A/B Diff & Merge view.
    pub diff_preset_a_id: String,
    /// Preset B ID for the A/B Diff & Merge view.
    pub diff_preset_b_id: String,
    /// Control paths currently checked for selective merge from Preset B into Preset A.
    pub selected_merge_paths: BTreeSet<String>,
    /// Last exported text or JSON buffer (for clipboard/inspection).
    pub last_exported_buffer: Option<String>,
}

impl Default for PresetLibrarian {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetLibrarian {
    /// Initializes the [`PresetLibrarian`] pre-populated with all 14 factory presets and
    /// 5 default Quick Access slots.
    #[must_use]
    pub fn new() -> Self {
        let presets = create_all_factory_presets();
        let active_preset_id = "factory-8opperc-nk2-live".to_string();
        let quick_access_slots = [
            Some("factory-8opperc-nk2-live".to_string()),
            Some("factory-8opperc-midimix-compat".to_string()),
            Some("factory-nord-drum-2-nk2".to_string()),
            Some("factory-synth-lead".to_string()),
            Some("factory-dual-controller-split-rig".to_string()),
        ];

        let mut lib = Self {
            presets,
            active_preset_id,
            quick_access_slots,
            search_query: String::new(),
            selected_group: None,
            selected_tag: None,
            selected_device_id: None,
            favorites_only: false,
            diff_preset_a_id: "factory-default".to_string(),
            diff_preset_b_id: "factory-8opperc-nk2-live".to_string(),
            selected_merge_paths: BTreeSet::new(),
            last_exported_buffer: None,
        };
        lib.select_all_diff_controls();
        lib
    }

    /// Looks up a preset by its ID (`"factory-default"`, etc.) or exact case-insensitive name.
    #[must_use]
    pub fn get_preset(&self, id_or_name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| {
            p.metadata.id == id_or_name || p.metadata.name.eq_ignore_ascii_case(id_or_name)
        })
    }

    /// Looks up a mutable preset by its ID or exact case-insensitive name.
    pub fn get_preset_mut(&mut self, id_or_name: &str) -> Option<&mut Preset> {
        self.presets.iter_mut().find(|p| {
            p.metadata.id == id_or_name || p.metadata.name.eq_ignore_ascii_case(id_or_name)
        })
    }

    /// Returns the currently active [`Preset`].
    #[must_use]
    pub fn active_preset(&self) -> Option<&Preset> {
        self.get_preset(&self.active_preset_id)
    }

    /// Activates a preset by ID or name, returning a reference to it if found.
    pub fn select_preset(&mut self, id_or_name: &str) -> Option<&Preset> {
        if let Some(id) = self
            .get_preset(id_or_name)
            .map(|p| p.metadata.id.clone())
        {
            self.active_preset_id = id;
            self.get_preset(&self.active_preset_id)
        } else {
            None
        }
    }

    /// Assigns a preset ID to one of the 5 Quick Access slots (`slot_index in 0..5`).
    pub fn assign_quick_slot(&mut self, slot_index: usize, preset_id: Option<String>) -> bool {
        if slot_index >= QUICK_ACCESS_SLOT_COUNT {
            return false;
        }
        if let Some(ref id) = preset_id {
            if self.get_preset(id).is_none() {
                return false;
            }
        }
        self.quick_access_slots[slot_index] = preset_id;
        true
    }

    /// Activates the preset stored in Quick Access slot `slot_index` (`0..5`).
    pub fn activate_quick_slot(&mut self, slot_index: usize) -> Option<&Preset> {
        let id = self.quick_access_slots.get(slot_index)?.clone()?;
        self.select_preset(&id)
    }

    /// Toggles the `favorite` star state of a preset by ID or name.
    pub fn toggle_favorite(&mut self, id_or_name: &str) -> Option<bool> {
        let preset = self.get_preset_mut(id_or_name)?;
        preset.metadata.favorite = !preset.metadata.favorite;
        Some(preset.metadata.favorite)
    }

    /// Returns all distinct preset group names sorted alphabetically.
    #[must_use]
    pub fn available_groups(&self) -> Vec<String> {
        let set: BTreeSet<String> = self
            .presets
            .iter()
            .map(|p| p.metadata.group.clone())
            .collect();
        set.into_iter().collect()
    }

    /// Returns all distinct preset tags sorted alphabetically.
    #[must_use]
    pub fn available_tags(&self) -> Vec<String> {
        let set: BTreeSet<String> = self
            .presets
            .iter()
            .flat_map(|p| p.metadata.tags.iter().cloned())
            .collect();
        set.into_iter().collect()
    }

    /// Returns all presets matching the current search query, group, tag, device, and favorite filters.
    #[must_use]
    pub fn filtered_presets(&self) -> Vec<&Preset> {
        let query = self.search_query.trim().to_ascii_lowercase();
        self.presets
            .iter()
            .filter(|p| {
                if self.favorites_only && !p.metadata.favorite {
                    return false;
                }
                if let Some(ref group) = self.selected_group {
                    if !p.metadata.group.eq_ignore_ascii_case(group) {
                        return false;
                    }
                }
                if let Some(ref tag) = self.selected_tag {
                    if !p
                        .metadata
                        .tags
                        .iter()
                        .any(|t| t.eq_ignore_ascii_case(tag))
                    {
                        return false;
                    }
                }
                if let Some(ref dev) = self.selected_device_id {
                    if !p.metadata.device_id.eq_ignore_ascii_case(dev) {
                        return false;
                    }
                }
                if !query.is_empty() {
                    let matches_name = p.metadata.name.to_ascii_lowercase().contains(&query);
                    let matches_desc = p
                        .metadata
                        .description
                        .to_ascii_lowercase()
                        .contains(&query);
                    let matches_group = p.metadata.group.to_ascii_lowercase().contains(&query);
                    let matches_tag = p
                        .metadata
                        .tags
                        .iter()
                        .any(|t| t.to_ascii_lowercase().contains(&query));
                    if !(matches_name || matches_desc || matches_group || matches_tag) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Duplicates an existing preset with a conflict-free `" copy"` name and adds it to the library.
    pub fn duplicate_preset(&mut self, source_id_or_name: &str) -> Option<Preset> {
        let source = self.get_preset(source_id_or_name)?.clone();
        let existing_names: Vec<String> = self
            .presets
            .iter()
            .map(|p| p.metadata.name.clone())
            .collect();
        let new_name = generate_duplicate_name(&source.metadata.name, &existing_names);
        let new_id = slugify_preset_name(&new_name);

        let mut dup = source;
        dup.metadata.id = new_id;
        dup.metadata.name = new_name;
        dup.metadata.group = "User".to_string();
        dup.metadata.is_factory = false;
        self.presets.push(dup.clone());
        Some(dup)
    }

    /// Deletes a user preset by ID (factory presets cannot be deleted and return `false`).
    pub fn delete_user_preset(&mut self, id_or_name: &str) -> bool {
        let Some(pos) = self.presets.iter().position(|p| {
            (p.metadata.id == id_or_name || p.metadata.name.eq_ignore_ascii_case(id_or_name))
                && !p.metadata.is_factory
        }) else {
            return false;
        };
        let removed = self.presets.remove(pos);
        for slot in &mut self.quick_access_slots {
            if slot.as_deref() == Some(removed.metadata.id.as_str()) {
                *slot = None;
            }
        }
        if self.active_preset_id == removed.metadata.id {
            self.active_preset_id = "factory-default".to_string();
        }
        true
    }

    /// Imports a preset from either JSON or legacy `mappings/default.txt` format and registers it.
    pub fn import_preset(
        &mut self,
        content: &str,
        fallback_name: impl Into<String>,
    ) -> Result<Preset, ConfigParseError> {
        let mut preset = import_preset_auto(content, fallback_name)?;
        let existing_names: Vec<String> = self
            .presets
            .iter()
            .map(|p| p.metadata.name.clone())
            .collect();
        if existing_names
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&preset.metadata.name))
        {
            let unique_name = generate_duplicate_name(&preset.metadata.name, &existing_names);
            preset.metadata.id = slugify_preset_name(&unique_name);
            preset.metadata.name = unique_name;
        }
        preset.metadata.is_factory = false;
        self.presets.push(preset.clone());
        Ok(preset)
    }

    /// Exports a preset by ID/name as pretty-printed JSON.
    pub fn export_preset_json(&mut self, id_or_name: &str) -> Result<String, ConfigParseError> {
        let preset = self.get_preset(id_or_name).ok_or_else(|| {
            ConfigParseError::document(format!("Preset not found: {id_or_name}"))
        })?;
        let json = format_preset_json(preset)?;
        self.last_exported_buffer = Some(json.clone());
        Ok(json)
    }

    /// Exports a preset's primary mapping in legacy `mappings/default.txt` format.
    pub fn export_preset_legacy_text(
        &mut self,
        id_or_name: &str,
    ) -> Result<String, ConfigParseError> {
        let preset = self.get_preset(id_or_name).ok_or_else(|| {
            ConfigParseError::document(format!("Preset not found: {id_or_name}"))
        })?;
        let txt = format_mapping_to_text(&preset.mapping);
        self.last_exported_buffer = Some(txt.clone());
        Ok(txt)
    }

    /// Computes the live [`PresetDiffReport`] for the currently selected Preset A and Preset B.
    #[must_use]
    pub fn current_diff_report(&self) -> Option<PresetDiffReport> {
        let a = self.get_preset(&self.diff_preset_a_id)?;
        let b = self.get_preset(&self.diff_preset_b_id)?;
        Some(diff_presets(a, b))
    }

    /// Populates `selected_merge_paths` with all modified/added controls from the current diff.
    pub fn select_all_diff_controls(&mut self) {
        self.selected_merge_paths.clear();
        if let Some(report) = self.current_diff_report() {
            for item in report.modified_controls {
                self.selected_merge_paths.insert(item.control_path);
            }
            for path in report.only_in_b {
                self.selected_merge_paths.insert(path);
            }
        }
    }

    /// Executes a selective merge of `selected_merge_paths` from `diff_preset_b_id` onto
    /// `diff_preset_a_id`, saving the result as a new user preset in the library.
    pub fn execute_selective_merge(&mut self, new_preset_name: Option<String>) -> Option<Preset> {
        let a = self.get_preset(&self.diff_preset_a_id)?.clone();
        let b = self.get_preset(&self.diff_preset_b_id)?.clone();
        let default_name = new_preset_name
            .unwrap_or_else(|| format!("{} + {} Merge", a.metadata.name, b.metadata.name));
        let existing_names: Vec<String> = self
            .presets
            .iter()
            .map(|p| p.metadata.name.clone())
            .collect();
        let final_name = if existing_names
            .iter()
            .any(|n| n.eq_ignore_ascii_case(&default_name))
        {
            generate_duplicate_name(&default_name, &existing_names)
        } else {
            default_name
        };

        let merged = merge_selected_controls(&a, &b, &self.selected_merge_paths, final_name);
        self.active_preset_id = merged.metadata.id.clone();
        self.presets.push(merged.clone());
        Some(merged)
    }
}

// =============================================================================
// 5. Interactive egui Preset Librarian & A/B Diff/Merge View
// =============================================================================

/// Renders the complete Preset Librarian & A/B Diff/Merge panel in `egui`.
pub fn render_preset_librarian_panel(
    ui: &mut Ui,
    librarian: &mut PresetLibrarian,
    theme_settings: &ThemeSettings,
) -> Option<LibrarianAction> {
    let palette = theme_settings.resolved_palette();
    let panel_fill = theme_settings.build_visuals().panel_fill;
    let mut emitted_action: Option<LibrarianAction> = None;

    egui::Frame::none()
        .fill(panel_fill)
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // 1. Header + 5 Quick Access Slots
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("PRESET LIBRARIAN")
                        .strong()
                        .color(palette.accent),
                );
                ui.separator();
                ui.label(RichText::new("QUICK SLOTS:").small().weak());

                let slot_ids = librarian.quick_access_slots.clone();
                for (slot_idx, slot_opt) in slot_ids.iter().enumerate() {
                    let label = if let Some(ref pid) = slot_opt {
                        let name = librarian
                            .get_preset(pid)
                            .map(|p| p.metadata.name.as_str())
                            .unwrap_or(pid.as_str());
                        format!("[{}] {}", slot_idx + 1, name)
                    } else {
                        format!("[{}] <Empty>", slot_idx + 1)
                    };
                    let is_active = slot_opt.as_deref() == Some(librarian.active_preset_id.as_str());
                    let stroke = if is_active {
                        Stroke::new(1.5, palette.accent)
                    } else {
                        Stroke::new(1.0, Color32::from_gray(75))
                    };
                    if ui
                        .add(egui::Button::new(RichText::new(label).small()).stroke(stroke))
                        .clicked()
                    {
                        if let Some(loaded) = librarian.activate_quick_slot(slot_idx) {
                            emitted_action =
                                Some(LibrarianAction::LoadPreset(loaded.metadata.id.clone()));
                        }
                    }
                }
            });

            ui.add_space(6.0);

            // 2. Search, Group, Tag, Device & Favorites Filter Bar
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Search:").small());
                ui.add(
                    egui::TextEdit::singleline(&mut librarian.search_query)
                        .desired_width(150.0)
                        .hint_text("Name, tag, desc..."),
                );

                if ui
                    .selectable_label(librarian.favorites_only, "★ Favorites")
                    .clicked()
                {
                    librarian.favorites_only = !librarian.favorites_only;
                }

                ui.separator();
                ui.label(RichText::new("Group:").small());
                let groups = librarian.available_groups();
                if ui
                    .selectable_label(librarian.selected_group.is_none(), "All")
                    .clicked()
                {
                    librarian.selected_group = None;
                }
                for group in groups {
                    let selected = librarian.selected_group.as_deref() == Some(group.as_str());
                    if ui.selectable_label(selected, &group).clicked() {
                        librarian.selected_group = if selected { None } else { Some(group) };
                    }
                }
            });

            ui.add_space(6.0);

            // 3. Preset Browser List
            let filtered_ids: Vec<(String, String, String, String, bool, bool)> = librarian
                .filtered_presets()
                .into_iter()
                .map(|p| {
                    (
                        p.metadata.id.clone(),
                        p.metadata.name.clone(),
                        p.metadata.group.clone(),
                        p.metadata.device_id.clone(),
                        p.metadata.favorite,
                        p.metadata.is_factory,
                    )
                })
                .collect();

            ScrollArea::vertical()
                .id_salt("librarian_preset_scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    for (id, name, group, dev_id, fav, is_factory) in filtered_ids {
                        let is_active = librarian.active_preset_id == id;
                        ui.horizontal(|ui| {
                            let star = if fav { "★" } else { "☆" };
                            if ui.small_button(star).clicked() {
                                let _ = librarian.toggle_favorite(&id);
                            }
                            if ui
                                .selectable_label(
                                    is_active,
                                    format!("{name}  [{group} / {dev_id}]"),
                                )
                                .clicked()
                            {
                                librarian.select_preset(&id);
                                emitted_action = Some(LibrarianAction::LoadPreset(id.clone()));
                            }
                            if ui.small_button("Set A").clicked() {
                                librarian.diff_preset_a_id = id.clone();
                                librarian.select_all_diff_controls();
                            }
                            if ui.small_button("Set B").clicked() {
                                librarian.diff_preset_b_id = id.clone();
                                librarian.select_all_diff_controls();
                            }
                            if ui.small_button("Dup").clicked() {
                                let _ = librarian.duplicate_preset(&id);
                            }
                            if !is_factory && ui.small_button("Del").clicked() {
                                let _ = librarian.delete_user_preset(&id);
                            }
                        });
                    }
                });

            ui.separator();

            // 4. A/B Preset Diff & Selective Merge View
            if let Some(report) = librarian.current_diff_report() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "A/B PRESET DIFF: [A] {}  vs  [B] {}  ({} modified, {} identical)",
                            report.preset_a_name,
                            report.preset_b_name,
                            report.modified_controls.len(),
                            report.identical_count
                        ))
                        .strong()
                        .color(palette.accent_strong),
                    );

                    if ui.small_button("Select All Diffs").clicked() {
                        librarian.select_all_diff_controls();
                    }
                    if ui.small_button("Clear Selection").clicked() {
                        librarian.selected_merge_paths.clear();
                    }
                    if ui
                        .button(
                            RichText::new(format!(
                                "Merge Selected ({}) -> New Preset",
                                librarian.selected_merge_paths.len()
                            ))
                            .strong(),
                        )
                        .clicked()
                    {
                        if let Some(merged) = librarian.execute_selective_merge(None) {
                            emitted_action =
                                Some(LibrarianAction::CreatedMergedPreset(merged.metadata.id));
                        }
                    }
                });

                ScrollArea::vertical()
                    .id_salt("librarian_ab_diff_scroll")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for diff_entry in &report.modified_controls {
                            let mut checked = librarian
                                .selected_merge_paths
                                .contains(&diff_entry.control_path);
                            let summary: Vec<String> = diff_entry
                                .field_diffs
                                .iter()
                                .map(|fd| {
                                    format!("{}: {} -> {}", fd.field_name, fd.value_a, fd.value_b)
                                })
                                .collect();
                            let row_text =
                                format!("{:<18}  {}", diff_entry.control_path, summary.join(" | "));
                            if ui.checkbox(&mut checked, RichText::new(row_text).monospace().small()).changed()
                            {
                                if checked {
                                    librarian
                                        .selected_merge_paths
                                        .insert(diff_entry.control_path.clone());
                                } else {
                                    librarian
                                        .selected_merge_paths
                                        .remove(&diff_entry.control_path);
                                }
                            }
                        }
                    });
            }
        });

    emitted_action
}

// =============================================================================
// 6. Comprehensive Unit & Headless egui Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_parser::{parse_legacy_mapping_text, parse_preset_json};

    #[test]
    fn all_14_factory_presets_are_present_and_valid() {
        let presets = create_all_factory_presets();
        assert_eq!(presets.len(), 14);

        let expected_names = [
            "Default",
            "Synth Lead",
            "Drums",
            "DAW Transport",
            "DJ Mixer",
            "Nord Drum 2",
            "Default MidiMix",
            "MidiMix Synth",
            "MidiMix DJ",
            "MidiMix Nord Drum 2",
            "8opPerc nanoKONTROL2 Live Rig",
            "8opPerc MIDImix Compatibility",
            "Konduktiva 8-Lane Macro Surface",
            "Dual-Controller Split Rig",
        ];

        for expected in expected_names {
            let found = presets
                .iter()
                .find(|p| p.metadata.name == expected)
                .unwrap_or_else(|| panic!("Missing factory preset: {expected}"));
            assert!(found.metadata.is_factory);
            assert!(!found.mapping.entries.is_empty());
        }

        // Verify Dual-Controller Split Rig carries both nanoKONTROL2 and Akai MIDImix mappings
        let split_rig = presets
            .iter()
            .find(|p| p.metadata.name == "Dual-Controller Split Rig")
            .expect("split rig");
        assert_eq!(split_rig.mapping.device_id, "nanokontrol2");
        let sec = split_rig
            .secondary_mapping
            .as_ref()
            .expect("secondary mapping present");
        assert_eq!(sec.device_id, "akai-midimix");
    }

    #[test]
    fn parses_actual_mappings_default_txt_and_roundtrips_legacy_text_and_json() {
        let repo_default_txt = include_str!("../mappings/default.txt");
        let parsed = parse_legacy_mapping_text(repo_default_txt)
            .expect("mappings/default.txt should parse cleanly");
        assert_eq!(parsed.device_id, "nanokontrol2");
        assert_eq!(parsed.entries.len(), 51);

        // Verify specific custom CCs in mappings/default.txt (e.g. track1.knob 16 -> 74, track5.solo 36 -> 100)
        let t1_knob = parsed.entries.get("track1.knob").expect("track1.knob");
        assert_eq!(t1_knob.input_number, 16);
        assert_eq!(t1_knob.output_number, 74);
        assert_eq!(t1_knob.label.as_deref(), Some("Track 1 Knob"));

        let t5_solo = parsed.entries.get("track5.solo").expect("track5.solo");
        assert_eq!(t5_solo.input_number, 36);
        assert_eq!(t5_solo.output_number, 100);
        assert_eq!(t5_solo.behavior, ButtonBehavior::Toggle);

        // Format back to text and re-parse losslessly
        let formatted_txt = format_mapping_to_text(&parsed);
        let reparsed =
            parse_legacy_mapping_text(&formatted_txt).expect("formatted text should re-parse");
        assert_eq!(parsed, reparsed);

        // Also test Akai MIDImix legacy text with [device akai-midimix] and note:N syntax
        let midimix_preset = factory_preset_midimix_nord_drum_2();
        let midimix_txt = format_mapping_to_text(&midimix_preset.mapping);
        assert!(midimix_txt.contains("[device akai-midimix]"));
        assert!(midimix_txt.contains("note:"));
        let reparsed_midimix =
            parse_legacy_mapping_text(&midimix_txt).expect("midimix text should re-parse");
        assert_eq!(midimix_preset.mapping, reparsed_midimix);

        // JSON Preset round-trip
        let json = format_preset_json(&midimix_preset).expect("serialize preset json");
        let from_json = parse_preset_json(&json).expect("deserialize preset json");
        assert_eq!(midimix_preset, from_json);
    }

    #[test]
    fn librarian_quick_slots_search_tags_favorites_and_duplicate_names_work() {
        let mut lib = PresetLibrarian::new();
        assert_eq!(lib.presets.len(), 14);

        // Quick access slots
        let slot0 = lib.activate_quick_slot(0).expect("slot 0 active");
        assert_eq!(slot0.metadata.name, "8opPerc nanoKONTROL2 Live Rig");
        assert!(lib.assign_quick_slot(4, Some("factory-Nord-Drum-2-nk2".to_lowercase())));

        // Search filter
        lib.search_query = "nord".to_string();
        let nord_results = lib.filtered_presets();
        assert_eq!(nord_results.len(), 3); // Nord Drum 2, MidiMix Nord Drum 2, Dual-Controller Split Rig

        // Group filter
        lib.search_query.clear();
        lib.selected_group = Some("8opPerc".to_string());
        assert_eq!(lib.filtered_presets().len(), 4);
        lib.selected_group = None;

        // Favorites filter
        lib.toggle_favorite("Synth Lead");
        lib.favorites_only = true;
        let favs = lib.filtered_presets();
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].metadata.name, "Synth Lead");
        lib.favorites_only = false;

        // Duplicate preset naming ("Synth Lead copy", "Synth Lead copy 2")
        let dup1 = lib.duplicate_preset("Synth Lead").expect("dup1");
        assert_eq!(dup1.metadata.name, "Synth Lead copy");
        let dup2 = lib.duplicate_preset("Synth Lead").expect("dup2");
        assert_eq!(dup2.metadata.name, "Synth Lead copy 2");

        // Factory presets cannot be deleted, user duplicates can
        assert!(!lib.delete_user_preset("Synth Lead"));
        assert!(lib.delete_user_preset("Synth Lead copy"));
    }

    #[test]
    fn ab_preset_diff_and_selective_merge_across_mappings_channels_ranges_and_values() {
        let mut lib = PresetLibrarian::new();
        lib.diff_preset_a_id = "factory-default".to_string();
        lib.diff_preset_b_id = "factory-nord-drum-2-nk2".to_string();

        let report = lib.current_diff_report().expect("diff report");
        assert!(!report.is_identical());
        assert!(report.modified_controls.len() >= 24);

        // Verify track1.knob shows mapping (CC 16 -> CC 30) and label diff
        let t1_knob_diff = report
            .modified_controls
            .iter()
            .find(|c| c.control_path == "track1.knob")
            .expect("track1.knob diff");
        assert!(t1_knob_diff
            .field_diffs
            .iter()
            .any(|f| f.field_name == "mapping" && f.value_a == "CC 16" && f.value_b == "CC 30"));

        // Verify track2.knob shows channel diff (Ch 1 -> Ch 2)
        let t2_knob_diff = report
            .modified_controls
            .iter()
            .find(|c| c.control_path == "track2.knob")
            .expect("track2.knob diff");
        assert!(t2_knob_diff
            .field_diffs
            .iter()
            .any(|f| f.field_name == "channel" && f.value_a == "Ch 1" && f.value_b == "Ch 2"));

        // Selectively merge ONLY track1.knob and track7.knob from Nord Drum 2 into Default
        lib.selected_merge_paths.clear();
        lib.selected_merge_paths.insert("track1.knob".to_string());
        lib.selected_merge_paths.insert("track7.knob".to_string());

        let merged = lib
            .execute_selective_merge(Some("Custom Hybrid Rig".to_string()))
            .expect("merged preset");
        assert_eq!(merged.metadata.name, "Custom Hybrid Rig");
        assert_eq!(
            merged.mapping.entries["track1.knob"].output_number,
            nord_drum_2::tone::SPECTRA
        );
        assert_eq!(
            merged.mapping.entries["track7.knob"].output_channel,
            nord_drum_2::channels::GLOBAL_FX
        );
        // Unselected track2.knob remains Default (CC 17 on Ch 1)
        assert_eq!(merged.mapping.entries["track2.knob"].output_number, 17);
        assert_eq!(merged.mapping.entries["track2.knob"].output_channel, 1);
    }

    #[test]
    fn headless_egui_librarian_panel_renders_without_panic() {
        let ctx = egui::Context::default();
        let mut lib = PresetLibrarian::new();
        let theme = ThemeSettings::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_preset_librarian_panel(ui, &mut lib, &theme);
            });
        });
    }
}
