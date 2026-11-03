# nkEditor4 — Pure Rust `egui` MIDI Controller Remapper & `8opPerc` Performance Cockpit

**nkEditor4** is the pure Rust (`egui` / `eframe`) desktop MIDI controller remapper, multi-device matrix router, and **8opPerc / Konduktiva live performance cockpit** (migrated from the legacy Electron `nkEditor3` application, which is archived under `legacy-electron/`).

It provides first-class hardware support for the **Korg nanoKONTROL2** (`0944:0117`) and **Akai Professional MIDI Mix** (`09e8:0031`), Clavia **Nord Drum 2** 6-channel synthesis & global FX mapping, USB-port-independent hardware identity persistence, bidirectional UDP **Open Sound Control (OSC)** integration, and a zero-dependency companion **Node.js & Konduktiva JavaScript SDK** (`js/`).

---

## Key Features (Epochs 1–20)

1. **15 `yaae-theme` Palettes & 4 `twoBowls` Background Presets (`src/theme.rs`)**:
   - Supports all 15 `yaae` themes (`Dark`, `Light`, `Monokai`, `Solarized Dark`, `Solarized Light`, `Nord`, `Dracula`, `Gruvbox`, `One Dark`, `Tokyo Night`, `Catppuccin`, `Synthwave`, `High Contrast`, `Matrix`, `Cyberpunk`) and 4 `twoBowls` background presets (`White`, `Cream`, `Black`, `Dark Grey`), plus custom hex overrides and `50%..200%` UI zoom scaling.
2. **USB-Port-Independent Hardware Discovery & Hot-Plug Persistence (`src/hardware.rs`, `src/midi_io.rs`)**:
   - Inspects `/sys/class/sound/card*`, `/dev/snd/by-id/`, `/dev/snd/by-path/`, and `/proc/asound/seq/clients` (`aconnect -l` / `amidi -l` fallbacks) to compute a stable `HardwareFingerprint` (`vendor_id:product_id:serial:card_name`, e.g., `0944:0117::nanoKONTROL2`).
   - Automatically recognizes controllers across USB port changes and ALSA client ID migrations (`24:0 -> 28:0 -> 32:0 -> 36:0`), restoring presets and resynchronizing all hardware LEDs (`28` CC LEDs on `nanoKONTROL2`, `24` Note LEDs on `MIDImix`).
3. **Multi-Device Mapping Engine & Soft Takeover (`src/mapping_engine.rs`, `src/led_controller.rs`)**:
   - Full `CC <-> Note` conversion, channel remapping (`1..=16`), min/max scaling, polarity inversion, response curves (`Linear`, `Exponential`, `Logarithmic`, `SCurve`), `Catch`/`Jump`/`Pickup` Soft Takeover, and dirty-flag LED diff coalescing.
4. **8opPerc Performance Cockpit & Form/Timbre Navigation Deck (`src/op8_model.rs`, `src/op8_cockpit.rs`)**:
   - 8-voice percussion matrix (`kick`, `snare`, `hat_closed`, `hat_open`, `clap`, `tom`, `rim`, `cymbal`) with per-voice and global density ladders (`0.1x..5.0x`), exact midpoint feel scaling (`CC 64 = 0.5`), Solo/Mute priority gating, per-lane timbre locks, searchable 4,096-entry Form Bank browser, bar/beat phase timeline, and interactive XY Feel/Density pad.
5. **4-Page Layer Engine, 14-Preset Librarian & Scene Morphing (`src/layer_manager.rs`, `src/librarian.rs`, `src/snapshot_pool.rs`)**:
   - 4 switchable controller pages (`P1: Density & Level`, `P2: Timbre Morph & Decay`, `P3: Pitch/Spectra & Filter`, `P4: Global Feel & Master FX`) with automatic Soft Takeover on page switch.
   - 14 built-in factory presets (10 original `nkEditor3` presets + 4 new `8opPerc`/Konduktiva presets), `mappings/default.txt` and JSON round-trip, 5 Quick Access slots, A/B Preset Diff & Selective Merge, and beat-synchronized Scene Morphing across JSONL preset pools (`nanokontrol2-random-preset-pool.jsonl`).
6. **Konduktiva Rhythm Visualizer, Gaussian Randomizer, Undo/Redo & Command Palette (`src/konduktiva_view.rs`, `src/randomizer.rs`, `src/history.rs`)**:
   - Live 8-lane `QuantizedMap` IOI rhythm row & note trigger visualizer, Box-Muller Gaussian drift randomizer (`1%..100%`) with per-control and per-lane lock shields, 128-step `Ctrl+Z` / `Ctrl+Y` Undo/Redo stack, and `Ctrl+K` / `/` `twoBowls` Command & Control Search Palette.
7. **Multi-Device Matrix Router, Protocol Inspector & Emergency PANIC (`src/multi_device_router.rs`, `src/diagnostics.rs`)**:
   - Links multiple controllers into composite rigs (e.g., Controller A driving `8opPerc` Voices 1–8 while Controller B drives Page 2 Timbre Morph or Nord Drum 2 pads, or merges onto one virtual port), per-port CC rate-limiting/coalescing (`PortRateLimiter`), Akai `MIDImix` 32-CC `Send All` burst detection (`80ms` gate), microsecond latency telemetry, and immediate Emergency **PANIC** (`CC 123`, `CC 64`, mute all lanes, full LED resync).

---

## CLI Usage & Verification Flags

```bash
# Build and run the desktop egui application (limit build jobs with CARGO_BUILD_JOBS=2)
CARGO_BUILD_JOBS=2 cargo run -- --theme Cyberpunk --bg cream --preset op8-nanokontrol2-live-rig

# List connected USB/ALSA MIDI hardware with canonical HardwareFingerprints
CARGO_BUILD_JOBS=2 cargo run -- --list-devices

# Run the Non-Audible Hardware Verification, 4-Port USB Migration & 1,000-Event Stress Suite
CARGO_BUILD_JOBS=2 cargo run -- --verify-hardware --dry-stress

# Run in non-blocking headless OSC + MIDI service mode on custom UDP ports
CARGO_BUILD_JOBS=2 cargo run -- --headless-osc --osc-port 9000 --osc-target 127.0.0.1:9001

# Run the companion Node.js SDK & Konduktiva stress harness
node js/stress-test-nanokontrol2.js --verify-hardware --dry-stress

# Run the full Rust (73 tests) and Node.js (13 tests) automated test suites (86 total)
CARGO_BUILD_JOBS=2 cargo test && npm test
```

---

## Bidirectional UDP OSC Address Space (Default Listen `9000` / Broadcast `9001`)

| OSC Address | Arguments | Description |
| :--- | :--- | :--- |
| `/nkeditor/status` | *(none)* | Queries active device, theme, BPM, and form status JSON |
| `/nkeditor/device/list` | *(none)* | Broadcasts JSON array of managed hardware sessions and fingerprints |
| `/nkeditor/device/select` | `<device_id>` | Selects the active hardware controller by fingerprint or definition ID |
| `/nkeditor/preset/load` | `<preset_id_or_slot>` | Loads a preset into the active controller pipeline and resyncs LEDs |
| `/nkeditor/preset/save` | `<preset_name>` | Saves the current mapping state under `<preset_name>` |
| `/nkeditor/control/set` | `<path> <value>` | Sets a software target value (`0..127`) and updates button LEDs |
| `/nkeditor/led/set` | `<button_id> <0\|1>` | Directly sets a hardware button LED (`track1.solo`, `transport.play`, etc.) |
| `/nkeditor/theme/set` | `<theme> [bg_preset]` | Switches the active `yaae-theme` and optional `twoBowls` background |
| `/nkeditor/layer/select` | `<1..4_or_page_id>` | Switches the active controller layer page and engages Soft Takeover |
| `/nkeditor/op8/state` | `[json_snapshot]` | Queries or applies the full 8-lane `Op8RigState` JSON snapshot |
| `/nkeditor/op8/form` | `<next\|prev\|name>` | Navigates the 4,096-entry `8opPerc` Form Bank |
| `/nkeditor/op8/timbre` | `<lane\|all> [name]` | Rerolls a single percussion lane (`0..7`) or advances the full 8-voice set |
| `/nkeditor/op8/lane/<0..7>/density` | `<0.1..5.0>` | Sets per-lane rhythmic density multiplier |
| `/nkeditor/op8/lane/<0..7>/mute` | `<0\|1>` | Sets per-lane Mute state and updates hardware Mute LED |
| `/nkeditor/op8/lane/<0..7>/solo` | `<0\|1>` | Sets per-lane Solo state and updates hardware Solo LED |
| `/nkeditor/op8/snapshot/save` | `[label]` | Atomically captures the 8-voice timbre state into the JSONL preset pool |
| `/nkeditor/panic` | *(none)* | Emergency PANIC: sends `CC 64=0`, `CC 120=0`, `CC 121=0`, `CC 123=0`, mutes all 8 `8opPerc` lanes, stops transport, and resyncs all hardware LEDs |
