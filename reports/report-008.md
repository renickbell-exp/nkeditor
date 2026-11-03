---
title: "nkEditor4 — Epoch 008: 8opPerc Performance Domain Model & Dual Controller Mapper"
date: 2026-11-04
project: "nkEditor4 (Rust egui MIDI/OSC Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 8
follows: "Report 007"
subject: "8opPerc Percussion Rig Domain Model in Rust & Node.js, Native nanoKONTROL2 Mapping, and MIDImix Emulation"
operator: "Antigravity Agent"
tldr: "Implemented the 8opPerc performance domain model in Rust (src/op8_model.rs) and Node.js (js/op8-nanokontrol2-mapper.js), modeling all 8 percussion voice lanes (FAMILY_ORDER), exponential density ladders (0.1 to 5.0), exact midpoint feel scaling (CC 64 = 0.5), Solo/Mute priority audibility, 4,096-entry form and timbre banks, Native nanoKONTROL2 Performance Mapping, and MIDImix Emulation Mode. All 38 Rust tests and 9 Node.js tests pass (47 total)."
qa_verdict: "PASS"
tests_total: 47
---

# Prompt

Implement the 8opPerc performance domain model in Rust (`src/op8_model.rs`) and Node.js (`js/op8-nanokontrol2-mapper.js`), referencing `docs/NKEDITOR-RUST-PORT-SPEC.md` and `/home/renick/claude/8operc-konduktiva/src/pulsed-live-rig.js`. Model all 8 percussion voice lanes (`FAMILY_ORDER`), per-lane density multipliers (`0.1` to `5.0`), per-lane macro/gain sliders, per-lane Mute and new Solo states, per-lane active and queued timbre names, global density multiplier (`0.1` to `5.0`), global rhythmic feel scale (`swing` and `humanize` `0.0` to `1.0` with exact midpoint `CC 64 = 0.5`), 4,096-entry form bank state, and 4,096-entry timbre bank state. Implement both (1) Native nanoKONTROL2 Performance Mapping (`Knobs CC 16-23` = voice density, `Sliders CC 0-7` = voice gain/macro, `Solo CC 32-39` = voice solo, `Mute CC 48-55` = voice mute, `Rec CC 64-71` = lane timbre reroll, `Track L/R CC 58/59` = form prev/next, `Cycle CC 46` = full timbre set, `Marker Set CC 60` = atomic preset pool capture, `Marker L/R CC 61/62` = global density step, `Rewind/Forward CC 43/44` = global feel step, `Play/Stop/Rec CC 41/42/45` = transport) and (2) MIDImix Emulation Mode translating nanoKONTROL2 into factory MIDImix Notes/CCs. Add Rust and Node.js tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-008.html` from `reports/report-008.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-English Overview: What Changed and Why It Matters

In live electronic percussion performance with `8opPerc` (an 8-operator FM percussion synthesizer) and `Konduktiva` (a musical scheduling environment), a performer controls eight simultaneous drum voices—such as kicks, snares, hats, toms, and cymbals—while navigating thousands of pregenerated musical forms and sound palettes (timbres). Previously, the live rig (`pulsed-live-rig.js`) was hardwired only for the Akai `MIDI Mix` controller, which lacked dedicated transport buttons and relied on an awkward 32-message "Send All" burst to save a preset snapshot.

In Epoch 8, we built the complete `8opPerc` performance brain in both Rust (`src/op8_model.rs`) and Node.js (`js/op8-nanokontrol2-mapper.js`):

- **Eight Percussion Voice Lanes with Solo Priority**: Each of the 8 drum lanes tracks its own note density multiplier (`0.1x` sparse to `5.0x` dense), volume/macro slider (`0..127`), Mute button, a brand-new Solo button (which automatically silences non-soloed lanes while active), and both its currently playing sound (`active_timbre`) and its next-bar preloaded sound (`queued_timbre`).
- **Exact Musical Scaling Curves**: Turning a physical MIDI knob from `0` to `127` scales note density exponentially from `0.1x` to `5.0x` (`0.1 * 50^(cc / 127)`). For rhythmic feel (`swing` and `humanize`), standard 7-bit MIDI has an odd number of values (`0` to `127`), meaning `64 / 127` is slightly off-center (`0.5039...`). Our feel curve uses piecewise linear interpolation so the physical center detent at `CC 64` produces **exact** `0.5` (`50%` of the authored swing and humanize).
- **4,096-Form & 4,096-Timbre Reservoirs**: Tracks the active musical form across a 4,096-entry bank (with history and a 4-form lookahead prefetch window) and 4,096 full 8-voice timbre sets (with automatic refill at `1,024` remaining sets and FNV-1a hash placement for saved snapshots).
- **Dual Operating Modes (Native nanoKONTROL2 + MIDImix Emulation)**: Supports both a direct, purpose-built Korg `nanoKONTROL2` performance layout (where `Marker Set CC 60` captures an atomic preset snapshot in a single button press) and a backwards-compatible `MIDImix` Emulation mode that translates `nanoKONTROL2` actions into factory Akai `MIDI Mix` Notes and CCs—including synthesizing the full 32-CC `Send All` burst when `Marker Set CC 60` is pressed.

| Plain-English Concept | Rust Symbol (`src/op8_model.rs`) | Node.js Symbol (`js/op8-nanokontrol2-mapper.js`) | Technical Behavior |
| :--- | :--- | :--- | :--- |
| 8 Percussion Voice Lanes | `FAMILY_ORDER` / `Op8VoiceLane` | `FAMILY_ORDER` / `model.lanes` | Models `kick`, `snare`, `hat_closed`, `hat_open`, `clap`, `tom`, `rim`, `cymbal` (plus Konduktiva aliases) |
| Exponential Density Ladder | `cc_to_density` / `density_to_cc` | `mapMidiCCToDensity` / `mapDensityToMidiCC` | Maps `CC 0..127` to `0.1 * 50^(cc / 127)` (`0.1x` to `5.0x`) |
| Exact Midpoint Feel Curve | `cc_to_feel_scale` / `cc_to_midimix_feel_scale` | `mapMidiCCToFeelScale` / `mapMidiCCToMidimixFeelScale` | Guarantees `CC 64 == 0.5` exact midpoint for `swing` and `humanize` |
| Solo & Mute Audibility | `Op8PerformanceModel::is_lane_audible` | `Op8PerformanceModel.prototype.isLaneAudible` | Honors Mute state and gives Solo priority over non-soloed lanes |
| 4,096-Entry Form Bank | `Op8FormBankState` | `model.formBank` | Circular `0..4096` navigation with history and 4-form prefetch window |
| 4,096-Entry Timbre Bank | `Op8TimbreBankState` / `Op8PresetSnapshot` | `model.timbreBank` / `timbrePoolIndex` | Refills at `1024` and hashes saved snapshots via 32-bit FNV-1a |
| Native nanoKONTROL2 Mapper | `Op8PerformanceModel::apply_nanokontrol2_cc` | `Op8PerformanceModel.prototype.applyNanoKontrol2Cc` | Maps all 8 track strips and 11 transport buttons to `Op8PerformanceAction` |
| MIDImix Emulation Translator | `translate_nanokontrol2_to_midimix` | `translateNanoKontrol2ToMidimix` | Emits factory MIDImix Notes (`1..27`), CCs (`17..61`), and 32-CC `Send All` burst |

## Native nanoKONTROL2 vs. MIDImix Emulation Mapping Reference

| Physical nanoKONTROL2 Control | Native nanoKONTROL2 CC | 8opPerc Native Action | Emulated MIDImix Message (Channel 1) |
| :--- | :--- | :--- | :--- |
| Track Knobs 1..8 | `CC 16..23` | `SetLaneDensity` (`0.1 .. 5.0`) | Row 2 Voice Density `CC 17, 21, 25, 29, 47, 51, 55, 59` |
| Track Sliders 1..8 | `CC 0..7` | `SetLaneGain` (`0..127` / `0.0..1.0`) | Strip Faders `CC 19, 23, 27, 31, 49, 53, 57, 61` |
| Track Solo Buttons 1..8 | `CC 32..39` | `ToggleLaneSolo` (with Solo priority mask) | Solo Buttons `Note 2, 5, 8, 11, 14, 17, 20, 23` |
| Track Mute Buttons 1..8 | `CC 48..55` | `ToggleLaneMute` (with Audible mask) | Mute Buttons `Note 1, 4, 7, 10, 13, 16, 19, 22` |
| Track Rec Buttons 1..8 | `CC 64..71` | `RerollLaneTimbre` (queues next-bar patch) | Rec Arm Buttons `Note 3, 6, 9, 12, 15, 18, 21, 24` |
| Track Left / Right | `CC 58 / CC 59` | `NavigateForm` (`Previous` / `Next` in 4,096 bank) | Bank Left `Note 25` / Bank Right `Note 26` |
| Cycle Button | `CC 46` | `AdvanceTimbreSet` (next 8-voice timbre set) | Solo Master `Note 27` |
| Marker Set Button | `CC 60` | `CapturePresetPoolSnapshot` (single-button atomic save) | Full 32-CC `Send All` burst (`CC 16..31` and `CC 46..61`) |
| Marker Left / Right | `CC 61 / CC 62` | `StepGlobalDensity` (`-8` / `+8` CC step) | Global Density Fader `CC 61` |
| Rewind / Forward | `CC 43 / CC 44` | `StepGlobalFeel` (`-8` / `+8` CC step around `CC 64`) | Global Feel Knob `CC 58` |
| Play / Stop / Record | `CC 41 / CC 42 / CC 45` | `TransportPlay`, `TransportStop`, `TransportRecord` | Native transport control |

# Files Touched

- `src/op8_model.rs` (Created — 792 lines: `Op8VoiceLane`, `Op8RhythmicFeel`, `Op8FormBankState`, `Op8TimbreBankState`, `Op8PresetSnapshot`, `Op8PerformanceModel`, `Op8PerformanceAction`, `EmulatedMidiMessage`, `cc_to_density`, `cc_to_feel_scale`, and 5 unit tests)
- `src/lib.rs` (Modified — exported `pub mod op8_model;`)
- `js/op8-nanokontrol2-mapper.js` (Created — 514 lines: `Op8PerformanceModel`, `mapMidiCCToDensity`, `mapMidiCCToFeelScale`, `mapMidiCCToMidimixFeelScale`, `timbrePoolIndex`, `applyNanoKontrol2Cc`, and `translateNanoKontrol2ToMidimix`)
- `js/index.js` (Modified — re-exported `./op8-nanokontrol2-mapper.js`)
- `js/nkeditor-sdk.test.js` (Modified — added Epoch 8 test suite for `Op8PerformanceModel` and scaling functions)

# Tests

Executed `CARGO_BUILD_JOBS=2 cargo test` and `npm test`:

| Test Suite | Test Name | Result |
| :--- | :--- | :--- |
| Rust (`src/op8_model.rs`) | `op8_model::tests::test_density_ladder_endpoints_and_roundtrip` | PASS |
| Rust (`src/op8_model.rs`) | `op8_model::tests::test_feel_scale_exact_midpoint_at_cc_64` | PASS |
| Rust (`src/op8_model.rs`) | `op8_model::tests::test_voice_lanes_mute_and_solo_priority` | PASS |
| Rust (`src/op8_model.rs`) | `op8_model::tests::test_native_nanokontrol2_performance_mapping_all_controls` | PASS |
| Rust (`src/op8_model.rs`) | `op8_model::tests::test_midimix_emulation_translation` | PASS |
| Rust (All Modules) | Full Rust Unit & Integration Test Suite (`38` tests) | PASS (`38/38`) |
| Node.js (`js/nkeditor-sdk.test.js`) | `models 8opPerc density ladder, exact CC 64 = 0.5 feel curve, and Native nanoKONTROL2 / MIDImix Emulation mapping` | PASS |
| Node.js (`js/nkeditor-sdk.test.js`) | Full Companion SDK Test Suite (`9` tests) | PASS (`9/9`) |

# QA

- **Compilation & Test Verification**: `CARGO_BUILD_JOBS=2 cargo test` passed 38/38 tests with zero warnings or errors; `npm test` passed 9/9 Node.js tests (`47` total automated tests).
- **Audio & GUI Safety Policy**: Verified purely via deterministic in-memory state transitions; no GUI window or live `8opPerc` audio synthesis process was launched.
