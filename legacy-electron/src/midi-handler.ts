/**
 * MIDI Handler Module
 *
 * Manages MIDI I/O connections for the nkEditor3 application.
 * Handles:
 * - Hardware input from nanoKONTROL2 (receiving CC messages)
 * - Hardware output to nanoKONTROL2 (controlling LEDs)
 * - Virtual MIDI output port (for remapped CC messages to DAWs)
 * - Hot-plug detection for device connect/disconnect
 *
 * This module uses easymidi for all MIDI operations, which wraps
 * RtMidi/ALSA on Linux systems.
 *
 * IMPORTANT: This module is designed to handle MIDI subsystem failures
 * gracefully. When ALSA/RtMidi is unavailable (common in containerized
 * environments or when /dev/snd/seq is inaccessible), the handler will:
 * - Emit 'error' events instead of throwing exceptions
 * - Continue operating in a "disconnected" state
 * - Retry connections when hotplug detection is enabled
 *
 * ALSA RESOURCE CONSERVATION: The hotplug detection mechanism is designed
 * to minimize ALSA sequencer client creation. When connected, it uses a
 * "heartbeat" approach that tests the existing connection rather than
 * enumerating all ports. Port enumeration only occurs when disconnected
 * and searching for a device. This prevents ALSA resource exhaustion that
 * can occur with frequent port enumeration (each enumeration creates a
 * temporary ALSA client).
 */

import easymidi from 'easymidi';
import { EventEmitter } from 'events';
import {
  getAvailablePorts,
  isMidiAvailable,
} from './midi-discovery.js';
import {
  DEFAULT_VIRTUAL_PORT_NAME,
  DEFAULT_HOTPLUG_INTERVAL_MS,
  HOTPLUG_DEBOUNCE_MS,
  MIDI_VALUE_ON,
  MIDI_VALUE_OFF,
} from './types.js';
import type {
  MidiHandlerOptions,
  CCMessage,
  MidiPorts,
  ConnectedEvent,
  DisconnectedEvent,
  PortsChangedEvent,
  ErrorEvent,
} from './types.js';

/**
 * Empty ports object returned when MIDI enumeration fails.
 * Using a constant avoids repeated object allocations.
 */
const EMPTY_PORTS: MidiPorts = Object.freeze({ inputs: [], outputs: [] });

/**
 * LED control channel (0-indexed, which is MIDI channel 1 in user-facing terms).
 * The nanoKONTROL2 expects LED control messages on channel 1.
 */
const LED_CHANNEL = 0;

/**
 * CC number used for connection heartbeat checks.
 *
 * We use CC 127 (Data Increment) which is a no-op on most devices.
 * This allows us to test if the MIDI output port is still valid without
 * affecting the device state. If the send fails, the connection is broken.
 *
 * Using a harmless CC is preferable to enumerating all ALSA ports because:
 * - It doesn't create new ALSA sequencer clients
 * - It's much faster than port enumeration
 * - It directly tests the connection we care about
 */
const HEARTBEAT_CC = 127;

/**
 * MidiHandler manages all MIDI I/O for the nkEditor3 application.
 *
 * Events emitted:
 * - 'cc': CC message received from hardware { channel, controller, value }
 * - 'connected': Device connected { inputPort, outputPort }
 * - 'disconnected': Device disconnected { reason }
 * - 'portsChanged': Available ports changed { inputs, outputs }
 * - 'error': Error occurred { message }
 *
 * @example
 * const handler = new MidiHandler({ virtualPortName: 'My MIDI Out' });
 *
 * handler.on('cc', (msg) => {
 *   console.log(`CC ${msg.controller} = ${msg.value} on channel ${msg.channel + 1}`);
 * });
 *
 * handler.on('connected', ({ inputPort, outputPort }) => {
 *   console.log('nanoKONTROL2 connected!');
 * });
 *
 * if (handler.connect()) {
 *   handler.startHotplugDetection();
 * }
 */
export class MidiHandler extends EventEmitter {
  /** Hardware MIDI input connection (from nanoKONTROL2) */
  private input: easymidi.Input | null = null;

  /** Hardware MIDI output connection (for LED control) */
  private output: easymidi.Output | null = null;

  /** Virtual MIDI output port (for remapped CC messages) */
  private virtualOutput: easymidi.Output | null = null;

  /** Timer for hot-plug detection polling */
  private pollInterval: NodeJS.Timeout | null = null;

  /** Debounce timer for port change detection */
  private debounceTimer: NodeJS.Timeout | null = null;

  /** Last known port state for change detection */
  private lastKnownPorts: MidiPorts = { inputs: [], outputs: [] };

  /** Configuration options */
  private readonly options: Required<MidiHandlerOptions>;

  /** Current connection state */
  private connected = false;

  /** Currently connected input port name */
  private currentInputPort: string | null = null;

  /** Currently connected output port name */
  private currentOutputPort: string | null = null;

  /**
   * Tracks whether the MIDI subsystem is available.
   * When false, all MIDI operations will gracefully degrade.
   * This can become true later if the subsystem becomes available.
   */
  private midiSubsystemAvailable = true;

  /**
   * Tracks whether virtual port creation has been attempted and failed.
   * Prevents repeated failed attempts to create the virtual port.
   */
  private virtualPortCreationFailed = false;

  /**
   * Creates a new MidiHandler instance.
   *
   * Does NOT enumerate MIDI ports during construction to avoid creating
   * unnecessary ALSA sequencer clients. Port enumeration is deferred until
   * connect() is called or hotplug detection starts.
   *
   * @param options - Configuration options
   * @param options.inputPort - Specific hardware input port name (auto-detect if not specified)
   * @param options.outputPort - Specific hardware output port name (auto-detect if not specified)
   * @param options.virtualPortName - Name for the virtual output port
   */
  constructor(options?: MidiHandlerOptions) {
    super();

    this.options = {
      inputPort: options?.inputPort ?? '',
      outputPort: options?.outputPort ?? '',
      virtualPortName: options?.virtualPortName ?? DEFAULT_VIRTUAL_PORT_NAME,
    };

    // Start with empty lastKnownPorts - the baseline will be established
    // on the first call to connect() or when hotplug detection starts.
    // This avoids creating ALSA sequencer clients during construction.
    // The first hotplug check will detect "all ports changed" which is
    // the correct behavior - it establishes the baseline and triggers
    // any necessary reconnection logic.
  }

  /**
   * Connects to MIDI ports.
   *
   * If input/output ports are not specified, attempts to auto-detect
   * the nanoKONTROL2 device. Creates a virtual output port for remapped
   * MIDI messages that other applications (DAWs) can connect to.
   *
   * IMPORTANT: This method will NEVER throw. If MIDI is unavailable,
   * it returns false and emits an 'error' event. The handler can continue
   * to exist in a disconnected state.
   *
   * @param inputPort - Hardware input port name (overrides constructor option)
   * @param outputPort - Hardware output port name (overrides constructor option)
   * @returns True if connection was successful, false otherwise
   *
   * @example
   * // Auto-detect nanoKONTROL2
   * const success = handler.connect();
   *
   * // Or specify ports explicitly
   * const success = handler.connect(
   *   'nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0',
   *   'nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0'
   * );
   */
  connect(inputPort?: string, outputPort?: string): boolean {
    // Close any existing connections first
    this.closeHardwarePorts();

    // Determine which ports to use
    let targetInputPort = inputPort ?? this.options.inputPort;
    let targetOutputPort = outputPort ?? this.options.outputPort;

    // Auto-detect if not specified
    // We fetch available ports ONCE here and reuse them for both detection
    // and updating lastKnownPorts. This avoids redundant ALSA enumeration calls.
    if (!targetInputPort || !targetOutputPort) {
      const ports = this.safeGetAvailablePorts();

      // Update the baseline for hotplug detection - this prevents a spurious
      // "portsChanged" event on the first hotplug check after connect()
      this.lastKnownPorts = ports;

      // Check if MIDI subsystem is unavailable
      if (ports.inputs.length === 0 && ports.outputs.length === 0 && !this.midiSubsystemAvailable) {
        // MIDI subsystem is unavailable - don't emit error, already reported
        return false;
      }

      // Find nanoKONTROL2 in the fetched ports (no additional ALSA calls)
      const inputMatch = this.findNanoKontrol2InPorts(ports.inputs);
      const outputMatch = this.findNanoKontrol2InPorts(ports.outputs);

      if (!inputMatch || !outputMatch) {
        // Device not found - emit error only if MIDI subsystem is available
        if (this.midiSubsystemAvailable) {
          this.emitError('nanoKONTROL2 not found. Please connect the device and try again.');
        }
        return false;
      }

      targetInputPort = targetInputPort || inputMatch;
      targetOutputPort = targetOutputPort || outputMatch;
    } else {
      // Ports were specified explicitly - still update baseline for hotplug
      this.lastKnownPorts = this.safeGetAvailablePorts();
    }

    // Attempt to open hardware input port
    try {
      this.input = new easymidi.Input(targetInputPort);
      this.setupInputEventHandlers();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.handleMidiSubsystemError(`Failed to open input port "${targetInputPort}": ${message}`);
      return false;
    }

    // Attempt to open hardware output port
    try {
      this.output = new easymidi.Output(targetOutputPort);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.handleMidiSubsystemError(`Failed to open output port "${targetOutputPort}": ${message}`);
      this.closeHardwarePorts();
      return false;
    }

    // Create virtual output port if not already open and not previously failed.
    // Virtual port failure is non-fatal - we can still operate without it,
    // just without the ability to send remapped CC messages to DAWs.
    if (!this.virtualOutput && !this.virtualPortCreationFailed) {
      this.tryCreateVirtualPort();
    }

    // Mark as connected and store port names
    this.connected = true;
    this.currentInputPort = targetInputPort;
    this.currentOutputPort = targetOutputPort;

    // Emit connected event
    const connectedEvent: ConnectedEvent = {
      inputPort: targetInputPort,
      outputPort: targetOutputPort,
    };
    this.emit('connected', connectedEvent);

    return true;
  }

  /**
   * Disconnects from all MIDI ports and cleans up resources.
   *
   * Closes hardware input/output ports and the virtual output port.
   * Stops hot-plug detection if running. Safe to call multiple times.
   */
  disconnect(): void {
    this.stopHotplugDetection();
    this.closeHardwarePorts();
    this.closeVirtualPort();

    if (this.connected) {
      this.connected = false;
      const disconnectedEvent: DisconnectedEvent = { reason: 'Manual disconnect' };
      this.emit('disconnected', disconnectedEvent);
    }
  }

  /**
   * Sends a CC message to the virtual output port.
   *
   * This is used for sending remapped MIDI messages to connected
   * DAWs and other MIDI applications.
   *
   * @param channel - MIDI channel (0-15, 0-indexed as easymidi expects)
   * @param cc - Controller number (0-127)
   * @param value - Controller value (0-127)
   *
   * @example
   * // Send CC 74 value 100 on channel 1 (0-indexed)
   * handler.sendCC(0, 74, 100);
   */
  sendCC(channel: number, cc: number, value: number): void {
    if (!this.virtualOutput) {
      this.emitError('Cannot send CC: Virtual output port not connected');
      return;
    }

    const clampedChannel = clampValue(channel, 0, 15);
    const clampedCC = clampValue(cc, 0, 127);
    const clampedValue = clampValue(value, 0, 127);

    try {
      // Type assertion needed as easymidi types may not be fully accurate
      (this.virtualOutput as { send(type: string, msg: object): void }).send('cc', {
        channel: clampedChannel,
        controller: clampedCC,
        value: clampedValue,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.emitError(`Failed to send CC to virtual output: ${message}`);
    }
  }

  /**
   * Sends a CC message to the hardware output port for LED control.
   *
   * LED messages are always sent on channel 0 (MIDI channel 1) as
   * required by the nanoKONTROL2.
   *
   * @param cc - Controller number for the LED (0-127)
   * @param value - LED state: 127 for on, 0 for off
   *
   * @example
   * // Turn on Solo button LED for track 1 (CC 32)
   * handler.sendLedCC(32, 127);
   *
   * // Turn off the LED
   * handler.sendLedCC(32, 0);
   */
  sendLedCC(cc: number, value: number): void {
    if (!this.output) {
      this.emitError('Cannot send LED CC: Hardware output port not connected');
      return;
    }

    const clampedCC = clampValue(cc, 0, 127);
    const clampedValue = value > 0 ? MIDI_VALUE_ON : MIDI_VALUE_OFF;

    try {
      this.output.send('cc', {
        channel: LED_CHANNEL,
        controller: clampedCC,
        value: clampedValue,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.emitError(`Failed to send LED CC to hardware: ${message}`);
    }
  }

  /**
   * Starts polling for device connect/disconnect events.
   *
   * Periodically checks available MIDI ports and compares them to the
   * last known state. If changes are detected, emits 'portsChanged' event.
   * If the current device disconnects, attempts to reconnect.
   *
   * @param intervalMs - Polling interval in milliseconds (default: 2000ms)
   *
   * @example
   * handler.startHotplugDetection(1000); // Poll every 1 second
   */
  startHotplugDetection(intervalMs: number = DEFAULT_HOTPLUG_INTERVAL_MS): void {
    // Stop any existing polling
    this.stopHotplugDetection();

    this.pollInterval = setInterval(() => {
      this.checkForPortChanges();
    }, intervalMs);

    // Don't prevent the process from exiting if this is the only thing running
    this.pollInterval.unref();
  }

  /**
   * Stops hot-plug detection polling.
   */
  stopHotplugDetection(): void {
    if (this.pollInterval) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }

    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }
  }

  /**
   * Returns the current connection status.
   *
   * @returns True if hardware ports are connected, false otherwise
   */
  isConnected(): boolean {
    return this.connected;
  }

  /**
   * Returns the name of the virtual output port.
   *
   * This is the port name that other applications (DAWs) should
   * connect to in order to receive remapped MIDI messages.
   *
   * @returns Virtual output port name
   */
  getVirtualPortName(): string {
    return this.options.virtualPortName;
  }

  /**
   * Returns the currently connected hardware port names.
   *
   * @returns Object with current input and output port names, or null if not connected
   */
  getCurrentPorts(): { input: string | null; output: string | null } {
    return {
      input: this.currentInputPort,
      output: this.currentOutputPort,
    };
  }

  /**
   * Returns whether the MIDI subsystem is currently available.
   *
   * When false, MIDI operations will fail gracefully. The subsystem
   * may become available later if the user fixes the underlying issue
   * (e.g., loading kernel modules, fixing permissions, restarting audio services).
   *
   * Hot-plug detection will automatically detect when the subsystem recovers.
   *
   * @returns True if MIDI subsystem is available, false otherwise
   *
   * @example
   * if (!handler.isMidiSubsystemAvailable()) {
   *   console.log('MIDI unavailable - running in demo mode');
   * }
   */
  isMidiSubsystemAvailable(): boolean {
    return this.midiSubsystemAvailable;
  }

  /**
   * Returns whether a virtual output port is available for sending
   * remapped CC messages to DAWs and other applications.
   *
   * The virtual port may be unavailable if:
   * - The MIDI subsystem is unavailable
   * - Virtual port creation failed (some systems allow port enumeration
   *   but not virtual port creation)
   * - The handler is not connected
   *
   * @returns True if virtual output is available, false otherwise
   *
   * @example
   * if (handler.hasVirtualOutput()) {
   *   handler.sendCC(0, 74, 100);
   * } else {
   *   console.log('Virtual output unavailable - CC messages will not reach DAW');
   * }
   */
  hasVirtualOutput(): boolean {
    return this.virtualOutput !== null;
  }

  /**
   * Sets up event handlers for the hardware input port.
   *
   * Listens for CC messages and forwards them to listeners via the 'cc' event.
   */
  private setupInputEventHandlers(): void {
    if (!this.input) {
      return;
    }

    this.input.on('cc', (msg: { channel: number; controller: number; value: number }) => {
      const ccMessage: CCMessage = {
        channel: msg.channel,
        controller: msg.controller,
        value: msg.value,
      };
      this.emit('cc', ccMessage);
    });
  }

  /**
   * Closes hardware input and output ports.
   *
   * Does not close the virtual output port, which persists across reconnections.
   */
  private closeHardwarePorts(): void {
    if (this.input) {
      try {
        this.input.close();
      } catch {
        // Ignore errors when closing - port may already be invalid
      }
      this.input = null;
    }

    if (this.output) {
      try {
        this.output.close();
      } catch {
        // Ignore errors when closing - port may already be invalid
      }
      this.output = null;
    }

    this.currentInputPort = null;
    this.currentOutputPort = null;
  }

  /**
   * Closes the virtual output port.
   */
  private closeVirtualPort(): void {
    if (this.virtualOutput) {
      try {
        this.virtualOutput.close();
      } catch {
        // Ignore errors when closing
      }
      this.virtualOutput = null;
    }
  }

  /**
   * Checks for device connection status changes.
   *
   * ALSA RESOURCE CONSERVATION: This method uses two different strategies
   * depending on connection state to minimize ALSA sequencer client creation:
   *
   * 1. WHEN CONNECTED: Uses a "heartbeat" approach - sends a harmless MIDI
   *    message to test if the connection is still valid. This does NOT create
   *    any new ALSA clients. Only if the heartbeat fails do we enumerate ports.
   *
   * 2. WHEN DISCONNECTED: Enumerates available ports to search for the device.
   *    This does create temporary ALSA clients, but only when necessary.
   *
   * This approach prevents ALSA resource exhaustion that can occur when
   * enumerating ports every 2 seconds indefinitely. The old approach would
   * eventually trigger "Cannot allocate memory" errors on systems with
   * limited ALSA capacity.
   *
   * Uses debouncing to avoid reacting to rapid port changes during
   * device initialization.
   */
  private checkForPortChanges(): void {
    // STRATEGY 1: If connected, use heartbeat check instead of port enumeration
    // This avoids creating new ALSA sequencer clients on every poll
    if (this.connected && this.output) {
      const connectionHealthy = this.performHeartbeatCheck();

      if (connectionHealthy) {
        // Connection is working - no need to enumerate ports
        return;
      }

      // Heartbeat failed - device likely disconnected
      // Now we need to enumerate ports to confirm and handle the situation
      this.handleDeviceDisconnect();

      // Fall through to port enumeration for reconnection attempt
    }

    // STRATEGY 2: Enumerate ports only when disconnected and looking for device
    const currentPorts = this.safeGetAvailablePorts();

    // If MIDI subsystem became available after being unavailable,
    // log a recovery message. This can happen if the user fixes
    // ALSA permissions or loads kernel modules.
    if (!this.midiSubsystemAvailable && (currentPorts.inputs.length > 0 || currentPorts.outputs.length > 0)) {
      this.midiSubsystemAvailable = true;
      // Reset virtual port creation flag to allow retry
      this.virtualPortCreationFailed = false;
    }

    // Check if ports have changed from last known state
    const portsChanged =
      !arraysEqual(currentPorts.inputs, this.lastKnownPorts.inputs) ||
      !arraysEqual(currentPorts.outputs, this.lastKnownPorts.outputs);

    if (!portsChanged) {
      // Ports haven't changed, but we're disconnected - attempt reconnect anyway
      // in case we previously failed due to a transient error
      if (!this.connected) {
        this.attemptReconnect();
      }
      return;
    }

    // Clear any pending debounce timer
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }

    // Debounce the port change handling
    this.debounceTimer = setTimeout(() => {
      this.handlePortsChanged(currentPorts);
    }, HOTPLUG_DEBOUNCE_MS);
  }

  /**
   * Performs a heartbeat check on the MIDI connection.
   *
   * Attempts to send a harmless CC message to verify the output port is still
   * functional. This is much cheaper than enumerating all ALSA ports because
   * it doesn't create any new ALSA sequencer clients.
   *
   * The heartbeat uses CC 127 (Data Increment) with value 0, which is
   * effectively a no-op on most MIDI devices including the nanoKONTROL2.
   *
   * @returns True if the connection appears healthy, false if send failed
   */
  private performHeartbeatCheck(): boolean {
    if (!this.output) {
      return false;
    }

    try {
      // Send a harmless CC message to test the connection
      // CC 127 (Data Increment) with value 0 is a no-op on most devices
      this.output.send('cc', {
        channel: LED_CHANNEL,
        controller: HEARTBEAT_CC,
        value: 0,
      });
      return true;
    } catch {
      // Send failed - connection is broken
      return false;
    }
  }

  /**
   * Handles detected port changes after debouncing.
   *
   * @param newPorts - The new set of available ports
   */
  private handlePortsChanged(newPorts: MidiPorts): void {
    // Store new ports state (old ports were used for comparison in checkForPortChanges)
    this.lastKnownPorts = newPorts;

    // Emit portsChanged event
    const portsChangedEvent: PortsChangedEvent = {
      inputs: newPorts.inputs,
      outputs: newPorts.outputs,
    };
    this.emit('portsChanged', portsChangedEvent);

    // Check if current device is still available
    if (this.connected && this.currentInputPort && this.currentOutputPort) {
      const inputStillAvailable = newPorts.inputs.includes(this.currentInputPort);
      const outputStillAvailable = newPorts.outputs.includes(this.currentOutputPort);

      if (!inputStillAvailable || !outputStillAvailable) {
        // Device disconnected
        this.handleDeviceDisconnect();
        return;
      }
    }

    // If not connected, attempt to reconnect
    if (!this.connected) {
      this.attemptReconnect();
    }
  }

  /**
   * Handles device disconnect event.
   *
   * Closes hardware ports and emits disconnected event. Does not close
   * the virtual port, allowing reconnection without affecting downstream
   * applications.
   */
  private handleDeviceDisconnect(): void {
    this.closeHardwarePorts();
    this.connected = false;

    const disconnectedEvent: DisconnectedEvent = {
      reason: 'Device disconnected',
    };
    this.emit('disconnected', disconnectedEvent);
  }

  /**
   * Attempts to reconnect to the nanoKONTROL2.
   *
   * Called automatically when hot-plug detection notices the device
   * has reappeared after being disconnected.
   */
  private attemptReconnect(): void {
    // Use originally specified ports if available, otherwise auto-detect
    const inputPort = this.options.inputPort || undefined;
    const outputPort = this.options.outputPort || undefined;

    const success = this.connect(inputPort, outputPort);

    if (!success) {
      // Silent failure - will retry on next poll
      // The connect() method already emits appropriate error events
    }
  }

  /**
   * Emits an error event with the given message.
   *
   * @param message - Error description
   */
  private emitError(message: string): void {
    const errorEvent: ErrorEvent = { message };
    this.emit('error', errorEvent);
  }

  /**
   * Safely retrieves available MIDI ports with error handling.
   *
   * If the MIDI subsystem is unavailable, returns empty port arrays
   * and marks the subsystem as unavailable. This prevents repeated
   * error spam during hotplug polling.
   *
   * @returns MidiPorts object with input and output arrays (may be empty)
   */
  private safeGetAvailablePorts(): MidiPorts {
    // Check cached availability state first to avoid unnecessary retries
    if (!this.midiSubsystemAvailable && !isMidiAvailable()) {
      return EMPTY_PORTS;
    }

    try {
      const ports = getAvailablePorts();
      // If we got results, the subsystem is working
      if (ports.inputs.length > 0 || ports.outputs.length > 0) {
        this.midiSubsystemAvailable = true;
      }
      return ports;
    } catch {
      // This shouldn't happen since getAvailablePorts() catches errors internally,
      // but we're being defensive here
      this.midiSubsystemAvailable = false;
      return EMPTY_PORTS;
    }
  }

  /**
   * Regular expression pattern to match nanoKONTROL2 MIDI ports.
   * Matches port names starting with "nanoKONTROL2:" or "nanoKONTROL2 ".
   * Case-insensitive to handle variations across different ALSA/PipeWire configurations.
   */
  private static readonly NANOKONTROL2_PATTERN = /^nanokontrol2[:\s]/i;

  /**
   * Finds a nanoKONTROL2 port name in an already-fetched list of port names.
   *
   * This method does NOT make any ALSA calls - it searches a provided array.
   * Use this when you already have the port list and want to avoid redundant
   * ALSA enumeration.
   *
   * @param portNames - Array of port names to search
   * @returns The first matching port name, or null if not found
   */
  private findNanoKontrol2InPorts(portNames: string[]): string | null {
    for (const portName of portNames) {
      if (MidiHandler.NANOKONTROL2_PATTERN.test(portName)) {
        return portName;
      }
    }
    return null;
  }

  /**
   * Handles MIDI subsystem errors by logging and emitting events.
   *
   * Checks if the error indicates the MIDI subsystem is unavailable
   * (e.g., ALSA initialization failure) and updates internal state accordingly.
   *
   * @param message - Error message describing what failed
   */
  private handleMidiSubsystemError(message: string): void {
    // Check for common MIDI subsystem failure patterns
    const isSubsystemFailure =
      message.includes('Failed to initialise RtMidi') ||
      message.includes('error creating ALSA sequencer') ||
      message.includes('Cannot allocate memory') ||
      message.includes('/dev/snd/seq');

    if (isSubsystemFailure) {
      this.midiSubsystemAvailable = false;
    }

    this.emitError(message);
  }

  /**
   * Attempts to create the virtual MIDI output port.
   *
   * Virtual port creation can fail in environments where MIDI is partially
   * available (e.g., can enumerate ports but not create new ones). This is
   * non-fatal - the handler can still operate without a virtual port, just
   * without the ability to send remapped CC messages to DAWs.
   *
   * If creation fails, sets virtualPortCreationFailed flag to prevent
   * repeated failed attempts.
   */
  private tryCreateVirtualPort(): void {
    try {
      this.virtualOutput = new easymidi.Output(this.options.virtualPortName, true);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.virtualPortCreationFailed = true;
      // Emit as a warning, not a fatal error - we can continue without virtual output
      this.emitError(
        `Failed to create virtual MIDI port "${this.options.virtualPortName}": ${message}. ` +
        `Remapped CC messages will not be available to other applications.`
      );
    }
  }
}

/**
 * Clamps a numeric value to the specified range.
 *
 * @param value - The value to clamp
 * @param min - Minimum allowed value
 * @param max - Maximum allowed value
 * @returns Clamped value
 */
function clampValue(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

/**
 * Compares two string arrays for equality.
 *
 * @param arrayA - First array
 * @param arrayB - Second array
 * @returns True if arrays contain the same elements in the same order
 */
function arraysEqual(arrayA: string[], arrayB: string[]): boolean {
  if (arrayA.length !== arrayB.length) {
    return false;
  }

  for (let i = 0; i < arrayA.length; i++) {
    if (arrayA[i] !== arrayB[i]) {
      return false;
    }
  }

  return true;
}
