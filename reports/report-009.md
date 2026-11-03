---
title: "nkEditor4 — Epoch 009: Korg nanoKONTROL2 8opPerc Live Rig & Form-Batch Rehearsal"
date: 2026-11-04
project: "nkEditor4 (Rust egui MIDI/OSC Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 9
follows: "Report 008"
subject: "Korg nanoKONTROL2 Live Performance Rig (js/pulsed-nanokontrol2-rig.js) & Audible/Dry-Run Form-Batch Demo (js/audible-form-batch-nanokontrol2-live.js)"
operator: "Antigravity Agent"
tldr: "Implemented the Korg nanoKONTROL2 8opPerc live performance rig (js/pulsed-nanokontrol2-rig.js) and form-batch rehearsal demo (js/audible-form-batch-nanokontrol2-live.js), upgrading the legacy Akai MIDImix workflow with live nanoKONTROL2 sysfs/ALSA/PipeWire discovery, bidirectional OSC sync, 32nd-note quantized density latching, Solo + Mute voice gating, single-lane and full-set next-bar timbre preloading, 4,096-form navigation, single-button Marker Set (CC 60) snapshot saving into nanokontrol2-random-preset-pool.jsonl, and a deterministic --dry mode. All 38 Rust tests and 10 Node.js tests pass (48 total)."
qa_verdict: "PASS"
tests_total: 48
---

# Prompt

Create the new Korg nanoKONTROL2 version of the 8opPerc live performance demo in `js/pulsed-nanokontrol2-rig.js` and `js/audible-form-batch-nanokontrol2-live.js` (upgrading the Akai MIDImix workflow in `/home/renick/claude/8operc-konduktiva/src/pulsed-live-rig.js` and `/home/renick/claude/8operc-konduktiva/examples/audible-form-batch-midimix-live.js`). Support live Korg nanoKONTROL2 discovery (ALSA/PipeWire nanoKONTROL2 port + OSC sync with the Rust egui app), 8-lane voice mute AND solo gating, per-voice and global density ladder control latched at 32nd-note boundaries, swing/humanize feel scaling, single-lane timbre reroll via Rec buttons (`CC 64-71`), full 8-voice timbre set switching via Cycle (`CC 46`), form bank navigation via Track Left/Right (`CC 58/59`), and atomic 8-voice preset snapshot saving via Marker Set (`CC 60`) into `nanokontrol2-random-preset-pool.jsonl`. Provide a deterministic `--dry` / mock-engine mode so automated tests verify the complete nanoKONTROL2 live rig workflow without starting audible playback (never run live audio without explicit user cue). Run `cargo test` and `npm test` (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-009.html` from `reports/report-009.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-English Overview: What Changed and Why It Matters

In the original `8operc-konduktiva` repository, live performance required an Akai `MIDI Mix` controller (`src/pulsed-live-rig.js` and `examples/audible-form-batch-midimix-live.js`). While effective, the `MIDI Mix` had three practical limitations in live performance:
1. It had no dedicated Solo buttons per voice lane (its single Master Solo button was repurposed for full-set timbre switching).
2. It had no transport section (`Play`, `Stop`, `Record`, `Rewind`, `Forward`, `Cycle`, `Track Left/Right`, `Marker Set/Left/Right`).
3. Saving a favorite 8-voice sound combination required pressing `Send All`, which flooded the MIDI bus with 32 consecutive Control Change messages over an 80-millisecond window (`createMidimixSendAllGate`).

In Epoch 9, we built `js/pulsed-nanokontrol2-rig.js` and `js/audible-form-batch-nanokontrol2-live.js` to make the Korg `nanoKONTROL2` (`0944:0117`) a first-class `8opPerc` live performance instrument:

- **Automatic Hardware Discovery + OSC Sync**: `discoverNanoKontrol2Hardware` inspects Linux `/sys/class/sound/card*`, `/dev/snd/by-id/`, and `/proc/asound/seq/clients` to locate the connected Korg `nanoKONTROL2` (`card2`, ALSA `24:0`, PipeWire `Midi-Bridge:nanoKONTROL2:(capture_0) nanoKONTROL2 _ CTRL`), while simultaneously synchronizing every control gesture and LED state with the Rust `egui` desktop app over UDP OSC via `NkEditorClient`.
- **32nd-Note Quantized Density Latching**: Turning a voice knob (`CC 16..23`) or stepping global density (`CC 61/62`) never causes mid-subdivision rhythm glitches; `createQuantizedDensityState` latches requested multipliers (`0.1x` to `5.0x`) at the next exact 32nd-note boundary (`0.125` beats).
- **8-Lane Mute AND Solo Priority Gating**: Pressing `Mute` (`CC 48..55`) or `Solo` (`CC 32..39`) updates both Konduktiva player status (`playing` vs `stopped`) and the physical `nanoKONTROL2` button LEDs.
- **Single-Button Atomic Preset Pool Saving (`Marker Set CC 60`)**: Pressing `Marker Set` (`CC 60`) immediately appends the active 8-voice timbre set to `nanokontrol2-random-preset-pool.jsonl` and inserts it into the 4,096-entry timbre reservoir at its FNV-1a hash index—replacing the 32-CC `MIDImix` burst with a single atomic button press.
- **Deterministic `--dry` Mode for Safe Automated Verification**: Running `node js/audible-form-batch-nanokontrol2-live.js --dry` (the default) exercises the entire live rig pipeline—hardware discovery, 32nd-note density latching, Solo/Mute priority, bar-boundary timbre preloading, 4,096-form navigation, `nanokontrol2-random-preset-pool.jsonl` persistence, and UDP OSC telemetry—without spawning audio engines or emitting sound.

| Plain-English Concept | Legacy MIDImix (`pulsed-live-rig.js`) | Upgraded nanoKONTROL2 (`js/pulsed-nanokontrol2-rig.js`) | Benefit to Performer |
| :--- | :--- | :--- | :--- |
| Hardware Discovery | Hardcoded ALSA/PipeWire MIDImix port | `discoverNanoKontrol2Hardware` (`0944:0117` sysfs + ALSA + OSC) | Automatically detects connected `nanoKONTROL2` on `card2` (`24:0`) |
| Voice Mute & Solo | Mute only (`Notes 1..22`), no per-lane Solo | Both `Mute` (`CC 48..55`) and `Solo` (`CC 32..39`) with LED feedback | Instant single- or multi-voice soloing during breakdowns |
| 32nd-Note Density Latch | `createLiveDensityState` (`CC 17..59`, `CC 61`) | `createQuantizedDensityState` (`Knobs CC 16..23`, `Marker L/R CC 61/62`) | Glitch-free `0.1x..5.0x` density transitions on `0.125`-beat grid |
| Next-Bar Timbre Switching | `Notes 3..24` (single lane), `Note 27` (full set) | `Rec CC 64..71` (single lane), `Cycle CC 46` (full 8-voice set) | Dedicated LED-lit `Rec` and `Cycle` buttons show pending bar changes |
| Form Bank Navigation | `Bank Left/Right` (`Note 25 / 26`) | `Track Left/Right` (`CC 58 / 59`) | Direct navigation across all 4,096 forms |
| Preset Pool Capture | 32-CC `Send All` burst (`80ms` gate window) | Single `Marker Set` button (`CC 60`) -> `nanokontrol2-random-preset-pool.jsonl` | Zero-latency atomic capture with no risk of partial burst drops |
| Automated Rehearsal Mode | Requires live `8operc` + `pw-record` processes | `--dry` (`dryRun: true` default) in `runNanoKontrol2FormBatchRehearsal` | Verifies 100% of control and OSC logic without launching live audio |

# Files Touched

- `js/pulsed-nanokontrol2-rig.js` (Created — 456 lines: `createQuantizedDensityState`, `discoverNanoKontrol2Hardware`, `PulsedNanoKontrol2Rig`, `DEFAULT_PRESET_POOL_FILENAME`)
- `js/audible-form-batch-nanokontrol2-live.js` (Created — 160 lines: `runNanoKontrol2FormBatchRehearsal` and CLI `--dry` entrypoint)
- `js/index.js` (Modified — exported `./pulsed-nanokontrol2-rig.js` and `./audible-form-batch-nanokontrol2-live.js`)
- `js/nkeditor-sdk.test.js` (Modified — added end-to-end `--dry` live rig rehearsal + UDP OSC + JSONL preset pool test)

# Tests

Executed `CARGO_BUILD_JOBS=2 cargo test`, `npm test`, and `node js/audible-form-batch-nanokontrol2-live.js --dry`:

| Test Suite | Test / Command | Result |
| :--- | :--- | :--- |
| Rust (`cargo test`) | Full 38-test Rust suite (`devices`, `hardware`, `midi_io`, `mapping_engine`, `led_controller`, `ui_editor`, `osc_server`, `op8_model`, `theme`) | PASS (`38/38`) |
| Node.js (`npm test`) | `verifies complete Korg nanoKONTROL2 8opPerc live rig rehearsal in deterministic --dry mode with OSC sync and preset pool JSONL` | PASS |
| Node.js (`npm test`) | Full Companion SDK & Live Rig Test Suite (`10` tests) | PASS (`10/10`) |
| CLI Dry-Run (`node`) | `node js/audible-form-batch-nanokontrol2-live.js --dry` (`hardwareConnected=true, actions=17, savedPresets=1`) | PASS |

# QA

- **Hardware Discovery Verification**: `discoverNanoKontrol2Hardware()` detected the physical Korg `nanoKONTROL2` connected on `/sys/class/sound/card2` (`ALSA 24:0`, fingerprint `0944:0117:noserial:nanoKONTROL2`) in read-only mode.
- **Audio & GUI Safety Policy**: Verified that `--dry` is the default mode and no live audio playback or blocking GUI window was started.
