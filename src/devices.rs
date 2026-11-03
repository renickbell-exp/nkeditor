//! Device definition system and synthesizer target CC reference for `nkEditor4`.
//!
//! Models:
//! 1. **Korg nanoKONTROL2** (`nanokontrol2`, USB `0944:0117`):
//!    - 8 tracks, each with 1 knob (`CC 16..23`), 1 slider (`CC 0..7`), and 3 LED buttons:
//!      `solo` (`CC 32..39`, toggle), `mute` (`CC 48..55`, toggle), `rec` (`CC 64..71`, momentary)
//!    - Transport section with 11 buttons (`play` CC 41, `stop` CC 42, `rewind` CC 43,
//!      `forward` CC 44, `record` CC 45, `cycle` CC 46, `track_left` CC 58, `track_right` CC 59,
//!      `marker_set` CC 60, `marker_left` CC 61, `marker_right` CC 62)
//!    - 28 controllable LEDs (24 track LEDs + 4 transport LEDs: `play`, `stop`, `record`, `cycle`)
//! 2. **Akai MIDI Mix** (`akai-midimix`, USB `09e8:0031`):
//!    - 8 tracks, each with 3 knobs (`knob1` CC `[16, 20, 24, 28, 46, 50, 54, 58]`,
//!      `knob2` CC `[17, 21, 25, 29, 47, 51, 55, 59]`, `knob3` CC `[18, 22, 26, 30, 48, 52, 56, 60]`),
//!      1 fader (`fader` CC `[19, 23, 27, 31, 49, 53, 57, 61]`), and 3 Note buttons:
//!      `mute` (Notes `[1, 4, 7, 10, 13, 16, 19, 22]`, toggle),
//!      `solo` (Notes `[2, 5, 8, 11, 14, 17, 20, 23]`, toggle),
//!      `rec_arm` (Notes `[3, 6, 9, 12, 15, 18, 21, 24]`, momentary)
//!    - Master section with `master_fader` (`CC 62`), `bank_left` (`Note 25`),
//!      `bank_right` (`Note 26`), and `solo_master` (`Note 27`)
//!    - 24 controllable LEDs (8 `mute` + 8 `solo` + 8 `rec_arm` via Note messages)
//! 3. **Nord Drum 2 Desktop** target percussion synthesizer CC and channel definitions:
//!    - Per-pad synthesis CCs across Tone, Noise, Click, and Mix sections (Pads 1..6 on Channels 1..6)
//!    - Global FX CCs on Channel 10

use serde::{Deserialize, Serialize};

// =============================================================================
// Core MIDI & Device Abstraction Types
// =============================================================================

/// MIDI message type used by physical controls and hardware LEDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MidiMessageType {
    /// Control Change (`0xB0 .. 0xBF`).
    Cc,
    /// Note On / Note Off (`0x90 .. 0x9F` / `0x80 .. 0x8F`).
    Note,
}

/// Visual widget archetype for continuous controls in the GUI editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContinuousVisualType {
    Knob,
    Slider,
    Fader,
}

/// Button interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonBehavior {
    /// Latches between On (`127`) and Off (`0`) on each press.
    Toggle,
    /// Active (`127`) while held, Off (`0`) when released.
    Momentary,
}

/// Definition of a continuous physical control (knob, slider, or fader).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuousControlDef {
    /// Semantic identifier within its track or section (e.g. `"knob"`, `"knob1"`, `"slider"`, `"fader"`).
    pub name: String,
    /// Human-readable label for UI display.
    pub display_name: String,
    /// Hardware MIDI CC number (`0..=127`).
    pub midi_number: u8,
    /// 1-indexed MIDI channel (`1..=16`).
    pub channel: u8,
    /// Continuous controls always use `MidiMessageType::Cc`.
    pub message_type: MidiMessageType,
    /// Visual representation hint (`Knob`, `Slider`, or `Fader`).
    pub visual_type: ContinuousVisualType,
}

/// Definition of a discrete physical button control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonControlDef {
    /// Semantic identifier within its track or section (e.g. `"solo"`, `"mute"`, `"rec"`, `"play"`).
    pub name: String,
    /// Human-readable label for UI display.
    pub display_name: String,
    /// Hardware MIDI CC or Note number (`0..=127`).
    pub midi_number: u8,
    /// 1-indexed MIDI channel (`1..=16`).
    pub channel: u8,
    /// Message type (`Cc` on nanoKONTROL2, `Note` on Akai MIDI Mix).
    pub message_type: MidiMessageType,
    /// Default interaction behavior (`Toggle` or `Momentary`).
    pub default_behavior: ButtonBehavior,
    /// Whether the physical button houses a controllable LED indicator.
    pub has_led: bool,
}

/// Discriminated union of physical control definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ControlDef {
    Continuous(ContinuousControlDef),
    Button(ButtonControlDef),
}

impl ControlDef {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Continuous(c) => &c.name,
            Self::Button(b) => &b.name,
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        match self {
            Self::Continuous(c) => &c.display_name,
            Self::Button(b) => &b.display_name,
        }
    }

    #[must_use]
    pub const fn midi_number(&self) -> u8 {
        match self {
            Self::Continuous(c) => c.midi_number,
            Self::Button(b) => b.midi_number,
        }
    }

    #[must_use]
    pub const fn channel(&self) -> u8 {
        match self {
            Self::Continuous(c) => c.channel,
            Self::Button(b) => b.channel,
        }
    }

    #[must_use]
    pub const fn message_type(&self) -> MidiMessageType {
        match self {
            Self::Continuous(c) => c.message_type,
            Self::Button(b) => b.message_type,
        }
    }

    #[must_use]
    pub const fn is_button(&self) -> bool {
        matches!(self, Self::Button(_))
    }

    #[must_use]
    pub const fn is_continuous(&self) -> bool {
        matches!(self, Self::Continuous(_))
    }
}

/// Layout of controls within a single repeating channel strip (track).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackLayout {
    /// Continuous controls in this track (knobs, sliders, faders).
    pub continuous_controls: Vec<ContinuousControlDef>,
    /// Button controls in this track (`solo`, `mute`, `rec` / `rec_arm`).
    pub button_controls: Vec<ButtonControlDef>,
}

/// Named group of controls outside the per-track strips (e.g. `transport` or `master`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraControlsSection {
    /// Machine-readable section identifier (e.g. `"transport"`, `"master"`).
    pub name: String,
    /// Human-readable section title.
    pub display_name: String,
    /// Controls belonging to this section.
    pub controls: Vec<ControlDef>,
}

/// Protocol used to illuminate or extinguish LEDs on a hardware controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedProtocol {
    /// Whether LEDs are addressed via `Cc` or `Note` messages.
    pub message_type: MidiMessageType,
    /// 1-indexed MIDI channel (`1..=16`).
    pub channel: u8,
    /// Value sent to turn an LED on (`127`).
    pub on_value: u8,
    /// Value sent to turn an LED off (`0`).
    pub off_value: u8,
}

/// Mapping between a dot-notated button path and its hardware LED MIDI number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedDef {
    /// Dot-notated button identifier (e.g. `"track1.solo"`, `"transport.play"`).
    pub button_name: String,
    /// MIDI CC or Note number that drives this LED.
    pub midi_number: u8,
}

/// Complete declarative hardware specification for a supported MIDI controller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDefinition {
    /// Unique identifier (`"nanokontrol2"`, `"akai-midimix"`).
    pub id: String,
    /// Human-readable model name (`"nanoKONTROL2"`, `"MIDI Mix"`).
    pub name: String,
    /// Hardware manufacturer (`"Korg"`, `"Akai"`).
    pub manufacturer: String,
    /// USB Vendor ID (`0x0944` for Korg, `0x09e8` for Akai).
    pub usb_vendor_id: u16,
    /// USB Product ID (`0x0117` for nanoKONTROL2, `0x0031` for MIDI Mix).
    pub usb_product_id: u16,
    /// Case-insensitive port prefixes matched against OS MIDI port names.
    pub port_prefixes: Vec<String>,
    /// Number of channel strips (`8`).
    pub track_count: usize,
    /// Per-track control layouts (`length == track_count`).
    pub track_layouts: Vec<TrackLayout>,
    /// Extra control sections (`transport` for nanoKONTROL2, `master` for MIDI Mix).
    pub extra_sections: Vec<ExtraControlsSection>,
    /// Hardware LED control protocol.
    pub led_protocol: LedProtocol,
    /// Complete list of controllable hardware LEDs.
    pub leds: Vec<LedDef>,
    /// Optional default virtual output port name.
    pub virtual_port_name: Option<String>,
}

impl DeviceDefinition {
    /// Total number of physical controls across all tracks and extra sections.
    #[must_use]
    pub fn total_control_count(&self) -> usize {
        let track_controls: usize = self
            .track_layouts
            .iter()
            .map(|t| t.continuous_controls.len() + t.button_controls.len())
            .sum();
        let extra_controls: usize = self.extra_sections.iter().map(|s| s.controls.len()).sum();
        track_controls + extra_controls
    }

    /// Look up the dot-notated control path (e.g. `"track1.knob"`, `"transport.play"`)
    /// from an incoming MIDI message type, 1-indexed channel, and MIDI number.
    #[must_use]
    pub fn find_control_by_midi(
        &self,
        message_type: MidiMessageType,
        channel: u8,
        midi_number: u8,
    ) -> Option<String> {
        for (idx, layout) in self.track_layouts.iter().enumerate() {
            let track_num = idx + 1;
            for c in &layout.continuous_controls {
                if c.message_type == message_type
                    && c.channel == channel
                    && c.midi_number == midi_number
                {
                    return Some(format!("track{track_num}.{}", c.name));
                }
            }
            for b in &layout.button_controls {
                if b.message_type == message_type
                    && b.channel == channel
                    && b.midi_number == midi_number
                {
                    return Some(format!("track{track_num}.{}", b.name));
                }
            }
        }

        for section in &self.extra_sections {
            for control in &section.controls {
                if control.message_type() == message_type
                    && control.channel() == channel
                    && control.midi_number() == midi_number
                {
                    return Some(format!("{}.{}", section.name, control.name()));
                }
            }
        }

        None
    }
}

// =============================================================================
// 1. Korg nanoKONTROL2 Hardware Constants & Definition
// =============================================================================

/// Factory CC numbers for the 8 rotary knobs on Korg nanoKONTROL2 (Tracks 1..8).
pub const NK2_KNOB_CCS: [u8; 8] = [16, 17, 18, 19, 20, 21, 22, 23];

/// Factory CC numbers for the 8 linear sliders on Korg nanoKONTROL2 (Tracks 1..8).
pub const NK2_SLIDER_CCS: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

/// Factory CC numbers for the 8 Solo buttons on Korg nanoKONTROL2 (Tracks 1..8).
pub const NK2_SOLO_CCS: [u8; 8] = [32, 33, 34, 35, 36, 37, 38, 39];

/// Factory CC numbers for the 8 Mute buttons on Korg nanoKONTROL2 (Tracks 1..8).
pub const NK2_MUTE_CCS: [u8; 8] = [48, 49, 50, 51, 52, 53, 54, 55];

/// Factory CC numbers for the 8 Record buttons on Korg nanoKONTROL2 (Tracks 1..8).
pub const NK2_REC_CCS: [u8; 8] = [64, 65, 66, 67, 68, 69, 70, 71];

/// Factory CC numbers for the 11 Transport buttons on Korg nanoKONTROL2.
pub mod nk2_transport_cc {
    pub const PLAY: u8 = 41;
    pub const STOP: u8 = 42;
    pub const REWIND: u8 = 43;
    pub const FORWARD: u8 = 44;
    pub const RECORD: u8 = 45;
    pub const CYCLE: u8 = 46;
    pub const TRACK_LEFT: u8 = 58;
    pub const TRACK_RIGHT: u8 = 59;
    pub const MARKER_SET: u8 = 60;
    pub const MARKER_LEFT: u8 = 61;
    pub const MARKER_RIGHT: u8 = 62;
}

/// Transport button CCs on the Korg nanoKONTROL2 that have physical LEDs.
pub const NK2_TRANSPORT_LED_CCS: [u8; 4] = [
    nk2_transport_cc::PLAY,
    nk2_transport_cc::STOP,
    nk2_transport_cc::RECORD,
    nk2_transport_cc::CYCLE,
];

/// All 28 LED CC numbers on the Korg nanoKONTROL2 (8 solo + 8 mute + 8 rec + 4 transport).
pub const NK2_ALL_LED_CCS: [u8; 28] = [
    // Solo (8)
    32, 33, 34, 35, 36, 37, 38, 39,
    // Mute (8)
    48, 49, 50, 51, 52, 53, 54, 55,
    // Rec (8)
    64, 65, 66, 67, 68, 69, 70, 71,
    // Transport with LEDs (4)
    41, 42, 45, 46,
];

/// Returns `true` if `cc` corresponds to one of the 28 LEDs on the Korg nanoKONTROL2.
#[must_use]
pub fn nk2_has_led_cc(cc: u8) -> bool {
    NK2_ALL_LED_CCS.contains(&cc)
}

/// Derives the dot-notated control identifier (e.g. `"track1.knob"`, `"transport.play"`)
/// from a raw Korg nanoKONTROL2 CC number (`0..=127`).
#[must_use]
pub fn derive_nk2_control_type_from_cc(cc: u8) -> Option<String> {
    if let Some(idx) = NK2_KNOB_CCS.iter().position(|&x| x == cc) {
        return Some(format!("track{}.knob", idx + 1));
    }
    if let Some(idx) = NK2_SLIDER_CCS.iter().position(|&x| x == cc) {
        return Some(format!("track{}.slider", idx + 1));
    }
    if let Some(idx) = NK2_SOLO_CCS.iter().position(|&x| x == cc) {
        return Some(format!("track{}.solo", idx + 1));
    }
    if let Some(idx) = NK2_MUTE_CCS.iter().position(|&x| x == cc) {
        return Some(format!("track{}.mute", idx + 1));
    }
    if let Some(idx) = NK2_REC_CCS.iter().position(|&x| x == cc) {
        return Some(format!("track{}.rec", idx + 1));
    }
    match cc {
        nk2_transport_cc::PLAY => Some("transport.play".to_string()),
        nk2_transport_cc::STOP => Some("transport.stop".to_string()),
        nk2_transport_cc::REWIND => Some("transport.rewind".to_string()),
        nk2_transport_cc::FORWARD => Some("transport.forward".to_string()),
        nk2_transport_cc::RECORD => Some("transport.record".to_string()),
        nk2_transport_cc::CYCLE => Some("transport.cycle".to_string()),
        nk2_transport_cc::TRACK_LEFT => Some("transport.track_left".to_string()),
        nk2_transport_cc::TRACK_RIGHT => Some("transport.track_right".to_string()),
        nk2_transport_cc::MARKER_SET => Some("transport.marker_set".to_string()),
        nk2_transport_cc::MARKER_LEFT => Some("transport.marker_left".to_string()),
        nk2_transport_cc::MARKER_RIGHT => Some("transport.marker_right".to_string()),
        _ => None,
    }
}

/// Builds the complete [`DeviceDefinition`] for the Korg nanoKONTROL2.
#[must_use]
pub fn nanokontrol2_definition() -> DeviceDefinition {
    let mut track_layouts = Vec::with_capacity(8);
    let mut leds = Vec::with_capacity(28);

    for track_idx in 0..8 {
        let track_num = track_idx + 1;
        let knob = ContinuousControlDef {
            name: "knob".to_string(),
            display_name: format!("Track {track_num} Knob"),
            midi_number: NK2_KNOB_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            visual_type: ContinuousVisualType::Knob,
        };
        let slider = ContinuousControlDef {
            name: "slider".to_string(),
            display_name: format!("Track {track_num} Slider"),
            midi_number: NK2_SLIDER_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            visual_type: ContinuousVisualType::Slider,
        };
        let solo = ButtonControlDef {
            name: "solo".to_string(),
            display_name: format!("Track {track_num} Solo"),
            midi_number: NK2_SOLO_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            default_behavior: ButtonBehavior::Toggle,
            has_led: true,
        };
        let mute = ButtonControlDef {
            name: "mute".to_string(),
            display_name: format!("Track {track_num} Mute"),
            midi_number: NK2_MUTE_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            default_behavior: ButtonBehavior::Toggle,
            has_led: true,
        };
        let rec = ButtonControlDef {
            name: "rec".to_string(),
            display_name: format!("Track {track_num} Rec"),
            midi_number: NK2_REC_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            default_behavior: ButtonBehavior::Momentary,
            has_led: true,
        };

        leds.push(LedDef {
            button_name: format!("track{track_num}.solo"),
            midi_number: NK2_SOLO_CCS[track_idx],
        });
        leds.push(LedDef {
            button_name: format!("track{track_num}.mute"),
            midi_number: NK2_MUTE_CCS[track_idx],
        });
        leds.push(LedDef {
            button_name: format!("track{track_num}.rec"),
            midi_number: NK2_REC_CCS[track_idx],
        });

        track_layouts.push(TrackLayout {
            continuous_controls: vec![knob, slider],
            button_controls: vec![solo, mute, rec],
        });
    }

    let transport_buttons = [
        ("play", "Play", nk2_transport_cc::PLAY, ButtonBehavior::Momentary, true),
        ("stop", "Stop", nk2_transport_cc::STOP, ButtonBehavior::Momentary, true),
        ("rewind", "Rewind", nk2_transport_cc::REWIND, ButtonBehavior::Momentary, false),
        ("forward", "Forward", nk2_transport_cc::FORWARD, ButtonBehavior::Momentary, false),
        ("record", "Record", nk2_transport_cc::RECORD, ButtonBehavior::Momentary, true),
        ("cycle", "Cycle", nk2_transport_cc::CYCLE, ButtonBehavior::Toggle, true),
        ("track_left", "Track Left", nk2_transport_cc::TRACK_LEFT, ButtonBehavior::Momentary, false),
        ("track_right", "Track Right", nk2_transport_cc::TRACK_RIGHT, ButtonBehavior::Momentary, false),
        ("marker_set", "Marker Set", nk2_transport_cc::MARKER_SET, ButtonBehavior::Momentary, false),
        ("marker_left", "Marker Left", nk2_transport_cc::MARKER_LEFT, ButtonBehavior::Momentary, false),
        ("marker_right", "Marker Right", nk2_transport_cc::MARKER_RIGHT, ButtonBehavior::Momentary, false),
    ];

    let transport_controls: Vec<ControlDef> = transport_buttons
        .into_iter()
        .map(|(name, display_name, midi_number, default_behavior, has_led)| {
            ControlDef::Button(ButtonControlDef {
                name: name.to_string(),
                display_name: display_name.to_string(),
                midi_number,
                channel: 1,
                message_type: MidiMessageType::Cc,
                default_behavior,
                has_led,
            })
        })
        .collect();

    leds.push(LedDef {
        button_name: "transport.play".to_string(),
        midi_number: nk2_transport_cc::PLAY,
    });
    leds.push(LedDef {
        button_name: "transport.stop".to_string(),
        midi_number: nk2_transport_cc::STOP,
    });
    leds.push(LedDef {
        button_name: "transport.record".to_string(),
        midi_number: nk2_transport_cc::RECORD,
    });
    leds.push(LedDef {
        button_name: "transport.cycle".to_string(),
        midi_number: nk2_transport_cc::CYCLE,
    });

    DeviceDefinition {
        id: "nanokontrol2".to_string(),
        name: "nanoKONTROL2".to_string(),
        manufacturer: "Korg".to_string(),
        usb_vendor_id: 0x0944,
        usb_product_id: 0x0117,
        port_prefixes: vec!["nanokontrol2".to_string()],
        track_count: 8,
        track_layouts,
        extra_sections: vec![ExtraControlsSection {
            name: "transport".to_string(),
            display_name: "Transport".to_string(),
            controls: transport_controls,
        }],
        led_protocol: LedProtocol {
            message_type: MidiMessageType::Cc,
            channel: 1,
            on_value: 127,
            off_value: 0,
        },
        leds,
        virtual_port_name: Some("nkEditor3 Out".to_string()),
    }
}

// =============================================================================
// 2. Akai MIDI Mix Hardware Constants & Definition
// =============================================================================

/// Knob Row 1 CC numbers for Akai MIDI Mix (Tracks 1..8).
pub const MIDIMIX_KNOB_ROW1_CCS: [u8; 8] = [16, 20, 24, 28, 46, 50, 54, 58];

/// Knob Row 2 CC numbers for Akai MIDI Mix (Tracks 1..8).
pub const MIDIMIX_KNOB_ROW2_CCS: [u8; 8] = [17, 21, 25, 29, 47, 51, 55, 59];

/// Knob Row 3 CC numbers for Akai MIDI Mix (Tracks 1..8).
pub const MIDIMIX_KNOB_ROW3_CCS: [u8; 8] = [18, 22, 26, 30, 48, 52, 56, 60];

/// Fader CC numbers for Akai MIDI Mix (Tracks 1..8).
pub const MIDIMIX_FADER_CCS: [u8; 8] = [19, 23, 27, 31, 49, 53, 57, 61];

/// Mute button Note numbers for Akai MIDI Mix (Tracks 1..8).
pub const MIDIMIX_MUTE_NOTES: [u8; 8] = [1, 4, 7, 10, 13, 16, 19, 22];

/// Solo button Note numbers for Akai MIDI Mix (Tracks 1..8).
pub const MIDIMIX_SOLO_NOTES: [u8; 8] = [2, 5, 8, 11, 14, 17, 20, 23];

/// Rec Arm button Note numbers for Akai MIDI Mix (Tracks 1..8).
pub const MIDIMIX_REC_ARM_NOTES: [u8; 8] = [3, 6, 9, 12, 15, 18, 21, 24];

/// Master section MIDI numbers for Akai MIDI Mix.
pub mod midimix_master {
    /// Master Fader CC number.
    pub const MASTER_FADER_CC: u8 = 62;
    /// Bank Left button Note number.
    pub const BANK_LEFT_NOTE: u8 = 25;
    /// Bank Right button Note number.
    pub const BANK_RIGHT_NOTE: u8 = 26;
    /// Solo Master button Note number.
    pub const SOLO_MASTER_NOTE: u8 = 27;
}

/// Builds the complete [`DeviceDefinition`] for the Akai MIDI Mix.
#[must_use]
pub fn akai_midimix_definition() -> DeviceDefinition {
    let mut track_layouts = Vec::with_capacity(8);
    let mut leds = Vec::with_capacity(24);

    for track_idx in 0..8 {
        let track_num = track_idx + 1;
        let knob1 = ContinuousControlDef {
            name: "knob1".to_string(),
            display_name: format!("Track {track_num} Knob 1"),
            midi_number: MIDIMIX_KNOB_ROW1_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            visual_type: ContinuousVisualType::Knob,
        };
        let knob2 = ContinuousControlDef {
            name: "knob2".to_string(),
            display_name: format!("Track {track_num} Knob 2"),
            midi_number: MIDIMIX_KNOB_ROW2_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            visual_type: ContinuousVisualType::Knob,
        };
        let knob3 = ContinuousControlDef {
            name: "knob3".to_string(),
            display_name: format!("Track {track_num} Knob 3"),
            midi_number: MIDIMIX_KNOB_ROW3_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            visual_type: ContinuousVisualType::Knob,
        };
        let fader = ContinuousControlDef {
            name: "fader".to_string(),
            display_name: format!("Track {track_num} Fader"),
            midi_number: MIDIMIX_FADER_CCS[track_idx],
            channel: 1,
            message_type: MidiMessageType::Cc,
            visual_type: ContinuousVisualType::Fader,
        };

        let mute = ButtonControlDef {
            name: "mute".to_string(),
            display_name: format!("Track {track_num} Mute"),
            midi_number: MIDIMIX_MUTE_NOTES[track_idx],
            channel: 1,
            message_type: MidiMessageType::Note,
            default_behavior: ButtonBehavior::Toggle,
            has_led: true,
        };
        let solo = ButtonControlDef {
            name: "solo".to_string(),
            display_name: format!("Track {track_num} Solo"),
            midi_number: MIDIMIX_SOLO_NOTES[track_idx],
            channel: 1,
            message_type: MidiMessageType::Note,
            default_behavior: ButtonBehavior::Toggle,
            has_led: true,
        };
        let rec_arm = ButtonControlDef {
            name: "rec_arm".to_string(),
            display_name: format!("Track {track_num} Rec Arm"),
            midi_number: MIDIMIX_REC_ARM_NOTES[track_idx],
            channel: 1,
            message_type: MidiMessageType::Note,
            default_behavior: ButtonBehavior::Momentary,
            has_led: true,
        };

        leds.push(LedDef {
            button_name: format!("track{track_num}.mute"),
            midi_number: MIDIMIX_MUTE_NOTES[track_idx],
        });
        leds.push(LedDef {
            button_name: format!("track{track_num}.solo"),
            midi_number: MIDIMIX_SOLO_NOTES[track_idx],
        });
        leds.push(LedDef {
            button_name: format!("track{track_num}.rec_arm"),
            midi_number: MIDIMIX_REC_ARM_NOTES[track_idx],
        });

        track_layouts.push(TrackLayout {
            continuous_controls: vec![knob1, knob2, knob3, fader],
            button_controls: vec![mute, solo, rec_arm],
        });
    }

    let master_controls = vec![
        ControlDef::Continuous(ContinuousControlDef {
            name: "master_fader".to_string(),
            display_name: "Master Fader".to_string(),
            midi_number: midimix_master::MASTER_FADER_CC,
            channel: 1,
            message_type: MidiMessageType::Cc,
            visual_type: ContinuousVisualType::Fader,
        }),
        ControlDef::Button(ButtonControlDef {
            name: "bank_left".to_string(),
            display_name: "Bank Left".to_string(),
            midi_number: midimix_master::BANK_LEFT_NOTE,
            channel: 1,
            message_type: MidiMessageType::Note,
            default_behavior: ButtonBehavior::Momentary,
            has_led: false,
        }),
        ControlDef::Button(ButtonControlDef {
            name: "bank_right".to_string(),
            display_name: "Bank Right".to_string(),
            midi_number: midimix_master::BANK_RIGHT_NOTE,
            channel: 1,
            message_type: MidiMessageType::Note,
            default_behavior: ButtonBehavior::Momentary,
            has_led: false,
        }),
        ControlDef::Button(ButtonControlDef {
            name: "solo_master".to_string(),
            display_name: "Solo Master".to_string(),
            midi_number: midimix_master::SOLO_MASTER_NOTE,
            channel: 1,
            message_type: MidiMessageType::Note,
            default_behavior: ButtonBehavior::Toggle,
            has_led: false,
        }),
    ];

    DeviceDefinition {
        id: "akai-midimix".to_string(),
        name: "MIDI Mix".to_string(),
        manufacturer: "Akai".to_string(),
        usb_vendor_id: 0x09E8,
        usb_product_id: 0x0031,
        port_prefixes: vec!["midi mix".to_string()],
        track_count: 8,
        track_layouts,
        extra_sections: vec![ExtraControlsSection {
            name: "master".to_string(),
            display_name: "Master".to_string(),
            controls: master_controls,
        }],
        led_protocol: LedProtocol {
            message_type: MidiMessageType::Note,
            channel: 1,
            on_value: 127,
            off_value: 0,
        },
        leds,
        virtual_port_name: None,
    }
}

// =============================================================================
// Device Registry Functions
// =============================================================================

/// Default device ID (`"nanokontrol2"`).
pub const DEFAULT_DEVICE_ID: &str = "nanokontrol2";

/// Returns all registered hardware controller definitions.
#[must_use]
pub fn get_all_device_definitions() -> Vec<DeviceDefinition> {
    vec![nanokontrol2_definition(), akai_midimix_definition()]
}

/// Retrieves a device definition by its unique ID (`"nanokontrol2"` or `"akai-midimix"`).
#[must_use]
pub fn get_device_definition(id: &str) -> Option<DeviceDefinition> {
    match id {
        "nanokontrol2" => Some(nanokontrol2_definition()),
        "akai-midimix" => Some(akai_midimix_definition()),
        _ => None,
    }
}

/// Returns the default device definition (`nanoKONTROL2`).
#[must_use]
pub fn get_default_device() -> DeviceDefinition {
    nanokontrol2_definition()
}

/// Identifies a registered device by matching a MIDI port name against the device's
/// port patterns (`^nanokontrol2[:\s]` or `^MIDI Mix[:\s]` / `^MIDI Mix MIDI`).
#[must_use]
pub fn identify_device(port_name: &str) -> Option<DeviceDefinition> {
    let lower = port_name.to_ascii_lowercase();
    if lower.starts_with("nanokontrol2:") || lower.starts_with("nanokontrol2 ") {
        return Some(nanokontrol2_definition());
    }
    if lower.starts_with("midi mix:") || lower.starts_with("midi mix ") {
        return Some(akai_midimix_definition());
    }
    None
}

// =============================================================================
// 3. Nord Drum 2 Desktop Target Synthesizer CC & Channel Definitions
// =============================================================================

/// Verified MIDI CC and Channel definitions for the **Nord Drum 2 Desktop** percussion synthesizer.
///
/// The Nord Drum 2 exposes 6 independent drum pads, each assigned to its own MIDI channel
/// (factory default: channels `1..=6`), plus Global Effects on channel `10`.
pub mod nord_drum_2 {
    use serde::{Deserialize, Serialize};

    /// Tone oscillator parameters (Pitch, Waveform, Timbre, Decay, and Pitch Bend).
    pub mod tone {
        /// Spectral character of the tone oscillator (`CC 30`).
        pub const SPECTRA: u8 = 30;
        /// Pitch coarse tuning MSB (`CC 31`).
        pub const PITCH_MSB: u8 = 31;
        /// Waveform selection for the tone oscillator (`CC 46`).
        pub const WAVE: u8 = 46;
        /// Rate at which timbre decays over note duration (`CC 47`).
        pub const TIMBRE_DECAY: u8 = 47;
        /// Initial transient emphasis on tone attack (`CC 48`).
        pub const PUNCH: u8 = 48;
        /// Decay curve shape: linear, exponential, or gated (`CC 49`).
        pub const DECAY_TYPE: u8 = 49;
        /// Primary decay time for the tone oscillator (`CC 50`).
        pub const DECAY: u8 = 50;
        /// Secondary decay time for the low-frequency component (`CC 51`).
        pub const DECAY_LO: u8 = 51;
        /// Harmonic content / timbral character (`CC 52`).
        pub const TIMBRE: u8 = 52;
        /// Envelope modulation depth applied to timbre (`CC 53`).
        pub const TIMBRE_ENV: u8 = 53;
        /// Pitch bend sweep range in semitones (`CC 54`).
        pub const BEND_AMOUNT: u8 = 54;
        /// Duration of the pitch bend sweep (`CC 55`).
        pub const BEND_TIME: u8 = 55;
    }

    /// Noise generator parameters (Filter, Envelope, and Decay shaping).
    pub mod noise {
        /// Cutoff frequency of the noise filter (`CC 14`).
        pub const FILTER_FREQ: u8 = 14;
        /// Filter type selection: LP, HP, BP (`CC 15`).
        pub const FILTER_TYPE: u8 = 15;
        /// Envelope modulation depth applied to the noise filter (`CC 16`).
        pub const FILTER_ENV: u8 = 16;
        /// Resonance / Q of the noise filter (`CC 17`).
        pub const RESONANCE: u8 = 17;
        /// Attack time or rate for the noise envelope (`CC 18`).
        pub const ATTACK_RATE: u8 = 18;
        /// Attack mode selector (`CC 19`).
        pub const ATK_MODE: u8 = 19;
        /// Decay curve shape for the noise component (`CC 20`).
        pub const DECAY_TYPE: u8 = 20;
        /// Primary decay time for the noise generator (`CC 21`).
        pub const DECAY: u8 = 21;
        /// Secondary decay time for low-frequency noise (`CC 22`).
        pub const DECAY_LO: u8 = 22;
    }

    /// Click transient parameters (attack layer for percussive definition).
    pub mod click {
        /// Volume level of the click transient layer (`CC 56`).
        pub const LEVEL: u8 = 56;
        /// Click waveform / character selection (`CC 57`).
        pub const TYPE: u8 = 57;
    }

    /// Mix output parameters (Level, Pan, and Tone/Noise balance).
    pub mod mix {
        /// Overall output level for this pad (`CC 7`).
        pub const LEVEL: u8 = 7;
        /// Stereo pan position for this pad (`CC 10`).
        pub const PAN: u8 = 10;
        /// Balance between tone and noise components (`CC 58`).
        pub const BALANCE: u8 = 58;
    }

    /// Global effects MIDI CC numbers (sent on `channels::GLOBAL_FX`, default channel 10).
    pub mod fx {
        /// Distortion drive amount (`CC 23`).
        pub const DIST_AMOUNT: u8 = 23;
        /// Distortion algorithm / type selection (`CC 24`).
        pub const DIST_TYPE: u8 = 24;
        /// EQ center / cutoff frequency (`CC 25`).
        pub const EQ_FREQ: u8 = 25;
        /// EQ boost or cut amount (`CC 26`).
        pub const EQ_GAIN: u8 = 26;
        /// Echo regeneration / feedback amount (`CC 27`).
        pub const ECHO_FEEDBACK: u8 = 27;
        /// Echo wet/dry mix level (`CC 28`).
        pub const ECHO_AMOUNT: u8 = 28;
    }

    /// Factory default 1-indexed MIDI channel assignments for the Nord Drum 2.
    pub mod channels {
        pub const PAD_1: u8 = 1;
        pub const PAD_2: u8 = 2;
        pub const PAD_3: u8 = 3;
        pub const PAD_4: u8 = 4;
        pub const PAD_5: u8 = 5;
        pub const PAD_6: u8 = 6;
        pub const GLOBAL_FX: u8 = 10;

        /// All 6 pad channels in order (`[1, 2, 3, 4, 5, 6]`).
        pub const PADS: [u8; 6] = [PAD_1, PAD_2, PAD_3, PAD_4, PAD_5, PAD_6];
    }

    /// Synthesis section classification for a Nord Drum 2 parameter.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum NordDrum2Section {
        Tone,
        Noise,
        Click,
        Mix,
        GlobalFx,
    }

    /// Descriptor for a single Nord Drum 2 MIDI CC parameter.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NordDrum2ParamDef {
        pub section: NordDrum2Section,
        pub name: &'static str,
        pub cc: u8,
    }

    /// Complete table of all 26 per-pad CC parameters and 6 global FX CC parameters (32 total).
    pub const ALL_PARAMS: [NordDrum2ParamDef; 32] = [
        // Tone (12)
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "SPECTRA", cc: tone::SPECTRA },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "PITCH_MSB", cc: tone::PITCH_MSB },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "WAVE", cc: tone::WAVE },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "TIMBRE_DECAY", cc: tone::TIMBRE_DECAY },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "PUNCH", cc: tone::PUNCH },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "DECAY_TYPE", cc: tone::DECAY_TYPE },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "DECAY", cc: tone::DECAY },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "DECAY_LO", cc: tone::DECAY_LO },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "TIMBRE", cc: tone::TIMBRE },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "TIMBRE_ENV", cc: tone::TIMBRE_ENV },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "BEND_AMOUNT", cc: tone::BEND_AMOUNT },
        NordDrum2ParamDef { section: NordDrum2Section::Tone, name: "BEND_TIME", cc: tone::BEND_TIME },
        // Noise (9)
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "FILTER_FREQ", cc: noise::FILTER_FREQ },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "FILTER_TYPE", cc: noise::FILTER_TYPE },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "FILTER_ENV", cc: noise::FILTER_ENV },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "RESONANCE", cc: noise::RESONANCE },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "ATTACK_RATE", cc: noise::ATTACK_RATE },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "ATK_MODE", cc: noise::ATK_MODE },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "DECAY_TYPE", cc: noise::DECAY_TYPE },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "DECAY", cc: noise::DECAY },
        NordDrum2ParamDef { section: NordDrum2Section::Noise, name: "DECAY_LO", cc: noise::DECAY_LO },
        // Click (2)
        NordDrum2ParamDef { section: NordDrum2Section::Click, name: "LEVEL", cc: click::LEVEL },
        NordDrum2ParamDef { section: NordDrum2Section::Click, name: "TYPE", cc: click::TYPE },
        // Mix (3)
        NordDrum2ParamDef { section: NordDrum2Section::Mix, name: "LEVEL", cc: mix::LEVEL },
        NordDrum2ParamDef { section: NordDrum2Section::Mix, name: "PAN", cc: mix::PAN },
        NordDrum2ParamDef { section: NordDrum2Section::Mix, name: "BALANCE", cc: mix::BALANCE },
        // Global FX (6)
        NordDrum2ParamDef { section: NordDrum2Section::GlobalFx, name: "DIST_AMOUNT", cc: fx::DIST_AMOUNT },
        NordDrum2ParamDef { section: NordDrum2Section::GlobalFx, name: "DIST_TYPE", cc: fx::DIST_TYPE },
        NordDrum2ParamDef { section: NordDrum2Section::GlobalFx, name: "EQ_FREQ", cc: fx::EQ_FREQ },
        NordDrum2ParamDef { section: NordDrum2Section::GlobalFx, name: "EQ_GAIN", cc: fx::EQ_GAIN },
        NordDrum2ParamDef { section: NordDrum2Section::GlobalFx, name: "ECHO_FEEDBACK", cc: fx::ECHO_FEEDBACK },
        NordDrum2ParamDef { section: NordDrum2Section::GlobalFx, name: "ECHO_AMOUNT", cc: fx::ECHO_AMOUNT },
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn nanokontrol2_definition_matches_hardware_spec() {
        let nk2 = nanokontrol2_definition();
        assert_eq!(nk2.id, "nanokontrol2");
        assert_eq!(nk2.name, "nanoKONTROL2");
        assert_eq!(nk2.manufacturer, "Korg");
        assert_eq!(nk2.usb_vendor_id, 0x0944);
        assert_eq!(nk2.usb_product_id, 0x0117);
        assert_eq!(nk2.virtual_port_name.as_deref(), Some("nkEditor3 Out"));
        assert_eq!(nk2.track_count, 8);
        assert_eq!(nk2.track_layouts.len(), 8);
        assert_eq!(nk2.total_control_count(), 51); // 8 * 5 + 11 transport

        // Verify track controls and CCs
        for (i, layout) in nk2.track_layouts.iter().enumerate() {
            assert_eq!(layout.continuous_controls.len(), 2);
            assert_eq!(layout.button_controls.len(), 3);

            let knob = &layout.continuous_controls[0];
            assert_eq!(knob.name, "knob");
            assert_eq!(knob.midi_number, NK2_KNOB_CCS[i]);
            assert_eq!(knob.channel, 1);
            assert_eq!(knob.message_type, MidiMessageType::Cc);
            assert_eq!(knob.visual_type, ContinuousVisualType::Knob);

            let slider = &layout.continuous_controls[1];
            assert_eq!(slider.name, "slider");
            assert_eq!(slider.midi_number, NK2_SLIDER_CCS[i]);
            assert_eq!(slider.channel, 1);
            assert_eq!(slider.message_type, MidiMessageType::Cc);
            assert_eq!(slider.visual_type, ContinuousVisualType::Slider);

            let solo = &layout.button_controls[0];
            assert_eq!(solo.name, "solo");
            assert_eq!(solo.midi_number, NK2_SOLO_CCS[i]);
            assert_eq!(solo.default_behavior, ButtonBehavior::Toggle);
            assert!(solo.has_led);

            let mute = &layout.button_controls[1];
            assert_eq!(mute.name, "mute");
            assert_eq!(mute.midi_number, NK2_MUTE_CCS[i]);
            assert_eq!(mute.default_behavior, ButtonBehavior::Toggle);
            assert!(mute.has_led);

            let rec = &layout.button_controls[2];
            assert_eq!(rec.name, "rec");
            assert_eq!(rec.midi_number, NK2_REC_CCS[i]);
            assert_eq!(rec.default_behavior, ButtonBehavior::Momentary);
            assert!(rec.has_led);
        }

        // Verify transport section
        assert_eq!(nk2.extra_sections.len(), 1);
        let transport = &nk2.extra_sections[0];
        assert_eq!(transport.name, "transport");
        assert_eq!(transport.controls.len(), 11);

        // Verify 28 LEDs
        assert_eq!(nk2.leds.len(), 28);
        assert_eq!(nk2.led_protocol.message_type, MidiMessageType::Cc);
        assert_eq!(nk2.led_protocol.channel, 1);
        assert_eq!(nk2.led_protocol.on_value, 127);
        assert_eq!(nk2.led_protocol.off_value, 0);

        for led in &nk2.leds {
            assert!(nk2_has_led_cc(led.midi_number));
        }

        // Verify derive_nk2_control_type_from_cc
        assert_eq!(
            derive_nk2_control_type_from_cc(16).as_deref(),
            Some("track1.knob")
        );
        assert_eq!(
            derive_nk2_control_type_from_cc(7).as_deref(),
            Some("track8.slider")
        );
        assert_eq!(
            derive_nk2_control_type_from_cc(32).as_deref(),
            Some("track1.solo")
        );
        assert_eq!(
            derive_nk2_control_type_from_cc(55).as_deref(),
            Some("track8.mute")
        );
        assert_eq!(
            derive_nk2_control_type_from_cc(64).as_deref(),
            Some("track1.rec")
        );
        assert_eq!(
            derive_nk2_control_type_from_cc(41).as_deref(),
            Some("transport.play")
        );
        assert_eq!(
            derive_nk2_control_type_from_cc(60).as_deref(),
            Some("transport.marker_set")
        );
        assert_eq!(derive_nk2_control_type_from_cc(99), None);
    }

    #[test]
    fn akai_midimix_definition_matches_hardware_spec() {
        let midimix = akai_midimix_definition();
        assert_eq!(midimix.id, "akai-midimix");
        assert_eq!(midimix.name, "MIDI Mix");
        assert_eq!(midimix.manufacturer, "Akai");
        assert_eq!(midimix.usb_vendor_id, 0x09E8);
        assert_eq!(midimix.usb_product_id, 0x0031);
        assert_eq!(midimix.track_count, 8);
        assert_eq!(midimix.track_layouts.len(), 8);
        assert_eq!(midimix.total_control_count(), 60); // 8 * 7 + 4 master

        for (i, layout) in midimix.track_layouts.iter().enumerate() {
            assert_eq!(layout.continuous_controls.len(), 4);
            assert_eq!(layout.button_controls.len(), 3);

            assert_eq!(layout.continuous_controls[0].name, "knob1");
            assert_eq!(
                layout.continuous_controls[0].midi_number,
                MIDIMIX_KNOB_ROW1_CCS[i]
            );
            assert_eq!(layout.continuous_controls[1].name, "knob2");
            assert_eq!(
                layout.continuous_controls[1].midi_number,
                MIDIMIX_KNOB_ROW2_CCS[i]
            );
            assert_eq!(layout.continuous_controls[2].name, "knob3");
            assert_eq!(
                layout.continuous_controls[2].midi_number,
                MIDIMIX_KNOB_ROW3_CCS[i]
            );
            assert_eq!(layout.continuous_controls[3].name, "fader");
            assert_eq!(
                layout.continuous_controls[3].midi_number,
                MIDIMIX_FADER_CCS[i]
            );
            assert_eq!(
                layout.continuous_controls[3].visual_type,
                ContinuousVisualType::Fader
            );

            for b in &layout.button_controls {
                assert_eq!(b.message_type, MidiMessageType::Note);
                assert!(b.has_led);
            }
            assert_eq!(layout.button_controls[0].midi_number, MIDIMIX_MUTE_NOTES[i]);
            assert_eq!(layout.button_controls[1].midi_number, MIDIMIX_SOLO_NOTES[i]);
            assert_eq!(
                layout.button_controls[2].midi_number,
                MIDIMIX_REC_ARM_NOTES[i]
            );
        }

        // Master section
        assert_eq!(midimix.extra_sections.len(), 1);
        let master = &midimix.extra_sections[0];
        assert_eq!(master.name, "master");
        assert_eq!(master.controls.len(), 4);
        assert_eq!(master.controls[0].name(), "master_fader");
        assert_eq!(master.controls[0].midi_number(), 62);
        assert_eq!(master.controls[0].message_type(), MidiMessageType::Cc);
        assert_eq!(master.controls[1].name(), "bank_left");
        assert_eq!(master.controls[1].midi_number(), 25);
        assert_eq!(master.controls[2].name(), "bank_right");
        assert_eq!(master.controls[2].midi_number(), 26);
        assert_eq!(master.controls[3].name(), "solo_master");
        assert_eq!(master.controls[3].midi_number(), 27);

        // 24 Note LEDs
        assert_eq!(midimix.leds.len(), 24);
        assert_eq!(midimix.led_protocol.message_type, MidiMessageType::Note);
    }

    #[test]
    fn device_registry_and_port_identification_work() {
        let all = get_all_device_definitions();
        assert_eq!(all.len(), 2);

        assert_eq!(get_default_device().id, "nanokontrol2");
        assert!(get_device_definition("nanokontrol2").is_some());
        assert!(get_device_definition("akai-midimix").is_some());
        assert!(get_device_definition("unknown").is_none());

        assert_eq!(
            identify_device("nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0")
                .unwrap()
                .id,
            "nanokontrol2"
        );
        assert_eq!(
            identify_device("nanoKONTROL2 MIDI 1").unwrap().id,
            "nanokontrol2"
        );
        assert_eq!(
            identify_device("MIDI Mix MIDI 1").unwrap().id,
            "akai-midimix"
        );
        assert_eq!(
            identify_device("MIDI Mix: MIDI Mix MIDI 1 28:0")
                .unwrap()
                .id,
            "akai-midimix"
        );
        assert!(identify_device("Unknown Synth").is_none());
        assert!(identify_device("").is_none());
    }

    #[test]
    fn structural_consistency_no_duplicate_midi_assignments_and_exact_leds() {
        for device in get_all_device_definitions() {
            assert_eq!(device.track_layouts.len(), device.track_count);

            let mut seen = HashMap::new();
            let mut buttons_with_leds = HashSet::new();

            for (idx, layout) in device.track_layouts.iter().enumerate() {
                let track_num = idx + 1;
                for c in &layout.continuous_controls {
                    assert!(c.midi_number <= 127);
                    assert!((1..=16).contains(&c.channel));
                    let key = (c.message_type, c.channel, c.midi_number);
                    let path = format!("track{track_num}.{}", c.name);
                    assert!(
                        seen.insert(key, path.clone()).is_none(),
                        "Duplicate MIDI key {key:?} on {}",
                        device.id
                    );
                    assert_eq!(
                        device.find_control_by_midi(c.message_type, c.channel, c.midi_number),
                        Some(path)
                    );
                }
                for b in &layout.button_controls {
                    assert!(b.midi_number <= 127);
                    assert!((1..=16).contains(&b.channel));
                    let key = (b.message_type, b.channel, b.midi_number);
                    let path = format!("track{track_num}.{}", b.name);
                    assert!(
                        seen.insert(key, path.clone()).is_none(),
                        "Duplicate MIDI key {key:?} on {}",
                        device.id
                    );
                    if b.has_led {
                        buttons_with_leds.insert(path);
                    }
                }
            }

            for section in &device.extra_sections {
                for control in &section.controls {
                    assert!(control.midi_number() <= 127);
                    assert!((1..=16).contains(&control.channel()));
                    let key = (
                        control.message_type(),
                        control.channel(),
                        control.midi_number(),
                    );
                    let path = format!("{}.{}", section.name, control.name());
                    assert!(
                        seen.insert(key, path.clone()).is_none(),
                        "Duplicate MIDI key {key:?} on {}",
                        device.id
                    );
                    if let ControlDef::Button(b) = control {
                        if b.has_led {
                            buttons_with_leds.insert(path);
                        }
                    }
                }
            }

            let led_names: HashSet<String> =
                device.leds.iter().map(|l| l.button_name.clone()).collect();
            assert_eq!(
                led_names, buttons_with_leds,
                "LED set mismatch on {}",
                device.id
            );
        }
    }

    #[test]
    fn nord_drum_2_cc_and_channel_constants_are_verified() {
        use nord_drum_2::*;

        assert_eq!(tone::SPECTRA, 30);
        assert_eq!(tone::PITCH_MSB, 31);
        assert_eq!(tone::WAVE, 46);
        assert_eq!(tone::TIMBRE_DECAY, 47);
        assert_eq!(tone::PUNCH, 48);
        assert_eq!(tone::DECAY_TYPE, 49);
        assert_eq!(tone::DECAY, 50);
        assert_eq!(tone::DECAY_LO, 51);
        assert_eq!(tone::TIMBRE, 52);
        assert_eq!(tone::TIMBRE_ENV, 53);
        assert_eq!(tone::BEND_AMOUNT, 54);
        assert_eq!(tone::BEND_TIME, 55);

        assert_eq!(noise::FILTER_FREQ, 14);
        assert_eq!(noise::FILTER_TYPE, 15);
        assert_eq!(noise::FILTER_ENV, 16);
        assert_eq!(noise::RESONANCE, 17);
        assert_eq!(noise::ATTACK_RATE, 18);
        assert_eq!(noise::ATK_MODE, 19);
        assert_eq!(noise::DECAY_TYPE, 20);
        assert_eq!(noise::DECAY, 21);
        assert_eq!(noise::DECAY_LO, 22);

        assert_eq!(click::LEVEL, 56);
        assert_eq!(click::TYPE, 57);

        assert_eq!(mix::LEVEL, 7);
        assert_eq!(mix::PAN, 10);
        assert_eq!(mix::BALANCE, 58);

        assert_eq!(fx::DIST_AMOUNT, 23);
        assert_eq!(fx::DIST_TYPE, 24);
        assert_eq!(fx::EQ_FREQ, 25);
        assert_eq!(fx::EQ_GAIN, 26);
        assert_eq!(fx::ECHO_FEEDBACK, 27);
        assert_eq!(fx::ECHO_AMOUNT, 28);

        assert_eq!(channels::PADS, [1, 2, 3, 4, 5, 6]);
        assert_eq!(channels::GLOBAL_FX, 10);
        assert_eq!(ALL_PARAMS.len(), 32);
    }
}
