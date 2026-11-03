---
title: "Epoch 015 — Konduktiva MusicalEnvironment Bridge & Live 8-Lane IOI Rhythm Visualizer"
date: "2026-11-04"
project: "nkEditor4 (Pure Rust egui MIDI Controller Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 15
follows: "report-014"
subject: "Implementation of js/konduktiva-bridge.js and src/konduktiva_view.rs streaming QuantizedMap IOI rhythm rows, mask maps, ladder rungs, and note triggers over UDP OSC into a live 8-lane egui Rhythm Activity & Step Visualizer"
operator: "Antigravity (Google DeepMind)"
tldr: "Implemented the Konduktiva MusicalEnvironment & Rhythm-Map Ladder Bridge in js/konduktiva-bridge.js and the live 8-lane Rhythm Activity & Step Visualizer in src/konduktiva_view.rs: (1) models and extracts Konduktiva sparse-to-dense QuantizedMap rhythm ladders (5 rungs per voice lane), IOI (inter-onset interval) rows, maskMap flags, velocityMap values, and deterministic per-hit variation depth contours; (2) streams real-time per-lane rhythm rows (/nkeditor/konduktiva/lane/<idx>/rhythm) and note triggers (/nkeditor/konduktiva/lane/<idx>/trigger) over UDP OSC; and (3) renders an interactive 8-row custom vector IOI step visualizer in egui with moving playhead and trigger flashes. Verified with 57 passing Rust tests and 12 passing Node.js tests (69 total)."
qa_verdict: "PASS"
tests_total: 69
---

# Prompt

Epoch 15 of 20: Deepen the Konduktiva integration across `js/konduktiva-bridge.js` and `src/konduktiva_view.rs`: connect Konduktiva `MusicalEnvironment` and rhythm-map ladders (`/home/renick/claude/konduktiva` and `/home/renick/claude/8operc-konduktiva`) to stream real-time per-player IOI (inter-onset interval) rhythm rows, mask maps, density graph positions, and note trigger events over OSC into the Rust `egui` cockpit. Render a live 8-lane Rhythm Activity & Step Visualizer in `egui` so the performer can see upcoming rhythm density changes and voice triggers alongside physical controller movements. Add automated tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-015.html` from `reports/report-015.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-Language Overview: What Changed and Why It Matters

In traditional step sequencers, a drum pattern is just a fixed 16-step grid of on/off boxes. **Konduktiva** (`/home/renick/claude/konduktiva` and `/home/renick/claude/8operc-konduktiva/src/pulsed-core-rhythm.js`) works differently: instead of a rigid grid, each percussion voice (`Player`) reads a **QuantizedMap rhythm ladder** (`rhythmMap`)—a stack of 5 pre-calculated rhythmic rows ordered from sparse (few hits per bar) to dense (rapid 16th- and 32nd-note subdivisions). Each row stores the exact beat positions where notes start (`onset_beats`) and the **Inter-Onset Interval (IOI)**—the time gap in beats until the next hit (`ioi_values`). When a performer turns a physical density knob on the Korg `nanoKONTROL2` (`CC 16..23`), Konduktiva climbs up or down this 5-rung ladder (`selected_rung_index` out of `total_rungs`).

Before Epoch 15, the performer could see the knob's numeric density multiplier (`1.00x` to `5.00x`), but could not *see* the actual rhythmic pattern on screen or watch upcoming hits approach the playhead.

In Epoch 15, we connected Konduktiva's internal rhythm engine directly to the Rust `egui` cockpit through `js/konduktiva-bridge.js` and `src/konduktiva_view.rs`:

1. **Per-Player IOI Rhythm Row & Ladder Extraction (`js/konduktiva-bridge.js`)**: Extracts each voice lane's active `QuantizedMap` rung (`selectedRungIndex`, `totalRungs`), its `onsetBeats` and `ioiValues`, its `maskFlags` (`maskMap` indicating which steps are silenced or muted), its `velocities` (`velocityMap`), and its deterministic per-hit timbre variation depth (`VARIATION_DEPTH_RANGES`).
2. **Real-Time UDP OSC Streaming (`KonduktivaBridge`)**: Whenever a physical knob changes a lane's density or a Konduktiva `Player` fires a note, `KonduktivaBridge` streams `/nkeditor/konduktiva/lane/<idx>/rhythm` and `/nkeditor/konduktiva/lane/<idx>/trigger` packets over UDP OSC to the Rust application.
3. **Live 8-Lane Rhythm Activity & Step Visualizer (`src/konduktiva_view.rs`)**: Renders an 8-row custom vector timeline in `egui` (`render_konduktiva_rhythm_visualizer`) showing proportional IOI blocks across the loop `keyspan` (`4`, `8`, or `16` beats), muted/masked steps, the active ladder rung badge (`Rung 3/5`), a moving white playhead line, and the latest triggered MIDI note, velocity, IOI, and FM timbre patch.

## 1. Plain-Language to Technical Component Mapping

The table below maps each musical and visual concept in Epoch 15 to its exact implementation in `js/konduktiva-bridge.js` and `src/konduktiva_view.rs`:

| Plain-Language Concept | What It Means Musically | Node.js Component (`js/konduktiva-bridge.js`) | Rust Component (`src/konduktiva_view.rs`) |
|---|---|---|---|
| **Inter-Onset Interval (IOI)** | The exact beat duration from one drum hit to the next (`nextBeat - currentBeat`) inside a looping bar | `row.keys` (`onsetBeats`) & `row.values` (`ioiValues`) | `KonduktivaLaneRhythmRow::onset_beats` & `ioi_values` |
| **5-Rung Rhythm Ladder (`rhythmMap`)** | Stack of 5 rhythmic patterns per voice from Rung 1 (sparse) to Rung 5 (maximum subdivision) | `buildRhythmMapLadderData`, `selectLadderRung` | `build_rhythm_map_ladder`, `QuantizedRhythmRung`, `update_from_density` |
| **Step Mask Map (`maskMap`)** | Per-step boolean flag (`true` = silent/masked, `false` = audible hit) driven by pattern masks and lane Mute/Solo | `extractPlayerRhythmRow` (`maskFlags`) | `KonduktivaLaneRhythmRow::mask_flags`, `active_step_at_beat` |
| **Per-Hit Variation Contour** | Deterministic dual-sine wave (`VARIATION_DEPTH_RANGES`) that breathes timbre variation depth between accents | `VARIATION_DEPTH_RANGES`, `computeVariationDepth` | `VARIATION_DEPTH_RANGES`, `compute_variation_depth` |
| **Real-Time OSC Telemetry Stream** | UDP OSC messages carrying updated IOI rows and per-hit note triggers from Node.js to Rust | `KonduktivaBridge.syncLane`, `emitNoteTrigger`, `stepBeatWindow` | `KonduktivaTelemetryState::apply_lane_rhythm_json`, `record_note_trigger` |
| **8-Lane Step Visualizer (`egui`)** | 8-row vector timeline painting beat gridlines, proportional IOI blocks, active step highlight, and playhead | Driven by `KonduktivaBridge` & `Op8PerformanceModel` | `render_konduktiva_rhythm_visualizer` |

## 2. Per-Family Variation Depth Ranges (`VARIATION_DEPTH_RANGES`)

Both `js/konduktiva-bridge.js` and `src/konduktiva_view.rs` implement the exact per-family variation depth contours defined in `/home/renick/claude/8operc-konduktiva/src/pulsed-core-rhythm.js`:

| Lane Index | `nkEditor4` Family | `Konduktiva` Family | Variation Depth Min | Variation Depth Max | Native 4-Beat Onsets (Rung 3) |
|---|---|---|---|---|---|
| `0` | `kick` | `kicks` | `0.45` | `0.95` | `0.0, 1.5, 2.0, 3.25` |
| `1` | `snare` | `snares` | `0.82` | `1.45` | `1.0, 3.0` |
| `2` | `hat_closed` | `hats` | `0.72` | `1.55` | `0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5` |
| `3` | `hat_open` | `toms` | `0.82` | `1.50` | `0.75, 2.75` |
| `4` | `clap` | `bells` | `0.78` | `1.50` | `1.0, 2.5, 3.0` |
| `5` | `tom` | `metals` | `0.70` | `1.28` | `0.5, 1.75, 3.5` |
| `6` | `rim` | `noise` | `0.75` | `1.48` | `0.25, 1.25, 2.25, 3.25` |
| `7` | `cymbal` | `fx` | `0.72` | `1.50` | `0.0, 2.0` |

# Files Touched

- `js/konduktiva-bridge.js` (Created — 405 lines): Implemented `VARIATION_DEPTH_RANGES`, `buildRhythmMapLadderData`, `selectLadderRung`, `computeVariationDepth`, `extractPlayerRhythmRow`, and `KonduktivaBridge` with real-time UDP OSC streaming of IOI rhythm rows and note triggers.
- `src/konduktiva_view.rs` (Created — 556 lines): Implemented `KonduktivaNoteTrigger`, `QuantizedRhythmRung`, `KonduktivaLaneRhythmRow`, `KonduktivaTelemetryState`, `build_rhythm_map_ladder`, `compute_variation_depth`, `render_konduktiva_rhythm_visualizer`, and 3 unit and headless `egui` tests.
- `src/lib.rs` (Modified): Exported `pub mod konduktiva_view;`.
- `js/index.js` (Modified): Re-exported `./konduktiva-bridge.js`.
- `js/nkeditor-sdk.test.js` (Modified): Added automated UDP loopback OSC test verifying `KonduktivaBridge` ladder rung selection, mute mask propagation, beat-window note trigger firing, and `/nkeditor/konduktiva/lane/<idx>/rhythm` + `/trigger` packet delivery.
- `reports/report-015.md`, `reports/report-015.html`, `reports/report-015.pdf` (Created): Epoch 015 report in Markdown, grayscale HTML, and PDF formats.

# Tests

Executed both the Rust test suite (`CARGO_BUILD_JOBS=2 cargo test`) and the Node.js test suite (`npm test`):

| Test Suite | Test Name | What It Verifies | Status |
|---|---|---|---|
| `src/konduktiva_view.rs` | `rhythm_ladder_shifts_rungs_and_ioi_rows_as_density_changes` | Verifies that changing lane density from `1.0x` -> `0.2x` -> `3.5x` shifts `selected_rung_index` across the 5-rung `QuantizedMap` ladder (`2 -> 0 -> 4`) and muting sets all `mask_flags` to `true` | PASS |
| `src/konduktiva_view.rs` | `ingests_osc_json_rhythm_rows_and_note_triggers_from_konduktiva_bridge` | Verifies `apply_lane_rhythm_json` and `record_note_trigger` parse incoming JSON rhythm rows and note triggers from `KonduktivaBridge` and resolve `active_step_at_beat` | PASS |
| `src/konduktiva_view.rs` | `headless_egui_renders_konduktiva_8_lane_rhythm_visualizer_without_panic` | Verifies `render_konduktiva_rhythm_visualizer` paints all 8 IOI step strips, beat gridlines, playhead cursor, and trigger readouts in a headless `egui::Context` | PASS |
| `js/nkeditor-sdk.test.js` | `streams Konduktiva QuantizedMap IOI rhythm rows, mask maps, ladder rungs, and note triggers over UDP OSC` | Verifies `KonduktivaBridge` extracts native IOIs `[1.5, 0.5, 1.25, 0.75]`, shifts rungs on `CC 16`, masks muted snare on `CC 49`, and streams `/nkeditor/konduktiva/lane/0/rhythm` and `/trigger` over UDP OSC | PASS |
| Existing Suites (Epochs 1–14) | 54 Rust tests + 11 Node.js tests | Theme, Devices, Hardware, MIDI I/O, Mapping Engine, LED Controller, UI Editor, OSC Server, 8opPerc Model, 8opPerc Cockpit, Layer Manager, Librarian, Snapshot Pool | PASS (65/65) |

# QA

- **Total Tests**: `69` passing (`57` Rust tests + `12` Node.js tests), `0` failing, `0` compiler warnings.
- **Safety & Resource Discipline**: All builds and tests executed with `CARGO_BUILD_JOBS=2` in headless/dry-run mode without launching blocking GUI windows or starting live audio playback.
- **Verdict**: **PASS**.
