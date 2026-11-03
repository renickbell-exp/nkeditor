//! Configuration Parser & Exporter (`src/config_parser.rs`).
//!
//! Supports lossless parsing and formatting of:
//! 1. **Legacy `nkEditor3` Human-Readable Text Mapping Files** (`mappings/default.txt`):
//!    - Optional `[device <id>]` header (defaults to `"nanokontrol2"` when omitted).
//!    - Section headers: `[track1]` through `[track8]`, `[transport]`, `[master]`, or any
//!      device-defined extra section.
//!    - Control lines: `controlType inputMidi -> outputMidi [chN] [toggle|momentary] ["label"]`
//!      supporting both plain CC numbers (`0..=127`) and `note:N` (`0..=127` Note messages).
//! 2. **JSON Preset Files** (`Preset` serialization with metadata, `DeviceMappingConfig`,
//!    saved continuous target values, and button states).

use crate::devices::{
    get_device_definition, ButtonBehavior, ControlDef, DeviceDefinition, MidiMessageType,
    DEFAULT_DEVICE_ID,
};
use crate::librarian::Preset;
use crate::mapping_engine::{ControlMappingEntry, DeviceMappingConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Error returned when parsing a text mapping file or JSON preset fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigParseError {
    /// 1-indexed line number where the error occurred (`0` for document-level errors).
    pub line_number: usize,
    /// Raw text of the offending line (if applicable).
    pub line_content: String,
    /// Human-readable explanation of what went wrong.
    pub message: String,
}

impl ConfigParseError {
    /// Creates a line-specific parse error.
    #[must_use]
    pub fn at_line(
        line_number: usize,
        line_content: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            line_number,
            line_content: line_content.into(),
            message: message.into(),
        }
    }

    /// Creates a document-level parse error.
    #[must_use]
    pub fn document(message: impl Into<String>) -> Self {
        Self {
            line_number: 0,
            line_content: String::new(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line_number > 0 {
            write!(
                f,
                "Line {}: {}\n  > {}",
                self.line_number, self.message, self.line_content
            )
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for ConfigParseError {}

/// Active section while scanning a legacy `.txt` mapping file.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ActiveSection {
    Track(usize),
    Extra(String),
}

impl ActiveSection {
    fn prefix(&self) -> String {
        match self {
            Self::Track(n) => format!("track{n}"),
            Self::Extra(name) => name.clone(),
        }
    }
}

/// Parses a MIDI token such as `"16"` (`Cc`, `16`) or `"note:25"` (`Note`, `25`).
fn parse_midi_token(
    token: &str,
    field_name: &str,
    line_number: usize,
    line: &str,
) -> Result<(MidiMessageType, u8), ConfigParseError> {
    let lower = token.to_ascii_lowercase();
    let (msg_type, num_str) = if let Some(rest) = lower.strip_prefix("note:") {
        (MidiMessageType::Note, rest)
    } else if let Some(rest) = lower.strip_prefix("cc:") {
        (MidiMessageType::Cc, rest)
    } else {
        (MidiMessageType::Cc, lower.as_str())
    };

    let parsed: i32 = num_str.parse().map_err(|_| {
        ConfigParseError::at_line(
            line_number,
            line,
            format!("Invalid {field_name} MIDI number: {token}"),
        )
    })?;

    if !(0..=127).contains(&parsed) {
        return Err(ConfigParseError::at_line(
            line_number,
            line,
            format!("{field_name} must be an integer between 0 and 127, got: {parsed}"),
        ));
    }

    Ok((msg_type, parsed as u8))
}

/// Scans lines for an optional `[device <id>]` header.
fn scan_device_header(lines: &[&str]) -> Result<Option<(String, usize)>, ConfigParseError> {
    for (idx, &raw_line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = trimmed[1..trimmed.len() - 1].trim();
            if inner.len() >= 6 && inner[..6].eq_ignore_ascii_case("device") {
                let rest = inner[6..].trim().to_ascii_lowercase();
                if rest.is_empty() {
                    return Err(ConfigParseError::at_line(
                        line_num,
                        raw_line,
                        "Missing device ID in [device] header",
                    ));
                }
                return Ok(Some((rest, line_num)));
            }
        }
    }
    Ok(None)
}

/// Finds a [`ControlDef`] within a [`DeviceDefinition`] given the active section and control name.
fn find_control_def<'a>(
    device: &'a DeviceDefinition,
    section: &ActiveSection,
    control_name: &str,
) -> Option<&'a ControlDef> {
    match section {
        ActiveSection::Track(_) => {
            let layout = device.track_layouts.first()?;
            for c in &layout.continuous_controls {
                if c.name.eq_ignore_ascii_case(control_name) {
                    return None; // Handled below via helper that returns Metadata
                }
            }
            None
        }
        ActiveSection::Extra(sec_name) => {
            let sec = device
                .extra_sections
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(sec_name))?;
            sec.controls
                .iter()
                .find(|c| c.name().eq_ignore_ascii_case(control_name))
        }
    }
}

/// Metadata resolved from the [`DeviceDefinition`] for a control in a section.
#[derive(Debug, Clone, Copy)]
struct ResolvedControlKind {
    is_button: bool,
    default_message_type: MidiMessageType,
    default_behavior: ButtonBehavior,
}

fn resolve_control_in_device(
    device: &DeviceDefinition,
    section: &ActiveSection,
    control_name: &str,
) -> Option<ResolvedControlKind> {
    match section {
        ActiveSection::Track(track_num) => {
            let idx = track_num.saturating_sub(1);
            let layout = device
                .track_layouts
                .get(idx)
                .or_else(|| device.track_layouts.first())?;
            for c in &layout.continuous_controls {
                if c.name.eq_ignore_ascii_case(control_name) {
                    return Some(ResolvedControlKind {
                        is_button: false,
                        default_message_type: c.message_type,
                        default_behavior: ButtonBehavior::Momentary,
                    });
                }
            }
            for b in &layout.button_controls {
                if b.name.eq_ignore_ascii_case(control_name) {
                    return Some(ResolvedControlKind {
                        is_button: true,
                        default_message_type: b.message_type,
                        default_behavior: b.default_behavior,
                    });
                }
            }
            None
        }
        ActiveSection::Extra(_) => {
            let def = find_control_def(device, section, control_name)?;
            match def {
                ControlDef::Continuous(c) => Some(ResolvedControlKind {
                    is_button: false,
                    default_message_type: c.message_type,
                    default_behavior: ButtonBehavior::Momentary,
                }),
                ControlDef::Button(b) => Some(ResolvedControlKind {
                    is_button: true,
                    default_message_type: b.message_type,
                    default_behavior: b.default_behavior,
                }),
            }
        }
    }
}

/// Parses a legacy `mappings/default.txt` format string into a [`DeviceMappingConfig`].
///
/// Supports both Korg `nanoKONTROL2` (default when `[device ...]` is omitted) and
/// Akai `MIDI Mix` (`[device akai-midimix]`), including `note:N` syntax for Note-based
/// buttons or cross-message transformations (`CC <-> Note`).
pub fn parse_legacy_mapping_text(content: &str) -> Result<DeviceMappingConfig, ConfigParseError> {
    let lines: Vec<&str> = content.lines().collect();
    let header = scan_device_header(&lines)?;
    let is_legacy_nk2 = header.is_none();
    let device_id = header
        .as_ref()
        .map(|(id, _)| id.as_str())
        .unwrap_or(DEFAULT_DEVICE_ID);

    let device = get_device_definition(device_id).ok_or_else(|| {
        let line_num = header.as_ref().map(|(_, ln)| *ln).unwrap_or(1);
        let raw_line = lines.get(line_num.saturating_sub(1)).copied().unwrap_or("");
        ConfigParseError::at_line(
            line_num,
            raw_line,
            format!("Unknown device ID: '{device_id}'. Registered devices: nanokontrol2, akai-midimix"),
        )
    })?;

    // Start from the device's default configuration so any unmentioned controls on non-legacy
    // configs still have valid defaults, while tracking defined sections for strict validation.
    let mut config = DeviceMappingConfig::from_device_definition(&device);
    let mut parsed_entries: BTreeMap<String, ControlMappingEntry> = BTreeMap::new();
    let mut current_section: Option<ActiveSection> = None;
    let mut defined_tracks = std::collections::BTreeSet::new();
    let mut has_transport = false;

    for (idx, &raw_line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Handle section header [trackN], [transport], [master], [device ...]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = trimmed[1..trimmed.len() - 1].trim().to_ascii_lowercase();
            if inner.starts_with("device ") || inner == "device" {
                continue;
            }
            if let Some(track_num_str) = inner.strip_prefix("track") {
                let track_num: usize = track_num_str.parse().map_err(|_| {
                    ConfigParseError::at_line(
                        line_num,
                        raw_line,
                        format!("Unknown section: {inner}"),
                    )
                })?;
                if track_num < 1 || track_num > device.track_count {
                    return Err(ConfigParseError::at_line(
                        line_num,
                        raw_line,
                        format!(
                            "Invalid track number: {track_num}. Must be between 1 and {}",
                            device.track_count
                        ),
                    ));
                }
                defined_tracks.insert(track_num);
                current_section = Some(ActiveSection::Track(track_num));
                continue;
            }

            let is_valid_extra = device
                .extra_sections
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(&inner));
            if is_valid_extra {
                if inner == "transport" {
                    has_transport = true;
                }
                current_section = Some(ActiveSection::Extra(inner));
                continue;
            }

            return Err(ConfigParseError::at_line(
                line_num,
                raw_line,
                format!("Unknown section: {inner}"),
            ));
        }

        // Must be inside a section
        let Some(ref section) = current_section else {
            return Err(ConfigParseError::at_line(
                line_num,
                raw_line,
                "Control mapping found outside of a section. Add a [trackN] or [transport] header first",
            ));
        };

        // Extract optional trailing quoted label: "Track 1 Knob"
        let (line_without_label, label) = extract_trailing_quoted_label(trimmed);

        let parts: Vec<&str> = line_without_label.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(ConfigParseError::at_line(
                line_num,
                raw_line,
                "Invalid control mapping format. Expected: controlType inputCC -> outputCC [chN] [behavior] [\"label\"]",
            ));
        }

        let control_name = parts[0].to_ascii_lowercase();
        let input_token = parts[1];
        let arrow = parts[2];
        let output_token = parts[3];

        if arrow != "->" {
            return Err(ConfigParseError::at_line(
                line_num,
                raw_line,
                format!("Expected '->' between input and output CC, got: {arrow}"),
            ));
        }

        let resolved = resolve_control_in_device(&device, section, &control_name).ok_or_else(|| {
            ConfigParseError::at_line(
                line_num,
                raw_line,
                format!(
                    "Invalid {} control type: {control_name}",
                    section.prefix()
                ),
            )
        })?;

        let (parsed_in_type, input_number) =
            parse_midi_token(input_token, "Input CC", line_num, raw_line)?;
        let (parsed_out_type, output_number) =
            parse_midi_token(output_token, "Output CC", line_num, raw_line)?;

        let mut output_channel: u8 = 1;
        let mut explicit_behavior: Option<ButtonBehavior> = None;

        for &token in &parts[4..] {
            let lower_tok = token.to_ascii_lowercase();
            if let Some(ch_str) = lower_tok.strip_prefix("ch") {
                let ch: i32 = ch_str.parse().map_err(|_| {
                    ConfigParseError::at_line(
                        line_num,
                        raw_line,
                        format!("Invalid channel specification: {token}"),
                    )
                })?;
                if !(1..=16).contains(&ch) {
                    return Err(ConfigParseError::at_line(
                        line_num,
                        raw_line,
                        format!("Channel must be an integer between 1 and 16, got: {ch}"),
                    ));
                }
                output_channel = ch as u8;
            } else if lower_tok == "toggle" {
                explicit_behavior = Some(ButtonBehavior::Toggle);
            } else if lower_tok == "momentary" {
                explicit_behavior = Some(ButtonBehavior::Momentary);
            } else {
                return Err(ConfigParseError::at_line(
                    line_num,
                    raw_line,
                    format!(
                        "Unknown token: {token}. Expected chN (channel) or toggle/momentary (behavior)"
                    ),
                ));
            }
        }

        if !resolved.is_button && explicit_behavior.is_some() {
            return Err(ConfigParseError::at_line(
                line_num,
                raw_line,
                format!(
                    "Behavior is not valid for continuous control type: {control_name}"
                ),
            ));
        }

        // Determine input and output message types:
        // If explicit `note:` prefix was used on input or output, honor it; otherwise fall back to device default.
        let in_has_prefix = input_token.to_ascii_lowercase().starts_with("note:")
            || input_token.to_ascii_lowercase().starts_with("cc:");
        let out_has_prefix = output_token.to_ascii_lowercase().starts_with("note:")
            || output_token.to_ascii_lowercase().starts_with("cc:");

        let input_message_type = if in_has_prefix {
            parsed_in_type
        } else {
            resolved.default_message_type
        };
        let output_message_type = if out_has_prefix {
            parsed_out_type
        } else {
            resolved.default_message_type
        };

        let control_path = format!("{}.{control_name}", section.prefix());
        let mut entry = if resolved.is_button {
            ControlMappingEntry::new_button(
                control_path.clone(),
                input_message_type,
                input_number,
                output_message_type,
                output_number,
                output_channel,
                explicit_behavior.unwrap_or(resolved.default_behavior),
            )
        } else {
            let mut c = ControlMappingEntry::new_continuous(
                control_path.clone(),
                input_number,
                output_number,
                output_channel,
            );
            c.input_message_type = input_message_type;
            c.output_message_type = output_message_type;
            c
        };
        entry.label = label;
        parsed_entries.insert(control_path, entry);
    }

    // For legacy nanoKONTROL2 files, verify all 8 tracks and [transport] were defined
    if is_legacy_nk2 {
        for t in 1..=8 {
            if !defined_tracks.contains(&t) {
                return Err(ConfigParseError::document(format!(
                    "Track {t} is not defined. All 8 tracks (track1-track8) must be present"
                )));
            }
            for req in ["knob", "slider", "solo", "mute", "rec"] {
                let path = format!("track{t}.{req}");
                if !parsed_entries.contains_key(&path) {
                    return Err(ConfigParseError::document(format!(
                        "Track {t} is missing required control: {req}"
                    )));
                }
            }
        }
        if !has_transport {
            return Err(ConfigParseError::document(
                "Transport section is not defined. A [transport] section is required",
            ));
        }
    }

    for (path, entry) in parsed_entries {
        config.entries.insert(path, entry);
    }

    Ok(config)
}

/// Extracts an optional trailing `"label"` from a control line.
fn extract_trailing_quoted_label(trimmed: &str) -> (&str, Option<String>) {
    if let Some(without_end_quote) = trimmed.strip_suffix('"') {
        if let Some(open_quote_idx) = without_end_quote.rfind('"') {
            let label = without_end_quote[open_quote_idx + 1..].to_string();
            let prefix = without_end_quote[..open_quote_idx].trim();
            return (prefix, Some(label));
        }
    }
    (trimmed, None)
}

/// Formats a MIDI number and message type for legacy `.txt` export (`"16"` for CC, `"note:1"` for Note).
fn format_midi_token(msg_type: MidiMessageType, number: u8, pad_width: usize) -> String {
    match msg_type {
        MidiMessageType::Cc => format!("{number:>pad_width$}"),
        MidiMessageType::Note => format!("note:{number}"),
    }
}

/// Formats a [`DeviceMappingConfig`] into the human-readable `mappings/default.txt` text format.
#[must_use]
pub fn format_mapping_to_text(config: &DeviceMappingConfig) -> String {
    let mut out = String::new();
    out.push_str("# nkEditor4 Mapping Configuration\n");
    out.push_str("# =========================================\n");

    if config.device_id != DEFAULT_DEVICE_ID {
        out.push_str(&format!("\n[device {}]\n", config.device_id));
    }

    let device = get_device_definition(&config.device_id);
    let track_count = device.as_ref().map(|d| d.track_count).unwrap_or(8);

    // Emit [track1]..[track8]
    for track_num in 1..=track_count {
        out.push_str(&format!("\n[track{track_num}]\n"));
        let control_order: Vec<String> = if let Some(ref dev) = device {
            let layout = dev
                .track_layouts
                .get(track_num - 1)
                .or_else(|| dev.track_layouts.first());
            if let Some(l) = layout {
                l.continuous_controls
                    .iter()
                    .map(|c| c.name.clone())
                    .chain(l.button_controls.iter().map(|b| b.name.clone()))
                    .collect()
            } else {
                vec![
                    "knob".into(),
                    "slider".into(),
                    "solo".into(),
                    "mute".into(),
                    "rec".into(),
                ]
            }
        } else {
            vec![
                "knob".into(),
                "slider".into(),
                "solo".into(),
                "mute".into(),
                "rec".into(),
            ]
        };

        for control_name in control_order {
            let path = format!("track{track_num}.{control_name}");
            if let Some(entry) = config.entries.get(&path) {
                out.push_str(&format_control_line(&control_name, entry, 7));
                out.push('\n');
            }
        }
    }

    // Emit extra sections ([transport], [master], etc.)
    if let Some(ref dev) = device {
        for section in &dev.extra_sections {
            out.push_str(&format!("\n[{}]\n", section.name));
            for control in &section.controls {
                let control_name = control.name();
                let path = format!("{}.{control_name}", section.name);
                if let Some(entry) = config.entries.get(&path) {
                    out.push_str(&format_control_line(control_name, entry, 12));
                    out.push('\n');
                }
            }
        }
    }

    out
}

fn format_control_line(
    control_name: &str,
    entry: &ControlMappingEntry,
    name_width: usize,
) -> String {
    let in_tok = format_midi_token(entry.input_message_type, entry.input_number, 2);
    let out_tok = format_midi_token(entry.output_message_type, entry.output_number, 2);
    let mut line = format!(
        "{control_name:<name_width$} {in_tok} -> {out_tok} ch{}",
        entry.output_channel
    );
    if entry.is_button {
        let beh = match entry.behavior {
            ButtonBehavior::Toggle => "toggle",
            ButtonBehavior::Momentary => "momentary",
        };
        line.push(' ');
        line.push_str(beh);
    }
    if let Some(ref label) = entry.label {
        line.push_str(&format!(" \"{label}\""));
    }
    line
}

// =============================================================================
// JSON Preset Import / Export & Auto-Detection
// =============================================================================

/// Serializes a [`Preset`] into pretty-printed JSON.
pub fn format_preset_json(preset: &Preset) -> Result<String, ConfigParseError> {
    serde_json::to_string_pretty(preset)
        .map_err(|e| ConfigParseError::document(format!("Failed to serialize preset JSON: {e}")))
}

/// Parses a [`Preset`] from a JSON string.
pub fn parse_preset_json(json_str: &str) -> Result<Preset, ConfigParseError> {
    serde_json::from_str::<Preset>(json_str)
        .map_err(|e| ConfigParseError::document(format!("Failed to parse preset JSON: {e}")))
}

/// Automatically detects whether `content` is a JSON [`Preset`] or a legacy
/// `mappings/default.txt` file and imports it as a [`Preset`].
pub fn import_preset_auto(
    content: &str,
    fallback_name: impl Into<String>,
) -> Result<Preset, ConfigParseError> {
    let trimmed = content.trim();
    if trimmed.starts_with('{') {
        parse_preset_json(trimmed)
    } else {
        let mapping = parse_legacy_mapping_text(content)?;
        let name = fallback_name.into();
        Ok(Preset::from_mapping(
            name,
            "Imported from legacy text mapping configuration.",
            "User",
            mapping,
        ))
    }
}
