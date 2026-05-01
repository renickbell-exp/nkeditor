# nkEditor3 Architecture

This document describes the architecture of **nkEditor3**, an Electron + React 18 + TypeScript desktop application that serves as a MIDI controller editor and CC remapper. It maps physical MIDI controller inputs to configurable output CC/Note messages sent to a virtual MIDI port, which DAWs and synthesizers can connect to.

**Version:** 2.0.0
**License:** MIT
**Node requirement:** >= 18.0.0
**Test count:** 356 tests across 7 test files (Vitest)

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Directory Structure](#2-directory-structure)
3. [Build and Toolchain](#3-build-and-toolchain)
4. [Process Architecture and IPC](#4-process-architecture-and-ipc)
5. [Device Abstraction Layer](#5-device-abstraction-layer)
6. [MIDI Message Flow](#6-midi-message-flow)
7. [Main Process Services](#7-main-process-services)
8. [Renderer Store Architecture](#8-renderer-store-architecture)
9. [Preset System](#9-preset-system)
10. [UI Component Hierarchy](#10-ui-component-hierarchy)
11. [Target Synthesizer Support](#11-target-synthesizer-support)
12. [Testing](#12-testing)
13. [Key Design Decisions](#13-key-design-decisions)

---

## 1. System Overview

nkEditor3 is a desktop MIDI CC remapper. A user connects a physical MIDI controller (Korg nanoKONTROL2 or Akai MidiMix), configures how each physical control maps to an output CC number and channel, and the application creates a virtual MIDI output port that DAWs and synthesizers can subscribe to.

### High-Level Data Flow

```
Physical Controller                nkEditor3                      DAW / Synth
(nanoKONTROL2 / MidiMix)     (Electron Desktop App)           (via virtual port)

  Knobs, Sliders,    MIDI CC/Note    MidiHandler     MappingEngine    Virtual Output
  Buttons          ──────────────>  (hardware I/O) ──> (remap logic) ──> "nkEditor3 Out"
                                         |                 |
                                    LED Control        Button State
                                    (CC/Note back       Tracking
                                     to hardware)
```

### Supported Devices

| Device | ID | Controls per Track | Message Types | Transport |
|--------|----|--------------------|---------------|-----------|
| Korg nanoKONTROL2 | `nanokontrol2` | 1 knob, 1 slider, 3 buttons | CC only | 11 buttons |
| Akai MidiMix | `akai-midimix` | 3 knobs, 1 fader, 3 buttons | CC (continuous) + Note (buttons) | None (master section instead) |

### Target Synthesizers

| Synth | Module |
|-------|--------|
| Nord Drum 2 Desktop | `src/shared/nord-drum-2-cc.ts` |

---

## 2. Directory Structure

```
nkEditor3/
  src/
    shared/                     # Types, schemas, constants shared between main and renderer
      types.ts                  # Core type definitions (MappingEntry, TrackMapping, etc.)
      schemas.ts                # Zod validation schemas + factory preset creation
      constants.ts              # Hardware CC tables, utility predicates
      ipc-protocol.ts           # IPC channel names, request/response types
      nord-drum-2-cc.ts         # Nord Drum 2 CC reference constants
      devices/
        device-definition.ts    # DeviceDefinition interface and control types
        registry.ts             # Device registry (Map<string, DeviceDefinition>)
        nanokontrol2.ts         # nanoKONTROL2 hardware definition
        akai-midimix.ts         # Akai MidiMix hardware definition
        index.ts                # Barrel export for the device layer
    main/
      main.ts                   # Electron app lifecycle, window creation
      preload.ts                # contextBridge API (window.electronAPI)
      ipc-handlers.ts           # IPC handler registration and routing
      utils/
        renderer-bridge.ts      # sendToRenderer() helper (breaks circular dep)
      services/
        midi-manager.ts         # High-level MIDI orchestrator
        midi-handler.ts         # Low-level easymidi I/O, hotplug detection
        mapping-engine.ts       # CC/Note remapping logic, button state tracking
        led-controller.ts       # LED state management and hardware sync
        midi-discovery.ts       # Port enumeration, device auto-detection
        config-manager.ts       # AppConfig persistence (config.json)
        file-manager.ts         # Preset file storage (presets/*.json)
        config-parser.ts        # Legacy mapping file parser
    renderer/
      main.tsx                  # React entry point (createRoot)
      App.tsx                   # Root component, layout, store initialization
      styles/
        globals.css             # Tailwind directives + custom styles
      stores/
        connection.ts           # MIDI connection state (Zustand)
        controls.ts             # Control values and button states (Zustand)
        preset.ts               # Preset library and selection (Zustand)
        settings.ts             # AppConfig mirror (Zustand)
        devices.ts              # Multi-device instance management (Zustand)
        undo.ts                 # Undo history with per-device stacks (Zustand)
      components/
        Header.tsx              # App header with connection status + tab nav
        DeviceTabBar.tsx        # Multi-device tab strip
        DeviceSelector.tsx      # Device type picker
        AddDeviceDialog.tsx     # Dialog for adding a new device instance
        Editor/
          EditorView.tsx        # Main nK2 editor view (tracks + transport)
          DeviceEditorContainer.tsx  # Multi-device editor router
          Track.tsx             # Single track strip (knob + slider + buttons)
          Knob.tsx              # Rotary knob control
          Slider.tsx            # Linear fader control
          Button.tsx            # Toggle/momentary button control
          Transport.tsx         # Transport section (11 buttons)
          ControlPopover.tsx    # CC mapping editor popover
          MidiMix/
            MidiMixEditor.tsx       # MidiMix-specific editor layout
            MidiMixColumn.tsx       # Single MidiMix channel strip
            MidiMixMasterSection.tsx # Master fader + global buttons
        Librarian/
          LibrarianView.tsx     # Preset browser and manager
          PresetList.tsx        # Scrollable preset list with filtering
          QuickAccess.tsx       # 5-slot quick access bar
          NewPresetDialog.tsx   # Create preset dialog
          EditPresetDialog.tsx  # Edit preset metadata dialog
          DuplicatePresetDialog.tsx # Duplicate preset dialog
          EditControlValuesDialog.tsx # Edit stored control values
        Settings/
          SettingsView.tsx      # Application settings panel
        Help/
          HelpView.tsx          # Help and documentation panel
      types/
        electron.d.ts           # Type augmentation for window.electronAPI
  tests/                        # Vitest test files
  presets/                      # Legacy preset directory (unused at runtime)
  mappings/                     # Legacy mapping files
  assets/                       # Application icons and images
  index.html                    # Electron renderer HTML shell
  vite.config.ts                # Vite + electron plugin configuration
  tsconfig.json                 # TypeScript configuration
  tailwind.config.js            # Tailwind CSS theme configuration
  package.json                  # Dependencies and scripts
```

---

## 3. Build and Toolchain

### Technology Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Runtime | Electron | ^28.0.0 |
| UI Framework | React | ^18.2.0 |
| Language | TypeScript | ^5.3.0 |
| Bundler | Vite | ^5.0.0 |
| Electron plugin | vite-plugin-electron | ^0.15.0 |
| CSS | Tailwind CSS | ^3.3.6 |
| State Management | Zustand | ^4.4.0 |
| Validation | Zod | ^3.22.0 |
| MIDI I/O | easymidi | ^3.1.0 |
| Testing | Vitest | ^1.0.0 |

### Build Configuration

**Vite** (`vite.config.ts`, lines 1-89) handles three build targets:

1. **Main process** (`src/main/main.ts`): Built to `dist-electron/main.cjs` as CommonJS (required by Electron). The `easymidi` native module is externalized from the bundle.
2. **Preload script** (`src/main/preload.ts`): Built to `dist-electron/preload.cjs` as CommonJS.
3. **Renderer** (`src/renderer/main.tsx`): Built to `dist/` as ESM with React plugin.

Path aliases are configured in both `tsconfig.json` (lines 24-28) and `vite.config.ts` (lines 66-72):

| Alias | Target |
|-------|--------|
| `@/*` | `src/*` |
| `@shared/*` | `src/shared/*` |
| `@renderer/*` | `src/renderer/*` |
| `@main/*` | `src/main/*` |

**TypeScript** (`tsconfig.json`) is configured with strict mode, `noUncheckedIndexedAccess`, `noUnusedLocals`, and `noUnusedParameters`.

### NPM Scripts

| Script | Purpose |
|--------|---------|
| `npm run dev` | Start Vite dev server (renderer only) |
| `npm run electron:dev` | Start Vite + Electron concurrently |
| `npm run build` | Full production build (typecheck + Vite + electron-builder) |
| `npm test` | Run all tests with Vitest |
| `npm run test:watch` | Run tests in watch mode |

### Packaging

Electron-builder (`package.json`, lines 55-76) packages the app for:
- **Linux**: AppImage and .deb
- **macOS**: DMG
- **Windows**: NSIS installer

---

## 4. Process Architecture and IPC

### Electron Process Model

The application follows Electron's standard two-process model with strict context isolation:

```
  Main Process (Node.js)                 Renderer Process (Chromium)
  ========================              ============================
  src/main/main.ts                      src/renderer/main.tsx
    - BrowserWindow creation              - React 18 application
    - IPC handler registration            - Zustand stores
    - Service instantiation               - Tailwind CSS UI
    - App lifecycle events
                    |                                |
                    |       contextBridge             |
                    +-------(preload.ts)-------------+
                            window.electronAPI
```

**Security posture** (`src/main/main.ts`, lines 30-35):
- `nodeIntegration: false` -- Node.js APIs are not available in the renderer
- `contextIsolation: true` -- preload runs in an isolated context
- `sandbox: false` -- required for the `easymidi` native module
- Content Security Policy in `index.html` (line 6): `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'`

### IPC Protocol

All IPC communication is defined in `src/shared/ipc-protocol.ts`. There are two communication patterns:

#### 1. Events (Main -> Renderer): `IPC_EVENTS`

Defined at `src/shared/ipc-protocol.ts`, lines 15-22. Sent via `sendToRenderer()` (`src/main/utils/renderer-bridge.ts`, line 32).

| Channel | Payload Type | Purpose |
|---------|-------------|---------|
| `midi:cc` | `MidiCCEvent` | CC message received from hardware (with optional remap info) |
| `midi:note` | `MidiNoteEvent` | Note On/Off from hardware |
| `midi:connected` | `MidiConnectedEvent` | Device connected |
| `midi:disconnected` | `MidiDisconnectedEvent` | Device disconnected |
| `midi:portsChanged` | `MidiPortsChangedEvent` | Available MIDI ports changed |
| `error` | `ErrorEvent` | Error occurred in main process |

#### 2. Invoke (Renderer -> Main): `IPC_INVOKE`

Defined at `src/shared/ipc-protocol.ts`, lines 25-46. The renderer calls `window.electronAPI.*` methods which use `ipcRenderer.invoke()`.

| Channel | Request | Response | Purpose |
|---------|---------|----------|---------|
| `midi:send` | `SendCCRequest` | `void` | Send CC to virtual output |
| `midi:connect` | `ConnectRequest` | `ConnectResponse` | Connect to hardware |
| `midi:disconnect` | -- | `void` | Disconnect hardware |
| `midi:ports` | -- | `GetPortsResponse` | List available MIDI ports |
| `midi:status` | -- | `MidiStatusResponse` | Get connection status |
| `preset:load` | `LoadPresetRequest` | `LoadPresetResponse` | Load preset from disk |
| `preset:save` | `SavePresetRequest` | `SavePresetResponse` | Save preset to disk |
| `preset:delete` | `DeletePresetRequest` | `DeletePresetResponse` | Delete preset file |
| `preset:list` | -- | `ListPresetsResponse` | List all preset metadata |
| `preset:apply` | `ApplyMappingRequest` | `ApplyMappingResponse` | Apply mapping to MidiManager |
| `config:get` | -- | `GetConfigResponse` | Get application config |
| `config:update` | `UpdateConfigRequest` | `UpdateConfigResponse` | Update application config |
| `device:list` | -- | -- | List registered device definitions |

### Preload Bridge

`src/main/preload.ts` (lines 38-186) exposes the `electronAPI` object to the renderer via `contextBridge.exposeInMainWorld()`. Each method maps to exactly one IPC invoke call. Event subscriptions return cleanup functions.

The exposed API type is exported as `ElectronAPI` (line 189), which renderer stores import for type safety.

### Renderer Bridge

`src/main/utils/renderer-bridge.ts` solves a circular dependency between `main.ts` (which creates the window) and `ipc-handlers.ts` (which needs to send events to the window). It provides:

- `setMainWindow(window)` -- called from `main.ts` after window creation (line 40) and on close (line 62)
- `sendToRenderer(channel, ...args)` -- safely sends via `webContents.send()`, silently no-ops if window is destroyed

---

## 5. Device Abstraction Layer

The device abstraction layer (`src/shared/devices/`) enables the application to support multiple MIDI controllers without embedding device-specific knowledge outside of definition files.

### DeviceDefinition Interface

Defined in `src/shared/devices/device-definition.ts` (lines 141-162):

```typescript
interface DeviceDefinition {
  id: string;                      // 'nanokontrol2', 'akai-midimix'
  name: string;                    // Human-readable name
  manufacturer: string;            // 'Korg', 'Akai'
  portPatterns: RegExp[];          // Regex for auto-detection from MIDI port names
  trackCount: number;              // Number of channel strips (8 for both devices)
  trackLayouts: TrackLayout[];     // Per-track control definitions
  extraSections: ExtraControlsSection[];  // Transport, master, etc.
  ledProtocol: LedProtocol;        // How to control LEDs (CC vs Note, on/off values)
  leds: LedDef[];                  // All controllable LED mappings
  virtualPortName?: string;        // Custom virtual port name
}
```

### Control Type Hierarchy

Controls use a discriminated union (`ControlDef`) with two variants:

- **`ContinuousControlDef`** (`kind: 'continuous'`): Knobs, sliders, faders. Always `messageType: 'cc'`. Has a `visualType` hint (`'knob' | 'slider' | 'fader'`).
- **`ButtonControlDef`** (`kind: 'button'`): Solo, mute, rec, transport. `messageType` can be `'cc'` (nK2) or `'note'` (MidiMix). Has `defaultBehavior` (`'toggle' | 'momentary'`) and `hasLed` flag.

### Device Registry

`src/shared/devices/registry.ts` (lines 20-23) maintains a `Map<string, DeviceDefinition>`:

```typescript
const DEVICE_REGISTRY: Map<string, DeviceDefinition> = new Map([
  [NANOKONTROL2_DEFINITION.id, NANOKONTROL2_DEFINITION],
  [AKAI_MIDIMIX_DEFINITION.id, AKAI_MIDIMIX_DEFINITION],
]);
```

**Public API** (lines 38-85):

| Function | Purpose |
|----------|---------|
| `getDeviceDefinition(id)` | Lookup by device ID |
| `getAllDeviceDefinitions()` | Return all registered definitions |
| `identifyDevice(portName)` | Match a MIDI port name against registered `portPatterns` |
| `getDefaultDevice()` | Return nanoKONTROL2 (fallback for legacy configs) |

### Adding a New Device

To add a new MIDI controller:

1. Create a definition file in `src/shared/devices/` (follow `akai-midimix.ts` as a template)
2. Register it in `src/shared/devices/registry.ts` by adding an entry to `DEVICE_REGISTRY`
3. Export it from `src/shared/devices/index.ts`
4. Optionally add factory presets in `src/shared/schemas.ts` and include them in `createSamplePresets()`

---

## 6. MIDI Message Flow

### Inbound Flow (Hardware -> Virtual Output)

```
Hardware Controller
       |
       | CC or Note On/Off messages (via ALSA/RtMidi)
       v
  MidiHandler (src/main/services/midi-handler.ts)
       |  Emits 'cc' event for CC messages
       |  Emits 'noteon'/'noteoff' for Note messages
       v
  MidiManager.handleIncomingCC() (src/main/services/midi-manager.ts, line 296)
       |
       |  Passes to MappingEngine.processCC()
       v
  MappingEngine.processMessage() (src/main/services/mapping-engine.ts, line 219)
       |  Lookup via composite key ("cc:42" or "note:25")
       |  Apply button behavior (toggle/momentary)
       |  Transform inputCC -> outputCC, channel
       |
       +---> ProcessedMessage returned to MidiManager
       |
       v
  MidiManager (line 331):
       |  handler.sendCC(channel, outputCC, value) --> Virtual Output
       |  emit('cc', MidiCCEvent) --> IPC_EVENTS.MIDI_CC --> Renderer
       |
       v
  LedController.setLed() (if internal LED mode)
       |  Send CC/Note to hardware output for LED feedback
```

**Critical design rule** (documented at `src/main/services/midi-manager.ts`, lines 292-295): Only **mapped** CCs are forwarded to the virtual output. Raw/unmapped CCs are never forwarded because hardware CC numbers may conflict with standard MIDI CCs (e.g., nanoKONTROL2 sliders use CC 0-7, where CC 7 is MIDI Volume).

### MappingEngine Internals

The `MappingEngine` (`src/main/services/mapping-engine.ts`) maintains two lookup maps for fast O(1) message routing:

1. **`compositeToMapping`** (`Map<string, MappingLookupEntry>`): Keys like `"cc:42"` or `"note:25"` prevent collisions between CC and Note messages with the same number. This is the primary lookup path.

2. **`ccToMapping`** (`Map<number, MappingLookupEntry>`): Legacy numeric lookup retained for backward compatibility with code that calls `getButtonState(cc)`.

**Composite key construction** (line 77):
```typescript
function makeLookupKey(messageType: MidiMessageType, midiNumber: number): string {
  return `${messageType}:${midiNumber}`;
}
```

**Button behavior processing** (`processButtonInput()`, lines 592-649):
- **Toggle**: On press (value >= 64), flip internal state, output 127 or 0. Releases are ignored.
- **Momentary**: Pass through normalized values (127 on press, 0 on release).

Both behaviors emit `buttonStateChanged` events for LED synchronization.

### LED Control

The `LedController` (`src/main/services/led-controller.ts`) has two operating modes determined by the device definition:

1. **Legacy mode** (no device definition): Uses nK2-specific CC-based LED control via `sendLedCC()`
2. **Device-aware mode**: Reads the device's `LedProtocol` to choose between CC and Note messages, with configurable on/off values and channel

LED modes (`src/main/services/midi-manager.ts`, lines 216-219):
- **Internal**: LEDs reflect button toggle state (updated by `MappingEngine.buttonStateChanged` events)
- **External**: LEDs are controlled by incoming MIDI from the DAW (button presses do not update LEDs)

### Hotplug Detection

`MidiHandler.checkForPortChanges()` (lines 637-690) uses two strategies to minimize ALSA resource consumption:

1. **When connected**: Sends a heartbeat CC message (CC 127, value 0) to test connection health without creating ALSA clients.
2. **When disconnected**: Enumerates available ports to search for the device. Port enumeration results are cached for 500ms (`midi-discovery.ts`, line 88) to prevent ALSA client exhaustion.

---

## 7. Main Process Services

Service instances are created in `ipc-handlers.ts` `initializeServices()` (lines 49-61) in this order:

1. **ConfigManager** (first, because others may depend on config)
2. **FileManager** (preset storage)
3. **MidiManager** (MIDI I/O orchestration)

### MidiManager

**File**: `src/main/services/midi-manager.ts`
**Role**: High-level orchestrator that composes MidiHandler, MappingEngine, and LedController.

Key behaviors:
- Constructor (line 53) creates a default identity mapping engine immediately so physical controls work on startup
- `connect()` (line 81) syncs LED states after successful connection
- `handleIncomingCC()` (line 296) is the central routing function for all incoming MIDI
- `loadMapping()` (line 168) updates the MappingEngine and re-syncs LEDs

### MidiHandler

**File**: `src/main/services/midi-handler.ts`
**Role**: Low-level MIDI I/O using the `easymidi` native module.

Manages three port types:
- **Hardware input** (`easymidi.Input`): Receives CC/Note from controller
- **Hardware output** (`easymidi.Output`): Sends LED control messages to controller
- **Virtual output** (`easymidi.Output`, created with `virtual: true`): Remapped output for DAWs

Virtual port creation is non-fatal (line 894) -- the app can operate without it for direct hardware monitoring.

### MappingEngine

**File**: `src/main/services/mapping-engine.ts`
**Role**: Pure remapping logic with no I/O dependencies.

The engine supports three entry points:
- `processCC()` (line 188) -- delegates to `processMessage('cc', ...)`
- `processNote()` (line 203) -- delegates to `processMessage('note', ...)`
- `processMessage()` (line 219) -- unified lookup and transformation

### FileManager

**File**: `src/main/services/file-manager.ts`
**Role**: Preset persistence as JSON files in Electron's `userData` directory.

Storage location: `{app.getPath('userData')}/presets/{sanitized_id}.json`

Path traversal prevention (line 157):
```typescript
const safeId = id.replace(/[^a-zA-Z0-9_-]/g, '_');
```

On first run (empty presets directory), `initializeSamplePresets()` (line 230) creates 10 factory presets by calling `createSamplePresets()` from `schemas.ts`.

### ConfigManager

**File**: `src/main/services/config-manager.ts`
**Role**: Application settings persistence as `{userData}/config.json`.

Features:
- Dirty-flag auto-save every 30 seconds (line 36)
- Forward-compatible loading: merges saved config with defaults to handle new fields (line 229)
- Zod validation on every update (line 82)
- Cleanup method saves pending changes on shutdown (line 51)

### MIDI Discovery

**File**: `src/main/services/midi-discovery.ts`
**Role**: Port enumeration and device identification.

Key functions:
- `getAvailablePorts()` -- Cached port enumeration (500ms TTL, line 88) to prevent ALSA exhaustion
- `findNanoKontrol2()` -- Legacy single-device search
- `discoverDevices()` (line 381) -- Multi-device discovery using the device registry's `identifyDevice()`
- `isMidiAvailable()` -- Checks subsystem health with cached failure state

Graceful degradation: When ALSA is unavailable, the app runs in "demo mode" without MIDI functionality. Error is logged once (line 251) to avoid spam during hotplug polling.

---

## 8. Renderer Store Architecture

All renderer state is managed by **Zustand** stores (6 stores total). Each store follows a consistent pattern: get the `electronAPI` from `window`, perform async operations via IPC, and update state via `set()`.

### Store Dependency Graph

```
App.tsx initializes all stores on mount
  |
  +-> useConnectionStore.initialize()  -- Subscribes to MIDI events
  +-> useSettingsStore.initialize()    -- Loads AppConfig from main
  +-> useControlsStore.initialize()   -- Subscribes to midi:cc for live control updates
  +-> usePresetStore.loadPresets()     -- Lists presets, auto-loads first one
  +-> useDevicesStore (passive)        -- Populated when user adds devices
  +-> useUndoStore (passive)           -- Populated on user actions
```

### Connection Store

**File**: `src/renderer/stores/connection.ts`
**Purpose**: Tracks MIDI connection state and port availability.

Legacy single-device state (lines 33-43):
```typescript
connected: boolean;
connecting: boolean;
inputPort: string | null;
outputPort: string | null;
virtualPort: string;
midiAvailable: boolean;
availablePorts: { inputs: string[]; outputs: string[] };
```

Multi-device additions:
- `portDeviceMap: Map<string, string>` -- Maps input port names to device instance IDs
- `registerPortForDevice()`, `unregisterPort()`, `getDeviceForPort()` for port ownership routing

### Controls Store

**File**: `src/renderer/stores/controls.ts`
**Purpose**: Real-time control values for the GUI (knob positions, slider values, button states).

This store has a **dual interface**:

1. **Legacy interface** (lines 207-230): Fixed `tracks: TrackState[]` (8 elements) and `transport: TransportState` matching the nK2 layout. Used by existing EditorView and Track components.

2. **Multi-device interface** (lines 137-201): `deviceControls: Map<string, DeviceControlState>` keyed by `instanceId`. Each device has dynamically-shaped `tracks` and `sections` derived from its `DeviceDefinition`.

`applyPresetValues()` (line 655) follows a strict three-phase protocol:
1. **Phase 1**: Build new state (pure computation)
2. **Phase 2**: Update GUI immediately (synchronous `set()`)
3. **Phase 3**: Send MIDI CC to synth (async, with per-control error handling)

This guarantees GUI responsiveness even if MIDI transmission fails.

### Preset Store

**File**: `src/renderer/stores/preset.ts`
**Purpose**: Manages the preset library, selection, and A/B comparison.

Key state:
- `presets: PresetMetadata[]` -- Metadata only (loaded from main process)
- `currentPreset: Preset | null` -- Full preset data including mapping config
- `abSlotA` / `abSlotB` / `abActive` -- A/B comparison state
- `hasUnsavedChanges: boolean` -- Dirty flag for save prompts
- `presetDeviceTypes: Map<string, string>` -- Associates presets with device types

`loadPreset()` (line 151) orchestrates the full preset activation:
1. Load preset data from disk via IPC
2. Apply mapping configuration to the MIDI manager (`preset:apply`)
3. Apply control values and labels to the controls store
4. Respect `transmitButtonsOnLoad` setting

`saveCurrentPreset()` (line 288) captures the current GUI state into control values, updates the modified timestamp, and writes to disk **without** reloading or re-applying the mapping, preserving the current MIDI state.

### Settings Store

**File**: `src/renderer/stores/settings.ts`
**Purpose**: Mirrors `AppConfig` from the main process with optimistic updates.

On update failure, the store reverts to the previous config (line 75). Changes are immediately propagated to the main process via `config:update` IPC.

### Devices Store

**File**: `src/renderer/stores/devices.ts`
**Purpose**: Manages multi-device instance lifecycle.

Each `DeviceInstance` (lines 22-43) tracks:
```typescript
instanceId: string;        // Generated UUID (e.g., 'dev-<uuid>')
deviceType: string;        // Registry key ('nanokontrol2', 'akai-midimix')
nickname: string;          // User-assigned display name
connected: boolean;        // Hardware connection state
inputPort: string | null;  // Bound MIDI input port
outputPort: string | null; // Bound MIDI output port
virtualPort: string;       // Virtual output port name
currentPresetId: string | null;
tabOrder: number;          // UI tab ordering
```

The first device added automatically becomes the active device (line 166).

### Undo Store

**File**: `src/renderer/stores/undo.ts`
**Purpose**: Control value undo history (max 20 snapshots per device).

Uses deep-cloned `ControlValues` snapshots stored as a stack. Maintains both:
- Legacy single-stack interface (`pushSnapshot()`, `popSnapshot()`) using key `'__default__'`
- Per-device stacks via `deviceUndoStacks: Map<string, UndoSnapshot[]>`

---

## 9. Preset System

### Data Model

A preset consists of three parts:

```typescript
interface Preset {
  metadata: PresetMetadata;                        // ID, name, tags, timestamps
  mapping: MappingConfig | DeviceMappingConfig;    // CC/Note remapping rules
  controlValues?: ControlValues;                   // Stored knob/slider/button states
}
```

### Mapping Config Shapes

There are two mapping configuration shapes that reflect the system's evolution:

#### 1. MappingConfig (Legacy nK2 shape)

Defined in `src/shared/types.ts` (lines 127-132) and validated by `src/shared/schemas.ts` (lines 80-83):

```typescript
interface MappingConfig {
  tracks: TrackMapping[];     // Exactly 8 elements, fixed control names
  transport: TransportMapping; // 11 fixed transport controls
}
```

Each `TrackMapping` has exactly 5 controls: `knob`, `slider`, `solo`, `mute`, `rec`.

#### 2. DeviceMappingConfig (Multi-device shape)

Defined in `src/shared/types.ts` (lines 199-206):

```typescript
interface DeviceMappingConfig {
  deviceId: string;
  tracks: Record<string, Record<string, MappingEntry>>;   // Dynamic control names
  sections: Record<string, Record<string, MappingEntry>>; // Extra sections
}
```

Type guards for discrimination (`src/shared/schemas.ts`, lines 333-346):
- `isDeviceMappingConfig(mapping)` -- checks for `deviceId` string property
- `isMappingConfig(mapping)` -- checks if `tracks` is an array

### MappingEntry

The atomic unit of a mapping (`src/shared/types.ts`, lines 66-91):

```typescript
interface MappingEntry {
  inputCC: number;           // CC/Note number received from hardware (0-127)
  outputCC: number;          // CC/Note number sent to DAW (0-127)
  channel: number;           // MIDI channel (1-16, 1-indexed)
  behavior?: ButtonBehavior; // 'toggle' | 'momentary' (buttons only)
  label?: string;            // Human-readable control label
  minValue?: number;         // Output range minimum (0-127)
  maxValue?: number;         // Output range maximum (0-127)
  onValue?: number;          // Button ON value (default 127)
  offValue?: number;         // Button OFF value (default 0)
  messageType?: MidiMessageType; // 'cc' | 'note' (defaults to 'cc')
}
```

### Zod Validation

All preset data is validated at boundaries using Zod schemas (`src/shared/schemas.ts`):

| Schema | Validates | Location |
|--------|-----------|----------|
| `mappingEntrySchema` | Single control mapping | Line 37 |
| `trackMappingSchema` | Track with 5 controls | Line 56 |
| `transportMappingSchema` | 11 transport controls | Line 65 |
| `mappingConfigSchema` | Full nK2 mapping (8 tracks + transport) | Line 80 |
| `deviceMappingConfigSchema` | Generic device mapping | Line 86 |
| `presetSchema` | Complete preset (metadata + mapping + values) | Line 161 |
| `appConfigSchema` | Application configuration | Line 210 |

Validation entry points:
- `validatePreset(data)` -- throws `ZodError` on failure (line 299)
- `safeValidatePreset(data)` -- returns `{success, data/error}` (line 322)
- `validateAppConfig(data)` -- throws on failure (line 315)

### Factory Presets

10 factory presets are created by `createSamplePresets()` (`src/shared/schemas.ts`, lines 1322-1335):

| Preset | Group | Device | Description |
|--------|-------|--------|-------------|
| Default | Factory | nK2 | Identity mapping (CC in = CC out) |
| Synth Lead | Factory | nK2 | Filter, resonance, envelope, LFO on knobs |
| Drums | Factory | nK2 | Drum machine control layout |
| DAW Transport | Factory | nK2 | Mackie Control-style transport |
| DJ Mixer | Factory | nK2 | Deck A/B with EQ and crossfader |
| Nord Drum 2 | Nord Drum 2 | nK2 | Tone Spectra/Decay for 6 pads + FX |
| Default MidiMix | MidiMix Factory | MidiMix | Identity mapping for MidiMix |
| MidiMix Synth | MidiMix Factory | MidiMix | Synth control layout |
| MidiMix DJ | MidiMix Factory | MidiMix | DJ-style EQ per channel |
| MidiMix Nord Drum 2 | Nord Drum 2 | MidiMix | 32 parameters (3 knob rows) for ND2 |

The Nord Drum 2 MidiMix preset (`createNordDrum2MidiMixPreset()`, line 1113) uses `DeviceMappingConfig` to leverage all 3 knob rows (Spectra, Noise Filter, Timbre) that the MidiMix provides but the nK2 MappingConfig shape cannot represent.

### Preset Storage

Presets are stored as individual JSON files in `{app.getPath('userData')}/presets/`. The `FileManager` uses an in-memory metadata cache (`Map<string, PresetMetadata>`) for fast listing. The cache is lazily initialized and supports quick refresh (checking for new/deleted files without re-reading contents).

---

## 10. UI Component Hierarchy

### Layout Architecture

`App.tsx` (lines 106-148) manages a responsive layout:

```
+------------------------------------------------------------------+
| Header (connection status, tab nav, device selector)              |
+------------------------------------------------------------------+
| DeviceTabBar (when multi-device, shown on editor/librarian tabs)  |
+------------------------------------------------------------------+
| Main Content Area                                                 |
|                                                                   |
|  Narrow (< 1400px):  Tabbed (Editor OR Librarian)                |
|  Wide (>= 1400px):   Split (Editor LEFT | Librarian RIGHT)       |
|                                                                   |
|  Settings and Help always take full width                         |
+------------------------------------------------------------------+
```

### Tab Navigation

Four tabs controlled by `TabId` type:

| Tab | Component | Purpose |
|-----|-----------|---------|
| `editor` | `EditorView` or `DeviceEditorContainer` | Visual controller representation with clickable CC mapping |
| `librarian` | `LibrarianView` | Preset browser with search, filter, create, duplicate, delete |
| `settings` | `SettingsView` | App config: soft takeover, theme, LED mode, MIDI channel, etc. |
| `help` | `HelpView` | Documentation and help content |

### Editor Components

```
EditorView (src/renderer/components/Editor/EditorView.tsx)
  +-> Track x8 (Track.tsx)
  |     +-> Knob (Knob.tsx) -- SVG rotary control
  |     +-> Slider (Slider.tsx) -- Vertical fader
  |     +-> Button x3 (Button.tsx) -- Solo (yellow), Mute (green), Rec (red)
  |     +-> ControlPopover (ControlPopover.tsx) -- CC/channel/behavior editor
  +-> Transport (Transport.tsx)
        +-> Button x11 -- Transport controls

DeviceEditorContainer (DeviceEditorContainer.tsx)
  +-> EditorView (for nK2 devices, passing instanceId)
  +-> MidiMixEditor (for MidiMix devices)
        +-> MidiMixColumn x8 (MidiMixColumn.tsx) -- 3 knobs + fader + buttons
        +-> MidiMixMasterSection (MidiMixMasterSection.tsx) -- Master fader
```

### Librarian Components

```
LibrarianView (Librarian/LibrarianView.tsx)
  +-> QuickAccess (QuickAccess.tsx) -- 5 quick-load slots
  +-> PresetList (PresetList.tsx) -- Filterable, sortable preset list
  +-> NewPresetDialog (NewPresetDialog.tsx)
  +-> EditPresetDialog (EditPresetDialog.tsx)
  +-> DuplicatePresetDialog (DuplicatePresetDialog.tsx)
  +-> EditControlValuesDialog (EditControlValuesDialog.tsx)
```

### Theme System

Tailwind CSS colors are driven by CSS variables set at runtime (`App.tsx`, lines 56-76). Colors defined in `tailwind.config.js` (lines 11-19) reference these variables:

| CSS Variable | Tailwind Class | Default |
|-------------|---------------|---------|
| `--nk-dark` | `bg-nk-dark` | `#1a1a1a` |
| `--nk-darker` | `bg-nk-darker` | `#0f0f0f` |
| `--nk-light` | `bg-nk-light` | `#2a2a2a` |
| `--nk-border` | `border-nk-border` | `#3a3a3a` |
| `--nk-accent` | `text-nk-accent` | `#ff6600` |
| `--nk-solo` | `bg-nk-solo` | `#ffcc00` |
| `--nk-mute` | `bg-nk-mute` | `#00cc66` |
| `--nk-rec` | `bg-nk-rec` | `#ff3333` |

Users can customize these via `AppConfig.customThemeColors` and save multiple themes via `AppConfig.userThemes`. UI scale (`uiScale`, 50-200%) and font family/size are also runtime-configurable.

---

## 11. Target Synthesizer Support

The application includes a CC reference module pattern for target synthesizers. Currently, the only target is the **Nord Drum 2 Desktop**.

### Nord Drum 2 CC Reference

**File**: `src/shared/nord-drum-2-cc.ts`

This module defines verified MIDI CC assignments organized by synthesis section:

```typescript
NORD_DRUM_2_CC = {
  TONE: { SPECTRA: 30, PITCH_MSB: 31, WAVE: 46, DECAY: 50, ... },
  NOISE: { FILTER_FREQ: 14, RESONANCE: 17, DECAY: 21, ... },
  CLICK: { LEVEL: 56, TYPE: 57 },
  MIX: { LEVEL: 7, PAN: 10, BALANCE: 58 },
}

NORD_DRUM_2_FX_CC = {
  DIST_AMOUNT: 23, DIST_TYPE: 24,
  EQ_FREQ: 25, EQ_GAIN: 26,
  ECHO_FEEDBACK: 27, ECHO_AMOUNT: 28,
}

NORD_DRUM_2_CHANNELS = {
  PAD_1: 1, PAD_2: 2, PAD_3: 3,
  PAD_4: 4, PAD_5: 5, PAD_6: 6,
  GLOBAL_FX: 10,
}
```

**Architecture**: Each pad responds to the same CC numbers on its own MIDI channel (factory channels 1-6). Global effects are controlled on channel 10. This design enables the factory presets to map controller tracks to individual pads via per-channel routing.

### Adding a New Target Synthesizer

1. Create a CC reference module in `src/shared/` following the `nord-drum-2-cc.ts` pattern
2. Create factory presets in `src/shared/schemas.ts` that import and use those CC constants
3. Add the new presets to `createSamplePresets()`

---

## 12. Testing

### Framework

Tests use **Vitest** (`^1.0.0`) configured in `vite.config.ts` (lines 81-88). The test configuration aliases Node.js built-in modules to bypass vite-plugin-electron-renderer shims that are incompatible with ESM test runners.

### Test Files

Located in `tests/` (7 files, 356 tests total):

| File | Scope |
|------|-------|
| `config-parser.test.ts` | Legacy mapping file parser |
| `config-preset.test.ts` | Preset creation, validation, factory presets |
| `device-definitions.test.ts` | Device definition integrity, registry, auto-detection |
| `nord-drum-2.test.ts` | Nord Drum 2 CC reference constants |
| `regression.test.ts` | Bug fix regression tests |
| `stores.test.ts` | Zustand store behavior |
| `ui-components.test.ts` | Component rendering and interaction |

### Running Tests

```bash
npm test              # Single run
npm run test:watch    # Watch mode
```

---

## 13. Key Design Decisions

### 1. Only Mapped CCs Are Forwarded

The MidiManager never forwards raw hardware CCs to the virtual output (`midi-manager.ts`, line 292). This prevents hardware CC numbers from colliding with standard MIDI CCs. For example, nanoKONTROL2 sliders use CC 0-7, where CC 7 is MIDI Volume -- forwarding raw slider values would silence synthesizers.

### 2. Default Identity Mapping on Startup

The MidiManager creates a default identity mapping engine in its constructor (line 67). Without this, incoming CCs would be dropped since only mapped CCs are forwarded. The identity mapping ensures physical controls work immediately even before a preset is loaded.

### 3. Dual Interface Pattern for Multi-Device Migration

Stores (controls, connection, undo, preset) maintain both a legacy single-device interface and a new multi-device interface keyed by `instanceId`. This allows incremental migration of UI components from the legacy nK2-specific code path to the generic multi-device path without breaking existing functionality.

### 4. Three-Phase Preset Application

When loading a preset, the controls store separates state computation (Phase 1), GUI update (Phase 2), and MIDI transmission (Phase 3). Phase 2 is synchronous to guarantee immediate visual feedback. Phase 3 is async with per-control error handling so that a single MIDI failure does not prevent other controls from being transmitted.

### 5. ALSA Resource Conservation

Port enumeration creates temporary ALSA sequencer clients. To prevent exhaustion:
- Results are cached for 500ms (`midi-discovery.ts`, line 88)
- Connected devices use heartbeat checks instead of re-enumeration (`midi-handler.ts`, line 704)
- Failure state is cached to prevent repeated initialization attempts

### 6. Composite Lookup Keys in MappingEngine

The mapping engine uses composite keys (`"cc:42"`, `"note:25"`) instead of plain numeric keys to prevent collisions between CC and Note messages that share the same MIDI number. This is essential for the Akai MidiMix, which uses CC for knobs/faders and Note for buttons.

### 7. Backward-Compatible Preset Schema

The preset schema accepts a union of `MappingConfig | DeviceMappingConfig` (`schemas.ts`, line 163). Existing nK2 presets (array-based `tracks`) continue to work alongside MidiMix presets (record-based `tracks` with `deviceId`). Type guards differentiate the two shapes at runtime.
