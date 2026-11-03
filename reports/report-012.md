---
title: "nkEditor4 — Epoch 012: Multi-Page & Modifier Layer Engine with Soft Takeover"
date: 2026-11-04
project: "nkEditor4 (Rust egui MIDI/OSC Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 12
follows: "Report 011"
subject: "4-Page Controller Layer Engine (src/layer_manager.rs), Cycle+Track Modifier Combos, Automatic Cross-Page Soft Takeover, and /nkeditor/layer/select OSC Endpoint"
operator: "Antigravity Agent"
tldr: "Implemented the Multi-Page & Modifier Layer Engine in src/layer_manager.rs, enabling compact 8-knob/8-slider controllers like the Korg nanoKONTROL2 to control 64 continuous parameters across 4 switchable pages (Page 1: Voice Density & Level, Page 2: Timbre Morph & Decay, Page 3: Pitch/Spectra & Noise Filter, Page 4: Global Feel, Swing, Humanize & Master FX). Supports switching via Marker Left/Right (CC 61/62), Cycle+Track (CC 46 + CC 58/59) and Cycle+Solo 1..4 combos, GUI page tabs, and OSC (/nkeditor/layer/select), automatically engaging Soft Takeover and syncing hardware LEDs on every page switch. All 45 Rust tests and 10 Node.js tests pass (55 total)."
qa_verdict: "PASS"
tests_total: 55
---

# Prompt

Implement a Multi-Page & Modifier Layer Engine in `src/layer_manager.rs` so compact 8-knob/8-slider controllers like the Korg nanoKONTROL2 can control deep multi-row parameter sets (surpassing the 3-knob-row Akai MIDImix): provide 4 switchable controller pages (`Page 1`: Voice Density & Level, `Page 2`: Timbre Morph & Decay, `Page 3`: Pitch/Spectra & Noise Filter, `Page 4`: Global Feel, Swing, Humanize & Master FX) selectable via nanoKONTROL2 Marker Left/Right (`CC 61/62`), Cycle+Track combos, GUI page tabs, or OSC (`/nkeditor/layer/select`). Automatically engage Soft Takeover (Catch/Pickup) and update hardware LEDs whenever the active page changes so physical knob positions never cause parameter jumps across pages. Add automated tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-012.html` from `reports/report-012.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-English Overview: What Changed and Why It Matters

An Akai `MIDI Mix` has 3 rows of 8 knobs plus 1 row of 8 faders (32 continuous controls), whereas a Korg `nanoKONTROL2` is much more compact and portable—offering 1 row of 8 knobs and 1 row of 8 sliders (16 continuous controls) plus a full 11-button transport deck. Without a paging system, a `nanoKONTROL2` performer can only control 16 continuous parameters at a time. Worse, if a naive paging system simply switches what a physical knob controls, turning a knob that was left at `110` on Page 1 will immediately cause a jarring jump if the parameter on Page 2 was sitting at `64`.

In Epoch 12, we built `src/layer_manager.rs` to give the Korg `nanoKONTROL2` **4 switchable controller pages (64 continuous controls total—twice as many as an Akai MIDImix)** with **automatic cross-page Soft Takeover protection**:

- **Page 1 — Voice Density & Level (`VoiceDensityAndLevel`)**: Knobs `1..8` control per-voice note density (`CC 16..23`, Ch 1); Sliders `1..8` control per-voice level/gain (`CC 0..7`, Ch 1).
- **Page 2 — Timbre Morph & Decay (`TimbreMorphAndDecay`)**: Knobs `1..8` control per-voice FM timbre morph/index (`CC 24..31`, Ch 1); Sliders `1..8` control per-voice envelope decay (`CC 8..15`, Ch 1).
- **Page 3 — Pitch/Spectra & Noise Filter (`PitchSpectraAndNoiseFilter`)**: Knobs `1..8` control per-voice harmonic spectra (`CC 30` Nord Drum 2 `TONE_SPECTRA` on Channels `1..8`); Sliders `1..8` control per-voice noise filter cutoff (`CC 14` Nord Drum 2 `NOISE_FILTER_FREQ` on Channels `1..8`).
- **Page 4 — Global Feel, Swing, Humanize & Master FX (`GlobalFeelAndMasterFx`)**: Knobs `1..8` control Global Density, Feel Scale, Swing Depth, Humanize, Distortion Drive, EQ Gain, Delay Send, and Reverb Mix (`CC 58..65`, Ch 10); Sliders `1..8` control Master Bus Level, Sub Lift, Transient Punch, Stereo Width, Delay Feedback, Reverb Decay, Limiter Threshold, and Output Trim (`CC 70..77`, Ch 10).
- **Marker Left / Marker Right (`CC 61 / CC 62`) Page Stepping**: Single-button step backward or forward across Pages `1..4`.
- **Cycle + Track / Solo Modifier Combos**: Holding `Cycle` (`CC 46`) lights the Cycle LED and turns `Track Left/Right` (`CC 58/59`) into page steppers and `Solo 1..4` (`CC 32..35`) into direct jump buttons for Pages `1..4`.
- **Interactive `egui` Layer Page Selector Bar (`render_layer_page_selector_bar`)**: One-click GUI page tabs displaying the active page, `CYCLE MOD` status, Soft Takeover lock counter (`X/16 LOCKED`), and an 8-column parameter legend for all 8 knobs and 8 sliders.
- **Bidirectional UDP OSC (`/nkeditor/layer/select`)**: Added `OscCommand::SelectLayer` in `src/osc_server.rs` and `selectLayer(page)` in `js/nkeditor-sdk.js`.
- **Zero-Jump Soft Takeover Across Page Switches (`MultiPageLayerManager::select_page`)**: Whenever the user switches from Page A to Page B, `select_page` snapshots Page A's current targets, ensures Soft Takeover (`Pickup` or `Catch`) is active on `DeviceMappingPipeline`, loads Page B's stored targets against the current physical knob/slider positions, and locks any mismatched control until the physical knob crosses or catches Page B's target value.

| Plain-English Layer Feature | Rust Symbol (`src/layer_manager.rs`) | Hardware / OSC Trigger | Technical Behavior |
| :--- | :--- | :--- | :--- |
| 4 Controller Pages | `ControllerPage` (`ALL: [Self; 4]`) | Pages `1..4` (`64` continuous controls) | Models Density/Level, Morph/Decay, Pitch/Filter, and Global Feel/Master FX |
| Per-Page Parameter Bank | `PageParameterBank::for_page` | Stores `DeviceMappingConfig` + `continuous_targets` | Independent CC/Channel mappings, 16 continuous targets, and button states per page |
| Cross-Page Soft Takeover | `MultiPageLayerManager::select_page` | Automatic on every page switch | Snapshots outgoing page, loads incoming targets, and locks mismatched physical knobs |
| Hardware Shortcut Handler | `MultiPageLayerManager::handle_nanokontrol2_cc` | `CC 61/62`, `CC 46 + CC 58/59`, `CC 46 + CC 32..35` | Tracks `cycle_modifier_held`, switches pages, and illuminates `track1..4.solo` LED |
| `egui` Layer Selector Bar | `render_layer_page_selector_bar` | 4 GUI tabs + 8-column K/S label strip | Renders crisp `Rounding::ZERO` tabs and live `X/16 LOCKED` takeover indicator |
| OSC Layer Endpoint | `OscCommand::SelectLayer` / `NkEditorClient.selectLayer` | `/nkeditor/layer/select <page>` | Switches or broadcasts active layer page over UDP OSC |

# Files Touched

- `src/layer_manager.rs` (Created — 852 lines: `ControllerPage`, `PageParameterBank`, `LayerShortcutOutcome`, `MultiPageLayerManager`, `render_layer_page_selector_bar`, and 3 unit/integration tests)
- `src/osc_server.rs` (Modified — added `OscCommand::SelectLayer` and `/nkeditor/layer/select` parser/handler)
- `src/lib.rs` (Modified — exported `pub mod layer_manager;`)
- `js/nkeditor-sdk.js` (Modified — added `OSC_ENDPOINTS.layerSelect` and `NkEditorClient.prototype.selectLayer`)

# Tests

Executed `CARGO_BUILD_JOBS=2 cargo test` and `npm test`:

| Test Suite | Test Name | Result |
| :--- | :--- | :--- |
| Rust (`src/layer_manager.rs`) | `layer_manager::tests::test_four_controller_pages_and_soft_takeover_lock_on_page_switch` | PASS |
| Rust (`src/layer_manager.rs`) | `layer_manager::tests::test_marker_buttons_and_cycle_modifier_combos_switch_pages_and_update_leds_and_osc` | PASS |
| Rust (`src/layer_manager.rs`) | `layer_manager::tests::test_headless_egui_renders_layer_page_selector_bar` | PASS |
| Rust (All Modules) | Full Rust Unit & Integration Test Suite (`45` tests) | PASS (`45/45`) |
| Node.js (`npm test`) | Full Companion SDK & Live Rig Test Suite (`10` tests) | PASS (`10/10`) |

# QA

- **Cross-Page Soft Takeover Verification**: Confirmed in `test_four_controller_pages_and_soft_takeover_lock_on_page_switch` that sweeping Knob 1 (`CC 16`) to `110` on Page 1 and switching to Page 2 (where target is `64` on `CC 24`) preserves `110` in Page 1's bank, locks Knob 1 on Page 2 (`SoftTakeoverLocked` when moved to `100`), unlocks and emits `CC 24 = 60` once swept across `64`, and re-locks against `110` when returning to Page 1.
- **Modifier & LED Verification**: Confirmed that `Marker Right` (`CC 62`), `Cycle + Track Right` (`CC 46 + CC 59`), and `Cycle + Solo 4` (`CC 46 + CC 35`) switch pages, update `track1.solo..track4.solo` and `transport.cycle` LEDs, and broadcast `/nkeditor/layer/select` over UDP OSC.
