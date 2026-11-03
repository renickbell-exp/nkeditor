---
title: "Epoch 016 — Intelligent Randomization Engine, Multi-Step Undo/Redo Stack & twoBowls Command Palette"
date: "2026-11-04"
project: "nkEditor4 (Pure Rust egui MIDI Controller Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 16
follows: "report-015"
subject: "Implementation of src/randomizer.rs and src/history.rs with bounded Gaussian drift, per-control lock shields, Ctrl+Z / Ctrl+Y multi-step Undo/Redo command history, and a Ctrl+K / '/' twoBowls Command & Control Search Palette"
operator: "Antigravity (Google DeepMind)"
tldr: "Implemented (1) the Intelligent Randomization & Mutation Engine in src/randomizer.rs supporting Global and Selected-Track/Lane scopes, per-control and per-lane lock shields, bounded Box-Muller Gaussian drift (1% to 100%), and 4 musical randomization presets; (2) the full multi-step Undo/Redo command history stack in src/history.rs (Ctrl+Z / Ctrl+Y) covering control value edits, mapping changes, preset loads, randomizations, and 8opPerc cockpit mutations; and (3) a keyboard-driven Command & Control Search Palette (Ctrl+K or '/') styled after twoBowls for jumping to any device, track, CC, theme, preset, or 8opPerc action. Verified with 61 passing Rust tests and 12 passing Node.js tests (73 total)."
qa_verdict: "PASS"
tests_total: 73
---

# Prompt

Epoch 16 of 20: Implement (1) an Intelligent Randomization & Mutation Engine in `src/randomizer.rs` supporting global randomization, selected-track/lane randomization, per-control lock shields, bounded Gaussian drift (`1%` to `100%`), and musical presets; (2) a full multi-step Undo/Redo command history stack (`src/history.rs`, `Ctrl+Z` / `Ctrl+Y`) covering control value edits, mapping changes, preset loads, randomizations, and `8opPerc` cockpit mutations; and (3) a keyboard-driven Command & Control Search Palette (`Ctrl+K` or `/`) styled after `twoBowls` for jumping to any device, track, CC, theme, or `8opPerc` form/timbre action. Add automated tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-016.html` from `reports/report-016.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-Language Overview: What Changed and Why It Matters

When experimenting with live rhythms and synthesizer mappings, performers want to explore unexpected sonic variations quickly—*without* the risk of ruining a groove they already like, and without having to hunt through nested menus to find a specific MIDI CC, theme, or controller track.

In Epoch 16, we added three studio-grade productivity and creative tools in `src/randomizer.rs` and `src/history.rs`:

1. **Intelligent Randomization & Bounded Gaussian Drift (`src/randomizer.rs`)**: Instead of blind coin-flip randomization that scrambles every knob from `0` to `127` and destroys the beat, `RandomizerEngine` uses a **bounded Gaussian drift** (`1%` to `100%`) powered by a deterministic `SplitMix64` pseudo-random generator and the **Box-Muller transform** (a mathematical formula that turns flat random numbers into a bell-curve distribution centered around the knob's current position). At `5%` drift (`SubtleHumanize`), knobs nudge slightly for organic movement; at `85%` (`PolyrhythmicChaos`), parameters leap into bold new territories. Performers can also place **lock shields** (`locked_controls` and `locked_lanes`) on any individual knob, slider, or drum lane (such as `kick` and `snare`) so protected controls never move during randomization.
2. **Multi-Step Undo/Redo Command History Stack (`src/history.rs`, `Ctrl+Z` / `Ctrl+Y`)**: Every creative action—whether turning a software target value, editing a CC/Note mapping in the Inspector, loading a new preset, running a randomization batch, or stepping `8opPerc` forms and timbres—is recorded as a reversible `UndoableCommand` on a 128-step `CommandHistoryStack`. Pressing `Ctrl+Z` immediately restores the previous state, and `Ctrl+Y` (or `Ctrl+Shift+Z`) re-applies it.
3. **Keyboard-Driven `twoBowls` Command & Control Search Palette (`Ctrl+K` or `/`)**: Inspired by keyboard-centric tools and `twoBowls` aesthetics (crisp zero-rounding borders, monospace category badges, high-contrast selection highlights), pressing `Ctrl+K` or `/` opens an instant search bar indexing every device, track, CC number, `yaae-theme` palette, `twoBowls` background preset, factory preset, randomizer preset, and `8opPerc` form/timbre action.

## 1. Plain-Language to Technical Component Mapping

The table below connects each feature in Epoch 16 to its exact Rust types and methods in `src/randomizer.rs` and `src/history.rs`:

| Feature Area | Plain-English Role | Rust Module & Symbols |
|---|---|---|
| **Deterministic Bell-Curve Generator** | Generates repeatable bell-curve (`Gaussian`) nudges so `1%..100%` drift feels smooth and musical | `src/randomizer.rs`: `SplitMix64Rng`, `next_gaussian_clamped`, `sample_mutated_value` |
| **Scope & Lock Shields** | Restricts mutation to all tracks (`Global`), one track (`SelectedTrack(1..8)`), or specific lanes (`SelectedLanes`), while skipping any shielded control or locked voice lane | `src/randomizer.rs`: `RandomizationScope`, `RandomizerEngine::locked_controls`, `locked_lanes`, `is_control_shielded` |
| **Musical Randomization Presets** | One-click presets configuring drift percentage, timbre reroll, and Kick/Snare lock shields | `src/randomizer.rs`: `MusicalRandomPreset` (`SubtleHumanize`, `PercussiveGrooveShift`, `HarmonicTimbreExplore`, `PolyrhythmicChaos`) |
| **Multi-Step Undo/Redo Stack** | Reverts (`Ctrl+Z`) and re-applies (`Ctrl+Y`) control edits, mapping edits, preset loads, randomization batches, and `8opPerc` mutations | `src/history.rs`: `CommandHistoryStack`, `UndoableCommand`, `undo`, `redo` |
| **`twoBowls` Command Search Palette** | Instant keyboard modal (`Ctrl+K` or `/`) searching across devices, tracks/CCs, 15 `yaae` themes, 4 `twoBowls` backgrounds, 14 presets, and `8opPerc` actions | `src/history.rs`: `CommandPalette`, `PaletteItem`, `PaletteCategory`, `PaletteAction`, `filtered_items`, `execute_selected` |
| **Interactive `egui` Panels** | Renders the Randomizer & Lock Shield deck, Undo/Redo toolbar, and `twoBowls` Command Palette | `render_randomizer_panel` (`src/randomizer.rs`) & `render_history_and_command_palette` (`src/history.rs`) |

## 2. Musical Randomization Presets (`MusicalRandomPreset`)

| Preset Variant | UI Title | Gaussian Drift % | Timbre Reroll | Auto-Shielded Voice Lanes |
|---|---|---|---|---|
| `MusicalRandomPreset::SubtleHumanize` | `Subtle Humanize (5%)` | `5%` | `false` | `kick` (Lane 1), `snare` (Lane 2) |
| `MusicalRandomPreset::PercussiveGrooveShift` | `Percussive Groove Shift (22%)` | `22%` | `false` | `kick` (Lane 1), `snare` (Lane 2) |
| `MusicalRandomPreset::HarmonicTimbreExplore` | `Harmonic Timbre Explore (45%)` | `45%` | `true` | `kick` (Lane 1) |
| `MusicalRandomPreset::PolyrhythmicChaos` | `Polyrhythmic Chaos (85%)` | `85%` | `true` | None (all unlocked lanes mutate) |

# Files Touched

- `src/randomizer.rs` (Created — 430 lines): Implemented `SplitMix64Rng`, `RandomizationScope`, `MutationMode`, `MusicalRandomPreset`, `RandomizationResult`, `RandomizerEngine`, and `render_randomizer_panel`.
- `src/history.rs` (Created — 781 lines): Implemented `UndoableCommand`, `CommandHistoryStack`, `PaletteCategory`, `PaletteAction`, `PaletteItem`, `CommandPalette`, `render_history_and_command_palette`, and 4 unit and headless `egui` tests.
- `src/lib.rs` (Modified): Exported `pub mod history;` and `pub mod randomizer;`.
- `reports/report-016.md`, `reports/report-016.html`, `reports/report-016.pdf` (Created): Epoch 016 report in Markdown, grayscale HTML, and PDF formats.

# Tests

Executed both the Rust test suite (`CARGO_BUILD_JOBS=2 cargo test`) and the Node.js test suite (`npm test`):

| Test Suite | Test Name | What It Verifies | Status |
|---|---|---|---|
| `src/history.rs` | `randomizer_supports_global_selected_track_lock_shields_and_gaussian_drift` | Verifies per-control lock shields (`track2.slider`), per-lane lock shields (`kick` lane 0), `25%` Gaussian drift bounds, and `SelectedTrack(4)` scope isolation with `HarmonicTimbreExplore` | PASS |
| `src/history.rs` | `multi_step_undo_redo_stack_reverts_and_reapplies_values_mappings_presets_randomizations_and_op8` | Executes a 4-step sequence (`ControlValueEdit`, `MappingEntryEdit`, `RandomizationBatch`, `Op8CockpitMutation`), undoes all 4 steps in reverse order verifying exact state restoration, and redoes all 4 steps forward | PASS |
| `src/history.rs` | `command_palette_searches_and_executes_devices_controls_themes_and_op8_actions` | Searches the `CommandPalette` for `"twobowls cream"` and `"next form 4,096"`, executes both actions, and verifies theme update + automatic Undo history recording | PASS |
| `src/history.rs` | `headless_egui_renders_randomizer_history_and_command_palette_without_panic` | Renders `render_randomizer_panel` and `render_history_and_command_palette` (with the `Ctrl+K` palette open and filtering `"cc 16"`) inside a headless `egui::Context` | PASS |
| Existing Suites (Epochs 1–15) | 57 Rust tests + 12 Node.js tests | Theme, Devices, Hardware, MIDI I/O, Mapping Engine, LED Controller, UI Editor, OSC Server, 8opPerc Model, 8opPerc Cockpit, Layer Manager, Librarian, Snapshot Pool, Konduktiva View | PASS (69/69) |

# QA

- **Total Tests**: `73` passing (`61` Rust tests + `12` Node.js tests), `0` failing, `0` compiler warnings.
- **Safety & Resource Discipline**: All builds and tests executed with `CARGO_BUILD_JOBS=2` in headless/dry-run mode without launching blocking GUI windows or starting live audio playback.
- **Verdict**: **PASS**.
