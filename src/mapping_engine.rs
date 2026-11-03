//! Refactored Multi-Device MIDI Remapping Engine (`src/mapping_engine.rs`).
//!
//! Provides simultaneous per-device MIDI control remapping pipelines supporting:
//! - **4 Cross-Message Transformations**: [`MessageTransformation::CcToCc`],
//!   [`MessageTransformation::NoteToCc`], [`MessageTransformation::CcToNote`], and
//!   [`MessageTransformation::NoteToNote`].
//! - **Per-Control Output MIDI Channel (`1..=16`)** and optional input channel filtering.
//! - **Min/Max Output Range Scaling** (`min_value..=max_value`, clamped to `0..=127`).
//! - **Polarity Inversion** (`invert: true` flips continuous `0..=127` to `127..=0` and
//!   swaps button `on_value` / `off_value`).
//! - **4 Continuous Response Curves**: [`ResponseCurve::Linear`],
//!   [`ResponseCurve::Exponential`], [`ResponseCurve::Logarithmic`], and
//!   [`ResponseCurve::SCurve`].
//! - **Toggle vs. Momentary Button State Machines** with customizable `on_value` and
//!   `off_value` payloads.
//! - **3 Soft Takeover Modes** ([`SoftTakeoverMode::Catch`], [`SoftTakeoverMode::Jump`],
//!   [`SoftTakeoverMode::Pickup`]) with real-time physical-vs-target delta calculation
//!   ([`SoftTakeoverStatus`]) so physical knob/slider jumps after a preset change never
//!   cause sudden audible parameter spikes.

use crate::devices::{
    akai_midimix_definition, nanokontrol2_definition, ButtonBehavior, ControlDef, DeviceDefinition,
    MidiMessageType, MIDIMIX_FADER_CCS, MIDIMIX_KNOB_ROW1_CCS, MIDIMIX_KNOB_ROW2_CCS,
    MIDIMIX_MUTE_NOTES, MIDIMIX_REC_ARM_NOTES, MIDIMIX_SOLO_NOTES,
};
use crate::midi_io::RawMidiPacket;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Default MIDI value emitted when a button enters the ON state.
pub const MIDI_VALUE_ON: u8 = 127;

/// Default MIDI value emitted when a button enters the OFF state.
pub const MIDI_VALUE_OFF: u8 = 0;

/// Default threshold (`>= 64`) used to distinguish a physical button press from a release.
pub const BUTTON_PRESS_THRESHOLD: u8 = 64;

/// Default Soft Takeover catch/pickup threshold in MIDI units (`3` out of `127`).
pub const DEFAULT_SOFT_TAKEOVER_THRESHOLD: u8 = 3;

// =============================================================================
// 1. Cross-Message Transformation & Composite Key Types
// =============================================================================

/// Describes the input-to-output MIDI message transformation for a control mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageTransformation {
    /// Control Change input mapped to Control Change output (`CC -> CC`).
    CcToCc,
    /// Note On/Off input mapped to Control Change output (`Note -> CC`).
    NoteToCc,
    /// Control Change input mapped to Note On/Off output (`CC -> Note`).
    CcToNote,
    /// Note On/Off input mapped to Note On/Off output (`Note -> Note`).
    NoteToNote,
}

impl MessageTransformation {
    /// Derives the transformation variant from input and output [`MidiMessageType`]s.
    #[must_use]
    pub const fn from_types(input: MidiMessageType, output: MidiMessageType) -> Self {
        match (input, output) {
            (MidiMessageType::Cc, MidiMessageType::Cc) => Self::CcToCc,
            (MidiMessageType::Note, MidiMessageType::Cc) => Self::NoteToCc,
            (MidiMessageType::Cc, MidiMessageType::Note) => Self::CcToNote,
            (MidiMessageType::Note, MidiMessageType::Note) => Self::NoteToNote,
        }
    }

    /// Returns the expected input [`MidiMessageType`].
    #[must_use]
    pub const fn input_type(self) -> MidiMessageType {
        match self {
            Self::CcToCc | Self::CcToNote => MidiMessageType::Cc,
            Self::NoteToCc | Self::NoteToNote => MidiMessageType::Note,
        }
    }

    /// Returns the produced output [`MidiMessageType`].
    #[must_use]
    pub const fn output_type(self) -> MidiMessageType {
        match self {
            Self::CcToCc | Self::NoteToCc => MidiMessageType::Cc,
            Self::CcToNote | Self::NoteToNote => MidiMessageType::Note,
        }
    }
}

impl fmt::Display for MessageTransformation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CcToCc => write!(f, "CC -> CC"),
            Self::NoteToCc => write!(f, "Note -> CC"),
            Self::CcToNote => write!(f, "CC -> Note"),
            Self::NoteToNote => write!(f, "Note -> Note"),
        }
    }
}

/// Composite lookup key `(message_type, midi_number)` that prevents collisions between
/// CC numbers and Note numbers sharing the same `0..=127` index (e.g. `cc:25` vs `note:25`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CompositeMidiKey {
    pub message_type: MidiMessageType,
    pub midi_number: u8,
}

impl CompositeMidiKey {
    /// Creates a new composite lookup key.
    #[must_use]
    pub const fn new(message_type: MidiMessageType, midi_number: u8) -> Self {
        Self {
            message_type,
            midi_number: midi_number & 0x7F,
        }
    }

    /// Formats the key as `"cc:42"` or `"note:25"` (compatible with legacy `makeLookupKey`).
    #[must_use]
    pub fn as_string(self) -> String {
        let prefix = match self.message_type {
            MidiMessageType::Cc => "cc",
            MidiMessageType::Note => "note",
        };
        format!("{prefix}:{}", self.midi_number)
    }
}

impl fmt::Display for CompositeMidiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

// =============================================================================
// 2. Continuous Response Curves, Scaling, and Polarity Inversion
// =============================================================================

/// Continuous transfer function applied between normalized input `t in [0.0, 1.0]`
/// and output range scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResponseCurve {
    /// $f(t) = t$ — Uniform 1:1 response across the full travel.
    #[default]
    Linear,
    /// $f(t) = t^2$ — Fine resolution at low values, steeper acceleration toward high values.
    Exponential,
    /// $f(t) = \frac{\ln(1 + 9t)}{\ln(10)}$ — Fast initial rise, fine resolution near maximum.
    Logarithmic,
    /// $f(t) = 3t^2 - 2t^3$ — Smooth Hermite S-curve with gentle extremes and steepest midpoint (`f(0.5) = 0.5`).
    SCurve,
}

impl ResponseCurve {
    /// Evaluates the normalized response curve for `t` clamped to `[0.0, 1.0]`.
    #[must_use]
    pub fn apply(self, t: f64) -> f64 {
        let x = t.clamp(0.0, 1.0);
        let y = match self {
            Self::Linear => x,
            Self::Exponential => x * x,
            Self::Logarithmic => (1.0 + 9.0 * x).ln() / 10.0_f64.ln(),
            Self::SCurve => x * x * (3.0 - 2.0 * x),
        };
        y.clamp(0.0, 1.0)
    }

    /// Transforms a 7-bit MIDI input value (`0..=127`) through optional polarity inversion,
    /// this [`ResponseCurve`], and `[min_value, max_value]` output range scaling.
    #[must_use]
    pub fn transform_value(
        self,
        raw_value: u8,
        min_value: u8,
        max_value: u8,
        invert: bool,
    ) -> u8 {
        let clamped_in = raw_value.min(127);
        let mut normalized = f64::from(clamped_in) / 127.0;
        if invert {
            normalized = 1.0 - normalized;
        }
        let curved = self.apply(normalized);
        let low = f64::from(min_value.min(max_value).min(127));
        let high = f64::from(min_value.max(max_value).min(127));
        let scaled = low + curved * (high - low);
        scaled.round().clamp(0.0, 127.0) as u8
    }
}

// =============================================================================
// 3. Soft Takeover Modes & Real-Time Physical-vs-Target Delta Engine
// =============================================================================

/// Soft Takeover algorithm preventing abrupt parameter jumps when physical knob/slider
/// positions differ from software/preset target values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SoftTakeoverMode {
    /// Control remains locked until the physical value enters `[target - threshold, target + threshold]`.
    #[default]
    Catch,
    /// No soft takeover: physical movement immediately updates the output to the physical value.
    Jump,
    /// Control unlocks when the physical value enters `[target - threshold, target + threshold]`
    /// OR when consecutive physical movements cross over the target value (`prev < target <= curr`
    /// or `prev > target >= curr`), preventing missed catches during fast fader sweeps.
    Pickup,
}

/// Direction the user needs to move the physical knob or slider to reach the software target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeoverDirection {
    /// Physical position matches the target (or control is actively engaged).
    Matched,
    /// Physical position is below the target (`physical < target`); move knob/fader up.
    MoveUp,
    /// Physical position is above the target (`physical > target`); move knob/fader down.
    MoveDown,
}

/// Real-time Soft Takeover telemetry computed on every physical movement or target update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftTakeoverStatus {
    /// `true` if the physical control has caught/picked up the target and is actively transmitting.
    pub engaged: bool,
    /// Active soft takeover mode (`Catch`, `Jump`, or `Pickup`).
    pub mode: SoftTakeoverMode,
    /// Catch/pickup window radius in MIDI units (`1..=20`, default `3`).
    pub threshold: u8,
    /// Most recent physical hardware value (`0..=127`).
    pub physical_value: u8,
    /// Current software/preset target value (`0..=127`).
    pub target_value: u8,
    /// Signed delta `physical_value - target_value` (`-127..=127`).
    pub delta: i16,
    /// Absolute distance `|physical_value - target_value|` (`0..=127`).
    pub abs_delta: u8,
    /// Direction the user must move the physical control to reach `target_value`.
    pub direction: TakeoverDirection,
}

/// Per-control Soft Takeover state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftTakeoverTracker {
    /// Current target value in raw 7-bit control space (`0..=127`).
    pub target_value: u8,
    /// Last observed physical value from hardware (`None` until first hardware event).
    pub last_physical_value: Option<u8>,
    /// Whether the physical control is currently unlocked/engaged.
    pub engaged: bool,
}

impl Default for SoftTakeoverTracker {
    fn default() -> Self {
        Self {
            target_value: 0,
            last_physical_value: None,
            engaged: true,
        }
    }
}

impl SoftTakeoverTracker {
    /// Creates a new tracker with an initial target value and engagement state.
    #[must_use]
    pub const fn new(target_value: u8, engaged: bool) -> Self {
        Self {
            target_value: if target_value > 127 { 127 } else { target_value },
            last_physical_value: None,
            engaged,
        }
    }

    /// Sets a new software/preset target value (e.g., when loading a preset or receiving an
    /// external OSC/GUI parameter change).
    ///
    /// In `Catch` and `Pickup` modes, this locks the control (`engaged = false`) unless the
    /// last known physical position is already within `threshold` of `new_target`.
    pub fn set_target(&mut self, new_target: u8, mode: SoftTakeoverMode, threshold: u8) {
        let clamped_target = new_target.min(127);
        self.target_value = clamped_target;
        match mode {
            SoftTakeoverMode::Jump => {
                self.engaged = true;
            }
            SoftTakeoverMode::Catch | SoftTakeoverMode::Pickup => {
                if let Some(phys) = self.last_physical_value {
                    self.engaged = phys.abs_diff(clamped_target) <= threshold;
                } else {
                    self.engaged = false;
                }
            }
        }
    }

    /// Evaluates a new physical hardware reading (`0..=127`) against the current target
    /// and updates engagement state, target tracking, and real-time delta metrics.
    pub fn evaluate(
        &mut self,
        physical_value: u8,
        mode: SoftTakeoverMode,
        threshold: u8,
    ) -> SoftTakeoverStatus {
        let phys = physical_value.min(127);
        let prev_phys = self.last_physical_value;

        match mode {
            SoftTakeoverMode::Jump => {
                self.engaged = true;
            }
            SoftTakeoverMode::Catch => {
                if !self.engaged && phys.abs_diff(self.target_value) <= threshold {
                    self.engaged = true;
                }
            }
            SoftTakeoverMode::Pickup => {
                if !self.engaged {
                    let within_threshold = phys.abs_diff(self.target_value) <= threshold;
                    let crossed_target = if let Some(prev) = prev_phys {
                        (prev < self.target_value && phys >= self.target_value)
                            || (prev > self.target_value && phys <= self.target_value)
                    } else {
                        false
                    };
                    if within_threshold || crossed_target {
                        self.engaged = true;
                    }
                }
            }
        }

        if self.engaged {
            self.target_value = phys;
        }
        self.last_physical_value = Some(phys);

        self.current_status(mode, threshold)
    }

    /// Computes the current [`SoftTakeoverStatus`] without mutating state.
    #[must_use]
    pub fn current_status(&self, mode: SoftTakeoverMode, threshold: u8) -> SoftTakeoverStatus {
        let phys = self.last_physical_value.unwrap_or(self.target_value);
        let delta = i16::from(phys) - i16::from(self.target_value);
        let abs_delta = phys.abs_diff(self.target_value);
        let direction = if self.engaged || delta == 0 {
            TakeoverDirection::Matched
        } else if delta < 0 {
            TakeoverDirection::MoveUp
        } else {
            TakeoverDirection::MoveDown
        };

        SoftTakeoverStatus {
            engaged: self.engaged,
            mode,
            threshold,
            physical_value: phys,
            target_value: self.target_value,
            delta,
            abs_delta,
            direction,
        }
    }
}

// =============================================================================
// 4. Per-Control Mapping Entry & Device Mapping Configuration
// =============================================================================

/// Complete remapping specification for a single physical control (continuous or button).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlMappingEntry {
    /// Dot-notated control path (e.g. `"track1.knob"`, `"track1.mute"`, `"transport.play"`).
    pub control_path: String,
    /// `true` for discrete buttons, `false` for continuous knobs/sliders/faders.
    pub is_button: bool,
    /// Incoming hardware MIDI CC or Note number (`0..=127`).
    pub input_number: u8,
    /// Outgoing remapped MIDI CC or Note number (`0..=127`).
    pub output_number: u8,
    /// Optional 1-indexed input MIDI channel filter (`Some(1..=16)` or `None` for omni).
    pub input_channel: Option<u8>,
    /// 1-indexed output MIDI channel (`1..=16`).
    pub output_channel: u8,
    /// Incoming MIDI message type (`Cc` or `Note`).
    pub input_message_type: MidiMessageType,
    /// Outgoing MIDI message type (`Cc` or `Note`), enabling `CC<->CC`, `Note<->CC`, `CC<->Note`, and `Note<->Note`.
    pub output_message_type: MidiMessageType,
    /// Button interaction mode (`Toggle` or `Momentary`), used when `is_button == true`.
    pub behavior: ButtonBehavior,
    /// Minimum scaled output value (`0..=127`, default `0`) for continuous controls.
    pub min_value: u8,
    /// Maximum scaled output value (`0..=127`, default `127`) for continuous controls.
    pub max_value: u8,
    /// Output MIDI value sent when a button transitions to ON (`0..=127`, default `127`).
    pub on_value: u8,
    /// Output MIDI value sent when a button transitions to OFF (`0..=127`, default `0`).
    pub off_value: u8,
    /// Polarity inversion (`true` inverts continuous `0..=127` travel or swaps button `on_value`/`off_value`).
    pub invert: bool,
    /// Continuous response curve (`Linear`, `Exponential`, `Logarithmic`, `SCurve`).
    pub curve: ResponseCurve,
    /// Optional user-defined label (e.g. `"Kick Density"`, `"Form Next"`).
    pub label: Option<String>,
}

impl ControlMappingEntry {
    /// Creates a standard continuous control mapping entry (`CC -> CC` by default).
    #[must_use]
    pub fn new_continuous(
        control_path: impl Into<String>,
        input_number: u8,
        output_number: u8,
        output_channel: u8,
    ) -> Self {
        Self {
            control_path: control_path.into(),
            is_button: false,
            input_number: input_number & 0x7F,
            output_number: output_number & 0x7F,
            input_channel: None,
            output_channel: output_channel.clamp(1, 16),
            input_message_type: MidiMessageType::Cc,
            output_message_type: MidiMessageType::Cc,
            behavior: ButtonBehavior::Momentary,
            min_value: 0,
            max_value: 127,
            on_value: MIDI_VALUE_ON,
            off_value: MIDI_VALUE_OFF,
            invert: false,
            curve: ResponseCurve::Linear,
            label: None,
        }
    }

    /// Creates a standard button control mapping entry.
    #[must_use]
    pub fn new_button(
        control_path: impl Into<String>,
        input_message_type: MidiMessageType,
        input_number: u8,
        output_message_type: MidiMessageType,
        output_number: u8,
        output_channel: u8,
        behavior: ButtonBehavior,
    ) -> Self {
        Self {
            control_path: control_path.into(),
            is_button: true,
            input_number: input_number & 0x7F,
            output_number: output_number & 0x7F,
            input_channel: None,
            output_channel: output_channel.clamp(1, 16),
            input_message_type,
            output_message_type,
            behavior,
            min_value: 0,
            max_value: 127,
            on_value: MIDI_VALUE_ON,
            off_value: MIDI_VALUE_OFF,
            invert: false,
            curve: ResponseCurve::Linear,
            label: None,
        }
    }

    /// Returns the [`MessageTransformation`] kind (`CcToCc`, `NoteToCc`, `CcToNote`, `NoteToNote`).
    #[must_use]
    pub const fn transformation(&self) -> MessageTransformation {
        MessageTransformation::from_types(self.input_message_type, self.output_message_type)
    }

    /// Returns the composite lookup key `(input_message_type, input_number)`.
    #[must_use]
    pub const fn lookup_key(&self) -> CompositeMidiKey {
        CompositeMidiKey::new(self.input_message_type, self.input_number)
    }

    /// Computes the scaled, curve-shaped, and polarity-adjusted output value for a continuous control.
    #[must_use]
    pub fn compute_continuous_output(&self, raw_value: u8) -> u8 {
        self.curve
            .transform_value(raw_value, self.min_value, self.max_value, self.invert)
    }

    /// Computes the output value for a button given its active (`true` = ON, `false` = OFF) state,
    /// respecting customizable `on_value` / `off_value` and `invert`.
    #[must_use]
    pub fn compute_button_output(&self, is_active: bool) -> u8 {
        let effective_active = if self.invert { !is_active } else { is_active };
        if effective_active {
            self.on_value.min(127)
        } else {
            self.off_value.min(127)
        }
    }
}

/// Complete mapping configuration for a device instance, keyed by dot-notated `control_path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceMappingConfig {
    /// Target device definition identifier (e.g. `"nanokontrol2"`, `"akai-midimix"`).
    pub device_id: String,
    /// Mapping entries keyed by `control_path` (e.g. `"track1.knob"`, `"transport.play"`).
    pub entries: BTreeMap<String, ControlMappingEntry>,
}

impl DeviceMappingConfig {
    /// Builds a default 1:1 identity mapping configuration from any [`DeviceDefinition`].
    #[must_use]
    pub fn from_device_definition(def: &DeviceDefinition) -> Self {
        let mut entries = BTreeMap::new();

        for (idx, layout) in def.track_layouts.iter().enumerate() {
            let track_num = idx + 1;
            for c in &layout.continuous_controls {
                let path = format!("track{track_num}.{}", c.name);
                let mut entry = ControlMappingEntry::new_continuous(
                    path.clone(),
                    c.midi_number,
                    c.midi_number,
                    c.channel,
                );
                entry.input_message_type = c.message_type;
                entry.output_message_type = c.message_type;
                entries.insert(path, entry);
            }
            for b in &layout.button_controls {
                let path = format!("track{track_num}.{}", b.name);
                let entry = ControlMappingEntry::new_button(
                    path.clone(),
                    b.message_type,
                    b.midi_number,
                    b.message_type,
                    b.midi_number,
                    b.channel,
                    b.default_behavior,
                );
                entries.insert(path, entry);
            }
        }

        for section in &def.extra_sections {
            for control in &section.controls {
                let path = format!("{}.{}", section.name, control.name());
                match control {
                    ControlDef::Continuous(c) => {
                        let mut entry = ControlMappingEntry::new_continuous(
                            path.clone(),
                            c.midi_number,
                            c.midi_number,
                            c.channel,
                        );
                        entry.input_message_type = c.message_type;
                        entry.output_message_type = c.message_type;
                        entries.insert(path, entry);
                    }
                    ControlDef::Button(b) => {
                        let entry = ControlMappingEntry::new_button(
                            path.clone(),
                            b.message_type,
                            b.midi_number,
                            b.message_type,
                            b.midi_number,
                            b.channel,
                            b.default_behavior,
                        );
                        entries.insert(path, entry);
                    }
                }
            }
        }

        Self {
            device_id: def.id.clone(),
            entries,
        }
    }

    /// Builds the default 51-control factory mapping for the Korg `nanoKONTROL2`.
    #[must_use]
    pub fn default_nanokontrol2() -> Self {
        Self::from_device_definition(&nanokontrol2_definition())
    }

    /// Builds the default 60-control factory mapping for the Akai `MIDI Mix`.
    #[must_use]
    pub fn default_akai_midimix() -> Self {
        Self::from_device_definition(&akai_midimix_definition())
    }

    /// Builds a `nanoKONTROL2 -> Akai MIDImix` compatibility mapping that translates
    /// Korg `nanoKONTROL2` CC buttons and knobs into Akai `MIDI Mix` Note and CC messages
    /// (`CC -> Note` for Mute/Solo/Rec buttons and Bank/Solo Master, `CC -> CC` for density knobs/sliders).
    #[must_use]
    pub fn nanokontrol2_to_midimix_emulation() -> Self {
        let mut config = Self::default_nanokontrol2();

        for track_idx in 0..8 {
            let track_num = track_idx + 1;
            // Knob (CC 16..23) -> MIDImix Row 2 Knob (CC 17, 21, 25, 29, 47, 51, 55, 59)
            if let Some(knob) = config.entries.get_mut(&format!("track{track_num}.knob")) {
                knob.output_message_type = MidiMessageType::Cc;
                knob.output_number = MIDIMIX_KNOB_ROW2_CCS[track_idx];
                knob.label = Some(format!("Lane {track_num} Density"));
            }
            // Slider (CC 0..7) -> MIDImix Fader (CC 19, 23, 27, 31, 49, 53, 57, 61)
            if let Some(slider) = config.entries.get_mut(&format!("track{track_num}.slider")) {
                slider.output_message_type = MidiMessageType::Cc;
                slider.output_number = MIDIMIX_FADER_CCS[track_idx];
                slider.label = Some(format!("Lane {track_num} Fader"));
            }
            // Mute button (CC 48..55) -> MIDImix Mute Note (Notes 1, 4, 7, 10, 13, 16, 19, 22)
            if let Some(mute) = config.entries.get_mut(&format!("track{track_num}.mute")) {
                mute.output_message_type = MidiMessageType::Note;
                mute.output_number = MIDIMIX_MUTE_NOTES[track_idx];
                mute.label = Some(format!("Lane {track_num} Mute"));
            }
            // Solo button (CC 32..39) -> MIDImix Solo Note (Notes 2, 5, 8, 11, 14, 17, 20, 23)
            if let Some(solo) = config.entries.get_mut(&format!("track{track_num}.solo")) {
                solo.output_message_type = MidiMessageType::Note;
                solo.output_number = MIDIMIX_SOLO_NOTES[track_idx];
                solo.label = Some(format!("Lane {track_num} Solo"));
            }
            // Rec button (CC 64..71) -> MIDImix Rec Arm Note (Notes 3, 6, 9, 12, 15, 18, 21, 24)
            if let Some(rec) = config.entries.get_mut(&format!("track{track_num}.rec")) {
                rec.output_message_type = MidiMessageType::Note;
                rec.output_number = MIDIMIX_REC_ARM_NOTES[track_idx];
                rec.label = Some(format!("Lane {track_num} Reroll Timbre"));
            }
        }

        // Transport Track Left / Right (CC 58 / 59) -> MIDImix Bank Left / Right (Note 25 / 26)
        if let Some(tl) = config.entries.get_mut("transport.track_left") {
            tl.output_message_type = MidiMessageType::Note;
            tl.output_number = 25;
            tl.label = Some("Form Previous".to_string());
        }
        if let Some(tr) = config.entries.get_mut("transport.track_right") {
            tr.output_message_type = MidiMessageType::Note;
            tr.output_number = 26;
            tr.label = Some("Form Next".to_string());
        }
        // Transport Cycle (CC 46) -> MIDImix Solo Master (Note 27, next 8-voice timbre set)
        if let Some(cycle) = config.entries.get_mut("transport.cycle") {
            cycle.output_message_type = MidiMessageType::Note;
            cycle.output_number = 27;
            cycle.behavior = ButtonBehavior::Momentary;
            cycle.label = Some("Next 8-Voice Timbre Set".to_string());
        }
        // Transport Marker Set (CC 60) -> MIDImix Row 1 Knob 8 CC 58 / Snapshot trigger
        if let Some(ms) = config.entries.get_mut("transport.marker_set") {
            ms.output_message_type = MidiMessageType::Cc;
            ms.output_number = MIDIMIX_KNOB_ROW1_CCS[7];
            ms.label = Some("Save Preset Snapshot".to_string());
        }

        config
    }
}

// =============================================================================
// 5. Processed Message & Evaluation Events
// =============================================================================

/// Result of transforming an incoming hardware MIDI event through a [`DeviceMappingPipeline`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessedMidiMessage {
    /// Owning device key (e.g. `"0944:0117::nanoKONTROL2"` or `"nanokontrol2"`).
    pub device_key: String,
    /// Dot-notated control path (e.g. `"track1.knob"`, `"transport.play"`).
    pub control_path: String,
    /// Incoming MIDI message type (`Cc` or `Note`).
    pub input_message_type: MidiMessageType,
    /// Outgoing MIDI message type (`Cc` or `Note`).
    pub output_message_type: MidiMessageType,
    /// Cross-message transformation applied (`CcToCc`, `NoteToCc`, `CcToNote`, `NoteToNote`).
    pub transformation: MessageTransformation,
    /// 1-indexed incoming MIDI channel (`1..=16`).
    pub input_channel: u8,
    /// 1-indexed outgoing MIDI channel (`1..=16`).
    pub output_channel: u8,
    /// Hardware input CC or Note number (`0..=127`).
    pub input_number: u8,
    /// Remapped output CC or Note number (`0..=127`).
    pub output_number: u8,
    /// Unmodified 7-bit input value or velocity (`0..=127`).
    pub raw_input_value: u8,
    /// Transformed 7-bit output value or velocity (`0..=127`).
    pub output_value: u8,
    /// Whether this control is a discrete button.
    pub is_button: bool,
    /// Current logical button state (`Some(true)` = ON, `Some(false)` = OFF) for buttons.
    pub button_state: Option<bool>,
    /// Soft takeover status snapshot for continuous controls.
    pub soft_takeover: Option<SoftTakeoverStatus>,
    /// Optional human-readable parameter label.
    pub label: Option<String>,
}

impl ProcessedMidiMessage {
    /// Encodes the remapped output message into a 3-byte [`RawMidiPacket`].
    #[must_use]
    pub const fn to_raw_midi_packet(&self) -> RawMidiPacket {
        match self.output_message_type {
            MidiMessageType::Cc => RawMidiPacket::control_change(
                self.output_channel,
                self.output_number,
                self.output_value,
            ),
            MidiMessageType::Note => RawMidiPacket::note_on(
                self.output_channel,
                self.output_number,
                self.output_value,
            ),
        }
    }
}

/// Event emitted whenever a button's logical ON/OFF state changes (used to drive hardware LEDs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonStateChangeEvent {
    /// Owning device key.
    pub device_key: String,
    /// Dot-notated control path (e.g. `"track1.solo"`).
    pub control_path: String,
    /// Input MIDI message type (`Cc` or `Note`).
    pub input_message_type: MidiMessageType,
    /// Input MIDI CC or Note number (`0..=127`).
    pub input_number: u8,
    /// New logical state (`true` = ON / LED lit, `false` = OFF / LED dark).
    pub is_on: bool,
    /// Emitted MIDI output value (`on_value` or `off_value`).
    pub output_value: u8,
}

/// Detailed outcome of evaluating an incoming MIDI message in [`DeviceMappingPipeline`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum MappingEvaluation {
    /// Message matched a control, passed Soft Takeover / button state rules, and produced output.
    Emitted {
        message: ProcessedMidiMessage,
        button_event: Option<ButtonStateChangeEvent>,
    },
    /// Message matched a continuous control, but output was suppressed by Soft Takeover (`Catch` or `Pickup`)
    /// to prevent a sudden parameter jump. Includes real-time delta telemetry for GUI display.
    SoftTakeoverLocked {
        device_key: String,
        control_path: String,
        status: SoftTakeoverStatus,
    },
    /// Message was a button release (`value < 64`) on a `Toggle` button and was intentionally ignored.
    ButtonReleaseIgnored {
        device_key: String,
        control_path: String,
        input_number: u8,
    },
    /// Message did not match any configured control in this pipeline.
    Unmapped,
}

impl MappingEvaluation {
    /// Returns `Some(ProcessedMidiMessage)` if the evaluation emitted a message, or `None` otherwise.
    #[must_use]
    pub fn into_emitted(self) -> Option<ProcessedMidiMessage> {
        match self {
            Self::Emitted { message, .. } => Some(message),
            _ => None,
        }
    }
}

// =============================================================================
// 6. Per-Device Mapping Pipeline
// =============================================================================

/// Stateful MIDI remapping pipeline for a single hardware controller.
#[derive(Debug, Clone)]
pub struct DeviceMappingPipeline {
    /// Unique identifier for the owning device (e.g. `HardwareFingerprint::canonical_key()`).
    pub device_key: String,
    /// Active mapping configuration for this controller.
    pub config: DeviceMappingConfig,
    /// Active Soft Takeover mode (`Catch`, `Jump`, or `Pickup`).
    pub soft_takeover_mode: SoftTakeoverMode,
    /// Active Soft Takeover threshold (`1..=20`, default `3`).
    pub soft_takeover_threshold: u8,
    /// Fast composite lookup map `(MidiMessageType, midi_number) -> control_path`.
    lookup_table: BTreeMap<CompositeMidiKey, String>,
    /// Per-button logical ON/OFF states (`control_path -> is_on`).
    button_states: BTreeMap<String, bool>,
    /// Per-continuous-control Soft Takeover trackers (`control_path -> SoftTakeoverTracker`).
    takeover_trackers: BTreeMap<String, SoftTakeoverTracker>,
    /// Most recent transformed output value per control (`control_path -> output_value`).
    last_output_values: BTreeMap<String, u8>,
}

impl DeviceMappingPipeline {
    /// Creates a new [`DeviceMappingPipeline`] from a device key and [`DeviceMappingConfig`].
    #[must_use]
    pub fn new(device_key: impl Into<String>, config: DeviceMappingConfig) -> Self {
        let mut pipeline = Self {
            device_key: device_key.into(),
            config,
            soft_takeover_mode: SoftTakeoverMode::Catch,
            soft_takeover_threshold: DEFAULT_SOFT_TAKEOVER_THRESHOLD,
            lookup_table: BTreeMap::new(),
            button_states: BTreeMap::new(),
            takeover_trackers: BTreeMap::new(),
            last_output_values: BTreeMap::new(),
        };
        pipeline.rebuild_lookup_tables(false);
        pipeline
    }

    /// Creates a pipeline pre-populated with the default 1:1 mapping for `def`.
    #[must_use]
    pub fn from_device_definition(device_key: impl Into<String>, def: &DeviceDefinition) -> Self {
        Self::new(device_key, DeviceMappingConfig::from_device_definition(def))
    }

    /// Configures the Soft Takeover mode and threshold (`1..=64`) for this pipeline.
    pub fn set_soft_takeover(&mut self, mode: SoftTakeoverMode, threshold: u8) {
        self.soft_takeover_mode = mode;
        self.soft_takeover_threshold = threshold.clamp(1, 64);
        if mode == SoftTakeoverMode::Jump {
            for tracker in self.takeover_trackers.values_mut() {
                tracker.engaged = true;
            }
        }
    }

    /// Updates a single [`ControlMappingEntry`] and rebuilds the fast composite lookup map
    /// while preserving existing button and soft takeover states.
    pub fn update_entry(&mut self, entry: ControlMappingEntry) {
        self.config
            .entries
            .insert(entry.control_path.clone(), entry);
        self.rebuild_lookup_tables(true);
    }

    /// Replaces the entire [`DeviceMappingConfig`] while preserving button states and takeover
    /// trackers for controls that still exist.
    pub fn update_config(&mut self, new_config: DeviceMappingConfig) {
        self.config = new_config;
        self.rebuild_lookup_tables(true);
    }

    /// Sets a software/preset target value (`0..=127`) for a continuous control, arming
    /// Soft Takeover (`Catch` or `Pickup`) if the physical knob/fader is not within `threshold`.
    pub fn set_target_value(&mut self, control_path: &str, target_value: u8) -> bool {
        let Some(entry) = self.config.entries.get(control_path) else {
            return false;
        };
        if entry.is_button {
            return false;
        }
        let tracker = self
            .takeover_trackers
            .entry(control_path.to_string())
            .or_default();
        tracker.set_target(
            target_value,
            self.soft_takeover_mode,
            self.soft_takeover_threshold,
        );
        self.last_output_values.insert(
            control_path.to_string(),
            entry.compute_continuous_output(target_value.min(127)),
        );
        true
    }

    /// Bulk-loads software/preset target values (`control_path -> target_value`) across
    /// multiple continuous controls, locking any control whose physical position differs from
    /// the new preset value by more than `soft_takeover_threshold`.
    pub fn load_preset_targets(&mut self, targets: &BTreeMap<String, u8>) {
        for (path, &val) in targets {
            let _ = self.set_target_value(path, val);
        }
    }

    /// Queries the current [`SoftTakeoverStatus`] for a continuous control.
    #[must_use]
    pub fn soft_takeover_status(&self, control_path: &str) -> Option<SoftTakeoverStatus> {
        self.takeover_trackers.get(control_path).map(|t| {
            t.current_status(self.soft_takeover_mode, self.soft_takeover_threshold)
        })
    }

    /// Returns the current logical ON/OFF state of a button by its `control_path`.
    #[must_use]
    pub fn button_state(&self, control_path: &str) -> bool {
        self.button_states
            .get(control_path)
            .copied()
            .unwrap_or(false)
    }

    /// Returns the current logical ON/OFF state of a button by its hardware CC or Note number.
    #[must_use]
    pub fn button_state_by_number(&self, input_number: u8) -> bool {
        let cc_key = CompositeMidiKey::new(MidiMessageType::Cc, input_number);
        if let Some(path) = self.lookup_table.get(&cc_key) {
            return self.button_state(path);
        }
        let note_key = CompositeMidiKey::new(MidiMessageType::Note, input_number);
        if let Some(path) = self.lookup_table.get(&note_key) {
            return self.button_state(path);
        }
        false
    }

    /// Explicitly sets a button's logical ON/OFF state (e.g. from external OSC or preset sync)
    /// and returns a [`ButtonStateChangeEvent`] if the state changed.
    pub fn set_button_state(
        &mut self,
        control_path: &str,
        is_on: bool,
    ) -> Option<ButtonStateChangeEvent> {
        let entry = self.config.entries.get(control_path)?;
        if !entry.is_button {
            return None;
        }
        let prev = self
            .button_states
            .insert(control_path.to_string(), is_on)
            .unwrap_or(false);
        let output_value = entry.compute_button_output(is_on);
        self.last_output_values
            .insert(control_path.to_string(), output_value);

        if prev != is_on {
            Some(ButtonStateChangeEvent {
                device_key: self.device_key.clone(),
                control_path: control_path.to_string(),
                input_message_type: entry.input_message_type,
                input_number: entry.input_number,
                is_on,
                output_value,
            })
        } else {
            None
        }
    }

    /// Resets all button states to `false` (OFF) and returns the list of state change events.
    pub fn reset_button_states(&mut self) -> Vec<ButtonStateChangeEvent> {
        let paths: Vec<String> = self.button_states.keys().cloned().collect();
        let mut events = Vec::new();
        for path in paths {
            if let Some(ev) = self.set_button_state(&path, false) {
                events.push(ev);
            }
        }
        events
    }

    /// Returns the total number of mapped controls in this pipeline.
    #[must_use]
    pub fn mapping_count(&self) -> usize {
        self.lookup_table.len()
    }

    /// Evaluates an incoming MIDI message (`Cc` or `Note`) with full Soft Takeover and
    /// button state machine diagnostics.
    pub fn evaluate_message(
        &mut self,
        input_message_type: MidiMessageType,
        input_channel: u8,
        midi_number: u8,
        raw_value: u8,
    ) -> MappingEvaluation {
        let key = CompositeMidiKey::new(input_message_type, midi_number);
        let Some(control_path) = self.lookup_table.get(&key).cloned() else {
            return MappingEvaluation::Unmapped;
        };
        let Some(entry) = self.config.entries.get(&control_path).cloned() else {
            return MappingEvaluation::Unmapped;
        };

        let normalized_in_ch = input_channel.clamp(1, 16);
        if let Some(expected_ch) = entry.input_channel {
            if expected_ch != normalized_in_ch {
                return MappingEvaluation::Unmapped;
            }
        }

        let clamped_raw = raw_value.min(127);

        if entry.is_button {
            let is_press = clamped_raw >= BUTTON_PRESS_THRESHOLD;
            match entry.behavior {
                ButtonBehavior::Toggle => {
                    if !is_press {
                        return MappingEvaluation::ButtonReleaseIgnored {
                            device_key: self.device_key.clone(),
                            control_path,
                            input_number: entry.input_number,
                        };
                    }
                    let current = self.button_state(&control_path);
                    let new_state = !current;
                    self.button_states.insert(control_path.clone(), new_state);
                    let output_value = entry.compute_button_output(new_state);
                    self.last_output_values
                        .insert(control_path.clone(), output_value);

                    let button_event = ButtonStateChangeEvent {
                        device_key: self.device_key.clone(),
                        control_path: control_path.clone(),
                        input_message_type: entry.input_message_type,
                        input_number: entry.input_number,
                        is_on: new_state,
                        output_value,
                    };

                    let message = ProcessedMidiMessage {
                        device_key: self.device_key.clone(),
                        control_path,
                        input_message_type: entry.input_message_type,
                        output_message_type: entry.output_message_type,
                        transformation: entry.transformation(),
                        input_channel: normalized_in_ch,
                        output_channel: entry.output_channel,
                        input_number: entry.input_number,
                        output_number: entry.output_number,
                        raw_input_value: clamped_raw,
                        output_value,
                        is_button: true,
                        button_state: Some(new_state),
                        soft_takeover: None,
                        label: entry.label.clone(),
                    };

                    MappingEvaluation::Emitted {
                        message,
                        button_event: Some(button_event),
                    }
                }
                ButtonBehavior::Momentary => {
                    let new_state = is_press;
                    self.button_states.insert(control_path.clone(), new_state);
                    let output_value = entry.compute_button_output(new_state);
                    self.last_output_values
                        .insert(control_path.clone(), output_value);

                    let button_event = ButtonStateChangeEvent {
                        device_key: self.device_key.clone(),
                        control_path: control_path.clone(),
                        input_message_type: entry.input_message_type,
                        input_number: entry.input_number,
                        is_on: new_state,
                        output_value,
                    };

                    let message = ProcessedMidiMessage {
                        device_key: self.device_key.clone(),
                        control_path,
                        input_message_type: entry.input_message_type,
                        output_message_type: entry.output_message_type,
                        transformation: entry.transformation(),
                        input_channel: normalized_in_ch,
                        output_channel: entry.output_channel,
                        input_number: entry.input_number,
                        output_number: entry.output_number,
                        raw_input_value: clamped_raw,
                        output_value,
                        is_button: true,
                        button_state: Some(new_state),
                        soft_takeover: None,
                        label: entry.label.clone(),
                    };

                    MappingEvaluation::Emitted {
                        message,
                        button_event: Some(button_event),
                    }
                }
            }
        } else {
            // Continuous control: evaluate Soft Takeover first
            let tracker = self
                .takeover_trackers
                .entry(control_path.clone())
                .or_default();
            let status = tracker.evaluate(
                clamped_raw,
                self.soft_takeover_mode,
                self.soft_takeover_threshold,
            );

            if !status.engaged {
                return MappingEvaluation::SoftTakeoverLocked {
                    device_key: self.device_key.clone(),
                    control_path,
                    status,
                };
            }

            let output_value = entry.compute_continuous_output(clamped_raw);
            self.last_output_values
                .insert(control_path.clone(), output_value);

            let message = ProcessedMidiMessage {
                device_key: self.device_key.clone(),
                control_path,
                input_message_type: entry.input_message_type,
                output_message_type: entry.output_message_type,
                transformation: entry.transformation(),
                input_channel: normalized_in_ch,
                output_channel: entry.output_channel,
                input_number: entry.input_number,
                output_number: entry.output_number,
                raw_input_value: clamped_raw,
                output_value,
                is_button: false,
                button_state: None,
                soft_takeover: Some(status),
                label: entry.label.clone(),
            };

            MappingEvaluation::Emitted {
                message,
                button_event: None,
            }
        }
    }

    /// Processes any incoming MIDI message (`Cc` or `Note`) and returns `Some(ProcessedMidiMessage)`
    /// when an output message should be transmitted.
    pub fn process_message(
        &mut self,
        input_message_type: MidiMessageType,
        input_channel: u8,
        midi_number: u8,
        raw_value: u8,
    ) -> Option<ProcessedMidiMessage> {
        self.evaluate_message(input_message_type, input_channel, midi_number, raw_value)
            .into_emitted()
    }

    /// Processes an incoming Control Change (`CC`) message (`1..=16` channel, `0..=127` CC, `0..=127` value).
    pub fn process_cc(
        &mut self,
        input_channel: u8,
        cc: u8,
        value: u8,
    ) -> Option<ProcessedMidiMessage> {
        self.process_message(MidiMessageType::Cc, input_channel, cc, value)
    }

    /// Processes an incoming Note On/Off message (`1..=16` channel, `0..=127` note, `0..=127` velocity).
    pub fn process_note(
        &mut self,
        input_channel: u8,
        note: u8,
        velocity: u8,
    ) -> Option<ProcessedMidiMessage> {
        self.process_message(MidiMessageType::Note, input_channel, note, velocity)
    }

    /// Decodes and evaluates a 3-byte [`RawMidiPacket`].
    pub fn evaluate_raw_packet(&mut self, packet: RawMidiPacket) -> MappingEvaluation {
        let Some((msg_type, ch, num, val)) = packet.decode() else {
            return MappingEvaluation::Unmapped;
        };
        self.evaluate_message(msg_type, ch, num, val)
    }

    fn rebuild_lookup_tables(&mut self, preserve_state: bool) {
        let old_buttons = if preserve_state {
            std::mem::take(&mut self.button_states)
        } else {
            BTreeMap::new()
        };
        let old_trackers = if preserve_state {
            std::mem::take(&mut self.takeover_trackers)
        } else {
            BTreeMap::new()
        };

        self.lookup_table.clear();
        self.button_states.clear();
        self.takeover_trackers.clear();

        for (path, entry) in &self.config.entries {
            self.lookup_table.insert(entry.lookup_key(), path.clone());
            if entry.is_button {
                let prev = old_buttons.get(path).copied().unwrap_or(false);
                self.button_states.insert(path.clone(), prev);
            } else {
                let prev = old_trackers.get(path).cloned().unwrap_or_default();
                self.takeover_trackers.insert(path.clone(), prev);
            }
        }
    }
}

// =============================================================================
// 7. Simultaneous Multi-Device Mapping Engine
// =============================================================================

/// Coordinates simultaneous [`DeviceMappingPipeline`]s across multiple connected hardware
/// MIDI controllers without cross-device CC/Note collisions.
#[derive(Debug, Clone)]
pub struct MultiDeviceMappingEngine {
    /// Active per-device pipelines keyed by `device_key` (e.g. `HardwareFingerprint::canonical_key()`).
    pub pipelines: BTreeMap<String, DeviceMappingPipeline>,
    /// Global default Soft Takeover mode applied to newly registered pipelines.
    pub default_soft_takeover_mode: SoftTakeoverMode,
    /// Global default Soft Takeover threshold (`1..=20`, default `3`).
    pub default_soft_takeover_threshold: u8,
}

impl Default for MultiDeviceMappingEngine {
    fn default() -> Self {
        Self {
            pipelines: BTreeMap::new(),
            default_soft_takeover_mode: SoftTakeoverMode::Catch,
            default_soft_takeover_threshold: DEFAULT_SOFT_TAKEOVER_THRESHOLD,
        }
    }
}

impl MultiDeviceMappingEngine {
    /// Creates a new [`MultiDeviceMappingEngine`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or replaces a device mapping pipeline and applies the engine's default
    /// Soft Takeover settings.
    pub fn register_device_pipeline(
        &mut self,
        device_key: impl Into<String>,
        config: DeviceMappingConfig,
    ) -> &mut DeviceMappingPipeline {
        let key = device_key.into();
        let mut pipeline = DeviceMappingPipeline::new(key.clone(), config);
        pipeline.set_soft_takeover(
            self.default_soft_takeover_mode,
            self.default_soft_takeover_threshold,
        );
        self.pipelines.entry(key).insert_entry(pipeline).into_mut()
    }

    /// Updates Soft Takeover mode and threshold across all registered device pipelines.
    pub fn set_global_soft_takeover(&mut self, mode: SoftTakeoverMode, threshold: u8) {
        let clamped = threshold.clamp(1, 64);
        self.default_soft_takeover_mode = mode;
        self.default_soft_takeover_threshold = clamped;
        for pipeline in self.pipelines.values_mut() {
            pipeline.set_soft_takeover(mode, clamped);
        }
    }

    /// Evaluates an incoming MIDI message for a specific `device_key`.
    pub fn evaluate_for_device(
        &mut self,
        device_key: &str,
        message_type: MidiMessageType,
        input_channel: u8,
        midi_number: u8,
        raw_value: u8,
    ) -> MappingEvaluation {
        let Some(pipeline) = self.pipelines.get_mut(device_key) else {
            return MappingEvaluation::Unmapped;
        };
        pipeline.evaluate_message(message_type, input_channel, midi_number, raw_value)
    }

    /// Evaluates a 3-byte [`RawMidiPacket`] for a specific `device_key`.
    pub fn evaluate_packet_for_device(
        &mut self,
        device_key: &str,
        packet: RawMidiPacket,
    ) -> MappingEvaluation {
        let Some(pipeline) = self.pipelines.get_mut(device_key) else {
            return MappingEvaluation::Unmapped;
        };
        pipeline.evaluate_raw_packet(packet)
    }
}

// =============================================================================
// Unit & Integration Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_four_message_transformations_and_output_channels() {
        let mut config = DeviceMappingConfig::default_nanokontrol2();

        // 1. CC-to-CC on channel 5 (track1.knob: CC 16 -> CC 74 ch 5)
        if let Some(e) = config.entries.get_mut("track1.knob") {
            e.input_message_type = MidiMessageType::Cc;
            e.output_message_type = MidiMessageType::Cc;
            e.output_number = 74;
            e.output_channel = 5;
        }

        // 2. CC-to-Note on channel 10 (track1.mute: CC 48 -> Note 36 ch 10)
        if let Some(e) = config.entries.get_mut("track1.mute") {
            e.input_message_type = MidiMessageType::Cc;
            e.output_message_type = MidiMessageType::Note;
            e.output_number = 36;
            e.output_channel = 10;
            e.behavior = ButtonBehavior::Momentary;
        }

        // 3. Note-to-CC on channel 2 (custom entry: Note 25 -> CC 11 ch 2)
        let mut note_to_cc = ControlMappingEntry::new_button(
            "extra.note_to_cc",
            MidiMessageType::Note,
            25,
            MidiMessageType::Cc,
            11,
            2,
            ButtonBehavior::Momentary,
        );
        note_to_cc.on_value = 100;
        note_to_cc.off_value = 10;
        config
            .entries
            .insert("extra.note_to_cc".to_string(), note_to_cc);

        // 4. Note-to-Note on channel 16 (custom entry: Note 60 -> Note 64 ch 16)
        let note_to_note = ControlMappingEntry::new_button(
            "extra.note_to_note",
            MidiMessageType::Note,
            60,
            MidiMessageType::Note,
            64,
            16,
            ButtonBehavior::Toggle,
        );
        config
            .entries
            .insert("extra.note_to_note".to_string(), note_to_note);

        let mut pipeline = DeviceMappingPipeline::new("nk2-unit-1", config);

        // Verify 1: CC-to-CC
        let msg1 = pipeline
            .process_cc(1, 16, 96)
            .expect("CC-to-CC should emit");
        assert_eq!(msg1.transformation, MessageTransformation::CcToCc);
        assert_eq!(msg1.output_message_type, MidiMessageType::Cc);
        assert_eq!(msg1.output_number, 74);
        assert_eq!(msg1.output_channel, 5);
        assert_eq!(msg1.output_value, 96);
        assert_eq!(
            msg1.to_raw_midi_packet().to_bytes(),
            [0xB4, 74, 96],
            "Channel 5 CC status byte must be 0xB4"
        );

        // Verify 2: CC-to-Note
        let msg2 = pipeline
            .process_cc(1, 48, 127)
            .expect("CC-to-Note should emit");
        assert_eq!(msg2.transformation, MessageTransformation::CcToNote);
        assert_eq!(msg2.output_message_type, MidiMessageType::Note);
        assert_eq!(msg2.output_number, 36);
        assert_eq!(msg2.output_channel, 10);
        assert_eq!(msg2.output_value, 127);
        assert_eq!(
            msg2.to_raw_midi_packet().to_bytes(),
            [0x99, 36, 127],
            "Channel 10 Note On status byte must be 0x99"
        );

        // Verify 3: Note-to-CC
        let msg3 = pipeline
            .process_note(1, 25, 127)
            .expect("Note-to-CC should emit");
        assert_eq!(msg3.transformation, MessageTransformation::NoteToCc);
        assert_eq!(msg3.output_message_type, MidiMessageType::Cc);
        assert_eq!(msg3.output_number, 11);
        assert_eq!(msg3.output_channel, 2);
        assert_eq!(msg3.output_value, 100);
        assert_eq!(msg3.to_raw_midi_packet().to_bytes(), [0xB1, 11, 100]);

        // Verify 4: Note-to-Note
        let msg4 = pipeline
            .process_note(1, 60, 127)
            .expect("Note-to-Note should emit");
        assert_eq!(msg4.transformation, MessageTransformation::NoteToNote);
        assert_eq!(msg4.output_message_type, MidiMessageType::Note);
        assert_eq!(msg4.output_number, 64);
        assert_eq!(msg4.output_channel, 16);
        assert_eq!(msg4.output_value, 127);
        assert_eq!(msg4.to_raw_midi_packet().to_bytes(), [0x9F, 64, 127]);
    }

    #[test]
    fn test_min_max_range_scaling_polarity_inversion_and_response_curves() {
        // Test endpoints and midpoints across all 4 response curves
        for curve in [
            ResponseCurve::Linear,
            ResponseCurve::Exponential,
            ResponseCurve::Logarithmic,
            ResponseCurve::SCurve,
        ] {
            assert_eq!(curve.transform_value(0, 0, 127, false), 0);
            assert_eq!(curve.transform_value(127, 0, 127, false), 127);
            assert_eq!(curve.transform_value(0, 0, 127, true), 127);
            assert_eq!(curve.transform_value(127, 0, 127, true), 0);
        }

        // At midpoint raw=64 (normalized ~0.5039):
        let lin_mid = ResponseCurve::Linear.transform_value(64, 0, 127, false);
        let exp_mid = ResponseCurve::Exponential.transform_value(64, 0, 127, false);
        let log_mid = ResponseCurve::Logarithmic.transform_value(64, 0, 127, false);
        let scurve_mid = ResponseCurve::SCurve.transform_value(64, 0, 127, false);

        assert_eq!(lin_mid, 64);
        assert!(
            exp_mid < lin_mid,
            "Exponential midpoint ({exp_mid}) should be below linear ({lin_mid})"
        );
        assert!(
            log_mid > lin_mid,
            "Logarithmic midpoint ({log_mid}) should be above linear ({lin_mid})"
        );
        assert_eq!(
            scurve_mid, 64,
            "S-Curve must pass through the center midpoint"
        );

        // S-Curve quarter points: gentler than linear at 25% (raw=32), steeper by 75% (raw=96)
        let s_q1 = ResponseCurve::SCurve.transform_value(32, 0, 127, false);
        let s_q3 = ResponseCurve::SCurve.transform_value(96, 0, 127, false);
        assert!(s_q1 < 32, "S-Curve at 25% ({s_q1}) should be < 32");
        assert!(s_q3 > 96, "S-Curve at 75% ({s_q3}) should be > 96");

        // Min/Max range scaling [20..=100] with and without polarity inversion
        assert_eq!(ResponseCurve::Linear.transform_value(0, 20, 100, false), 20);
        assert_eq!(
            ResponseCurve::Linear.transform_value(127, 20, 100, false),
            100
        );
        assert_eq!(ResponseCurve::Linear.transform_value(0, 20, 100, true), 100);
        assert_eq!(
            ResponseCurve::Linear.transform_value(127, 20, 100, true),
            20
        );
    }

    #[test]
    fn test_toggle_and_momentary_buttons_with_custom_on_off_values() {
        let mut config = DeviceMappingConfig::default_nanokontrol2();

        // Configure track1.solo (CC 32) as Toggle with custom on_value=110, off_value=15
        if let Some(solo) = config.entries.get_mut("track1.solo") {
            solo.behavior = ButtonBehavior::Toggle;
            solo.on_value = 110;
            solo.off_value = 15;
        }

        // Configure track1.rec (CC 64) as Momentary with custom on_value=99, off_value=5
        if let Some(rec) = config.entries.get_mut("track1.rec") {
            rec.behavior = ButtonBehavior::Momentary;
            rec.on_value = 99;
            rec.off_value = 5;
        }

        let mut pipeline = DeviceMappingPipeline::new("nk2", config);

        // 1. Toggle button: press #1 -> ON (110)
        let p1 = pipeline
            .process_cc(1, 32, 127)
            .expect("Toggle press 1 should emit");
        assert_eq!(p1.output_value, 110);
        assert_eq!(p1.button_state, Some(true));
        assert!(pipeline.button_state("track1.solo"));

        // Release on Toggle button -> ignored!
        let r1 = pipeline.evaluate_message(MidiMessageType::Cc, 1, 32, 0);
        assert!(matches!(r1, MappingEvaluation::ButtonReleaseIgnored { .. }));
        assert!(pipeline.button_state("track1.solo"));

        // Press #2 -> OFF (15)
        let p2 = pipeline
            .process_cc(1, 32, 127)
            .expect("Toggle press 2 should emit");
        assert_eq!(p2.output_value, 15);
        assert_eq!(p2.button_state, Some(false));
        assert!(!pipeline.button_state("track1.solo"));

        // 2. Momentary button: press -> ON (99), release -> OFF (5)
        let mp = pipeline
            .process_cc(1, 64, 127)
            .expect("Momentary press should emit");
        assert_eq!(mp.output_value, 99);
        assert_eq!(mp.button_state, Some(true));

        let mr = pipeline
            .process_cc(1, 64, 0)
            .expect("Momentary release should emit");
        assert_eq!(mr.output_value, 5);
        assert_eq!(mr.button_state, Some(false));
    }

    #[test]
    fn test_soft_takeover_catch_jump_and_pickup_with_realtime_delta() {
        let mut pipeline =
            DeviceMappingPipeline::new("nk2", DeviceMappingConfig::default_nanokontrol2());

        // --- Mode 1: CATCH (threshold = 3) ---
        pipeline.set_soft_takeover(SoftTakeoverMode::Catch, 3);
        // Physical knob starts at 20; preset loads target = 80
        let _ = pipeline.process_cc(1, 16, 20);
        assert!(pipeline.set_target_value("track1.knob", 80));

        // Physical moves to 35 -> still locked, delta = -45, direction = MoveUp
        let ev1 = pipeline.evaluate_message(MidiMessageType::Cc, 1, 16, 35);
        match ev1 {
            MappingEvaluation::SoftTakeoverLocked { status, .. } => {
                assert!(!status.engaged);
                assert_eq!(status.physical_value, 35);
                assert_eq!(status.target_value, 80);
                assert_eq!(status.delta, -45);
                assert_eq!(status.abs_delta, 45);
                assert_eq!(status.direction, TakeoverDirection::MoveUp);
            }
            other => panic!("Expected SoftTakeoverLocked, got {other:?}"),
        }

        // Fast jump in Catch mode from 35 straight to 95 (skipping [77..83] window) -> STILL LOCKED!
        let ev2 = pipeline.evaluate_message(MidiMessageType::Cc, 1, 16, 95);
        match ev2 {
            MappingEvaluation::SoftTakeoverLocked { status, .. } => {
                assert!(!status.engaged);
                assert_eq!(status.delta, 15);
                assert_eq!(status.direction, TakeoverDirection::MoveDown);
            }
            other => panic!("Expected Catch mode to stay locked on overshoot, got {other:?}"),
        }

        // Physical moves into threshold window at 82 (|82 - 80| = 2 <= 3) -> CATCHES!
        let ev3 = pipeline.evaluate_message(MidiMessageType::Cc, 1, 16, 82);
        match ev3 {
            MappingEvaluation::Emitted { message, .. } => {
                let st = message.soft_takeover.expect("soft takeover status present");
                assert!(st.engaged);
                assert_eq!(st.delta, 0);
                assert_eq!(st.direction, TakeoverDirection::Matched);
                assert_eq!(message.output_value, 82);
            }
            other => panic!("Expected Emitted on catch within threshold, got {other:?}"),
        }

        // --- Mode 2: PICKUP (threshold = 3) ---
        pipeline.set_soft_takeover(SoftTakeoverMode::Pickup, 3);
        // Physical is at 82; preset loads new target = 50 -> locks control
        assert!(pipeline.set_target_value("track1.knob", 50));
        let st_locked = pipeline
            .soft_takeover_status("track1.knob")
            .expect("tracker exists");
        assert!(!st_locked.engaged);
        assert_eq!(st_locked.delta, 32); // 82 - 50 = +32 (MoveDown)
        assert_eq!(st_locked.direction, TakeoverDirection::MoveDown);

        // User sweeps fader quickly downward from 82 -> 65 (not yet crossed 50)
        let p_step1 = pipeline.evaluate_message(MidiMessageType::Cc, 1, 16, 65);
        assert!(matches!(p_step1, MappingEvaluation::SoftTakeoverLocked { .. }));

        // Next hardware sample jumps from 65 -> 38 (crosses 50 outside the [47..53] window!)
        // Pickup mode detects the zero-crossing (65 > 50 && 38 <= 50) and immediately unlocks!
        let p_step2 = pipeline.evaluate_message(MidiMessageType::Cc, 1, 16, 38);
        match p_step2 {
            MappingEvaluation::Emitted { message, .. } => {
                assert_eq!(message.output_value, 38);
                assert!(message.soft_takeover.unwrap().engaged);
            }
            other => panic!("Expected Pickup mode to unlock on target crossing, got {other:?}"),
        }

        // --- Mode 3: JUMP ---
        pipeline.set_soft_takeover(SoftTakeoverMode::Jump, 3);
        assert!(pipeline.set_target_value("track1.knob", 120));
        // Even though physical jumps to 10 (far from 120), Jump mode emits immediately
        let j_msg = pipeline
            .process_cc(1, 16, 10)
            .expect("Jump mode must always emit");
        assert_eq!(j_msg.output_value, 10);
    }

    #[test]
    fn test_simultaneous_multi_device_pipelines_without_collision() {
        let mut engine = MultiDeviceMappingEngine::new();
        engine.register_device_pipeline(
            "0944:0117::nanoKONTROL2",
            DeviceMappingConfig::nanokontrol2_to_midimix_emulation(),
        );
        engine.register_device_pipeline(
            "09e8:0031::MIDI Mix",
            DeviceMappingConfig::default_akai_midimix(),
        );

        // Both devices receive MIDI number 16 on Channel 1:
        // - On nanoKONTROL2 (emulation): CC 16 is track1.knob -> mapped to MIDImix Row 2 CC 17
        // - On Akai MIDI Mix: CC 16 is track1.knob1 -> mapped to CC 16, while Note 16 is track6.mute!
        let nk2_out = engine
            .evaluate_for_device("0944:0117::nanoKONTROL2", MidiMessageType::Cc, 1, 16, 100)
            .into_emitted()
            .expect("nk2 CC 16 mapped");
        assert_eq!(nk2_out.control_path, "track1.knob");
        assert_eq!(nk2_out.output_number, 17);

        let akai_cc_out = engine
            .evaluate_for_device("09e8:0031::MIDI Mix", MidiMessageType::Cc, 1, 16, 64)
            .into_emitted()
            .expect("akai CC 16 mapped");
        assert_eq!(akai_cc_out.control_path, "track1.knob1");
        assert_eq!(akai_cc_out.output_number, 16);

        let akai_note_out = engine
            .evaluate_for_device("09e8:0031::MIDI Mix", MidiMessageType::Note, 1, 16, 127)
            .into_emitted()
            .expect("akai Note 16 mapped");
        assert_eq!(akai_note_out.control_path, "track6.mute");
        assert_eq!(akai_note_out.transformation, MessageTransformation::NoteToNote);
        assert_eq!(akai_note_out.output_number, 16);
    }

    #[test]
    fn test_nanokontrol2_to_midimix_emulation_preset_completeness() {
        let mut pipeline = DeviceMappingPipeline::new(
            "0944:0117::nanoKONTROL2",
            DeviceMappingConfig::nanokontrol2_to_midimix_emulation(),
        );
        assert_eq!(pipeline.mapping_count(), 51);

        // Track 1..8 Mute CCs (48..55) -> MIDImix Mute Notes (1, 4, 7, 10, 13, 16, 19, 22)
        for idx in 0..8 {
            let cc = 48 + idx as u8;
            let msg = pipeline
                .process_cc(1, cc, 127)
                .expect("Mute CC should emit Note");
            assert_eq!(msg.transformation, MessageTransformation::CcToNote);
            assert_eq!(msg.output_number, MIDIMIX_MUTE_NOTES[idx]);
            assert_eq!(msg.output_value, 127);
        }

        // Track Left (CC 58) -> Note 25 (Form Previous), Track Right (CC 59) -> Note 26 (Form Next)
        let prev_form = pipeline.process_cc(1, 58, 127).unwrap();
        assert_eq!(prev_form.output_message_type, MidiMessageType::Note);
        assert_eq!(prev_form.output_number, 25);

        let next_form = pipeline.process_cc(1, 59, 127).unwrap();
        assert_eq!(next_form.output_message_type, MidiMessageType::Note);
        assert_eq!(next_form.output_number, 26);

        // Cycle (CC 46) -> Note 27 (Solo Master / Next 8-voice timbre set)
        let next_timbre_set = pipeline.process_cc(1, 46, 127).unwrap();
        assert_eq!(next_timbre_set.output_message_type, MidiMessageType::Note);
        assert_eq!(next_timbre_set.output_number, 27);
    }

    #[test]
    fn test_input_channel_filter_raw_packet_decode_and_state_preservation() {
        let mut config = DeviceMappingConfig::default_akai_midimix();
        // Restrict track1.knob1 (CC 16) to only accept input on MIDI Channel 3
        if let Some(k1) = config.entries.get_mut("track1.knob1") {
            k1.input_channel = Some(3);
            k1.output_channel = 7;
        }

        let mut pipeline = DeviceMappingPipeline::new("midimix", config);

        // Channel 1 CC 16 -> Unmapped due to input_channel = Some(3)
        assert_eq!(
            pipeline.evaluate_raw_packet(RawMidiPacket::control_change(1, 16, 90)),
            MappingEvaluation::Unmapped
        );

        // Channel 3 CC 16 -> Emitted on output channel 7
        let out = pipeline
            .evaluate_raw_packet(RawMidiPacket::control_change(3, 16, 90))
            .into_emitted()
            .expect("Channel 3 CC 16 must match");
        assert_eq!(out.input_channel, 3);
        assert_eq!(out.output_channel, 7);

        // Note On (0x90) and Note Off (0x80) on Momentary button track1.rec_arm (Note 3)
        let press = pipeline
            .evaluate_raw_packet(RawMidiPacket::note_on(1, 3, 127))
            .into_emitted()
            .expect("Note On should emit");
        assert_eq!(press.button_state, Some(true));
        assert_eq!(press.output_value, 127);

        let release = pipeline
            .evaluate_raw_packet(RawMidiPacket::note_off(1, 3, 64))
            .into_emitted()
            .expect("Note Off (0x80) should normalize velocity to 0 and emit release");
        assert_eq!(release.button_state, Some(false));
        assert_eq!(release.output_value, 0);

        // Toggle track1.mute (Note 1) ON, then update_config and verify button state is preserved
        let _ = pipeline.process_note(1, 1, 127);
        assert!(pipeline.button_state("track1.mute"));
        pipeline.update_config(DeviceMappingConfig::default_akai_midimix());
        assert!(
            pipeline.button_state("track1.mute"),
            "Button toggle state must be preserved across update_config"
        );
    }
}
