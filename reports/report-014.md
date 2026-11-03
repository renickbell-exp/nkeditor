---
title: "Epoch 014 — 8opPerc Timbre Snapshot Pool & Beat-Synchronized Scene Morphing Manager"
date: "2026-11-04"
project: "nkEditor4 (Pure Rust egui MIDI Controller Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 14
follows: "report-013"
subject: "Implementation of src/snapshot_pool.rs and js/snapshot-pool-manager.js with JSONL preset pools, named scene JSON files, rehearsal star ratings, selective voice-lane locks, and beat-synchronized scene morphing"
operator: "Antigravity (Google DeepMind)"
tldr: "Implemented the 8opPerc Timbre Snapshot & Scene Morphing Manager in src/snapshot_pool.rs and js/snapshot-pool-manager.js: (1) durable 8-voice snapshot management in JSONL preset pools (nanokontrol2-random-preset-pool.jsonl and legacy midimix-random-preset-pool.jsonl) plus standalone named timbre-state JSON files; (2) rehearsal snapshot browsing, 0-5 star ratings, favorites, and selective per-lane voice locks (e.g. locking kick and snare while recalling hat/clap/tom timbres from another snapshot); and (3) smooth beat-synchronized scene morphing interpolating continuous per-lane density, macro/gain, global density, and rhythmic feel between Scene A and Scene B over a configurable number of musical beats. Verified with 54 passing Rust tests and 11 passing Node.js tests (65 total)."
qa_verdict: "PASS"
tests_total: 65
---

# Prompt

Epoch 14 of 20: Implement the 8opPerc Timbre Snapshot & Scene Morphing Manager in `src/snapshot_pool.rs` and `js/`: manage durable 8-voice snapshots in JSONL preset pools (`nanokontrol2-random-preset-pool.jsonl` and legacy `midimix-random-preset-pool.jsonl`) plus named timbre-state JSON files. Allow the performer in the `egui` cockpit (or via `nanoKONTROL2` controls and OSC) to browse saved 8-voice snapshots, rate/favorite snapshots during rehearsal, lock specific voice lanes (e.g. keep kick and snare while loading hat/clap/tom timbres from another snapshot), and smoothly morph continuous density/macro parameters between two scenes over a configurable number of beats. Add automated Rust and Node.js tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-014.html` from `reports/report-014.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-Language Overview: What Changed and Why It Matters

During a live algorithmic percussion performance or rehearsal with `8opPerc`, the performer frequently discovers a great-sounding combination of 8 drum sounds (FM timbres) and rhythmic densities and captures it on the fly by pressing `Marker Set (CC 60)` on the Korg `nanoKONTROL2` (or `Send All` on an Akai `MIDI Mix`). Over several rehearsals, hundreds of these snapshots accumulate in a line-by-line log file (`nanokontrol2-random-preset-pool.jsonl` or the earlier `midimix-random-preset-pool.jsonl`).

However, simply saving snapshots to a file is only half the workflow. In live performance, the musician needs to:

1. **Browse, Rate, and Star Snapshots (`SnapshotPoolManager`)**: Quickly filter captured snapshots by star rating (`1★` to `5★`), favorites (`★`), or search keywords (`gqom`, `kick-laser`, `minimal`), and save favorite setups as standalone named `.json` scene files.
2. **Mix-and-Match Voices with Per-Lane Locking (`lane_locks`)**: Often a performer loves their current `kick` and `snare` groove and wants to keep those two lanes anchored while loading the `hat_closed`, `hat_open`, `clap`, `tom`, `rim`, and `cymbal` sounds from a completely different snapshot.
3. **Smoothly Morph Between Two Scenes Over Musical Beats (`SceneMorphEngine`)**: Instead of an abrupt jump from a sparse intro scene (`Scene A`) to a high-energy climax scene (`Scene B`), the performer can select a musical duration (such as `2`, `4`, `8`, `16`, or `32` beats) and let the engine smoothly interpolate every unlocked lane's rhythmic density (`0.1x` to `5.0x`), macro/gain slider (`0..127`), global density, and swing/humanize feel scale across those beats.

In Epoch 14, we implemented this complete system in both Rust (`src/snapshot_pool.rs`) and Node.js (`js/snapshot-pool-manager.js`).

## 1. Architecture & Plain-Language Mapping Table

The table below connects each plain-English capability to its exact Rust (`src/snapshot_pool.rs`) and Node.js (`js/snapshot-pool-manager.js`) implementation components:

| Capability | Plain-English Explanation | Rust Component (`src/snapshot_pool.rs`) | Node.js Component (`js/snapshot-pool-manager.js`) |
|---|---|---|---|
| **JSONL Preset Pool Compatibility** | Reads and writes both `nanokontrol2-random-preset-pool.jsonl` (`kick..cymbal` keys) and legacy `midimix-random-preset-pool.jsonl` (`kicks, snares, hats, toms, bells, metals, noise, fx` keys) | `SnapshotSceneRecord::from_json_value`, `import_jsonl_str`, `export_jsonl_str`, `append_snapshot_to_jsonl_file` | `normalizeSnapshotRecord`, `formatSnapshotJsonlLine`, `importJsonlString`, `loadJsonlFile`, `appendSnapshotToJsonlFile` |
| **Named Timbre-State JSON Files** | Saves and loads standalone, human-readable `.json` scene files containing all 8 timbres, densities, gains, mutes, solos, global feel, ratings, and tags | `save_named_scene_json_file`, `load_named_scene_json_file`, `to_pretty_json` | `saveNamedSceneJsonFile`, `loadNamedSceneJsonFile` |
| **Rehearsal Ratings & Favorites** | Assigns `0..=5` star ratings and favorite flags to snapshots and filters the browser by minimum rating, favorite status, or text search | `set_rating`, `toggle_favorite`, `filtered_snapshots` | `setRating`, `toggleFavorite`, `getSnapshot` |
| **Selective Voice-Lane Locking** | Locks any subset of the 8 percussion lanes (e.g., locking `kick` and `snare`) so recalling a snapshot or morphing scenes only updates unlocked lanes | `SnapshotPoolManager::lane_locks`, `set_lane_lock`, `recall_snapshot_into_model` | `SnapshotPoolManager.laneLocks`, `setLaneLock`, `recallSnapshotIntoModel` |
| **Beat-Synchronized Scene Morpher** | Interpolates continuous densities, macro gains, global density, and feel scale from Scene A to Scene B over configurable beats (`2..32` beats) or manual crossfader | `SceneMorphEngine`, `MorphedSceneFrame`, `start_morph`, `advance_beats`, `apply_current_morph_to_model` | `startMorph`, `advanceMorphBeats`, `evaluateMorphFrame` |
| **Interactive `egui` Cockpit Panel** | Renders the 8-voice lock bar, filterable snapshot list with star rating buttons, and Scene A/B beat morphing deck | `render_snapshot_pool_panel` | Synced via UDP OSC `/nkeditor/op8/snapshot/save` & `/nkeditor/op8/*` |

## 2. Legacy `midimix-random-preset-pool.jsonl` & `nanokontrol2-random-preset-pool.jsonl` Normalization

Snapshots captured in earlier `8operc-konduktiva` sessions (`midimix-random-preset-pool.jsonl`) store their `timbres` map using `KONDUKTIVA_FAMILY_ORDER` keys (`kicks`, `snares`, `hats`, `toms`, `bells`, `metals`, `noise`, `fx`) or live player labels (`percussion`, `alt-toms`), whereas `nanokontrol2-random-preset-pool.jsonl` uses canonical `FAMILY_ORDER` keys (`kick`, `snare`, `hat_closed`, `hat_open`, `clap`, `tom`, `rim`, `cymbal`) or 8-element arrays.

Both `parse_timbres_field` in `src/snapshot_pool.rs` and `normalizeSnapshotRecord` in `js/snapshot-pool-manager.js` route every key through `resolve_lane_index` (`0..8`), guaranteeing that snapshots from either controller pool load into the exact same 8 voice lanes without data loss and hash to the exact same deterministic FNV-1a `pool_bank_index` (`0..4096`).

# Files Touched

- `src/snapshot_pool.rs` (Created — 784 lines): Implemented `SnapshotSceneRecord`, `SceneMorphEngine`, `MorphedSceneFrame`, `SnapshotPoolManager`, `render_snapshot_pool_panel`, and 4 unit and headless `egui` tests.
- `js/snapshot-pool-manager.js` (Created — 385 lines): Implemented `normalizeSnapshotRecord`, `formatSnapshotJsonlLine`, `timbreSetSignature`, and `SnapshotPoolManager` with JSONL/JSON file I/O, selective lane locking, and beat-synchronized scene morphing.
- `src/lib.rs` (Modified): Exported `pub mod snapshot_pool;`.
- `js/index.js` (Modified): Re-exported `./snapshot-pool-manager.js`.
- `js/nkeditor-sdk.test.js` (Modified): Added automated end-to-end Node.js test verifying `nanokontrol2-random-preset-pool.jsonl`, legacy `midimix-random-preset-pool.jsonl`, named scene `.json` persistence, rehearsal ratings/favorites, selective `kick` + `snare` lane locking, and 8-beat scene morphing.
- `reports/report-014.md`, `reports/report-014.html`, `reports/report-014.pdf` (Created): Epoch 014 report in Markdown, grayscale HTML, and PDF formats.

# Tests

Executed both the Rust test suite (`CARGO_BUILD_JOBS=2 cargo test`) and the Node.js test suite (`npm test`):

| Test Suite | Test Name | What It Verifies | Status |
|---|---|---|---|
| `src/snapshot_pool.rs` | `jsonl_preset_pools_nanokontrol2_and_legacy_midimix_roundtrip_and_named_json` | Imports both `nanokontrol2-random-preset-pool.jsonl` and legacy `midimix-random-preset-pool.jsonl` lines, verifies `bells -> lane 4`, `metals -> lane 5`, `noise -> lane 6` normalization, and round-trips JSONL and named `.json` scene files | PASS |
| `src/snapshot_pool.rs` | `selective_voice_lane_locking_preserves_kick_and_snare_during_snapshot_recall` | Locks `kick` (lane 0) and `snare` (lane 1), recalls `scene-gqom-overdrive`, and verifies lanes 0 and 1 keep their anchored timbres and densities while lanes `2..=7` update to Gqom timbres and densities | PASS |
| `src/snapshot_pool.rs` | `scene_morph_engine_interpolates_densities_gains_and_feel_over_configurable_beats` | Runs an 8-beat morph between `scene-boot-default` and `scene-gqom-overdrive` with `kick` locked; verifies exact 50% midpoint interpolation at beat `4.0` and 100% completion at beat `8.0` | PASS |
| `src/snapshot_pool.rs` | `headless_egui_snapshot_pool_panel_renders_without_panic` | Renders `render_snapshot_pool_panel` inside a headless `egui::Context` and verifies all lock toggles, star rating buttons, and morph controls render cleanly | PASS |
| `js/nkeditor-sdk.test.js` | `manages JSONL preset pools (nanokontrol2 + legacy midimix), named scene JSON files, voice locks, and beat-synchronized scene morphing` | Verifies Node.js `SnapshotPoolManager` disk JSONL/JSON persistence, star rating/favorite toggles, `kick`+`snare` lock preservation on recall, and 8-beat continuous scene morphing | PASS |
| Existing Suites (Epochs 1–13) | 50 Rust tests + 10 Node.js tests | Theme, Devices, Hardware, MIDI I/O, Mapping Engine, LED Controller, UI Editor, OSC Server, 8opPerc Model, 8opPerc Cockpit, Layer Manager, Librarian | PASS (60/60) |

# QA

- **Total Tests**: `65` passing (`54` Rust tests + `11` Node.js tests), `0` failing, `0` compiler warnings.
- **Safety & Resource Discipline**: All builds and tests executed with `CARGO_BUILD_JOBS=2` in headless/dry-run mode without opening blocking GUI windows or starting live audio playback.
- **Verdict**: **PASS**.
