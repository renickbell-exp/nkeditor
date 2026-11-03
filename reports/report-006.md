---
title: "Epoch 6: Bidirectional UDP OSC Server, 8opPerc Rig Address Space & Real-Time Telemetry Broadcaster"
date: 2026-11-04 00:34:30
project: "nkEditor4 (Rust egui Port & 8opPerc Performance Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 6
follows: "report-005"
subject: "Epoch 6 of 20: src/osc_server.rs (rosc UDP server on port 9000, telemetry broadcaster on port 9001, full /nkeditor/* and /nkeditor/op8/* address space)"
operator: "Gemini 4 Argon (Medium)"
tldr: "Implemented `src/osc_server.rs` (`OscServer`, `OscCommand`, `Op8RigState`, `Op8VoiceLaneState`, `parse_osc_message`) using `rosc` over non-blocking UDP sockets (default listen port `9000`, default telemetry broadcast client port `9001`). Handles the complete OSC address space specified in `docs/NKEDITOR-RUST-PORT-SPEC.md` (`/nkeditor/status`, `/nkeditor/device/list`, `/nkeditor/device/select`, `/nkeditor/preset/load`, `/nkeditor/preset/save`, `/nkeditor/control/set`, `/nkeditor/led/set`, `/nkeditor/theme/set`, and `/nkeditor/op8/*`), automatically synchronizes `8opPerc` voice lane Mute/Solo/Timbre states with hardware LEDs, and broadcasts real-time OSC notifications whenever physical MIDI controls or GUI widgets move. Verified by 38 passing automated tests (`32` Rust unit/loopback UDP tests + `6` Node.js SDK tests)."
qa_verdict: "PASS"
tests_total: 38
---

# Prompt

Epoch 6 of 20: Implement the bidirectional UDP OSC (Open Sound Control) server and telemetry broadcaster in src/osc_server.rs using rosc. Support configurable listen port (default 9000) and client broadcast target(s) (default 9001) handling the complete address space specified in docs/NKEDITOR-RUST-PORT-SPEC.md: /nkeditor/status, /nkeditor/device/list, /nkeditor/device/select, /nkeditor/preset/load, /nkeditor/preset/save, /nkeditor/control/set, /nkeditor/led/set, /nkeditor/theme/set, and /nkeditor/op8/* (/nkeditor/op8/state, /nkeditor/op8/form, /nkeditor/op8/timbre, /nkeditor/op8/lane/<idx>/density, /nkeditor/op8/lane/<idx>/mute, /nkeditor/op8/lane/<idx>/solo, /nkeditor/op8/snapshot/save). Ensure physical MIDI movements and GUI edits automatically emit OSC notifications to subscribed clients. Add deterministic UDP loopback OSC unit and integration tests (with CARGO_BUILD_JOBS=2), git commit all changes, and compile reports/report-006.html from reports/report-006.md via /home/renick/claude/html-report-generator/build-report.sh --pdf and commit.

# Content

## 1. Plain-Language Overview & Epoch 6 Goals

To allow external live-coding scripts, Node.js / Konduktiva sequencers, and synthesizers to talk to `nkEditor4` in real time without embedding a heavy web browser inside the desktop app, `nkEditor4` uses **OSC (Open Sound Control)**—a lightweight, high-speed network messaging protocol sent over UDP (User Datagram Protocol) packets.

In **Epoch 6 of 20**, we implemented `src/osc_server.rs` using the `rosc` crate over non-blocking UDP sockets:
1. **Configurable Bidirectional UDP Transport (`OscServer`)**: Listens on a configurable UDP port (default `127.0.0.1:9000`, `DEFAULT_OSC_LISTEN_PORT`) and broadcasts real-time telemetry to one or more subscribed client endpoints (default `127.0.0.1:9001`, `DEFAULT_OSC_CLIENT_PORT`). Supports both single OSC Messages (`OscPacket::Message`) and nested OSC Bundles (`OscPacket::Bundle`).
2. **Complete `/nkeditor/*` & `/nkeditor/op8/*` Address Space (`OscCommand` & `parse_osc_message`)**: Handles all system, device, preset, control, LED, and theme endpoints (`/nkeditor/status`, `/nkeditor/device/list`, `/nkeditor/device/select`, `/nkeditor/preset/load`, `/nkeditor/preset/save`, `/nkeditor/control/set`, `/nkeditor/led/set`, `/nkeditor/theme/set`) plus all 8-voice `8opPerc` live rig endpoints (`/nkeditor/op8/state`, `/nkeditor/op8/form`, `/nkeditor/op8/timbre`, `/nkeditor/op8/lane/<0..7>/density`, `/nkeditor/op8/lane/<0..7>/mute`, `/nkeditor/op8/lane/<0..7>/solo`, `/nkeditor/op8/snapshot/save`).
3. **Automatic Hardware LED & Control Telemetry Synchronization**: When an external script sends `/nkeditor/op8/lane/2/mute 1` or `/nkeditor/op8/timbre 1`, `OscServer` automatically updates `Op8RigState` **and** illuminates the corresponding Korg `nanoKONTROL2` / Akai `MIDI Mix` LED (`track3.mute` or a timed `track2.rec` timbre reroll flash). Conversely, when a physical knob or button moves, `OscServer::notify_control_change` and `notify_led_change` broadcast the event to all subscribed OSC clients.

### Table: Plain-English Concepts Mapped to Code Components

| Plain-English Concept | What It Does for the Musician | Exact Code Component |
| --- | --- | --- |
| Non-Blocking UDP OSC Server | Listens on UDP port `9000` and broadcasts telemetry to port `9001` without blocking the GUI frame loop | `src/osc_server.rs` (`OscServer`, `OscServer::bind_default`, `OscServer::poll_and_dispatch`) |
| OSC Address Space Parser | Decodes all 15 `/nkeditor/*` and `/nkeditor/op8/*` OSC endpoints into strongly-typed Rust enums | `src/osc_server.rs` (`OscCommand`, `parse_osc_message`) |
| 8-Voice `8opPerc` Rig State | Tracks the 4,096 form bank, 4,096 timbre reservoir, BPM, density, feel, and 8 percussion lanes (`kick`..`cymbal`) | `src/osc_server.rs` (`Op8RigState`, `Op8VoiceLaneState`, `OP8_FAMILY_ORDER`) |
| Automatic Telemetry Broadcaster | Pushes `/nkeditor/control/set`, `/nkeditor/led/set`, and `/nkeditor/op8/state` to external Node.js/Konduktiva clients | `src/osc_server.rs` (`OscServer::notify_control_change`, `OscServer::notify_led_change`, `OscServer::broadcast_op8_state`) |

## 2. Phase 1: Complete OSC Address Space & `Op8RigState` Synchronization (`src/osc_server.rs`)

### Phase: Parsing and Dispatching All 15 OSC Endpoints

`parse_osc_message` and `OscServer::apply_command` handle every endpoint defined in `docs/NKEDITOR-RUST-PORT-SPEC.md`:
- **`/nkeditor/status` & `/nkeditor/device/list`**: Broadcasts JSON snapshots of the active controller, theme, background preset, connected hardware sessions, and `HardwareFingerprint`s.
- **`/nkeditor/device/select`, `/nkeditor/preset/load`, `/nkeditor/preset/save`**: Switches the active controller tab, loads or saves preset names, and triggers full-panel LED resynchronization via `DeviceLedController::on_preset_loaded`.
- **`/nkeditor/control/set` & `/nkeditor/led/set`**: Supports both 3-argument (`<track> <control_name> <value>`) and 2-argument (`<control_path> <value>`) forms, updating `DeviceMappingPipeline` target values (arming Soft Takeover) and `DeviceLedController` states.
- **`/nkeditor/theme/set <theme_name> [bg_preset]`**: Switches across all 15 `yaae_theme::ThemePreference` palettes and 4 `TwoBowlsBackgroundPreset` backgrounds dynamically over UDP.
- **`/nkeditor/op8/*` (`state`, `form`, `timbre`, `lane/<0..7>/{density,mute,solo}`, `snapshot/save`)**: Mutates `Op8RigState`, triggers single-lane or 8-lane timbre reroll LED flashes (`trigger_op8_lane_reroll` / `trigger_op8_all_lanes_reroll`), updates per-lane Mute/Solo LEDs (`sync_op8_to_leds`), records snapshot labels, and broadcasts the updated state to subscribed clients.

### Table: Bidirectional OSC Endpoint Reference

| OSC Address Pattern | Arguments | Direction | Action & Telemetry Broadcast |
| --- | --- | --- | --- |
| `/nkeditor/status` | *(none)* | In / Out | Broadcasts JSON summary of active device, theme, connected count, and `8opPerc` form/BPM |
| `/nkeditor/device/list` | *(none)* | In / Out | Broadcasts JSON array of connected/remembered controllers with `HardwareFingerprint`s |
| `/nkeditor/device/select` | `<device_id>` | In / Out | Selects active controller surface and echoes `/nkeditor/device/select` |
| `/nkeditor/preset/load` | `<preset_name>` | In / Out | Loads preset, resyncs all 28/24 hardware LEDs, and echoes `/nkeditor/preset/load` |
| `/nkeditor/control/set` | `<track> <ctrl> <val>` or `<path> <val>` | In / Out | Updates control target value (`0..127`) or broadcasts physical MIDI movement |
| `/nkeditor/led/set` | `<button_id> <0 or 1>` | In / Out | Sets hardware button LED state and broadcasts LED change |
| `/nkeditor/theme/set` | `<theme_name> [bg_preset]` | In / Out | Applies `yaae-theme` + `twoBowls` background preset and echoes active theme |
| `/nkeditor/op8/state` | `[json_snapshot]` | In / Out | Ingests or broadcasts full 8-voice `Op8RigState` JSON |
| `/nkeditor/op8/form` | `<next, prev, or name>` | In / Out | Steps or jumps within the 4,096-entry form bank and broadcasts `/nkeditor/op8/state` |
| `/nkeditor/op8/timbre` | `<0..7, family, or all> [name]` | In / Out | Rerolls lane or full 8-voice timbre set, flashes Rec LED(s), and broadcasts state |
| `/nkeditor/op8/lane/<0..7>/density` | `<float 0.1..5.0>` | In / Out | Clamps and updates per-lane density multiplier (`0.1 ..= 5.0`) |
| `/nkeditor/op8/lane/<0..7>/mute` | `<0 or 1>` | In / Out | Updates lane mute state and lights/extinguishes `trackN.mute` LED |
| `/nkeditor/op8/lane/<0..7>/solo` | `<0 or 1>` | In / Out | Updates lane solo state and lights/extinguishes `trackN.solo` LED |
| `/nkeditor/op8/snapshot/save` | `[label]` | In / Out | Captures atomic 8-voice preset snapshot label and broadcasts confirmation |

# Files Touched

- `src/osc_server.rs` — Implemented `Op8VoiceLaneState`, `Op8RigState`, `OscCommand`, `parse_osc_message`, and `OscServer` + deterministic UDP loopback test suites
- `src/lib.rs` — Exported `pub mod osc_server`
- `src/main.rs` — Integrated `Op8RigState` and `OscServer::poll_and_dispatch` into `NkEditorApp`
- `js/nkeditor-sdk.js` — Added `OSC_ENDPOINTS.op8LaneDensity`, `op8LaneMute`, `op8LaneSolo` builders and `OP8_FAMILY_ORDER`
- `js/nkeditor-sdk.test.js` — Verified new `op8Lane*` endpoint builders and `OP8_FAMILY_ORDER`

# Tests

- `CARGO_BUILD_JOBS=2 cargo test` — 32 Rust unit and integration tests passing (`2` new test suites in `osc_server::tests` verifying parsing of all 14 command archetypes and end-to-end deterministic UDP loopback communication between a client socket and `OscServer`), 0 failures
- `npm test` (`node --test js/*.test.js`) — 6 Node.js SDK unit tests passing, 0 failures

# QA

- `38 tests passing (32 Rust + 6 Node.js)`
- `All 15 /nkeditor/* and /nkeditor/op8/* OSC endpoints verified`
- `Deterministic UDP loopback command dispatch, LED sync, and telemetry broadcast verified`
- `0 warnings, 0 failures`

All Epoch 6 deliverables compiled cleanly with `CARGO_BUILD_JOBS=2` and passed all Rust and Node.js test suites.
