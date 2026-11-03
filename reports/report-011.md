---
title: "nkEditor4 — Epoch 011: Form Bank & Performance Navigation Deck in Rust egui"
date: 2026-11-04
project: "nkEditor4 (Rust egui MIDI/OSC Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 11
follows: "Report 010"
subject: "Searchable 4,096-Form Bank Browser, Section & Bar Timeline Progress Bar, and Interactive Global Density/Feel XY Control Pad"
operator: "Antigravity Agent"
tldr: "Extended the Rust egui 8opPerc Performance Cockpit (src/op8_cockpit.rs) with a 3-column Form Bank & Performance Navigation Deck: (1) a searchable Form Bank Browser displaying the active form (gqom_drive), 4 hydrated neighbor forms, canonical tags/descriptors, and Previous/Next/Queue Form controls synced with nanoKONTROL2 Track Left/Right (CC 58/59); (2) a Section & Bar Timeline progress bar showing BPM, bar/beat phase, and pending next-bar timbre/form latch indicators; and (3) an interactive Global Density & Rhythmic Feel (Swing/Humanize) 2D XY control pad and readout panel. All 42 Rust tests and 10 Node.js tests pass (52 total)."
qa_verdict: "PASS"
tests_total: 52
---

# Prompt

Extend the Rust egui 8opPerc Performance Cockpit (`src/op8_cockpit.rs`) with a Form Bank & Performance Navigation Deck: (1) a searchable Form Bank Browser displaying the active form (e.g. `gqom_drive`), hydrated neighbor forms, form tags/descriptors, and one-click Previous / Next / Queue Form controls synced with nanoKONTROL2 Track Left/Right (`CC 58/59`); (2) a Section & Bar Timeline progress bar showing current BPM, bar/beat phase, and pending next-bar timbre/form latch indicators; and (3) an interactive Global Density & Rhythmic Feel (Swing / Humanize) XY control and readout panel. Add automated tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-011.html` from `reports/report-011.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-English Overview: What Changed and Why It Matters

In Epoch 10, we built the 8-column Percussion Voice Matrix for controlling the 8 individual drum voices. During a live set, however, a performer also shapes the macro-structure of the music: choosing which musical composition ("form") out of 4,096 pregenerated structures will play next, watching where the current beat sits inside the bar and 16-bar section so transitions land cleanly on the downbeat, and sweeping overall note density against rhythmic feel (`swing` and `humanize`).

In Epoch 11, we extended `src/op8_cockpit.rs` with the **Form Bank & Performance Navigation Deck**, organized into three coordinated panels directly above the 8-voice matrix:

- **1. Searchable Form Bank Browser (`render_form_bank_browser`)**: Displays the active form (defaulting to `gqom_drive`) along with its structural tags (`base_form`, `motif`, `density_curve`, `section_role` from `8operc-konduktiva`'s `pulsed-form-change-factory.js`), the 4 hydrated neighbor forms in the prefetch window (`FORM_BANK_PREFETCH = 4`), a real-time search box across all 4,096 forms, and one-click `<< PREV [CC58]`, `NEXT [CC59] >>`, and `QUEUE` buttons synced with the Korg `nanoKONTROL2` Track Left/Right buttons (`CC 58/59`) and OSC `/nkeditor/op8/form`.
- **2. Section & Bar Timeline Progress Bar (`render_section_and_bar_timeline`)**: Displays the current tempo (`BPM`), active structural section role (`ANCHOR`, `LIFT`, `PRESSURE`, `BREAK`, `RETURN`), 4-beat step cells (`BEAT 1..4`), continuous bar-phase and 16-bar section progress bars, and live status readouts for **Pending Next-Bar Timbre Latches** and **Pending Next-Bar Form Latches**. When the beat clock crosses a bar boundary (`advance_beat_phase`), queued timbres and queued forms automatically commit on the downbeat.
- **3. Interactive Global Density & Rhythmic Feel 2D XY Control Pad (`render_global_density_feel_xy_panel`)**: Renders a 2D vector XY pad (`Sense::click_and_drag()`) where dragging horizontally sweeps Global Density (`0.10x` to `5.00x`, logarithmic `CC 61/62` scale) and dragging vertically sweeps Global Feel Scale (`0.00` to `1.00`, exact midpoint `CC 64 = 0.50` crosshair, `CC 43/44`), accompanied by live tabular readouts of effective `swing` and `humanize` plus a one-click `RESET CENTER (1.00x | CC64=0.50)` button.

| Plain-English Navigation Feature | Rust Symbol (`src/op8_cockpit.rs`) | Hardware / OSC Binding | Technical Behavior |
| :--- | :--- | :--- | :--- |
| 4,096-Form Metadata Catalogue | `FormBankEntryDescriptor` / `describe_form_entry` | 10 `PULSED_FORM_ARCHETYPES` (`gqom_drive`..`singeli_drive`) | Deterministic archetype, motif, phase, section role, and density curve tags |
| Searchable Form Browser | `Op8CockpitViewState::search_forms` / `render_form_bank_browser` | `Track Left/Right` (`CC 58 / 59`) + `/nkeditor/op8/form` | Case-insensitive search across all 4,096 forms + 4 hydrated neighbor buttons |
| Next-Bar Form Queueing | `Op8CockpitViewState::queue_form_for_next_bar` | `queued_form_index` + `/nkeditor/op8/form` | Queues selected form to commit automatically on the next bar downbeat |
| Section & Bar Timeline | `Op8CockpitViewState::timeline_progress` / `advance_beat_phase` | Bar downbeat auto-commits queued forms & timbres | Computes 16-bar section role, `BEAT 1..4` step cells, and bar/section fill ratios |
| 2D Density & Feel XY Pad | `set_global_density_and_feel_xy` / `render_global_density_feel_xy_panel` | `Marker L/R` (`CC 61/62`) & `Rewind/Forward` (`CC 43/44`) | Maps X to `0.1x..5.0x` density and Y to `0.0..1.0` feel (`CC 64 = 0.5` guide) |

# Files Touched

- `src/op8_cockpit.rs` (Modified — expanded to 1,284 lines: added `PULSED_FORM_ARCHETYPES`, `FormBankEntryDescriptor`, `describe_form_entry`, `active_form_descriptor`, `hydrated_neighbor_forms`, `search_forms`, `timeline_progress`, `queue_form_for_next_bar`, `advance_beat_phase`, `set_global_density_and_feel_xy`, `render_form_and_navigation_deck`, `render_form_bank_browser`, `render_section_and_bar_timeline`, `render_global_density_feel_xy_panel`, and unit tests)

# Tests

Executed `CARGO_BUILD_JOBS=2 cargo test` and `npm test`:

| Test Suite | Test Name | Result |
| :--- | :--- | :--- |
| Rust (`src/op8_cockpit.rs`) | `op8_cockpit::tests::test_form_bank_browser_search_timeline_latch_and_xy_pad` | PASS |
| Rust (`src/op8_cockpit.rs`) | `op8_cockpit::tests::test_timbre_lock_prevents_global_set_from_overwriting_locked_lanes` | PASS |
| Rust (`src/op8_cockpit.rs`) | `op8_cockpit::tests::test_cockpit_interactions_drive_hardware_leds_and_udp_osc_telemetry` | PASS |
| Rust (`src/op8_cockpit.rs`) | `op8_cockpit::tests::test_headless_egui_renders_8_column_op8_cockpit_matrix` | PASS |
| Rust (All Modules) | Full Rust Unit & Integration Test Suite (`42` tests) | PASS (`42/42`) |
| Node.js (`npm test`) | Full Companion SDK & Live Rig Test Suite (`10` tests) | PASS (`10/10`) |

# QA

- **Form Search & Barline Auto-Commit Verification**: Verified that searching for `"footwork_drive"` filters matching entries from the 4,096-form catalogue, queueing a form and a single-lane timbre holds both pending across mid-bar beat steps (`beat 2.0`), and crossing the bar boundary (`beat 4.0`) atomically commits both the new form and the queued timbre.
- **Headless `egui` Verification**: Verified that the 3-column Form Bank & Performance Navigation Deck (`Browser`, `Timeline`, and `XY Pad`) renders cleanly in a headless `egui::Context` frame with `Rounding::ZERO` and `AccentPalette` styling.
