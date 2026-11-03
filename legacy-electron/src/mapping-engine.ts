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
} from './types.js';
import { MIDI_VALUE_ON, MIDI_VALUE_OFF, hasLed } from './types.js';
import { isContinuousControl } from './constants.js';

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
 * Internal structure for fast CC lookup.
 * Maps input CC to its mapping entry and metadata.
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

  /** Button toggle states: inputCC -> isOn */
  private buttonStates: Map<number, boolean> = new Map();

  /** Fast lookup map: inputCC -> mapping info */
  private ccToMapping: Map<number, MappingLookupEntry> = new Map();

  /**
   * Creates a new MappingEngine with the given configuration.
   *
   * @param config - The mapping configuration to use
   */
  constructor(config: MappingConfig) {
    super();
    this.config = config;
    this.buildLookupMap();
  }

  /**
   * Processes an incoming CC message from hardware.
   *
   * Looks up the mapping for the input CC, applies any transformations
   * (toggle behavior, channel remapping), and emits the processed result.
   *
   * @param channel - Input MIDI channel (0-indexed from easymidi)
   * @param cc - Input CC number (0-127)
   * @param value - Input CC value (0-127)
   * @returns ProcessedMessage if CC is mapped, null if not mapped
   */
  processCC(_channel: number, cc: number, value: number): ProcessedMessage | null {
    const lookupEntry = this.ccToMapping.get(cc);

    if (!lookupEntry) {
      // CC is not mapped - ignore it
      return null;
    }

    const { mapping, controlType, isButton, behavior } = lookupEntry;

    let outputValue = value;
    let buttonState: boolean | undefined;

    if (isButton) {
      const processedButton = this.processButtonInput(cc, value, behavior);
      if (processedButton === null) {
        // Button input should be ignored (e.g., release on toggle button)
        return null;
      }
      outputValue = processedButton.outputValue;
      buttonState = processedButton.buttonState;
    }

    const processedMessage: ProcessedMessage = {
      inputCC: cc,
      outputCC: mapping.outputCC,
      channel: mapping.channel,
      value: outputValue,
      controlType,
      isButton,
      buttonState,
      label: mapping.label,
    };

    // Emit the processed message
    this.emit('output', processedMessage);

    return processedMessage;
  }

  /**
   * Gets the current state of a toggle button.
   *
   * @param inputCC - The input CC number to check
   * @returns True if button is on, false if off or not a tracked button
   */
  getButtonState(inputCC: number): boolean {
    return this.buttonStates.get(inputCC) ?? false;
  }

  /**
   * Sets the state of a button.
   * Used for external state synchronization (e.g., LED sync).
   *
   * @param inputCC - The input CC number
   * @param isOn - The new state
   */
  setButtonState(inputCC: number, isOn: boolean): void {
    const lookupEntry = this.ccToMapping.get(inputCC);
    if (!lookupEntry?.isButton) {
      return;
    }

    const previousState = this.buttonStates.get(inputCC) ?? false;
    this.buttonStates.set(inputCC, isOn);

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
    for (const [cc, wasOn] of this.buttonStates) {
      if (wasOn) {
        this.buttonStates.set(cc, false);
        const lookupEntry = this.ccToMapping.get(cc);
        if (lookupEntry) {
          const event: ButtonStateChangeEvent = {
            cc,
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
   * @returns Array of input CC numbers for buttons with LEDs
   */
  getButtonCCs(): number[] {
    const buttonCCs: number[] = [];

    for (const [cc, entry] of this.ccToMapping) {
      if (entry.isButton && hasLed(cc)) {
        buttonCCs.push(cc);
      }
    }

    return buttonCCs;
  }

  /**
   * Updates the mapping configuration.
   * Rebuilds the lookup map and preserves button states where possible.
   *
   * @param config - The new mapping configuration
   */
  updateConfig(config: MappingConfig): void {
    // Save current button states for any CCs that still exist
    const previousStates = new Map(this.buttonStates);

    this.config = config;
    this.buildLookupMap();

    // Restore button states for CCs that still exist in new config
    for (const [cc, wasOn] of previousStates) {
      if (this.ccToMapping.has(cc) && this.ccToMapping.get(cc)?.isButton) {
        this.buttonStates.set(cc, wasOn);
      }
    }
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
   * @returns Number of CC mappings
   */
  getMappingCount(): number {
    return this.ccToMapping.size;
  }

  // ===========================================================================
  // Private Methods
  // ===========================================================================

  /**
   * Builds the fast lookup map from the configuration.
   * Creates O(1) lookup from input CC to mapping info.
   */
  private buildLookupMap(): void {
    this.ccToMapping.clear();
    this.buttonStates.clear();

    // Process track mappings
    for (let trackIndex = 0; trackIndex < this.config.tracks.length; trackIndex++) {
      const track = this.config.tracks[trackIndex];
      if (!track) continue;

      const trackNumber = trackIndex + 1;
      this.addTrackMappings(track, trackNumber);
    }

    // Process transport mappings
    this.addTransportMappings(this.config.transport);
  }

  /**
   * Adds all mappings from a track to the lookup map.
   */
  private addTrackMappings(track: TrackMapping, trackNumber: number): void {
    const controlTypes: ControlType[] = ['knob', 'slider', 'solo', 'mute', 'rec'];

    for (const controlType of controlTypes) {
      const mapping = track[controlType];
      const isButton = !isContinuousControl(controlType);
      const fullControlType = `track${trackNumber}.${controlType}`;

      this.ccToMapping.set(mapping.inputCC, {
        mapping,
        controlType: fullControlType,
        isButton,
        behavior: mapping.behavior,
      });

      // Initialize button state to off
      if (isButton) {
        this.buttonStates.set(mapping.inputCC, false);
      }
    }
  }

  /**
   * Adds all transport mappings to the lookup map.
   */
  private addTransportMappings(transport: TransportMapping): void {
    const transportControls: TransportControlType[] = [
      'rewind', 'forward', 'stop', 'play', 'record', 'cycle',
      'track_left', 'track_right', 'marker_set', 'marker_left', 'marker_right',
    ];

    for (const controlType of transportControls) {
      const mapping = transport[controlType];
      const fullControlType = `transport.${controlType}`;

      // All transport controls are buttons
      this.ccToMapping.set(mapping.inputCC, {
        mapping,
        controlType: fullControlType,
        isButton: true,
        behavior: mapping.behavior,
      });

      // Initialize button state to off
      this.buttonStates.set(mapping.inputCC, false);
    }
  }

  /**
   * Processes button input according to its behavior mode.
   *
   * Toggle buttons: On value >= 64, flip state and output 127 or 0
   * Momentary buttons: Pass through value (127 on press, 0 on release)
   *
   * @param cc - Button CC number
   * @param value - Input value (0-127)
   * @param behavior - Button behavior mode
   * @returns Processed value and state, or null if input should be ignored
   */
  private processButtonInput(
    cc: number,
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
      const currentState = this.buttonStates.get(cc) ?? false;
      const newState = !currentState;
      this.buttonStates.set(cc, newState);

      // Emit state change event
      const lookupEntry = this.ccToMapping.get(cc);
      if (lookupEntry) {
        const event: ButtonStateChangeEvent = {
          cc,
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
    this.buttonStates.set(cc, buttonState);

    // Emit state change for momentary buttons too (for LED feedback)
    const lookupEntry = this.ccToMapping.get(cc);
    if (lookupEntry) {
      const event: ButtonStateChangeEvent = {
        cc,
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
