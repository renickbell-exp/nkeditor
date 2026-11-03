//! Multi-Step Undo/Redo Command History Stack & Keyboard-Driven Command Palette (`src/history.rs`).
//!
//! Provides:
//! 1. **Full Multi-Step Undo/Redo Stack ([`CommandHistoryStack`], `Ctrl+Z` / `Ctrl+Y`)**:
//!    Reversible command history covering:
//!    - Control value edits ([`UndoableCommand::ControlValueEdit`])
//!    - Mapping inspector edits ([`UndoableCommand::MappingEntryEdit`])
//!    - Preset loads ([`UndoableCommand::PresetLoad`])
//!    - Randomization & Gaussian drift batches ([`UndoableCommand::RandomizationBatch`])
//!    - `8opPerc` cockpit mutations ([`UndoableCommand::Op8CockpitMutation`])
//! 2. **Keyboard-Driven Command & Control Search Palette ([`CommandPalette`], `Ctrl+K` or `/`)**:
//!    Styled after `twoBowls` (crisp zero-rounding borders, tabular monospace badges,
//!    high-contrast accent selection) for jumping to any device, track, CC, `yaae`/`twoBowls`
//!    theme, factory preset, randomizer preset, or `8opPerc` form/timbre action.

use crate::librarian::create_all_factory_presets;
use crate::mapping_engine::{ControlMappingEntry, DeviceMappingConfig, DeviceMappingPipeline};
use crate::op8_model::{FormDirection, Op8PerformanceModel, FAMILY_ORDER};
use crate::randomizer::{MusicalRandomPreset, RandomizationResult};
use crate::theme::{ThemePreference, ThemeSettings, TwoBowlsBackgroundPreset};
use egui::{Color32, Key, RichText, ScrollArea, Stroke, Ui};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Default maximum number of reversible commands retained in the undo stack.
pub const DEFAULT_MAX_HISTORY_DEPTH: usize = 128;

// =============================================================================
// 1. Reversible Undo/Redo Command Stack (`UndoableCommand` & `CommandHistoryStack`)
// =============================================================================

/// Reversible command stored in [`CommandHistoryStack`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UndoableCommand {
    /// Single continuous control target value edit (`control_path`, `before -> after`).
    ControlValueEdit {
        control_path: String,
        before: u8,
        after: u8,
    },
    /// Single control mapping inspector edit (`ControlMappingEntry` `before -> after`).
    MappingEntryEdit {
        before: ControlMappingEntry,
        after: ControlMappingEntry,
    },
    /// Full preset load replacing `DeviceMappingConfig` and continuous target values.
    PresetLoad {
        before_preset_id: String,
        after_preset_id: String,
        before_config: DeviceMappingConfig,
        after_config: DeviceMappingConfig,
        before_targets: BTreeMap<String, u8>,
        after_targets: BTreeMap<String, u8>,
    },
    /// Batch of control target changes produced by [`crate::randomizer::RandomizerEngine`].
    RandomizationBatch { result: RandomizationResult },
    /// Snapshot of an `8opPerc` cockpit mutation (`before_state -> after_state`).
    Op8CockpitMutation {
        description: String,
        before_state: Box<Op8PerformanceModel>,
        after_state: Box<Op8PerformanceModel>,
    },
}

impl UndoableCommand {
    /// Human-readable summary of the command for UI history badges.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::ControlValueEdit {
                control_path,
                before,
                after,
            } => format!("Set {control_path}: {before} -> {after}"),
            Self::MappingEntryEdit { after, .. } => {
                format!(
                    "Map {} -> {:?} {} Ch{}",
                    after.control_path,
                    after.output_message_type,
                    after.output_number,
                    after.output_channel
                )
            }
            Self::PresetLoad {
                before_preset_id,
                after_preset_id,
                ..
            } => format!("Load Preset: {before_preset_id} -> {after_preset_id}"),
            Self::RandomizationBatch { result } => {
                format!(
                    "Randomize {}: {} controls",
                    result.label,
                    result.control_deltas.len()
                )
            }
            Self::Op8CockpitMutation { description, .. } => {
                format!("8opPerc: {description}")
            }
        }
    }

    /// Reverts this command on `pipeline` and `op8_model`.
    pub fn apply_undo(
        &self,
        pipeline: &mut DeviceMappingPipeline,
        op8_model: &mut Op8PerformanceModel,
    ) {
        match self {
            Self::ControlValueEdit {
                control_path,
                before,
                ..
            } => {
                pipeline.set_target_value(control_path, *before);
            }
            Self::MappingEntryEdit { before, .. } => {
                pipeline.update_entry(before.clone());
            }
            Self::PresetLoad {
                before_config,
                before_targets,
                ..
            } => {
                pipeline.update_config(before_config.clone());
                pipeline.load_preset_targets(before_targets);
            }
            Self::RandomizationBatch { result } => {
                for (path, &(before, _after)) in &result.control_deltas {
                    pipeline.set_target_value(path, before);
                }
            }
            Self::Op8CockpitMutation { before_state, .. } => {
                *op8_model = (**before_state).clone();
            }
        }
    }

    /// Re-applies this command on `pipeline` and `op8_model`.
    pub fn apply_redo(
        &self,
        pipeline: &mut DeviceMappingPipeline,
        op8_model: &mut Op8PerformanceModel,
    ) {
        match self {
            Self::ControlValueEdit {
                control_path,
                after,
                ..
            } => {
                pipeline.set_target_value(control_path, *after);
            }
            Self::MappingEntryEdit { after, .. } => {
                pipeline.update_entry(after.clone());
            }
            Self::PresetLoad {
                after_config,
                after_targets,
                ..
            } => {
                pipeline.update_config(after_config.clone());
                pipeline.load_preset_targets(after_targets);
            }
            Self::RandomizationBatch { result } => {
                for (path, &(_before, after)) in &result.control_deltas {
                    pipeline.set_target_value(path, after);
                }
            }
            Self::Op8CockpitMutation { after_state, .. } => {
                *op8_model = (**after_state).clone();
            }
        }
    }
}

/// Multi-step Undo/Redo command history stack (`Ctrl+Z` / `Ctrl+Y`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandHistoryStack {
    /// Commands available to undo (most recent at the back).
    pub undo_stack: Vec<UndoableCommand>,
    /// Commands available to redo (most recent at the back).
    pub redo_stack: Vec<UndoableCommand>,
    /// Maximum history depth before oldest entries are evicted.
    pub max_depth: usize,
}

impl Default for CommandHistoryStack {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_HISTORY_DEPTH)
    }
}

impl CommandHistoryStack {
    /// Creates an empty [`CommandHistoryStack`] with the given `max_depth`.
    #[must_use]
    pub const fn new(max_depth: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: if max_depth == 0 { 1 } else { max_depth },
        }
    }

    /// Records a newly executed [`UndoableCommand`], clearing the `redo_stack`.
    pub fn push(&mut self, command: UndoableCommand) {
        self.redo_stack.clear();
        self.undo_stack.push(command);
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }

    /// Convenience helper to record a `8opPerc` mutation given `before` and `after` snapshots.
    pub fn record_op8_mutation(
        &mut self,
        description: impl Into<String>,
        before: Op8PerformanceModel,
        after: Op8PerformanceModel,
    ) {
        self.push(UndoableCommand::Op8CockpitMutation {
            description: description.into(),
            before_state: Box::new(before),
            after_state: Box::new(after),
        });
    }

    /// Returns `true` if at least one command can be undone.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if at least one command can be redone.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Undoes the most recent command (`Ctrl+Z`), applying its inverse to `pipeline` and
    /// `op8_model` and moving it onto `redo_stack`.
    pub fn undo(
        &mut self,
        pipeline: &mut DeviceMappingPipeline,
        op8_model: &mut Op8PerformanceModel,
    ) -> Option<String> {
        let cmd = self.undo_stack.pop()?;
        let summary = cmd.summary();
        cmd.apply_undo(pipeline, op8_model);
        self.redo_stack.push(cmd);
        Some(summary)
    }

    /// Redoes the most recently undone command (`Ctrl+Y` or `Ctrl+Shift+Z`), re-applying it
    /// to `pipeline` and `op8_model` and moving it back onto `undo_stack`.
    pub fn redo(
        &mut self,
        pipeline: &mut DeviceMappingPipeline,
        op8_model: &mut Op8PerformanceModel,
    ) -> Option<String> {
        let cmd = self.redo_stack.pop()?;
        let summary = cmd.summary();
        cmd.apply_redo(pipeline, op8_model);
        self.undo_stack.push(cmd);
        Some(summary)
    }
}

// =============================================================================
// 2. Keyboard-Driven Command & Control Search Palette (`Ctrl+K` or `/`)
// =============================================================================

/// Category badge for an entry in the [`CommandPalette`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteCategory {
    Device,
    TrackControl,
    Theme,
    Preset,
    Randomizer,
    Op8Action,
    History,
}

impl PaletteCategory {
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Device => "DEVICE",
            Self::TrackControl => "CONTROL",
            Self::Theme => "THEME",
            Self::Preset => "PRESET",
            Self::Randomizer => "RANDOM",
            Self::Op8Action => "8OPPERC",
            Self::History => "HISTORY",
        }
    }
}

/// Action executed when a [`PaletteItem`] is chosen in the [`CommandPalette`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteAction {
    /// Switch active hardware controller (`"nanokontrol2"` or `"akai-midimix"`).
    SelectDevice(String),
    /// Focus/select a specific control path (`"track1.knob"`, `"transport.play"`, etc.).
    JumpToControl {
        control_path: String,
        cc_or_note: u8,
    },
    /// Switch `yaae-theme` palette.
    SelectYaaeTheme(ThemePreference),
    /// Switch `twoBowls` background preset.
    SelectBackgroundPreset(TwoBowlsBackgroundPreset),
    /// Load a factory or user preset by ID.
    LoadPreset(String),
    /// Apply a musical randomization preset (`SubtleHumanize`, etc.).
    TriggerRandomPreset(MusicalRandomPreset),
    /// Step the 4,096-entry `8opPerc` form bank (`Previous` or `Next`).
    NavigateOp8Form(FormDirection),
    /// Reroll a single `8opPerc` voice lane timbre (`0..8`).
    RerollOp8LaneTimbre(usize),
    /// Advance to the next full 8-voice timbre set (`Cycle CC 46`).
    AdvanceOp8TimbreSet,
    /// Capture an atomic 8-voice preset snapshot (`Marker Set CC 60`).
    CaptureOp8Snapshot,
    /// Execute Undo (`Ctrl+Z`).
    Undo,
    /// Execute Redo (`Ctrl+Y`).
    Redo,
}

/// Single searchable item in the [`CommandPalette`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteItem {
    /// Unique item identifier.
    pub id: String,
    /// Category badge (`DEVICE`, `CONTROL`, `THEME`, `PRESET`, `8OPPERC`, etc.).
    pub category: PaletteCategory,
    /// Display title shown in the palette list.
    pub title: String,
    /// Secondary subtitle / shortcut / CC readout.
    pub subtitle: String,
    /// Action dispatched when selected.
    pub action: PaletteAction,
}

/// Keyboard-driven Command & Control Search Palette (`Ctrl+K` or `/`) styled after `twoBowls`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPalette {
    /// Whether the palette modal/bar is currently open.
    pub is_open: bool,
    /// Current search query entered by the user.
    pub query: String,
    /// Currently highlighted row index inside `filtered_items()`.
    pub selected_index: usize,
    /// Complete catalog of searchable commands, controls, themes, presets, and `8opPerc` actions.
    pub catalog: Vec<PaletteItem>,
    /// Most recently executed [`PaletteAction`].
    pub last_executed_action: Option<PaletteAction>,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    /// Builds a [`CommandPalette`] pre-populated with all devices, 8 tracks & CCs,
    /// 15 `yaae-theme` palettes, 4 `twoBowls` background presets, 14 factory presets,
    /// 4 musical randomizer presets, Undo/Redo, and `8opPerc` form/timbre actions.
    #[must_use]
    pub fn new() -> Self {
        let mut catalog = Vec::new();

        // 1. Devices
        catalog.push(PaletteItem {
            id: "dev-nanokontrol2".into(),
            category: PaletteCategory::Device,
            title: "Switch Device: Korg nanoKONTROL2".into(),
            subtitle: "USB 0944:0117 (8 tracks + transport, 28 CC LEDs)".into(),
            action: PaletteAction::SelectDevice("nanokontrol2".into()),
        });
        catalog.push(PaletteItem {
            id: "dev-akai-midimix".into(),
            category: PaletteCategory::Device,
            title: "Switch Device: Akai MIDI Mix".into(),
            subtitle: "USB 09e8:0031 (8 tracks x 3 knobs + master, 24 Note LEDs)".into(),
            action: PaletteAction::SelectDevice("akai-midimix".into()),
        });

        // 2. Tracks & Controls (CCs)
        for t in 1..=8u8 {
            let fam = FAMILY_ORDER[(t - 1) as usize].to_ascii_uppercase();
            let knob_cc = 15 + t;
            let slider_cc = t - 1;
            catalog.push(PaletteItem {
                id: format!("ctrl-track{t}-knob"),
                category: PaletteCategory::TrackControl,
                title: format!("Jump to Track {t} Knob ({fam} Density)"),
                subtitle: format!("track{t}.knob — CC {knob_cc}"),
                action: PaletteAction::JumpToControl {
                    control_path: format!("track{t}.knob"),
                    cc_or_note: knob_cc,
                },
            });
            catalog.push(PaletteItem {
                id: format!("ctrl-track{t}-slider"),
                category: PaletteCategory::TrackControl,
                title: format!("Jump to Track {t} Slider ({fam} Gain/Macro)"),
                subtitle: format!("track{t}.slider — CC {slider_cc}"),
                action: PaletteAction::JumpToControl {
                    control_path: format!("track{t}.slider"),
                    cc_or_note: slider_cc,
                },
            });
        }

        // 3. All 15 yaae-theme palettes + 4 twoBowls background presets
        let all_themes = [
            ThemePreference::Default,
            ThemePreference::Light,
            ThemePreference::Dark,
            ThemePreference::SolarizedDark,
            ThemePreference::Monokai,
            ThemePreference::Dracula,
            ThemePreference::Nord,
            ThemePreference::GruvboxDark,
            ThemePreference::LightGrayscale,
            ThemePreference::DarkGrayscale,
            ThemePreference::HighContrastBw,
            ThemePreference::HighContrastColor,
            ThemePreference::HighContrastLight,
            ThemePreference::CreamLowContrast,
            ThemePreference::MediumContrast,
        ];
        for pref in all_themes {
            let name = format!("{pref:?}");
            catalog.push(PaletteItem {
                id: format!("theme-{}", name.to_ascii_lowercase()),
                category: PaletteCategory::Theme,
                title: format!("Theme: {name}"),
                subtitle: format!("yaae-theme [{name}]"),
                action: PaletteAction::SelectYaaeTheme(pref),
            });
        }
        for &bg in TwoBowlsBackgroundPreset::all() {
            catalog.push(PaletteItem {
                id: format!("bg-{}", bg.display_name().to_ascii_lowercase().replace(' ', "-")),
                category: PaletteCategory::Theme,
                title: format!("Background: twoBowls {}", bg.display_name()),
                subtitle: format!("Canvas {}", bg.hex()),
                action: PaletteAction::SelectBackgroundPreset(bg),
            });
        }

        // 4. All 14 Factory Presets
        for preset in create_all_factory_presets() {
            catalog.push(PaletteItem {
                id: format!("preset-{}", preset.metadata.id),
                category: PaletteCategory::Preset,
                title: format!("Load Preset: {}", preset.metadata.name),
                subtitle: format!("{} [{}]", preset.metadata.group, preset.metadata.device_id),
                action: PaletteAction::LoadPreset(preset.metadata.id),
            });
        }

        // 5. Musical Randomizer Presets
        for rp in MusicalRandomPreset::ALL {
            catalog.push(PaletteItem {
                id: format!("rand-{:?}", rp).to_ascii_lowercase(),
                category: PaletteCategory::Randomizer,
                title: format!("Randomize: {}", rp.title()),
                subtitle: "Gaussian Drift & Lock Shield Engine".into(),
                action: PaletteAction::TriggerRandomPreset(rp),
            });
        }

        // 6. 8opPerc Form & Timbre Actions
        catalog.push(PaletteItem {
            id: "op8-form-next".into(),
            category: PaletteCategory::Op8Action,
            title: "8opPerc: Next Form in 4,096 Bank".into(),
            subtitle: "Track Right (CC 59) / /nkeditor/op8/form".into(),
            action: PaletteAction::NavigateOp8Form(FormDirection::Next),
        });
        catalog.push(PaletteItem {
            id: "op8-form-prev".into(),
            category: PaletteCategory::Op8Action,
            title: "8opPerc: Previous Form in 4,096 Bank".into(),
            subtitle: "Track Left (CC 58) / /nkeditor/op8/form".into(),
            action: PaletteAction::NavigateOp8Form(FormDirection::Previous),
        });
        catalog.push(PaletteItem {
            id: "op8-timbre-set".into(),
            category: PaletteCategory::Op8Action,
            title: "8opPerc: Advance Full 8-Voice Timbre Set".into(),
            subtitle: "Cycle (CC 46) / Respects Locked Lanes".into(),
            action: PaletteAction::AdvanceOp8TimbreSet,
        });
        catalog.push(PaletteItem {
            id: "op8-snapshot-save".into(),
            category: PaletteCategory::Op8Action,
            title: "8opPerc: Capture 8-Voice Preset Pool Snapshot".into(),
            subtitle: "Marker Set (CC 60) -> nanokontrol2-random-preset-pool.jsonl".into(),
            action: PaletteAction::CaptureOp8Snapshot,
        });
        for (idx, fam) in FAMILY_ORDER.iter().enumerate() {
            catalog.push(PaletteItem {
                id: format!("op8-reroll-{fam}"),
                category: PaletteCategory::Op8Action,
                title: format!(
                    "8opPerc: Reroll Lane {} ({}) Timbre",
                    idx + 1,
                    fam.to_ascii_uppercase()
                ),
                subtitle: format!("Rec {} (CC {})", idx + 1, 64 + idx),
                action: PaletteAction::RerollOp8LaneTimbre(idx),
            });
        }

        // 7. History Undo / Redo
        catalog.push(PaletteItem {
            id: "history-undo".into(),
            category: PaletteCategory::History,
            title: "History: Undo Last Command".into(),
            subtitle: "Ctrl+Z".into(),
            action: PaletteAction::Undo,
        });
        catalog.push(PaletteItem {
            id: "history-redo".into(),
            category: PaletteCategory::History,
            title: "History: Redo Command".into(),
            subtitle: "Ctrl+Y / Ctrl+Shift+Z".into(),
            action: PaletteAction::Redo,
        });

        Self {
            is_open: false,
            query: String::new(),
            selected_index: 0,
            catalog,
            last_executed_action: None,
        }
    }

    /// Opens the command palette and resets the selection cursor.
    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_index = 0;
    }

    /// Closes the command palette.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Updates the search query and clamps `selected_index`.
    pub fn set_query(&mut self, q: impl Into<String>) {
        self.query = q.into();
        self.selected_index = 0;
    }

    /// Returns all [`PaletteItem`]s matching `self.query` (supports multi-token case-insensitive matching).
    #[must_use]
    pub fn filtered_items(&self) -> Vec<&PaletteItem> {
        let tokens: Vec<String> = self
            .query
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect();
        if tokens.is_empty() {
            return self.catalog.iter().collect();
        }
        self.catalog
            .iter()
            .filter(|item| {
                let hay_title = item.title.to_ascii_lowercase();
                let hay_sub = item.subtitle.to_ascii_lowercase();
                let hay_badge = item.category.badge().to_ascii_lowercase();
                tokens.iter().all(|tok| {
                    hay_title.contains(tok) || hay_sub.contains(tok) || hay_badge.contains(tok)
                })
            })
            .collect()
    }

    /// Moves the selection cursor down (`+1`) or up (`-1`) within `filtered_items()`.
    pub fn move_selection(&mut self, delta: isize) {
        let len = self.filtered_items().len();
        if len == 0 {
            self.selected_index = 0;
            return;
        }
        let next = (self.selected_index as isize + delta).rem_euclid(len as isize);
        self.selected_index = next as usize;
    }

    /// Executes the currently highlighted item (or the item at `index`) and records it in
    /// `last_executed_action`.
    pub fn execute_selected(
        &mut self,
        history: &mut CommandHistoryStack,
        pipeline: &mut DeviceMappingPipeline,
        op8_model: &mut Op8PerformanceModel,
        theme_settings: &mut ThemeSettings,
    ) -> Option<PaletteAction> {
        let items = self.filtered_items();
        let chosen = items.get(self.selected_index).or_else(|| items.first())?;
        let action = chosen.action.clone();

        match &action {
            PaletteAction::SelectYaaeTheme(pref) => {
                theme_settings.preference = *pref;
            }
            PaletteAction::SelectBackgroundPreset(bg) => {
                theme_settings.background_preset = Some(*bg);
            }
            PaletteAction::NavigateOp8Form(dir) => {
                let before = op8_model.clone();
                op8_model.form_bank.step(*dir);
                history.record_op8_mutation(format!("Navigate Form {dir:?}"), before, op8_model.clone());
            }
            PaletteAction::RerollOp8LaneTimbre(lane_idx) => {
                let before = op8_model.clone();
                let cc = 64 + (*lane_idx as u8);
                let _ = op8_model.apply_nanokontrol2_cc(cc, 127);
                history.record_op8_mutation(
                    format!("Reroll Lane {}", lane_idx + 1),
                    before,
                    op8_model.clone(),
                );
            }
            PaletteAction::AdvanceOp8TimbreSet => {
                let before = op8_model.clone();
                let _ = op8_model.apply_nanokontrol2_cc(46, 127);
                history.record_op8_mutation("Advance Timbre Set", before, op8_model.clone());
            }
            PaletteAction::CaptureOp8Snapshot => {
                let _ = op8_model.apply_nanokontrol2_cc(60, 127);
            }
            PaletteAction::Undo => {
                let _ = history.undo(pipeline, op8_model);
            }
            PaletteAction::Redo => {
                let _ = history.redo(pipeline, op8_model);
            }
            _ => {}
        }

        self.last_executed_action = Some(action.clone());
        self.is_open = false;
        Some(action)
    }
}

// =============================================================================
// 3. Interactive egui Undo/Redo Toolbar & twoBowls Command Palette View
// =============================================================================

/// Renders the Undo/Redo toolbar (`Ctrl+Z` / `Ctrl+Y`) and the keyboard-driven
/// `twoBowls`-styled Command & Control Search Palette (`Ctrl+K` or `/`).
pub fn render_history_and_command_palette(
    ui: &mut Ui,
    history: &mut CommandHistoryStack,
    palette_ui: &mut CommandPalette,
    pipeline: &mut DeviceMappingPipeline,
    op8_model: &mut Op8PerformanceModel,
    theme_settings: &mut ThemeSettings,
) -> Option<PaletteAction> {
    let accent_palette = theme_settings.resolved_palette();
    let panel_fill = theme_settings.build_visuals().panel_fill;
    let mut triggered_action: Option<PaletteAction> = None;

    // Handle global keyboard shortcuts (Ctrl+Z, Ctrl+Y, Ctrl+K, Escape)
    ui.input(|input| {
        if input.modifiers.ctrl && input.key_pressed(Key::Z) && !input.modifiers.shift {
            let _ = history.undo(pipeline, op8_model);
        } else if (input.modifiers.ctrl && input.key_pressed(Key::Y))
            || (input.modifiers.ctrl && input.modifiers.shift && input.key_pressed(Key::Z))
        {
            let _ = history.redo(pipeline, op8_model);
        } else if (input.modifiers.ctrl && input.key_pressed(Key::K))
            || (!palette_ui.is_open && input.key_pressed(Key::Slash))
        {
            palette_ui.is_open = !palette_ui.is_open;
        } else if palette_ui.is_open && input.key_pressed(Key::Escape) {
            palette_ui.close();
        } else if palette_ui.is_open && input.key_pressed(Key::ArrowDown) {
            palette_ui.move_selection(1);
        } else if palette_ui.is_open && input.key_pressed(Key::ArrowUp) {
            palette_ui.move_selection(-1);
        }
    });

    egui::Frame::none()
        .fill(panel_fill)
        .stroke(Stroke::new(1.0, accent_palette.accent_weak))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // 1. Undo/Redo + Command Palette Trigger Bar
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("HISTORY & COMMAND PALETTE")
                        .strong()
                        .color(accent_palette.accent),
                );
                ui.separator();

                let undo_label = if let Some(last) = history.undo_stack.last() {
                    format!("↶ Undo ({}) [Ctrl+Z]", last.summary())
                } else {
                    "↶ Undo [Ctrl+Z]".to_string()
                };
                if ui
                    .add_enabled(history.can_undo(), egui::Button::new(RichText::new(undo_label).small()))
                    .clicked()
                {
                    let _ = history.undo(pipeline, op8_model);
                }

                let redo_label = if let Some(last) = history.redo_stack.last() {
                    format!("↷ Redo ({}) [Ctrl+Y]", last.summary())
                } else {
                    "↷ Redo [Ctrl+Y]".to_string()
                };
                if ui
                    .add_enabled(history.can_redo(), egui::Button::new(RichText::new(redo_label).small()))
                    .clicked()
                {
                    let _ = history.redo(pipeline, op8_model);
                }

                ui.separator();
                if ui
                    .button(
                        RichText::new("⌘ Command Search Palette [Ctrl+K / '/']")
                            .small()
                            .strong(),
                    )
                    .clicked()
                {
                    palette_ui.is_open = !palette_ui.is_open;
                }
            });

            // 2. Expanded twoBowls Command & Control Search Palette
            if palette_ui.is_open {
                ui.add_space(6.0);
                egui::Frame::none()
                    .fill(Color32::from_gray(18))
                    .stroke(Stroke::new(1.5, accent_palette.accent))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("/>")
                                    .monospace()
                                    .strong()
                                    .color(accent_palette.accent_strong),
                            );
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut palette_ui.query)
                                    .desired_width(320.0)
                                    .hint_text("Type device, CC, track, theme, preset, or 8opPerc command..."),
                            );
                            if resp.changed() {
                                palette_ui.selected_index = 0;
                            }
                            if ui.small_button("Run [Enter]").clicked() {
                                triggered_action = palette_ui.execute_selected(
                                    history,
                                    pipeline,
                                    op8_model,
                                    theme_settings,
                                );
                            }
                        });

                        let rows: Vec<(usize, String, String, String)> = palette_ui
                            .filtered_items()
                            .into_iter()
                            .enumerate()
                            .take(12)
                            .map(|(i, item)| {
                                (
                                    i,
                                    item.category.badge().to_string(),
                                    item.title.clone(),
                                    item.subtitle.clone(),
                                )
                            })
                            .collect();

                        ScrollArea::vertical()
                            .id_salt("twobowls_command_palette_scroll")
                            .max_height(150.0)
                            .show(ui, |ui| {
                                for (idx, badge, title, subtitle) in rows {
                                    let is_sel = idx == palette_ui.selected_index;
                                    let text = format!("[{badge:<7}]  {title:<42}  {subtitle}");
                                    if ui
                                        .selectable_label(
                                            is_sel,
                                            RichText::new(text).monospace().small(),
                                        )
                                        .clicked()
                                    {
                                        palette_ui.selected_index = idx;
                                        triggered_action = palette_ui.execute_selected(
                                            history,
                                            pipeline,
                                            op8_model,
                                            theme_settings,
                                        );
                                    }
                                }
                            });
                    });
            }
        });

    triggered_action
}

// =============================================================================
// 4. Comprehensive Unit & Headless egui Tests (Randomizer, History & Palette)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping_engine::DeviceMappingConfig;
    use crate::randomizer::{ render_randomizer_panel, RandomizationScope, RandomizerEngine };

    #[test]
    fn randomizer_supports_global_selected_track_lock_shields_and_gaussian_drift() {
        let mut pipeline =
            DeviceMappingPipeline::new("nanokontrol2", DeviceMappingConfig::default_nanokontrol2());
        let mut op8 = Op8PerformanceModel::new();
        let mut randomizer = RandomizerEngine::new(42);

        // Initialize all continuous targets to 64
        for t in 1..=8 {
            pipeline.set_target_value(&format!("track{t}.knob"), 64);
            pipeline.set_target_value(&format!("track{t}.slider"), 64);
        }

        // Shield track1.knob and lock lane 0 (kick)
        randomizer.set_control_lock("track2.slider", true);
        randomizer.locked_lanes[0] = true;
        randomizer.set_drift_percent(25.0);

        let batch = randomizer.mutate_pipeline(&mut pipeline);
        // Track 1 controls and track2.slider must be completely untouched
        assert!(!batch.control_deltas.contains_key("track1.knob"));
        assert!(!batch.control_deltas.contains_key("track1.slider"));
        assert!(!batch.control_deltas.contains_key("track2.slider"));
        assert_eq!(
            pipeline
                .soft_takeover_status("track1.knob")
                .expect("t1 knob")
                .target_value,
            64
        );
        assert_eq!(
            pipeline
                .soft_takeover_status("track2.slider")
                .expect("t2 slider")
                .target_value,
            64
        );
        // Unlocked controls (track2.knob, track3.knob, etc.) were mutated within [0, 127]
        assert!(batch.control_deltas.contains_key("track2.knob"));
        assert!(batch.control_deltas.contains_key("track3.knob"));

        // SelectedTrack(4) scope only mutates track 4 (lane index 3)
        randomizer.scope = RandomizationScope::SelectedTrack(4);
        randomizer.apply_musical_preset(MusicalRandomPreset::HarmonicTimbreExplore);
        let op8_batch = randomizer.mutate_op8_model(&mut op8);
        assert_eq!(op8_batch.mutated_lanes, vec![3]);
        assert_eq!(op8.lanes[3].reroll_count, 1);
        assert_eq!(op8.lanes[0].reroll_count, 0);
    }

    #[test]
    fn multi_step_undo_redo_stack_reverts_and_reapplies_values_mappings_presets_randomizations_and_op8() {
        let mut pipeline =
            DeviceMappingPipeline::new("nanokontrol2", DeviceMappingConfig::default_nanokontrol2());
        let mut op8 = Op8PerformanceModel::new();
        let mut history = CommandHistoryStack::new(32);

        // Step 1: Control value edit on track1.knob (64 -> 112)
        pipeline.set_target_value("track1.knob", 64);
        pipeline.set_target_value("track1.knob", 112);
        history.push(UndoableCommand::ControlValueEdit {
            control_path: "track1.knob".to_string(),
            before: 64,
            after: 112,
        });

        // Step 2: Mapping entry edit on track1.knob (CC 16 -> CC 74 on Ch 3)
        let before_entry = pipeline.config.entries["track1.knob"].clone();
        let mut after_entry = before_entry.clone();
        after_entry.output_number = 74;
        after_entry.output_channel = 3;
        pipeline.update_entry(after_entry.clone());
        history.push(UndoableCommand::MappingEntryEdit {
            before: before_entry,
            after: after_entry,
        });

        // Step 3: Randomization batch across unlocked controls
        let mut randomizer = RandomizerEngine::new(99);
        randomizer.locked_lanes[0] = true; // keep track1.knob at 112
        let batch = randomizer.mutate_pipeline(&mut pipeline);
        let mutated_t2 = batch.control_deltas["track2.knob"].1;
        history.push(UndoableCommand::RandomizationBatch { result: batch });

        // Step 4: 8opPerc cockpit mutation (navigate form 0 -> 1)
        let before_op8 = op8.clone();
        op8.form_bank.step(FormDirection::Next);
        assert_eq!(op8.form_bank.active_index, 1);
        history.record_op8_mutation("Next Form", before_op8, op8.clone());

        // Undo Step 4 (8opPerc form 1 -> 0)
        assert!(history.undo(&mut pipeline, &mut op8).is_some());
        assert_eq!(op8.form_bank.active_index, 0);

        // Undo Step 3 (Randomization batch reverted)
        assert!(history.undo(&mut pipeline, &mut op8).is_some());
        assert_eq!(
            pipeline
                .soft_takeover_status("track2.knob")
                .expect("t2")
                .target_value,
            batch_before(&history)
        );

        // Undo Step 2 (Mapping CC 74 Ch 3 -> CC 16 Ch 1)
        assert!(history.undo(&mut pipeline, &mut op8).is_some());
        assert_eq!(pipeline.config.entries["track1.knob"].output_number, 16);
        assert_eq!(pipeline.config.entries["track1.knob"].output_channel, 1);

        // Undo Step 1 (Control value 112 -> 64)
        assert!(history.undo(&mut pipeline, &mut op8).is_some());
        assert_eq!(
            pipeline
                .soft_takeover_status("track1.knob")
                .expect("t1")
                .target_value,
            64
        );
        assert!(!history.can_undo());

        // Now Redo all 4 steps and verify exact forward restoration
        for _ in 0..4 {
            assert!(history.redo(&mut pipeline, &mut op8).is_some());
        }
        assert_eq!(
            pipeline
                .soft_takeover_status("track1.knob")
                .expect("t1")
                .target_value,
            112
        );
        assert_eq!(pipeline.config.entries["track1.knob"].output_number, 74);
        assert_eq!(
            pipeline
                .soft_takeover_status("track2.knob")
                .expect("t2")
                .target_value,
            mutated_t2
        );
        assert_eq!(op8.form_bank.active_index, 1);
    }

    fn batch_before(history: &CommandHistoryStack) -> u8 {
        for cmd in &history.redo_stack {
            if let UndoableCommand::RandomizationBatch { result } = cmd {
                return result.control_deltas["track2.knob"].0;
            }
        }
        64
    }

    #[test]
    fn command_palette_searches_and_executes_devices_controls_themes_and_op8_actions() {
        let mut palette = CommandPalette::new();
        let mut history = CommandHistoryStack::new(16);
        let mut pipeline =
            DeviceMappingPipeline::new("nanokontrol2", DeviceMappingConfig::default_nanokontrol2());
        let mut op8 = Op8PerformanceModel::new();
        let mut theme = ThemeSettings::default();

        // Search for "Cream" twoBowls background
        palette.open();
        palette.set_query("twobowls cream");
        let matches = palette.filtered_items();
        assert_eq!(matches.len(), 1);
        let act = palette
            .execute_selected(&mut history, &mut pipeline, &mut op8, &mut theme)
            .expect("executed");
        assert_eq!(
            act,
            PaletteAction::SelectBackgroundPreset(TwoBowlsBackgroundPreset::Cream)
        );
        assert_eq!(
            theme.background_preset,
            Some(TwoBowlsBackgroundPreset::Cream)
        );

        // Search for "Next Form" 8opPerc action and execute -> pushes onto undo history!
        palette.open();
        palette.set_query("next form 4,096");
        let act2 = palette
            .execute_selected(&mut history, &mut pipeline, &mut op8, &mut theme)
            .expect("executed next form");
        assert_eq!(act2, PaletteAction::NavigateOp8Form(FormDirection::Next));
        assert_eq!(op8.form_bank.active_index, 1);
        assert!(history.can_undo());
    }

    #[test]
    fn headless_egui_renders_randomizer_history_and_command_palette_without_panic() {
        let ctx = egui::Context::default();
        let mut randomizer = RandomizerEngine::new(7);
        let mut history = CommandHistoryStack::new(16);
        let mut palette = CommandPalette::new();
        palette.open();
        palette.set_query("cc 16");
        let mut pipeline =
            DeviceMappingPipeline::new("nanokontrol2", DeviceMappingConfig::default_nanokontrol2());
        let mut op8 = Op8PerformanceModel::new();
        let mut theme = ThemeSettings::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_randomizer_panel(ui, &mut randomizer, &mut pipeline, &mut op8, &theme);
                let _ = render_history_and_command_palette(
                    ui,
                    &mut history,
                    &mut palette,
                    &mut pipeline,
                    &mut op8,
                    &mut theme,
                );
            });
        });
    }
}
