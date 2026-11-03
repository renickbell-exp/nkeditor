//! Bidirectional UDP OSC (Open Sound Control) Server & Telemetry Broadcaster (`src/osc_server.rs`).
//!
//! Built on [`rosc`] over non-blocking UDP sockets.
//! - Default server listen port: `9000` ([`DEFAULT_OSC_LISTEN_PORT`])
//! - Default telemetry broadcast client port: `9001` ([`DEFAULT_OSC_CLIENT_PORT`])
//!
//! Handles the complete `nkEditor4` and `8opPerc` OSC address space specified in
//! `docs/NKEDITOR-RUST-PORT-SPEC.md`:
//! - `/nkeditor/status`
//! - `/nkeditor/device/list`
//! - `/nkeditor/device/select <device_id>`
//! - `/nkeditor/preset/load <preset_name_or_slot>`
//! - `/nkeditor/preset/save <preset_name>`
//! - `/nkeditor/control/set <track_or_path> [control_name] <value>`
//! - `/nkeditor/led/set <button_id> <on_off>`
//! - `/nkeditor/theme/set <theme_name> [bg_preset]`
//! - `/nkeditor/op8/state [json]`
//! - `/nkeditor/op8/form <direction_or_name>`
//! - `/nkeditor/op8/timbre <lane_or_all> [timbre_name]`
//! - `/nkeditor/op8/lane/<index>/density <value>`
//! - `/nkeditor/op8/lane/<index>/mute <0|1>`
//! - `/nkeditor/op8/lane/<index>/solo <0|1>`
//! - `/nkeditor/op8/snapshot/save [label]`

use crate::led_controller::{LedOperatingMode, MultiDeviceLedManager, DEFAULT_REROLL_FLASH_TICKS};
use crate::mapping_engine::{MultiDeviceMappingEngine, ProcessedMidiMessage};
use crate::midi_io::MultiDeviceManager;
use crate::theme::{parse_theme_preference, ThemeSettings, TwoBowlsBackgroundPreset};
use crate::ui_editor::ControllerEditorUiState;
use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};

/// Default UDP port on which the Rust `nkEditor4` OSC server listens (`9000`).
pub const DEFAULT_OSC_LISTEN_PORT: u16 = 9000;

/// Default UDP port to which `nkEditor4` broadcasts telemetry notifications (`9001`).
pub const DEFAULT_OSC_CLIENT_PORT: u16 = 9001;

/// Canonical 8-voice percussion family order matching `8operc-konduktiva`.
pub const OP8_FAMILY_ORDER: [&str; 8] = [
    "kick",
    "snare",
    "hat_closed",
    "hat_open",
    "clap",
    "tom",
    "rim",
    "cymbal",
];

// =============================================================================
// 1. 8opPerc Live Rig State Synchronized via OSC
// =============================================================================

/// State of a single percussion voice lane (`0..8`) in the `8opPerc` live rig.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8VoiceLaneState {
    /// 0-based lane index (`0..8`, corresponding to `track1..track8`).
    pub index: usize,
    /// Percussion family name (`"kick"`, `"snare"`, `"hat_closed"`, etc.).
    pub family: String,
    /// Active precomputed FM timbre preset name.
    pub timbre_name: String,
    /// Per-voice rhythmic density multiplier (`0.1 ..= 5.0`, default `1.0`).
    pub density: f32,
    /// Per-voice level/decay macro (`0..=127`, default `100`).
    pub level: u8,
    /// Whether this voice lane is muted (`trackN.mute`).
    pub muted: bool,
    /// Whether this voice lane is soloed (`trackN.solo`).
    pub soloed: bool,
    /// Number of times this lane's timbre has been rerolled during the session.
    pub reroll_count: u32,
}

/// Complete `8opPerc` live rig state synchronized bidirectionally between `nkEditor4` and Node.js.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op8RigState {
    /// Current form index within the 4,096-entry form reservoir (`0..4096`).
    pub active_form_index: usize,
    /// Human-readable active form identifier (e.g. `"form-0001-poly-grid"`).
    pub active_form_name: String,
    /// Total forms in the bank (`4096`).
    pub total_forms: usize,
    /// Current full 8-voice timbre set index within the 4,096-entry timbre reservoir (`0..4096`).
    pub active_timbre_set_index: usize,
    /// Total 8-voice timbre sets in the reservoir (`4096`).
    pub total_timbre_sets: usize,
    /// Tempo in beats per minute.
    pub bpm: f32,
    /// Global rhythmic density multiplier (`0.1 ..= 5.0`, default `1.0`).
    pub global_density: f32,
    /// Global feel scale (`swing` & `humanize`, `0.0 ..= 1.0`, default `0.5`).
    pub feel_scale: f32,
    /// Whether the Konduktiva transport is currently playing.
    pub playing: bool,
    /// Whether live WAV stem/mix capture is active.
    pub recording: bool,
    /// Current 0-indexed beat within the bar (`0..3`).
    pub beat_in_bar: u8,
    /// Labels of saved 8-voice timbre snapshots captured during the session.
    pub saved_snapshots: Vec<String>,
    /// The 8 percussion voice lanes.
    pub lanes: [Op8VoiceLaneState; 8],
}

impl Default for Op8RigState {
    fn default() -> Self {
        let lanes = std::array::from_fn(|idx| Op8VoiceLaneState {
            index: idx,
            family: OP8_FAMILY_ORDER[idx].to_string(),
            timbre_name: format!("{}-preset-0001", OP8_FAMILY_ORDER[idx]),
            density: 1.0,
            level: 100,
            muted: false,
            soloed: false,
            reroll_count: 0,
        });
        Self {
            active_form_index: 0,
            active_form_name: "form-0001-opening".to_string(),
            total_forms: 4096,
            active_timbre_set_index: 0,
            total_timbre_sets: 4096,
            bpm: 132.0,
            global_density: 1.0,
            feel_scale: 0.5,
            playing: false,
            recording: false,
            beat_in_bar: 0,
            saved_snapshots: Vec::new(),
            lanes,
        }
    }
}

impl Op8RigState {
    /// Navigates the 4,096-entry form bank (`"next"`, `"previous"` / `"prev"`, or explicit form name).
    pub fn navigate_form(&mut self, direction_or_name: &str) {
        let trimmed = direction_or_name.trim();
        match trimmed.to_ascii_lowercase().as_str() {
            "next" | "+1" => {
                self.active_form_index = (self.active_form_index + 1) % self.total_forms.max(1);
                self.active_form_name = format!("form-{:04}", self.active_form_index + 1);
            }
            "previous" | "prev" | "-1" => {
                let total = self.total_forms.max(1);
                self.active_form_index = (self.active_form_index + total - 1) % total;
                self.active_form_name = format!("form-{:04}", self.active_form_index + 1);
            }
            _ => {
                self.active_form_name = trimmed.to_string();
            }
        }
    }

    /// Rerolls or assigns a timbre for a single voice lane (`0..8`) or all 8 lanes (`"all"`).
    /// Returns `Some(lane_idx)` for a single-lane reroll or `None` for an all-lanes reroll.
    pub fn reroll_timbre(
        &mut self,
        lane_target: &str,
        explicit_timbre_name: Option<&str>,
    ) -> Option<usize> {
        let normalized = lane_target.trim().to_ascii_lowercase();
        if normalized == "all" || normalized == "*" {
            self.active_timbre_set_index =
                (self.active_timbre_set_index + 1) % self.total_timbre_sets.max(1);
            for lane in &mut self.lanes {
                lane.reroll_count += 1;
                lane.timbre_name = match explicit_timbre_name {
                    Some(name) => format!("{}-{name}", lane.family),
                    None => format!(
                        "{}-set-{:04}-r{}",
                        lane.family,
                        self.active_timbre_set_index + 1,
                        lane.reroll_count
                    ),
                };
            }
            return None;
        }

        // Parse lane index (0..7) or match family name ("kick", "snare", etc.)
        let lane_idx = normalized
            .parse::<usize>()
            .ok()
            .filter(|&i| i < 8)
            .or_else(|| {
                OP8_FAMILY_ORDER
                    .iter()
                    .position(|&fam| fam == normalized.as_str())
            })?;

        let lane = &mut self.lanes[lane_idx];
        lane.reroll_count += 1;
        lane.timbre_name = match explicit_timbre_name {
            Some(name) => name.to_string(),
            None => format!("{}-preset-{:04}", lane.family, lane.reroll_count + 1),
        };
        Some(lane_idx)
    }
}

// =============================================================================
// 2. Parsed OSC Command Enum & Address Space Parser
// =============================================================================

/// Strongly-typed command parsed from an incoming OSC message.
#[derive(Debug, Clone, PartialEq)]
pub enum OscCommand {
    /// `/nkeditor/status`
    QueryStatus,
    /// `/nkeditor/device/list`
    QueryDeviceList,
    /// `/nkeditor/device/select <device_id>`
    SelectDevice { device_id: String },
    /// `/nkeditor/preset/load <preset_name_or_slot>`
    LoadPreset { preset: String },
    /// `/nkeditor/preset/save <preset_name>`
    SavePreset { preset: String },
    /// `/nkeditor/control/set <track_or_path> [control_name] <value>`
    SetControl { control_path: String, value: u8 },
    /// `/nkeditor/led/set <button_id> <on_off>`
    SetLed { button_id: String, is_on: bool },
    /// `/nkeditor/theme/set <theme_name> [bg_preset]`
    SetTheme {
        theme_name: String,
        background_preset: Option<String>,
    },
    /// `/nkeditor/layer/select <page_1_to_4_or_name>`
    SelectLayer { page: String },
    /// `/nkeditor/op8/state [json_snapshot]`
    Op8State { payload_json: Option<String> },
    /// `/nkeditor/op8/form <direction_or_name>`
    Op8Form { target: String },
    /// `/nkeditor/op8/timbre <lane_or_all> [timbre_name]`
    Op8Timbre {
        lane_target: String,
        timbre_name: Option<String>,
    },
    /// `/nkeditor/op8/lane/<index>/density <value>`
    Op8LaneDensity { lane_index: usize, density: f32 },
    /// `/nkeditor/op8/lane/<index>/mute <0|1>`
    Op8LaneMute { lane_index: usize, muted: bool },
    /// `/nkeditor/op8/lane/<index>/solo <0|1>`
    Op8LaneSolo { lane_index: usize, soloed: bool },
    /// `/nkeditor/op8/snapshot/save [label]`
    Op8SaveSnapshot { label: Option<String> },
    /// `/nkeditor/panic` — Emergency All Notes Off, sustain reset, mute all 8opPerc lanes, and LED resync
    Panic,
}

/// Parses a `rosc::OscMessage` into a typed [`OscCommand`].
pub fn parse_osc_message(msg: &OscMessage) -> Result<OscCommand, String> {
    let addr = msg.addr.trim();
    match addr {
        "/nkeditor/status" => Ok(OscCommand::QueryStatus),
        "/nkeditor/panic" => Ok(OscCommand::Panic),
        "/nkeditor/device/list" => Ok(OscCommand::QueryDeviceList),
        "/nkeditor/device/select" => {
            let device_id = arg_as_string(msg.args.first())
                .ok_or_else(|| "/nkeditor/device/select requires <device_id>".to_string())?;
            Ok(OscCommand::SelectDevice { device_id })
        }
        "/nkeditor/preset/load" => {
            let preset = arg_as_string(msg.args.first()).ok_or_else(|| {
                "/nkeditor/preset/load requires <preset_name_or_slot>".to_string()
            })?;
            Ok(OscCommand::LoadPreset { preset })
        }
        "/nkeditor/preset/save" => {
            let preset = arg_as_string(msg.args.first())
                .ok_or_else(|| "/nkeditor/preset/save requires <preset_name>".to_string())?;
            Ok(OscCommand::SavePreset { preset })
        }
        "/nkeditor/control/set" => {
            // Supports either:
            //   3 args: <track_num_or_section> <control_name> <value>
            //   2 args: <dot_control_path> <value>
            if msg.args.len() >= 3 {
                let first = arg_as_string(msg.args.first()).unwrap_or_default();
                let control_name = arg_as_string(msg.args.get(1)).unwrap_or_default();
                let val = arg_as_u8(msg.args.get(2))
                    .ok_or_else(|| "/nkeditor/control/set missing numeric <value>".to_string())?;
                let prefix = if first.chars().all(|c| c.is_ascii_digit()) {
                    format!("track{first}")
                } else {
                    first
                };
                Ok(OscCommand::SetControl {
                    control_path: format!("{prefix}.{control_name}"),
                    value: val,
                })
            } else if msg.args.len() == 2 {
                let control_path = arg_as_string(msg.args.first()).unwrap_or_default();
                let value = arg_as_u8(msg.args.get(1))
                    .ok_or_else(|| "/nkeditor/control/set missing numeric <value>".to_string())?;
                Ok(OscCommand::SetControl {
                    control_path,
                    value,
                })
            } else {
                Err("/nkeditor/control/set requires (<track> <control> <value>) or (<path> <value>)".to_string())
            }
        }
        "/nkeditor/led/set" => {
            let button_id = arg_as_string(msg.args.first())
                .ok_or_else(|| "/nkeditor/led/set requires <button_id>".to_string())?;
            let is_on = arg_as_bool(msg.args.get(1))
                .ok_or_else(|| "/nkeditor/led/set requires <on_off>".to_string())?;
            Ok(OscCommand::SetLed { button_id, is_on })
        }
        "/nkeditor/theme/set" => {
            let theme_name = arg_as_string(msg.args.first())
                .ok_or_else(|| "/nkeditor/theme/set requires <theme_name>".to_string())?;
            let background_preset = arg_as_string(msg.args.get(1));
            Ok(OscCommand::SetTheme {
                theme_name,
                background_preset,
            })
        }
        "/nkeditor/layer/select" => {
            let page = arg_as_string(msg.args.first())
                .ok_or_else(|| "/nkeditor/layer/select requires <page>".to_string())?;
            Ok(OscCommand::SelectLayer { page })
        }
        "/nkeditor/op8/state" => {
            let payload_json = arg_as_string(msg.args.first());
            Ok(OscCommand::Op8State { payload_json })
        }
        "/nkeditor/op8/form" => {
            let target = arg_as_string(msg.args.first())
                .ok_or_else(|| "/nkeditor/op8/form requires <direction_or_name>".to_string())?;
            Ok(OscCommand::Op8Form { target })
        }
        "/nkeditor/op8/timbre" => {
            let lane_target = arg_as_string(msg.args.first())
                .ok_or_else(|| "/nkeditor/op8/timbre requires <lane_or_all>".to_string())?;
            let timbre_name = arg_as_string(msg.args.get(1));
            Ok(OscCommand::Op8Timbre {
                lane_target,
                timbre_name,
            })
        }
        "/nkeditor/op8/snapshot/save" => {
            let label = arg_as_string(msg.args.first());
            Ok(OscCommand::Op8SaveSnapshot { label })
        }
        other => {
            // Match "/nkeditor/op8/lane/<index>/<property>"
            if let Some(rest) = other.strip_prefix("/nkeditor/op8/lane/") {
                let (idx_str, prop) = rest
                    .split_once('/')
                    .ok_or_else(|| format!("Invalid lane OSC path: {other}"))?;
                let lane_index: usize = idx_str
                    .parse()
                    .map_err(|_| format!("Invalid lane index in {other}"))?;
                if lane_index >= 8 {
                    return Err(format!("Lane index {lane_index} out of range 0..=7"));
                }
                match prop {
                    "density" => {
                        let density = arg_as_f32(msg.args.first()).ok_or_else(|| {
                            format!("{other} requires float density value")
                        })?;
                        Ok(OscCommand::Op8LaneDensity {
                            lane_index,
                            density: density.clamp(0.1, 5.0),
                        })
                    }
                    "mute" => {
                        let muted = arg_as_bool(msg.args.first())
                            .ok_or_else(|| format!("{other} requires 0|1|bool"))?;
                        Ok(OscCommand::Op8LaneMute { lane_index, muted })
                    }
                    "solo" => {
                        let soloed = arg_as_bool(msg.args.first())
                            .ok_or_else(|| format!("{other} requires 0|1|bool"))?;
                        Ok(OscCommand::Op8LaneSolo { lane_index, soloed })
                    }
                    _ => Err(format!("Unknown op8 lane property '{prop}' in {other}")),
                }
            } else {
                Err(format!("Unsupported OSC address: {other}"))
            }
        }
    }
}

fn arg_as_string(arg: Option<&OscType>) -> Option<String> {
    match arg? {
        OscType::String(s) => Some(s.clone()),
        OscType::Int(i) => Some(i.to_string()),
        OscType::Long(l) => Some(l.to_string()),
        OscType::Float(f) => Some(f.to_string()),
        OscType::Double(d) => Some(d.to_string()),
        OscType::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn arg_as_f32(arg: Option<&OscType>) -> Option<f32> {
    match arg? {
        OscType::Float(f) => Some(*f),
        OscType::Double(d) => Some(*d as f32),
        OscType::Int(i) => Some(*i as f32),
        OscType::Long(l) => Some(*l as f32),
        OscType::String(s) => s.trim().parse::<f32>().ok(),
        _ => None,
    }
}

fn arg_as_u8(arg: Option<&OscType>) -> Option<u8> {
    let f = arg_as_f32(arg)?;
    Some(f.round().clamp(0.0, 127.0) as u8)
}

fn arg_as_bool(arg: Option<&OscType>) -> Option<bool> {
    match arg? {
        OscType::Bool(b) => Some(*b),
        OscType::Int(i) => Some(*i != 0),
        OscType::Long(l) => Some(*l != 0),
        OscType::Float(f) => Some(f.abs() > f32::EPSILON),
        OscType::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "on" | "yes" => Some(true),
            "0" | "false" | "off" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

// =============================================================================
// 3. Bidirectional UDP OSC Server & Telemetry Broadcaster
// =============================================================================

/// Non-blocking UDP OSC server and telemetry broadcaster for `nkEditor4`.
pub struct OscServer {
    socket: UdpSocket,
    /// Bound local socket address (e.g. `127.0.0.1:9000`).
    pub listen_addr: SocketAddr,
    /// Subscribed client broadcast target addresses (default `["127.0.0.1:9001"]`).
    pub client_targets: Vec<SocketAddr>,
    /// Total OSC messages received and dispatched.
    pub messages_received: u64,
    /// Total OSC telemetry messages transmitted to client targets.
    pub messages_sent: u64,
}

impl OscServer {
    /// Binds a non-blocking UDP OSC server on `listen_addr` (e.g. `"127.0.0.1:9000"`) with
    /// initial broadcast `client_targets` (e.g. `["127.0.0.1:9001"]`).
    pub fn bind(listen_addr: &str, client_targets: Vec<SocketAddr>) -> Result<Self, String> {
        let socket = UdpSocket::bind(listen_addr).map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        let bound_addr = socket.local_addr().map_err(|e| e.to_string())?;
        Ok(Self {
            socket,
            listen_addr: bound_addr,
            client_targets,
            messages_received: 0,
            messages_sent: 0,
        })
    }

    /// Binds using the standard defaults (`127.0.0.1:9000` listen, `127.0.0.1:9001` broadcast).
    pub fn bind_default() -> Result<Self, String> {
        let listen = format!("127.0.0.1:{DEFAULT_OSC_LISTEN_PORT}");
        let target: SocketAddr = format!("127.0.0.1:{DEFAULT_OSC_CLIENT_PORT}")
            .parse()
            .map_err(|e: std::net::AddrParseError| e.to_string())?;
        Self::bind(&listen, vec![target])
    }

    /// Adds a client broadcast target if not already registered.
    pub fn add_client_target(&mut self, target: SocketAddr) {
        if !self.client_targets.contains(&target) {
            self.client_targets.push(target);
        }
    }

    /// Encodes and broadcasts an [`OscMessage`] to all subscribed `client_targets`.
    pub fn broadcast_message(&mut self, msg: OscMessage) -> Result<usize, String> {
        let packet = OscPacket::Message(msg);
        let bytes = encoder::encode(&packet).map_err(|e| e.to_string())?;
        let mut sent_count = 0;
        for target in &self.client_targets {
            if self.socket.send_to(&bytes, target).is_ok() {
                sent_count += 1;
                self.messages_sent += 1;
            }
        }
        Ok(sent_count)
    }

    /// Emits an OSC notification when a physical MIDI movement or GUI edit produces a
    /// [`ProcessedMidiMessage`].
    pub fn notify_control_change(&mut self, msg: &ProcessedMidiMessage) {
        let osc_msg = OscMessage {
            addr: "/nkeditor/control/set".to_string(),
            args: vec![
                OscType::String(msg.device_key.clone()),
                OscType::String(msg.control_path.clone()),
                OscType::Int(i32::from(msg.output_value)),
                OscType::Int(i32::from(msg.output_channel)),
                OscType::Int(i32::from(msg.output_number)),
            ],
        };
        let _ = self.broadcast_message(osc_msg);
    }

    /// Emits an OSC notification when a hardware button LED changes state.
    pub fn notify_led_change(&mut self, device_key: &str, button_name: &str, is_on: bool) {
        let osc_msg = OscMessage {
            addr: "/nkeditor/led/set".to_string(),
            args: vec![
                OscType::String(button_name.to_string()),
                OscType::Int(i32::from(is_on)),
                OscType::String(device_key.to_string()),
            ],
        };
        let _ = self.broadcast_message(osc_msg);
    }

    /// Broadcasts `/nkeditor/op8/state` with the serialized JSON of [`Op8RigState`].
    pub fn broadcast_op8_state(&mut self, op8_state: &Op8RigState) {
        if let Ok(json) = serde_json::to_string(op8_state) {
            let _ = self.broadcast_message(OscMessage {
                addr: "/nkeditor/op8/state".to_string(),
                args: vec![OscType::String(json)],
            });
        }
    }

    /// Drains all pending UDP datagrams from the non-blocking socket, parses each into
    /// [`OscCommand`]s, applies them to the application state, and broadcasts telemetry replies.
    pub fn poll_and_dispatch(
        &mut self,
        ui_state: &mut ControllerEditorUiState,
        theme_settings: &mut ThemeSettings,
        device_manager: &mut MultiDeviceManager,
        mapping_engine: &mut MultiDeviceMappingEngine,
        led_manager: &mut MultiDeviceLedManager,
        op8_state: &mut Op8RigState,
    ) -> Vec<OscCommand> {
        let mut buf = [0u8; 8192];
        let mut executed = Vec::new();

        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((size, _src_addr)) => {
                    if let Ok((_, packet)) = decoder::decode_udp(&buf[..size]) {
                        let mut msgs = Vec::new();
                        collect_osc_messages(packet, &mut msgs);
                        for m in msgs {
                            if let Ok(cmd) = parse_osc_message(&m) {
                                self.messages_received += 1;
                                self.apply_command(
                                    &cmd,
                                    ui_state,
                                    theme_settings,
                                    device_manager,
                                    mapping_engine,
                                    led_manager,
                                    op8_state,
                                );
                                executed.push(cmd);
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }

        executed
    }

    /// Applies a single parsed [`OscCommand`] to the live application state and emits any
    /// corresponding OSC telemetry broadcast.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_command(
        &mut self,
        cmd: &OscCommand,
        ui_state: &mut ControllerEditorUiState,
        theme_settings: &mut ThemeSettings,
        device_manager: &mut MultiDeviceManager,
        mapping_engine: &mut MultiDeviceMappingEngine,
        led_manager: &mut MultiDeviceLedManager,
        op8_state: &mut Op8RigState,
    ) {
        match cmd {
            OscCommand::QueryStatus => {
                let status_json = serde_json::json!({
                    "active_device": ui_state.active_device_key,
                    "active_definition": ui_state.active_definition_id,
                    "theme": theme_settings.preference.display_name(),
                    "background_preset": theme_settings.background_preset.map(|p| p.display_name()),
                    "connected_devices": device_manager.connected_sessions().len(),
                    "active_form": op8_state.active_form_name,
                    "bpm": op8_state.bpm,
                })
                .to_string();
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/status".to_string(),
                    args: vec![OscType::String(status_json)],
                });
            }
            OscCommand::QueryDeviceList => {
                let list: Vec<_> = device_manager
                    .sessions
                    .values()
                    .map(|s| {
                        serde_json::json!({
                            "fingerprint": s.fingerprint.canonical_key(),
                            "display_name": s.display_name,
                            "definition_id": s.definition_id,
                            "state": s.state,
                            "alsa_seq_addr": s.alsa_seq_addr,
                            "active_preset": s.active_preset,
                        })
                    })
                    .collect();
                let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/device/list".to_string(),
                    args: vec![OscType::String(json)],
                });
            }
            OscCommand::SelectDevice { device_id } => {
                let def_id = if device_id.contains("midimix") || device_id.contains("09e8:0031") {
                    "akai-midimix"
                } else {
                    "nanokontrol2"
                };
                ui_state.select_device(device_id.clone(), def_id);
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/device/select".to_string(),
                    args: vec![OscType::String(device_id.clone())],
                });
            }
            OscCommand::LoadPreset { preset } => {
                let active_key = ui_state.active_device_key.clone();
                if let Some(session) = device_manager.sessions.get_mut(&active_key) {
                    session.active_preset = preset.clone();
                }
                if let (Some(pipe), Some(led_ctrl)) = (
                    mapping_engine.pipelines.get(&active_key),
                    led_manager.controllers.get_mut(&active_key),
                ) {
                    let _ = led_ctrl.on_preset_loaded(pipe);
                }
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/preset/load".to_string(),
                    args: vec![OscType::String(preset.clone())],
                });
            }
            OscCommand::SavePreset { preset } => {
                let active_key = ui_state.active_device_key.clone();
                if let Some(session) = device_manager.sessions.get_mut(&active_key) {
                    session.active_preset = preset.clone();
                }
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/preset/save".to_string(),
                    args: vec![OscType::String(preset.clone())],
                });
            }
            OscCommand::SetControl {
                control_path,
                value,
            } => {
                let active_key = ui_state.active_device_key.clone();
                if let Some(pipe) = mapping_engine.pipelines.get_mut(&active_key) {
                    let _ = pipe.set_target_value(control_path, *value);
                    if let Some(entry) = pipe.config.entries.get(control_path).cloned() {
                        if entry.is_button {
                            let is_on = *value >= 64;
                            let _ = pipe.set_button_state(control_path, is_on);
                            if let Some(led_ctrl) = led_manager.controllers.get_mut(&active_key) {
                                let _ = led_ctrl.set_led_by_name(control_path, is_on);
                            }
                        }
                    }
                }
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/control/set".to_string(),
                    args: vec![
                        OscType::String(control_path.clone()),
                        OscType::Int(i32::from(*value)),
                    ],
                });
            }
            OscCommand::SetLed { button_id, is_on } => {
                let active_key = ui_state.active_device_key.clone();
                if let Some(led_ctrl) = led_manager.controllers.get_mut(&active_key) {
                    if !led_ctrl.set_led_by_name(button_id, *is_on) {
                        if let Ok(num) = button_id.parse::<u8>() {
                            let _ = led_ctrl.set_led_by_number(num, *is_on);
                        }
                    }
                }
                self.notify_led_change(&active_key, button_id, *is_on);
            }
            OscCommand::SetTheme {
                theme_name,
                background_preset,
            } => {
                if let Some(pref) = parse_theme_preference(theme_name) {
                    theme_settings.preference = pref;
                }
                if let Some(bg_str) = background_preset {
                    theme_settings.background_preset = TwoBowlsBackgroundPreset::from_name(bg_str);
                }
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/theme/set".to_string(),
                    args: vec![OscType::String(
                        theme_settings.preference.display_name().to_string(),
                    )],
                });
            }
            OscCommand::SelectLayer { page } => {
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/layer/select".to_string(),
                    args: vec![OscType::String(page.clone())],
                });
            }
            OscCommand::Op8State { payload_json } => {
                if let Some(json) = payload_json {
                    if let Ok(parsed) = serde_json::from_str::<Op8RigState>(json) {
                        *op8_state = parsed;
                        sync_op8_to_leds(op8_state, led_manager, &ui_state.active_device_key);
                    }
                }
                self.broadcast_op8_state(op8_state);
            }
            OscCommand::Op8Form { target } => {
                op8_state.navigate_form(target);
                self.broadcast_op8_state(op8_state);
            }
            OscCommand::Op8Timbre {
                lane_target,
                timbre_name,
            } => {
                let single_lane =
                    op8_state.reroll_timbre(lane_target, timbre_name.as_deref());
                if let Some(led_ctrl) = led_manager
                    .controllers
                    .get_mut(&ui_state.active_device_key)
                {
                    led_ctrl.set_mode(LedOperatingMode::Op8Cockpit);
                    match single_lane {
                        Some(idx) => {
                            led_ctrl.trigger_op8_lane_reroll(idx, DEFAULT_REROLL_FLASH_TICKS);
                        }
                        None => {
                            led_ctrl.trigger_op8_all_lanes_reroll(DEFAULT_REROLL_FLASH_TICKS);
                        }
                    }
                }
                self.broadcast_op8_state(op8_state);
            }
            OscCommand::Op8LaneDensity {
                lane_index,
                density,
            } => {
                if let Some(lane) = op8_state.lanes.get_mut(*lane_index) {
                    lane.density = (*density).clamp(0.1, 5.0);
                }
                let _ = self.broadcast_message(OscMessage {
                    addr: format!("/nkeditor/op8/lane/{lane_index}/density"),
                    args: vec![OscType::Float(*density)],
                });
            }
            OscCommand::Op8LaneMute { lane_index, muted } => {
                if let Some(lane) = op8_state.lanes.get_mut(*lane_index) {
                    lane.muted = *muted;
                }
                sync_op8_to_leds(op8_state, led_manager, &ui_state.active_device_key);
                let _ = self.broadcast_message(OscMessage {
                    addr: format!("/nkeditor/op8/lane/{lane_index}/mute"),
                    args: vec![OscType::Int(i32::from(*muted))],
                });
            }
            OscCommand::Op8LaneSolo { lane_index, soloed } => {
                if let Some(lane) = op8_state.lanes.get_mut(*lane_index) {
                    lane.soloed = *soloed;
                }
                sync_op8_to_leds(op8_state, led_manager, &ui_state.active_device_key);
                let _ = self.broadcast_message(OscMessage {
                    addr: format!("/nkeditor/op8/lane/{lane_index}/solo"),
                    args: vec![OscType::Int(i32::from(*soloed))],
                });
            }
            OscCommand::Op8SaveSnapshot { label } => {
                let snap_label = label.clone().unwrap_or_else(|| {
                    format!("snapshot-{:04}", op8_state.saved_snapshots.len() + 1)
                });
                op8_state.saved_snapshots.push(snap_label.clone());
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/op8/snapshot/save".to_string(),
                    args: vec![OscType::String(snap_label)],
                });
            }
            OscCommand::Panic => {
                op8_state.playing = false;
                op8_state.recording = false;
                for lane in &mut op8_state.lanes {
                    lane.muted = true;
                    lane.soloed = false;
                }
                let keys: Vec<String> = led_manager.controllers.keys().cloned().collect();
                for key in &keys {
                    sync_op8_to_leds(op8_state, led_manager, key);
                }
                let _ = self.broadcast_message(OscMessage {
                    addr: "/nkeditor/panic".to_string(),
                    args: vec![OscType::String("all_notes_off_and_muted".to_string())],
                });
                self.broadcast_op8_state(op8_state);
            }
        }
    }
}

fn sync_op8_to_leds(
    op8_state: &Op8RigState,
    led_manager: &mut MultiDeviceLedManager,
    active_device_key: &str,
) {
    let Some(led_ctrl) = led_manager.controllers.get_mut(active_device_key) else {
        return;
    };
    led_ctrl.set_mode(LedOperatingMode::Op8Cockpit);
    let mut snap = led_ctrl.op8_snapshot.clone();
    snap.playing = op8_state.playing;
    snap.recording = op8_state.recording;
    snap.beat_in_bar = op8_state.beat_in_bar;
    for (idx, lane) in op8_state.lanes.iter().enumerate() {
        snap.lanes[idx].muted = lane.muted;
        snap.lanes[idx].soloed = lane.soloed;
    }
    led_ctrl.apply_op8_cockpit_snapshot(&snap);
}

fn collect_osc_messages(packet: OscPacket, out: &mut Vec<OscMessage>) {
    match packet {
        OscPacket::Message(m) => out.push(m),
        OscPacket::Bundle(b) => {
            for p in b.content {
                collect_osc_messages(p, out);
            }
        }
    }
}

// =============================================================================
// Unit & Deterministic UDP Loopback Integration Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::nanokontrol2_definition;
    use crate::mapping_engine::DeviceMappingConfig;
    use crate::theme::ThemePreference;
    use std::time::Duration;

    fn send_osc_udp(client: &UdpSocket, target: SocketAddr, addr: &str, args: Vec<OscType>) {
        let pkt = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args,
        });
        let bytes = encoder::encode(&pkt).expect("encode OSC");
        client.send_to(&bytes, target).expect("send UDP OSC");
    }

    #[test]
    fn test_osc_address_space_parsing_completeness() {
        let cases = vec![
            (
                OscMessage {
                    addr: "/nkeditor/status".to_string(),
                    args: vec![],
                },
                OscCommand::QueryStatus,
            ),
            (
                OscMessage {
                    addr: "/nkeditor/device/list".to_string(),
                    args: vec![],
                },
                OscCommand::QueryDeviceList,
            ),
            (
                OscMessage {
                    addr: "/nkeditor/device/select".to_string(),
                    args: vec![OscType::String("akai-midimix".to_string())],
                },
                OscCommand::SelectDevice {
                    device_id: "akai-midimix".to_string(),
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/preset/load".to_string(),
                    args: vec![OscType::String("nord-drum-live".to_string())],
                },
                OscCommand::LoadPreset {
                    preset: "nord-drum-live".to_string(),
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/preset/save".to_string(),
                    args: vec![OscType::String("my-preset".to_string())],
                },
                OscCommand::SavePreset {
                    preset: "my-preset".to_string(),
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/control/set".to_string(),
                    args: vec![
                        OscType::Int(1),
                        OscType::String("knob".to_string()),
                        OscType::Int(95),
                    ],
                },
                OscCommand::SetControl {
                    control_path: "track1.knob".to_string(),
                    value: 95,
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/led/set".to_string(),
                    args: vec![
                        OscType::String("track2.solo".to_string()),
                        OscType::Int(1),
                    ],
                },
                OscCommand::SetLed {
                    button_id: "track2.solo".to_string(),
                    is_on: true,
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/theme/set".to_string(),
                    args: vec![
                        OscType::String("gruvbox_dark".to_string()),
                        OscType::String("cream".to_string()),
                    ],
                },
                OscCommand::SetTheme {
                    theme_name: "gruvbox_dark".to_string(),
                    background_preset: Some("cream".to_string()),
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/op8/form".to_string(),
                    args: vec![OscType::String("next".to_string())],
                },
                OscCommand::Op8Form {
                    target: "next".to_string(),
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/op8/timbre".to_string(),
                    args: vec![OscType::String("all".to_string())],
                },
                OscCommand::Op8Timbre {
                    lane_target: "all".to_string(),
                    timbre_name: None,
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/op8/lane/3/density".to_string(),
                    args: vec![OscType::Float(3.25)],
                },
                OscCommand::Op8LaneDensity {
                    lane_index: 3,
                    density: 3.25,
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/op8/lane/0/mute".to_string(),
                    args: vec![OscType::Int(1)],
                },
                OscCommand::Op8LaneMute {
                    lane_index: 0,
                    muted: true,
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/op8/lane/5/solo".to_string(),
                    args: vec![OscType::Int(1)],
                },
                OscCommand::Op8LaneSolo {
                    lane_index: 5,
                    soloed: true,
                },
            ),
            (
                OscMessage {
                    addr: "/nkeditor/op8/snapshot/save".to_string(),
                    args: vec![OscType::String("jam-01".to_string())],
                },
                OscCommand::Op8SaveSnapshot {
                    label: Some("jam-01".to_string()),
                },
            ),
        ];

        for (msg, expected) in cases {
            let parsed = parse_osc_message(&msg).expect("valid OSC message");
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn test_deterministic_udp_loopback_osc_server_and_telemetry_broadcast() {
        let client_sock = UdpSocket::bind("127.0.0.1:0").expect("bind client UDP");
        client_sock
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let client_addr = client_sock.local_addr().unwrap();

        let mut server =
            OscServer::bind("127.0.0.1:0", vec![client_addr]).expect("bind server UDP");
        let server_addr = server.listen_addr;

        let mut ui_state = ControllerEditorUiState::default();
        let mut theme_settings = ThemeSettings::default();
        let mut device_manager = MultiDeviceManager::new();
        let mut mapping_engine = MultiDeviceMappingEngine::new();
        let mut led_manager = MultiDeviceLedManager::new();
        let mut op8_state = Op8RigState::default();

        mapping_engine.register_device_pipeline(
            "nanokontrol2",
            DeviceMappingConfig::from_device_definition(&nanokontrol2_definition()),
        );
        let _ = led_manager.ensure_controller("nanokontrol2", "nanokontrol2");

        // 1. Send /nkeditor/theme/set "dracula" "cream" over UDP loopback
        send_osc_udp(
            &client_sock,
            server_addr,
            "/nkeditor/theme/set",
            vec![
                OscType::String("dracula".to_string()),
                OscType::String("cream".to_string()),
            ],
        );

        // 2. Send /nkeditor/op8/form "next"
        send_osc_udp(
            &client_sock,
            server_addr,
            "/nkeditor/op8/form",
            vec![OscType::String("next".to_string())],
        );

        // 3. Send /nkeditor/op8/lane/2/mute 1 (Hat Closed lane mute -> lights track3.mute LED CC 50)
        send_osc_udp(
            &client_sock,
            server_addr,
            "/nkeditor/op8/lane/2/mute",
            vec![OscType::Int(1)],
        );

        // 4. Send /nkeditor/op8/snapshot/save "live-take-1"
        send_osc_udp(
            &client_sock,
            server_addr,
            "/nkeditor/op8/snapshot/save",
            vec![OscType::String("live-take-1".to_string())],
        );

        // Poll and dispatch on server
        let cmds = server.poll_and_dispatch(
            &mut ui_state,
            &mut theme_settings,
            &mut device_manager,
            &mut mapping_engine,
            &mut led_manager,
            &mut op8_state,
        );
        assert_eq!(cmds.len(), 4);
        assert_eq!(theme_settings.preference, ThemePreference::Dracula);
        assert_eq!(
            theme_settings.background_preset,
            Some(TwoBowlsBackgroundPreset::Cream)
        );
        assert_eq!(op8_state.active_form_index, 1);
        assert_eq!(op8_state.active_form_name, "form-0002");
        assert!(op8_state.lanes[2].muted);
        assert!(
            led_manager
                .controllers
                .get("nanokontrol2")
                .unwrap()
                .led_state_by_name("track3.mute"),
            "OSC /nkeditor/op8/lane/2/mute must illuminate nanoKONTROL2 track3.mute LED"
        );
        assert_eq!(op8_state.saved_snapshots, vec!["live-take-1".to_string()]);

        // Verify physical MIDI movement automatically emits OSC notification to subscribed client
        let msg = mapping_engine
            .pipelines
            .get_mut("nanokontrol2")
            .unwrap()
            .process_cc(1, 16, 112)
            .expect("process CC 16");
        server.notify_control_change(&msg);

        // Read back the broadcasted OSC packets on client_sock
        let mut recv_addrs = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok((sz, _)) = client_sock.recv_from(&mut buf) {
            if let Ok((_, OscPacket::Message(m))) = decoder::decode_udp(&buf[..sz]) {
                recv_addrs.push(m.addr);
            }
            if recv_addrs.len() >= 5 {
                break;
            }
        }
        assert!(recv_addrs.contains(&"/nkeditor/theme/set".to_string()));
        assert!(recv_addrs.contains(&"/nkeditor/op8/state".to_string()));
        assert!(recv_addrs.contains(&"/nkeditor/op8/lane/2/mute".to_string()));
        assert!(recv_addrs.contains(&"/nkeditor/op8/snapshot/save".to_string()));
        assert!(recv_addrs.contains(&"/nkeditor/control/set".to_string()));
    }

    #[test]
    fn test_rust_to_node_sdk_osc_udp_roundtrip() {
        use std::process::Command;

        // Bind Rust OscServer on an ephemeral port
        let mut server = OscServer::bind("127.0.0.1:0", Vec::new()).expect("bind Rust OscServer");
        let server_port = server.listen_addr.port();

        let mut ui_state = ControllerEditorUiState::default();
        let mut theme_settings = ThemeSettings::default();
        let mut device_manager = MultiDeviceManager::new();
        let mut mapping_engine = MultiDeviceMappingEngine::new();
        let mut led_manager = MultiDeviceLedManager::new();
        let mut op8_state = Op8RigState::default();

        mapping_engine.register_device_pipeline(
            "nanokontrol2",
            DeviceMappingConfig::from_device_definition(&nanokontrol2_definition()),
        );
        let _ = led_manager.ensure_controller("nanokontrol2", "nanokontrol2");

        // Run a Node.js child process that uses js/index.js (NkEditorClient) to send OSC commands to Rust
        let js_script = format!(
            r#"
            import {{ NkEditorClient }} from './js/index.js';
            const client = new NkEditorClient({{
              serverHost: '127.0.0.1',
              serverPort: {server_port},
              listenHost: '127.0.0.1',
              listenPort: 0,
            }});
            await client.connect();
            await client.setTheme('solarized_dark', 'dark_grey');
            await client.setControl(1, 'knob', 108);
            await client.setLed('track1.solo', true);
            await client.setOp8LaneDensity(0, 4.25);
            await client.navigateOp8Form('next');
            await client.close();
            "#
        );

        let status = Command::new("node")
            .args(["--input-type=module", "-e", &js_script])
            .status()
            .expect("spawn node for OSC roundtrip");
        assert!(status.success(), "Node.js NkEditorClient script must succeed");

        let dispatched = server.poll_and_dispatch(
            &mut ui_state,
            &mut theme_settings,
            &mut device_manager,
            &mut mapping_engine,
            &mut led_manager,
            &mut op8_state,
        );

        assert_eq!(dispatched.len(), 5);
        assert_eq!(theme_settings.preference, ThemePreference::SolarizedDark);
        assert_eq!(
            theme_settings.background_preset,
            Some(TwoBowlsBackgroundPreset::DarkGrey)
        );
        assert_eq!(
            mapping_engine
                .pipelines
                .get("nanokontrol2")
                .unwrap()
                .soft_takeover_status("track1.knob")
                .unwrap()
                .target_value,
            108
        );
        assert!(
            led_manager
                .controllers
                .get("nanokontrol2")
                .unwrap()
                .led_state_by_name("track1.solo")
        );
        assert!((op8_state.lanes[0].density - 4.25).abs() < 0.01);
        assert_eq!(op8_state.active_form_index, 1);
    }
}
