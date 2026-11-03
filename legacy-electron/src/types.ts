/**
 * Core type definitions for the nkEditor3 MIDI CC remapper.
 *
 * These types define the structure of control mappings, configuration,
 * and runtime state for the nanoKONTROL2 controller.
 */

// =============================================================================
// Control Type Definitions
// =============================================================================

/**
 * Types of continuous controls on track channels.
 * - knob: rotary encoder/potentiometer
 * - slider: linear fader
 */
export type ContinuousControlType = 'knob' | 'slider';

/**
 * Types of button controls on track channels.
 * - solo: solo button (typically yellow LED)
 * - mute: mute button (typically green LED)
 * - rec: record arm button (typically red LED)
 */
export type ButtonControlType = 'solo' | 'mute' | 'rec';

/**
 * All control types available on a track channel.
 */
export type ControlType = ContinuousControlType | ButtonControlType;

/**
 * Transport section control types.
 * These controls affect playback and navigation in the DAW.
 */
export type TransportControlType =
  | 'rewind'
  | 'forward'
  | 'stop'
  | 'play'
  | 'record'
  | 'cycle'
  | 'track_left'
  | 'track_right'
  | 'marker_set'
  | 'marker_left'
  | 'marker_right';

/**
 * Button behavior determines how button presses are interpreted.
 * - toggle: Press toggles between on/off states
 * - momentary: Button is on only while pressed
 */
export type ButtonBehavior = 'toggle' | 'momentary';

// =============================================================================
// Mapping Entry Types
// =============================================================================

/**
 * MIDI message types used by controls.
 * 'cc' for Control Change messages, 'note' for Note On/Off messages.
 */
export type MidiMessageType = 'cc' | 'note';

/**
 * A single control mapping entry.
 * Maps an input CC/Note from hardware to an output CC/Note sent to the DAW.
 */
export interface MappingEntry {
  /** CC or Note number received from hardware (0-127) */
  inputCC: number;
  /** CC or Note number to send to DAW (0-127) */
  outputCC: number;
  /** MIDI channel (1-16, not 0-15) */
  channel: number;
  /** Button behavior, only applicable for button controls */
  behavior?: ButtonBehavior | undefined;
  /** Optional human-readable label for the control */
  label?: string | undefined;
  /**
   * MIDI message type for this control.
   * Defaults to 'cc' if omitted, preserving backward compatibility
   * with nanoKONTROL2 configurations that predate multi-device support.
   */
  messageType?: MidiMessageType;
}

/**
 * Complete mapping for a single track channel.
 * Each track has exactly 5 controls: knob, slider, solo, mute, rec.
 */
export interface TrackMapping {
  knob: MappingEntry;
  slider: MappingEntry;
  solo: MappingEntry;
  mute: MappingEntry;
  rec: MappingEntry;
}

/**
 * Complete mapping for the transport section.
 * All 11 transport controls are required.
 */
export interface TransportMapping {
  rewind: MappingEntry;
  forward: MappingEntry;
  stop: MappingEntry;
  play: MappingEntry;
  record: MappingEntry;
  cycle: MappingEntry;
  track_left: MappingEntry;
  track_right: MappingEntry;
  marker_set: MappingEntry;
  marker_left: MappingEntry;
  marker_right: MappingEntry;
}

/**
 * Complete mapping configuration for the entire controller.
 * Contains 8 track mappings (index 0-7) and one transport mapping.
 */
export interface MappingConfig {
  /** Track channel mappings, index 0-7 corresponds to tracks 1-8 */
  tracks: TrackMapping[];
  /** Transport section mapping */
  transport: TransportMapping;
}

// =============================================================================
// Connection and Port Types
// =============================================================================

/**
 * Connection status for MIDI ports.
 */
export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

/**
 * Available MIDI ports on the system.
 */
export interface MidiPorts {
  /** List of available MIDI input port names */
  inputs: string[];
  /** List of available MIDI output port names */
  outputs: string[];
}

// =============================================================================
// Runtime State Types
// =============================================================================

/**
 * Runtime state for a toggle button.
 * Used to track the current on/off state of toggle-mode buttons.
 */
export interface ButtonState {
  /** The CC number this state tracks */
  cc: number;
  /** Whether the button is currently in the "on" state */
  isOn: boolean;
}

/**
 * Runtime state for an LED.
 * LEDs share CC numbers with their corresponding buttons.
 */
export interface LedState {
  /** The CC number this LED responds to */
  cc: number;
  /** Whether the LED is currently illuminated */
  isOn: boolean;
}

// =============================================================================
// Parser Types
// =============================================================================

/**
 * Result of parsing a mapping file.
 * Either contains a valid config or an error message.
 * When a [device] header is present, deviceId identifies the target device.
 * When absent, deviceId defaults to 'nanokontrol2' for backward compatibility.
 */
export type ParseResult =
  | { success: true; config: MappingConfig; deviceId: string }
  | { success: false; error: string };

/**
 * A single parsed line from the mapping file.
 * Used internally during parsing.
 */
export interface ParsedLine {
  controlType: ControlType | TransportControlType;
  inputCC: number;
  outputCC: number;
  channel: number;
  behavior?: ButtonBehavior | undefined;
  label?: string | undefined;
}

// =============================================================================
// MIDI Handler Types
// =============================================================================

/**
 * Represents a MIDI Control Change message.
 * Note: easymidi uses 0-indexed channels (0-15), but user-facing display uses 1-16.
 */
export interface CCMessage {
  /** MIDI channel (0-15, 0-indexed for easymidi) */
  channel: number;
  /** Controller number (0-127) */
  controller: number;
  /** Controller value (0-127) */
  value: number;
}

/**
 * Result of searching for the nanoKONTROL2 device.
 */
export interface NanoKontrol2Ports {
  /** Input port name, or null if not found */
  input: string | null;
  /** Output port name, or null if not found */
  output: string | null;
}

/**
 * Configuration options for the MidiHandler.
 */
export interface MidiHandlerOptions {
  /** Hardware input port name (from nanoKONTROL2) */
  inputPort?: string;
  /** Hardware output port name (for LED control) */
  outputPort?: string;
  /** Name for the virtual output port (default: "nkEditor3 Out") */
  virtualPortName?: string;
}

/**
 * Event payload when device is connected.
 */
export interface ConnectedEvent {
  /** The hardware input port that was connected */
  inputPort: string;
  /** The hardware output port that was connected */
  outputPort: string;
}

/**
 * Event payload when device is disconnected.
 */
export interface DisconnectedEvent {
  /** Reason for disconnection */
  reason: string;
}

/**
 * Event payload when available ports change.
 */
export interface PortsChangedEvent {
  /** Current list of input port names */
  inputs: string[];
  /** Current list of output port names */
  outputs: string[];
}

/**
 * Event payload for error events.
 */
export interface ErrorEvent {
  /** Error message describing what went wrong */
  message: string;
}

// =============================================================================
// MIDI Constants
// =============================================================================

/** MIDI CC value for "on" state */
export const MIDI_VALUE_ON = 127;

/** MIDI CC value for "off" state */
export const MIDI_VALUE_OFF = 0;

/** Default virtual output port name */
export const DEFAULT_VIRTUAL_PORT_NAME = 'nkEditor3 Out';

/** Default hotplug polling interval in milliseconds */
export const DEFAULT_HOTPLUG_INTERVAL_MS = 2000;

/** Hotplug debounce time in milliseconds */
export const HOTPLUG_DEBOUNCE_MS = 500;

/**
 * CC numbers for buttons that have LEDs on the nanoKONTROL2.
 * These are the only CCs that can control LEDs.
 */
export const LED_CC_NUMBERS = [
  // Solo buttons (Track 1-8)
  32, 33, 34, 35, 36, 37, 38, 39,
  // Mute buttons (Track 1-8)
  48, 49, 50, 51, 52, 53, 54, 55,
  // Record buttons (Track 1-8)
  64, 65, 66, 67, 68, 69, 70, 71,
  // Transport buttons with LEDs
  41, 42, 45, 46, // Play, Stop, Record, Cycle
] as const;

/**
 * Check if a CC number corresponds to a button with an LED.
 */
export function hasLed(cc: number): boolean {
  return (LED_CC_NUMBERS as readonly number[]).includes(cc);
}
