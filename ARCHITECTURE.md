# nkEditor4 — System Architecture (`ARCHITECTURE.md`)

## 1. High-Level System Topology

`nkEditor4` replaces the legacy Electron/Chromium architecture (`legacy-electron/`) with a zero-overhead pure Rust `egui = "0.29"` / `eframe = "0.29"` application (`src/`) coupled over non-blocking UDP Open Sound Control (OSC) with a zero-dependency Node.js & Konduktiva live performance SDK (`js/`).

```
+-----------------------------------------------------------------------------------+
|                        Physical USB / ALSA MIDI Hardware                          |
|  - Korg nanoKONTROL2 (0944:0117, /dev/snd/by-id/usb-KORG_INC._nanoKONTROL2-00)    |
|  - Akai Professional MIDI Mix (09e8:0031)                                         |
|  - Clavia Nord Drum 2 (Ch 1..6 Pads + Ch 10 Global FX via USB/DIN Interface)      |
+-----------------------------------------+-----------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
|                      nkEditor4 Rust Core & egui Desktop App                       |
|                                                                                   |
|  [hardware.rs & midi_io.rs]  --> HardwareFingerprint (vendor:product:serial:card) |
|         |                        Automatic 4-Port USB Hot-Plug & LED Restoration  |
|         v                                                                         |
|  [multi_device_router.rs]    --> Composite Rig Matrix (Role A: Op8 | Role B: P2)  |
|         |                        Per-Port CC Rate-Limiter & Emergency PANIC       |
|         v                                                                         |
|  [mapping_engine.rs]         --> CC<->Note, Curves, Range, Soft Takeover Ring     |
|  [layer_manager.rs]          --> 4 Switchable Pages (Density/Morph/Pitch/Global)  |
|  [led_controller.rs]         --> 28-CC (nanoKONTROL2) & 24-Note (MIDImix) LEDs    |
|  [op8_model.rs & cockpit.rs] --> 8-Voice Percussion Matrix, Form Bank & XY Pad    |
|  [librarian.rs & snapshot]   --> 14 Factory Presets, A/B Diff & Scene Morphing    |
|  [randomizer & history.rs]   --> Box-Muller Gaussian Drift, Undo/Redo & Palette   |
|  [diagnostics.rs]            --> Live MIDI/OSC Inspector, 80ms Burst & Latency    |
+-----------------------------------------+-----------------------------------------+
                                          |
                        Bidirectional UDP OSC (9000 <-> 9001)
                                          |
                                          v
+-----------------------------------------------------------------------------------+
|                Companion Node.js & Konduktiva Live Performance SDK                |
|  - js/osc-codec.js & js/nkeditor-sdk.js (4-byte aligned OSC + Promise/Event API)  |
|  - js/op8-nanokontrol2-mapper.js (Native nanoKONTROL2 & MIDImix Emulation)        |
|  - js/pulsed-nanokontrol2-rig.js (32nd-note quantized density, Solo/Mute, Pool)   |
|  - js/konduktiva-bridge.js (QuantizedMap IOI rhythm rows, ladder rungs, triggers) |
|  - js/snapshot-pool-manager.js (JSONL preset pools & beat-synced scene morphing)  |
+-----------------------------------------------------------------------------------+
```

---

## 2. Rust Module Inventory (`src/`)

| Module | Primary Responsibility | Key Types & Entry Points |
| :--- | :--- | :--- |
| `src/theme.rs` | 15 `yaae-theme` palettes, 4 `twoBowls` background presets (`White`, `Cream`, `Black`, `Dark Grey`), custom hex overrides, and `50%..200%` UI zoom | `ThemePreference`, `TwoBowlsBackgroundPreset`, `ThemeSettings` |
| `src/devices.rs` | Exact hardware specifications for Korg `nanoKONTROL2` (28 CC LEDs), Akai `MIDI Mix` (24 Note LEDs), and Clavia `Nord Drum 2` synthesis CCs | `DeviceDefinition`, `nanokontrol2_definition`, `akai_midimix_definition`, `nord_drum_2` |
| `src/hardware.rs` | Linux `/sys/class/sound`, `/dev/snd/by-{id,path}`, and `/proc/asound/seq/clients` discovery (`aconnect`/`amidi` fallbacks) | `HardwareFingerprint`, `DiscoveredMidiDevice`, `discover_connected_midi_hardware` |
| `src/midi_io.rs` | Simultaneous multi-device session manager with USB-port-independent hot-plug reconciliation and persistent JSON profile storage | `MultiDeviceManager`, `ManagedDeviceSession`, `HotplugEvent`, `RawMidiPacket` |
| `src/mapping_engine.rs` | Multi-device MIDI remapping pipeline (`CC <-> Note`), channel routing, response curves, polarity inversion, and Soft Takeover (`Catch`, `Jump`, `Pickup`) | `DeviceMappingPipeline`, `MultiDeviceMappingEngine`, `ControlMappingEntry` |
| `src/led_controller.rs` | Protocol-aware hardware LED controller supporting `Internal`, `External`, and `Op8Cockpit` modes with dirty-flag diff coalescing | `DeviceLedController`, `MultiDeviceLedManager`, `Op8CockpitLedSnapshot` |
| `src/ui_editor.rs` | Interactive `egui` vector controller surface (`nanoKONTROL2` & `MIDImix`), Soft Takeover dual-arc ring indicators, and Mapping Inspector | `ControllerEditorUiState`, `render_controller_workspace` |
| `src/osc_server.rs` | Non-blocking UDP OSC server (`9000`) and telemetry broadcaster (`9001`) built on `rosc` | `OscServer`, `OscCommand`, `Op8RigState`, `parse_osc_message` |
| `src/op8_model.rs` | `8opPerc` 8-voice percussion domain model, exponential density ladder (`0.1x..5.0x`), exact `CC 64 = 0.5` feel curve, and `MIDImix` emulation translator | `Op8PerformanceModel`, `Op8VoiceLane`, `Op8PerformanceAction` |
| `src/op8_cockpit.rs` | Interactive 8-column `8opPerc` Performance Cockpit, searchable 4,096-entry Form Bank browser, bar/beat phase timeline, and Global Density/Feel XY pad | `Op8CockpitViewState`, `render_op8_cockpit` |
| `src/layer_manager.rs` | 4-page parameter layer engine (`Density/Level`, `Morph/Decay`, `Pitch/Filter`, `Global Feel/FX`) with modifier combos and automatic Soft Takeover on switch | `ControllerPage`, `MultiPageLayerManager`, `render_layer_page_selector_bar` |
| `src/config_parser.rs` & `src/librarian.rs` | 14 factory presets, legacy `mappings/default.txt` & JSON parser/exporter, 5 Quick Access slots, and A/B Preset Diff & Selective Merge | `PresetLibrarian`, `PresetDiffReport`, `render_preset_librarian_panel` |
| `src/snapshot_pool.rs` | Durable 8-voice timbre snapshot manager (`nanokontrol2-random-preset-pool.jsonl` & `midimix-random-preset-pool.jsonl`), lane locks, and beat-synced Scene Morphing | `SnapshotPoolManager`, `SceneMorphEngine`, `render_snapshot_pool_panel` |
| `src/konduktiva_view.rs` | Real-time 8-lane `Konduktiva` `QuantizedMap` IOI rhythm row, mask map, ladder rung, and note trigger visualizer | `KonduktivaTelemetryState`, `render_konduktiva_rhythm_visualizer` |
| `src/randomizer.rs` & `src/history.rs` | Box-Muller Gaussian drift randomizer (`1%..100%`) with lock shields, 128-step `Ctrl+Z`/`Ctrl+Y` Undo/Redo stack, and `Ctrl+K` `twoBowls` Command Palette | `RandomizerEngine`, `CommandHistoryStack`, `CommandPalette` |
| `src/diagnostics.rs` | Real-time MIDI In/Out, LED, and OSC protocol inspector, Akai `MIDImix` 32-CC `Send All` burst detector (`80ms` gate), and microsecond latency tracker | `DiagnosticsInspector`, `BurstGestureDetector`, `LatencyMetrics` |
| `src/multi_device_router.rs` | Composite multi-controller rig matrix router, dedicated vs. merged virtual output port routing, per-port CC rate-limiter/coalescer, and Emergency PANIC | `MultiDeviceMatrixRouter`, `PortRateLimiter`, `execute_emergency_panic` |
| `src/verification_harness.rs` | Non-audible Korg `nanoKONTROL2` hardware verifier, 4-port USB migration simulator, 1,000-event dry stress rehearsal, and 60-theme/view `egui` audit | `run_full_verification_and_stress_suite`, `CliLaunchConfig` |

---

## 3. USB-Port-Independent Hardware Identity Persistence

When a USB MIDI controller is unplugged and re-plugged into a different USB port on Linux, its dynamic `/sys/class/sound/cardN` number, `/dev/snd/by-path/` topology symlink, and ALSA Sequencer client ID (`24:0 -> 28:0 -> 32:0 -> 36:0`) change.

`nkEditor4` solves this by computing a canonical `HardwareFingerprint` (`vendor_id:product_id:serial:card_name`, e.g., `0944:0117::nanoKONTROL2`) from the USB device parent directory (`device/../idVendor` and `device/../idProduct`, stopping before walking up into the PCI root hub). When `MultiDeviceManager::reconcile_discovered_devices` detects a matching fingerprint on a new USB port or ALSA client ID, it emits `HotplugEvent::ReconnectedRemembered`, automatically restores the controller's assigned preset and role, and flushes a full-panel LED resynchronization burst (`28` CC packets on `nanoKONTROL2`, `24` Note packets on `MIDImix`).
