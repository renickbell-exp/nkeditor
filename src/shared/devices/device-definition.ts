/**
 * Device Definition Type System
 *
 * Provides a hardware-agnostic abstraction layer for MIDI controller definitions.
 * Each supported device declares its track layout, control types, LED protocol,
 * and port identification patterns through this type system.
 *
 * This enables the rest of the application to work with any supported controller
 * without embedding device-specific knowledge outside of the device definition files.
 */

// =============================================================================
// MIDI Message Types
// =============================================================================

/** MIDI message types used by controls and LEDs */
export type MidiMessageType = 'cc' | 'note';

// =============================================================================
// Control Definitions
// =============================================================================

/**
 * Properties shared by all physical controls on a device.
 * Every control maps to a specific MIDI message on a specific channel.
 */
interface BaseControlDef {
  /** Semantic identifier within its track (e.g., 'knob', 'knob1', 'fader', 'solo') */
  name: string;
  /** Human-readable label for display in the UI */
  displayName: string;
  /** MIDI CC number or Note number this control sends/receives */
  midiNumber: number;
  /** MIDI channel (1-indexed, matching user-facing convention) */
  channel: number;
  /** Whether this control sends CC or Note messages */
  messageType: MidiMessageType;
}

/**
 * A control that sends a continuous range of values (0-127).
 * Knobs, sliders, and faders fall into this category.
 */
export interface ContinuousControlDef extends BaseControlDef {
  kind: 'continuous';
  /** Continuous controls always communicate via CC messages */
  messageType: 'cc';
  /** Visual representation hint for the UI renderer */
  visualType: 'knob' | 'slider' | 'fader';
}

/**
 * A control with discrete on/off states.
 * Buttons may send CC (nanoKONTROL2) or Note (MidiMix) messages
 * depending on the hardware protocol.
 */
export interface ButtonControlDef extends BaseControlDef {
  kind: 'button';
  /** Button message type varies by device: CC for nK2, Note for MidiMix */
  messageType: MidiMessageType;
  /** Whether the button latches (toggle) or is active only while held (momentary) */
  defaultBehavior: 'toggle' | 'momentary';
  /** Whether the hardware has an LED indicator for this button */
  hasLed: boolean;
}

/** Discriminated union of all control types on a device */
export type ControlDef = ContinuousControlDef | ButtonControlDef;

// =============================================================================
// Track and Section Layout
// =============================================================================

/**
 * Layout of controls within a single track/channel strip.
 * Tracks are the repeating vertical strips on a controller surface.
 */
export interface TrackLayout {
  /** Continuous controls in this track (knobs, sliders, faders) */
  continuousControls: ContinuousControlDef[];
  /** Button controls in this track (solo, mute, rec, etc.) */
  buttonControls: ButtonControlDef[];
}

/**
 * A named group of controls outside the per-track layout.
 * Used for transport sections, master controls, and other global controls.
 */
export interface ExtraControlsSection {
  /** Machine-readable identifier (e.g., 'transport', 'master') */
  name: string;
  /** Human-readable section label */
  displayName: string;
  /** All controls in this section */
  controls: ControlDef[];
}

// =============================================================================
// LED Protocol
// =============================================================================

/**
 * Describes how the device expects to receive LED state updates.
 * Some devices use CC messages for LEDs (nK2), others use Note messages (MidiMix).
 */
export interface LedProtocol {
  /** Whether LEDs are controlled via CC or Note messages */
  messageType: MidiMessageType;
  /** MIDI channel for LED messages (1-indexed) */
  channel: number;
  /** MIDI value that turns an LED on */
  onValue: number;
  /** MIDI value that turns an LED off */
  offValue: number;
}

/**
 * Maps a button to the MIDI number used to control its LED.
 * The LED number often matches the button's own MIDI number, but this
 * is not guaranteed across all devices.
 */
export interface LedDef {
  /** Dot-notation path to the button (e.g., 'track1.solo', 'transport.play') */
  buttonName: string;
  /** MIDI CC or Note number that controls this LED */
  midiNumber: number;
}

// =============================================================================
// Device Definition
// =============================================================================

/**
 * Complete hardware definition for a MIDI controller.
 *
 * A DeviceDefinition is a pure data declaration: it contains no runtime logic,
 * no state, and no side effects. It describes the physical layout and MIDI
 * protocol of a controller so that the application can auto-detect, render,
 * and communicate with it.
 */
export interface DeviceDefinition {
  /** Unique identifier for this device (e.g., 'nanokontrol2', 'akai-midimix') */
  id: string;
  /** Human-readable device name */
  name: string;
  /** Device manufacturer */
  manufacturer: string;
  /** Regex patterns matched against MIDI port names for auto-detection */
  portPatterns: RegExp[];
  /** Number of channel strips (tracks) on the device */
  trackCount: number;
  /** Per-track control layouts. Length must equal trackCount. */
  trackLayouts: TrackLayout[];
  /** Control sections outside the per-track layout (transport, master, etc.) */
  extraSections: ExtraControlsSection[];
  /** Protocol for controlling device LEDs */
  ledProtocol: LedProtocol;
  /** List of all controllable LEDs on the device */
  leds: LedDef[];
  /** Name for the virtual MIDI output port created by the application */
  virtualPortName?: string;
}
