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
import { hasLed, MIDI_VALUE_ON, MIDI_VALUE_OFF } from './types.js';

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

  /** Current LED states: CC number -> isOn */
  private ledStates: Map<number, boolean> = new Map();

  /**
   * Creates a new LedController.
   *
   * @param midiHandler - The MidiHandler instance for hardware communication
   */
  constructor(midiHandler: MidiHandler) {
    this.midiHandler = midiHandler;
  }

  /**
   * Sets the state of a single LED and sends the update to hardware.
   *
   * Only CCs that correspond to buttons with LEDs will be sent.
   * The state is tracked internally to avoid redundant hardware updates.
   *
   * @param cc - The CC number for the LED (same as button CC)
   * @param isOn - True to turn LED on, false to turn off
   */
  setLed(cc: number, isOn: boolean): void {
    // Only process CCs that have LEDs
    if (!hasLed(cc)) {
      return;
    }

    // Check if state actually changed to avoid redundant updates
    const currentState = this.ledStates.get(cc);
    if (currentState === isOn) {
      return;
    }

    // Update internal state
    this.ledStates.set(cc, isOn);

    // Send update to hardware
    this.sendLedUpdate(cc, isOn);
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
   * Iterates through all known LED CCs and sets them to off.
   * Call this when disconnecting or shutting down to leave
   * the hardware in a clean state.
   */
  allOff(): void {
    // Turn off all tracked LEDs
    for (const cc of this.ledStates.keys()) {
      this.ledStates.set(cc, false);
      this.sendLedUpdate(cc, false);
    }

    // Also turn off all possible LED CCs even if not tracked
    // This ensures a clean state even if some LEDs were set externally
    const allLedCCs = this.getAllLedCCs();
    for (const cc of allLedCCs) {
      this.sendLedUpdate(cc, false);
    }

    // Clear internal state
    this.ledStates.clear();
  }

  /**
   * Gets the current state of an LED.
   *
   * @param cc - The CC number to check
   * @returns True if LED is on, false if off or not tracked
   */
  getLedState(cc: number): boolean {
    return this.ledStates.get(cc) ?? false;
  }

  /**
   * Gets all currently tracked LED CC numbers.
   *
   * @returns Array of CC numbers with tracked LED states
   */
  getTrackedCCs(): number[] {
    return Array.from(this.ledStates.keys());
  }

  /**
   * Forces an LED update to hardware without checking current state.
   *
   * Useful for ensuring hardware state matches expected state
   * after reconnection or when state may be inconsistent.
   *
   * @param cc - The CC number for the LED
   * @param isOn - Desired LED state
   */
  forceUpdate(cc: number, isOn: boolean): void {
    if (!hasLed(cc)) {
      return;
    }

    this.ledStates.set(cc, isOn);
    this.sendLedUpdate(cc, isOn);
  }

  // ===========================================================================
  // Private Methods
  // ===========================================================================

  /**
   * Sends an LED update to the hardware via the MIDI handler.
   *
   * @param cc - The CC number for the LED
   * @param isOn - Desired LED state
   */
  private sendLedUpdate(cc: number, isOn: boolean): void {
    const value = isOn ? MIDI_VALUE_ON : MIDI_VALUE_OFF;
    this.midiHandler.sendLedCC(cc, value);
  }

  /**
   * Gets all CC numbers that have LEDs on the nanoKONTROL2.
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
