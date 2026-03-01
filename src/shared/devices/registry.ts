/**
 * Device Registry
 *
 * Central registry for all supported MIDI controller definitions.
 * Provides lookup by device ID, auto-detection from MIDI port names,
 * and enumeration of all registered devices.
 *
 * The nanoKONTROL2 is the default device for backward compatibility
 * with existing configurations that predate multi-device support.
 */

import type { DeviceDefinition } from './device-definition.js';
import { NANOKONTROL2_DEFINITION } from './nanokontrol2.js';
import { AKAI_MIDIMIX_DEFINITION } from './akai-midimix.js';

// =============================================================================
// Registry
// =============================================================================

const DEVICE_REGISTRY: Map<string, DeviceDefinition> = new Map([
  [NANOKONTROL2_DEFINITION.id, NANOKONTROL2_DEFINITION],
  [AKAI_MIDIMIX_DEFINITION.id, AKAI_MIDIMIX_DEFINITION],
]);

/** The default device ID, used when no specific device is identified */
const DEFAULT_DEVICE_ID = 'nanokontrol2';

// =============================================================================
// Public API
// =============================================================================

/**
 * Retrieves a device definition by its unique ID.
 *
 * @param id - Device identifier (e.g., 'nanokontrol2', 'akai-midimix')
 * @returns The device definition, or undefined if not found
 */
export function getDeviceDefinition(id: string): DeviceDefinition | undefined {
  return DEVICE_REGISTRY.get(id);
}

/**
 * Returns all registered device definitions.
 * Order is not guaranteed across calls.
 */
export function getAllDeviceDefinitions(): DeviceDefinition[] {
  return Array.from(DEVICE_REGISTRY.values());
}

/**
 * Identifies a device by matching a MIDI port name against registered
 * port patterns. Returns the first matching device definition.
 *
 * Port names are matched against each device's portPatterns array.
 * The first device with a matching pattern wins, so pattern specificity
 * matters when devices could share similar port names.
 *
 * @param portName - The MIDI port name as reported by the operating system
 * @returns The matching device definition, or null if no device matches
 */
export function identifyDevice(portName: string): DeviceDefinition | null {
  for (const device of DEVICE_REGISTRY.values()) {
    const isMatch = device.portPatterns.some((pattern) => pattern.test(portName));
    if (isMatch) {
      return device;
    }
  }
  return null;
}

/**
 * Returns the default device definition (nanoKONTROL2).
 * Used as a fallback when no device has been explicitly selected or detected.
 */
export function getDefaultDevice(): DeviceDefinition {
  const defaultDevice = DEVICE_REGISTRY.get(DEFAULT_DEVICE_ID);
  if (!defaultDevice) {
    // This is a programming error: the default device must always exist in the registry
    throw new Error(
      `Default device '${DEFAULT_DEVICE_ID}' is missing from the device registry. ` +
      'This indicates a broken build or missing device definition module.',
    );
  }
  return defaultDevice;
}
