# nkEditor3

A desktop MIDI CC remapping application for hardware MIDI controllers. nkEditor3 intercepts MIDI messages from supported controllers, remaps them according to user-defined presets, and outputs the transformed messages through a virtual MIDI port. This allows complete customization of which CC numbers and MIDI channels each physical control sends, without modifying the hardware itself.

Built with Electron, React, and TypeScript.

## Supported Devices

| Device | Manufacturer | Tracks | Controls per Track | Message Types |
|---|---|---|---|---|
| **nanoKONTROL2** | Korg | 8 | 1 knob, 1 slider, 3 buttons + transport | CC |
| **MIDI Mix** | Akai | 8 | 3 knobs, 1 fader, 3 buttons + master | CC (knobs/faders), Note (buttons) |

The architecture supports adding new devices through a device definition system. Each device declares its physical layout, MIDI protocol, LED behavior, and port identification patterns in a self-contained definition module.

## Features

- **Multi-Device Support** -- Connect and configure multiple MIDI controllers simultaneously. Switch between devices using a tab interface with a device selector dropdown.
- **Device-Specific Editors** -- Each supported device has a purpose-built visual editor that mirrors its physical layout (e.g., the MidiMix editor shows 3 knob rows per track, while the nK2 editor shows 1 knob + 1 slider).
- **CC and Note Remapping** -- Remap controls that send CC messages (knobs, sliders, faders) or Note messages (MidiMix buttons) to arbitrary output numbers and MIDI channels (1-16).
- **Preset System** -- Save, load, duplicate, and organize mapping configurations as presets. Presets store both the CC mapping and the current control values (knob/slider positions, button states).
- **Factory Presets** -- Ships with eight ready-to-use presets: five for nanoKONTROL2 (Default, Synth Lead, Drums, DAW Transport, DJ Mixer) and three for Akai MidiMix (Default MidiMix, MidiMix Synth, MidiMix DJ).
- **Visual Editor** -- Interactive GUI that mirrors the physical controller layout. Click any control to edit its output CC/Note number, MIDI channel, label, value range, and button behavior (toggle/momentary).
- **Librarian** -- Browse, search, filter (by tags and groups), favorite, rename, and manage your preset library. Supports A/B comparison between two presets.
- **Quick Access Slots** -- Five configurable slots for instant preset switching.
- **LED Control** -- Two LED modes: *internal* (LEDs reflect button state) or *external* (LEDs controlled by incoming MIDI from a DAW).
- **Soft Takeover** -- Three modes (catch, jump, pickup) to prevent parameter jumps when physical knob/slider positions differ from stored preset values.
- **Hotplug Detection** -- Automatic detection when the nanoKONTROL2 is connected or disconnected.
- **Auto-Connect** -- Optionally connects to the nanoKONTROL2 automatically on application startup.
- **Virtual MIDI Output** -- Creates a virtual MIDI port named "nkEditor3 Out" that other applications (DAWs, synths) can receive remapped CC messages from.
- **Custom Themes** -- Configurable UI colors, font family, font size, and UI scale (50-200%).
- **Undo Support** -- Undo changes to control values in the editor.
- **Randomize Controls** -- Randomize knob and slider values globally or for selected controls only.
- **Control Value Storage** -- Presets optionally save and restore knob/slider positions and button states, with an option to transmit button MIDI states on preset load.
- **Value Range Limiting** -- Configure minimum and maximum output values for continuous controls, and custom on/off values for buttons.

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop framework | Electron 28 |
| Frontend | React 18, TypeScript 5 |
| State management | Zustand 4 |
| Styling | Tailwind CSS 3, PostCSS, Autoprefixer |
| MIDI I/O | easymidi 3 (Node.js native module via RtMidi) |
| Validation | Zod 3 |
| Build tooling | Vite 5, vite-plugin-electron |
| Packaging | electron-builder |
| Testing | Vitest |

## Prerequisites

- **Node.js** >= 18.0.0
- **npm** (ships with Node.js)
- **ALSA development libraries** (Linux) -- required by the `easymidi` native module for MIDI port access. Install with:
  ```
  # Arch / Manjaro
  sudo pacman -S alsa-lib

  # Debian / Ubuntu
  sudo apt install libasound2-dev
  ```
- **A supported MIDI controller** (optional for development -- the app will run without hardware but MIDI features require a controller). Currently supported: Korg nanoKONTROL2, Akai MIDI Mix.

## Installation

```bash
# Clone or navigate to the project directory
cd nkEditor3

# Install dependencies (includes native module compilation for easymidi)
npm install
```

## How to Run

### Development Mode

```bash
npm run dev
```

This starts the Vite dev server and launches Electron with hot-reload. The renderer loads from `http://localhost:5173` and DevTools open automatically.

### Production Build

```bash
npm run build
```

This runs TypeScript compilation, Vite build, and electron-builder packaging. Output:
- Renderer bundle: `dist/`
- Electron main process: `dist-electron/`
- Packaged application: `release/`

### Other Scripts

| Command | Description |
|---|---|
| `npm run dev` | Start Vite dev server with Electron |
| `npm run build` | Full production build and package |
| `npm run build:vite` | Build only the Vite frontend (no packaging) |
| `npm run preview` | Preview the production Vite build |
| `npm run typecheck` | Run TypeScript type checking without emitting |
| `npm test` | Run tests with Vitest |
| `npm run test:watch` | Run tests in watch mode |

## Project Structure

```
nkEditor3/
  index.html              # Electron renderer entry HTML
  package.json            # Dependencies and build configuration
  vite.config.ts          # Vite + Electron plugin configuration
  tsconfig.json           # TypeScript configuration
  tailwind.config.js      # Tailwind CSS theme (nanoKONTROL2-inspired colors)
  postcss.config.js       # PostCSS with Tailwind and Autoprefixer
  mappings/
    default.txt           # Example text-based mapping file
  presets/                 # (empty) Local preset storage placeholder
  src/
    shared/               # Code shared between main and renderer processes
      constants.ts        # Hardware CC assignments, LED mappings, utility functions
      ipc-protocol.ts     # IPC channel names and request/response type definitions
      schemas.ts          # Zod validation schemas, factory preset generators
      types.ts            # Core TypeScript type definitions
      devices/            # Device abstraction layer
        device-definition.ts  # DeviceDefinition interface and control types
        registry.ts           # Central device registry with lookup and auto-detection
        nanokontrol2.ts       # Korg nanoKONTROL2 hardware definition
        akai-midimix.ts       # Akai MIDI Mix hardware definition
    main/                 # Electron main process
      main.ts             # App lifecycle, window creation
      preload.ts          # Context bridge exposing IPC API to renderer
      ipc-handlers.ts     # IPC handler registration and request routing
      services/
        midi-manager.ts   # High-level MIDI orchestrator (connect, remap, LED sync)
        midi-handler.ts   # Low-level MIDI I/O via easymidi
        midi-discovery.ts # Port scanning and nanoKONTROL2 detection
        mapping-engine.ts # CC remapping logic and button state tracking
        led-controller.ts # Hardware LED state management
        config-manager.ts # App configuration persistence (JSON in userData)
        file-manager.ts   # Preset file I/O (JSON in userData/presets)
        config-parser.ts  # Text-based mapping file parser
      utils/
        renderer-bridge.ts  # Sends events from main to renderer window
    renderer/             # React frontend (renderer process)
      main.tsx            # React DOM entry point
      App.tsx             # Root component, layout, tab navigation
      components/
        Header.tsx        # Top bar with connection status and tab navigation
        Editor/
          EditorView.tsx  # Main editor with track grid and transport
          Track.tsx       # Single track strip (knob, slider, 3 buttons)
          Knob.tsx        # Rotary knob control
          Slider.tsx      # Linear fader control
          Button.tsx      # Solo/Mute/Rec button
          Transport.tsx   # Transport button bar
          ControlPopover.tsx  # Inline CC mapping editor popover
        Librarian/
          LibrarianView.tsx      # Preset library browser
          PresetList.tsx         # Scrollable preset list with filtering
          QuickAccess.tsx        # Quick access slot bar
          NewPresetDialog.tsx    # Create new preset dialog
          EditPresetDialog.tsx   # Edit preset metadata dialog
          DuplicatePresetDialog.tsx  # Duplicate preset dialog
          EditControlValuesDialog.tsx # Edit control values dialog
        Settings/
          SettingsView.tsx  # Configuration panel
        Help/
          HelpView.tsx    # Built-in documentation and troubleshooting
      stores/
        connection.ts     # MIDI connection state (Zustand)
        controls.ts       # Control values state (knobs, sliders, buttons)
        preset.ts         # Preset library and A/B comparison state
        settings.ts       # App configuration state
        undo.ts           # Undo stack for control value changes
      styles/
        globals.css       # Global styles and Tailwind imports
      types/
        electron.d.ts     # Type declarations for window.electronAPI
  dist/                   # Built renderer output
  dist-electron/          # Built main process output
  release/                # Packaged application output
  tests/
    config-parser.test.ts     # Config parser unit tests
    config-preset.test.ts     # Device-aware preset and config tests
    device-definitions.test.ts # Device definition and registry tests
    stores.test.ts            # Multi-device Zustand store tests
    ui-components.test.ts     # UI component routing tests
    regression.test.ts        # Backward compatibility regression tests
```

## Multi-Device Architecture

The application uses a device abstraction layer that decouples MIDI handling from specific hardware implementations.

### Adding a New Device

1. Create a device definition file in `src/shared/devices/` implementing the `DeviceDefinition` interface
2. Register the device in `src/shared/devices/registry.ts`
3. Create a device-specific editor component if the control layout differs significantly from existing editors
4. Add factory presets in `src/shared/schemas.ts`
5. Add tests in the `tests/` directory

## Configuration

Application configuration is stored as JSON at the Electron `userData` path (typically `~/.config/nkeditor3/config.json` on Linux). Configuration is managed through the Settings tab in the UI.

### Configurable Settings

| Setting | Default | Description |
|---|---|---|
| Auto-connect | `true` | Connect to nanoKONTROL2 on startup |
| Soft takeover mode | `catch` | How knobs/sliders behave when preset values differ from physical position (`catch`, `jump`, `pickup`) |
| Soft takeover threshold | `3` | Sensitivity for catch mode (1-20) |
| LED mode | `internal` | `internal` = LEDs follow button state; `external` = LEDs controlled by DAW |
| Global MIDI channel | `1` | Default channel for new mappings (1-16) |
| Transmit buttons on load | `true` | Send button MIDI states when loading a preset |
| Value display | `decimal` | Show CC values in decimal or hex |
| Theme | `dark` | Color theme (`light`, `dark`, `system`) |
| UI scale | `100` | Interface zoom level (50-200%) |
| Font family | `Inter` | UI font |
| Font size | `14` | Base font size in pixels (10-24) |
| Custom theme colors | -- | Override background, accent, solo/mute/rec LED colors |

### Preset Storage

Presets are stored as individual JSON files in `~/.config/nkeditor3/presets/` (Linux). Each file contains:
- **Metadata** -- name, description, author, tags, group, timestamps, favorite flag
- **Mapping** -- complete CC remapping table for all 51 controls
- **Control values** (optional) -- saved knob/slider positions and button states

## Usage

### Getting Started

1. Connect a supported MIDI controller (nanoKONTROL2, MIDI Mix) via USB.
2. Launch nkEditor3 (`npm run dev` for development).
3. The app auto-detects the controller and connects (if auto-connect is enabled).
4. Use the device selector dropdown to add additional devices, and switch between them using the tab bar.
5. The "nkEditor3 Out" virtual MIDI port appears in your DAW or synth software -- route it as a MIDI input.
6. Move controls on the hardware; the GUI updates in real-time and remapped MIDI messages are sent to the virtual output.

### Editor Tab

- The editor displays a visual replica of the active controller. Each device has a purpose-built editor layout.
- **Click any control** to open a popover where you can set the output CC number, MIDI channel, label, button behavior (toggle/momentary), and value range (min/max for knobs/sliders, on/off values for buttons).
- Use the **Save** button to persist changes to the current preset.
- **Randomize** knob and slider values (all or selected controls only).
- **Undo** reverts the last control value change.
- **Copy/Paste Labels** transfers control labels between presets via the clipboard.

### Librarian Tab

- Browse all saved presets with search, tag filtering, and group filtering.
- **Load** a preset to activate its mapping and restore saved control values.
- **Create**, **duplicate**, **rename**, **delete**, and **favorite** presets.
- Assign presets to **Quick Access** slots for fast switching.
- **A/B compare** two presets side by side.

### Settings Tab

- Configure MIDI connection, soft takeover, LED mode, and display preferences.
- Customize theme colors, font, and UI scale.
- Manage user-defined color themes.

### Wide Screen Layout

On displays 1400px or wider, the Editor and Librarian panels display side by side for a streamlined workflow.

## Packaging

electron-builder is configured to produce platform-specific packages:

| Platform | Format |
|---|---|
| Linux | AppImage, .deb |
| macOS | .dmg |
| Windows | NSIS installer |

Build with `npm run build` and find the output in the `release/` directory.

## License

MIT
