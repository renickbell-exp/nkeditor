/**
 * MIDI Manager Service
 *
 * High-level orchestrator for MIDI functionality in the Electron main process.
 * Wraps Phase 1 modules: MidiHandler, MappingEngine, LedController.
 *
 * Provides a unified interface for:
 * - Device connection/disconnection
 * - CC message processing and remapping
 * - LED synchronization
 * - Event forwarding to renderer
 */

import { EventEmitter } from 'events';
import { MidiHandler } from './midi-handler.js';
import { MappingEngine } from './mapping-engine.js';
import { LedController } from './led-controller.js';
import { getAvailablePorts, findNanoKontrol2 } from './midi-discovery.js';
import { deriveControlTypeFromCC } from '@shared/constants.js';
import { createDefaultMappingConfig } from '@shared/schemas.js';
import type { MappingConfig, CCMessage } from '@shared/types.js';
import type {
  MidiCCEvent,
  MidiConnectedEvent,
  MidiDisconnectedEvent,
  MidiPortsChangedEvent,
  ErrorEvent,
  GetPortsResponse,
  MidiStatusResponse,
} from '@shared/ipc-protocol.js';

// =============================================================================
// MidiManager Class
// =============================================================================

/**
 * MidiManager orchestrates MIDI operations for the application.
 *
 * Events:
 * - 'cc': CC message processed (MidiCCEvent)
 * - 'connected': Device connected (MidiConnectedEvent)
 * - 'disconnected': Device disconnected (MidiDisconnectedEvent)
 * - 'portsChanged': Available ports changed (MidiPortsChangedEvent)
 * - 'error': Error occurred (ErrorEvent)
 */
export class MidiManager extends EventEmitter {
  private midiHandler: MidiHandler;
  private mappingEngine: MappingEngine | null = null;
  private ledController: LedController;
  /** LED mode: 'internal' = LEDs reflect button state, 'external' = LEDs controlled by DAW */
  private ledMode: 'internal' | 'external' = 'internal';

  constructor() {
    super();

    // Initialize MIDI handler
    this.midiHandler = new MidiHandler({
      virtualPortName: 'nkEditor3 Out',
    });

    // Initialize LED controller (uses MIDI handler for output)
    this.ledController = new LedController(this.midiHandler);

    // Initialize default identity mapping engine so physical controls work immediately
    // Without this, incoming CCs would be dropped (no raw CC forwarding to prevent
    // hardware CC conflicts like slider CC 0-7 colliding with MIDI Volume CC 7)
    this.mappingEngine = new MappingEngine(createDefaultMappingConfig());
    this.setupMappingEngineEvents();

    // Set up event forwarding from MIDI handler
    this.setupEventHandlers();
  }

  // ===========================================================================
  // Connection Management
  // ===========================================================================

  /**
   * Connects to MIDI device.
   */
  connect(inputPort?: string, outputPort?: string): boolean {
    const success = this.midiHandler.connect(inputPort, outputPort);

    if (success && this.mappingEngine) {
      // Sync LED states on connect
      this.ledController.syncFromEngine(this.mappingEngine);
    }

    return success;
  }

  /**
   * Disconnects from MIDI device.
   */
  disconnect(): void {
    // Turn off all LEDs before disconnecting
    this.ledController.allOff();
    this.midiHandler.disconnect();
  }

  /**
   * Returns connection status.
   */
  isConnected(): boolean {
    return this.midiHandler.isConnected();
  }

  /**
   * Gets current MIDI status.
   */
  getStatus(): MidiStatusResponse {
    const ports = this.midiHandler.getCurrentPorts();
    return {
      connected: this.midiHandler.isConnected(),
      inputPort: ports.input,
      outputPort: ports.output,
      virtualPort: this.midiHandler.getVirtualPortName(),
      midiAvailable: this.midiHandler.isMidiSubsystemAvailable(),
    };
  }

  /**
   * Gets current hardware port names.
   */
  getCurrentPorts(): { input: string | null; output: string | null } {
    return this.midiHandler.getCurrentPorts();
  }

  /**
   * Gets available MIDI ports.
   */
  getAvailablePorts(): GetPortsResponse {
    const ports = getAvailablePorts();
    const device = findNanoKontrol2();

    return {
      inputs: ports.inputs,
      outputs: ports.outputs,
      nanoKontrol2Found: device.input !== null && device.output !== null,
    };
  }

  // ===========================================================================
  // Hotplug Detection
  // ===========================================================================

  /**
   * Starts hotplug detection.
   */
  startHotplugDetection(intervalMs?: number): void {
    this.midiHandler.startHotplugDetection(intervalMs);
  }

  /**
   * Stops hotplug detection.
   */
  stopHotplugDetection(): void {
    this.midiHandler.stopHotplugDetection();
  }

  // ===========================================================================
  // Mapping Configuration
  // ===========================================================================

  /**
   * Loads a mapping configuration.
   */
  loadMapping(config: MappingConfig): void {
    if (this.mappingEngine) {
      this.mappingEngine.updateConfig(config);
    } else {
      this.mappingEngine = new MappingEngine(config);
      this.setupMappingEngineEvents();
    }

    // Sync LEDs with new mapping
    if (this.midiHandler.isConnected()) {
      this.ledController.syncFromEngine(this.mappingEngine);
    }
  }

  /**
   * Gets the current mapping configuration.
   */
  getMapping(): MappingConfig | null {
    return this.mappingEngine?.getConfig() ?? null;
  }

  // ===========================================================================
  // CC Operations
  // ===========================================================================

  /**
   * Sends a CC message to the virtual output.
   */
  sendCC(channel: number, cc: number, value: number): void {
    this.midiHandler.sendCC(channel, cc, value);
  }

  /**
   * Sends an LED update to the hardware.
   */
  setLed(cc: number, isOn: boolean): void {
    this.ledController.setLed(cc, isOn);
  }

  // ===========================================================================
  // LED Mode Configuration
  // ===========================================================================

  /**
   * Sets the LED mode.
   * - 'internal': LEDs reflect button state (updated on button press)
   * - 'external': LEDs are controlled by incoming MIDI from DAW (button presses don't update LEDs)
   */
  setLedMode(mode: 'internal' | 'external'): void {
    this.ledMode = mode;
    console.log(`[midi-manager] LED mode set to: ${mode}`);
  }

  /**
   * Gets the current LED mode.
   */
  getLedMode(): 'internal' | 'external' {
    return this.ledMode;
  }

  // ===========================================================================
  // Private Methods
  // ===========================================================================

  /**
   * Sets up event handlers from MIDI handler.
   */
  private setupEventHandlers(): void {
    // Forward CC messages (process through mapping engine if available)
    this.midiHandler.on('cc', (msg: CCMessage) => {
      this.handleIncomingCC(msg);
    });

    // Forward connection events
    this.midiHandler.on('connected', ({ inputPort, outputPort }) => {
      const event: MidiConnectedEvent = {
        inputPort,
        outputPort,
        virtualPort: this.midiHandler.getVirtualPortName(),
      };
      this.emit('connected', event);

      // Sync LEDs on reconnect
      if (this.mappingEngine) {
        this.ledController.syncFromEngine(this.mappingEngine);
      }
    });

    this.midiHandler.on('disconnected', ({ reason }) => {
      const event: MidiDisconnectedEvent = { reason };
      this.emit('disconnected', event);
    });

    this.midiHandler.on('portsChanged', ({ inputs, outputs }) => {
      const event: MidiPortsChangedEvent = { inputs, outputs };
      this.emit('portsChanged', event);
    });

    this.midiHandler.on('error', ({ message }) => {
      const event: ErrorEvent = { message };
      this.emit('error', event);
    });
  }

  /**
   * Sets up event handlers from mapping engine.
   */
  private setupMappingEngineEvents(): void {
    if (!this.mappingEngine) return;

    // Sync LED when button state changes (only in internal mode)
    this.mappingEngine.on('buttonStateChanged', ({ cc, isOn }) => {
      // In external mode, LEDs are controlled by incoming MIDI from DAW,
      // so we don't update them based on button presses
      if (this.ledMode === 'internal') {
        this.ledController.setLed(cc, isOn);
      }
    });
  }

  /**
   * Handles incoming CC message from hardware.
   *
   * IMPORTANT: Only mapped CCs are forwarded to the virtual output.
   * Raw/unmapped CCs are NOT forwarded because hardware CC numbers may conflict
   * with standard MIDI CCs (e.g., nanoKONTROL2 sliders use CC 0-7, where CC 7
   * is MIDI Volume - forwarding raw slider values would silence synths).
   */
  private handleIncomingCC(msg: CCMessage): void {
    // If no mapping engine, only update GUI - do NOT forward raw CCs to synth
    // (This branch should rarely execute since we initialize a default mapping engine,
    // but we keep it for safety)
    if (!this.mappingEngine) {
      const controlType = deriveControlTypeFromCC(msg.controller);
      const event: MidiCCEvent = {
        channel: msg.channel,
        controller: msg.controller,
        value: msg.value,
        controlType: controlType ?? undefined,
      };
      this.emit('cc', event);
      return;
    }

    // Process through mapping engine
    const processed = this.mappingEngine.processCC(msg.channel, msg.controller, msg.value);

    if (!processed) {
      // CC not mapped in current preset - do NOT forward to synth
      // Only update the GUI to reflect physical control movements
      const controlType = deriveControlTypeFromCC(msg.controller);
      const event: MidiCCEvent = {
        channel: msg.channel,
        controller: msg.controller,
        value: msg.value,
        controlType: controlType ?? undefined,
      };
      this.emit('cc', event);
      return;
    }

    // Send REMAPPED message to virtual output (safe - uses configured output CC)
    // Note: processed.channel is 1-indexed, sendCC expects 0-indexed
    this.midiHandler.sendCC(processed.channel - 1, processed.outputCC, processed.value);

    // Emit processed message to renderer
    const event: MidiCCEvent = {
      channel: msg.channel,
      controller: msg.controller,
      value: msg.value,
      outputCC: processed.outputCC,
      outputChannel: processed.channel,
      controlType: processed.controlType,
    };
    this.emit('cc', event);
  }
}
