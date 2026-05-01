# CLAUDE.md -- nkEditor3

## Project Summary

nkEditor3 is a desktop MIDI CC/Note remapping application built with Electron 28, React 18, and TypeScript 5. It intercepts MIDI messages from hardware controllers (Korg nanoKONTROL2, Akai MidiMix), remaps them per user-defined presets, and outputs transformed messages through a virtual MIDI port. The target synthesizer is the Nord Drum 2 Desktop (per-pad channels 1-6, global FX channel 10).

## Quick Reference

| Task | Command |
|---|---|
| Dev server | `npm run dev` |
| Build | `npm run build` |
| Build (Vite only) | `npm run build:vite` |
| Type check | `npm run typecheck` |
| Run tests | `npm test` |
| Watch tests | `npm run test:watch` |

**Path aliases** (tsconfig.json + vite.config.ts):

| Alias | Resolves to |
|---|---|
| `@/*` | `src/*` |
| `@shared/*` | `src/shared/*` |
| `@renderer/*` | `src/renderer/*` |
| `@main/*` | `src/main/*` |

**Strict tsconfig flags**: `noUnusedLocals`, `noUnusedParameters`, `noImplicitReturns`, `noFallthroughCasesInSwitch`, `noUncheckedIndexedAccess`.

## Architecture at a Glance

```
Electron Main Process          Preload (contextBridge)        Renderer (React)
========================       ====================           =================
MidiManager                    electronAPI                    Zustand Stores
  MidiHandler (easymidi)  -->  IPC_EVENTS (main->renderer)    useControlsStore
  MappingEngine           <--  IPC_INVOKE (renderer->main)    usePresetStore
  LedController                                               useConnectionStore
ConfigManager                                                 useDevicesStore
FileManager                                                   useSettingsStore
                                                              useUndoStore
```

### Three layers

1. **Main process** (`src/main/`): MIDI I/O via easymidi, CC remapping, LED control, preset file storage, app config. Entry point: `src/main/main.ts`. IPC handled in `src/main/ipc-handlers.ts`. Services in `src/main/services/`.
2. **Renderer** (`src/renderer/`): React UI with Zustand state management. Views: Editor, Librarian, Settings, Help. Components mirror physical controller layouts. Entry: `src/renderer/main.tsx` -> `App.tsx`.
3. **Shared** (`src/shared/`): Types, Zod schemas, constants, IPC protocol definitions, device definitions. Imported by both main and renderer.

### MIDI Flow

```
Hardware Controller
  -> easymidi input (MidiHandler)
  -> MappingEngine.remap(inputCC) -> {outputCC, channel}
  -> Virtual MIDI output ("nkEditor3 Out")
  -> IPC_EVENTS.MIDI_CC to renderer for UI update
```

LED feedback flows in reverse: main process sends CC/Note to hardware output port.

### Two Mapping Config Shapes

| Shape | Use case | Structure |
|---|---|---|
| `MappingConfig` | nanoKONTROL2 (legacy) | `tracks: TrackMapping[]` (length 8), `transport: TransportMapping` -- fixed schema |
| `DeviceMappingConfig` | Multi-device (MidiMix, etc.) | `deviceId: string`, `tracks: Record<string, Record<string, MappingEntry>>`, `sections: Record<string, Record<string, MappingEntry>>` -- dynamic schema |

Type guards: `isMappingConfig()` (tracks is array), `isDeviceMappingConfig()` (has deviceId string) -- both in `src/shared/schemas.ts`.

## Key Concepts

### Device Abstraction

- **`DeviceDefinition`** (`src/shared/devices/device-definition.ts`): Pure data declaration of a controller's physical layout, MIDI protocol, LED behavior, port patterns for auto-detection. No runtime logic.
- **Device Registry** (`src/shared/devices/registry.ts`): Map of `DeviceDefinition` by ID. Key functions: `getDeviceDefinition(id)`, `identifyDevice(portName)`, `getDefaultDevice()`, `getAllDeviceDefinitions()`.
- **Registered devices**: `nanokontrol2`, `akai-midimix`. Default is `nanokontrol2`.
- Import from barrel: `@shared/devices` (re-exports everything).

### Preset System

- 10 factory presets: 6 nK2 (Default, Synth Lead, Drums, DAW Transport, DJ Mixer, Nord Drum 2) + 4 MidiMix (Default, Synth, DJ, Nord Drum 2). Created by `createSamplePresets()` in `src/shared/schemas.ts`.
- Presets store both mapping config AND optional `ControlValues` (knob/slider positions, button states).
- Preset metadata: id, name, description, author, group, tags, favorite, timestamps.
- File storage managed by `FileManager` in main process.
- A/B comparison support in preset store.

### IPC Protocol (`src/shared/ipc-protocol.ts`)

| Direction | Mechanism | Constant | Examples |
|---|---|---|---|
| Main -> Renderer | `webContents.send()` via renderer-bridge | `IPC_EVENTS` | `midi:cc`, `midi:connected`, `midi:disconnected`, `midi:portsChanged`, `error` |
| Renderer -> Main | `ipcRenderer.invoke()` / `ipcMain.handle()` | `IPC_INVOKE` | `midi:connect`, `preset:load`, `preset:save`, `config:get`, `device:list` |

Preload script (`src/main/preload.ts`) exposes `window.electronAPI` via `contextBridge`. Type: `ElectronAPI`.

### Zustand Stores (`src/renderer/stores/`)

| Store | File | Responsibility |
|---|---|---|
| `useControlsStore` | `controls.ts` | Per-device control state (knobs, sliders, buttons). Multi-device keyed by instanceId. |
| `usePresetStore` | `preset.ts` | Preset library, selection, A/B comparison, device-type filtering. |
| `useConnectionStore` | `connection.ts` | MIDI connection state, port discovery, multi-device port routing. |
| `useDevicesStore` | `devices.ts` | Device instance lifecycle, active device selection, tab ordering. |
| `useSettingsStore` | `settings.ts` | App configuration (theme, soft takeover, LED mode, etc.). |
| `useUndoStore` | `undo.ts` | Per-device undo stacks (max 20 steps). |

All stores maintain backward compatibility with legacy single-device API while supporting multi-device via instanceId.

## Development Conventions

- **Strict TypeScript**: All strict flags enabled. No `any` allowed.
- **Zod validation**: All preset/config data validated at runtime via schemas in `src/shared/schemas.ts`.
- **`as const` assertions**: Used extensively for MIDI CC arrays, IPC channel names, control type lists (see `src/shared/constants.ts`, `src/shared/ipc-protocol.ts`).
- **Type guards**: `isMappingConfig()`, `isDeviceMappingConfig()`, `isMidiCCEvent()`, `isPreset()` for discriminating union types.
- **MIDI channels**: 1-indexed in user-facing code and types (1-16). 0-indexed only at the easymidi boundary.
- **Import paths**: Use `@shared/`, `@main/`, `@renderer/` aliases. Shared code uses `.js` extensions in import specifiers.
- **Tests**: Vitest. 356 tests across 7 test files in `tests/`. All tests must pass before merging. Test files: `config-parser`, `config-preset`, `device-definitions`, `nord-drum-2`, `regression`, `stores`, `ui-components`.
- **No ARCHITECTURE.md or DEVELOPMENT.md** exist yet. The README.md is the primary documentation.

## Common Tasks

### Add a new MIDI controller

1. Create device definition in `src/shared/devices/<device-name>.ts` implementing `DeviceDefinition`.
2. Register in `src/shared/devices/registry.ts`.
3. Export from `src/shared/devices/index.ts`.
4. Add factory presets in `src/shared/schemas.ts` (`createSamplePresets()`).
5. Add device-specific editor component if layout differs (see `src/renderer/components/Editor/MidiMix/`).

### Add a new target synthesizer

1. Create CC reference constants in `src/shared/<synth-name>-cc.ts` (see `nord-drum-2-cc.ts` as template).
2. Create factory preset functions in `src/shared/schemas.ts` using those constants.
3. Add to `createSamplePresets()` return array.

### Modify IPC protocol

1. Add channel name to `IPC_EVENTS` or `IPC_INVOKE` in `src/shared/ipc-protocol.ts`.
2. Define request/response types in the same file.
3. Add handler in `src/main/ipc-handlers.ts`.
4. Expose via preload in `src/main/preload.ts`.
5. Update `ElectronAPI` type (auto-derived from preload exports).

### Add a Zustand store

1. Create in `src/renderer/stores/`.
2. Follow existing pattern: `create<StoreState>((set, get) => ({...}))`.
3. Initialize in `App.tsx` useEffect if needed.

## Critical File Index

| File | Description |
|---|---|
| `src/shared/types.ts` | Core types: MappingEntry, MappingConfig, DeviceMappingConfig, control types |
| `src/shared/ipc-protocol.ts` | IPC channel names (IPC_EVENTS, IPC_INVOKE), all request/response types, AppConfig |
| `src/shared/schemas.ts` | Zod schemas, validation, factory presets, type guards, default configs |
| `src/shared/constants.ts` | Hardware CC maps, MIDI value constants, control type utilities |
| `src/shared/devices/device-definition.ts` | DeviceDefinition interface and control type hierarchy |
| `src/shared/devices/registry.ts` | Device registry: lookup, auto-detection, enumeration |
| `src/shared/nord-drum-2-cc.ts` | Nord Drum 2 CC reference constants (target synth) |
| `src/main/main.ts` | Electron app lifecycle, window creation |
| `src/main/ipc-handlers.ts` | All IPC handlers; coordinates MidiManager, FileManager, ConfigManager |
| `src/main/preload.ts` | contextBridge API exposed as window.electronAPI |
| `src/main/services/midi-manager.ts` | Orchestrates MidiHandler + MappingEngine + LedController |
| `src/main/services/mapping-engine.ts` | CC/Note remapping logic |
| `src/renderer/App.tsx` | Root component, store initialization, tab navigation |
| `src/renderer/stores/controls.ts` | Multi-device control state management (largest store) |
| `src/renderer/stores/preset.ts` | Preset CRUD, A/B comparison, device-type filtering |

## Claude Code Permissions Reference

Allowed operations (from `.claude/settings.local.json`):

- **Testing**: `npm test`, `npm run typecheck`, `npx vitest run`, `npx tsc`
- **Building**: `npm run build`, `npm run build:vite`, `npx vite build`
- **Dev**: `npm run dev` (with timeout)
- **Node/npm**: `npm install`, `npm ls`, `npm info`, `node -e`, `npx tsx`
- **Git**: `git add`, `git commit`, `git push`, `git branch`, `git rm`, `git diff`, `git status`, `git log`
- **System**: `ls`, `tree`, `grep`, `find`, `wc`, `echo`, `chmod`
- **Web**: `WebSearch`, `WebFetch` for specific domains (korg.com, github.com, nordkeyboards.com, etc.)
