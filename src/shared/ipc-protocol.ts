/**
 * IPC Protocol Types
 *
 * Defines the message types and payloads for communication between
 * the Electron main process and renderer process.
 */

import type { MappingConfig, MidiPorts, CCMessage } from './types.js';

// =============================================================================
// IPC Channel Names
// =============================================================================

/** Events sent from main to renderer */
export const IPC_EVENTS = {
  MIDI_CC: 'midi:cc',
  MIDI_NOTE: 'midi:note',
  MIDI_CONNECTED: 'midi:connected',
  MIDI_DISCONNECTED: 'midi:disconnected',
  MIDI_PORTS_CHANGED: 'midi:portsChanged',
  ERROR: 'error',
} as const;

/** Invoke channels (renderer calls main) */
export const IPC_INVOKE = {
  // MIDI operations
  MIDI_SEND: 'midi:send',
  MIDI_CONNECT: 'midi:connect',
  MIDI_DISCONNECT: 'midi:disconnect',
  MIDI_GET_PORTS: 'midi:ports',
  MIDI_GET_STATUS: 'midi:status',

  // Preset operations
  PRESET_LOAD: 'preset:load',
  PRESET_SAVE: 'preset:save',
  PRESET_DELETE: 'preset:delete',
  PRESET_LIST: 'preset:list',
  PRESET_APPLY: 'preset:apply',

  // Config operations
  CONFIG_GET: 'config:get',
  CONFIG_UPDATE: 'config:update',

  // Device operations
  DEVICE_LIST: 'device:list',
} as const;

// =============================================================================
// Event Payloads (Main -> Renderer)
// =============================================================================

/** Payload for midi:cc event */
export interface MidiCCEvent extends CCMessage {
  /** Mapped output CC (after remapping) */
  outputCC?: number;
  /** Mapped output channel (after remapping) */
  outputChannel?: number;
  /** Control type identifier */
  controlType?: string;
}

/** Payload for midi:note event - sent when a Note On/Off is received from hardware */
export interface MidiNoteEvent {
  /** MIDI channel (0-indexed for easymidi) */
  channel: number;
  /** Note number (0-127) */
  note: number;
  /** Velocity (0-127) */
  velocity: number;
  /** Whether this is a Note On (true) or Note Off (false) */
  isNoteOn: boolean;
  /** Mapped control type identifier, if the note is mapped */
  controlType?: string;
  /** Mapped output note number, if remapped */
  outputNote?: number;
  /** Mapped output channel (1-indexed), if remapped */
  outputChannel?: number;
}

/** Payload for midi:connected event */
export interface MidiConnectedEvent {
  inputPort: string;
  outputPort: string;
  virtualPort: string;
}

/** Payload for midi:disconnected event */
export interface MidiDisconnectedEvent {
  reason: string;
}

/** Payload for midi:portsChanged event */
export interface MidiPortsChangedEvent extends MidiPorts {}

/** Payload for error event */
export interface ErrorEvent {
  message: string;
  code?: string;
}

// =============================================================================
// Invoke Request/Response Types (Renderer -> Main)
// =============================================================================

/** Request to send CC message */
export interface SendCCRequest {
  channel: number;
  cc: number;
  value: number;
}

/** Request to connect to MIDI device */
export interface ConnectRequest {
  inputPort?: string;
  outputPort?: string;
}

/** Response from connect request */
export interface ConnectResponse {
  success: boolean;
  error?: string;
  inputPort?: string;
  outputPort?: string;
}

/** Summary of a discovered MIDI device, safe for IPC serialization */
export interface DiscoveredDeviceSummary {
  deviceId: string;
  deviceName: string;
  inputPort: string;
  outputPort: string;
}

/** Response from get ports request */
export interface GetPortsResponse extends MidiPorts {
  nanoKontrol2Found: boolean;
  /** All recognized devices found on available MIDI ports */
  discoveredDevices?: DiscoveredDeviceSummary[];
}

/** MIDI connection status */
export interface MidiStatusResponse {
  connected: boolean;
  inputPort: string | null;
  outputPort: string | null;
  virtualPort: string;
  midiAvailable: boolean;
  /** ID of the connected device, if identified */
  deviceId?: string;
  /** Human-readable name of the connected device */
  deviceName?: string;
}

/** Preset metadata */
export interface PresetMetadata {
  id: string;
  name: string;
  description?: string;
  author?: string;
  /** Group/folder for organizing presets */
  group?: string;
  createdAt: string;
  modifiedAt: string;
  tags: string[];
  favorite: boolean;
}

// =============================================================================
// Control Values Types (for preset state storage)
// =============================================================================

/** Control values for a single track */
export interface TrackControlValues {
  /** Knob value (0-127) */
  knob: number;
  /** Knob label (optional) */
  knobLabel?: string;
  /** Slider value (0-127) */
  slider: number;
  /** Slider label (optional) */
  sliderLabel?: string;
  /** Solo button state */
  solo: boolean;
  /** Solo button label (optional) */
  soloLabel?: string;
  /** Mute button state */
  mute: boolean;
  /** Mute button label (optional) */
  muteLabel?: string;
  /** Rec button state */
  rec: boolean;
  /** Rec button label (optional) */
  recLabel?: string;
}

/** Transport button states */
export interface TransportControlValues {
  play: boolean;
  playLabel?: string;
  stop: boolean;
  stopLabel?: string;
  rewind: boolean;
  rewindLabel?: string;
  forward: boolean;
  forwardLabel?: string;
  record: boolean;
  recordLabel?: string;
  cycle: boolean;
  cycleLabel?: string;
  track_left: boolean;
  track_leftLabel?: string;
  track_right: boolean;
  track_rightLabel?: string;
  marker_set: boolean;
  marker_setLabel?: string;
  marker_left: boolean;
  marker_leftLabel?: string;
  marker_right: boolean;
  marker_rightLabel?: string;
}

/** All control values for a preset */
export interface ControlValues {
  /** Control values for all 8 tracks */
  tracks: TrackControlValues[];
  /** Transport button states */
  transport: TransportControlValues;
}

/** Full preset data */
export interface Preset {
  metadata: PresetMetadata;
  mapping: MappingConfig;
  /** Optional stored control values - when present, loading the preset will restore these values */
  controlValues?: ControlValues;
}

/** Request to save a preset */
export interface SavePresetRequest {
  preset: Preset;
  overwrite?: boolean;
}

/** Response from save preset */
export interface SavePresetResponse {
  success: boolean;
  error?: string;
  id?: string;
}

/** Request to load a preset */
export interface LoadPresetRequest {
  id: string;
}

/** Response from load preset */
export interface LoadPresetResponse {
  success: boolean;
  error?: string;
  preset?: Preset;
}

/** Request to delete a preset */
export interface DeletePresetRequest {
  id: string;
}

/** Response from delete preset */
export interface DeletePresetResponse {
  success: boolean;
  error?: string;
}

/** Response from list presets */
export interface ListPresetsResponse {
  presets: PresetMetadata[];
}

/** Request to apply a mapping configuration */
export interface ApplyMappingRequest {
  mapping: MappingConfig;
}

/** Response from apply mapping */
export interface ApplyMappingResponse {
  success: boolean;
  error?: string;
}

/** Application configuration */
/** Custom theme colors for user-defined themes */
export interface ThemeColors {
  /** Main background color */
  background: string;
  /** Darker background (e.g., body, scrollbar) */
  backgroundDarker: string;
  /** Light surface color (e.g., hover states, inputs) */
  surface: string;
  /** Border color */
  border: string;
  /** Primary accent color */
  accent: string;
  /** Solo button LED color */
  solo: string;
  /** Mute button LED color */
  mute: string;
  /** Record button LED color */
  rec: string;
}

/** A saved user-defined theme */
export interface UserTheme {
  name: string;
  colors: ThemeColors;
}

/** Default theme color values */
export const DEFAULT_THEME_COLORS: ThemeColors = {
  background: '#1a1a1a',
  backgroundDarker: '#0f0f0f',
  surface: '#2a2a2a',
  border: '#3a3a3a',
  accent: '#ff6600',
  solo: '#ffcc00',
  mute: '#00cc66',
  rec: '#ff3333',
};

export interface AppConfig {
  /** Soft takeover mode: catch, jump, or pickup */
  softTakeoverMode: 'catch' | 'jump' | 'pickup';
  /** Soft takeover threshold (default: 3) */
  softTakeoverThreshold: number;
  /** Theme: light or dark */
  theme: 'light' | 'dark' | 'system';
  /** Show values in decimal or hex */
  valueDisplay: 'decimal' | 'hex';
  /** Auto-connect to nanoKONTROL2 on startup */
  autoConnect: boolean;
  /** Whether to transmit button MIDI states on preset load (default: true) */
  transmitButtonsOnLoad: boolean;
  /** Default MIDI channel for new mappings (1-16, default 1) */
  globalMidiChannel: number;
  /** LED mode: internal (button state) or external (DAW controlled) */
  ledMode: 'internal' | 'external';
  /** Recently used presets (max 10) */
  recentPresets: string[];
  /** Quick access preset slots (5 slots) */
  quickAccessSlots: (string | null)[];
  /** Window bounds for restoration */
  windowBounds?: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  /** Custom theme colors (overrides defaults when set) */
  customThemeColors?: ThemeColors;
  /** Saved user-defined themes */
  userThemes?: UserTheme[];
  /** UI scale percentage (50-200, default 100) */
  uiScale?: number;
  /** Font family for the interface */
  fontFamily?: string;
  /** Font size in pixels (10-24, default 14) */
  fontSize?: number;
}

/** Response from get config */
export interface GetConfigResponse {
  config: AppConfig;
}

/** Request to update config */
export interface UpdateConfigRequest {
  updates: Partial<AppConfig>;
}

/** Response from update config */
export interface UpdateConfigResponse {
  success: boolean;
  error?: string;
  config?: AppConfig;
}

// =============================================================================
// Type Guards
// =============================================================================

export function isMidiCCEvent(event: unknown): event is MidiCCEvent {
  return (
    typeof event === 'object' &&
    event !== null &&
    'channel' in event &&
    'controller' in event &&
    'value' in event
  );
}

export function isPreset(data: unknown): data is Preset {
  return (
    typeof data === 'object' &&
    data !== null &&
    'metadata' in data &&
    'mapping' in data
  );
}
