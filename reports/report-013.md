---
title: "Epoch 013 — Preset Librarian, 14 Factory Presets, Legacy/JSON Parser & A/B Diff/Merge Engine"
date: "2026-11-04"
project: "nkEditor4 (Pure Rust egui MIDI Controller Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 13
follows: "report-012"
subject: "Implementation of src/config_parser.rs and src/librarian.rs with 14 factory presets, JSON and mappings/default.txt import/export, 5 Quick Access slots, and A/B Preset Diff & Selective Merge"
operator: "Antigravity (Google DeepMind)"
tldr: "Implemented the complete Preset Librarian in src/librarian.rs and legacy/JSON configuration parser in src/config_parser.rs: (1) all 10 original nkEditor3 factory presets plus 4 new nkEditor4/8opPerc factory presets (14 total); (2) lossless import and export for both JSON preset files and legacy mappings/default.txt text configurations (including [device akai-midimix] and note:N syntax); (3) 5 Quick Access preset slots, search, group/tag/device filtering, and favorites; and (4) an interactive A/B Preset Diff & Selective Merge engine and egui view. Verified with 50 passing Rust tests and 10 passing Node.js tests (60 total)."
qa_verdict: "PASS"
tests_total: 60
---

# Prompt

Epoch 13 of 20: Implement the complete Preset Librarian in `src/librarian.rs` and `src/config_parser.rs`: (1) include all 10 original factory presets from `nkEditor3` (`Default`, `Synth Lead`, `Drums`, `DAW Transport`, `DJ Mixer`, `Nord Drum 2` for `nanoKONTROL2`; `Default MidiMix`, `MidiMix Synth`, `MidiMix DJ`, `MidiMix Nord Drum 2` for `Akai MIDImix`) PLUS 4 new factory presets (`8opPerc nanoKONTROL2 Live Rig`, `8opPerc MIDImix Compatibility`, `Konduktiva 8-Lane Macro Surface`, `Dual-Controller Split Rig`); (2) support importing and exporting both JSON preset files and legacy `mappings/default.txt` format; (3) provide 5 Quick Access preset slots, search, tag/group filtering, and favorites; and (4) implement an A/B Preset Diff & Selective Merge view in `egui` comparing any two presets across mappings, channels, ranges, and control values. Add automated tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-013.html` from `reports/report-013.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-Language Overview: What Changed and Why It Matters

When performing live or working in a studio with multiple synthesizers, drum machines, and software environments, a musician needs to switch the entire layout of their MIDI controller in a single click—without losing track of custom tweaks or breaking old mapping files saved from previous versions of the software. Furthermore, when someone creates two variations of a setup (for example, a standard mapping and a specialized `Nord Drum 2` setup), they often want to see *exactly* which knobs or buttons changed between the two presets and copy only a few specific controls from one preset into another.

In Epoch 13, we built `src/config_parser.rs` and `src/librarian.rs` to solve four practical problems:

- **14 Built-in Factory Presets (`create_all_factory_presets`)**: We preserved all **10 original factory presets** from `nkEditor3` (6 for the Korg `nanoKONTROL2` and 4 for the Akai `MIDI Mix`) so existing workflows behave identically, and added **4 new factory presets** tailored for `nkEditor4`, `8opPerc`, and `Konduktiva` live performance (`8opPerc nanoKONTROL2 Live Rig`, `8opPerc MIDImix Compatibility`, `Konduktiva 8-Lane Macro Surface`, and `Dual-Controller Split Rig`).
- **Lossless Legacy Text (`mappings/default.txt`) & JSON Import/Export (`src/config_parser.rs`)**: Users can load and save full `Preset` JSON files (which store metadata, tags, continuous target values, button states, and secondary controller mappings) as well as human-readable `mappings/default.txt` text files (supporting `[device akai-midimix]`, `[track1]..[track8]`, `[transport]`, `[master]`, and `note:N` MIDI Note syntax).
- **5 Quick Access Slots, Instant Search, Group/Tag Filters & Favorites (`PresetLibrarian`)**: The Librarian header provides 5 one-click Quick Access preset buttons (`quick_access_slots`), a real-time search box matching preset names, descriptions, groups, and tags, category filters, and favorite star toggles (`toggle_favorite`).
- **A/B Preset Diff & Selective Merge View (`diff_presets`, `merge_selected_controls`, `render_preset_librarian_panel`)**: Users can pick any two presets (`Preset A` and `Preset B`) to inspect a side-by-side table of every control that differs across MIDI CC/Note numbers, channels, min/max ranges, response curves, button behaviors, labels, and saved control values—and selectively merge checked controls into a new hybrid preset.

## 1. Complete Catalog of All 14 Factory Presets

The table below maps all 14 built-in factory presets in `src/librarian.rs` to their target hardware controller, group, and mapping architecture:

| # | Preset Name | Preset ID | Target Device | Group | Key Mapping Details & Rust Factory Function |
|---|---|---|---|---|---|
| 1 | `Default` | `factory-default` | `nanokontrol2` | `Factory` | 51-control 1:1 CC passthrough on Ch 1 (`factory_preset_default_nk2`) |
| 2 | `Synth Lead` | `factory-synth-lead` | `nanokontrol2` | `Factory` | Knobs -> Filter/Res/Env/FX CCs `74, 71, 73, 72, 91, 93, 1, 11`; Sliders -> OSC/Master CCs `16..22, 7` (`factory_preset_synth_lead_nk2`) |
| 3 | `Drums` | `factory-drums` | `nanokontrol2` | `Factory` | Knobs -> Drum Tune/Decay CCs `10, 42..48`; Sliders -> Drum Levels; Solo -> Momentary `Trigger` (`factory_preset_drums_nk2`) |
| 4 | `DAW Transport` | `factory-daw-transport` | `nanokontrol2` | `Factory` | Mackie-compatible transport CCs (`Play=94, Stop=93, Rec=95, Rew=91, Fwd=92, Loop=86`) + 8 mixer strips (`factory_preset_daw_transport_nk2`) |
| 5 | `DJ Mixer` | `factory-dj-mixer` | `nanokontrol2` | `Factory` | Deck A (Tracks 1-4) & Deck B (Tracks 5-8) 3-band EQ + Filter CCs `22..29`, X-Fade, Cue, Play, Sync (`factory_preset_dj_mixer_nk2`) |
| 6 | `Nord Drum 2` | `factory-nord-drum-2-nk2` | `nanokontrol2` | `Nord Drum 2` | Tracks 1-6 -> Pads 1-6 (Ch 1-6, Knob=`SPECTRA` CC 30, Slider=`DECAY` CC 50); Tracks 7-8 -> Global FX on Ch 10 (`factory_preset_nord_drum_2_nk2`) |
| 7 | `Default MidiMix` | `factory-default-midimix` | `akai-midimix` | `MidiMix Factory` | 60-control 1:1 identity mapping with CC knobs/faders and Note buttons (`factory_preset_default_midimix`) |
| 8 | `MidiMix Synth` | `factory-midimix-synth` | `akai-midimix` | `MidiMix Factory` | Row 1 Knobs -> Synth CCs `74, 71, 73, 72, 91, 93, 1, 11`; Faders -> OSC/Env/FX/Master levels (`factory_preset_midimix_synth`) |
| 9 | `MidiMix DJ` | `factory-midimix-dj` | `akai-midimix` | `MidiMix Factory` | Row 1 Knobs -> EQ High CCs `22..29`; Faders -> Ch 1-8 Vol; Mute -> `Cue`; Solo -> `PFL` (`factory_preset_midimix_dj`) |
| 10 | `MidiMix Nord Drum 2` | `factory-midimix-nord-drum-2` | `akai-midimix` | `Nord Drum 2` | 32 continuous controls: Row1=`SPECTRA`, Row2=`FILTER_FREQ`, Row3=`TIMBRE`, Fader=`LEVEL` across Pads 1-6 (Ch 1-6) + Global FX (Ch 10) (`factory_preset_midimix_nord_drum_2`) |
| 11 | `8opPerc nanoKONTROL2 Live Rig` | `factory-8opperc-nk2-live` | `nanokontrol2` | `8opPerc` | Native 8-lane percussion rig (`kick..shaker` Density, Gain/Macro, Solo, Mute, Reroll, Form Prev/Next, Timbre Set, Snapshot) (`factory_preset_op8_nanokontrol2_live_rig`) |
| 12 | `8opPerc MIDImix Compatibility` | `factory-8opperc-midimix-compat` | `nanokontrol2` | `8opPerc` | Cross-message `CC -> Note` and `CC -> CC` emulation translating `nanoKONTROL2` into Akai `MIDI Mix` hardware events (`factory_preset_op8_midimix_compatibility`) |
| 13 | `Konduktiva 8-Lane Macro Surface` | `factory-konduktiva-macro-surface` | `nanokontrol2` | `8opPerc` | Ch 2 expressive surface with `SCurve` macro knobs (`CC 102..109`, range `8..120`) and `Logarithmic` stem faders (`CC 110..117`) (`factory_preset_konduktiva_macro_surface`) |
| 14 | `Dual-Controller Split Rig` | `factory-dual-controller-split-rig` | `nanokontrol2` | `8opPerc` | Dual-controller preset pairing `nanoKONTROL2` (`8opPerc` Live Rig on Ch 1) + secondary `akai-midimix` (`Nord Drum 2` 32-param synth control on Ch 1-6 & Ch 10) (`factory_preset_dual_controller_split_rig`) |

## 2. Legacy Text (`mappings/default.txt`) & JSON Parser (`src/config_parser.rs`)

The table below connects the human-readable configuration features to their implementation in `src/config_parser.rs`:

| Feature | Plain-Language Role | Rust Symbol in `src/config_parser.rs` |
|---|---|---|
| Legacy `.txt` Parser | Parses `[device <id>]`, `[track1]..[track8]`, `[transport]`, `[master]`, `note:N` syntax, `ch1..ch16`, `toggle`/`momentary`, and trailing `"label"` strings with line-numbered errors | `parse_legacy_mapping_text` & `ConfigParseError` |
| Legacy `.txt` Formatter | Formats any `DeviceMappingConfig` back into clean, human-readable `mappings/default.txt` syntax for export | `format_mapping_to_text` |
| JSON Preset Import/Export | Serializes and deserializes full `Preset` structs (including metadata, tags, continuous targets, button states, and `secondary_mapping`) | `parse_preset_json` & `format_preset_json` |
| Format Auto-Detection | Automatically detects whether an imported file is JSON (`{...}`) or legacy text (`[track1]...`) | `import_preset_auto` |

## 3. A/B Preset Diff & Selective Merge Engine (`src/librarian.rs`)

- **Field-Level Comparison (`diff_presets`, `PresetDiffReport`, `ControlDiffEntry`, `FieldDiff`)**: Compares any two presets (`preset_a` vs `preset_b`) across every control path (`track1.knob`, `transport.play`, etc.) and records differences in `mapping` (`CC`/`Note` type and number), `channel` (`Ch 1..16`), `range` (`min_value..max_value`), `curve` (`Linear`, `Exponential`, `Logarithmic`, `SCurve`), `invert`, `behavior` (`Toggle` vs `Momentary`), `on_off` values, `label`, and saved `target_value` / `button_state`.
- **Selective Per-Control Merge (`merge_selected_controls`, `PresetLibrarian::execute_selective_merge`)**: Allows the user to check or uncheck individual control paths in `selected_merge_paths` and merge only those selected controls from Preset B onto Preset A, producing a conflict-free user preset.
- **Interactive `egui` Panel (`render_preset_librarian_panel`)**: Renders the 5 Quick Access slot buttons, search/favorite/group filter bar, scrollable preset browser with `★` favorite toggle, `Set A` / `Set B` diff selectors, `Dup` / `Del` actions, and the interactive A/B Diff & Selective Merge checklist.

# Files Touched

- `src/config_parser.rs` (Created — 456 lines): Implemented `ConfigParseError`, `parse_legacy_mapping_text`, `format_mapping_to_text`, `parse_preset_json`, `format_preset_json`, and `import_preset_auto`.
- `src/librarian.rs` (Created — 1,073 lines): Implemented `PresetMetadata`, `Preset`, all 14 factory preset builders (`create_all_factory_presets`), `FieldDiff`, `ControlDiffEntry`, `PresetDiffReport`, `diff_presets`, `merge_selected_controls`, `PresetLibrarian`, `render_preset_librarian_panel`, and 5 comprehensive unit and headless `egui` tests.
- `src/lib.rs` (Modified): Exported `pub mod config_parser;` and `pub mod librarian;`.
- `reports/report-013.md`, `reports/report-013.html`, `reports/report-013.pdf` (Created): Epoch 013 report in Markdown, grayscale HTML, and PDF formats.

# Tests

Executed both the Rust test suite (`CARGO_BUILD_JOBS=2 cargo test`) and the Node.js test suite (`npm test`):

| Test Suite | Test Name | What It Verifies | Status |
|---|---|---|---|
| `src/librarian.rs` | `all_14_factory_presets_are_present_and_valid` | Verifies all 10 original `nkEditor3` presets and 4 new `nkEditor4`/`8opPerc` presets exist, are marked `is_factory`, and `Dual-Controller Split Rig` carries both primary `nanokontrol2` and secondary `akai-midimix` mappings | PASS |
| `src/librarian.rs` | `parses_actual_mappings_default_txt_and_roundtrips_legacy_text_and_json` | Parses the repository's actual `mappings/default.txt` file (51 controls, verifying custom CCs like `track1.knob 16 -> 74` and `track5.solo 36 -> 100`), round-trips both `nanoKONTROL2` and `Akai MIDImix` (`[device akai-midimix]` + `note:N`) through legacy text and JSON losslessly | PASS |
| `src/librarian.rs` | `librarian_quick_slots_search_tags_favorites_and_duplicate_names_work` | Verifies 5 Quick Access slots, search filtering, group filtering, favorite star toggling, conflict-free duplicate naming (`Synth Lead copy`, `Synth Lead copy 2`), and factory deletion protection | PASS |
| `src/librarian.rs` | `ab_preset_diff_and_selective_merge_across_mappings_channels_ranges_and_values` | Verifies `diff_presets` between `Default` and `Nord Drum 2` detects CC and channel diffs, and `execute_selective_merge` merges only checked controls (`track1.knob`, `track7.knob`) while leaving unchecked controls intact | PASS |
| `src/librarian.rs` | `headless_egui_librarian_panel_renders_without_panic` | Verifies `render_preset_librarian_panel` renders Quick Access slots, search/group filters, preset list, and A/B Diff checklist in a headless `egui::Context` without panicking | PASS |
| Rust Suite (Epochs 1–12) | 45 existing unit & integration tests | Theme, Devices, Hardware, MIDI I/O, Mapping Engine, LED Controller, UI Editor, OSC Server, 8opPerc Model, 8opPerc Cockpit, Layer Manager | PASS (45/45) |
| Node.js SDK (`npm test`) | 10 companion JS SDK & live rig tests | OSC codec, `NkEditorClient`, `8opPerc` mapper, and `--dry` live rig rehearsal | PASS (10/10) |

# QA

- **Total Tests**: `60` passing (`50` Rust tests + `10` Node.js tests), `0` failing, `0` compiler warnings.
- **Safety & Resource Discipline**: All builds and tests executed with `CARGO_BUILD_JOBS=2` in headless mode without launching blocking GUI windows or live audio playback.
- **Verdict**: **PASS**.
