# Development Guide -- nkEditor3

This document covers everything a developer needs to set up, build, test, debug, and extend the nkEditor3 MIDI controller editor. It assumes familiarity with TypeScript and React but not with this codebase.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Setup and Installation](#setup-and-installation)
3. [Common Commands](#common-commands)
4. [Project Structure](#project-structure)
5. [Testing](#testing)
6. [Debugging](#debugging)
7. [Adding New Devices](#adding-new-devices)
8. [Adding New Presets](#adding-new-presets)
9. [Adding New Target Synths](#adding-new-target-synths)
10. [Build Configuration](#build-configuration)
11. [Known Gotchas](#known-gotchas)

---

## Prerequisites

### Required

| Dependency | Minimum Version | Purpose |
|---|---|---|
| Node.js | >= 18.0.0 | Runtime and build tooling (enforced in `package.json` `engines` field) |
| npm | Ships with Node.js | Package management |
| ALSA development libraries | (Linux only) | Required by `easymidi` (via RtMidi) for MIDI port access |

### Installing ALSA Libraries (Linux Only)

The `easymidi` package depends on native RtMidi bindings that require ALSA headers at compile time. Without these, `npm install` will fail during native module compilation.

```bash
# Arch / Manjaro
sudo pacman -S alsa-lib

# Debian / Ubuntu
sudo apt install libasound2-dev

# Fedora / RHEL
sudo dnf install alsa-lib-devel
```

### Optional

- **A supported MIDI controller** -- Korg nanoKONTROL2 or Akai MIDI Mix. The application runs without hardware, but MIDI I/O features require a physical device.
- **A MIDI monitor** -- Tools like `amidi`, `aseqdump`, or `MIDI Monitor` (macOS) are useful for verifying that the virtual MIDI output port (`nkEditor3 Out`) is sending correctly remapped messages.

---

## Setup and Installation

```bash
# Clone or navigate to the project directory
cd nkEditor3

# Install all dependencies, including native module compilation for easymidi
npm install
```

The `npm install` step compiles the `easymidi` native addon via `node-gyp`. If you see compilation errors, verify that ALSA development libraries are installed (Linux) and that your Node.js version is >= 18.

There is no additional setup step. The project uses no external databases, environment variable files, or Docker containers.

---

## Common Commands

All commands are defined in `package.json` under `scripts`:

| Command | What It Does |
|---|---|
| `npm run dev` | Starts the Vite dev server and launches Electron with hot-reload. The renderer loads from `http://localhost:5173`. DevTools open automatically. |
| `npm run build` | Full production pipeline: TypeScript compilation (`tsc`), Vite frontend build, and `electron-builder` packaging. |
| `npm run build:vite` | Builds only the Vite frontend into `dist/` without running electron-builder. Useful for testing the production bundle without packaging. |
| `npm run preview` | Serves the production Vite build locally for inspection. |
| `npm run typecheck` | Runs `tsc --noEmit` to check for type errors without producing output files. |
| `npm test` | Runs the full test suite with `vitest run` (single pass, exits on completion). |
| `npm run test:watch` | Runs Vitest in watch mode, re-running affected tests on file changes. |

### Build Outputs

| Directory | Contents |
|---|---|
| `dist/` | Bundled renderer (React frontend) |
| `dist-electron/` | Compiled Electron main process (`main.cjs`, `preload.cjs`) |
| `release/` | Packaged application (AppImage, .deb, .dmg, or NSIS installer) |

---

## Project Structure

```
nkEditor3/
  index.html                    # Electron renderer entry HTML
  package.json                  # Dependencies, scripts, electron-builder config
  vite.config.ts                # Vite + Electron plugin + Vitest config
  tsconfig.json                 # TypeScript compiler options and path aliases
  tailwind.config.js            # Tailwind CSS theme with nK2-inspired color tokens
  postcss.config.js             # PostCSS with Tailwind and Autoprefixer plugins

  src/
    config-parser.ts            # Text-based mapping file parser (canonical impl)

    shared/                     # Code shared between main and renderer processes
      constants.ts              # nK2 hardware CC tables, LED mappings, utility fns
      ipc-protocol.ts           # IPC channel names and request/response types
      schemas.ts                # Zod validation schemas, factory preset generators
      types.ts                  # Core TypeScript interfaces (MappingEntry, etc.)
      nord-drum-2-cc.ts         # Nord Drum 2 MIDI CC constants and channel map
      devices/                  # Device abstraction layer
        device-definition.ts    # DeviceDefinition interface and control types
        registry.ts             # Central device registry (lookup, auto-detect)
        nanokontrol2.ts         # Korg nanoKONTROL2 hardware definition
        akai-midimix.ts         # Akai MIDI Mix hardware definition
        index.ts                # Barrel re-export for the devices module

    main/                       # Electron main process
      main.ts                   # App lifecycle, window creation, IPC setup
      preload.ts                # Context bridge exposing IPC API to renderer
      ipc-handlers.ts           # IPC handler registration and request routing
      services/
        midi-manager.ts         # High-level MIDI orchestrator (connect, remap)
        midi-handler.ts         # Low-level MIDI I/O via easymidi
        midi-discovery.ts       # Port scanning and device auto-detection
        mapping-engine.ts       # CC/Note remapping logic, button state tracking
        led-controller.ts       # Hardware LED state management
        config-manager.ts       # App config persistence (JSON in userData)
        file-manager.ts         # Preset file I/O (JSON in userData/presets)
        config-parser.ts        # Re-export from root config-parser
      utils/
        renderer-bridge.ts      # Sends events from main process to renderer

    renderer/                   # React frontend (renderer process)
      main.tsx                  # React DOM entry point
      App.tsx                   # Root component, layout, tab navigation
      styles/
        globals.css             # Tailwind imports, CSS variables, component styles
      types/
        electron.d.ts           # Type declarations for window.electronAPI
      stores/                   # Zustand state management
        devices.ts              # Device instance store (add/remove/switch)
        connection.ts           # MIDI connection state
        controls.ts             # Per-device control values (knobs, sliders, buttons)
        preset.ts               # Preset library and A/B comparison
        settings.ts             # App configuration state
        undo.ts                 # Per-device undo stack
      components/
        Header.tsx              # Top bar with connection status and tab nav
        DeviceSelector.tsx      # Device dropdown for adding controllers
        DeviceTabBar.tsx        # Tabs for switching between devices
        AddDeviceDialog.tsx     # Dialog for adding a new device instance
        Editor/
          DeviceEditorContainer.tsx  # Routes to correct editor by device type
          EditorView.tsx        # nK2 editor: track grid and transport
          Track.tsx             # Single track strip (knob, slider, 3 buttons)
          Knob.tsx              # Rotary knob control
          Slider.tsx            # Linear fader control
          Button.tsx            # Solo/Mute/Rec button
          Transport.tsx         # Transport button bar
          ControlPopover.tsx    # Inline CC mapping editor popover
          MidiMix/
            MidiMixEditor.tsx       # MidiMix editor layout
            MidiMixColumn.tsx       # Single MidiMix track column
            MidiMixMasterSection.tsx # Master fader and global buttons
        Librarian/
          LibrarianView.tsx     # Preset library browser
          PresetList.tsx        # Scrollable preset list with filtering
          QuickAccess.tsx       # Quick access slot bar
          NewPresetDialog.tsx   # Create new preset dialog
          EditPresetDialog.tsx  # Edit preset metadata dialog
          DuplicatePresetDialog.tsx  # Duplicate preset dialog
          EditControlValuesDialog.tsx # Bulk-edit control values
        Settings/
          SettingsView.tsx      # Configuration panel
        Help/
          HelpView.tsx          # Built-in docs and troubleshooting

  tests/                        # All test files (7 files, 356 tests)
  mappings/
    default.txt                 # Example text-based mapping file
  presets/                      # Local preset storage placeholder
  assets/                       # Application icons/images
```

### Architecture Overview

The application follows Electron's process model:

- **Main process** (`src/main/`): Manages MIDI I/O via `easymidi`, persists configuration and presets to disk, and handles the application lifecycle. Communicates with the renderer via typed IPC channels defined in `src/shared/ipc-protocol.ts`.
- **Renderer process** (`src/renderer/`): React 18 UI with Zustand stores for state management. Accesses MIDI operations through `window.electronAPI`, which is provided by the preload script's context bridge.
- **Shared code** (`src/shared/`): Types, Zod schemas, device definitions, and constants that both processes import. This code must remain free of process-specific dependencies (no `electron`, no `react`, no `easymidi`).

---

## Testing

### Overview

The test suite uses **Vitest** (v1.x) configured directly within `vite.config.ts` under the `test` key. There are 7 test files containing 356 tests that run in approximately 1.4 seconds.

### Running Tests

```bash
# Run all tests once (CI mode)
npm test

# Run tests in watch mode (re-runs on file changes)
npm run test:watch

# Run a specific test file
npx vitest run tests/device-definitions.test.ts

# Run tests matching a pattern
npx vitest run -t "nanoKONTROL2 definition"

# Run with verbose reporter (see every test name)
npx vitest run --reporter=verbose
```

### Test Files

| File | Tests | Coverage |
|---|---|---|
| `tests/device-definitions.test.ts` | Device definition structure, CC number validation, port pattern matching, registry lookup, cross-reference against legacy `HARDWARE_CC` and `LED_CC` constants |
| `tests/config-parser.test.ts` | Text-based mapping file parsing, error handling, `ParseError` class, `default.txt` fixture validation |
| `tests/config-preset.test.ts` | Device-aware config parsing (`[device akai-midimix]` header), `note:N` syntax, `createDefaultMappingForDevice()`, `createSamplePresets()`, `MappingConfig` vs `DeviceMappingConfig` type guards |
| `tests/stores.test.ts` | Multi-device Zustand stores: devices store (add/remove/switch), controls store (per-device state), undo store (per-device stacks), preset store (device-aware filtering), connection store (port-to-device mapping), legacy backward compatibility |
| `tests/ui-components.test.ts` | UI component routing logic: DeviceSelector integration with store, DeviceTabBar tab state, DeviceEditorContainer device-type routing, MidiMixEditor column/master structure |
| `tests/regression.test.ts` | Backward compatibility safety net: all pre-existing nK2 constants, types, and public APIs remain accessible and correct after multi-device refactoring |
| `tests/nord-drum-2.test.ts` | Nord Drum 2 MIDI CC reference validation: section structure, CC value ranges, duplicate detection within sections, specific CC-to-parameter mapping verification against hardware spec |

### Test Architecture

Tests operate at the **logic and store level** without a DOM renderer. Zustand stores are designed to function without the Electron API, enabling headless testing. Each test file that uses Zustand stores resets them in a `beforeEach` hook to prevent state leaks between tests (Zustand stores are singletons).

Example store reset pattern:

```typescript
function resetAllStores(): void {
  useDevicesStore.setState({
    devices: new Map(),
    activeDeviceId: null,
    deviceOrder: [],
  });

  useControlsStore.setState({
    deviceControls: new Map(),
    tracks: Array(8).fill(null).map(() => ({
      knob: { value: 0 },
      slider: { value: 0 },
      solo: { active: false },
      mute: { active: false },
      rec: { active: false },
    })),
    // ... transport state
  });
}
```

### Vitest Configuration and the Built-in Module Alias Workaround

The Vitest configuration lives inside `vite.config.ts` (not a separate `vitest.config.ts`). It contains a critical workaround in the `test.alias` section:

```typescript
// vite.config.ts
test: {
  alias: Object.fromEntries(
    builtinModules.flatMap((mod) => [
      [mod, mod],
      [`node:${mod}`, `node:${mod}`],
    ]),
  ),
},
```

**Why this exists**: The `vite-plugin-electron-renderer` plugin shims Node.js built-in modules (`fs`, `path`, etc.) so they work inside the Electron renderer. However, these shims are ESM-incompatible and break Vitest. The alias block forces Vitest to resolve built-in modules to their real Node.js implementations instead of the `.vite-electron-renderer` shims.

Without this workaround, tests that import from modules using `fs`, `path`, or other Node built-ins will fail with cryptic resolution errors.

---

## Debugging

### DevTools

In development mode (`npm run dev`), Electron DevTools open automatically. This is controlled in `src/main/main.ts`:

```typescript
if (isDev) {
  mainWindow.loadURL('http://localhost:5173');
  mainWindow.webContents.openDevTools();
}
```

The renderer loads from the Vite dev server at `http://localhost:5173`, so React Fast Refresh works out of the box.

### Logging Prefixes

Console output uses prefix tags to identify which process or module a message originates from:

| Prefix | Source |
|---|---|
| `[main]` | Electron main process lifecycle (`src/main/main.ts`) |
| `[devices-store]` | Device instance store operations (`src/renderer/stores/devices.ts`) |

Watch for these prefixes in the terminal (main process) and in DevTools console (renderer process) when diagnosing issues.

### MIDI Port Inspection

On Linux, you can inspect available MIDI ports without the application:

```bash
# List all MIDI ports (ALSA sequencer)
aconnect -l

# Monitor raw MIDI messages from a port
aseqdump -p "nanoKONTROL2:0"

# List all raw MIDI ports
amidi -l
```

The application's virtual output port is named `nkEditor3 Out`. To verify it is active:

```bash
# Check if the virtual port appears while the app is running
aconnect -l | grep nkEditor3
```

### Inspecting Preset Files

Presets are stored as individual JSON files in the Electron `userData` directory:

```bash
# Linux default path
ls ~/.config/nkeditor3/presets/

# App config
cat ~/.config/nkeditor3/config.json
```

Each preset file contains `metadata`, `mapping`, and optionally `controlValues` sections. You can validate a preset file against the Zod schema from the Node REPL if needed.

---

## Adding New Devices

This section walks through adding support for a new MIDI controller. The example adds a hypothetical "Novation LaunchControl XL".

### Step 1: Create the Device Definition

Create a new file at `src/shared/devices/launchcontrol-xl.ts` implementing the `DeviceDefinition` interface (defined in `src/shared/devices/device-definition.ts`).

Study the existing definitions for reference:
- `src/shared/devices/nanokontrol2.ts` -- CC-only device, 8 tracks, 1 knob + 1 slider + 3 buttons per track, transport section
- `src/shared/devices/akai-midimix.ts` -- CC + Note device, 8 tracks, 3 knobs + 1 fader + 3 buttons per track, master section

The definition must declare:

```typescript
import type { DeviceDefinition } from './device-definition.js';

export const LAUNCHCONTROL_XL_DEFINITION: DeviceDefinition = {
  // Unique ID used throughout the codebase to identify this device
  id: 'launchcontrol-xl',

  // Human-readable name shown in the UI
  name: 'LaunchControl XL',
  manufacturer: 'Novation',

  // Regex patterns matched against OS-reported MIDI port names for auto-detection
  portPatterns: [/^Launch Control XL[:\s]/i],

  // Number of channel strips on the physical hardware
  trackCount: 8,

  // Per-track control layouts (length must equal trackCount)
  trackLayouts: [/* ... build with a helper function ... */],

  // Control sections outside per-track layout (transport, master, etc.)
  extraSections: [/* ... */],

  // How the hardware expects to receive LED state updates
  ledProtocol: {
    messageType: 'note',  // or 'cc'
    channel: 1,
    onValue: 127,
    offValue: 0,
  },

  // All controllable LEDs on the device
  leds: [/* ... */],

  // Optional: custom virtual port name (defaults to device-level handling)
  virtualPortName: 'nkEditor3 Out',
};
```

Key types to understand:
- `ContinuousControlDef` -- knobs, sliders, faders (kind: `'continuous'`, messageType: `'cc'`)
- `ButtonControlDef` -- buttons (kind: `'button'`, messageType: `'cc'` or `'note'`)
- `TrackLayout` -- groups `continuousControls` and `buttonControls` for one track
- `ExtraControlsSection` -- named group of controls outside the track grid

### Step 2: Register the Device

Edit `src/shared/devices/registry.ts` to import and register the new definition:

```typescript
import { LAUNCHCONTROL_XL_DEFINITION } from './launchcontrol-xl.js';

const DEVICE_REGISTRY: Map<string, DeviceDefinition> = new Map([
  [NANOKONTROL2_DEFINITION.id, NANOKONTROL2_DEFINITION],
  [AKAI_MIDIMIX_DEFINITION.id, AKAI_MIDIMIX_DEFINITION],
  [LAUNCHCONTROL_XL_DEFINITION.id, LAUNCHCONTROL_XL_DEFINITION],  // Add here
]);
```

Also add the re-export in the barrel module `src/shared/devices/index.ts`:

```typescript
export { LAUNCHCONTROL_XL_DEFINITION } from './launchcontrol-xl.js';
```

### Step 3: Create a Device-Specific Editor (if needed)

If the new device has a significantly different control layout from existing devices, create a new editor component. Use the MidiMix editor as a template:

```
src/renderer/components/Editor/LaunchControlXL/
  LaunchControlXLEditor.tsx
  LaunchControlXLColumn.tsx
  (any additional layout components)
```

Then add a routing case in `src/renderer/components/Editor/DeviceEditorContainer.tsx`:

```typescript
switch (activeDevice.deviceType) {
  case 'nanokontrol2':
    return <EditorView instanceId={activeDeviceId} />;
  case 'akai-midimix':
    return <MidiMixEditor instanceId={activeDeviceId} />;
  case 'launchcontrol-xl':
    return <LaunchControlXLEditor instanceId={activeDeviceId} />;
  // ...
}
```

### Step 4: Add Factory Presets

See [Adding New Presets](#adding-new-presets) below.

### Step 5: Add Tests

Create or extend test files in the `tests/` directory:

- Add structural validation tests for the new device definition (see `tests/device-definitions.test.ts` for the pattern)
- Add port pattern matching tests to verify auto-detection
- If new store logic was added, extend `tests/stores.test.ts`

### Step 6: Update the Config Parser (if needed)

If the new device uses control names not already supported by the text-based config parser (`src/config-parser.ts`), add parsing rules for those control types. The parser supports a `[device <id>]` header for multi-device configs.

---

## Adding New Presets

Factory presets are created programmatically in `src/shared/schemas.ts` and registered in the `createSamplePresets()` function.

### Current Factory Presets (10 total)

**nanoKONTROL2 presets (6):**
| Preset | Description |
|---|---|
| Default | Identity mapping -- CCs pass through unchanged |
| Synth Lead | Knobs mapped to filter/resonance/envelope, sliders to oscillator levels |
| Drums | Tuning/decay on knobs, individual drum levels on sliders |
| DAW Transport | Mackie Control-style transport, channel mixer layout |
| DJ Mixer | EQ on knobs, deck faders, cue/sync buttons |
| Nord Drum 2 | Per-pad Spectra/Decay on tracks 1-6, Global FX on tracks 7-8 |

**Akai MidiMix presets (4):**
| Preset | Description |
|---|---|
| Default MidiMix | Identity mapping with correct CC/Note message types |
| MidiMix Synth | Filter/mod on knobs, oscillator levels on faders |
| MidiMix DJ | EQ High on knobs, channel volumes on faders |
| MidiMix Nord Drum 2 | 32 parameters: Spectra, Noise Filter, Timbre, Level per pad |

### Step-by-Step: Creating a New Factory Preset

1. **Define a factory function** in `src/shared/schemas.ts`:

```typescript
function createMyNewPreset(): Preset {
  // Start from an identity mapping for the target device
  const preset = createDefaultPreset(
    'My Preset Name',           // name (max 100 chars)
    'Description of the preset', // description (max 500 chars)
    'Factory'                    // group name (max 50 chars)
  );

  // Tag it for filtering and identification
  preset.metadata.tags = ['factory', 'my-tag', 'another-tag'];

  // Modify the mapping: remap knobs, sliders, buttons
  const trackIndices: TrackIndex[] = [0, 1, 2, 3, 4, 5, 6, 7];
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.knob.outputCC = 74;       // Remap knob output
    track.knob.channel = i + 1;     // Set MIDI channel (1-indexed)
    track.knob.label = 'Filter';    // Set display label
  }

  return preset;
}
```

2. **Register it** in `createSamplePresets()` at the bottom of `src/shared/schemas.ts`:

```typescript
export function createSamplePresets(): Preset[] {
  return [
    // ... existing presets ...
    createMyNewPreset(),  // Add here
  ];
}
```

3. **For non-nK2 devices**, use `DeviceMappingConfig` instead of `MappingConfig`. See `createNordDrum2MidiMixPreset()` in `src/shared/schemas.ts` for the pattern -- it builds a `tracks` record with string keys and uses the `deviceMappingConfigSchema` shape.

### Preset Data Structure

Every preset contains:

```typescript
{
  metadata: {
    id: string,          // Auto-generated unique ID
    name: string,        // Display name
    description?: string,
    author?: string,
    group?: string,      // Folder for organization
    createdAt: string,   // ISO 8601 datetime
    modifiedAt: string,
    tags: string[],      // Max 10 tags, 30 chars each
    favorite: boolean,
  },
  mapping: MappingConfig | DeviceMappingConfig,  // The CC/Note remapping table
  controlValues?: ControlValues,                  // Saved knob/slider/button state
}
```

The `mapping` field uses a **union type**: `MappingConfig` for nK2 presets (array-based tracks with fixed 5-control structure) and `DeviceMappingConfig` for other devices (record-based tracks with arbitrary control names). Use the `isMappingConfig()` and `isDeviceMappingConfig()` type guards from `src/shared/schemas.ts` to distinguish them at runtime.

---

## Adding New Target Synths

A "target synth" is a synthesizer whose MIDI CC parameter map is codified as constants, enabling presets that map controller knobs/sliders to specific synth parameters by name.

### Current Target Synths

| Synth | File | Scope |
|---|---|---|
| Nord Drum 2 Desktop | `src/shared/nord-drum-2-cc.ts` | Per-pad synthesis (Tone, Noise, Click, Mix) + Global FX (Distortion, EQ, Echo) |

### Step-by-Step: Adding a New Target Synth

1. **Create a CC reference module** at `src/shared/<synth-name>-cc.ts`. Follow the structure of `src/shared/nord-drum-2-cc.ts`:

```typescript
/**
 * <Synth Name> MIDI CC Reference Constants
 *
 * Document the source (manufacturer MIDI implementation chart),
 * the channel architecture, and any firmware version notes.
 */

export const MY_SYNTH_CC = {
  FILTER: {
    CUTOFF: 74,
    RESONANCE: 71,
    // ... more parameters
  },
  OSCILLATOR: {
    WAVE: 46,
    // ...
  },
} as const;

export const MY_SYNTH_CHANNELS = {
  VOICE_1: 1,
  VOICE_2: 2,
  // ...
} as const;
```

2. **Create factory presets** that use the CC constants. Import them in `src/shared/schemas.ts`:

```typescript
import { MY_SYNTH_CC, MY_SYNTH_CHANNELS } from './my-synth-cc.js';
```

Then build preset factory functions that map controller controls to synth parameters (see `createNordDrum2NK2Preset()` and `createNordDrum2MidiMixPreset()` for full examples).

3. **Add validation tests** in `tests/` to verify:
   - All CC values are in the valid range (0-127)
   - No duplicate CCs within a section (accidental copy-paste errors)
   - Specific CC values match the manufacturer's MIDI implementation chart
   - Channel assignments are in valid range (1-16)

See `tests/nord-drum-2.test.ts` for the established test pattern.

---

## Build Configuration

### vite.config.ts

The Vite configuration (`vite.config.ts`) handles three concerns:

**1. React plugin** -- `@vitejs/plugin-react` for JSX transform and Fast Refresh.

**2. Electron plugin** -- `vite-plugin-electron` defines two entry points:
- `src/main/main.ts` -- The Electron main process. Built to `dist-electron/main.cjs` with CJS format (Electron requires CJS for the main process). `easymidi` is externalized from the bundle since it is a native addon.
- `src/main/preload.ts` -- The preload script. Built to `dist-electron/preload.cjs` with CJS format.

**3. Electron renderer plugin** -- `vite-plugin-electron-renderer` allows the renderer process to import Node.js built-in modules (needed because the preload script bridges Node APIs).

**Path aliases** are defined in both the main Vite `resolve.alias` and each Electron entry's `vite.resolve.alias`:

| Alias | Resolves To |
|---|---|
| `@` | `src/` |
| `@shared` | `src/shared/` |
| `@renderer` | `src/renderer/` |
| `@main` | `src/main/` |

**Vitest config** is embedded in the `test` key (see [Testing](#testing) for the built-in module alias workaround).

### tsconfig.json

Key compiler options:

| Option | Value | Rationale |
|---|---|---|
| `target` | `ES2022` | Modern JavaScript features, matches Electron's V8 |
| `module` | `ESNext` | ESM source code, bundled by Vite |
| `moduleResolution` | `bundler` | Allows bare specifier imports that Vite resolves |
| `strict` | `true` | Full strict mode |
| `noUnusedLocals` | `true` | Catches dead code |
| `noUnusedParameters` | `true` | Catches unused function parameters |
| `noImplicitReturns` | `true` | All code paths must return |
| `noFallthroughCasesInSwitch` | `true` | Prevents switch fallthrough bugs |
| `noUncheckedIndexedAccess` | `true` | Array/object index access returns `T \| undefined` |
| `jsx` | `react-jsx` | React 17+ automatic JSX transform |

**Path aliases** mirror those in `vite.config.ts`:

```json
{
  "paths": {
    "@/*": ["src/*"],
    "@shared/*": ["src/shared/*"],
    "@renderer/*": ["src/renderer/*"],
    "@main/*": ["src/main/*"]
  }
}
```

### tailwind.config.js

- **Content paths**: Scans `index.html` and `src/renderer/**/*.{js,ts,jsx,tsx}` for class usage.
- **Dark mode**: Uses the `class` strategy (applied via a CSS class, not OS preference).
- **Custom colors**: nanoKONTROL2-inspired tokens (`nk-dark`, `nk-accent`, `nk-solo`, `nk-mute`, `nk-rec`) use CSS custom properties for runtime theming support.
- **Fonts**: `Inter` primary, `JetBrains Mono` for monospace.

### electron-builder

Configured directly in `package.json` under the `build` key:

```json
{
  "build": {
    "appId": "com.nkeditor3.app",
    "productName": "nkEditor3",
    "directories": { "output": "release" },
    "files": ["dist/**/*", "dist-electron/**/*"],
    "linux": { "target": ["AppImage", "deb"], "category": "Audio" },
    "mac": { "target": ["dmg"], "category": "public.app-category.music" },
    "win": { "target": ["nsis"] }
  }
}
```

### PostCSS

`postcss.config.js` enables two plugins: `tailwindcss` and `autoprefixer`. This is an ESM module (consistent with the project's `"type": "module"` in `package.json`).

---

## Known Gotchas

### 1. ALSA Library Missing on Linux

**Symptom**: `npm install` fails with compilation errors mentioning `RtMidi`, `asound.h`, or `libasound`.

**Fix**: Install the ALSA development package for your distribution (see [Prerequisites](#prerequisites)).

### 2. CJS vs ESM Tension

The project uses `"type": "module"` in `package.json`, making all `.js`/`.ts` files ESM by default. However, **Electron's main process requires CJS** for the entry point. This is handled by:

- Vite builds `main.ts` and `preload.ts` with `format: 'cjs'` and `entryFileNames: '[name].cjs'`
- The `package.json` `"main"` field points to `dist-electron/main.cjs`
- The preload script is loaded as `preload.cjs` in the `BrowserWindow` config

If you see `require is not defined` or `module is not defined` errors in the main process, verify that the build output format remains CJS in `vite.config.ts`.

### 3. MappingConfig Duality (MappingConfig vs DeviceMappingConfig)

The codebase has two mapping shapes:

- **`MappingConfig`** (`src/shared/types.ts`): The original nK2-specific shape with a fixed `tracks: TrackMapping[]` array of 8 tracks, each with exactly `knob`, `slider`, `solo`, `mute`, `rec` keys, plus a `transport` section.
- **`DeviceMappingConfig`** (`src/shared/types.ts`): The generic shape introduced for multi-device support, with `tracks: Record<string, Record<string, MappingEntry>>` and `sections: Record<string, Record<string, MappingEntry>>` keyed by arbitrary names.

Preset `mapping` fields are a **union** of these two types. Always use the type guard functions before accessing shape-specific properties:

```typescript
import { isMappingConfig, isDeviceMappingConfig } from '@shared/schemas';

if (isMappingConfig(preset.mapping)) {
  // Safe to access preset.mapping.tracks[0].knob
  // Safe to access preset.mapping.transport.play
}

if (isDeviceMappingConfig(preset.mapping)) {
  // Safe to access preset.mapping.deviceId
  // Safe to access preset.mapping.tracks['1']['knob1']
  // Safe to access preset.mapping.sections['master']['master_fader']
}
```

**Why both exist**: `MappingConfig` was the original format and is deeply embedded in the nK2 code paths, the config parser, and the Zod validation schemas. Changing it would break backward compatibility with saved preset files. `DeviceMappingConfig` was added to support devices with different control layouts without forcing them into the nK2 mold.

### 4. Path Aliases Must Be Defined in Three Places

Any new path alias must be added to:

1. `vite.config.ts` -- root `resolve.alias` (for the renderer)
2. `vite.config.ts` -- each Electron entry's `vite.resolve.alias` (for main/preload)
3. `tsconfig.json` -- `compilerOptions.paths` (for TypeScript resolution and IDE support)

Forgetting one of these will cause confusing errors that only appear in specific build targets.

### 5. easymidi Is Externalized in the Main Process Bundle

In `vite.config.ts`, `easymidi` is listed in `rollupOptions.external` for the main process build:

```typescript
rollupOptions: {
  external: ['easymidi'],
}
```

This is necessary because `easymidi` is a native Node.js addon (`.node` binary). Vite/Rollup cannot bundle native modules -- they must be resolved at runtime from `node_modules`. If you add another native addon dependency, externalize it the same way.

### 6. Zustand Store State Leaks in Tests

Zustand stores are JavaScript module-level singletons. State set in one test persists into the next unless explicitly reset. Every test file that uses stores must reset them in `beforeEach`:

```typescript
beforeEach(() => {
  resetAllStores();
});
```

See `tests/stores.test.ts` and `tests/ui-components.test.ts` for the full reset pattern.

### 7. noUncheckedIndexedAccess and Array Access

TypeScript's `noUncheckedIndexedAccess` is enabled, which means accessing `array[i]` returns `T | undefined` even when you know the index is valid. The codebase handles this with:

- Non-null assertions (`array[i]!`) when the index is guaranteed valid by construction
- The `getTrack()` helper in `src/shared/schemas.ts` that asserts a track exists and provides a clear error if the preset is malformed
- Explicit type narrowing with `as const` tuples for hardware CC tables

### 8. Sandbox Disabled for the Main Window

The `BrowserWindow` is created with `sandbox: false`:

```typescript
webPreferences: {
  sandbox: false, // Required for easymidi native module
}
```

This is required because the preload script needs access to native Node.js modules via the context bridge. The application compensates with `contextIsolation: true` and `nodeIntegration: false`, so the renderer process itself does not have direct Node.js access.

### 9. Port Patterns Are Order-Sensitive in the Registry

The `identifyDevice()` function in `src/shared/devices/registry.ts` returns the **first** device whose port pattern matches. If two devices could match the same port name, the device registered first in the `DEVICE_REGISTRY` map wins. Keep patterns specific enough to avoid ambiguity.

### 10. The Config Parser Re-Export Pattern

The canonical config parser implementation lives at `src/config-parser.ts` (root of `src/`). The main process services import it via a re-export module at `src/main/services/config-parser.ts`:

```typescript
export { parseMapping, parseMappingOrThrow, ParseError } from '../../config-parser.js';
```

This exists to maintain clean import paths in the service layer without duplicating the parser code. If you modify the parser, you only need to edit `src/config-parser.ts`.
