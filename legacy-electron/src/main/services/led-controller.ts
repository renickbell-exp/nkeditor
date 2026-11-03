/**
 * LED Controller Module
 *
 * Manages LED states on the nanoKONTROL2 hardware controller.
 * LEDs are controlled by sending CC messages to the hardware output port.
 *
 * Key responsibilities:
 * - Track LED states for all buttons with LEDs
 * - Send LED update messages to hardware
 * - Synchronize LED states with button states from the mapping engine
 * - Turn all LEDs off on disconnect/shutdown
 *
 * LED Protocol:
 * - LEDs use the same CC numbers as their corresponding buttons
 * - Value 127 = LED on, Value 0 = LED off
 * - Messages are sent on MIDI channel 1 (0-indexed: channel 0)
 */

import type { MidiHandler } from './midi-handler.js';
import type { MappingEngine } from './mapping-engine.js';
import { hasLed, MIDI_VALUE_ON, MIDI_VALUE_OFF } from '@shared/types.js';
import type { DeviceDefinition, LedDef } from '@shared/devices/device-definition.js';

// =============================================================================
// LedController Class
// =============================================================================

/**
 * LedController manages the LED states on the nanoKONTROL2.
 *
 * Provides methods to set individual LEDs, sync all LEDs from the
 * mapping engine's button states, and turn all LEDs off.
 *
 * @example
 * const ledController = new LedController(midiHandler);
 *
 * // Set individual LED
 * ledController.setLed(32, true);  // Turn on Solo button 1 LED
 *
 * // Sync all LEDs from engine state
 * ledController.syncFromEngine(mappingEngine);
 *
 * // Turn all LEDs off (e.g., on shutdown)
 * ledController.allOff();
 */
export class LedController {
  /** MIDI handler for sending LED messages to hardware */
  private readonly midiHandler: MidiHandler;

  /**
   * Current LED states keyed by button name when a device definition is
   * available, or by CC number string when in legacy mode.
   * Using string keys provides a uniform interface for both modes.
   */
  private ledStates: Map<string, boolean> = new Map();

  /**
   * Optional device definition for device-aware LED control.
   * When present, uses the device's LED protocol (CC or Note)
   * and its LED definitions for addressing.
   */
  private deviceDefinition: DeviceDefinition | null = null;

  /**
   * Creates a new LedController.
   *
   * @param midiHandler - The MidiHandler instance for hardware communication
   * @param deviceDefinition - Optional device definition for protocol-aware LED control
   */
  constructor(midiHandler: MidiHandler, deviceDefinition?: DeviceDefinition) {
    this.midiHandler = midiHandler;
    this.deviceDefinition = deviceDefinition ?? null;
  }

  /**
   * Sets the device definition, enabling device-aware LED control.
   * Call this when the connected device is identified.
   *
   * @param deviceDefinition - The device definition to use
   */
  setDeviceDefinition(deviceDefinition: DeviceDefinition): void {
    this.deviceDefinition = deviceDefinition;
  }

  /**
   * Sets the state of a single LED and sends the update to hardware.
   *
   * When a device definition is available, uses the device's LED protocol
   * and definitions. Otherwise falls back to nK2-specific CC-based behavior.
   *
   * @param cc - The CC or Note number for the LED (same as button MIDI number)
   * @param isOn - True to turn LED on, false to turn off
   */
  setLed(cc: number, isOn: boolean): void {
    if (this.deviceDefinition) {
      // Device-aware mode: find the LED definition for this MIDI number
      const ledDef = this.findLedByMidiNumber(cc);
      if (!ledDef) {
        return;
      }

      const stateKey = ledDef.buttonName;
      const currentState = this.ledStates.get(stateKey);
      if (currentState === isOn) {
        return;
      }

      this.ledStates.set(stateKey, isOn);
      this.sendLedUpdateForDevice(ledDef, isOn);
    } else {
      // Legacy mode: nK2-specific CC-only behavior
      if (!hasLed(cc)) {
        return;
      }

      const stateKey = String(cc);
      const currentState = this.ledStates.get(stateKey);
      if (currentState === isOn) {
        return;
      }

      this.ledStates.set(stateKey, isOn);
      this.sendLedUpdate(cc, isOn);
    }
  }

  /**
   * Synchronizes all LED states from the mapping engine's button states.
   *
   * Call this when:
   * - Initially connecting to the device
   * - After the device reconnects
   * - When loading a new mapping configuration
   *
   * @param engine - The MappingEngine to read button states from
   */
  syncFromEngine(engine: MappingEngine): void {
    const buttonCCs = engine.getButtonCCs();

    for (const cc of buttonCCs) {
      const isOn = engine.getButtonState(cc);
      this.setLed(cc, isOn);
    }
  }

  /**
   * Turns all tracked LEDs off.
   *
   * Iterates through all known LED definitions and sets them to off.
   * Call this when disconnecting or shutting down to leave
   * the hardware in a clean state.
   */
  allOff(): void {
    if (this.deviceDefinition) {
      // Device-aware mode: turn off all LEDs defined by the device
      for (const ledDef of this.deviceDefinition.leds) {
        this.sendLedUpdateForDevice(ledDef, false);
      }
    } else {
      // Legacy mode: turn off all tracked + known nK2 LEDs
      for (const stateKey of this.ledStates.keys()) {
        const cc = parseInt(stateKey, 10);
        if (!isNaN(cc)) {
          this.sendLedUpdate(cc, false);
        }
      }

      const allLedCCs = this.getAllLedCCs();
      for (const cc of allLedCCs) {
        this.sendLedUpdate(cc, false);
      }
    }

    this.ledStates.clear();
  }

  /**
   * Gets the current state of an LED.
   *
   * @param cc - The CC or Note number to check
   * @returns True if LED is on, false if off or not tracked
   */
  getLedState(cc: number): boolean {
    if (this.deviceDefinition) {
      // Device-aware mode: find the button name for this MIDI number
      const ledDef = this.findLedByMidiNumber(cc);
      if (!ledDef) {
        return false;
      }
      return this.ledStates.get(ledDef.buttonName) ?? false;
    }
    // Legacy mode: numeric string key
    return this.ledStates.get(String(cc)) ?? false;
  }

  /**
   * Gets all currently tracked LED state keys.
   *
   * In device-aware mode, returns button names (e.g., "track1.solo").
   * In legacy mode, returns CC number strings.
   *
   * @returns Array of tracked LED state keys
   */
  getTrackedKeys(): string[] {
    return Array.from(this.ledStates.keys());
  }

  /**
   * Gets all currently tracked LED CC numbers.
   * Backward compatible: returns numeric CC values when possible.
   *
   * @returns Array of CC numbers with tracked LED states
   */
  getTrackedCCs(): number[] {
    if (this.deviceDefinition) {
      // In device-aware mode, extract MIDI numbers from LED definitions
      const trackedNames = new Set(this.ledStates.keys());
      return this.deviceDefinition.leds
        .filter((led) => trackedNames.has(led.buttonName))
        .map((led) => led.midiNumber);
    }
    // Legacy mode: parse numeric keys
    return Array.from(this.ledStates.keys())
      .map((key) => parseInt(key, 10))
      .filter((cc) => !isNaN(cc));
  }

  /**
   * Forces an LED update to hardware without checking current state.
   *
   * Useful for ensuring hardware state matches expected state
   * after reconnection or when state may be inconsistent.
   *
   * @param cc - The CC or Note number for the LED
   * @param isOn - Desired LED state
   */
  forceUpdate(cc: number, isOn: boolean): void {
    if (this.deviceDefinition) {
      const ledDef = this.findLedByMidiNumber(cc);
      if (!ledDef) {
        return;
      }
      this.ledStates.set(ledDef.buttonName, isOn);
      this.sendLedUpdateForDevice(ledDef, isOn);
    } else {
      if (!hasLed(cc)) {
        return;
      }
      this.ledStates.set(String(cc), isOn);
      this.sendLedUpdate(cc, isOn);
    }
  }

  // ===========================================================================
  // Private Methods
  // ===========================================================================

  /**
   * Sends an LED update to the hardware via the MIDI handler using CC messages.
   * This is the legacy nK2-specific path.
   *
   * @param cc - The CC number for the LED
   * @param isOn - Desired LED state
   */
  private sendLedUpdate(cc: number, isOn: boolean): void {
    const value = isOn ? MIDI_VALUE_ON : MIDI_VALUE_OFF;
    this.midiHandler.sendLedCC(cc, value);
  }

  /**
   * Sends an LED update using the device definition's protocol.
   * Routes to CC or Note output based on the device's LED protocol.
   *
   * @param ledDef - The LED definition describing which MIDI number to target
   * @param isOn - Desired LED state
   */
  private sendLedUpdateForDevice(ledDef: LedDef, isOn: boolean): void {
    if (!this.deviceDefinition) {
      return;
    }

    const { ledProtocol } = this.deviceDefinition;
    const value = isOn ? ledProtocol.onValue : ledProtocol.offValue;
    // LED channel is 1-indexed in the device definition, easymidi expects 0-indexed
    const channel = ledProtocol.channel - 1;

    if (ledProtocol.messageType === 'note') {
      this.midiHandler.sendLedNote(channel, ledDef.midiNumber, value);
    } else {
      this.midiHandler.sendLedCC(ledDef.midiNumber, value);
    }
  }

  /**
   * Finds an LED definition by its MIDI number in the device definition.
   *
   * @param midiNumber - The MIDI CC or Note number to search for
   * @returns The matching LedDef, or undefined if not found
   */
  private findLedByMidiNumber(midiNumber: number): LedDef | undefined {
    if (!this.deviceDefinition) {
      return undefined;
    }
    return this.deviceDefinition.leds.find((led) => led.midiNumber === midiNumber);
  }

  /**
   * Gets all CC numbers that have LEDs on the nanoKONTROL2.
   * Used in legacy mode when no device definition is available.
   *
   * @returns Array of all LED CC numbers
   */
  private getAllLedCCs(): number[] {
    return [
      // Solo buttons (Track 1-8)
      32, 33, 34, 35, 36, 37, 38, 39,
      // Mute buttons (Track 1-8)
      48, 49, 50, 51, 52, 53, 54, 55,
      // Record buttons (Track 1-8)
      64, 65, 66, 67, 68, 69, 70, 71,
      // Transport buttons with LEDs (Play, Stop, Record, Cycle)
      41, 42, 45, 46,
    ];
  }
}
