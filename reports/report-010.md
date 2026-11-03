---
title: "nkEditor4 — Epoch 010: Interactive 8opPerc Performance Cockpit View in Rust egui"
date: 2026-11-04
project: "nkEditor4 (Rust egui MIDI/OSC Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 10
follows: "Report 009"
subject: "Interactive 8opPerc Performance Cockpit View (src/op8_cockpit.rs) with 8-Column Percussion Voice Matrix, Timbre Lock, and Bidirectional OSC/LED Wiring"
operator: "Antigravity Agent"
tldr: "Implemented the interactive 8opPerc Performance Cockpit view in Rust egui (src/op8_cockpit.rs), rendering an 8-column Percussion Voice Matrix aligned with the 8 physical tracks of the Korg nanoKONTROL2. Each lane displays its family name, active and queued next-bar timbre badges, interactive density ladder meter (0.1x to 5.0x), level/decay macro slider, Solo/Mute/Rec LED buttons, Timbre Lock toggle, and one-click Timbre Reroll button, wired bidirectionally to OscServer and MultiDeviceLedManager. All 41 Rust tests and 10 Node.js tests pass (51 total)."
qa_verdict: "PASS"
tests_total: 51
---

# Prompt

Build the interactive 8opPerc Performance Cockpit view in the Rust egui application (`src/op8_cockpit.rs`): render an 8-column Percussion Voice Matrix aligned with the 8 physical tracks of the Korg nanoKONTROL2. For each voice lane, display its family name, active timbre patch badge, queued next-bar timbre indicator, interactive density ladder meter (`0.1x` to `5.0x`), level/decay macro slider, Solo/Mute/Rec LED buttons, timbre Lock toggle (preventing global timbre set changes from overwriting locked lanes), and one-click Timbre Reroll button. Wire all cockpit interactions bidirectionally to the OSC server and hardware LED controller so GUI clicks update both the Konduktiva rig and the physical nanoKONTROL2 LEDs. Add automated tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-010.html` from `reports/report-010.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-English Overview: What Changed and Why It Matters

When performing live with the `8opPerc` percussion rig and a Korg `nanoKONTROL2`, the performer needs an at-a-glance visual dashboard ("cockpit") on their computer screen that lines up column-for-column with the 8 physical fader strips under their fingers. Without a visual cockpit, the performer cannot see which sound patch (`timbre`) is currently loaded on each drum voice, which new sound is waiting to drop on the next bar downbeat (`queued_timbre`), or what the exact note density multiplier (`0.1x` to `5.0x`) is for each lane.

In Epoch 10, we built `src/op8_cockpit.rs` to provide this interactive `egui` Performance Cockpit:

- **8-Column Percussion Voice Matrix Aligned with Korg nanoKONTROL2**: Renders 8 vertical voice columns (`T1 KICK` through `T8 CYMBAL`) styled with `yaae-theme` and `twoBowls` crisp zero-rounding borders (`Rounding::ZERO`). Each column shows the voice's family name, its Konduktiva live role (`kicks`, `snares`, `hats`, `toms`, `percussion`, `alt-toms`, `noise`, `fx`), and an `AUDIBLE` vs `SILENCED` status pill that reflects both Mute and Solo priority.
- **Active & Next-Bar Queued Timbre Badges**: Displays the currently sounding FM percussion patch alongside the queued patch scheduled for the next barline.
- **Interactive Density Ladder Meter & Macro Slider**: Combines a custom vector progress bar and logarithmic slider (`0.10x` to `5.00x`, displaying both per-lane density and effective `global * lane` density) with a `0..127` level/decay macro slider.
- **Per-Lane Timbre Lock Toggle**: Added `timbre_locked` to `Op8VoiceLane` and a `LOCKED / UNLOCKED` button on every column. When a performer locks a favorite kick or snare sound, pressing `Cycle` (`CC 46`) to switch the full 8-voice timbre set updates all unlocked voices while preserving the locked voices untouched.
- **Bidirectional OSC & Hardware LED Synchronization**: Clicking any Solo (`S`), Mute (`M`), Rec/Reroll (`R`), Density, Form, or Snapshot control in the `egui` cockpit immediately updates the physical LEDs on the Korg `nanoKONTROL2` via `MultiDeviceLedManager` and broadcasts the corresponding Open Sound Control (`OSC`) message to `Konduktiva` via `OscServer`.

| Plain-English Cockpit Element | Code Component (`src/op8_cockpit.rs`) | Hardware Control / LED | OSC Broadcast Endpoint |
| :--- | :--- | :--- | :--- |
| Cockpit State & Controller | `Op8CockpitViewState` | Syncs all 28 `nanoKONTROL2` LEDs | `/nkeditor/op8/state` |
| 8-Column Matrix Renderer | `render_op8_cockpit` / `render_voice_lane_column` | Aligned 1:1 with Tracks `1..8` | All `/nkeditor/op8/*` endpoints |
| Density Ladder Meter (`0.1x..5.0x`) | `Op8CockpitViewState::set_lane_density` | Track Knobs `CC 16..23` | `/nkeditor/op8/lane/<idx>/density` |
| Level / Decay Macro Slider | `Op8CockpitViewState::set_lane_gain_cc` | Track Sliders `CC 0..7` | `/nkeditor/control/set` |
| Solo Button (`S`) | `Op8CockpitViewState::toggle_lane_solo` | Solo Buttons & LEDs `CC 32..39` | `/nkeditor/op8/lane/<idx>/solo` |
| Mute Button (`M`) | `Op8CockpitViewState::toggle_lane_mute` | Mute Buttons & LEDs `CC 48..55` | `/nkeditor/op8/lane/<idx>/mute` |
| Single-Lane Timbre Reroll (`R`) | `Op8CockpitViewState::reroll_lane_timbre` | Rec Buttons & Flash LEDs `CC 64..71` | `/nkeditor/op8/timbre <lane> <name>` |
| Timbre Lock Toggle | `Op8CockpitViewState::toggle_lane_timbre_lock` | Protects lane during `Cycle CC 46` | Preserved in `/nkeditor/op8/state` |

# Files Touched

- `src/op8_cockpit.rs` (Created — 852 lines: `Op8CockpitViewState`, `render_op8_cockpit`, `render_cockpit_header_bar`, `render_voice_lane_column`, and 3 unit/integration tests)
- `src/op8_model.rs` (Modified — added `timbre_locked: bool` to `Op8VoiceLane` and updated `advance_full_timbre_set` to skip locked lanes)
- `src/lib.rs` (Modified — exported `pub mod op8_cockpit;`)

# Tests

Executed `CARGO_BUILD_JOBS=2 cargo test` and `npm test`:

| Test Suite | Test Name | Result |
| :--- | :--- | :--- |
| Rust (`src/op8_cockpit.rs`) | `op8_cockpit::tests::test_timbre_lock_prevents_global_set_from_overwriting_locked_lanes` | PASS |
| Rust (`src/op8_cockpit.rs`) | `op8_cockpit::tests::test_cockpit_interactions_drive_hardware_leds_and_udp_osc_telemetry` | PASS |
| Rust (`src/op8_cockpit.rs`) | `op8_cockpit::tests::test_headless_egui_renders_8_column_op8_cockpit_matrix` | PASS |
| Rust (All Modules) | Full Rust Unit & Integration Test Suite (`41` tests) | PASS (`41/41`) |
| Node.js (`npm test`) | Full Companion SDK & Live Rig Test Suite (`10` tests) | PASS (`10/10`) |

# QA

- **Headless `egui` Verification**: Verified `render_op8_cockpit` across a `1440x900` headless `egui::Context` frame with `ThemeSettings::default()`, confirming vector shape emission for all 8 voice columns and header controls without opening a window.
- **UDP Loopback OSC & Hardware LED Verification**: Confirmed that toggling Solo/Mute, rerolling timbres, and adjusting lane density in `Op8CockpitViewState` simultaneously updates `DeviceLedController` (`CC 33`, `CC 50`, `CC 64` LEDs) and emits UDP OSC datagrams (`/nkeditor/op8/lane/1/solo`, `/nkeditor/op8/lane/2/mute`, `/nkeditor/op8/timbre`, `/nkeditor/op8/lane/0/density`, `/nkeditor/op8/state`).
