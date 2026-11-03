//! Visual theme engine for `nkEditor4`.
//!
//! Integrates all 15 [`yaae_theme::ThemePreference`] variants, the 4 `twoBowls`
//! background presets (`White`, `Cream`, `Black`, `Dark Grey`), manual CSS hex
//! color overrides (`CustomColorOverrides`), zero-rounding crisp borders
//! (`egui::Rounding::ZERO`), and UI scaling (`50%` to `200%`).

use egui::{Color32, Context, Rounding, Stroke, Style, Visuals};
use serde::{Deserialize, Serialize};
pub use yaae_theme::{
    focus_ring_for_theme, ui_visuals, AccentPalette, ColorConfig, FocusRingConfig, ThemePreference,
};

/// Minimum allowed UI scale percentage.
pub const UI_SCALE_MIN_PERCENT: u16 = 50;

/// Maximum allowed UI scale percentage.
pub const UI_SCALE_MAX_PERCENT: u16 = 200;

/// Default UI scale percentage.
pub const UI_SCALE_DEFAULT_PERCENT: u16 = 100;

/// The four `twoBowls` background presets for quick surface background switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TwoBowlsBackgroundPreset {
    /// Crisp pure white (`#FFFFFF`).
    White,
    /// Warm paper cream (`#F8F2E4`).
    Cream,
    /// Pure OLED black (`#000000`).
    Black,
    /// Neutral studio dark grey (`#181818`).
    #[default]
    DarkGrey,
}

impl TwoBowlsBackgroundPreset {
    /// Returns all 4 `twoBowls` background presets in display order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::White, Self::Cream, Self::Black, Self::DarkGrey]
    }

    /// Human-readable display name for UI dropdowns and OSC commands.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Cream => "Cream",
            Self::Black => "Black",
            Self::DarkGrey => "Dark Grey",
        }
    }

    /// Canonical `#RRGGBB` hex string for this background preset.
    #[must_use]
    pub const fn hex(&self) -> &'static str {
        match self {
            Self::White => "#FFFFFF",
            Self::Cream => "#F8F2E4",
            Self::Black => "#000000",
            Self::DarkGrey => "#181818",
        }
    }

    /// Returns the `egui::Color32` value for this background preset.
    #[must_use]
    pub const fn to_color32(&self) -> Color32 {
        match self {
            Self::White => Color32::from_rgb(0xFF, 0xFF, 0xFF),
            Self::Cream => Color32::from_rgb(0xF8, 0xF2, 0xE4),
            Self::Black => Color32::from_rgb(0x00, 0x00, 0x00),
            Self::DarkGrey => Color32::from_rgb(0x18, 0x18, 0x18),
        }
    }

    /// Returns `true` if this preset is a light background (`White` or `Cream`).
    #[must_use]
    pub const fn is_light(&self) -> bool {
        matches!(self, Self::White | Self::Cream)
    }

    /// Parse a preset name case-insensitively (e.g. `"white"`, `"cream"`, `"black"`,
    /// `"dark_grey"`, `"dark grey"`, `"dark-grey"`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match normalized.as_str() {
            "white" | "#ffffff" => Some(Self::White),
            "cream" | "#f8f2e4" => Some(Self::Cream),
            "black" | "#000000" => Some(Self::Black),
            "dark_grey" | "dark_gray" | "darkgrey" | "darkgray" | "#181818" => Some(Self::DarkGrey),
            _ => None,
        }
    }
}

impl std::fmt::Display for TwoBowlsBackgroundPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Parses a CSS hex color string (`#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`, with or without `#`)
/// into an [`egui::Color32`].
pub fn parse_hex_color(input: &str) -> Result<Color32, String> {
    let trimmed = input.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);

    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("Invalid hex digits in color string: '{input}'"));
    }

    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).map_err(|e| e.to_string())? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).map_err(|e| e.to_string())? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).map_err(|e| e.to_string())? * 17;
            Ok(Color32::from_rgb(r, g, b))
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1], 16).map_err(|e| e.to_string())? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).map_err(|e| e.to_string())? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).map_err(|e| e.to_string())? * 17;
            let a = u8::from_str_radix(&hex[3..4], 16).map_err(|e| e.to_string())? * 17;
            Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
            Ok(Color32::from_rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
            let a = u8::from_str_radix(&hex[6..8], 16).map_err(|e| e.to_string())?;
            Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => Err(format!(
            "Hex color must have 3, 4, 6, or 8 hex digits, got {} in '{input}'",
            hex.len()
        )),
    }
}

/// Formats an [`egui::Color32`] into an uppercase `#RRGGBB` string.
#[must_use]
pub fn format_hex_rgb(color: Color32) -> String {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// Formats an [`egui::Color32`] into an uppercase `#RRGGBBAA` string.
#[must_use]
pub fn format_hex_rgba(color: Color32) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}

/// Formats an [`egui::Color32`] into `#RRGGBB` if fully opaque (`a == 255`), or `#RRGGBBAA` otherwise.
#[must_use]
pub fn format_hex_color(color: Color32) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

/// Manual hex color overrides for user-customizable theme slots (mirrors legacy `ThemeColors`
/// plus optional text override).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CustomColorOverrides {
    /// Main panel/window background hex override (e.g. `#1A1A1A`).
    pub background: Option<String>,
    /// Darker/extreme background hex override (e.g. `#0F0F0F`).
    pub background_darker: Option<String>,
    /// Widget surface hex override (e.g. `#2A2A2A`).
    pub surface: Option<String>,
    /// Widget and window border hex override (e.g. `#3A3A3A`).
    pub border: Option<String>,
    /// Primary text hex override.
    pub text: Option<String>,
    /// Primary accent/highlight hex override (e.g. `#FF6600`).
    pub accent: Option<String>,
    /// Solo button active LED hex override (e.g. `#FFCC00`).
    pub solo: Option<String>,
    /// Mute button active LED hex override (e.g. `#00CC66`).
    pub mute: Option<String>,
    /// Record button active LED hex override (e.g. `#FF3333`).
    pub rec: Option<String>,
}

impl CustomColorOverrides {
    /// Creates an empty override set where all colors defer to the active `yaae_theme::ThemePreference`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            background: None,
            background_darker: None,
            surface: None,
            border: None,
            text: None,
            accent: None,
            solo: None,
            mute: None,
            rec: None,
        }
    }

    /// Creates the classic `nkEditor3` default hardware-inspired color overrides.
    #[must_use]
    pub fn nkeditor3_classic() -> Self {
        Self {
            background: Some("#1A1A1A".to_string()),
            background_darker: Some("#0F0F0F".to_string()),
            surface: Some("#2A2A2A".to_string()),
            border: Some("#3A3A3A".to_string()),
            text: Some("#E5E7EB".to_string()),
            accent: Some("#FF6600".to_string()),
            solo: Some("#FFCC00".to_string()),
            mute: Some("#00CC66".to_string()),
            rec: Some("#FF3333".to_string()),
        }
    }

    /// Returns `true` if no manual overrides are set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.background.is_none()
            && self.background_darker.is_none()
            && self.surface.is_none()
            && self.border.is_none()
            && self.text.is_none()
            && self.accent.is_none()
            && self.solo.is_none()
            && self.mute.is_none()
            && self.rec.is_none()
    }

    /// Validates all populated hex strings in this override struct.
    pub fn validate(&self) -> Result<(), String> {
        let fields = [
            ("background", &self.background),
            ("background_darker", &self.background_darker),
            ("surface", &self.surface),
            ("border", &self.border),
            ("text", &self.text),
            ("accent", &self.accent),
            ("solo", &self.solo),
            ("mute", &self.mute),
            ("rec", &self.rec),
        ];
        for (name, val) in fields {
            if let Some(hex) = val {
                parse_hex_color(hex).map_err(|e| format!("{name}: {e}"))?;
            }
        }
        Ok(())
    }

    /// Helper to parse an optional hex field.
    #[must_use]
    pub fn parsed_background(&self) -> Option<Color32> {
        self.background.as_deref().and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_background_darker(&self) -> Option<Color32> {
        self.background_darker
            .as_deref()
            .and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_surface(&self) -> Option<Color32> {
        self.surface.as_deref().and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_border(&self) -> Option<Color32> {
        self.border.as_deref().and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_text(&self) -> Option<Color32> {
        self.text.as_deref().and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_accent(&self) -> Option<Color32> {
        self.accent.as_deref().and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_solo(&self) -> Option<Color32> {
        self.solo.as_deref().and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_mute(&self) -> Option<Color32> {
        self.mute.as_deref().and_then(|s| parse_hex_color(s).ok())
    }

    #[must_use]
    pub fn parsed_rec(&self) -> Option<Color32> {
        self.rec.as_deref().and_then(|s| parse_hex_color(s).ok())
    }
}

/// Clamps a UI scale percentage to the supported `[50, 200]` range.
#[must_use]
pub fn clamp_ui_scale_percent(percent: u16) -> u16 {
    percent.clamp(UI_SCALE_MIN_PERCENT, UI_SCALE_MAX_PERCENT)
}

/// Converts a UI scale percentage (`50..=200`) into an `egui` zoom factor (`0.5..=2.0`).
#[must_use]
pub fn ui_scale_factor(percent: u16) -> f32 {
    f32::from(clamp_ui_scale_percent(percent)) / 100.0
}

/// Parses a `yaae_theme::ThemePreference` by name (case-insensitive, supports display names
/// and snake_case identifiers).
#[must_use]
pub fn parse_theme_preference(name: &str) -> Option<ThemePreference> {
    let normalized = name
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' ', '/'], "_");
    match normalized.as_str() {
        "default" => Some(ThemePreference::Default),
        "light" => Some(ThemePreference::Light),
        "dark" => Some(ThemePreference::Dark),
        "solarized_dark" | "solarizeddark" => Some(ThemePreference::SolarizedDark),
        "monokai" => Some(ThemePreference::Monokai),
        "dracula" => Some(ThemePreference::Dracula),
        "nord" => Some(ThemePreference::Nord),
        "gruvbox_dark" | "gruvboxdark" => Some(ThemePreference::GruvboxDark),
        "light_grayscale" | "lightgrayscale" => Some(ThemePreference::LightGrayscale),
        "dark_grayscale" | "darkgrayscale" => Some(ThemePreference::DarkGrayscale),
        "high_contrast_bw" | "high_contrast_b_w" | "highcontrastbw" => {
            Some(ThemePreference::HighContrastBw)
        }
        "high_contrast_color" | "highcontrastcolor" => Some(ThemePreference::HighContrastColor),
        "high_contrast_light" | "highcontrastlight" => Some(ThemePreference::HighContrastLight),
        "cream_low_contrast" | "creamlowcontrast" => Some(ThemePreference::CreamLowContrast),
        "medium_contrast" | "mediumcontrast" => Some(ThemePreference::MediumContrast),
        _ => None,
    }
}

/// Enforces zero-rounding (`Rounding::ZERO`) across all widgets, menus, windows, and popups
/// in an `egui::Visuals` struct for crisp hardware-cockpit borders.
pub fn enforce_zero_rounding(visuals: &mut Visuals) {
    visuals.window_rounding = Rounding::ZERO;
    visuals.menu_rounding = Rounding::ZERO;
    visuals.widgets.noninteractive.rounding = Rounding::ZERO;
    visuals.widgets.inactive.rounding = Rounding::ZERO;
    visuals.widgets.hovered.rounding = Rounding::ZERO;
    visuals.widgets.active.rounding = Rounding::ZERO;
    visuals.widgets.open.rounding = Rounding::ZERO;
}

/// Complete theme configuration state for `nkEditor4`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    /// Selected `yaae-theme` named theme (all 15 variants supported).
    pub preference: ThemePreference,
    /// Optional `twoBowls` background preset (`White`, `Cream`, `Black`, `Dark Grey`).
    pub background_preset: Option<TwoBowlsBackgroundPreset>,
    /// Manual hex color overrides for background, surface, border, accent, and LEDs.
    pub custom_overrides: CustomColorOverrides,
    /// Whether `HighContrastBw` renders status colors (meters, solo/mute/rec) in pure grayscale.
    pub true_black_and_white: bool,
    ///Whether to enforce zero-rounding (`Rounding::ZERO`) crisp borders across the UI.
    pub zero_rounding: bool,
    /// UI scale percentage (`50` to `200`, default `100`).
    pub ui_scale_percent: u16,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            preference: ThemePreference::Default,
            background_preset: None,
            custom_overrides: CustomColorOverrides::new(),
            true_black_and_white: false,
            zero_rounding: true,
            ui_scale_percent: UI_SCALE_DEFAULT_PERCENT,
        }
    }
}

impl ThemeSettings {
    /// Creates a new `ThemeSettings` for a specific `ThemePreference`.
    #[must_use]
    pub fn with_preference(preference: ThemePreference) -> Self {
        Self {
            preference,
            ..Self::default()
        }
    }

    /// Sets the `UI scale` percentage, automatically clamping to `50..=200`.
    pub fn set_ui_scale_percent(&mut self, percent: u16) {
        self.ui_scale_percent = clamp_ui_scale_percent(percent);
    }

    /// Returns the effective `egui` zoom factor (`0.5..=2.0`).
    #[must_use]
    pub fn zoom_factor(&self) -> f32 {
        ui_scale_factor(self.ui_scale_percent)
    }

    /// Builds the resolved [`AccentPalette`] for this theme configuration, applying any
    /// manual `CustomColorOverrides` on top of `yaae_theme::AccentPalette::for_theme`.
    #[must_use]
    pub fn resolved_palette(&self) -> AccentPalette {
        let mut palette = AccentPalette::for_theme(&self.preference, self.true_black_and_white);

        if let Some(accent) = self.custom_overrides.parsed_accent() {
            palette.accent = accent;
            palette.accent_strong = accent;
            palette.strip_selection_border = accent;
        }
        if let Some(solo) = self.custom_overrides.parsed_solo() {
            palette.solo_active_bg = solo;
            palette.strip_solo_text = solo;
        }
        if let Some(mute) = self.custom_overrides.parsed_mute() {
            palette.mute_active_bg = mute;
            palette.strip_mute_text = mute;
        }
        if let Some(rec) = self.custom_overrides.parsed_rec() {
            palette.record_arm_active_bg = rec;
            palette.strip_record_text = rec;
        }

        palette
    }

    /// Builds the resolved [`Visuals`] for this theme configuration, combining:
    /// 1. Base `Visuals::light()` or `Visuals::dark()` from `ThemePreference::is_light()`
    /// 2. `yaae_theme::ui_visuals::apply_ui_visuals` or High-Contrast specialized visuals
    /// 3. Optional `TwoBowlsBackgroundPreset` (`White`, `Cream`, `Black`, `Dark Grey`)
    /// 4. Optional `CustomColorOverrides`
    /// 5. Crisp zero-rounding (`Rounding::ZERO`)
    #[must_use]
    pub fn build_visuals(&self) -> Visuals {
        let mut vis = if self.preference.is_light() {
            Visuals::light()
        } else {
            Visuals::dark()
        };

        // Apply yaae-theme's per-theme UI visuals if available
        if !ui_visuals::apply_ui_visuals(self.preference, &mut vis) {
            // Handle Default, HighContrastBw, HighContrastColor, HighContrastLight
            match self.preference {
                ThemePreference::Default => {
                    let cfg = self.preference.color_config();
                    if let Ok(bg) = parse_hex_color(&cfg.background) {
                        vis.panel_fill = bg;
                        vis.window_fill = bg;
                    }
                }
                ThemePreference::HighContrastBw => {
                    vis.panel_fill = Color32::BLACK;
                    vis.window_fill = Color32::BLACK;
                    vis.extreme_bg_color = Color32::BLACK;
                    vis.override_text_color = Some(Color32::WHITE);
                    vis.window_stroke = Stroke::new(1.5, Color32::WHITE);
                    vis.widgets.noninteractive.bg_fill = Color32::BLACK;
                    vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::WHITE);
                    vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::WHITE);
                    vis.widgets.inactive.bg_fill = Color32::from_gray(25);
                    vis.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::WHITE);
                    vis.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::WHITE);
                    vis.widgets.hovered.bg_fill = Color32::from_gray(60);
                    vis.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::WHITE);
                    vis.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
                    vis.widgets.active.bg_fill = Color32::from_gray(90);
                    vis.widgets.active.bg_stroke = Stroke::new(2.0, Color32::WHITE);
                    vis.widgets.active.fg_stroke = Stroke::new(2.0, Color32::WHITE);
                }
                ThemePreference::HighContrastColor => {
                    let bg = Color32::from_rgb(5, 5, 10);
                    let cyan = Color32::from_rgb(0, 255, 255);
                    let yellow = Color32::from_rgb(255, 255, 0);
                    vis.panel_fill = bg;
                    vis.window_fill = bg;
                    vis.extreme_bg_color = Color32::BLACK;
                    vis.override_text_color = Some(Color32::WHITE);
                    vis.window_stroke = Stroke::new(1.5, cyan);
                    vis.widgets.noninteractive.bg_fill = bg;
                    vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, cyan);
                    vis.widgets.inactive.bg_stroke = Stroke::new(1.0, cyan);
                    vis.widgets.hovered.bg_stroke = Stroke::new(1.5, yellow);
                    vis.widgets.active.bg_stroke = Stroke::new(2.0, yellow);
                }
                ThemePreference::HighContrastLight => {
                    vis.panel_fill = Color32::WHITE;
                    vis.window_fill = Color32::WHITE;
                    vis.extreme_bg_color = Color32::WHITE;
                    vis.override_text_color = Some(Color32::BLACK);
                    vis.window_stroke = Stroke::new(1.5, Color32::BLACK);
                    vis.widgets.noninteractive.bg_fill = Color32::WHITE;
                    vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::BLACK);
                    vis.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::BLACK);
                    vis.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::from_rgb(0, 0, 200));
                    vis.widgets.active.bg_stroke = Stroke::new(2.0, Color32::from_rgb(0, 0, 200));
                }
                _ => {}
            }
        }

        // Apply twoBowls background preset if selected
        if let Some(bg_preset) = self.background_preset {
            let bg = bg_preset.to_color32();
            vis.panel_fill = bg;
            vis.window_fill = bg;
            vis.widgets.noninteractive.bg_fill = bg;
            vis.widgets.noninteractive.weak_bg_fill = bg;
            if bg_preset.is_light() && !self.preference.is_light() {
                vis.override_text_color = Some(Color32::from_rgb(24, 24, 24));
            } else if !bg_preset.is_light() && self.preference.is_light() {
                vis.override_text_color = Some(Color32::from_rgb(235, 235, 235));
            }
        }

        // Apply manual hex overrides if provided
        if let Some(bg) = self.custom_overrides.parsed_background() {
            vis.panel_fill = bg;
            vis.window_fill = bg;
            vis.widgets.noninteractive.bg_fill = bg;
        }
        if let Some(bg_darker) = self.custom_overrides.parsed_background_darker() {
            vis.extreme_bg_color = bg_darker;
            vis.faint_bg_color = bg_darker;
            vis.code_bg_color = bg_darker;
        }
        if let Some(surface) = self.custom_overrides.parsed_surface() {
            vis.widgets.inactive.bg_fill = surface;
            vis.widgets.inactive.weak_bg_fill = surface;
        }
        if let Some(border) = self.custom_overrides.parsed_border() {
            vis.window_stroke = Stroke::new(1.0, border);
            vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
            vis.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
        }
        if let Some(text) = self.custom_overrides.parsed_text() {
            vis.override_text_color = Some(text);
            vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
            vis.widgets.inactive.fg_stroke = Stroke::new(1.0, text);
        }
        if let Some(accent) = self.custom_overrides.parsed_accent() {
            vis.selection.bg_fill = accent.linear_multiply(0.4);
            vis.selection.stroke = Stroke::new(1.0, accent);
            vis.hyperlink_color = accent;
            vis.widgets.hovered.bg_stroke = Stroke::new(1.0, accent);
            vis.widgets.active.bg_stroke = Stroke::new(1.5, accent);
        }

        if self.zero_rounding {
            enforce_zero_rounding(&mut vis);
        }

        vis
    }

    /// Installs this theme configuration into an [`egui::Context`]:
    /// - Sets `true_black_and_white` flag in `yaae_theme::theme_palette`
    /// - Installs the resolved [`AccentPalette`] into context memory
    /// - Applies the resolved [`Visuals`] (with `Rounding::ZERO`) to the context style
    /// - Sets `ctx.set_zoom_factor(self.zoom_factor())` for UI scaling (`50%..=200%`)
    pub fn apply_to_ctx(&self, ctx: &Context) {
        yaae_theme::theme_palette::set_true_black_and_white(self.true_black_and_white);
        let palette = self.resolved_palette();
        palette.install(ctx);

        let visuals = self.build_visuals();
        let mut style: Style = (*ctx.style()).clone();
        style.visuals = visuals;
        ctx.set_style(style);

        let zoom = self.zoom_factor();
        ctx.set_zoom_factor(zoom);
        ctx.options_mut(|opt| opt.zoom_factor = zoom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_15_yaae_themes_are_supported_and_distinct() {
        let all = ThemePreference::all();
        assert_eq!(all.len(), 15);

        let expected = [
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
        assert_eq!(all, &expected);

        let ctx = Context::default();
        for &pref in all {
            let parsed = parse_theme_preference(pref.display_name());
            assert_eq!(parsed, Some(pref), "Failed to parse {}", pref.display_name());

            let settings = ThemeSettings::with_preference(pref);
            let vis = settings.build_visuals();
            assert_eq!(vis.window_rounding, Rounding::ZERO);
            assert_eq!(vis.menu_rounding, Rounding::ZERO);
            assert_eq!(vis.widgets.inactive.rounding, Rounding::ZERO);
            assert_eq!(vis.widgets.hovered.rounding, Rounding::ZERO);
            assert_eq!(vis.widgets.active.rounding, Rounding::ZERO);

            settings.apply_to_ctx(&ctx);
            let installed = AccentPalette::from_ctx(&ctx);
            assert_eq!(
                installed.grayscale_accents,
                pref == ThemePreference::HighContrastBw
            );
        }
    }

    #[test]
    fn four_twobowls_background_presets_match_specification() {
        let presets = TwoBowlsBackgroundPreset::all();
        assert_eq!(presets.len(), 4);

        assert_eq!(TwoBowlsBackgroundPreset::White.hex(), "#FFFFFF");
        assert_eq!(
            TwoBowlsBackgroundPreset::White.to_color32(),
            Color32::from_rgb(0xFF, 0xFF, 0xFF)
        );
        assert!(TwoBowlsBackgroundPreset::White.is_light());

        assert_eq!(TwoBowlsBackgroundPreset::Cream.hex(), "#F8F2E4");
        assert_eq!(
            TwoBowlsBackgroundPreset::Cream.to_color32(),
            Color32::from_rgb(0xF8, 0xF2, 0xE4)
        );
        assert!(TwoBowlsBackgroundPreset::Cream.is_light());

        assert_eq!(TwoBowlsBackgroundPreset::Black.hex(), "#000000");
        assert_eq!(
            TwoBowlsBackgroundPreset::Black.to_color32(),
            Color32::from_rgb(0x00, 0x00, 0x00)
        );
        assert!(!TwoBowlsBackgroundPreset::Black.is_light());

        assert_eq!(TwoBowlsBackgroundPreset::DarkGrey.hex(), "#181818");
        assert_eq!(
            TwoBowlsBackgroundPreset::DarkGrey.to_color32(),
            Color32::from_rgb(0x18, 0x18, 0x18)
        );
        assert!(!TwoBowlsBackgroundPreset::DarkGrey.is_light());

        for &preset in presets {
            assert_eq!(
                TwoBowlsBackgroundPreset::from_name(preset.display_name()),
                Some(preset)
            );
            assert_eq!(TwoBowlsBackgroundPreset::from_name(preset.hex()), Some(preset));

            let mut settings = ThemeSettings::with_preference(ThemePreference::Dark);
            settings.background_preset = Some(preset);
            let vis = settings.build_visuals();
            assert_eq!(vis.panel_fill, preset.to_color32());
            assert_eq!(vis.window_fill, preset.to_color32());
        }
    }

    #[test]
    fn hex_parsing_and_formatting_roundtrip() {
        let c1 = parse_hex_color("#FF6600").unwrap();
        assert_eq!(c1, Color32::from_rgb(255, 102, 0));
        assert_eq!(format_hex_rgb(c1), "#FF6600");
        assert_eq!(format_hex_color(c1), "#FF6600");

        let c2 = parse_hex_color("f8f2e4").unwrap();
        assert_eq!(c2, Color32::from_rgb(0xF8, 0xF2, 0xE4));
        assert_eq!(format_hex_rgb(c2), "#F8F2E4");

        let c3 = parse_hex_color("#0F8").unwrap();
        assert_eq!(c3, Color32::from_rgb(0x00, 0xFF, 0x88));

        let c4 = parse_hex_color("#00FF80FF").unwrap();
        assert_eq!(c4, Color32::from_rgba_unmultiplied(0x00, 0xFF, 0x80, 0xFF));
        assert_eq!(format_hex_rgba(c4), "#00FF80FF");
        assert_eq!(format_hex_color(c4), "#00FF80");

        let c5 = parse_hex_color("#FF000080").unwrap();
        assert_eq!(c5, Color32::from_rgba_unmultiplied(0xFF, 0x00, 0x00, 0x80));
        assert_eq!(format_hex_rgba(c5), "#FF000080");
        assert_eq!(format_hex_color(c5), "#FF000080");

        assert!(parse_hex_color("#ZZZZZZ").is_err());
        assert!(parse_hex_color("#12345").is_err());
    }

    #[test]
    fn custom_color_overrides_apply_to_visuals_and_palette() {
        let mut settings = ThemeSettings::with_preference(ThemePreference::Dark);
        settings.custom_overrides = CustomColorOverrides::nkeditor3_classic();
        assert!(settings.custom_overrides.validate().is_ok());

        let vis = settings.build_visuals();
        assert_eq!(vis.panel_fill, Color32::from_rgb(0x1A, 0x1A, 0x1A));
        assert_eq!(vis.extreme_bg_color, Color32::from_rgb(0x0F, 0x0F, 0x0F));
        assert_eq!(
            vis.widgets.inactive.bg_fill,
            Color32::from_rgb(0x2A, 0x2A, 0x2A)
        );
        assert_eq!(vis.window_stroke.color, Color32::from_rgb(0x3A, 0x3A, 0x3A));

        let palette = settings.resolved_palette();
        assert_eq!(palette.accent, Color32::from_rgb(0xFF, 0x66, 0x00));
        assert_eq!(palette.solo_active_bg, Color32::from_rgb(0xFF, 0xCC, 0x00));
        assert_eq!(palette.mute_active_bg, Color32::from_rgb(0x00, 0xCC, 0x66));
        assert_eq!(
            palette.record_arm_active_bg,
            Color32::from_rgb(0xFF, 0x33, 0x33)
        );
    }

    #[test]
    fn ui_scaling_clamps_between_50_and_200_percent() {
        assert_eq!(clamp_ui_scale_percent(10), 50);
        assert_eq!(clamp_ui_scale_percent(50), 50);
        assert_eq!(clamp_ui_scale_percent(100), 100);
        assert_eq!(clamp_ui_scale_percent(150), 150);
        assert_eq!(clamp_ui_scale_percent(200), 200);
        assert_eq!(clamp_ui_scale_percent(350), 200);

        let mut settings = ThemeSettings::default();
        assert_eq!(settings.ui_scale_percent, 100);
        assert!((settings.zoom_factor() - 1.0).abs() < f32::EPSILON);

        settings.set_ui_scale_percent(25);
        assert_eq!(settings.ui_scale_percent, 50);
        assert!((settings.zoom_factor() - 0.5).abs() < f32::EPSILON);

        settings.set_ui_scale_percent(250);
        assert_eq!(settings.ui_scale_percent, 200);
        assert!((settings.zoom_factor() - 2.0).abs() < f32::EPSILON);

        let ctx = Context::default();
        settings.set_ui_scale_percent(125);
        settings.apply_to_ctx(&ctx);
        assert!((ctx.zoom_factor() - 1.25).abs() < f32::EPSILON);
    }
}
