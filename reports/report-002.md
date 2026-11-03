---
title: "Epoch 2: Port-Independent USB/ALSA Hardware Fingerprinting & Simultaneous Multi-Device MIDI Manager"
date: 2026-11-04 00:08:30
project: "nkEditor4 (Rust egui Port & 8opPerc Performance Cockpit)"
project_dir: "/home/renick/antigravity/nkeditor-agy"
report_number: 2
follows: "report-001"
subject: "Epoch 2 of 20: src/hardware.rs (sysfs, /dev/snd, /proc/asound fingerprinting) & src/midi_io.rs (MultiDeviceManager & hot-plug persistence)"
operator: "Gemini 4 Argon (Medium)"
tldr: "Implemented `src/hardware.rs` and `src/midi_io.rs` for simultaneous multi-device MIDI controller management and USB port-independent hardware identity persistence. `src/hardware.rs` inspects Linux `/sys/class/sound/card*/` (`id`, `device/../idVendor`, `device/../idProduct`, `device/../serial`, `device/../manufacturer`, `device/../product`), `/dev/snd/by-id/`, `/dev/snd/by-path/`, and `/proc/asound/seq/clients` (with `aconnect -l` and `amidi -l` enrichment) to compute a stable `HardwareFingerprint` (`vendor_id:product_id:serial:card_name`, verified live against the connected Korg nanoKONTROL2 `0944:0117::nanoKONTROL2` on `card2` / ALSA `24:0` and Behringer UMC1820 `1397:0503:B8AAF8D3:UMC1820` on `card0` / ALSA `16:0`). `src/midi_io.rs` implements `MultiDeviceManager` with persistent profile storage, hot-plug detection across changing USB ports (`24:0` -> `32:0`), and automatic 28-LED resynchronization, verified by 22 passing automated tests (`17` Rust unit/integration tests + `5` Node.js SDK tests)."
qa_verdict: "PASS"
tests_total: 22
---

# Prompt

Epoch 2 of 20: Implement src/hardware.rs and src/midi_io.rs for simultaneous multi-device MIDI controller management and USB port-independent hardware identity persistence. Inspect Linux /sys/class/sound/card*/ (id, device/../idVendor, device/../idProduct, device/../serial, device/../manufacturer, device/../product), /dev/snd/by-id/, /dev/snd/by-path/, and /proc/asound/seq/clients (plus aconnect -l / amidi -l fallback) to compute a stable HardwareFingerprint (vendor_id:product_id:serial:card_name, such as 0944:0117 for the currently connected Korg nanoKONTROL2 on card2 / ALSA client 24:0). Build the MultiDeviceManager that connects multiple controllers simultaneously, remembers each device's hardware identity and assigned preset/role in persistent config, and automatically detects hot-plug removal and reconnection—even when plugged into a different physical USB port where the ALSA client number (e.g. 24:0 -> 28:0) or USB bus path changes. Add unit and integration tests including read-only verification against the connected Korg nanoKONTROL2 (if present) and simulated multi-port USB re-plug scenarios (with CARGO_BUILD_JOBS=2), git commit all changes, and compile reports/report-002.html from reports/report-002.md via /home/renick/claude/html-report-generator/build-report.sh --pdf and commit.

# Content

## 1. Plain-Language Overview & Epoch 2 Goals

When a musician plugs a USB MIDI controller into a Linux computer, the operating system's audio and MIDI subsystem (**ALSA**, the Advanced Linux Sound Architecture) assigns it a temporary sound card number (like `card2`) and a temporary MIDI client number (like `24:0`). If the musician unplugs that controller during a rehearsal and plugs it into a different USB socket on the laptop or USB hub, Linux often gives it a brand-new card number (`card3` or `card4`), a different ALSA client number (`28:0` or `32:0`), and a different physical bus path (`/dev/snd/by-path/pci-...-usb-0:2.4:1.0`).

In naive MIDI programs, changing the USB socket breaks the connection: the software forgets which controller is which, loses the loaded mapping preset, and leaves all the physical button lights (LEDs) dark.

In **Epoch 2 of 20**, we solved this problem permanently by building two core Rust modules:
1. **Port-Independent Hardware Fingerprinting (`src/hardware.rs`)**: Reads the hardware's permanent USB identity directly from the Linux kernel (`/sys/class/sound/card*/`, `/dev/snd/by-id/`, `/dev/snd/by-path/`, and `/proc/asound/seq/clients`, plus `aconnect -l` and `amidi -l`) to create a **`HardwareFingerprint`** (`vendor_id:product_id:serial:card_name`). Even if the USB socket, card number, and ALSA port number all change, the hardware fingerprint (`0944:0117::nanoKONTROL2` for the Korg nanoKONTROL2) stays 100% identical.
2. **Simultaneous Multi-Device Manager (`src/midi_io.rs`)**: Manages multiple MIDI controllers at the same time (`MultiDeviceManager`), saves each controller's assigned preset, role (`DeviceRole`), and LED states in `PersistentHardwareConfig`, and automatically detects when a controller is unplugged and plugged back into any USB port, restoring its preset and queuing the exact MIDI messages to relight all 28 hardware LEDs.

### Table: Plain-English Concepts Mapped to Code Components

| Plain-English Concept | What It Does for the Musician | Exact Code Component |
| --- | --- | --- |
| Permanent Hardware Fingerprint | Identifies a controller by its USB Vendor/Product ID, Serial, and Card ID (`0944:0117::nanoKONTROL2`) instead of a temporary port number | `src/hardware.rs` (`HardwareFingerprint`, `HardwareFingerprint::matches_replugged`) |
| Linux Sysfs Sound Card Scanner | Reads `/sys/class/sound/card*/` and `/dev/snd/by-{id,path}` symlinks | `src/hardware.rs` (`SysfsSoundCard`, `scan_sysfs_sound_cards`, `scan_dev_snd_symlinks`) |
| ALSA Sequencer & RawMIDI Discovery | Parses `/proc/asound/seq/clients`, `aconnect -l`, and `amidi -l` | `src/hardware.rs` (`AlsaSeqClient`, `AlsaRawMidiEntry`, `parse_proc_asound_seq_clients`) |
| Unified Device Correlator | Merges sysfs USB identity with live ALSA `24:0` and `hw:2,0,0` addresses | `src/hardware.rs` (`DiscoveredMidiDevice`, `correlate_hardware_devices`, `discover_connected_midi_hardware`) |
| Simultaneous Multi-Device Manager | Connects multiple controllers at once, tracks hot-plug events, and persists settings | `src/midi_io.rs` (`MultiDeviceManager`, `ManagedDeviceSession`, `HotplugEvent`) |
| Automatic LED Resynchronization | Generates the 28 CC or 24 Note MIDI packets to restore button LEDs on reconnect | `src/midi_io.rs` (`ManagedDeviceSession::build_led_sync_packets`, `RawMidiPacket`) |

## 2. Phase 1: Linux Sysfs, `/dev/snd`, and `/proc/asound` Hardware Discovery (`src/hardware.rs`)

### Phase: Correlating USB Descriptors with Dynamic ALSA Ports

`src/hardware.rs` inspects four layers of Linux kernel and ALSA state in read-only mode:
- **`/sys/class/sound/card*/`**: Reads `id` (e.g. `nanoKONTROL2`), checks for `midiC*D*` / `dmmidi*` child nodes, and reads USB device attributes (`idVendor`, `idProduct`, `serial`, `manufacturer`, `product`) from `device/..`.
- **`/dev/snd/by-id/` and `/dev/snd/by-path/`**: Maps each `../controlCN` symlink target back to `card_index` so every `SysfsSoundCard` knows both its stable USB symlink (`usb-KORG_INC._nanoKONTROL2-00`) and its current physical USB socket (`pci-0000:00:14.0-usb-0:5.3:1.0`).
- **`/proc/asound/seq/clients` and `aconnect -l` / `amidi -l`**: Extracts the active ALSA Sequencer client and port (`24:0`, `"nanoKONTROL2 _ CTRL"`) and RawMIDI address (`hw:2,0,0`, `"nanoKONTROL2 _ SLIDER/KNOB"`).

### Bug: Preventing USB Host Controller Serial Bleed When Device Lacks a USB Serial Descriptor

- **symptom**: During our first read-only test against the live Korg nanoKONTROL2 on `card2`, `nk2.fingerprint.canonical_key()` returned `"0944:0117:0000:00:14.0:nanoKONTROL2"` instead of `"0944:0117::nanoKONTROL2"`.
- **root_cause**: Unlike the Behringer UMC1820 (which exposes `/sys/class/sound/card0/device/../serial` = `B8AAF8D3`), the Korg nanoKONTROL2 does not have a `serial` file in its USB device directory (`1-5.3`). When `read_usb_attr` walked up 4 parent directories looking for `serial`, it climbed out of the USB device into the PCI USB host controller (`usb1`), whose `serial` file contains the PCI bus address `0000:00:14.0`.
- **fix**: Updated `read_usb_attr` in `src/hardware.rs` to anchor strictly on the directory containing `idVendor` and `idProduct` (`card_dir/device/..` or `card_dir/device`) and never walk higher up the sysfs tree.
- **rationale**: Anchoring on the USB device directory that owns `idVendor` and `idProduct` guarantees that devices with a true hardware serial number (like `UMC1820` -> `B8AAF8D3`) report their serial, while devices without a USB serial descriptor (like `nanoKONTROL2`) cleanly report `""` instead of inheriting the PCI host controller's address.

### Table: Live Host Hardware Discovery Results (`discover_connected_midi_hardware()`)

| Discovered Device | `HardwareFingerprint` | Matched Definition | Sysfs Card & `/dev/snd/by-id` | ALSA Seq & RawMIDI | Physical USB Path (`/dev/snd/by-path`) |
| --- | --- | --- | --- | --- | --- |
| **Korg nanoKONTROL2** | `0944:0117::nanoKONTROL2` | `Some("nanokontrol2")` | `card2` (`usb-KORG_INC._nanoKONTROL2-00`) | `24:0` (`nanoKONTROL2 _ CTRL`), `hw:2,0,0` | `pci-0000:00:14.0-usb-0:5.3:1.0` |
| **Behringer UMC1820** | `1397:0503:B8AAF8D3:UMC1820` | `None` (`Auxiliary`) | `card0` (`usb-BEHRINGER_UMC1820_B8AAF8D3-00`) | `16:0` (`UMC1820 MIDI 1`), `hw:0,0,0` | `pci-0000:00:14.0-usb-0:6:1.0` |

## 3. Phase 2: Simultaneous Multi-Device Manager & Hot-Plug Persistence (`src/midi_io.rs`)

### Phase: Multi-Controller Sessions, USB Socket Migration, and LED Resynchronization

`src/midi_io.rs` provides `MultiDeviceManager`, which maintains active and remembered `ManagedDeviceSession` entries alongside a JSON-serializable `PersistentHardwareConfig`:
- **Simultaneous Multi-Device Management**: Connects and tracks multiple controllers at the same time (for example, a Korg `nanoKONTROL2` on `24:0`, an Akai `MIDI Mix` on `28:0`, and a `UMC1820` on `16:0`), each with its own independent `DeviceRole`, `active_preset`, `control_values`, and `led_states`.
- **Hot-Plug Removal & USB Port Migration (`reconcile_discovered_devices`)**: Emits `HotplugEvent::ConnectedNew` when a device is seen for the first time, `HotplugEvent::Disconnected` when a connected controller disappears, and `HotplugEvent::ReconnectedRemembered` when a remembered device reappears—even if plugged into a different physical USB port where its ALSA address changes (e.g. `24:0` -> `32:0`), its card index changes (`card2` -> `card4`), and its `/dev/snd/by-path` changes (`usb-0:5.3:1.0` -> `usb-0:2.4:1.0`).
- **Automatic Hardware LED Resynchronization (`build_led_sync_packets`)**: Immediately upon reconnection, `MultiDeviceManager` restores the device's `active_preset` and queues all 28 CC packets (for `nanokontrol2`) or 24 Note packets (for `akai-midimix`) to bring the physical controller's LEDs back into sync with the software state.

# Files Touched

- `src/hardware.rs` — Implemented Linux sysfs (`/sys/class/sound/card*/`), `/dev/snd/by-id/`, `/dev/snd/by-path/`, `/proc/asound/seq/clients`, `aconnect -l`, and `amidi -l` discovery and `HardwareFingerprint` correlation
- `src/midi_io.rs` — Implemented `MultiDeviceManager`, `ManagedDeviceSession`, `PersistentHardwareConfig`, `HotplugEvent`, and `RawMidiPacket` LED synchronization
- `src/lib.rs` — Exported `hardware` and `midi_io` modules
- `src/main.rs` — Integrated `MultiDeviceManager::poll_host_hardware()` into `NkEditorApp` and `--check-init` CLI diagnostics
- `js/nkeditor-sdk.js` — Added `formatHardwareFingerprint` and `parseHardwareFingerprint` helpers to the companion Node.js SDK
- `js/nkeditor-sdk.test.js` — Added unit tests for `HardwareFingerprint` formatting and parsing

# Tests

- `CARGO_BUILD_JOBS=2 cargo test` — 17 Rust unit and integration tests passing (`7` new tests in `hardware::tests` and `midi_io::tests` including live read-only Korg nanoKONTROL2 verification and simulated multi-port USB re-plug migration), 0 failures
- `CARGO_BUILD_JOBS=2 cargo run -- --check-init` — Verified live discovery of 2 connected hardware MIDI devices (`nanoKONTROL2` and `UMC1820`)
- `npm test` (`node --test js/*.test.js`) — 5 Node.js SDK unit tests passing, 0 failures

# QA

- `22 tests passing (17 Rust + 5 Node.js)`
- `Live hardware fingerprint verified: 0944:0117::nanoKONTROL2 (card2, ALSA 24:0, hw:2,0,0)`
- `Simulated USB port migration (ALSA 24:0 -> 32:0) & 28-LED resync verified`
- `0 warnings, 0 failures`

All Epoch 2 deliverables compiled cleanly with `CARGO_BUILD_JOBS=2` and passed all Rust and Node.js test suites in read-only hardware mode.
