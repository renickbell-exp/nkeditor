/**
 * Mapping Engine Module
 *
 * Core CC remapping logic for the nkEditor3 application.
 * Processes incoming MIDI CC messages from the nanoKONTROL2 hardware
 * and transforms them according to the loaded mapping configuration.
 *
 * Key responsibilities:
 * - Map input CCs to output CCs and channels
 * - Handle toggle vs momentary button behaviors
 * - Track button states for toggle buttons
 * - Emit events for processed messages and state changes
 */

import { EventEmitter } from 'events';
import type {
  MappingConfig,
  MappingEntry,
  TrackMapping,
  TransportMapping,
  ButtonBehavior,
  ControlType,
  TransportControlType,
} from '@shared/types.js';
import { MIDI_VALUE_ON, MIDI_VALUE_OFF, hasLed } from '@shared/types.js';
import { isContinuousControl } from '@shared/constants.js';
import type { MidiMessageType, DeviceDefinition } from '@shared/devices/device-definition.js';

// =============================================================================
// Types
// =============================================================================

/**
 * Result of processing an incoming CC message.
 * Contains all information needed to send the remapped message
 * and update UI/LEDs.
 */
export interface ProcessedMessage {
  /** Original input CC number from hardware */
  inputCC: number;
  /** Remapped output CC number */
  outputCC: number;
  /** Output MIDI channel (1-indexed for display/sending) */
  channel: number;
  /** CC value to send (0-127) */
  value: number;
  /** Full control type identifier (e.g., 'track1.slider', 'transport.play') */
  controlType: string;
  /** Whether this control is a button */
  isButton: boolean;
  /** For toggle buttons, the new state after processing */
  buttonState?: boolean | undefined;
  /** Optional label from mapping configuration */
  label?: string | undefined;
  /**
   * MIDI message type that produced this result.
   * Defaults to 'cc' for backward compatibility with nanoKONTROL2.
   */
  messageType?: MidiMessageType;
}

// =============================================================================
// Composite Key Utility
// =============================================================================

/**
 * Creates a composite lookup key that distinguishes between CC and Note
 * messages with the same MIDI number.
 *
 * Without this, a CC 25 from a nanoKONTROL2 would collide with Note 25
 * from a MidiMix in the lookup map. The composite key prevents that.
 *
 * @param messageType - 'cc' or 'note'
 * @param midiNumber - The CC number or Note number (0-127)
 * @returns A string like "cc:42" or "note:25"
 */
export function makeLookupKey(messageType: MidiMessageType, midiNumber: number): string {
  return `${messageType}:${midiNumber}`;
}

/**
 * Event emitted when a button's toggle state changes.
 */
export interface ButtonStateChangeEvent {
  /** CC number of the button */
  cc: number;
  /** New state of the button (true = on, false = off) */
  isOn: boolean;
  /** Full control type identifier */
  controlType: string;
}

/**
 * Internal structure for fast CC/Note lookup.
 * Maps an input MIDI number to its mapping entry and metadata.
 */
interface MappingLookupEntry {
  /** The mapping entry from configuration */
  mapping: MappingEntry;
  /** Full control type identifier (e.g., 'track1.solo') */
  controlType: string;
  /** Whether this is a button control */
  isButton: boolean;
  /** Button behavior (only for buttons) */
  behavior?: ButtonBehavior | undefined;
  /** MIDI message type for this entry (defaults to 'cc') */
  messageType: MidiMessageType;
}

// =============================================================================
// MappingEngine Class
// =============================================================================

/**
 * MappingEngine processes MIDI CC messages and applies the configured remapping.
 *
 * Events emitted:
 * - 'output': Remapped CC ready to send (ProcessedMessage)
 * - 'buttonStateChanged': Button toggle state changed (ButtonStateChangeEvent)
 *
 * @example
 * const engine = new MappingEngine(config);
 *
 * engine.on('output', (msg) => {
 *   midiHandler.sendCC(msg.channel - 1, msg.outputCC, msg.value);
 * });
 *
 * engine.on('buttonStateChanged', ({ cc, isOn }) => {
 *   ledController.setLed(cc, isOn);
 * });
 *
 * // Process incoming CC from hardware
 * midiHandler.on('cc', (msg) => {
 *   engine.processCC(msg.channel, msg.controller, msg.value);
 * });
 */
export class MappingEngine extends EventEmitter {
  /** Current mapping configuration */
  private config: MappingConfig;

  /** Button toggle states: composite key -> isOn */
  private buttonStates: Map<string, boolean> = new Map();

  /**
   * Legacy lookup map: inputCC -> mapping info (numeric keys).
   * Retained for backward compatibility with code that calls getButtonState(cc).
   * When no device definition is set, this is the primary lookup.
   */
  private ccToMapping: Map<number, MappingLookupEntry> = new Map();

  /**
   * Composite key lookup map: "cc:42" or "note:25" -> mapping info.
   * Prevents collisions between CC and Note messages with the same number.
   * This is the primary lookup when a device definition is available.
   */
  private compositeToMapping: Map<string, MappingLookupEntry> = new Map();

  /**
   * Optional device definition for building the lookup map
   * with correct message types per control.
   */
  private deviceDefinition: DeviceDefinition | null = null;

  /**
   * Creates a new MappingEngine with the given configuration.
   *
   * @param config - The mapping configuration to use
   * @param deviceDefinition - Optional device definition for message type awareness
   */
  constructor(config: MappingConfig, deviceDefinition?: DeviceDefinition) {
    super();
    this.config = config;
    this.deviceDefinition = deviceDefinition ?? null;
    this.buildLookupMap();
  }

  /**
   * Processes an incoming CC message from hardware.
   *
   * Delegates to processMessage() with message type 'cc'.
   * This method's signature and behavior are preserved exactly for backward compatibility.
   *
   * @param channel - Input MIDI channel (0-indexed from easymidi)
   * @param cc - Input CC number (0-127)
   * @param value - Input CC value (0-127)
   * @returns ProcessedMessage if CC is mapped, null if not mapped
   */
  processCC(_channel: number, cc: number, value: number): ProcessedMessage | null {
    return this.processMessage('cc', _channel, cc, value);
  }

  /**
   * Processes an incoming Note message from hardware.
   *
   * Delegates to processMessage() with message type 'note'.
   * Used by devices like the Akai MidiMix that send Note On/Off for buttons.
   *
   * @param channel - Input MIDI channel (0-indexed from easymidi)
   * @param note - Input Note number (0-127)
   * @param velocity - Input velocity (0-127)
   * @returns ProcessedMessage if Note is mapped, null if not mapped
   */
  processNote(channel: number, note: number, velocity: number): ProcessedMessage | null {
    return this.processMessage('note', channel, note, velocity);
  }

  /**
   * Unified entry point for processing any MIDI message (CC or Note).
   *
   * Looks up the mapping using a composite key (e.g., "cc:42" or "note:25"),
   * applies button transformations, and emits the processed result.
   *
   * @param messageType - 'cc' or 'note'
   * @param channel - Input MIDI channel (0-indexed from easymidi)
   * @param midiNumber - CC number or Note number (0-127)
   * @param value - CC value or Note velocity (0-127)
   * @returns ProcessedMessage if the message is mapped, null if not mapped
   */
  processMessage(
    messageType: MidiMessageType,
    _channel: number,
    midiNumber: number,
    value: number
  ): ProcessedMessage | null {
    const compositeKey = makeLookupKey(messageType, midiNumber);
    const lookupEntry = this.compositeToMapping.get(compositeKey)
      ?? (messageType === 'cc' ? this.ccToMapping.get(midiNumber) : undefined);

    if (!lookupEntry) {
      return null;
    }

    const { mapping, controlType, isButton, behavior } = lookupEntry;

    let outputValue = value;
    let buttonState: boolean | undefined;

    if (isButton) {
      const processedButton = this.processButtonInput(compositeKey, value, behavior);
      if (processedButton === null) {
        // Button input should be ignored (e.g., release on toggle button)
        return null;
      }
      outputValue = processedButton.outputValue;
      buttonState = processedButton.buttonState;
    }

    const processedMessage: ProcessedMessage = {
      inputCC: midiNumber,
      outputCC: mapping.outputCC,
      channel: mapping.channel,
      value: outputValue,
      controlType,
      isButton,
      buttonState,
      label: mapping.label,
      messageType,
    };

    this.emit('output', processedMessage);

    return processedMessage;
  }

  /**
   * Gets the current state of a toggle button.
   *
   * Looks up using CC composite key first (backward compat with nK2 code
   * that calls getButtonState with a raw CC number).
   *
   * @param inputCC - The input CC number to check
   * @returns True if button is on, false if off or not a tracked button
   */
  getButtonState(inputCC: number): boolean {
    // Try composite key first (handles both CC and Note lookups)
    const ccKey = makeLookupKey('cc', inputCC);
    if (this.buttonStates.has(ccKey)) {
      return this.buttonStates.get(ccKey) ?? false;
    }
    // Fallback: try note key (for devices where buttons are Note-based)
    const noteKey = makeLookupKey('note', inputCC);
    return this.buttonStates.get(noteKey) ?? false;
  }

  /**
   * Sets the state of a button.
   * Used for external state synchronization (e.g., LED sync).
   *
   * @param inputCC - The input CC or Note number
   * @param isOn - The new state
   */
  setButtonState(inputCC: number, isOn: boolean): void {
    // Find the lookup entry using composite keys
    const ccKey = makeLookupKey('cc', inputCC);
    const noteKey = makeLookupKey('note', inputCC);
    let lookupEntry = this.compositeToMapping.get(ccKey) ?? this.compositeToMapping.get(noteKey);

    // Fallback to legacy numeric map
    if (!lookupEntry) {
      lookupEntry = this.ccToMapping.get(inputCC);
    }
    if (!lookupEntry?.isButton) {
      return;
    }

    const stateKey = makeLookupKey(lookupEntry.messageType, inputCC);
    const previousState = this.buttonStates.get(stateKey) ?? false;
    this.buttonStates.set(stateKey, isOn);

    if (previousState !== isOn) {
      const event: ButtonStateChangeEvent = {
        cc: inputCC,
        isOn,
        controlType: lookupEntry.controlType,
      };
      this.emit('buttonStateChanged', event);
    }
  }

  /**
   * Resets all button states to off.
   * Useful when reconnecting to ensure consistent state.
   */
  resetButtonStates(): void {
    for (const [compositeKey, wasOn] of this.buttonStates) {
      if (wasOn) {
        this.buttonStates.set(compositeKey, false);
        const lookupEntry = this.compositeToMapping.get(compositeKey);
        if (lookupEntry) {
          const event: ButtonStateChangeEvent = {
            cc: lookupEntry.mapping.inputCC,
            isOn: false,
            controlType: lookupEntry.controlType,
          };
          this.emit('buttonStateChanged', event);
        }
      }
    }
  }

  /**
   * Gets all button CC numbers that have LEDs.
   *
   * When a device definition is available, uses its LED definitions
   * to determine which buttons have LEDs (works for any device).
   * Without a device definition, falls back to the nK2-specific hasLed() check.
   *
   * @returns Array of input CC/Note numbers for buttons with LEDs
   */
  getButtonCCs(): number[] {
    const buttonCCs: number[] = [];

    if (this.deviceDefinition) {
      // Use the device definition's LED list for correctness across devices
      const ledMidiNumbers = new Set(this.deviceDefinition.leds.map((led) => led.midiNumber));
      for (const [, entry] of this.compositeToMapping) {
        if (entry.isButton && ledMidiNumbers.has(entry.mapping.inputCC)) {
          buttonCCs.push(entry.mapping.inputCC);
        }
      }
    } else {
      // Legacy path: nK2-specific hasLed() check
      for (const [cc, entry] of this.ccToMapping) {
        if (entry.isButton && hasLed(cc)) {
          buttonCCs.push(cc);
        }
      }
    }

    return buttonCCs;
  }

  /**
   * Updates the mapping configuration.
   * Rebuilds the lookup map and preserves button states where possible.
   *
   * @param config - The new mapping configuration
   * @param deviceDefinition - Optional updated device definition
   */
  updateConfig(config: MappingConfig, deviceDefinition?: DeviceDefinition): void {
    // Save current button states for any keys that still exist
    const previousStates = new Map(this.buttonStates);

    this.config = config;
    if (deviceDefinition !== undefined) {
      this.deviceDefinition = deviceDefinition;
    }
    this.buildLookupMap();

    // Restore button states for composite keys that still exist in new config
    for (const [compositeKey, wasOn] of previousStates) {
      if (this.compositeToMapping.has(compositeKey) && this.compositeToMapping.get(compositeKey)?.isButton) {
        this.buttonStates.set(compositeKey, wasOn);
      }
    }
  }

  /**
   * Sets the device definition for message-type-aware mapping.
   * Rebuilds the lookup map to incorporate device-specific message types.
   *
   * @param deviceDefinition - The device definition to use
   */
  setDeviceDefinition(deviceDefinition: DeviceDefinition): void {
    this.deviceDefinition = deviceDefinition;
    this.buildLookupMap();
  }

  /**
   * Gets the current mapping configuration.
   *
   * @returns The current MappingConfig
   */
  getConfig(): MappingConfig {
    return this.config;
  }

  /**
   * Gets the total number of mapped controls.
   *
   * @returns Number of mapped controls (CC + Note)
   */
  getMappingCount(): number {
    return this.compositeToMapping.size || this.ccToMapping.size;
  }

  // ===========================================================================
  // Private Methods
  // ===========================================================================

  /**
   * Builds the fast lookup maps from the configuration.
   *
   * Populates both the legacy numeric ccToMapping (for backward compat)
   * and the new compositeToMapping (for CC/Note disambiguation).
   * When a device definition is available, uses each control's declared
   * messageType. Otherwise defaults to 'cc' for all controls.
   */
  private buildLookupMap(): void {
    this.ccToMapping.clear();
    this.compositeToMapping.clear();
    this.buttonStates.clear();

    // Build a lookup from device definition for per-control message types
    const deviceMessageTypes = this.buildDeviceMessageTypeMap();

    // Process track mappings
    for (let trackIndex = 0; trackIndex < this.config.tracks.length; trackIndex++) {
      const track = this.config.tracks[trackIndex];
      if (!track) continue;

      const trackNumber = trackIndex + 1;
      this.addTrackMappings(track, trackNumber, deviceMessageTypes);
    }

    // Process transport mappings
    this.addTransportMappings(this.config.transport, deviceMessageTypes);
  }

  /**
   * Builds a map from control identifier to its MIDI message type
   * using the device definition. Returns an empty map when no device
   * definition is available, causing all controls to default to 'cc'.
   */
  private buildDeviceMessageTypeMap(): Map<string, MidiMessageType> {
    const messageTypes = new Map<string, MidiMessageType>();

    if (!this.deviceDefinition) {
      return messageTypes;
    }

    // Collect message types from track layouts
    for (let trackIndex = 0; trackIndex < this.deviceDefinition.trackLayouts.length; trackIndex++) {
      const layout = this.deviceDefinition.trackLayouts[trackIndex];
      if (!layout) continue;
      const trackNumber = trackIndex + 1;

      for (const control of layout.continuousControls) {
        messageTypes.set(`track${trackNumber}.${control.name}`, control.messageType);
      }
      for (const button of layout.buttonControls) {
        messageTypes.set(`track${trackNumber}.${button.name}`, button.messageType);
      }
    }

    // Collect message types from extra sections
    for (const section of this.deviceDefinition.extraSections) {
      for (const control of section.controls) {
        messageTypes.set(`${section.name}.${control.name}`, control.messageType);
      }
    }

    return messageTypes;
  }

  /**
   * Registers a single mapping entry into both lookup maps.
   */
  private registerLookupEntry(
    midiNumber: number,
    messageType: MidiMessageType,
    entry: MappingLookupEntry
  ): void {
    const compositeKey = makeLookupKey(messageType, midiNumber);
    this.compositeToMapping.set(compositeKey, entry);

    // Also register in legacy numeric map for backward compat
    this.ccToMapping.set(midiNumber, entry);

    // Initialize button state using composite key
    if (entry.isButton) {
      this.buttonStates.set(compositeKey, false);
    }
  }

  /**
   * Adds all mappings from a track to the lookup maps.
   */
  private addTrackMappings(
    track: TrackMapping,
    trackNumber: number,
    deviceMessageTypes: Map<string, MidiMessageType>
  ): void {
    const controlTypes: ControlType[] = ['knob', 'slider', 'solo', 'mute', 'rec'];

    for (const controlType of controlTypes) {
      const mapping = track[controlType];
      const isButton = !isContinuousControl(controlType);
      const fullControlType = `track${trackNumber}.${controlType}`;

      // Determine message type: mapping entry > device definition > default 'cc'
      const messageType = mapping.messageType
        ?? deviceMessageTypes.get(fullControlType)
        ?? 'cc';

      const entry: MappingLookupEntry = {
        mapping,
        controlType: fullControlType,
        isButton,
        behavior: mapping.behavior,
        messageType,
      };

      this.registerLookupEntry(mapping.inputCC, messageType, entry);
    }
  }

  /**
   * Adds all transport mappings to the lookup maps.
   */
  private addTransportMappings(
    transport: TransportMapping,
    deviceMessageTypes: Map<string, MidiMessageType>
  ): void {
    const transportControls: TransportControlType[] = [
      'rewind', 'forward', 'stop', 'play', 'record', 'cycle',
      'track_left', 'track_right', 'marker_set', 'marker_left', 'marker_right',
    ];

    for (const controlType of transportControls) {
      const mapping = transport[controlType];
      const fullControlType = `transport.${controlType}`;

      // Determine message type: mapping entry > device definition > default 'cc'
      const messageType = mapping.messageType
        ?? deviceMessageTypes.get(fullControlType)
        ?? 'cc';

      const entry: MappingLookupEntry = {
        mapping,
        controlType: fullControlType,
        isButton: true,
        behavior: mapping.behavior,
        messageType,
      };

      this.registerLookupEntry(mapping.inputCC, messageType, entry);
    }
  }

  /**
   * Processes button input according to its behavior mode.
   *
   * Toggle buttons: On value >= 64, flip state and output 127 or 0
   * Momentary buttons: Pass through value (127 on press, 0 on release)
   *
   * @param compositeKey - Composite lookup key (e.g., "cc:32" or "note:1")
   * @param value - Input value (0-127)
   * @param behavior - Button behavior mode
   * @returns Processed value and state, or null if input should be ignored
   */
  private processButtonInput(
    compositeKey: string,
    value: number,
    behavior: ButtonBehavior | undefined
  ): { outputValue: number; buttonState: boolean } | null {
    const isPress = value >= 64;

    if (behavior === 'toggle') {
      // Toggle buttons only respond to press events
      if (!isPress) {
        return null;
      }

      // Flip the state
      const currentState = this.buttonStates.get(compositeKey) ?? false;
      const newState = !currentState;
      this.buttonStates.set(compositeKey, newState);

      // Emit state change event
      const lookupEntry = this.compositeToMapping.get(compositeKey);
      if (lookupEntry) {
        const event: ButtonStateChangeEvent = {
          cc: lookupEntry.mapping.inputCC,
          isOn: newState,
          controlType: lookupEntry.controlType,
        };
        this.emit('buttonStateChanged', event);
      }

      return {
        outputValue: newState ? MIDI_VALUE_ON : MIDI_VALUE_OFF,
        buttonState: newState,
      };
    }

    // Momentary behavior: pass through with normalized values
    const outputValue = isPress ? MIDI_VALUE_ON : MIDI_VALUE_OFF;
    const buttonState = isPress;

    // Update button state (for LED tracking on momentary buttons)
    this.buttonStates.set(compositeKey, buttonState);

    // Emit state change for momentary buttons too (for LED feedback)
    const lookupEntry = this.compositeToMapping.get(compositeKey);
    if (lookupEntry) {
      const event: ButtonStateChangeEvent = {
        cc: lookupEntry.mapping.inputCC,
        isOn: buttonState,
        controlType: lookupEntry.controlType,
      };
      this.emit('buttonStateChanged', event);
    }

    return {
      outputValue,
      buttonState,
    };
  }
}
