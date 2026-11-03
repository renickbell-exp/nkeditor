//! Real-Time MIDI & OSC Protocol Inspector, Burst Detector & Latency Diagnostics (`src/diagnostics.rs`).
//!
//! Provides:
//! 1. **Timestamped Multi-Protocol Event Log ([`DiagnosticsInspector`], [`DiagnosticEvent`])**:
//!    Logs incoming physical MIDI (`MidiIn`), remapped outgoing MIDI (`MidiOut`),
//!    hardware LED feedback updates (`LedFeedback`), and incoming/outgoing UDP OSC
//!    packets (`OscIn` / `OscOut`) with microsecond timestamps (`timestamp_us`).
//! 2. **Multi-Dimensional Filtering ([`DiagnosticFilter`])**:
//!    Filters traffic in real time by device key, MIDI channel (`1..=16`), MIDI message
//!    type (`Cc` vs `Note`), event category toggles, or OSC address prefix
//!    (e.g. `"/nkeditor/op8"`).
//! 3. **Burst & Gesture Detector ([`BurstGestureDetector`], [`DetectedGesture`])**:
//!    Detects both **Akai MIDImix 32-CC `Send All` bursts** within the canonical `80ms`
//!    (`80,000 us`) hardware gate window (`CC 16..=31` and `CC 46..=61`) and
//!    **Korg `nanoKONTROL2` `Marker Set (CC 60)`** atomic snapshot triggers.
//! 4. **Microsecond Mapping Latency Tracker ([`LatencyMetrics`]) & JSON Export**:
//!    Tracks min, max, last, and average mapping latency in microseconds (`us`) and
//!    exports diagnostic sessions to structured JSON.

use crate::devices::MidiMessageType;
use crate::mapping_engine::ProcessedMidiMessage;
use crate::op8_model::{MIDIMIX_SEND_ALL_CCS, NK2_CC_MARKER_SET};
use crate::theme::ThemeSettings;
use egui::{Color32, RichText, ScrollArea, Stroke, Ui};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical 80ms (`80,000` microseconds) gate window for Akai MIDImix 32-CC `Send All` hardware bursts.
pub const MIDIMIX_SEND_ALL_WINDOW_US: u64 = 80_000;

/// Default maximum number of diagnostic events retained in the rolling ring buffer.
pub const DEFAULT_MAX_DIAGNOSTIC_EVENTS: usize = 512;

// =============================================================================
// 1. Diagnostic Event Types & Microsecond Latency Telemetry
// =============================================================================

/// Discriminated payload for a logged MIDI, LED, or OSC diagnostic event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosticEventKind {
    /// Incoming physical MIDI packet from a hardware controller.
    MidiIn {
        device_key: String,
        message_type: MidiMessageType,
        channel: u8,
        midi_number: u8,
        value: u8,
        raw_bytes: [u8; 3],
    },
    /// Remapped outgoing MIDI packet emitted by [`crate::mapping_engine::DeviceMappingPipeline`].
    MidiOut {
        device_key: String,
        control_path: String,
        input_message_type: MidiMessageType,
        output_message_type: MidiMessageType,
        input_number: u8,
        output_number: u8,
        output_channel: u8,
        raw_input_value: u8,
        output_value: u8,
        latency_us: u64,
    },
    /// Outgoing hardware LED update transmitted by [`crate::led_controller::HardwareLedController`].
    LedFeedback {
        device_key: String,
        button_name: String,
        message_type: MidiMessageType,
        channel: u8,
        midi_number: u8,
        is_on: bool,
        raw_value: u8,
    },
    /// Incoming UDP OSC packet received by [`crate::osc_server::OscServer`].
    OscIn {
        address: String,
        args_summary: String,
        peer: String,
        byte_len: usize,
    },
    /// Outgoing UDP OSC packet broadcast to subscribed clients.
    OscOut {
        address: String,
        args_summary: String,
        peer: String,
        byte_len: usize,
    },
}

/// Timestamped entry in the [`DiagnosticsInspector`] event log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticEvent {
    /// Monotonic sequence number (`1, 2, ...`).
    pub seq: u64,
    /// Timestamp in microseconds since session start.
    pub timestamp_us: u64,
    /// Event payload.
    pub payload: DiagnosticEventKind,
}

impl DiagnosticEvent {
    /// Short badge label for tabular display (`MIDI IN`, `MIDI OUT`, `LED`, `OSC IN`, `OSC OUT`).
    #[must_use]
    pub const fn badge(&self) -> &'static str {
        match self.payload {
            DiagnosticEventKind::MidiIn { .. } => "MIDI IN ",
            DiagnosticEventKind::MidiOut { .. } => "MIDI OUT",
            DiagnosticEventKind::LedFeedback { .. } => "LED OUT ",
            DiagnosticEventKind::OscIn { .. } => "OSC IN  ",
            DiagnosticEventKind::OscOut { .. } => "OSC OUT ",
        }
    }

    /// Formats the event as a single tabular monospace line for the inspector log.
    #[must_use]
    pub fn format_log_line(&self) -> String {
        let ms = (self.timestamp_us as f64) / 1000.0;
        match &self.payload {
            DiagnosticEventKind::MidiIn {
                device_key,
                message_type,
                channel,
                midi_number,
                value,
                raw_bytes,
            } => format!(
                "#{:<4} {:>9.3}ms [MIDI IN ] dev={device_key} {:?} #{midi_number:<3} val={value:<3} ch={channel:<2} [{:02X} {:02X} {:02X}]",
                self.seq, ms, message_type, raw_bytes[0], raw_bytes[1], raw_bytes[2]
            ),
            DiagnosticEventKind::MidiOut {
                device_key,
                control_path,
                output_message_type,
                output_number,
                output_channel,
                output_value,
                latency_us,
                ..
            } => format!(
                "#{:<4} {:>9.3}ms [MIDI OUT] dev={device_key} {control_path:<16} -> {:?} #{output_number:<3} val={output_value:<3} ch={output_channel:<2} ({latency_us}us)",
                self.seq, ms, output_message_type
            ),
            DiagnosticEventKind::LedFeedback {
                device_key,
                button_name,
                message_type,
                channel,
                midi_number,
                is_on,
                raw_value,
            } => format!(
                "#{:<4} {:>9.3}ms [LED OUT ] dev={device_key} {button_name:<16} {:?} #{midi_number:<3} ch={channel:<2} state={} ({raw_value})",
                self.seq,
                ms,
                message_type,
                if *is_on { "ON " } else { "OFF" }
            ),
            DiagnosticEventKind::OscIn {
                address,
                args_summary,
                peer,
                byte_len,
            } => format!(
                "#{:<4} {:>9.3}ms [OSC IN  ] {address:<32} args=[{args_summary}] from={peer} ({byte_len}B)",
                self.seq, ms
            ),
            DiagnosticEventKind::OscOut {
                address,
                args_summary,
                peer,
                byte_len,
            } => format!(
                "#{:<4} {:>9.3}ms [OSC OUT ] {address:<32} args=[{args_summary}] to={peer} ({byte_len}B)",
                self.seq, ms
            ),
        }
    }
}

/// Microsecond mapping latency statistics across all evaluated MIDI messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatencyMetrics {
    /// Total remapped MIDI messages measured.
    pub sample_count: u64,
    /// Most recent mapping latency in microseconds.
    pub last_latency_us: u64,
    /// Minimum observed mapping latency in microseconds.
    pub min_latency_us: u64,
    /// Maximum observed mapping latency in microseconds.
    pub max_latency_us: u64,
    /// Running average mapping latency in microseconds.
    pub avg_latency_us: f64,
}

impl Default for LatencyMetrics {
    fn default() -> Self {
        Self {
            sample_count: 0,
            last_latency_us: 0,
            min_latency_us: 0,
            max_latency_us: 0,
            avg_latency_us: 0.0,
        }
    }
}

impl LatencyMetrics {
    /// Records a single mapping evaluation latency measurement in microseconds (`us`).
    pub fn record_sample(&mut self, latency_us: u64) {
        self.sample_count += 1;
        self.last_latency_us = latency_us;
        if self.sample_count == 1 {
            self.min_latency_us = latency_us;
            self.max_latency_us = latency_us;
            self.avg_latency_us = latency_us as f64;
        } else {
            self.min_latency_us = self.min_latency_us.min(latency_us);
            self.max_latency_us = self.max_latency_us.max(latency_us);
            let n = self.sample_count as f64;
            self.avg_latency_us =
                ((self.avg_latency_us * (n - 1.0)) + (latency_us as f64)) / n;
        }
    }
}

// =============================================================================
// 2. Burst & Gesture Detector (Akai MIDImix 32-CC 80ms Burst & nanoKONTROL2 CC 60)
// =============================================================================

/// High-level controller burst or snapshot gesture detected from incoming MIDI traffic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gesture_type", rename_all = "snake_case")]
pub enum DetectedGesture {
    /// Complete Akai MIDImix 32-CC `Send All` hardware dump received within the `80ms` (`80,000 us`) gate window.
    MidimixSendAllBurst {
        device_key: String,
        start_timestamp_us: u64,
        end_timestamp_us: u64,
        duration_us: u64,
        cc_values: BTreeMap<u8, u8>,
    },
    /// Korg `nanoKONTROL2` `Marker Set (CC 60)` atomic preset snapshot trigger.
    NanoKontrol2MarkerSetSnapshot {
        device_key: String,
        timestamp_us: u64,
        trigger_cc: u8,
        value: u8,
    },
}

/// Stateful detector for Akai MIDImix 32-CC `Send All` bursts (`80ms` gate window)
/// and Korg `nanoKONTROL2` `Marker Set (CC 60)` snapshot triggers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BurstGestureDetector {
    /// Gate window in microseconds for a 32-CC `Send All` burst (default `80_000` = `80ms`).
    pub send_all_window_us: u64,
    /// Rolling map of `cc_number -> (timestamp_us, value)` for the active MIDImix burst candidate.
    pending_midimix_ccs: BTreeMap<u8, (u64, u8)>,
    /// Device key associated with the current burst candidate.
    pending_device_key: String,
    /// History of all detected bursts and snapshot gestures.
    pub detected_gestures: Vec<DetectedGesture>,
}

impl Default for BurstGestureDetector {
    fn default() -> Self {
        Self::new(MIDIMIX_SEND_ALL_WINDOW_US)
    }
}

impl BurstGestureDetector {
    /// Creates a new [`BurstGestureDetector`] with the specified gate window in microseconds.
    #[must_use]
    pub const fn new(send_all_window_us: u64) -> Self {
        Self {
            send_all_window_us,
            pending_midimix_ccs: BTreeMap::new(),
            pending_device_key: String::new(),
            detected_gestures: Vec::new(),
        }
    }

    /// Inspects an incoming MIDI event at `timestamp_us` and returns `Some(DetectedGesture)`
    /// if it completes an Akai MIDImix 32-CC `Send All` burst within `send_all_window_us`
    /// or triggers a Korg `nanoKONTROL2` `Marker Set (CC 60)` snapshot.
    pub fn observe_midi_in(
        &mut self,
        device_key: &str,
        message_type: MidiMessageType,
        midi_number: u8,
        value: u8,
        timestamp_us: u64,
    ) -> Option<DetectedGesture> {
        if message_type != MidiMessageType::Cc {
            return None;
        }

        // 1. Check Korg nanoKONTROL2 Marker Set (CC 60) press (value >= 64)
        let is_nk2 = device_key.to_ascii_lowercase().contains("nanokontrol2")
            || device_key.contains("0944:0117");
        if is_nk2 && midi_number == NK2_CC_MARKER_SET && value >= 64 {
            let gesture = DetectedGesture::NanoKontrol2MarkerSetSnapshot {
                device_key: device_key.to_string(),
                timestamp_us,
                trigger_cc: midi_number,
                value,
            };
            self.detected_gestures.push(gesture.clone());
            return Some(gesture);
        }

        // 2. Check if this CC belongs to the 32 Akai MIDImix Send-All CCs (16..=31 or 46..=61)
        if MIDIMIX_SEND_ALL_CCS.contains(&midi_number) {
            if self.pending_device_key != device_key {
                self.pending_midimix_ccs.clear();
                self.pending_device_key = device_key.to_string();
            }

            // Evict any pending CCs older than `send_all_window_us` (80ms)
            let cutoff = timestamp_us.saturating_sub(self.send_all_window_us);
            self.pending_midimix_ccs
                .retain(|_, (ts, _)| *ts >= cutoff);

            self.pending_midimix_ccs
                .insert(midi_number, (timestamp_us, value));

            if self.pending_midimix_ccs.len() == MIDIMIX_SEND_ALL_CCS.len() {
                let start_ts = self
                    .pending_midimix_ccs
                    .values()
                    .map(|(ts, _)| *ts)
                    .min()
                    .unwrap_or(timestamp_us);
                let end_ts = self
                    .pending_midimix_ccs
                    .values()
                    .map(|(ts, _)| *ts)
                    .max()
                    .unwrap_or(timestamp_us);
                let duration_us = end_ts.saturating_sub(start_ts);

                if duration_us <= self.send_all_window_us {
                    let cc_values: BTreeMap<u8, u8> = self
                        .pending_midimix_ccs
                        .iter()
                        .map(|(&cc, &(_, val))| (cc, val))
                        .collect();
                    self.pending_midimix_ccs.clear();
                    let gesture = DetectedGesture::MidimixSendAllBurst {
                        device_key: device_key.to_string(),
                        start_timestamp_us: start_ts,
                        end_timestamp_us: end_ts,
                        duration_us,
                        cc_values,
                    };
                    self.detected_gestures.push(gesture.clone());
                    return Some(gesture);
                }
            }
        }

        None
    }
}

// =============================================================================
// 3. Multi-Dimensional Filter & Diagnostics Inspector State
// =============================================================================

/// Real-time filter criteria for the [`DiagnosticsInspector`] event stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFilter {
    /// Optional device key substring filter (e.g. `Some("nanokontrol2")` or `None` for All).
    pub device_key: Option<String>,
    /// Optional 1-indexed MIDI channel filter (`Some(1..=16)` or `None` for All).
    pub channel: Option<u8>,
    /// Optional MIDI message type filter (`Some(Cc)`, `Some(Note)`, or `None` for All).
    pub midi_message_type: Option<MidiMessageType>,
    /// Optional OSC address prefix filter (e.g. `"/nkeditor/op8"`).
    pub osc_address_prefix: String,
    /// Include `MidiIn` events.
    pub show_midi_in: bool,
    /// Include `MidiOut` events.
    pub show_midi_out: bool,
    /// Include `LedFeedback` events.
    pub show_led: bool,
    /// Include `OscIn` and `OscOut` events.
    pub show_osc: bool,
}

impl Default for DiagnosticFilter {
    fn default() -> Self {
        Self {
            device_key: None,
            channel: None,
            midi_message_type: None,
            osc_address_prefix: String::new(),
            show_midi_in: true,
            show_midi_out: true,
            show_led: true,
            show_osc: true,
        }
    }
}

impl DiagnosticFilter {
    /// Evaluates whether `event` passes all active filter criteria.
    #[must_use]
    pub fn matches(&self, event: &DiagnosticEvent) -> bool {
        match &event.payload {
            DiagnosticEventKind::MidiIn {
                device_key,
                message_type,
                channel,
                ..
            } => {
                if !self.show_midi_in {
                    return false;
                }
                if let Some(ref dev) = self.device_key {
                    if !device_key
                        .to_ascii_lowercase()
                        .contains(&dev.to_ascii_lowercase())
                    {
                        return false;
                    }
                }
                if let Some(ch) = self.channel {
                    if *channel != ch {
                        return false;
                    }
                }
                if let Some(mt) = self.midi_message_type {
                    if *message_type != mt {
                        return false;
                    }
                }
                true
            }
            DiagnosticEventKind::MidiOut {
                device_key,
                output_message_type,
                output_channel,
                ..
            } => {
                if !self.show_midi_out {
                    return false;
                }
                if let Some(ref dev) = self.device_key {
                    if !device_key
                        .to_ascii_lowercase()
                        .contains(&dev.to_ascii_lowercase())
                    {
                        return false;
                    }
                }
                if let Some(ch) = self.channel {
                    if *output_channel != ch {
                        return false;
                    }
                }
                if let Some(mt) = self.midi_message_type {
                    if *output_message_type != mt {
                        return false;
                    }
                }
                true
            }
            DiagnosticEventKind::LedFeedback {
                device_key,
                message_type,
                channel,
                ..
            } => {
                if !self.show_led {
                    return false;
                }
                if let Some(ref dev) = self.device_key {
                    if !device_key
                        .to_ascii_lowercase()
                        .contains(&dev.to_ascii_lowercase())
                    {
                        return false;
                    }
                }
                if let Some(ch) = self.channel {
                    if *channel != ch {
                        return false;
                    }
                }
                if let Some(mt) = self.midi_message_type {
                    if *message_type != mt {
                        return false;
                    }
                }
                true
            }
            DiagnosticEventKind::OscIn { address, .. }
            | DiagnosticEventKind::OscOut { address, .. } => {
                if !self.show_osc
                    || self.device_key.is_some()
                    || self.channel.is_some()
                    || self.midi_message_type.is_some()
                {
                    return false;
                }
                let prefix = self.osc_address_prefix.trim();
                if !prefix.is_empty() && !address.starts_with(prefix) {
                    return false;
                }
                true
            }
        }
    }
}

/// Complete Real-Time MIDI & OSC Protocol Inspector, Burst Detector, and Latency Tracker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsInspector {
    /// Rolling buffer of timestamped diagnostic events.
    pub events: Vec<DiagnosticEvent>,
    /// Maximum events retained in `events`.
    pub max_events: usize,
    /// Next sequence number.
    pub next_seq: u64,
    /// Active filter criteria.
    pub filter: DiagnosticFilter,
    /// Microsecond mapping latency statistics.
    pub latency: LatencyMetrics,
    /// Burst and snapshot gesture detector.
    pub burst_detector: BurstGestureDetector,
    /// Whether logging is paused by the user.
    pub paused: bool,
    /// Most recent JSON export string.
    pub last_exported_json: Option<String>,
}

impl Default for DiagnosticsInspector {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_DIAGNOSTIC_EVENTS)
    }
}

impl DiagnosticsInspector {
    /// Creates a new [`DiagnosticsInspector`] with the specified ring-buffer capacity.
    #[must_use]
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Vec::new(),
            max_events: max_events.max(16),
            next_seq: 1,
            filter: DiagnosticFilter::default(),
            latency: LatencyMetrics::default(),
            burst_detector: BurstGestureDetector::default(),
            paused: false,
            last_exported_json: None,
        }
    }

    fn push_event(&mut self, timestamp_us: u64, payload: DiagnosticEventKind) {
        if self.paused {
            return;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(DiagnosticEvent {
            seq,
            timestamp_us,
            payload,
        });
        if self.events.len() > self.max_events {
            self.events.remove(0);
        }
    }

    /// Logs an incoming physical MIDI message at `timestamp_us` and feeds the [`BurstGestureDetector`].
    pub fn log_midi_in(
        &mut self,
        timestamp_us: u64,
        device_key: impl Into<String>,
        message_type: MidiMessageType,
        channel: u8,
        midi_number: u8,
        value: u8,
        raw_bytes: [u8; 3],
    ) -> Option<DetectedGesture> {
        let dev = device_key.into();
        let gesture = self.burst_detector.observe_midi_in(
            &dev,
            message_type,
            midi_number,
            value,
            timestamp_us,
        );
        self.push_event(
            timestamp_us,
            DiagnosticEventKind::MidiIn {
                device_key: dev,
                message_type,
                channel,
                midi_number,
                value,
                raw_bytes,
            },
        );
        gesture
    }

    /// Logs a remapped outgoing [`ProcessedMidiMessage`] along with its mapping evaluation
    /// latency in microseconds (`latency_us`).
    pub fn log_midi_out(
        &mut self,
        timestamp_us: u64,
        msg: &ProcessedMidiMessage,
        latency_us: u64,
    ) {
        self.latency.record_sample(latency_us);
        self.push_event(
            timestamp_us,
            DiagnosticEventKind::MidiOut {
                device_key: msg.device_key.clone(),
                control_path: msg.control_path.clone(),
                input_message_type: msg.input_message_type,
                output_message_type: msg.output_message_type,
                input_number: msg.input_number,
                output_number: msg.output_number,
                output_channel: msg.output_channel,
                raw_input_value: msg.raw_input_value,
                output_value: msg.output_value,
                latency_us,
            },
        );
    }

    /// Logs a hardware LED feedback update.
    pub fn log_led_feedback(
        &mut self,
        timestamp_us: u64,
        device_key: impl Into<String>,
        button_name: impl Into<String>,
        message_type: MidiMessageType,
        channel: u8,
        midi_number: u8,
        is_on: bool,
        raw_value: u8,
    ) {
        self.push_event(
            timestamp_us,
            DiagnosticEventKind::LedFeedback {
                device_key: device_key.into(),
                button_name: button_name.into(),
                message_type,
                channel,
                midi_number,
                is_on,
                raw_value,
            },
        );
    }

    /// Logs an incoming UDP OSC packet.
    pub fn log_osc_in(
        &mut self,
        timestamp_us: u64,
        address: impl Into<String>,
        args_summary: impl Into<String>,
        peer: impl Into<String>,
        byte_len: usize,
    ) {
        self.push_event(
            timestamp_us,
            DiagnosticEventKind::OscIn {
                address: address.into(),
                args_summary: args_summary.into(),
                peer: peer.into(),
                byte_len,
            },
        );
    }

    /// Logs an outgoing UDP OSC packet.
    pub fn log_osc_out(
        &mut self,
        timestamp_us: u64,
        address: impl Into<String>,
        args_summary: impl Into<String>,
        peer: impl Into<String>,
        byte_len: usize,
    ) {
        self.push_event(
            timestamp_us,
            DiagnosticEventKind::OscOut {
                address: address.into(),
                args_summary: args_summary.into(),
                peer: peer.into(),
                byte_len,
            },
        );
    }

    /// Returns all logged events matching `self.filter`.
    #[must_use]
    pub fn filtered_events(&self) -> Vec<&DiagnosticEvent> {
        self.events
            .iter()
            .filter(|ev| self.filter.matches(ev))
            .collect()
    }

    /// Clears all logged events and detected gestures.
    pub fn clear(&mut self) {
        self.events.clear();
        self.burst_detector.detected_gestures.clear();
        self.latency = LatencyMetrics::default();
    }

    /// Exports the diagnostic log, latency telemetry, and detected gestures to pretty-printed JSON.
    pub fn export_diagnostics_json(&mut self) -> Result<String, String> {
        let filtered: Vec<&DiagnosticEvent> = self.filtered_events();
        let doc = serde_json::json!({
            "total_events": self.events.len(),
            "filtered_events_count": filtered.len(),
            "latency_metrics": self.latency,
            "detected_gestures": self.burst_detector.detected_gestures,
            "events": filtered,
        });
        let json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        self.last_exported_json = Some(json.clone());
        Ok(json)
    }
}

// =============================================================================
// 4. Interactive egui Protocol Inspector & Burst Diagnostics Panel
// =============================================================================

/// Renders the Real-Time MIDI & OSC Protocol Inspector, Burst Detector, and Latency panel in `egui`.
pub fn render_diagnostics_inspector_panel(
    ui: &mut Ui,
    inspector: &mut DiagnosticsInspector,
    theme_settings: &ThemeSettings,
) {
    let palette = theme_settings.resolved_palette();
    let panel_fill = theme_settings.build_visuals().panel_fill;

    egui::Frame::none()
        .fill(panel_fill)
        .stroke(Stroke::new(1.0, palette.accent_weak))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // 1. Header + Latency Telemetry + Export JSON
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("MIDI & OSC PROTOCOL INSPECTOR & BURST DETECTOR")
                        .strong()
                        .color(palette.accent),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Latency: last={}us  avg={:.1}us  min={}us  max={}us  (n={})",
                        inspector.latency.last_latency_us,
                        inspector.latency.avg_latency_us,
                        inspector.latency.min_latency_us,
                        inspector.latency.max_latency_us,
                        inspector.latency.sample_count
                    ))
                    .monospace()
                    .small(),
                );
                ui.separator();
                if ui
                    .small_button(if inspector.paused { "▶ Resume" } else { "⏸ Pause" })
                    .clicked()
                {
                    inspector.paused = !inspector.paused;
                }
                if ui.small_button("🗑 Clear").clicked() {
                    inspector.clear();
                }
                if ui.small_button("⬇ Export JSON").clicked() {
                    let _ = inspector.export_diagnostics_json();
                }
            });

            ui.add_space(4.0);

            // 2. Filter Bar (Event Types, Device, Channel, Message Type, OSC Prefix)
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut inspector.filter.show_midi_in, "MIDI In");
                ui.checkbox(&mut inspector.filter.show_midi_out, "MIDI Out");
                ui.checkbox(&mut inspector.filter.show_led, "LED");
                ui.checkbox(&mut inspector.filter.show_osc, "OSC");

                ui.separator();
                ui.label(RichText::new("Device:").small());
                for (lbl, val) in [
                    ("All", None),
                    ("nanoKONTROL2", Some("nanokontrol2".to_string())),
                    ("Akai MIDImix", Some("akai-midimix".to_string())),
                ] {
                    if ui
                        .selectable_label(inspector.filter.device_key == val, lbl)
                        .clicked()
                    {
                        inspector.filter.device_key = val;
                    }
                }

                ui.separator();
                ui.label(RichText::new("Type:").small());
                for (lbl, mt) in [
                    ("All", None),
                    ("CC", Some(MidiMessageType::Cc)),
                    ("Note", Some(MidiMessageType::Note)),
                ] {
                    if ui
                        .selectable_label(inspector.filter.midi_message_type == mt, lbl)
                        .clicked()
                    {
                        inspector.filter.midi_message_type = mt;
                    }
                }

                ui.separator();
                ui.label(RichText::new("OSC Prefix:").small());
                ui.add(
                    egui::TextEdit::singleline(&mut inspector.filter.osc_address_prefix)
                        .desired_width(130.0)
                        .hint_text("/nkeditor/op8..."),
                );
            });

            // 3. Burst & Gesture Detector Readout Strip
            if !inspector.burst_detector.detected_gestures.is_empty() {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "⚡ DETECTED GESTURES ({}):",
                            inspector.burst_detector.detected_gestures.len()
                        ))
                        .small()
                        .strong()
                        .color(palette.accent_strong),
                    );
                    for g in inspector.burst_detector.detected_gestures.iter().rev().take(3) {
                        let badge_txt = match g {
                            DetectedGesture::MidimixSendAllBurst {
                                device_key,
                                duration_us,
                                cc_values,
                                ..
                            } => format!(
                                "[MIDImix 32-CC Send-All: {} CCs in {:.2}ms ({device_key})]",
                                cc_values.len(),
                                (*duration_us as f64) / 1000.0
                            ),
                            DetectedGesture::NanoKontrol2MarkerSetSnapshot {
                                device_key,
                                timestamp_us,
                                trigger_cc,
                                ..
                            } => format!(
                                "[nK2 Marker Set CC {trigger_cc} Snapshot @ {:.2}ms ({device_key})]",
                                (*timestamp_us as f64) / 1000.0
                            ),
                        };
                        ui.label(RichText::new(badge_txt).monospace().small());
                    }
                });
            }

            ui.separator();

            // 4. Scrollable Monospace Event Stream
            let lines: Vec<String> = inspector
                .filtered_events()
                .into_iter()
                .map(DiagnosticEvent::format_log_line)
                .collect();

            ScrollArea::vertical()
                .id_salt("diagnostics_inspector_log_scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    for line in lines {
                        ui.label(RichText::new(line).monospace().small());
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
    use crate::mapping_engine::{DeviceMappingConfig, DeviceMappingPipeline};
    use crate::midi_io::RawMidiPacket;

    #[test]
    fn logs_and_filters_midi_in_out_led_and_osc_with_microsecond_latency() {
        let mut inspector = DiagnosticsInspector::new(128);
        let mut pipeline =
            DeviceMappingPipeline::new("nanokontrol2", DeviceMappingConfig::default_nanokontrol2());

        // 1. Incoming physical MIDI CC 16 on nanoKONTROL2
        let raw = RawMidiPacket::control_change(1, 16, 100);
        inspector.log_midi_in(
            1_000,
            "nanokontrol2",
            MidiMessageType::Cc,
            1,
            16,
            100,
            [raw.status, raw.data1, raw.data2],
        );

        // Evaluate through pipeline and log MidiOut with 14us latency
        let eval = pipeline
            .evaluate_raw_packet(raw)
            .into_emitted()
            .expect("emitted");
        inspector.log_midi_out(1_014, &eval, 14);

        // 2. Log a Note LED update on Akai MIDImix
        inspector.log_led_feedback(
            2_000,
            "akai-midimix",
            "track1.mute",
            MidiMessageType::Note,
            1,
            1,
            true,
            127,
        );

        // 3. Log OSC In and OSC Out packets
        inspector.log_osc_in(
            3_000,
            "/nkeditor/op8/lane/0/density",
            "2.5",
            "127.0.0.1:9001",
            36,
        );
        inspector.log_osc_out(3_050, "/nkeditor/status", "ok", "127.0.0.1:9001", 28);

        assert_eq!(inspector.events.len(), 5);
        assert_eq!(inspector.latency.sample_count, 1);
        assert_eq!(inspector.latency.last_latency_us, 14);

        // Filter by device = "akai-midimix" -> only the LED event matches
        inspector.filter.device_key = Some("akai-midimix".to_string());
        assert_eq!(inspector.filtered_events().len(), 1);
        inspector.filter.device_key = None;

        // Filter by midi_message_type = Note -> only the MIDImix Note LED event matches (plus OSC if enabled)
        inspector.filter.show_osc = false;
        inspector.filter.midi_message_type = Some(MidiMessageType::Note);
        assert_eq!(inspector.filtered_events().len(), 1);
        inspector.filter.midi_message_type = None;
        inspector.filter.show_osc = true;

        // Filter by OSC address prefix "/nkeditor/op8"
        inspector.filter.show_midi_in = false;
        inspector.filter.show_midi_out = false;
        inspector.filter.show_led = false;
        inspector.filter.osc_address_prefix = "/nkeditor/op8".to_string();
        assert_eq!(inspector.filtered_events().len(), 1);
    }

    #[test]
    fn burst_detector_identifies_midimix_32_cc_send_all_within_80ms_and_nanokontrol2_marker_set() {
        let mut inspector = DiagnosticsInspector::new(128);

        // 1. Korg nanoKONTROL2 Marker Set (CC 60, value 127) triggers NanoKontrol2MarkerSetSnapshot
        let g_nk2 = inspector
            .log_midi_in(
                10_000,
                "0944:0117::nanoKONTROL2",
                MidiMessageType::Cc,
                1,
                NK2_CC_MARKER_SET,
                127,
                [0xB0, NK2_CC_MARKER_SET, 127],
            )
            .expect("should detect Marker Set CC 60 snapshot trigger");
        assert!(matches!(
            g_nk2,
            DetectedGesture::NanoKontrol2MarkerSetSnapshot {
                trigger_cc: 60,
                value: 127,
                ..
            }
        ));

        // 2. Akai MIDImix 32-CC Send-All burst arriving within 48ms (<= 80ms gate window)
        let mut detected_burst = None;
        for (idx, &cc) in MIDIMIX_SEND_ALL_CCS.iter().enumerate() {
            let ts_us = 100_000 + (idx as u64) * 1_500; // 31 * 1.5ms = 46.5ms total
            let val = (idx as u8) * 3;
            if let Some(g) = inspector.log_midi_in(
                ts_us,
                "09e8:0031::akai-midimix",
                MidiMessageType::Cc,
                1,
                cc,
                val,
                [0xB0, cc, val],
            ) {
                detected_burst = Some(g);
            }
        }

        let burst = detected_burst.expect("32-CC burst within 80ms must be detected");
        match burst {
            DetectedGesture::MidimixSendAllBurst {
                duration_us,
                cc_values,
                ..
            } => {
                assert_eq!(duration_us, 46_500);
                assert!(duration_us <= MIDIMIX_SEND_ALL_WINDOW_US);
                assert_eq!(cc_values.len(), 32);
            }
            _ => panic!("Expected MidimixSendAllBurst"),
        }

        // 3. Export diagnostics to JSON and verify structure
        let json = inspector.export_diagnostics_json().expect("export json");
        assert!(json.contains("midimix_send_all_burst"));
        assert!(json.contains("nano_kontrol2_marker_set_snapshot"));
    }

    #[test]
    fn headless_egui_renders_diagnostics_inspector_panel_without_panic() {
        let ctx = egui::Context::default();
        let mut inspector = DiagnosticsInspector::new(64);
        inspector.log_midi_in(500, "nanokontrol2", MidiMessageType::Cc, 1, 60, 127, [0xB0, 60, 127]);
        let theme = ThemeSettings::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_diagnostics_inspector_panel(ui, &mut inspector, &theme);
            });
        });
    }
}
