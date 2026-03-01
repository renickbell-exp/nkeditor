/**
 * Connection Store
 *
 * Manages MIDI connection state and operations.
 *
 * Multi-device support:
 * The existing single-device connection state is preserved for backward
 * compatibility. Additional helpers are provided for routing incoming MIDI
 * events to the correct device instance and for managing per-device
 * connection lifecycle when the devices store is available.
 *
 * The routeMidiEventToDevice helper inspects the MIDI port name from an
 * incoming event and matches it against registered device instances to
 * determine which device should receive the event.
 */

import { create } from 'zustand';
import type { ElectronAPI } from '../../main/preload';

// Get the electron API from window
const getElectronAPI = (): ElectronAPI | null => {
  return (window as { electronAPI?: ElectronAPI }).electronAPI ?? null;
};

// =============================================================================
// Store Types
// =============================================================================

/** Mapping from MIDI port name to the device instanceId that owns it */
type PortToDeviceMap = Map<string, string>;

interface ConnectionState {
  // Legacy single-device state
  connected: boolean;
  connecting: boolean;
  inputPort: string | null;
  outputPort: string | null;
  virtualPort: string;
  midiAvailable: boolean;
  availablePorts: {
    inputs: string[];
    outputs: string[];
  };

  // Multi-device routing
  /** Maps input port names to their owning device instanceId */
  portDeviceMap: PortToDeviceMap;

  // Legacy actions
  initialize: () => Promise<void>;
  connect: (inputPort?: string, outputPort?: string) => Promise<boolean>;
  disconnect: () => Promise<void>;
  refreshPorts: () => Promise<void>;

  // Multi-device actions

  /**
   * Registers an input port as belonging to a specific device instance.
   * Used by the device lifecycle to claim ownership of a MIDI port.
   *
   * @param inputPort - The MIDI input port name
   * @param instanceId - The device instance that owns this port
   */
  registerPortForDevice: (inputPort: string, instanceId: string) => void;

  /**
   * Removes a port-to-device mapping.
   * Should be called when a device disconnects or is removed.
   *
   * @param inputPort - The MIDI input port name to unregister
   */
  unregisterPort: (inputPort: string) => void;

  /**
   * Looks up which device instance owns a given MIDI input port.
   * Returns the instanceId, or null if no device is registered for that port.
   *
   * @param inputPort - The MIDI input port name to look up
   */
  getDeviceForPort: (inputPort: string) => string | null;

  /**
   * Removes all port mappings for a specific device instance.
   * Called during device removal to clean up all port associations.
   *
   * @param instanceId - The device instance being removed
   */
  unregisterAllPortsForDevice: (instanceId: string) => void;
}

// =============================================================================
// Store Implementation
// =============================================================================

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  // Initial state
  connected: false,
  connecting: false,
  inputPort: null,
  outputPort: null,
  virtualPort: 'nkEditor3 Out',
  midiAvailable: false,
  availablePorts: { inputs: [], outputs: [] },
  portDeviceMap: new Map(),

  // Initialize connection and set up event listeners
  initialize: async () => {
    const api = getElectronAPI();
    if (!api) {
      console.warn('[connection] Electron API not available');
      return;
    }

    // Get initial status
    try {
      const status = await api.getStatus();
      set({
        connected: status.connected,
        inputPort: status.inputPort,
        outputPort: status.outputPort,
        virtualPort: status.virtualPort,
        midiAvailable: status.midiAvailable,
      });
    } catch (error) {
      console.error('[connection] Failed to get initial status:', error);
    }

    // Get available ports
    await get().refreshPorts();

    // Set up event listeners
    api.onMidiConnected((event) => {
      set({
        connected: true,
        connecting: false,
        inputPort: event.inputPort,
        outputPort: event.outputPort,
        virtualPort: event.virtualPort,
      });
    });

    api.onMidiDisconnected(() => {
      set({
        connected: false,
        inputPort: null,
        outputPort: null,
      });
    });

    api.onMidiPortsChanged((event) => {
      set({
        availablePorts: {
          inputs: event.inputs,
          outputs: event.outputs,
        },
      });
    });
  },

  // Connect to MIDI device
  connect: async (inputPort, outputPort) => {
    const api = getElectronAPI();
    if (!api) return false;

    set({ connecting: true });

    try {
      const result = await api.connect({ inputPort, outputPort });

      if (result.success) {
        set({
          connected: true,
          connecting: false,
          inputPort: result.inputPort ?? null,
          outputPort: result.outputPort ?? null,
        });
        return true;
      } else {
        set({ connecting: false });
        console.error('[connection] Connect failed:', result.error);
        return false;
      }
    } catch (error) {
      set({ connecting: false });
      console.error('[connection] Connect error:', error);
      return false;
    }
  },

  // Disconnect from MIDI device
  disconnect: async () => {
    const api = getElectronAPI();
    if (!api) return;

    try {
      await api.disconnect();
      set({
        connected: false,
        inputPort: null,
        outputPort: null,
      });
    } catch (error) {
      console.error('[connection] Disconnect error:', error);
    }
  },

  // Refresh available ports
  refreshPorts: async () => {
    const api = getElectronAPI();
    if (!api) return;

    try {
      const response = await api.getPorts();
      set({
        availablePorts: {
          inputs: response.inputs,
          outputs: response.outputs,
        },
        midiAvailable: response.nanoKontrol2Found || response.inputs.length > 0,
      });
    } catch (error) {
      console.error('[connection] Failed to refresh ports:', error);
    }
  },

  // =========================================================================
  // Multi-device routing
  // =========================================================================

  registerPortForDevice: (inputPort, instanceId) => {
    set((state) => {
      const newMap = new Map(state.portDeviceMap);
      newMap.set(inputPort, instanceId);
      return { portDeviceMap: newMap };
    });
  },

  unregisterPort: (inputPort) => {
    set((state) => {
      const newMap = new Map(state.portDeviceMap);
      newMap.delete(inputPort);
      return { portDeviceMap: newMap };
    });
  },

  getDeviceForPort: (inputPort) => {
    return get().portDeviceMap.get(inputPort) ?? null;
  },

  unregisterAllPortsForDevice: (instanceId) => {
    set((state) => {
      const newMap = new Map(state.portDeviceMap);
      for (const [port, deviceId] of newMap) {
        if (deviceId === instanceId) {
          newMap.delete(port);
        }
      }
      return { portDeviceMap: newMap };
    });
  },
}));
