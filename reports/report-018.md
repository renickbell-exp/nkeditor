---
title: "Epoch 018 — Multi-Device Matrix Routing, Per-Port CC Rate-Limiting & Emergency Hardware/Software PANIC"
date: "2026-11-04"
project: "nkEditor4 (Pure Rust egui MIDI Controller Remapper & 8opPerc Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 18
follows: "report-017"
subject: "Implementation of src/multi_device_router.rs with composite multi-controller performance rig linking, dedicated vs. merged virtual output port routing, per-port MIDI CC rate-limiting and latest-value coalescing, and immediate emergency PANIC (GUI button, Ctrl+Shift+P shortcut, and /nkeditor/panic OSC endpoint)"
operator: "Antigravity (Google DeepMind)"
tldr: "Implemented advanced Multi-Device Matrix Routing and Hardware Safety in src/multi_device_router.rs and src/osc_server.rs: (1) composite performance rig linking where Controller A (Korg nanoKONTROL2) drives 8opPerc Voices 1-8 Density & Mute/Solo while Controller B (second nanoKONTROL2 or Akai MIDImix) simultaneously drives Layer Page 2 Timbre Morph or Nord Drum 2 6-pad synthesis (Ch 1-6 + Ch 10 FX), or merges multiple controllers into a single virtual output port with channel offset; (2) per-port MIDI CC rate-limiting and latest-value coalescing (PortRateLimiter) to prevent USB/DIN buffer overflows during rapid sweeps or scene morphs; and (3) an immediate emergency PANIC command (GUI button, Ctrl+Shift+P, and /nkeditor/panic OSC endpoint) sending Sustain Off (CC 64), All Sound Off (CC 120), Reset All Controllers (CC 121), and All Notes Off (CC 123), muting all 8 8opPerc lanes, and forcing a full hardware LED resync. Verified with 68 passing Rust tests and 12 passing Node.js tests (80 total)."
qa_verdict: "PASS"
tests_total: 80
---

# Prompt

Epoch 18 of 20: Implement advanced Multi-Device Matrix Routing and Hardware Safety in `src/multi_device_router.rs`: allow linking multiple simultaneous MIDI controllers into composite performance rigs (e.g. Controller A = Korg `nanoKONTROL2` driving `8opPerc` Voices 1-8 Density & Mute/Solo while Controller B = second `nanoKONTROL2` or Akai `MIDImix` drives Page 2 Timbre Morph or Nord Drum 2 pads, or merging two controllers into one virtual output port). Add per-port MIDI CC rate-limiting and coalescing to prevent USB/DIN buffer overflows during rapid sweeps or scene morphs, and implement an immediate emergency PANIC command (GUI button, keyboard shortcut, and `/nkeditor/panic` OSC endpoint) that sends All Notes Off (`CC 123`), resets sustain (`CC 64`), mutes all `8opPerc` lanes, and resyncs hardware LEDs. Add automated tests (with `CARGO_BUILD_JOBS=2`), git commit all changes, and compile `reports/report-018.html` from `reports/report-018.md` via `/home/renick/claude/html-report-generator/build-report.sh --pdf` and commit.

# Content

## Plain-Language Overview: What Changed and Why It Matters

When performing live electronic percussion, a single compact 8-track MIDI controller often isn't enough: you want one physical controller dedicated to your primary drum groove (`Voices 1–8 Density & Mute/Solo`) and a second controller sitting beside it dedicated to deep sound-design (`Page 2: Timbre Morph & Decay` or Clavia `Nord Drum 2` 6-pad synthesis)—without having to press page-switch buttons mid-performance. At the same time, moving 8 faders at once or running a continuous 8-voice scene morph can flood hardware USB or 5-pin DIN MIDI ports (`31.25 kbps`, or roughly `1 millisecond` per 3-byte MIDI message), causing sluggish latency or stuck notes. And if a synth ever hangs or a rhythm runs away on stage, the performer needs a single **Emergency PANIC** button that silences every hardware and software voice in one click.

In Epoch 18, we implemented the **Multi-Device Matrix Router, Per-Port CC Rate-Limiter, and Emergency Hardware/Software PANIC Engine** in `src/multi_device_router.rs` and `src/osc_server.rs`:

1. **Composite Multi-Controller Rig Linking (`MultiDeviceMatrixRouter` & `CompositeRigPreset`)**: Allows linking multiple simultaneous physical controllers into a single composite performance rig. For example, **Controller A** (`Korg nanoKONTROL2 #1`) can drive `8opPerc` Voices 1–8 Density and Mute/Solo while **Controller B** (`nanoKONTROL2 #2` or `Akai MIDImix`) is pinned directly to `ControllerPage::TimbreMorphAndDecay` (`Page 2`) or `ControllerRole::NordDrum2SynthSurface` (`Channels 1..=6` + Global FX `Channel 10`)—without disturbing Controller A's active layer page. Controllers can either emit to dedicated output ports (`MergeMode::DedicatedPerController`) or merge into a single shared virtual output port (`MergeMode::SingleVirtualOutput`) with per-controller MIDI channel offsets (`output_channel_offset`) so identical CC numbers never collide.
2. **Per-Port MIDI CC Rate-Limiting & Latest-Value Coalescing (`PortRateLimiter`)**: Protects USB and 5-pin DIN MIDI buffers from overflowing during rapid fader sweeps or scene morphs. If multiple continuous CC updates arrive for the same `(output_port, channel, cc_number)` within the configurable microsecond window (`min_cc_interval_us`, default `2,000 us` = `2.0 ms`), intermediate CC values are absorbed (`total_coalesced_updates`) so only the newest value is held and transmitted when `flush_due_packets(now_us)` runs. Meanwhile, rhythmic `Note` events and emergency `PANIC` messages use `PacketPriority::ImmediatePriority` to bypass rate-limiting with zero queuing delay.
3. **Immediate Emergency PANIC (`execute_emergency_panic` & `/nkeditor/panic`)**: Triggerable via the high-contrast GUI **EMERGENCY PANIC** button, the `Ctrl+Shift+P` keyboard shortcut, or the `/nkeditor/panic` UDP OSC endpoint (`OscCommand::Panic`). In a single atomic operation, it purges all pending coalesced CC queues, transmits **Sustain Pedal Off (`CC 64 = 0`)**, **All Sound Off (`CC 120 = 0`)**, **Reset All Controllers (`CC 121 = 0`)**, and **All Notes Off (`CC 123 = 0`)** across all active ports and channels, mutes all 8 `8opPerc` voice lanes (`muted = true`, `soloed = false`, `playing = false`), and forces a full hardware LED panel resynchronization across every connected controller (`28` CC LEDs on `nanoKONTROL2`, `24` Note LEDs on `MIDImix`).

## 1. Plain-Language to Technical Component Mapping

| Feature Area | Plain-English Role | Rust Module & Symbols |
|---|---|---|
| **Composite Rig Roles** | Assigns each physical controller to `8opPerc` primary voices, a pinned Layer Page (`Page 1..4`), `Nord Drum 2` 6-pad synthesis, `MIDImix` emulation, or standard remapping | `src/multi_device_router.rs`: `ControllerRole`, `ControllerLinkEntry`, `CompositeRigPreset` |
| **Dedicated vs. Merged Output Routing** | Routes each controller to its own virtual port or merges multiple controllers into one shared port with channel offset | `src/multi_device_router.rs`: `MergeMode`, `MultiDeviceMatrixRouter::resolve_output_port`, `route_incoming_midi` |
| **Per-Port CC Rate-Limiter & Coalescer** | Absorbs intermediate rapid CC sweeps within `min_cc_interval_us` (`2,000 us` default) while passing Notes and Panic CCs immediately | `src/multi_device_router.rs`: `PortRateLimiter`, `PortCcKey`, `PacketPriority`, `submit_packet`, `flush_due_packets` |
| **Emergency Hardware/Software PANIC** | Purges queues, sends `CC 64=0`, `CC 120=0`, `CC 121=0`, `CC 123=0`, mutes all 8 `8opPerc` lanes, stops transport, and resyncs all hardware LEDs | `src/multi_device_router.rs`: `execute_emergency_panic`, `PanicReport`; `src/osc_server.rs`: `OscCommand::Panic` (`/nkeditor/panic`) |
| **Interactive `egui` Router & Safety Panel** | Renders the composite preset bar, controller role matrix table, rate-limiter telemetry, and emergency PANIC button | `src/multi_device_router.rs`: `render_multi_device_router_panel` |

## 2. Built-In Composite Performance Rig Templates (`CompositeRigPreset`)

| Preset Variant | Controller A Role | Controller B Role | Output Port Topology |
|---|---|---|---|
| `DualNanoKontrol2SplitRig` | `Op8PrimaryVoices` (`Voices 1–8 Density & Mute/Solo`) | `LayerPagePinned(TimbreMorphAndDecay)` (`Page 2`) | `DedicatedPerController` (`Out A` & `Out B`) |
| `NanoKontrol2PlusMidiMixHybrid` | `Op8PrimaryVoices` (`Korg nanoKONTROL2`) | `LayerPagePinned(TimbreMorphAndDecay)` (`Akai MIDImix`) | `DedicatedPerController` (`Out A` & `Out B`) |
| `NanoKontrol2PlusNordDrum2Rig` | `Op8PrimaryVoices` (`Korg nanoKONTROL2`) | `NordDrum2SynthSurface` (`Ch 1..=6 Pads + Ch 10 FX`) | `DedicatedPerController` (`Out A` & `Out B`) |
| `MergedVirtualOutputRig` | `StandardRemapper` (`Ch 1`, offset `+0`) | `StandardRemapper` (`Ch 2`, offset `+1`) | `SingleVirtualOutput` (`nkEditor4 Composite Virtual Out`) |

# Files Touched

- `src/multi_device_router.rs` (Created — 780 lines): Implemented `ControllerRole`, `MergeMode`, `ControllerLinkEntry`, `CompositeRigPreset`, `PacketPriority`, `RoutedOutputPacket`, `PortCcKey`, `PortRateLimiter`, `PanicReport`, `MatrixRouteOutcome`, `MultiDeviceMatrixRouter`, `render_multi_device_router_panel`, and 4 unit and headless `egui` tests.
- `src/osc_server.rs` (Modified): Added `OscCommand::Panic` (`/nkeditor/panic`) to `parse_osc_message` and `apply_command`.
- `src/lib.rs` (Modified): Exported `pub mod multi_device_router;`.
- `reports/report-018.md`, `reports/report-018.html`, `reports/report-018.pdf` (Created): Epoch 018 report in Markdown, grayscale HTML, and PDF formats.

# Tests

Executed both the Rust test suite (`CARGO_BUILD_JOBS=2 cargo test`) and the Node.js test suite (`npm test`):

| Test Suite | Test Name | What It Verifies | Status |
|---|---|---|---|
| `src/multi_device_router.rs` | `composite_rig_links_controller_a_to_op8_voices_and_controller_b_to_page2_or_nord_drum_2_or_merged_port` | Verifies Controller A driving `8opPerc` lane density while Controller B simultaneously drives pinned `Page 2: Timbre Morph` (`CC 24`) without disturbing Controller A's active page, plus `MergedVirtualOutputRig` merging both controllers onto one port with Channel `1` and Channel `2` (`+1` offset) | PASS |
| `src/multi_device_router.rs` | `per_port_cc_rate_limiter_coalesces_rapid_sweeps_and_flushes_latest_value` | Verifies that 4 rapid CC updates within a `2,000 us` window emit the first value (`10`), coalesce 3 intermediate updates (`30, 75, 118`), and flush only the newest value (`118`) at `t = 3,000 us` | PASS |
| `src/multi_device_router.rs` | `emergency_panic_sends_cc123_and_cc64_mutes_all_op8_lanes_resyncs_leds_and_parses_osc` | Verifies `/nkeditor/panic` OSC parsing, purging of pending coalesced CCs, emission of `CC 64 = 0` and `CC 123 = 0`, muting of all 8 `8opPerc` lanes, and full 28-LED (`nanoKONTROL2`) + 24-LED (`MIDImix`) hardware resync | PASS |
| `src/multi_device_router.rs` | `headless_egui_renders_multi_device_router_panel_without_panic` | Renders `render_multi_device_router_panel` inside a headless `egui::Context` without panic | PASS |
| Existing Suites (Epochs 1–17) | 64 Rust tests + 12 Node.js tests | All previous modules and SDK suites | PASS (76/76) |

# QA

- **Total Tests**: `80` passing (`68` Rust tests + `12` Node.js tests), `0` failing, `0` compiler warnings.
- **Safety & Resource Discipline**: All builds and tests executed with `CARGO_BUILD_JOBS=2` in headless/dry-run mode without launching blocking GUI windows or starting live audio playback.
- **Verdict**: **PASS**.
