# nkEditor4 (Rust `egui` Port & `8opPerc` Performance Cockpit) Reference Specification

## 1. Mission & Architecture Overview

We are porting and refactoring `nkEditor3` (originally an Electron + React + TypeScript application) into a pure Rust desktop application using `egui = "0.29"` and `eframe = "0.29"`, paired with an **External Node.js / Konduktiva JavaScript SDK** and a **bidirectional OSC (Open Sound Control) server**.

### Core Architectural Pillars
1. **Pure Rust Desktop Application (`Cargo.toml` at repository root)**:
   - Built with `egui = "0.29"`, `eframe = "0.29"`, `yaae-theme = { path = "/home/renick/claude/yaae/crates/yaae-theme", features = ["egui", "serde"] }`, `midir` (or ALSA sequencer / rawmidi bindings), `rosc` (or custom OSC 1.0/1.1 codec over UDP), `serde`, and `serde_json`.
   - Always compile and test Rust with `CARGO_BUILD_JOBS=2`.
2. **All `yaae` / `twoBowls` Visual Themes (`src/theme.rs`)**:
   - Integrates `yaae_theme::ThemePreference` (all 15 named themes: `Default`, `Light`, `Dark`, `SolarizedDark`, `Monokai`, `Dracula`, `Nord`, `GruvboxDark`, `LightGrayscale`, `DarkGrayscale`, `HighContrastBw`, `HighContrastColor`, `HighContrastLight`, `CreamLowContrast`, `MediumContrast`).
   - Supports the 4 `twoBowls` background presets (`White` `#FFFFFF`, `Cream` `#F8F2E4`, `Black` `#000000`, `Dark Grey` `#181818`), manual hex color overrides (`CustomColorOverrides`), zero-rounding crisp borders (`Rounding::ZERO`), and UI scaling (`50%` to `200%`).
3. **Multi-Device Simultaneous Routing & Hardware Identity Persistence (`src/hardware.rs`, `src/midi_io.rs`)**:
   - Simultaneously connects to and manages multiple MIDI controllers (e.g., multiple Korg `nanoKONTROL2` units, Akai `MIDI Mix`, Behringer `UMC1820`, generic MIDI controllers).
   - **Port-Independent Hardware Fingerprinting**: Reads Linux `/sys/class/sound/card*/` (`id`, `device/../idVendor`, `device/../idProduct`, `device/../serial`, `device/../manufacturer`, `device/../product`), `/dev/snd/by-id/`, and `/proc/asound/seq/clients`.
   - When a controller is unplugged and plugged back in (even into a different physical USB port where its ALSA client ID or `/dev/snd/by-path` changes), the app matches its stable hardware fingerprint (`vendor_id:product_id:serial:card_id`, e.g. `0944:0117` for Korg `nanoKONTROL2`) and automatically reconnects, restores its assigned preset/profile, and resynchronizes physical LED states.
4. **Full `8opPerc` Performance Cockpit & nanoKONTROL2 Live Rig (`src/op8_cockpit.rs` + `js/audible-form-batch-nanokontrol2-live.js`)**:
   - Replaces and upgrades the MIDImix-only workflow in `/home/renick/claude/8operc-konduktiva/src/pulsed-live-rig.js` and `/home/renick/claude/8operc-konduktiva/examples/audible-form-batch-midimix-live.js` with a first-class **Korg nanoKONTROL2** mapping and interactive `egui` performance cockpit.
   - Bidirectional OSC/JS synchronization allows the `egui` app to display and control the 8 percussion voice lanes (`kick`, `snare`, `hat_closed`, `hat_open`, `clap`, `tom`, `rim`, `cymbal` / `FAMILY_ORDER`), per-voice density ladders, global density multiplier, rhythmic feel (`swing` / `humanize`), 4,096-entry form bank navigation (`prev`/`next` form, active form name, section progress), and 4,096-entry timbre bank / preset pool snapshots.
5. **External Node.js / Konduktiva JavaScript SDK & Bidirectional OSC API (`src/osc_server.rs` + `js/nkeditor-sdk.js`)**:
   - The Rust binary stays pure Rust and exposes a bidirectional UDP OSC server (plus optional JSON/HTTP or WebSocket bridge) so external Node.js and Konduktiva scripts can query state, load presets, remap controls, update LEDs, and drive the `8opPerc` performance cockpit in real time.

---

## 2. Connected Hardware Reference (Korg nanoKONTROL2)

Currently connected to the host machine:
- **USB ID**: `0944:0117` (`KORG, Inc. nanoKONTROL2 MIDI Controller`)
- **Sysfs Sound Card**: `/sys/class/sound/card2/id` -> `nanoKONTROL2`
- **Stable By-ID Symlink**: `/dev/snd/by-id/usb-KORG_INC._nanoKONTROL2-00 -> ../controlC2`
- **ALSA Sequencer Port**: Client `24` (`nanoKONTROL2`), Port `0` (`nanoKONTROL2 _ CTRL`)
- **ALSA RawMIDI Device**: `hw:2,0,0` (`nanoKONTROL2 _ SLIDER/KNOB`)

### Hardware & Audio Safety Policy
- **Allowed During Autonomous Epochs**: Read-only MIDI/USB hardware discovery, port enumeration, sysfs identity fingerprinting, and non-audible LED/loopback verification on the connected Korg nanoKONTROL2.
- **Strictly Prohibited During Autonomous Epochs**: Do **NOT** launch live audible `8opPerc` synthesis or start audio playback automatically. Always wait for the user's explicit cue after Epoch 20 before starting live audio playback.

---

## 3. Korg nanoKONTROL2 Factory MIDI Map & `8opPerc` Performance Mapping

### Korg nanoKONTROL2 Physical Controls (MIDI Channel 1, CC Messages)
- **8 Track Knobs (Tracks 1..8)**: `CC 16, 17, 18, 19, 20, 21, 22, 23`
- **8 Track Sliders (Tracks 1..8)**: `CC 0, 1, 2, 3, 4, 5, 6, 7`
- **8 Solo Buttons (Tracks 1..8, with LED)**: `CC 32, 33, 34, 35, 36, 37, 38, 39`
- **8 Mute Buttons (Tracks 1..8, with LED)**: `CC 48, 49, 50, 51, 52, 53, 54, 55`
- **8 Rec Buttons (Tracks 1..8, with LED)**: `CC 64, 65, 66, 67, 68, 69, 70, 71`
- **Transport Buttons**:
  - `Rewind`: `CC 43` (no LED)
  - `Forward`: `CC 44` (no LED)
  - `Stop`: `CC 42` (LED)
  - `Play`: `CC 41` (LED)
  - `Record`: `CC 45` (LED)
  - `Cycle`: `CC 46` (LED)
  - `Track Left`: `CC 58` (no LED)
  - `Track Right`: `CC 59` (no LED)
  - `Marker Set`: `CC 60` (no LED)
  - `Marker Left`: `CC 61` (no LED)
  - `Marker Right`: `CC 62` (no LED)

### `8opPerc` MIDImix-to-nanoKONTROL2 Translation & Direct Mapping
In `/home/renick/claude/8operc-konduktiva/src/pulsed-live-rig.js`, the Akai MIDImix uses:
- Mute buttons (`Note 1, 4, 7, 10, 13, 16, 19, 22` ch 1) -> Toggle voice lane mute (`playerNames[0..7]`)
- Rec Arm buttons (`Note 3, 6, 9, 12, 15, 18, 21, 24` ch 1) -> Reroll precomputed timbre for single lane (`playerNames[0..7]`)
- Row 2 knobs (`CC 17, 21, 25, 29, 47, 51, 55, 59` ch 1) -> Per-voice density ladder (`0.1 .. 5.0`)
- Master / global density (`CC 61` ch 1) -> Global density multiplier (`0.1 .. 5.0`)
- Global feel controller (`CC 58` ch 1) -> Global rhythmic feel scale (`swing` & `humanize` `0.0 .. 1.0`, midpoint `CC 64` = `0.5`)
- Bank Left / Right (`Note 25 / Note 26` ch 1) -> Form bank navigation (`previous` / `next` form)
- Solo Master (`Note 27` ch 1) -> Next full 8-voice timbre set from the 4,096-set reservoir
- Send All burst -> Save current 8-voice timbre snapshot to `midimix-random-preset-pool.jsonl`

On the **Korg nanoKONTROL2**, our `8opPerc` Performance Cockpit & companion JS rig (`js/audible-form-batch-nanokontrol2-live.js`) support **both**:
1. **Direct Native nanoKONTROL2 Protocol**:
   - **Track Knobs 1..8 (`CC 16..23`)**: Per-voice density multiplier (`0.1 .. 5.0`) for lanes 1..8.
   - **Track Sliders 1..8 (`CC 0..7`)**: Per-voice level/decay macro (or switches via modifier to global density `Slider 8` / global feel `Slider 7` in Global Layer mode).
   - **Track Mute Buttons 1..8 (`CC 48..55`)**: Toggle voice lane mute (with LED feedback reflecting active/muted state).
   - **Track Solo Buttons 1..8 (`CC 32..39`)**: Solo individual percussion voice lane(s) (new performance capability not wired on MIDImix, with LED feedback).
   - **Track Rec Buttons 1..8 (`CC 64..71`)**: Reroll/advance precomputed timbre for that single lane (flashes/lights Rec LED).
   - **Track Left / Track Right (`CC 58 / CC 59`)**: Form bank `previous` / `next` form navigation.
   - **Cycle (`CC 46`, with LED)**: Advance to next full 8-voice timbre set.
   - **Marker Set (`CC 60`)**: Capture/save current 8-voice timbre snapshot to preset pool (replaces MIDImix's awkward 32-CC Send All burst with a single dedicated button!).
   - **Marker Left / Marker Right (`CC 61 / CC 62`)**: Step global density multiplier down/up or shift active bank page.
   - **Rewind / Forward (`CC 43 / CC 44`)**: Step global feel (`swing`/`humanize`) down/up or audition previous/next saved snapshot.
   - **Play / Stop / Record (`CC 41 / CC 42 / CC 45`, with LEDs)**: Start/pause Konduktiva transport, panic/mute-all, and toggle live WAV stem/mix capture indicator.
2. **MIDImix Compatibility Emulation Mode**:
   - Can also emit the exact MIDImix Note/CC messages (`Notes 1..27`, `CC 17..61`) over the virtual MIDI output port or OSC so unmodified legacy MIDImix scripts work out of the box with a nanoKONTROL2.

---

## 4. Bidirectional OSC Address Space & Node.js/Konduktiva SDK

### OSC Endpoints (Rust App Listening on Configurable UDP Port, Default `9000`; Telemetry Broadcast to Configurable Client Port, Default `9001`)
- `/nkeditor/status` -> Queries or broadcasts full JSON/OSC device, preset, and cockpit state
- `/nkeditor/device/list` -> Returns connected & remembered hardware devices with fingerprints
- `/nkeditor/device/select <device_id>` -> Switches active device tab
- `/nkeditor/preset/load <preset_name_or_slot>` -> Loads a mapping preset and syncs LEDs
- `/nkeditor/preset/save <preset_name>` -> Saves current mapping & control values
- `/nkeditor/control/set <track> <control_name> <value>` -> Updates a control value (0..127) and transmits MIDI/LED
- `/nkeditor/led/set <button_id> <on_off>` -> Explicitly sets hardware LED state
- `/nkeditor/theme/set <theme_name> [bg_preset]` -> Switches `yaae-theme` palette and `twoBowls` background preset
- `/nkeditor/op8/state` -> Receives or broadcasts `8opPerc` live rig state (active form, BPM, global density, feel scale, 8 voice lane states/timbres/densities/mutes/solos)
- `/nkeditor/op8/form <direction_or_name>` -> Navigates `8opPerc` form bank (`next`, `previous`, or specific form name)
- `/nkeditor/op8/timbre <lane_or_all> [timbre_name]` -> Rerolls or selects timbre for a voice lane (0..7) or all lanes
- `/nkeditor/op8/lane/<index>/density <value>` -> Sets per-lane density (`0.1 .. 5.0`)
- `/nkeditor/op8/lane/<index>/mute <0|1>` -> Sets per-lane mute state and updates nanoKONTROL2 Mute LED
- `/nkeditor/op8/lane/<index>/solo <0|1>` -> Sets per-lane solo state and updates nanoKONTROL2 Solo LED
- `/nkeditor/op8/snapshot/save [label]` -> Triggers atomic 8-voice preset pool capture
