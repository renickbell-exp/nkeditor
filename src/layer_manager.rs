//! Multi-Page & Modifier Layer Engine (`src/layer_manager.rs`).
//!
//! Enables compact 8-knob / 8-slider MIDI controllers like the Korg `nanoKONTROL2` to control
//! deep multi-row synthesis and performance parameter sets (surpassing the 3-knob-row Akai
//! `MIDI Mix`) across **4 switchable controller pages**:
//! - **Page 1 (`VoiceDensityAndLevel`)**: Per-voice note density (`0.1x..5.0x`) & level/gain (`0..127`)
//! - **Page 2 (`TimbreMorphAndDecay`)**: Per-voice FM timbre morph/index & envelope decay
//! - **Page 3 (`PitchSpectraAndNoiseFilter`)**: Per-voice pitch/spectra harmonic ratio (Nord Drum 2
//!   `CC 30` on Ch `1..8`) & noise filter cutoff (`CC 14` on Ch `1..8`)
//! - **Page 4 (`GlobalFeelAndMasterFx`)**: Global density, feel scale, swing, humanize & master FX
//!   (`CC 58..65` and `CC 70..77` on Ch `10`)
//!
//! Pages can be switched via:
//! 1. Korg `nanoKONTROL2` `Marker Left / Marker Right` (`CC 61 / CC 62`)
//! 2. `Cycle + Track Left/Right` (`CC 46 + CC 58/59`) or `Cycle + Solo 1..4` (`CC 46 + CC 32..35`)
//!    modifier combinations
//! 3. Interactive `egui` page tabs ([`render_layer_page_selector_bar`])
//! 4. Bidirectional UDP OSC (`/nkeditor/layer/select <1..4 or page_name>`)
//!
//! Whenever the active page changes, [`MultiPageLayerManager::select_page`] automatically saves
//! the outgoing page's targets, loads the incoming page's mapping and target values into
//! [`DeviceMappingPipeline`], engages **Soft Takeover** (`Catch` or `Pickup`) so physical knob
//! positions never cause sudden parameter jumps across pages, and resynchronizes hardware LEDs.

use crate::devices::nanokontrol2_definition;
use crate::led_controller::{LedOperatingMode, MultiDeviceLedManager};
use crate::mapping_engine::{
    ControlMappingEntry, DeviceMappingConfig, DeviceMappingPipeline, SoftTakeoverMode,
};
use crate::op8_model::{
     FAMILY_ORDER, NK2_CC_CYCLE, NK2_CC_MARKER_LEFT, NK2_CC_MARKER_RIGHT, NK2_CC_TRACK_LEFT,
    NK2_CC_TRACK_RIGHT, NK2_DENSITY_KNOB_CCS, NK2_GAIN_SLIDER_CCS,
};
use crate::osc_server::OscServer;
use crate::theme::ThemeSettings;
use eframe::egui::{self, FontId, RichText, Rounding, Stroke, Ui};
use rosc::{OscMessage, OscType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// =============================================================================
// 1. ControllerPage Enum (4 Switchable Layer Pages)
// =============================================================================

/// The 4 switchable parameter layer pages for an 8-track controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ControllerPage {
    /// Page 1: Voice Density (`Knobs 1..8`) & Level/Gain (`Sliders 1..8`).
    #[default]
    VoiceDensityAndLevel,
    /// Page 2: Timbre Morph (`Knobs 1..8`) & Envelope Decay (`Sliders 1..8`).
    TimbreMorphAndDecay,
    /// Page 3: Pitch/Spectra (`Knobs 1..8`) & Noise Filter Cutoff (`Sliders 1..8`).
    PitchSpectraAndNoiseFilter,
    /// Page 4: Global Feel, Swing, Humanize (`Knobs 1..8`) & Master FX (`Sliders 1..8`).
    GlobalFeelAndMasterFx,
}

impl ControllerPage {
    /// All 4 controller pages in canonical order (`Page 1 .. Page 4`).
    pub const ALL: [Self; 4] = [
        Self::VoiceDensityAndLevel,
        Self::TimbreMorphAndDecay,
        Self::PitchSpectraAndNoiseFilter,
        Self::GlobalFeelAndMasterFx,
    ];

    /// 0-based page index (`0..4`).
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::VoiceDensityAndLevel => 0,
            Self::TimbreMorphAndDecay => 1,
            Self::PitchSpectraAndNoiseFilter => 2,
            Self::GlobalFeelAndMasterFx => 3,
        }
    }

    /// 1-based page number (`1..=4`).
    #[must_use]
    pub const fn number(self) -> u8 {
        (self.index() as u8) + 1
    }

    /// Canonical snake_case identifier for OSC and JSON serialization.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::VoiceDensityAndLevel => "page1_density_level",
            Self::TimbreMorphAndDecay => "page2_morph_decay",
            Self::PitchSpectraAndNoiseFilter => "page3_pitch_filter",
            Self::GlobalFeelAndMasterFx => "page4_global_feel_fx",
        }
    }

    /// Human-readable page title for GUI tabs and headers.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::VoiceDensityAndLevel => "Page 1: Voice Density & Level",
            Self::TimbreMorphAndDecay => "Page 2: Timbre Morph & Decay",
            Self::PitchSpectraAndNoiseFilter => "Page 3: Pitch/Spectra & Noise Filter",
            Self::GlobalFeelAndMasterFx => "Page 4: Global Feel, Swing, Humanize & Master FX",
        }
    }

    /// Compact tab badge label.
    #[must_use]
    pub const fn short_tab_label(self) -> &'static str {
        match self {
            Self::VoiceDensityAndLevel => "P1: DENSITY & LEVEL",
            Self::TimbreMorphAndDecay => "P2: MORPH & DECAY",
            Self::PitchSpectraAndNoiseFilter => "P3: PITCH & FILTER",
            Self::GlobalFeelAndMasterFx => "P4: GLOBAL FEEL & FX",
        }
    }

    /// Returns the next page (wrapping `Page 4 -> Page 1`).
    #[must_use]
    pub const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % 4]
    }

    /// Returns the previous page (wrapping `Page 1 -> Page 4`).
    #[must_use]
    pub const fn previous(self) -> Self {
        Self::ALL[(self.index() + 3) % 4]
    }

    /// Parses a page from a 1-based number (`"1".."4"`), 0-based index, `next`/`prev`, or name.
    #[must_use]
    pub fn from_str_or_number(token: &str, current: Self) -> Option<Self> {
        let norm = token.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match norm.as_str() {
            "next" | "+1" => Some(current.next()),
            "prev" | "previous" | "-1" => Some(current.previous()),
            "1" | "p1" | "page1" | "page_1" | "page1_density_level" | "voice_density_and_level"
            | "density" => Some(Self::VoiceDensityAndLevel),
            "2" | "p2" | "page2" | "page_2" | "page2_morph_decay" | "timbre_morph_and_decay"
            | "morph" => Some(Self::TimbreMorphAndDecay),
            "3" | "p3" | "page3" | "page_3" | "page3_pitch_filter"
            | "pitch_spectra_and_noise_filter" | "pitch" | "filter" => {
                Some(Self::PitchSpectraAndNoiseFilter)
            }
            "4" | "p4" | "page4" | "page_4" | "page4_global_feel_fx"
            | "global_feel_and_master_fx" | "global" | "fx" => Some(Self::GlobalFeelAndMasterFx),
            _ => None,
        }
    }
}

// =============================================================================
// 2. Per-Page Parameter Bank & Factory Page Definitions
// =============================================================================

/// Complete mapping, target values, button states, and labels for one [`ControllerPage`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageParameterBank {
    /// Which controller page this bank represents.
    pub page: ControllerPage,
    /// Per-control MIDI mapping configuration for this page.
    pub config: DeviceMappingConfig,
    /// Stored 7-bit target values (`0..=127`) for all 16 continuous controls (`track1.knob..track8.slider`).
    pub continuous_targets: BTreeMap<String, u8>,
    /// Stored logical ON/OFF states for buttons on this page.
    pub button_states: BTreeMap<String, bool>,
    /// Display labels for Knobs `1..8` on this page.
    pub knob_labels: [String; 8],
    /// Display labels for Sliders `1..8` on this page.
    pub slider_labels: [String; 8],
}

impl PageParameterBank {
    /// Builds the canonical factory [`PageParameterBank`] for `page` on a Korg `nanoKONTROL2`.
    #[must_use]
    pub fn for_page(page: ControllerPage) -> Self {
        let def = nanokontrol2_definition();
        let mut config = DeviceMappingConfig::from_device_definition(&def);
        let mut continuous_targets = BTreeMap::new();
        let mut button_states = BTreeMap::new();

        for b_key in config.entries.keys() {
            if config.entries[b_key].is_button {
                button_states.insert(b_key.clone(), false);
            }
        }

        let (knob_labels, slider_labels) = match page {
            ControllerPage::VoiceDensityAndLevel => {
                for i in 0..8 {
                    let track_num = i + 1;
                    let fam = FAMILY_ORDER[i];
                    let knob_path = format!("track{track_num}.knob");
                    let slider_path = format!("track{track_num}.slider");

                    if let Some(entry) = config.entries.get_mut(&knob_path) {
                        entry.output_number = NK2_DENSITY_KNOB_CCS[i];
                        entry.output_channel = 1;
                        entry.label = Some(format!("{fam} Density"));
                    }
                    if let Some(entry) = config.entries.get_mut(&slider_path) {
                        entry.output_number = NK2_GAIN_SLIDER_CCS[i];
                        entry.output_channel = 1;
                        entry.label = Some(format!("{fam} Level"));
                    }
                    continuous_targets.insert(knob_path, 72); // ~1.0x nominal density
                    continuous_targets.insert(slider_path, 100);
                }
                (
                    std::array::from_fn(|i| format!("{} Dens", FAMILY_ORDER[i])),
                    std::array::from_fn(|i| format!("{} Lvl", FAMILY_ORDER[i])),
                )
            }
            ControllerPage::TimbreMorphAndDecay => {
                for i in 0..8 {
                    let track_num = i + 1;
                    let fam = FAMILY_ORDER[i];
                    let knob_path = format!("track{track_num}.knob");
                    let slider_path = format!("track{track_num}.slider");

                    if let Some(entry) = config.entries.get_mut(&knob_path) {
                        entry.output_number = 24 + (i as u8);
                        entry.output_channel = 1;
                        entry.label = Some(format!("{fam} Morph"));
                    }
                    if let Some(entry) = config.entries.get_mut(&slider_path) {
                        entry.output_number = 8 + (i as u8);
                        entry.output_channel = 1;
                        entry.label = Some(format!("{fam} Decay"));
                    }
                    continuous_targets.insert(knob_path, 64);
                    continuous_targets.insert(slider_path, 84);
                }
                (
                    std::array::from_fn(|i| format!("{} Morph", FAMILY_ORDER[i])),
                    std::array::from_fn(|i| format!("{} Decay", FAMILY_ORDER[i])),
                )
            }
            ControllerPage::PitchSpectraAndNoiseFilter => {
                for i in 0..8 {
                    let track_num = i + 1;
                    let fam = FAMILY_ORDER[i];
                    let knob_path = format!("track{track_num}.knob");
                    let slider_path = format!("track{track_num}.slider");

                    if let Some(entry) = config.entries.get_mut(&knob_path) {
                        // Nord Drum 2 TONE_SPECTRA (CC 30) on per-voice channel 1..8
                        entry.output_number = 30;
                        entry.output_channel = (i as u8) + 1;
                        entry.label = Some(format!("{fam} Spectra"));
                    }
                    if let Some(entry) = config.entries.get_mut(&slider_path) {
                        // Nord Drum 2 NOISE_FILTER_FREQ (CC 14) on per-voice channel 1..8
                        entry.output_number = 14;
                        entry.output_channel = (i as u8) + 1;
                        entry.label = Some(format!("{fam} Filter"));
                    }
                    continuous_targets.insert(knob_path, 52);
                    continuous_targets.insert(slider_path, 92);
                }
                (
                    std::array::from_fn(|i| format!("{} Spec", FAMILY_ORDER[i])),
                    std::array::from_fn(|i| format!("{} Filt", FAMILY_ORDER[i])),
                )
            }
            ControllerPage::GlobalFeelAndMasterFx => {
                let global_knobs = [
                    "Global Dens",
                    "Feel Scale",
                    "Swing Depth",
                    "Humanize",
                    "Dist Drive",
                    "EQ Gain",
                    "Delay Send",
                    "Reverb Mix",
                ];
                let global_sliders = [
                    "Master Bus",
                    "Sub Lift",
                    "Trans Punch",
                    "Stereo Width",
                    "Delay Fdbk",
                    "Reverb Dec",
                    "Limiter Thresh",
                    "Output Trim",
                ];
                let default_knob_vals: [u8; 8] = [72, 64, 48, 56, 20, 64, 32, 40];
                let default_slider_vals: [u8; 8] = [108, 96, 88, 64, 44, 52, 116, 100];

                for i in 0..8 {
                    let track_num = i + 1;
                    let knob_path = format!("track{track_num}.knob");
                    let slider_path = format!("track{track_num}.slider");

                    if let Some(entry) = config.entries.get_mut(&knob_path) {
                        entry.output_number = 58 + (i as u8);
                        entry.output_channel = 10;
                        entry.label = Some(global_knobs[i].to_string());
                    }
                    if let Some(entry) = config.entries.get_mut(&slider_path) {
                        entry.output_number = 70 + (i as u8);
                        entry.output_channel = 10;
                        entry.label = Some(global_sliders[i].to_string());
                    }
                    continuous_targets.insert(knob_path, default_knob_vals[i]);
                    continuous_targets.insert(slider_path, default_slider_vals[i]);
                }
                (
                    global_knobs.map(ToOwned::to_owned),
                    global_sliders.map(ToOwned::to_owned),
                )
            }
        };

        // Light the Solo LED corresponding to the page index (track1.solo .. track4.solo) by default
        let indicator_button = format!("track{}.solo", page.number());
        button_states.insert(indicator_button, true);

        Self {
            page,
            config,
            continuous_targets,
            button_states,
            knob_labels,
            slider_labels,
        }
    }
}

// =============================================================================
// 3. MultiPageLayerManager
// =============================================================================

/// Result of processing a physical MIDI CC event through [`MultiPageLayerManager`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerShortcutOutcome {
    /// The CC updated the `Cycle` (`CC 46`) modifier button state (`held`).
    ModifierStateChanged { cycle_modifier_held: bool },
    /// The CC triggered a page switch to `new_page`, engaging Soft Takeover on `locked_controls`.
    PageSwitched {
        previous_page: ControllerPage,
        new_page: ControllerPage,
        locked_controls: usize,
    },
    /// The CC was not a layer-switching shortcut and should be processed normally on `active_page`.
    PassThrough,
}

/// Manages the 4 switchable controller pages, modifier combinations (`Cycle + Track`,
/// `Cycle + Solo 1..4`, `Marker Left/Right`), automatic Soft Takeover engagement on page switch,
/// and hardware LED synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPageLayerManager {
    /// Device key of the managed controller (e.g. `"0944:0117:noserial:nanoKONTROL2"`).
    pub device_key: String,
    /// Currently active controller page (`Page 1..4`).
    pub active_page: ControllerPage,
    /// Parameter banks for all 4 pages.
    pub pages: [PageParameterBank; 4],
    /// Whether the `Cycle` (`CC 46`) modifier button is currently held down.
    pub cycle_modifier_held: bool,
    /// Whether `Marker Left` (`CC 61`) and `Marker Right` (`CC 62`) directly step pages.
    pub marker_buttons_switch_pages: bool,
    /// Soft Takeover mode enforced whenever switching pages (`Catch` or `Pickup`, default `Pickup`).
    pub takeover_mode_on_switch: SoftTakeoverMode,
    /// Total number of page switches performed during the session.
    pub page_switch_count: u64,
}

impl Default for MultiPageLayerManager {
    fn default() -> Self {
        Self::new("0944:0117:noserial:nanoKONTROL2")
    }
}

impl MultiPageLayerManager {
    /// Creates a new [`MultiPageLayerManager`] initialized on `Page 1: Voice Density & Level`.
    #[must_use]
    pub fn new(device_key: impl Into<String>) -> Self {
        Self {
            device_key: device_key.into(),
            active_page: ControllerPage::VoiceDensityAndLevel,
            pages: ControllerPage::ALL.map(PageParameterBank::for_page),
            cycle_modifier_held: false,
            marker_buttons_switch_pages: true,
            takeover_mode_on_switch: SoftTakeoverMode::Pickup,
            page_switch_count: 0,
        }
    }

    /// Returns a reference to the currently active [`PageParameterBank`].
    #[must_use]
    pub fn active_bank(&self) -> &PageParameterBank {
        &self.pages[self.active_page.index()]
    }

    /// Returns a mutable reference to the currently active [`PageParameterBank`].
    pub fn active_bank_mut(&mut self) -> &mut PageParameterBank {
        &mut self.pages[self.active_page.index()]
    }

    /// Initializes `pipeline` and `led_manager` with the active page's mapping, continuous targets,
    /// and button LED states.
    pub fn apply_active_page_to_pipeline(
        &self,
        pipeline: &mut DeviceMappingPipeline,
        led_manager: &mut MultiDeviceLedManager,
    ) {
        let bank = self.active_bank();
        pipeline.update_config(bank.config.clone());
        pipeline.load_preset_targets(&bank.continuous_targets);
        for (btn_path, &is_on) in &bank.button_states {
            let _ = pipeline.set_button_state(btn_path, is_on);
        }
        self.sync_page_leds(pipeline, led_manager);
    }

    /// Saves the current continuous target values and button states from `pipeline` back into
    /// `self.pages[self.active_page.index()]`.
    pub fn snapshot_current_page_from_pipeline(&mut self, pipeline: &DeviceMappingPipeline) {
        let bank = &mut self.pages[self.active_page.index()];
        let continuous_keys: Vec<String> = bank.continuous_targets.keys().cloned().collect();
        for path in continuous_keys {
            if let Some(status) = pipeline.soft_takeover_status(&path) {
                bank.continuous_targets.insert(path, status.target_value);
            }
        }
        let button_keys: Vec<String> = bank.button_states.keys().cloned().collect();
        for path in button_keys {
            bank.button_states
                .insert(path.clone(), pipeline.button_state(&path));
        }
    }

    /// Counts how many continuous controls (`track1.knob..track8.slider`) in `pipeline` are
    /// currently locked by Soft Takeover (`!status.engaged`).
    #[must_use]
    pub fn locked_control_count(&self, pipeline: &DeviceMappingPipeline) -> usize {
        self.active_bank()
            .continuous_targets
            .keys()
            .filter(|path| {
                pipeline
                    .soft_takeover_status(path)
                    .is_some_and(|st| !st.engaged)
            })
            .count()
    }

    /// Switches to `next_page`, automatically:
    /// 1. Saving the outgoing page's continuous target values and button states from `pipeline`,
    /// 2. Ensuring Soft Takeover (`Catch` or `Pickup`) is active on `pipeline`,
    /// 3. Loading `next_page`'s [`DeviceMappingConfig`] and `continuous_targets` into `pipeline`
    ///    so any knob/slider whose physical position differs from `next_page`'s target is locked
    ///    until caught/picked up,
    /// 4. Updating hardware LEDs via `led_manager`, and
    /// 5. Broadcasting `/nkeditor/layer/select` via `osc_server`.
    ///
    /// Returns the number of continuous controls locked for Soft Takeover on the new page.
    pub fn select_page(
        &mut self,
        next_page: ControllerPage,
        pipeline: &mut DeviceMappingPipeline,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> usize {
        // 1. Save outgoing page state
        self.snapshot_current_page_from_pipeline(pipeline);

        // 2. Switch active page
        self.active_page = next_page;
        self.page_switch_count += 1;

        // 3. Ensure Soft Takeover (`Catch` or `Pickup`) is engaged so physical knobs never jump
        if pipeline.soft_takeover_mode == SoftTakeoverMode::Jump {
            let mode = match self.takeover_mode_on_switch {
                SoftTakeoverMode::Jump => SoftTakeoverMode::Pickup,
                other => other,
            };
            pipeline.set_soft_takeover(mode, pipeline.soft_takeover_threshold);
        }

        // 4. Load new page config, continuous targets, and button states
        let bank = self.active_bank().clone();
        pipeline.update_config(bank.config);
        pipeline.load_preset_targets(&bank.continuous_targets);
        for (btn_path, is_on) in &bank.button_states {
            let _ = pipeline.set_button_state(btn_path, *is_on);
        }

        // 5. Update hardware LEDs
        self.sync_page_leds(pipeline, led_manager);

        // 6. Broadcast OSC notification (/nkeditor/layer/select <page_num> <page_id> <page_title>)
        if let Some(server) = osc_server {
            let _ = server.broadcast_message(OscMessage {
                addr: "/nkeditor/layer/select".to_string(),
                args: vec![
                    OscType::Int(i32::from(next_page.number())),
                    OscType::String(next_page.id().to_string()),
                    OscType::String(next_page.title().to_string()),
                ],
            });
        }

        self.locked_control_count(pipeline)
    }

    /// Synchronizes the hardware LEDs on `led_manager` to reflect the active page's button states
    /// and page indicator LED (`track1.solo..track4.solo` + `transport.cycle` when modifier held).
    pub fn sync_page_leds(
        &self,
        pipeline: &DeviceMappingPipeline,
        led_manager: &mut MultiDeviceLedManager,
    ) {
        let Some(ctrl) = led_manager.controllers.get_mut(&self.device_key) else {
            return;
        };
        if ctrl.mode != LedOperatingMode::Internal {
            ctrl.set_mode(LedOperatingMode::Internal);
        }
        ctrl.sync_from_pipeline(pipeline);
        let _ = ctrl.set_led_by_name("transport.cycle", self.cycle_modifier_held);
    }

    /// Evaluates an incoming Korg `nanoKONTROL2` CC (`cc`, `value`) for page-switching shortcuts:
    /// - `Cycle` (`CC 46`) press/release toggles `self.cycle_modifier_held`
    /// - While `cycle_modifier_held`:
    ///   - `Track Left` (`CC 58`) -> `previous()` page
    ///   - `Track Right` (`CC 59`) -> `next()` page
    ///   - `Solo 1..4` (`CC 32..=35`) -> direct jump to `Page 1..4`
    /// - When `marker_buttons_switch_pages`:
    ///   - `Marker Left` (`CC 61`) -> `previous()` page
    ///   - `Marker Right` (`CC 62`) -> `next()` page
    pub fn handle_nanokontrol2_cc(
        &mut self,
        cc: u8,
        value: u8,
        pipeline: &mut DeviceMappingPipeline,
        led_manager: &mut MultiDeviceLedManager,
        osc_server: &mut Option<OscServer>,
    ) -> LayerShortcutOutcome {
        let is_pressed = value >= 64;

        // 1. Cycle modifier (CC 46)
        if cc == NK2_CC_CYCLE {
            self.cycle_modifier_held = is_pressed;
            if let Some(ctrl) = led_manager.controllers.get_mut(&self.device_key) {
                let _ = ctrl.set_led_by_name("transport.cycle", is_pressed);
            }
            return LayerShortcutOutcome::ModifierStateChanged {
                cycle_modifier_held: is_pressed,
            };
        }

        // Ignore button releases for shortcut triggers
        if !is_pressed {
            return LayerShortcutOutcome::PassThrough;
        }

        let prev_page = self.active_page;
        let target_page = if self.cycle_modifier_held {
            match cc {
                NK2_CC_TRACK_LEFT => Some(prev_page.previous()),
                NK2_CC_TRACK_RIGHT => Some(prev_page.next()),
                32..=35 => Some(ControllerPage::ALL[(cc - 32) as usize]),
                _ => None,
            }
        } else if self.marker_buttons_switch_pages {
            match cc {
                NK2_CC_MARKER_LEFT => Some(prev_page.previous()),
                NK2_CC_MARKER_RIGHT => Some(prev_page.next()),
                _ => None,
            }
        } else {
            None
        };

        if let Some(new_page) = target_page {
            let locked_controls = self.select_page(new_page, pipeline, led_manager, osc_server);
            LayerShortcutOutcome::PageSwitched {
                previous_page: prev_page,
                new_page,
                locked_controls,
            }
        } else {
            LayerShortcutOutcome::PassThrough
        }
    }

    /// updates a single control's mapping entry on the active page and refreshes `pipeline`.
    pub fn update_active_page_entry(
        &mut self,
        entry: ControlMappingEntry,
        pipeline: &mut DeviceMappingPipeline,
    ) {
        self.active_bank_mut()
            .config
            .entries
            .insert(entry.control_path.clone(), entry.clone());
        pipeline.update_entry(entry);
    }
}

// =============================================================================
// 4. Interactive egui Layer Page Selector Bar
// =============================================================================

/// Renders the interactive 4-Page Layer Selector Bar and active 8-track parameter assignment
/// legend inside `ui`.
pub fn render_layer_page_selector_bar(
    ui: &mut Ui,
    layer_manager: &mut MultiPageLayerManager,
    pipeline: &mut DeviceMappingPipeline,
    theme_settings: &ThemeSettings,
    led_manager: &mut MultiDeviceLedManager,
    osc_server: &mut Option<OscServer>,
) {
    let palette = theme_settings.resolved_palette();
    let text_color = ui.visuals().text_color();
    let locked_count = layer_manager.locked_control_count(pipeline);

    egui::Frame::none()
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("MULTI-PAGE LAYER ENGINE [CC61/62 | CYCLE+TRACK]")
                            .font(FontId::monospace(11.5))
                            .strong()
                            .color(palette.accent_strong),
                    );
                    ui.separator();

                    for page in ControllerPage::ALL {
                        let is_active = layer_manager.active_page == page;
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(page.short_tab_label())
                                        .font(FontId::monospace(10.5))
                                        .strong()
                                        .color(if is_active {
                                            palette.accent_strong
                                        } else {
                                            text_color
                                        }),
                                )
                                .stroke(Stroke::new(
                                    if is_active { 2.0 } else { 1.0 },
                                    if is_active {
                                        palette.strip_selection_border
                                    } else {
                                        palette.accent_weak
                                    },
                                ))
                                .rounding(Rounding::ZERO),
                            )
                            .clicked()
                        {
                            layer_manager.select_page(page, pipeline, led_manager, osc_server);
                        }
                    }

                    ui.separator();

                    let mod_text = if layer_manager.cycle_modifier_held {
                        "CYCLE MOD: HELD"
                    } else {
                        "CYCLE MOD: IDLE"
                    };
                    ui.label(
                        RichText::new(mod_text)
                            .font(FontId::monospace(10.0))
                            .color(if layer_manager.cycle_modifier_held {
                                palette.strip_solo_text
                            } else {
                                palette.accent_weak
                            }),
                    );

                    ui.separator();

                    ui.label(
                        RichText::new(format!(
                            "SOFT TAKEOVER ({:?}): {locked_count}/16 LOCKED",
                            pipeline.soft_takeover_mode
                        ))
                        .font(FontId::monospace(10.0))
                        .color(if locked_count > 0 {
                            palette.strip_record_text
                        } else {
                            palette.accent_strong
                        }),
                    );
                });

                ui.add_space(4.0);

                // Active Page 8-Column Knob & Slider Assignment Strip
                let bank = layer_manager.active_bank().clone();
                ui.columns(8, |cols| {
                    for (i, col_ui) in cols.iter_mut().enumerate() {
                        let knob_path = format!("track{}.knob", i + 1);
                        let slider_path = format!("track{}.slider", i + 1);
                        let knob_st = pipeline.soft_takeover_status(&knob_path);
                        let slider_st = pipeline.soft_takeover_status(&slider_path);

                        col_ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "K{}: {} ({})",
                                    i + 1,
                                    bank.knob_labels[i],
                                    knob_st.map_or(0, |s| s.target_value)
                                ))
                                .font(FontId::monospace(9.0))
                                .color(if knob_st.is_some_and(|s| !s.engaged) {
                                    palette.strip_record_text
                                } else {
                                    text_color
                                }),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "S{}: {} ({})",
                                    i + 1,
                                    bank.slider_labels[i],
                                    slider_st.map_or(0, |s| s.target_value)
                                ))
                                .font(FontId::monospace(9.0))
                                .color(if slider_st.is_some_and(|s| !s.engaged) {
                                    palette.strip_record_text
                                } else {
                                    palette.accent_weak
                                }),
                            );
                        });
                    }
                });
            });
        });
}

// =============================================================================
// 5. Unit & Integration Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi_io::RawMidiPacket;
    use rosc::decoder;
    use std::net::UdpSocket;
    use std::time::Duration;

    #[test]
    fn test_four_controller_pages_and_soft_takeover_lock_on_page_switch() {
        let dev_key = "0944:0117:noserial:nanoKONTROL2";
        let def = nanokontrol2_definition();
        let mut pipeline = DeviceMappingPipeline::from_device_definition(dev_key, &def);
        pipeline.set_soft_takeover(SoftTakeoverMode::Pickup, 2);

        let mut led_manager = MultiDeviceLedManager::new();
        led_manager.ensure_controller(dev_key, "nanokontrol2");
        let mut osc_server = None;

        let mut layer_mgr = MultiPageLayerManager::new(dev_key);
        layer_mgr.apply_active_page_to_pipeline(&mut pipeline, &mut led_manager);
        assert_eq!(layer_mgr.active_page, ControllerPage::VoiceDensityAndLevel);

        // Move physical Knob 1 (CC 16) on Page 1 to 72 (matches Page 1 target 72) and then sweep to 110
        let _ = pipeline.evaluate_raw_packet(RawMidiPacket::control_change(1, 16, 72));
        let eval_p1 = pipeline.evaluate_raw_packet(RawMidiPacket::control_change(1, 16, 110));
        assert!(matches!(eval_p1, crate::mapping_engine::MappingEvaluation::Emitted { .. }));
        assert_eq!(
            pipeline.soft_takeover_status("track1.knob").unwrap().target_value,
            110
        );

        // Switch to Page 2 (Timbre Morph & Decay, where track1.knob target is 64 and output CC is 24)
        let locked = layer_mgr.select_page(
            ControllerPage::TimbreMorphAndDecay,
            &mut pipeline,
            &mut led_manager,
            &mut osc_server,
        );
        assert!(locked >= 1, "track1.knob must be locked because physical=110 != target=64");
        assert_eq!(
            layer_mgr.pages[0].continuous_targets.get("track1.knob"),
            Some(&110),
            "Page 1 target value (110) must be preserved when switching to Page 2"
        );

        // Moving physical Knob 1 to 100 on Page 2 must be suppressed by Soft Takeover (no jump!)
        let eval_locked = pipeline.evaluate_raw_packet(RawMidiPacket::control_change(1, 16, 100));
        assert!(
            matches!(
                eval_locked,
                crate::mapping_engine::MappingEvaluation::SoftTakeoverLocked { .. }
            ),
            "Physical knob at 100 must not cause a jump when Page 2 target is 64"
        );

        // Sweeping physical Knob 1 across Page 2's target (64 -> 60) picks up the parameter and emits CC 24!
        let eval_unlocked = pipeline.evaluate_raw_packet(RawMidiPacket::control_change(1, 16, 60));
        if let crate::mapping_engine::MappingEvaluation::Emitted { message, .. } = eval_unlocked {
            assert_eq!(message.output_number, 24);
            assert_eq!(message.output_value, 60);
        } else {
            panic!("Expected Emitted after crossing Page 2 target 64");
        }

        // Switching back to Page 1 restores Page 1's saved target (110) and locks Knob 1 (currently at 60)
        layer_mgr.select_page(
            ControllerPage::VoiceDensityAndLevel,
            &mut pipeline,
            &mut led_manager,
            &mut osc_server,
        );
        let st_p1 = pipeline.soft_takeover_status("track1.knob").unwrap();
        assert_eq!(st_p1.target_value, 110);
        assert_eq!(st_p1.physical_value, 60);
        assert!(!st_p1.engaged);
    }

    #[test]
    fn test_marker_buttons_and_cycle_modifier_combos_switch_pages_and_update_leds_and_osc() {
        let client_sock = UdpSocket::bind("127.0.0.1:0").expect("bind UDP client");
        client_sock
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let client_addr = client_sock.local_addr().unwrap();
        let mut osc_server =
            Some(OscServer::bind("127.0.0.1:0", vec![client_addr]).expect("bind OscServer"));

        let dev_key = "0944:0117:noserial:nanoKONTROL2";
        let def = nanokontrol2_definition();
        let mut pipeline = DeviceMappingPipeline::from_device_definition(dev_key, &def);
        let mut led_manager = MultiDeviceLedManager::new();
        led_manager.ensure_controller(dev_key, "nanokontrol2");

        let mut layer_mgr = MultiPageLayerManager::new(dev_key);
        layer_mgr.apply_active_page_to_pipeline(&mut pipeline, &mut led_manager);

        // 1. Marker Right (CC 62) -> steps Page 1 -> Page 2
        let out = layer_mgr.handle_nanokontrol2_cc(
            NK2_CC_MARKER_RIGHT,
            127,
            &mut pipeline,
            &mut led_manager,
            &mut osc_server,
        );
        assert!(matches!(
            out,
            LayerShortcutOutcome::PageSwitched {
                previous_page: ControllerPage::VoiceDensityAndLevel,
                new_page: ControllerPage::TimbreMorphAndDecay,
                ..
            }
        ));
        assert!(
            led_manager
                .controllers
                .get(dev_key)
                .unwrap()
                .led_state_by_name("track2.solo"),
            "Page 2 indicator LED (track2.solo) should be lit"
        );

        // 2. Hold Cycle (CC 46 = 127) + Track Right (CC 59 = 127) -> steps Page 2 -> Page 3
        layer_mgr.handle_nanokontrol2_cc(
            NK2_CC_CYCLE,
            127,
            &mut pipeline,
            &mut led_manager,
            &mut osc_server,
        );
        assert!(layer_mgr.cycle_modifier_held);
        layer_mgr.handle_nanokontrol2_cc(
            NK2_CC_TRACK_RIGHT,
            127,
            &mut pipeline,
            &mut led_manager,
            &mut osc_server,
        );
        assert_eq!(
            layer_mgr.active_page,
            ControllerPage::PitchSpectraAndNoiseFilter
        );

        // 3. While Cycle is still held, press Solo 4 (CC 35) -> jumps directly to Page 4
        layer_mgr.handle_nanokontrol2_cc(35, 127, &mut pipeline, &mut led_manager, &mut osc_server);
        assert_eq!(layer_mgr.active_page, ControllerPage::GlobalFeelAndMasterFx);
        assert!(
            led_manager
                .controllers
                .get(dev_key)
                .unwrap()
                .led_state_by_name("track4.solo")
        );

        // Release Cycle (CC 46 = 0)
        layer_mgr.handle_nanokontrol2_cc(
            NK2_CC_CYCLE,
            0,
            &mut pipeline,
            &mut led_manager,
            &mut osc_server,
        );
        assert!(!layer_mgr.cycle_modifier_held);

        // Verify UDP OSC /nkeditor/layer/select notifications were broadcast
        let mut addrs = Vec::new();
        let mut buf = [0u8; 2048];
        while let Ok((len, _)) = client_sock.recv_from(&mut buf) {
            if let Ok((_, rosc::OscPacket::Message(msg))) = decoder::decode_udp(&buf[..len]) {
                addrs.push(msg.addr);
            }
        }
        assert!(addrs.contains(&"/nkeditor/layer/select".to_string()));
    }

    #[test]
    fn test_headless_egui_renders_layer_page_selector_bar() {
        let ctx = egui::Context::default();
        let theme_settings = ThemeSettings::default();
        theme_settings.apply_to_ctx(&ctx);

        let dev_key = "0944:0117:noserial:nanoKONTROL2";
        let def = nanokontrol2_definition();
        let mut pipeline = DeviceMappingPipeline::from_device_definition(dev_key, &def);
        let mut led_manager = MultiDeviceLedManager::new();
        led_manager.ensure_controller(dev_key, "nanokontrol2");
        let mut osc_server = None;
        let mut layer_mgr = MultiPageLayerManager::new(dev_key);
        layer_mgr.apply_active_page_to_pipeline(&mut pipeline, &mut led_manager);

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1280.0, 200.0),
            )),
            ..Default::default()
        };

        let full_output = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_layer_page_selector_bar(
                    ui,
                    &mut layer_mgr,
                    &mut pipeline,
                    &theme_settings,
                    &mut led_manager,
                    &mut osc_server,
                );
            });
        });

        assert!(!full_output.shapes.is_empty());
    }
}
