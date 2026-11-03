#!/usr/bin/env node
/**
 * nkEditor3 - MIDI CC Remapper for Korg nanoKONTROL2
 *
 * Entry point for the CLI application.
 */

// Re-export types
export type {
  ContinuousControlType,
  ButtonControlType,
  ControlType,
  TransportControlType,
  ButtonBehavior,
  MappingEntry,
  TrackMapping,
  TransportMapping,
  MappingConfig,
  ConnectionStatus,
  MidiPorts,
  ButtonState,
  LedState,
  ParseResult,
  ParsedLine,
  CCMessage,
  NanoKontrol2Ports,
  MidiHandlerOptions,
  ConnectedEvent,
  DisconnectedEvent,
  PortsChangedEvent,
  ErrorEvent,
} from './types.js';

// Re-export type constants and hasLed from types (primary definition)
export {
  MIDI_VALUE_ON,
  MIDI_VALUE_OFF,
  DEFAULT_VIRTUAL_PORT_NAME,
  DEFAULT_HOTPLUG_INTERVAL_MS,
  HOTPLUG_DEBOUNCE_MS,
  LED_CC_NUMBERS,
  hasLed,
} from './types.js';

// Re-export hardware constants from constants.ts
export {
  HARDWARE_CC,
  LED_CC,
  MIDI_VALUES,
  MIDI_CHANNELS,
  TRACK_CONSTANTS,
  TRACK_CONTROL_TYPES,
  TRANSPORT_CONTROL_TYPES,
  DEFAULT_TOGGLE_CONTROLS,
  DEFAULT_MOMENTARY_CONTROLS,
  hasLedCC,
  isButtonControl,
  isContinuousControl,
  getDefaultBehavior,
} from './constants.js';

// Re-export parser
export { parseMapping, parseMappingOrThrow, ParseError } from './config-parser.js';

// Re-export MIDI discovery
export {
  getAvailablePorts,
  findNanoKontrol2,
  isNanoKontrol2Port,
  formatPortsForDisplay,
  isMidiAvailable,
  resetMidiAvailability,
  invalidatePortCache,
  getLastMidiError,
} from './midi-discovery.js';

// Re-export MIDI handler
export { MidiHandler } from './midi-handler.js';

// Re-export mapping engine
export { MappingEngine } from './mapping-engine.js';
export type { ProcessedMessage, ButtonStateChangeEvent } from './mapping-engine.js';

// Re-export LED controller
export { LedController } from './led-controller.js';
