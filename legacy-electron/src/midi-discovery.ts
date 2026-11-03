/**
 * MIDI Port Discovery Module
 *
 * Handles detection and enumeration of MIDI ports on Linux systems.
 * Specifically designed to locate the Korg nanoKONTROL2 controller.
 *
 * Linux port names typically follow the format:
 * "nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0"
 *
 * The pattern matches variations in case and spacing that may occur
 * across different ALSA/PipeWire configurations.
 *
 * This module handles MIDI subsystem failures gracefully, allowing the
 * application to run in "demo mode" when MIDI hardware or drivers are
 * unavailable (e.g., ALSA sequencer initialization failure).
 */

import easymidi from 'easymidi';
import type { MidiPorts, NanoKontrol2Ports } from './types.js';

/**
 * Regular expression pattern to match nanoKONTROL2 MIDI ports on Linux.
 *
 * Matches port names like:
 * - "nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0" (ALSA direct)
 * - "nanoKONTROL2:nanoKONTROL2 MIDI 1 28:0" (ALSA direct)
 * - "NANOKONTROL2:nanoKONTROL2 MIDI 1 24:0" (case variations)
 * - "nanoKONTROL2:nanoKONTROL2 nanoKONTROL2 _ CTR 20:0" (PipeWire naming)
 *
 * The pattern is case-insensitive and matches any port starting with "nanokontrol2".
 */
const NANOKONTROL2_PATTERN = /^nanokontrol2[:\s]/i;

// =============================================================================
// MIDI Subsystem Availability Tracking
// =============================================================================

/**
 * Tracks whether the MIDI subsystem has been confirmed unavailable.
 * Once set to true, we avoid repeated initialization attempts and log spam.
 * Can be reset via resetMidiAvailability() to retry initialization.
 */
let midiSubsystemUnavailable = false;

/**
 * Stores the error message from the last MIDI initialization failure.
 * Useful for diagnostics without re-triggering the failure.
 */
let lastMidiError: string | null = null;

/**
 * Tracks whether we've already logged the MIDI unavailability message.
 * Prevents log spam when getAvailablePorts() is called repeatedly.
 */
let hasLoggedUnavailability = false;

/**
 * Empty ports object returned when MIDI subsystem is unavailable.
 * Defined as a constant to avoid repeated object allocation.
 */
const EMPTY_PORTS: MidiPorts = Object.freeze({
  inputs: [],
  outputs: [],
});

// =============================================================================
// Port Enumeration Cache
// =============================================================================
//
// IMPORTANT: easymidi's getInputs() and getOutputs() functions create a new
// RtMidi/ALSA sequencer client on every call. While the client is closed
// after enumeration, rapid successive calls can exhaust available ALSA
// client slots before the system can reclaim them. This causes the dreaded:
//   "error creating ALSA sequencer client object" / "Cannot allocate memory"
//
// The cache prevents this by rate-limiting calls to easymidi's port enumeration.
// Cache duration is set to allow hotplug detection to still function (2000ms polls)
// while preventing rapid-fire calls during startup or error recovery.
// =============================================================================

/**
 * Duration in milliseconds to cache port enumeration results.
 * Set to 500ms as a balance between responsiveness and resource protection.
 * Hotplug detection polls at 2000ms intervals, so this won't affect normal operation.
 */
const PORT_CACHE_DURATION_MS = 500;

/**
 * Cached port enumeration results.
 * Stored as a frozen object to prevent accidental mutation.
 */
let cachedPorts: MidiPorts | null = null;

/**
 * Timestamp when the port cache was last populated.
 * Used to determine cache validity.
 */
let portCacheTimestamp = 0;

/**
 * Checks if the MIDI subsystem is currently available.
 *
 * This function uses cached results from getAvailablePorts() to avoid
 * creating additional ALSA sequencer clients. If no cached data is available,
 * it triggers a port enumeration.
 *
 * If the subsystem has previously failed, it returns false immediately
 * without retrying (to avoid performance impact and log spam).
 *
 * Use resetMidiAvailability() to clear the failure state and allow retry.
 *
 * @returns True if MIDI subsystem is available, false otherwise
 *
 * @example
 * if (isMidiAvailable()) {
 *   const ports = getAvailablePorts();
 *   // ... work with MIDI
 * } else {
 *   console.log('Running in demo mode - MIDI unavailable');
 * }
 */
export function isMidiAvailable(): boolean {
  // If we've already determined MIDI is unavailable, return cached result
  if (midiSubsystemUnavailable) {
    return false;
  }

  // Use getAvailablePorts() which has caching built in.
  // This avoids creating an additional ALSA client just for the availability check.
  // If getAvailablePorts() succeeds, MIDI is available.
  // If it fails, it sets midiSubsystemUnavailable = true internally.
  getAvailablePorts();
  return !midiSubsystemUnavailable;
}

/**
 * Retrieves the error message from the last MIDI initialization failure.
 *
 * Returns null if MIDI has not failed or if no error was recorded.
 * Useful for displaying diagnostics to users without triggering another
 * initialization attempt.
 *
 * @returns The last error message, or null if no failure recorded
 */
export function getLastMidiError(): string | null {
  return lastMidiError;
}

/**
 * Resets the MIDI availability state, allowing re-initialization attempts.
 *
 * Call this when conditions may have changed (e.g., user connected hardware,
 * audio service restarted) and you want to retry MIDI initialization.
 *
 * After calling this, the next call to getAvailablePorts() or isMidiAvailable()
 * will attempt to initialize the MIDI subsystem again.
 *
 * @example
 * // User clicked "Retry MIDI" button
 * resetMidiAvailability();
 * if (isMidiAvailable()) {
 *   console.log('MIDI is now available!');
 * }
 */
export function resetMidiAvailability(): void {
  midiSubsystemUnavailable = false;
  lastMidiError = null;
  hasLoggedUnavailability = false;
  // Clear the port cache to force fresh enumeration
  cachedPorts = null;
  portCacheTimestamp = 0;
}

/**
 * Retrieves all available MIDI input and output port names on the system.
 *
 * Uses easymidi's port enumeration which wraps the underlying ALSA/RtMidi
 * functionality. Port names are returned as-is from the system.
 *
 * IMPORTANT: Results are cached for PORT_CACHE_DURATION_MS to prevent
 * rapid successive calls from exhausting ALSA sequencer client slots.
 * Each call to easymidi's getInputs()/getOutputs() creates a new ALSA
 * client, and rapid creation/destruction can overwhelm the system.
 *
 * If the MIDI subsystem is unavailable (e.g., ALSA initialization failure),
 * this function returns empty arrays instead of throwing. The error is logged
 * once to avoid spam on repeated calls.
 *
 * @returns Object containing arrays of input and output port names
 *
 * @example
 * const ports = getAvailablePorts();
 * console.log('Inputs:', ports.inputs);
 * // ['nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0', 'Midi Through:Midi Through Port-0 14:0']
 * console.log('Outputs:', ports.outputs);
 * // ['nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0', 'Midi Through:Midi Through Port-0 14:0']
 *
 * @example
 * // When MIDI is unavailable, returns empty arrays without throwing
 * const ports = getAvailablePorts();
 * if (ports.inputs.length === 0 && ports.outputs.length === 0) {
 *   if (!isMidiAvailable()) {
 *     console.log('MIDI subsystem unavailable');
 *   } else {
 *     console.log('No MIDI devices connected');
 *   }
 * }
 */
export function getAvailablePorts(): MidiPorts {
  // Fast path: if MIDI subsystem is already known to be unavailable,
  // return empty ports immediately without attempting initialization
  if (midiSubsystemUnavailable) {
    return EMPTY_PORTS;
  }

  // Check if we have valid cached data
  const now = Date.now();
  const cacheAge = now - portCacheTimestamp;
  const cacheIsValid = cachedPorts !== null && cacheAge < PORT_CACHE_DURATION_MS;

  if (cacheIsValid && cachedPorts !== null) {
    return cachedPorts;
  }

  // Cache is stale or empty - perform actual enumeration
  try {
    const inputs = easymidi.getInputs();
    const outputs = easymidi.getOutputs();

    // Create and cache the result as a frozen object
    cachedPorts = Object.freeze({
      inputs: inputs ?? [],
      outputs: outputs ?? [],
    });
    portCacheTimestamp = now;

    return cachedPorts;
  } catch (error) {
    // Extract error message for logging and diagnostics
    const errorMessage = error instanceof Error ? error.message : String(error);

    // Store the error for later retrieval via getLastMidiError()
    lastMidiError = errorMessage;

    // Mark MIDI subsystem as unavailable to prevent repeated initialization attempts
    midiSubsystemUnavailable = true;

    // Log the error only once to avoid spam during hotplug polling
    if (!hasLoggedUnavailability) {
      hasLoggedUnavailability = true;
      console.error(
        `[midi-discovery] MIDI subsystem unavailable: ${errorMessage}. ` +
        `Application will continue in demo mode without MIDI functionality. ` +
        `Call resetMidiAvailability() to retry.`
      );
    }

    return EMPTY_PORTS;
  }
}

/**
 * Invalidates the port cache, forcing the next call to getAvailablePorts()
 * to perform a fresh enumeration.
 *
 * This is useful when you know ports have changed (e.g., after a device
 * connect/disconnect) and want fresh data immediately without waiting
 * for the cache to expire.
 *
 * Note: This does NOT reset the MIDI subsystem unavailability state.
 * Use resetMidiAvailability() for that purpose.
 */
export function invalidatePortCache(): void {
  cachedPorts = null;
  portCacheTimestamp = 0;
}

/**
 * Searches for a nanoKONTROL2 device among available MIDI ports.
 *
 * Scans both input and output ports for names matching the nanoKONTROL2 pattern.
 * Returns the first matching port found for each direction (input/output).
 *
 * The nanoKONTROL2 typically appears as both an input and output port with
 * the same or very similar names. Both are needed for full functionality:
 * - Input: Receives CC messages from physical controls
 * - Output: Sends CC messages to control LEDs
 *
 * @returns Object with input and output port names, or null if not found
 *
 * @example
 * const device = findNanoKontrol2();
 * if (device.input && device.output) {
 *   console.log('nanoKONTROL2 found!');
 *   console.log('Input:', device.input);
 *   console.log('Output:', device.output);
 * } else {
 *   console.log('nanoKONTROL2 not connected');
 * }
 */
export function findNanoKontrol2(): NanoKontrol2Ports {
  const ports = getAvailablePorts();

  const inputPort = findMatchingPort(ports.inputs);
  const outputPort = findMatchingPort(ports.outputs);

  return {
    input: inputPort,
    output: outputPort,
  };
}

/**
 * Searches a list of port names for one matching the nanoKONTROL2 pattern.
 *
 * @param portNames - Array of port names to search
 * @returns The first matching port name, or null if none found
 */
function findMatchingPort(portNames: string[]): string | null {
  for (const portName of portNames) {
    if (NANOKONTROL2_PATTERN.test(portName)) {
      return portName;
    }
  }
  return null;
}

/**
 * Checks if the given port name matches the nanoKONTROL2 pattern.
 *
 * Useful for validating user-specified port names or for filtering
 * port lists in UI components.
 *
 * @param portName - The port name to check
 * @returns True if the port name matches the nanoKONTROL2 pattern
 *
 * @example
 * if (isNanoKontrol2Port(userSpecifiedPort)) {
 *   console.log('Valid nanoKONTROL2 port');
 * }
 */
export function isNanoKontrol2Port(portName: string): boolean {
  return NANOKONTROL2_PATTERN.test(portName);
}

/**
 * Formats port information for display in CLI output.
 *
 * Creates a human-readable summary of available ports, useful for
 * the --list-ports CLI command. If the MIDI subsystem is unavailable,
 * displays a helpful message indicating demo mode.
 *
 * @param ports - The available ports to format
 * @returns Formatted string for display
 *
 * @example
 * const ports = getAvailablePorts();
 * console.log(formatPortsForDisplay(ports));
 * // MIDI Input Ports:
 * //   1. nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0
 * //   2. Midi Through:Midi Through Port-0 14:0
 * //
 * // MIDI Output Ports:
 * //   1. nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0
 * //   2. Midi Through:Midi Through Port-0 14:0
 */
export function formatPortsForDisplay(ports: MidiPorts): string {
  const lines: string[] = [];

  // Check if MIDI subsystem is unavailable and show appropriate message
  if (midiSubsystemUnavailable) {
    lines.push('MIDI Subsystem Status: UNAVAILABLE');
    lines.push('');
    lines.push('The MIDI subsystem could not be initialized.');
    if (lastMidiError) {
      lines.push(`Reason: ${lastMidiError}`);
    }
    lines.push('');
    lines.push('The application is running in demo mode without MIDI functionality.');
    lines.push('');
    lines.push('Possible causes:');
    lines.push('  - ALSA sequencer device not available (/dev/snd/seq)');
    lines.push('  - Insufficient system memory for MIDI subsystem');
    lines.push('  - Audio server (PipeWire/PulseAudio) not running');
    lines.push('  - Missing permissions for audio devices');
    lines.push('');
    lines.push('To retry MIDI initialization, restart the application.');
    return lines.join('\n');
  }

  lines.push('MIDI Input Ports:');
  if (ports.inputs.length === 0) {
    lines.push('  (none found)');
  } else {
    ports.inputs.forEach((port, index) => {
      const marker = isNanoKontrol2Port(port) ? ' [nanoKONTROL2]' : '';
      lines.push(`  ${index + 1}. ${port}${marker}`);
    });
  }

  lines.push('');
  lines.push('MIDI Output Ports:');
  if (ports.outputs.length === 0) {
    lines.push('  (none found)');
  } else {
    ports.outputs.forEach((port, index) => {
      const marker = isNanoKontrol2Port(port) ? ' [nanoKONTROL2]' : '';
      lines.push(`  ${index + 1}. ${port}${marker}`);
    });
  }

  return lines.join('\n');
}
