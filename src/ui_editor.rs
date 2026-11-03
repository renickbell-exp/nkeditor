//! Interactive Rust `egui` Controller Surface & Visual Mapping Inspector (`src/ui_editor.rs`).
//!
//! Styled with `yaae-theme` and `twoBowls` aesthetics (`egui::Rounding::ZERO` crisp borders,
//! high-contrast tabular readouts, and [`AccentPalette`] highlights).
//!
//! Renders three coordinated UI regions:
//! 1. **Multi-Device Header Bar**: Displays all connected and remembered MIDI controllers with
//!    USB [`HardwareFingerprint`] badges, online/offline connection badges, active ALSA addresses,
//!    quick `yaae-theme` (15 themes) and `twoBowls` background (4 presets) selectors, Decimal/Hex
//!    readout toggle, and virtual MIDI output port status.
//! 2. **Purpose-Built Visual Controller Surfaces**:
//!    - **Korg nanoKONTROL2**: Left transport deck (`Cycle`, `Marker Set/< />`, `Track < />`,
//!      `Rewind`, `Forward`, `Stop`, `Play`, `Record`) + 8 track channel strips, each with a
//!      custom vector rotary knob (rendering both the active software target arc and the physical
//!      Soft Takeover ghost ring + delta indicator), LED-lit `S` / `M` / `R` buttons, and a
//!      custom vertical vector slider.
//!    - **Akai MIDI Mix**: 8 track channel strips (`Knob 1`, `Knob 2`, `Knob 3` rotary rows +
//!      `Mute`, `Solo`, `Rec Arm` LED buttons + vertical `Fader`) + right `Master` strip
//!      (`Bank Left`, `Bank Right`, `Solo Master`, `Master Fader`).
//! 3. **Inspector Side Panel**: Edits any selected control's CC/Note output number, MIDI channel
//!    (`1..=16`), custom label, [`ResponseCurve`] (`Linear`, `Exponential`, `Logarithmic`,
//!    `SCurve`), `min_value`/`max_value` range, polarity inversion, `Toggle`/`Momentary` button
//!    behavior, `on_value`/`off_value`, and one-click **Nord Drum 2 Desktop** parameter assignment
//!    across all 32 synthesis and Global FX CCs.

use crate::devices::nord_drum_2::{self, NordDrum2ParamDef, NordDrum2Section};
use crate::devices::{
    akai_midimix_definition, nanokontrol2_definition, ButtonBehavior, DeviceDefinition,
    MidiMessageType,
};
use crate::led_controller::{DeviceLedController, LedOperatingMode, MultiDeviceLedManager};
use crate::mapping_engine::{
    ControlMappingEntry, DeviceMappingPipeline, MultiDeviceMappingEngine, ResponseCurve,
    SoftTakeoverMode, SoftTakeoverStatus, TakeoverDirection,
};
use crate::midi_io::{DeviceConnectionState, MultiDeviceManager};
use crate::theme::{AccentPalette, ThemePreference, ThemeSettings, TwoBowlsBackgroundPreset};
use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Rounding, Sense, Shape, Stroke, Ui, Vec2,
};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Numeric readout format for MIDI CC/Note numbers and values in the editor UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ValueDisplayFormat {
    /// Standard base-10 integer (`0..127`).
    #[default]
    Decimal,
    /// Two-digit uppercase hexadecimal (`00..7F`).
    Hexadecimal,
}

impl ValueDisplayFormat {
    /// Formats a 7-bit MIDI byte (`0..=127`) according to this display format.
    #[must_use]
    pub fn format_u8(self, value: u8) -> String {
        match self {
            Self::Decimal => format!("{value:3}"),
            Self::Hexadecimal => format!("0x{value:02X}"),
        }
    }
}

/// Interactive state for the `nkEditor4` Controller Surface & Inspector UI.
#[derive(Debug, Clone)]
pub struct ControllerEditorUiState {
    /// Currently selected device key (either a canonical `HardwareFingerprint` or definition ID
    /// like `"nanokontrol2"` / `"akai-midimix"`).
    pub active_device_key: String,
    /// Currently active hardware layout definition ID (`"nanokontrol2"` or `"akai-midimix"`).
    pub active_definition_id: String,
    /// Currently selected control path for the Inspector Side Panel (e.g. `"track1.knob"`).
    pub selected_control_path: String,
    /// Numeric display format (`Decimal` or `Hexadecimal`).
    pub value_format: ValueDisplayFormat,
    /// Whether the Virtual MIDI Output port (`nkEditor4 Out`) is active.
    pub virtual_port_active: bool,
    /// Virtual MIDI Output port name.
    pub virtual_port_name: String,
    /// Selected Nord Drum 2 target pad (`1..=6`) in the Inspector quick-assign helper.
    pub inspector_nd2_pad: u8,
    /// Selected index into [`nord_drum_2::ALL_PARAMS`] (`0..32`) in the Inspector quick-assign helper.
    pub inspector_nd2_param_idx: usize,
    /// Count of headless/interactive frames rendered (useful for telemetry and tests).
    pub rendered_frame_count: u64,
}

impl Default for ControllerEditorUiState {
    fn default() -> Self {
        Self {
            active_device_key: "nanokontrol2".to_string(),
            active_definition_id: "nanokontrol2".to_string(),
            selected_control_path: "track1.knob".to_string(),
            value_format: ValueDisplayFormat::Decimal,
            virtual_port_active: true,
            virtual_port_name: "nkEditor4 Out".to_string(),
            inspector_nd2_pad: 1,
            inspector_nd2_param_idx: 0,
            rendered_frame_count: 0,
        }
    }
}

impl ControllerEditorUiState {
    /// Selects a device tab by its `device_key` and `definition_id`, resetting `selected_control_path`
    /// to a valid default control on that device if needed.
    pub fn select_device(&mut self, device_key: impl Into<String>, definition_id: &str) {
        self.active_device_key = device_key.into();
        self.active_definition_id = definition_id.to_string();
        if definition_id == "akai-midimix" && self.selected_control_path == "track1.knob" {
            self.selected_control_path = "track1.knob1".to_string();
        } else if definition_id == "nanokontrol2" && self.selected_control_path == "track1.knob1" {
            self.selected_control_path = "track1.knob".to_string();
        }
    }

    /// Assigns a **Nord Drum 2 Desktop** parameter (`nord_drum_2::ALL_PARAMS[param_idx]`) to the
    /// currently selected control in `pipeline`.
    ///
    /// - For per-pad sections (`Tone`, `Noise`, `Click`, `Mix`), sets `output_channel = pad.clamp(1, 6)`.
    /// - For `GlobalFx`, sets `output_channel = nord_drum_2::channels::GLOBAL_FX` (`10`).
    /// - Sets `output_message_type = MidiMessageType::Cc`, `output_number = param.cc`, and a
    ///   descriptive `label`.
    pub fn assign_nord_drum_2_parameter(
        &self,
        pipeline: &mut DeviceMappingPipeline,
        pad: u8,
        param_idx: usize,
    ) -> Option<ControlMappingEntry> {
        let param: &NordDrum2ParamDef = nord_drum_2::ALL_PARAMS.get(param_idx)?;
        let mut entry = pipeline
            .config
            .entries
            .get(&self.selected_control_path)?
            .clone();

        entry.output_message_type = MidiMessageType::Cc;
        entry.output_number = param.cc;
        if param.section == NordDrum2Section::GlobalFx {
            entry.output_channel = nord_drum_2::channels::GLOBAL_FX;
            entry.label = Some(format!("ND2 FX {}", param.name));
        } else {
            let clamped_pad = pad.clamp(1, 6);
            entry.output_channel = clamped_pad;
            entry.label = Some(format!("ND2 P{clamped_pad} {}", param.name));
        }

        pipeline.update_entry(entry.clone());
        Some(entry)
    }
}

// =============================================================================
// 1. Multi-Device Header Bar
// =============================================================================

/// Renders the top Multi-Device Header Bar showing connected/remembered hardware controllers
/// with USB fingerprint badges, connection indicators, quick `yaae-theme` / `twoBowls` selectors,
/// and virtual output status.
///
/// Returns `true` if `theme_settings` was modified and should be re-applied to the `egui::Context`.
pub fn render_multi_device_header_bar(
    ui: &mut Ui,
    ui_state: &mut ControllerEditorUiState,
    theme_settings: &mut ThemeSettings,
    device_manager: &MultiDeviceManager,
    mapping_engine: &mut MultiDeviceMappingEngine,
    led_manager: &mut MultiDeviceLedManager,
) -> bool {
    let mut theme_changed = false;
    let palette = theme_settings.resolved_palette();

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("nkEditor4")
                .strong()
                .size(16.0)
                .color(palette.accent),
        );
        ui.separator();

        // Hardware Controller Model Tabs (always available even offline)
        for (def_id, label) in [
            ("nanokontrol2", "Korg nanoKONTROL2"),
            ("akai-midimix", "Akai MIDI Mix"),
        ] {
            let is_active = ui_state.active_definition_id == def_id
                && ui_state.active_device_key == def_id;
            if ui.selectable_label(is_active, label).clicked() {
                ui_state.select_device(def_id, def_id);
            }
        }

        // Connected & Remembered Hardware Sessions with USB Fingerprint Badges
        for (key, session) in &device_manager.sessions {
            let def_id = session.definition_id.as_deref().unwrap_or("nanokontrol2");
            let is_online = session.state == DeviceConnectionState::Connected;
            let status_tag = if is_online { "ONLINE" } else { "OFFLINE" };
            let alsa_str = session.alsa_seq_addr.as_deref().unwrap_or("-");
            let badge_text = format!(
                "{} [{}] {} ({})",
                session.display_name, session.fingerprint, status_tag, alsa_str
            );
            let is_selected = ui_state.active_device_key == *key;
            let btn = ui.selectable_label(is_selected, badge_text);
            if btn.clicked() {
                ui_state.select_device(key.clone(), def_id);
            }
        }

        ui.separator();

        // Soft Takeover quick selector
        if let Some(pipeline) = mapping_engine.pipelines.get_mut(&ui_state.active_device_key) {
            ui.label("Takeover:");
            for (mode, name) in [
                (SoftTakeoverMode::Catch, "Catch"),
                (SoftTakeoverMode::Pickup, "Pickup"),
                (SoftTakeoverMode::Jump, "Jump"),
            ] {
                if ui
                    .selectable_label(pipeline.soft_takeover_mode == mode, name)
                    .clicked()
                {
                    let threshold = pipeline.soft_takeover_threshold;
                    pipeline.set_soft_takeover(mode, threshold);
                }
            }
        }

        // LED Operating Mode quick selector
        if let Some(led_ctrl) = led_manager.controllers.get_mut(&ui_state.active_device_key) {
            ui.separator();
            ui.label("LEDs:");
            for (mode, name) in [
                (LedOperatingMode::Internal, "Int"),
                (LedOperatingMode::External, "Ext"),
                (LedOperatingMode::Op8Cockpit, "8opPerc"),
            ] {
                if ui.selectable_label(led_ctrl.mode == mode, name).clicked() {
                    led_ctrl.set_mode(mode);
                }
            }
        }

        ui.separator();

        // Quick Theme Selector (all 15 yaae-theme variants)
        egui::ComboBox::from_id_salt("nk_header_theme_combo")
            .selected_text(format!("Theme: {}", theme_settings.preference.display_name()))
            .show_ui(ui, |ui| {
                for &pref in ThemePreference::all() {
                    if ui
                        .selectable_value(
                            &mut theme_settings.preference,
                            pref,
                            pref.display_name(),
                        )
                        .changed()
                    {
                        theme_changed = true;
                    }
                }
            });

        // Quick twoBowls Background Preset Selector
        let current_bg_label = theme_settings
            .background_preset
            .map_or("Theme Default", |p| p.display_name());
        egui::ComboBox::from_id_salt("nk_header_bg_combo")
            .selected_text(format!("BG: {current_bg_label}"))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut theme_settings.background_preset, None, "Theme Default")
                    .changed()
                {
                    theme_changed = true;
                }
                for &preset in TwoBowlsBackgroundPreset::all() {
                    if ui
                        .selectable_value(
                            &mut theme_settings.background_preset,
                            Some(preset),
                            format!("{} ({})", preset.display_name(), preset.hex()),
                        )
                        .changed()
                    {
                        theme_changed = true;
                    }
                }
            });

        // Decimal / Hex Readout Toggle
        for (fmt, label) in [
            (ValueDisplayFormat::Decimal, "DEC"),
            (ValueDisplayFormat::Hexadecimal, "HEX"),
        ] {
            if ui
                .selectable_label(ui_state.value_format == fmt, label)
                .clicked()
            {
                ui_state.value_format = fmt;
            }
        }

        ui.separator();
        let virt_status = if ui_state.virtual_port_active {
            format!("V-OUT: {} [ON]", ui_state.virtual_port_name)
        } else {
            format!("V-OUT: {} [OFF]", ui_state.virtual_port_name)
        };
        ui.label(egui::RichText::new(virt_status).monospace());
    });

    theme_changed
}

// =============================================================================
// 2. Vector Rotary Knob, Vertical Slider & LED Button Primitives
// =============================================================================

/// Renders a custom crisp-bordered vector rotary knob showing:
/// - Outer bezel and 270-degree reference track (`-135°` to `+135°`)
/// - Active target value sweep arc and pointer line in `palette.accent`
/// - Physical Soft Takeover ghost ring and marker when `soft_takeover` is present
/// - Real-time delta badge (`▲ +delta` / `▼ -delta`) when Soft Takeover is locked
/// - High-contrast tabular value and CC/Note readout
pub fn draw_vector_rotary_knob(
    ui: &mut Ui,
    control_path: &str,
    entry: &ControlMappingEntry,
    current_value: u8,
    soft_takeover: Option<SoftTakeoverStatus>,
    is_selected: bool,
    value_format: ValueDisplayFormat,
    palette: &AccentPalette,
    diameter: f32,
) -> (bool, Option<u8>) {
    let mut clicked_select = false;
    let mut new_value = None;

    let desired_size = Vec2::new(diameter + 16.0, diameter + 34.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

    if response.clicked() || response.drag_started() {
        clicked_select = true;
    }

    if response.dragged() {
        let delta_y = -response.drag_delta().y;
        if delta_y.abs() > 0.1 {
            let step = (delta_y * 0.8).round() as i16;
            let updated = (i16::from(current_value) + step).clamp(0, 127) as u8;
            if updated != current_value {
                new_value = Some(updated);
            }
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let border_color = if is_selected {
            palette.strip_selection_border
        } else {
            ui.visuals().widgets.inactive.bg_stroke.color
        };
        let border_width = if is_selected { 2.0 } else { 1.0 };

        // Crisp zero-rounding cell frame
        painter.rect(
            rect,
            Rounding::ZERO,
            ui.visuals().extreme_bg_color,
            Stroke::new(border_width, border_color),
        );

        let center = Pos2::new(rect.center().x, rect.top() + diameter * 0.5 + 4.0);
        let radius = diameter * 0.38;

        // Outer knob body
        painter.circle(
            center,
            radius,
            ui.visuals().widgets.inactive.bg_fill,
            Stroke::new(1.2, border_color),
        );

        // Angle helper: t in [0, 1] -> angle in radians (-225 deg to +45 deg in screen coords)
        let start_angle = -1.25 * PI;
        let sweep_angle = 1.5 * PI;
        let norm_val = f32::from(current_value.min(127)) / 127.0;
        let target_angle = start_angle + norm_val * sweep_angle;

        // Draw target value arc segments
        draw_arc(
            painter,
            center,
            radius + 3.0,
            start_angle,
            target_angle,
            Stroke::new(2.5, palette.accent),
        );

        // Draw target pointer tick
        let tip = Pos2::new(
            center.x + radius * 0.85 * target_angle.cos(),
            center.y + radius * 0.85 * target_angle.sin(),
        );
        painter.line_segment([center, tip], Stroke::new(2.0, palette.accent));

        // Draw Soft Takeover physical ghost ring & marker if present
        if let Some(st) = soft_takeover {
            let phys_norm = f32::from(st.physical_value.min(127)) / 127.0;
            let phys_angle = start_angle + phys_norm * sweep_angle;
            let ghost_color = if st.engaged {
                palette.accent.linear_multiply(0.5)
            } else {
                palette.solo_active_bg
            };
            let ghost_pos = Pos2::new(
                center.x + (radius + 5.5) * phys_angle.cos(),
                center.y + (radius + 5.5) * phys_angle.sin(),
            );
            painter.circle_stroke(ghost_pos, 2.5, Stroke::new(1.5, ghost_color));

            if !st.engaged {
                // Draw ghost arc between physical and target to visualize delta
                let (a0, a1) = if phys_angle < target_angle {
                    (phys_angle, target_angle)
                } else {
                    (target_angle, phys_angle)
                };
                draw_arc(
                    painter,
                    center,
                    radius + 5.5,
                    a0,
                    a1,
                    Stroke::new(1.5, ghost_color),
                );
            }
        }

        // Tabular readout: value and CC/Note badge
        let text_color = ui.visuals().text_color();
        let val_text = if let Some(st) = soft_takeover.filter(|s| !s.engaged) {
            let arrow = match st.direction {
                TakeoverDirection::MoveUp => "^",
                TakeoverDirection::MoveDown => "v",
                TakeoverDirection::Matched => "=",
            };
            format!("{}{:+}", arrow, st.delta)
        } else {
            value_format.format_u8(current_value)
        };

        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 18.0),
            Align2::CENTER_CENTER,
            val_text,
            FontId::monospace(10.0),
            text_color,
        );

        let kind_prefix = match entry.output_message_type {
            MidiMessageType::Cc => "CC",
            MidiMessageType::Note => "NT",
        };
        let cc_text = format!("{kind_prefix}{:02}", entry.output_number);
        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 7.0),
            Align2::CENTER_CENTER,
            cc_text,
            FontId::monospace(9.0),
            text_color.linear_multiply(0.75),
        );
    }

    let _ = control_path;
    (clicked_select, new_value)
}

fn draw_arc(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    stroke: Stroke,
) {
    let steps = 16usize;
    if (end_angle - start_angle).abs() < 0.01 {
        return;
    }
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = start_angle + t * (end_angle - start_angle);
        pts.push(Pos2::new(
            center.x + radius * a.cos(),
            center.y + radius * a.sin(),
        ));
    }
    painter.add(Shape::line(pts, stroke));
}

/// Renders a custom crisp-bordered vertical vector slider/fader showing target level,
/// Soft Takeover physical ghost line when locked, and tabular value readout.
pub fn draw_vector_vertical_slider(
    ui: &mut Ui,
    entry: &ControlMappingEntry,
    current_value: u8,
    soft_takeover: Option<SoftTakeoverStatus>,
    is_selected: bool,
    value_format: ValueDisplayFormat,
    palette: &AccentPalette,
    size: Vec2,
) -> (bool, Option<u8>) {
    let mut clicked_select = false;
    let mut new_value = None;

    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    if response.clicked() || response.drag_started() {
        clicked_select = true;
    }

    let track_top = rect.top() + 8.0;
    let track_bottom = rect.bottom() - 24.0;
    let track_height = (track_bottom - track_top).max(20.0);

    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let norm = 1.0 - ((pos.y - track_top) / track_height).clamp(0.0, 1.0);
            let updated = (norm * 127.0).round().clamp(0.0, 127.0) as u8;
            if updated != current_value {
                new_value = Some(updated);
            }
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let border_color = if is_selected {
            palette.strip_selection_border
        } else {
            ui.visuals().widgets.inactive.bg_stroke.color
        };
        let border_width = if is_selected { 2.0 } else { 1.0 };

        painter.rect(
            rect,
            Rounding::ZERO,
            ui.visuals().extreme_bg_color,
            Stroke::new(border_width, border_color),
        );

        let track_rect = Rect::from_min_max(
            Pos2::new(rect.center().x - 6.0, track_top),
            Pos2::new(rect.center().x + 6.0, track_bottom),
        );
        painter.rect(
            track_rect,
            Rounding::ZERO,
            ui.visuals().widgets.inactive.bg_fill,
            Stroke::new(1.0, border_color),
        );

        // Filled bar for target value
        let norm_val = f32::from(current_value.min(127)) / 127.0;
        let fill_top = track_bottom - norm_val * track_height;
        let fill_rect = Rect::from_min_max(
            Pos2::new(track_rect.left() + 1.0, fill_top),
            Pos2::new(track_rect.right() - 1.0, track_bottom),
        );
        painter.rect_filled(fill_rect, Rounding::ZERO, palette.accent);

        // Fader cap
        let cap_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, fill_top),
            Vec2::new(size.x - 8.0, 6.0),
        );
        painter.rect(
            cap_rect,
            Rounding::ZERO,
            palette.accent_strong,
            Stroke::new(1.0, border_color),
        );

        // Physical ghost marker if Soft Takeover is locked
        if let Some(st) = soft_takeover.filter(|s| !s.engaged) {
            let phys_norm = f32::from(st.physical_value.min(127)) / 127.0;
            let phys_y = track_bottom - phys_norm * track_height;
            painter.line_segment(
                [
                    Pos2::new(rect.left() + 3.0, phys_y),
                    Pos2::new(rect.right() - 3.0, phys_y),
                ],
                Stroke::new(2.0, palette.solo_active_bg),
            );
        }

        let text_color = ui.visuals().text_color();
        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 15.0),
            Align2::CENTER_CENTER,
            value_format.format_u8(current_value),
            FontId::monospace(10.0),
            text_color,
        );
        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 5.0),
            Align2::CENTER_CENTER,
            format!("CC{:02}", entry.output_number),
            FontId::monospace(9.0),
            text_color.linear_multiply(0.75),
        );
    }

    (clicked_select, new_value)
}

/// Renders a hardware button with a physical LED indicator bar (Solo/Mute/Rec or Transport).
pub fn draw_led_hardware_button(
    ui: &mut Ui,
    short_label: &str,
    entry: &ControlMappingEntry,
    led_is_on: bool,
    is_selected: bool,
    led_active_color: Color32,
    palette: &AccentPalette,
    size: Vec2,
) -> (bool, bool) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let clicked = response.clicked();
    let right_clicked = response.secondary_clicked();

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let border_color = if is_selected {
            palette.strip_selection_border
        } else {
            ui.visuals().widgets.inactive.bg_stroke.color
        };
        let border_width = if is_selected { 2.0 } else { 1.0 };

        let bg_fill = if led_is_on {
            led_active_color.linear_multiply(0.35)
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };

        painter.rect(
            rect,
            Rounding::ZERO,
            bg_fill,
            Stroke::new(border_width, border_color),
        );

        // Top LED indicator bar
        let led_bar = Rect::from_min_max(
            Pos2::new(rect.left() + 3.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 3.0, rect.top() + 5.0),
        );
        let led_color = if led_is_on {
            led_active_color
        } else {
            ui.visuals().extreme_bg_color
        };
        painter.rect_filled(led_bar, Rounding::ZERO, led_color);

        painter.text(
            Pos2::new(rect.center().x, rect.center().y + 2.0),
            Align2::CENTER_CENTER,
            short_label,
            FontId::monospace(10.0),
            ui.visuals().text_color(),
        );
    }

    let _ = entry;
    (clicked, right_clicked)
}

// =============================================================================
// 3. Korg nanoKONTROL2 & Akai MIDI Mix Purpose-Built Controller Layouts
// =============================================================================

/// Renders the complete Korg `nanoKONTROL2` interactive surface:
/// - Left Transport Deck (11 transport buttons, 4 with hardware LEDs: `Play`, `Stop`, `Record`, `Cycle`)
/// - 8 Track Columns (`track1..track8`), each with custom rotary `Knob`, `S`/`M`/`R` LED buttons,
///   and vertical `Slider`.
pub fn render_nanokontrol2_surface(
    ui: &mut Ui,
    ui_state: &mut ControllerEditorUiState,
    pipeline: &mut DeviceMappingPipeline,
    led_controller: &mut DeviceLedController,
    palette: &AccentPalette,
) {
    ui.horizontal(|ui| {
        // 1. Left Transport Deck
        ui.vertical(|ui| {
            ui.set_width(175.0);
            ui.label(egui::RichText::new("TRANSPORT DECK").strong().monospace());
            ui.separator();

            // Row 1: Cycle
            render_transport_button_row(
                ui,
                ui_state,
                pipeline,
                led_controller,
                palette,
                &[("transport.cycle", "CYCLE")],
            );
            ui.add_space(4.0);

            // Row 2: Marker Set / < / >
            ui.label(egui::RichText::new("MARKER").small().monospace());
            render_transport_button_row(
                ui,
                ui_state,
                pipeline,
                led_controller,
                palette,
                &[
                    ("transport.marker_set", "SET"),
                    ("transport.marker_left", "<"),
                    ("transport.marker_right", ">"),
                ],
            );
            ui.add_space(4.0);

            // Row 3: Track < / >
            ui.label(egui::RichText::new("TRACK").small().monospace());
            render_transport_button_row(
                ui,
                ui_state,
                pipeline,
                led_controller,
                palette,
                &[
                    ("transport.track_left", "TRK <"),
                    ("transport.track_right", "TRK >"),
                ],
            );
            ui.add_space(6.0);

            // Row 4: Rewind / Forward / Stop / Play / Record
            ui.label(egui::RichText::new("MAIN TRANSPORT").small().monospace());
            render_transport_button_row(
                ui,
                ui_state,
                pipeline,
                led_controller,
                palette,
                &[
                    ("transport.rewind", "<<"),
                    ("transport.forward", ">>"),
                    ("transport.stop", "STOP"),
                ],
            );
            render_transport_button_row(
                ui,
                ui_state,
                pipeline,
                led_controller,
                palette,
                &[("transport.play", "PLAY"), ("transport.record", "REC")],
            );
        });

        ui.separator();

        // 2. 8 Track Columns (Tracks 1..8)
        for track_num in 1..=8 {
            ui.vertical(|ui| {
                ui.set_width(82.0);
                let knob_path = format!("track{track_num}.knob");
                let slider_path = format!("track{track_num}.slider");
                let solo_path = format!("track{track_num}.solo");
                let mute_path = format!("track{track_num}.mute");
                let rec_path = format!("track{track_num}.rec");

                ui.label(
                    egui::RichText::new(format!("TRACK {track_num}"))
                        .strong()
                        .monospace(),
                );

                // Rotary Knob
                if let Some(knob_entry) = pipeline.config.entries.get(&knob_path).cloned() {
                    let st = pipeline.soft_takeover_status(&knob_path);
                    let cur_val = st.map_or(0, |s| s.target_value);
                    let is_sel = ui_state.selected_control_path == knob_path;
                    let (clicked, changed_val) = draw_vector_rotary_knob(
                        ui,
                        &knob_path,
                        &knob_entry,
                        cur_val,
                        st,
                        is_sel,
                        ui_state.value_format,
                        palette,
                        48.0,
                    );
                    if clicked {
                        ui_state.selected_control_path = knob_path.clone();
                    }
                    if let Some(v) = changed_val {
                        let _ = pipeline.process_message(
                            knob_entry.input_message_type,
                            1,
                            knob_entry.input_number,
                            v,
                        );
                    }
                }

                // S / M / R Buttons + Vertical Slider side-by-side
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        for (path, short, color) in [
                            (&solo_path, "S", palette.solo_active_bg),
                            (&mute_path, "M", palette.mute_active_bg),
                            (&rec_path, "R", palette.record_arm_active_bg),
                        ] {
                            if let Some(btn_entry) = pipeline.config.entries.get(path).cloned() {
                                let led_on = led_controller.led_state_by_name(path)
                                    || pipeline.button_state(path);
                                let is_sel = ui_state.selected_control_path == *path;
                                let (clicked, _) = draw_led_hardware_button(
                                    ui,
                                    short,
                                    &btn_entry,
                                    led_on,
                                    is_sel,
                                    color,
                                    palette,
                                    Vec2::new(28.0, 28.0),
                                );
                                if clicked {
                                    ui_state.selected_control_path = path.clone();
                                    if let crate::mapping_engine::MappingEvaluation::Emitted {
                                        button_event: Some(ev),
                                        ..
                                    } = pipeline.evaluate_message(
                                        btn_entry.input_message_type,
                                        1,
                                        btn_entry.input_number,
                                        127,
                                    ) {
                                        let _ = led_controller.handle_internal_button_event(&ev);
                                    }
                                }
                            }
                        }
                    });

                    if let Some(slider_entry) = pipeline.config.entries.get(&slider_path).cloned() {
                        let st = pipeline.soft_takeover_status(&slider_path);
                        let cur_val = st.map_or(0, |s| s.target_value);
                        let is_sel = ui_state.selected_control_path == slider_path;
                        let (clicked, changed_val) = draw_vector_vertical_slider(
                            ui,
                            &slider_entry,
                            cur_val,
                            st,
                            is_sel,
                            ui_state.value_format,
                            palette,
                            Vec2::new(40.0, 96.0),
                        );
                        if clicked {
                            ui_state.selected_control_path = slider_path.clone();
                        }
                        if let Some(v) = changed_val {
                            let _ = pipeline.process_message(
                                slider_entry.input_message_type,
                                1,
                                slider_entry.input_number,
                                v,
                            );
                        }
                    }
                });
            });
        }
    });
}

fn render_transport_button_row(
    ui: &mut Ui,
    ui_state: &mut ControllerEditorUiState,
    pipeline: &mut DeviceMappingPipeline,
    led_controller: &mut DeviceLedController,
    palette: &AccentPalette,
    buttons: &[(&str, &str)],
) {
    ui.horizontal(|ui| {
        for &(path, label) in buttons {
            if let Some(entry) = pipeline.config.entries.get(path).cloned() {
                let led_on =
                    led_controller.led_state_by_name(path) || pipeline.button_state(path);
                let is_sel = ui_state.selected_control_path == path;
                let active_color = if path.ends_with("record") {
                    palette.record_arm_active_bg
                } else {
                    palette.accent
                };
                let (clicked, _) = draw_led_hardware_button(
                    ui,
                    label,
                    &entry,
                    led_on,
                    is_sel,
                    active_color,
                    palette,
                    Vec2::new(50.0, 26.0),
                );
                if clicked {
                    ui_state.selected_control_path = path.to_string();
                    if let crate::mapping_engine::MappingEvaluation::Emitted {
                        button_event: Some(ev),
                        ..
                    } = pipeline.evaluate_message(
                        entry.input_message_type,
                        1,
                        entry.input_number,
                        127,
                    ) {
                        let _ = led_controller.handle_internal_button_event(&ev);
                    }
                }
            }
        }
    });
}

/// Renders the complete Akai `MIDI Mix` interactive surface:
/// - 8 Track Columns (`track1..track8`), each with 3 rotary knobs (`knob1`, `knob2`, `knob3`),
///   `MUTE` / `SOLO` / `REC ARM` Note-driven LED buttons, and a vertical `Fader`.
/// - Right `MASTER` strip (`Bank Left`, `Bank Right`, `Solo Master`, and `Master Fader`).
pub fn render_akai_midimix_surface(
    ui: &mut Ui,
    ui_state: &mut ControllerEditorUiState,
    pipeline: &mut DeviceMappingPipeline,
    led_controller: &mut DeviceLedController,
    palette: &AccentPalette,
) {
    ui.horizontal(|ui| {
        for track_num in 1..=8 {
            ui.vertical(|ui| {
                ui.set_width(72.0);
                ui.label(
                    egui::RichText::new(format!("CH {track_num}"))
                        .strong()
                        .monospace(),
                );

                // 3 Rotary Knob Rows
                for knob_idx in 1..=3 {
                    let knob_path = format!("track{track_num}.knob{knob_idx}");
                    if let Some(knob_entry) = pipeline.config.entries.get(&knob_path).cloned() {
                        let st = pipeline.soft_takeover_status(&knob_path);
                        let cur_val = st.map_or(0, |s| s.target_value);
                        let is_sel = ui_state.selected_control_path == knob_path;
                        let (clicked, changed_val) = draw_vector_rotary_knob(
                            ui,
                            &knob_path,
                            &knob_entry,
                            cur_val,
                            st,
                            is_sel,
                            ui_state.value_format,
                            palette,
                            40.0,
                        );
                        if clicked {
                            ui_state.selected_control_path = knob_path.clone();
                        }
                        if let Some(v) = changed_val {
                            let _ = pipeline.process_message(
                                knob_entry.input_message_type,
                                1,
                                knob_entry.input_number,
                                v,
                            );
                        }
                    }
                }

                // Mute, Solo, Rec Arm Note Buttons
                let mute_path = format!("track{track_num}.mute");
                let solo_path = format!("track{track_num}.solo");
                let rec_path = format!("track{track_num}.rec_arm");
                for (path, label, color) in [
                    (&mute_path, "MUTE", palette.mute_active_bg),
                    (&solo_path, "SOLO", palette.solo_active_bg),
                    (&rec_path, "REC", palette.record_arm_active_bg),
                ] {
                    if let Some(btn_entry) = pipeline.config.entries.get(path).cloned() {
                        let led_on = led_controller.led_state_by_name(path)
                            || pipeline.button_state(path);
                        let is_sel = ui_state.selected_control_path == *path;
                        let (clicked, _) = draw_led_hardware_button(
                            ui,
                            label,
                            &btn_entry,
                            led_on,
                            is_sel,
                            color,
                            palette,
                            Vec2::new(56.0, 22.0),
                        );
                        if clicked {
                            ui_state.selected_control_path = path.clone();
                            if let crate::mapping_engine::MappingEvaluation::Emitted {
                                button_event: Some(ev),
                                ..
                            } = pipeline.evaluate_message(
                                btn_entry.input_message_type,
                                1,
                                btn_entry.input_number,
                                127,
                            ) {
                                let _ = led_controller.handle_internal_button_event(&ev);
                            }
                        }
                    }
                }

                // Track Fader
                let fader_path = format!("track{track_num}.fader");
                if let Some(fader_entry) = pipeline.config.entries.get(&fader_path).cloned() {
                    let st = pipeline.soft_takeover_status(&fader_path);
                    let cur_val = st.map_or(0, |s| s.target_value);
                    let is_sel = ui_state.selected_control_path == fader_path;
                    let (clicked, changed_val) = draw_vector_vertical_slider(
                        ui,
                        &fader_entry,
                        cur_val,
                        st,
                        is_sel,
                        ui_state.value_format,
                        palette,
                        Vec2::new(44.0, 88.0),
                    );
                    if clicked {
                        ui_state.selected_control_path = fader_path.clone();
                    }
                    if let Some(v) = changed_val {
                        let _ = pipeline.process_message(
                            fader_entry.input_message_type,
                            1,
                            fader_entry.input_number,
                            v,
                        );
                    }
                }
            });
        }

        ui.separator();

        // Master Strip
        ui.vertical(|ui| {
            ui.set_width(82.0);
            ui.label(egui::RichText::new("MASTER").strong().monospace());
            for (path, label) in [
                ("master.bank_left", "BANK <"),
                ("master.bank_right", "BANK >"),
                ("master.solo_master", "SOLO MST"),
            ] {
                if let Some(btn_entry) = pipeline.config.entries.get(path).cloned() {
                    let is_sel = ui_state.selected_control_path == path;
                    let (clicked, _) = draw_led_hardware_button(
                        ui,
                        label,
                        &btn_entry,
                        pipeline.button_state(path),
                        is_sel,
                        palette.accent,
                        palette,
                        Vec2::new(70.0, 24.0),
                    );
                    if clicked {
                        ui_state.selected_control_path = path.to_string();
                        let _ = pipeline.evaluate_message(
                            btn_entry.input_message_type,
                            1,
                            btn_entry.input_number,
                            127,
                        );
                    }
                }
            }

            if let Some(mf_entry) = pipeline.config.entries.get("master.master_fader").cloned() {
                let st = pipeline.soft_takeover_status("master.master_fader");
                let cur_val = st.map_or(0, |s| s.target_value);
                let is_sel = ui_state.selected_control_path == "master.master_fader";
                let (clicked, changed_val) = draw_vector_vertical_slider(
                    ui,
                    &mf_entry,
                    cur_val,
                    st,
                    is_sel,
                    ui_state.value_format,
                    palette,
                    Vec2::new(48.0, 120.0),
                );
                if clicked {
                    ui_state.selected_control_path = "master.master_fader".to_string();
                }
                if let Some(v) = changed_val {
                    let _ = pipeline.process_message(
                        mf_entry.input_message_type,
                        1,
                        mf_entry.input_number,
                        v,
                    );
                }
            }
        });
    });
}

// =============================================================================
// 4. Inspector Side Panel (Control Mapping & Nord Drum 2 Parameter Assignment)
// =============================================================================

/// Renders the right-hand Inspector Side Panel for viewing and editing the selected control's
/// CC/Note number, channel (`1..=16`), label, response curve, min/max range, polarity inversion,
/// button behavior, and Nord Drum 2 target parameter assignment.
pub fn render_inspector_side_panel(
    ui: &mut Ui,
    ui_state: &mut ControllerEditorUiState,
    pipeline: &mut DeviceMappingPipeline,
    palette: &AccentPalette,
) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new("CONTROL INSPECTOR")
                .strong()
                .monospace()
                .color(palette.accent),
        );
        ui.separator();

        let Some(mut entry) = pipeline
            .config
            .entries
            .get(&ui_state.selected_control_path)
            .cloned()
        else {
            ui.label("Select a knob, slider, or button to inspect.");
            return;
        };

        let mut modified = false;

        ui.label(
            egui::RichText::new(format!("Control: {}", entry.control_path))
                .strong()
                .monospace(),
        );
        ui.label(
            egui::RichText::new(format!(
                "Hardware In: {:?} #{}  |  Transform: {}",
                entry.input_message_type,
                entry.input_number,
                entry.transformation()
            ))
            .monospace(),
        );
        ui.separator();

        // Output Message Type (CC vs Note)
        ui.horizontal(|ui| {
            ui.label("Output Type:");
            for (t, label) in [
                (MidiMessageType::Cc, "CC"),
                (MidiMessageType::Note, "Note"),
            ] {
                if ui
                    .selectable_label(entry.output_message_type == t, label)
                    .clicked()
                {
                    entry.output_message_type = t;
                    modified = true;
                }
            }
        });

        // Output CC / Note Number (0..=127)
        ui.horizontal(|ui| {
            ui.label("Output # (0..127):");
            let mut num = entry.output_number;
            if ui
                .add(egui::DragValue::new(&mut num).range(0..=127))
                .changed()
            {
                entry.output_number = num;
                modified = true;
            }
        });

        // Output MIDI Channel (1..=16)
        ui.horizontal(|ui| {
            ui.label("MIDI Channel (1..16):");
            let mut ch = entry.output_channel;
            if ui
                .add(egui::DragValue::new(&mut ch).range(1..=16))
                .changed()
            {
                entry.output_channel = ch;
                modified = true;
            }
        });

        // Custom Label
        ui.horizontal(|ui| {
            ui.label("Label:");
            let mut label_buf = entry.label.clone().unwrap_or_default();
            if ui.text_edit_singleline(&mut label_buf).changed() {
                entry.label = if label_buf.trim().is_empty() {
                    None
                } else {
                    Some(label_buf)
                };
                modified = true;
            }
        });

        // Polarity Inversion
        if ui
            .checkbox(&mut entry.invert, "Invert Polarity (0..127 <-> 127..0)")
            .changed()
        {
            modified = true;
        }

        ui.separator();

        if entry.is_button {
            ui.label(egui::RichText::new("BUTTON BEHAVIOR").strong().monospace());
            ui.horizontal(|ui| {
                for (beh, name) in [
                    (ButtonBehavior::Toggle, "Toggle"),
                    (ButtonBehavior::Momentary, "Momentary"),
                ] {
                    if ui.selectable_label(entry.behavior == beh, name).clicked() {
                        entry.behavior = beh;
                        modified = true;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("ON Value:");
                if ui
                    .add(egui::DragValue::new(&mut entry.on_value).range(0..=127))
                    .changed()
                {
                    modified = true;
                }
                ui.label("OFF Value:");
                if ui
                    .add(egui::DragValue::new(&mut entry.off_value).range(0..=127))
                    .changed()
                {
                    modified = true;
                }
            });
        } else {
            ui.label(
                egui::RichText::new("CONTINUOUS SCALING & CURVE")
                    .strong()
                    .monospace(),
            );
            ui.horizontal(|ui| {
                ui.label("Min:");
                if ui
                    .add(egui::DragValue::new(&mut entry.min_value).range(0..=127))
                    .changed()
                {
                    modified = true;
                }
                ui.label("Max:");
                if ui
                    .add(egui::DragValue::new(&mut entry.max_value).range(0..=127))
                    .changed()
                {
                    modified = true;
                }
            });

            ui.horizontal_wrapped(|ui| {
                ui.label("Curve:");
                for (curve, name) in [
                    (ResponseCurve::Linear, "Linear"),
                    (ResponseCurve::Exponential, "Exp"),
                    (ResponseCurve::Logarithmic, "Log"),
                    (ResponseCurve::SCurve, "S-Curve"),
                ] {
                    if ui.selectable_label(entry.curve == curve, name).clicked() {
                        entry.curve = curve;
                        modified = true;
                    }
                }
            });

            if let Some(st) = pipeline.soft_takeover_status(&entry.control_path) {
                ui.separator();
                ui.label(egui::RichText::new("SOFT TAKEOVER STATUS").strong().monospace());
                let status_str = if st.engaged { "ENGAGED" } else { "LOCKED" };
                ui.label(
                    egui::RichText::new(format!(
                        "State: {status_str} ({:?}, T={})\nPhys: {} | Target: {} | Delta: {:+}",
                        st.mode, st.threshold, st.physical_value, st.target_value, st.delta
                    ))
                    .monospace(),
                );
            }
        }

        if modified {
            pipeline.update_entry(entry);
        }

        ui.separator();

        // Nord Drum 2 Desktop Target Assignment Helper
        ui.label(
            egui::RichText::new("NORD DRUM 2 TARGET ASSIGN")
                .strong()
                .monospace()
                .color(palette.accent),
        );
        ui.horizontal(|ui| {
            ui.label("Pad (Ch 1..6):");
            ui.add(egui::DragValue::new(&mut ui_state.inspector_nd2_pad).range(1..=6));
        });

        let selected_param = &nord_drum_2::ALL_PARAMS
            [ui_state.inspector_nd2_param_idx.min(nord_drum_2::ALL_PARAMS.len() - 1)];
        egui::ComboBox::from_id_salt("nk_inspector_nd2_param")
            .selected_text(format!(
                "{:?} {} (CC {})",
                selected_param.section, selected_param.name, selected_param.cc
            ))
            .show_ui(ui, |ui| {
                for (idx, p) in nord_drum_2::ALL_PARAMS.iter().enumerate() {
                    ui.selectable_value(
                        &mut ui_state.inspector_nd2_param_idx,
                        idx,
                        format!("{:?}: {} (CC {})", p.section, p.name, p.cc),
                    );
                }
            });

        if ui.button("Assign Nord Drum 2 CC").clicked() {
            let _ = ui_state.assign_nord_drum_2_parameter(
                pipeline,
                ui_state.inspector_nd2_pad,
                ui_state.inspector_nd2_param_idx,
            );
        }
    });
}

// =============================================================================
// 5. Complete Root Workspace Renderer
// =============================================================================

/// Ensures default `nanokontrol2` and `akai-midimix` pipelines and LED controllers exist
/// even when running in offline/disconnected preview mode, and renders the complete
/// Header Bar + Controller Surface + Inspector Side Panel.
pub fn render_controller_workspace(
    ctx: &egui::Context,
    ui_state: &mut ControllerEditorUiState,
    theme_settings: &mut ThemeSettings,
    device_manager: &MultiDeviceManager,
    mapping_engine: &mut MultiDeviceMappingEngine,
    led_manager: &mut MultiDeviceLedManager,
) -> bool {
    // Ensure offline preview pipelines exist for both supported controller definitions
    if !mapping_engine.pipelines.contains_key("nanokontrol2") {
        let def: DeviceDefinition = nanokontrol2_definition();
        mapping_engine.register_device_pipeline(
            "nanokontrol2",
            crate::mapping_engine::DeviceMappingConfig::from_device_definition(&def),
        );
    }
    if !mapping_engine.pipelines.contains_key("akai-midimix") {
        let def: DeviceDefinition = akai_midimix_definition();
        mapping_engine.register_device_pipeline(
            "akai-midimix",
            crate::mapping_engine::DeviceMappingConfig::from_device_definition(&def),
        );
    }
    let _ = led_manager.ensure_controller("nanokontrol2", "nanokontrol2");
    let _ = led_manager.ensure_controller("akai-midimix", "akai-midimix");

    let mut theme_changed = false;
    let palette = theme_settings.resolved_palette();

    egui::TopBottomPanel::top("nkeditor_multi_device_header").show(ctx, |ui| {
        if render_multi_device_header_bar(
            ui,
            ui_state,
            theme_settings,
            device_manager,
            mapping_engine,
            led_manager,
        ) {
            theme_changed = true;
        }
    });

    egui::SidePanel::right("nkeditor_inspector_panel")
        .default_width(275.0)
        .resizable(true)
        .show(ctx, |ui| {
            let key = ui_state.active_device_key.clone();
            if let Some(pipeline) = mapping_engine.pipelines.get_mut(&key) {
                render_inspector_side_panel(ui, ui_state, pipeline, &palette);
            }
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        let key = ui_state.active_device_key.clone();
        let def_id = ui_state.active_definition_id.clone();
        if let (Some(pipeline), Some(led_ctrl)) = (
            mapping_engine.pipelines.get_mut(&key),
            led_manager.controllers.get_mut(&key),
        ) {
            egui::ScrollArea::both().show(ui, |ui| {
                if def_id == "akai-midimix" {
                    render_akai_midimix_surface(ui, ui_state, pipeline, led_ctrl, &palette);
                } else {
                    render_nanokontrol2_surface(ui, ui_state, pipeline, led_ctrl, &palette);
                }
            });
        }
    });

    ui_state.rendered_frame_count += 1;
    theme_changed
}

// =============================================================================
// Headless egui & State Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_egui_renders_nanokontrol2_and_akai_midimix_surfaces_with_soft_takeover_ring() {
        let ctx = egui::Context::default();
        let mut ui_state = ControllerEditorUiState::default();
        let mut theme_settings = ThemeSettings::with_preference(ThemePreference::SolarizedDark);
        theme_settings.background_preset = Some(TwoBowlsBackgroundPreset::DarkGrey);
        theme_settings.apply_to_ctx(&ctx);

        let device_manager = MultiDeviceManager::new();
        let mut mapping_engine = MultiDeviceMappingEngine::new();
        let mut led_manager = MultiDeviceLedManager::new();

        // Render Frame 1: Korg nanoKONTROL2 surface with a locked Soft Takeover knob on track1.knob
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = render_controller_workspace(
                ctx,
                &mut ui_state,
                &mut theme_settings,
                &device_manager,
                &mut mapping_engine,
                &mut led_manager,
            );
        });
        assert_eq!(ui_state.rendered_frame_count, 1);

        // Arm a Soft Takeover lock on track1.knob (physical = 25, target = 90) and render Frame 2
        {
            let nk2_pipe = mapping_engine.pipelines.get_mut("nanokontrol2").unwrap();
            let _ = nk2_pipe.process_cc(1, 16, 25);
            assert!(nk2_pipe.set_target_value("track1.knob", 90));
        }

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = render_controller_workspace(
                ctx,
                &mut ui_state,
                &mut theme_settings,
                &device_manager,
                &mut mapping_engine,
                &mut led_manager,
            );
        });
        assert_eq!(ui_state.rendered_frame_count, 2);

        // Switch active surface to Akai MIDI Mix and Hexadecimal readout and render Frame 3
        ui_state.select_device("akai-midimix", "akai-midimix");
        ui_state.value_format = ValueDisplayFormat::Hexadecimal;
        assert_eq!(ui_state.selected_control_path, "track1.knob1");
        assert_eq!(ui_state.value_format.format_u8(127), "0x7F");

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = render_controller_workspace(
                ctx,
                &mut ui_state,
                &mut theme_settings,
                &device_manager,
                &mut mapping_engine,
                &mut led_manager,
            );
        });
        assert_eq!(ui_state.rendered_frame_count, 3);
    }

    #[test]
    fn test_inspector_nord_drum_2_parameter_assignment_for_pads_and_global_fx() {
        let mut ui_state = ControllerEditorUiState::default();
        let mut pipeline = DeviceMappingPipeline::new(
            "nanokontrol2",
            crate::mapping_engine::DeviceMappingConfig::default_nanokontrol2(),
        );

        // 1. Assign Pad 4 Tone TIMBRE (index 8 -> CC 52 on Channel 4) to track1.knob
        ui_state.selected_control_path = "track1.knob".to_string();
        let timbre_idx = nord_drum_2::ALL_PARAMS
            .iter()
            .position(|p| p.name == "TIMBRE")
            .unwrap();
        let updated = ui_state
            .assign_nord_drum_2_parameter(&mut pipeline, 4, timbre_idx)
            .expect("assigned TIMBRE");
        assert_eq!(updated.output_message_type, MidiMessageType::Cc);
        assert_eq!(updated.output_number, 52);
        assert_eq!(updated.output_channel, 4);
        assert_eq!(updated.label.as_deref(), Some("ND2 P4 TIMBRE"));

        // Verify pipeline immediately routes CC 16 -> CC 52 on Channel 4
        let out_msg = pipeline.process_cc(1, 16, 100).unwrap();
        assert_eq!(out_msg.output_number, 52);
        assert_eq!(out_msg.output_channel, 4);

        // 2. Assign Global FX ECHO_AMOUNT (CC 28 on Channel 10) to track8.slider
        ui_state.selected_control_path = "track8.slider".to_string();
        let echo_idx = nord_drum_2::ALL_PARAMS
            .iter()
            .position(|p| p.name == "ECHO_AMOUNT")
            .unwrap();
        let fx_updated = ui_state
            .assign_nord_drum_2_parameter(&mut pipeline, 2, echo_idx)
            .expect("assigned ECHO_AMOUNT");
        assert_eq!(fx_updated.output_number, 28);
        assert_eq!(
            fx_updated.output_channel,
            nord_drum_2::channels::GLOBAL_FX,
            "Global FX parameters must automatically route to Channel 10"
        );
        assert_eq!(fx_updated.label.as_deref(), Some("ND2 FX ECHO_AMOUNT"));
    }
}
