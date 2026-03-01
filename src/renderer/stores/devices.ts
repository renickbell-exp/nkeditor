/**
 * Device Instance Store
 *
 * Central store tracking all connected/configured device instances at runtime.
 * Each device instance represents one physical MIDI controller with its own
 * connection state, preset assignment, and virtual output port.
 *
 * This store manages the lifecycle of device instances and determines which
 * device is currently active (visible in the UI). It does NOT manage per-device
 * control state -- that responsibility belongs to the controls store, which
 * keys its state by instanceId from this store.
 */

import { create } from 'zustand';
import { getDeviceDefinition } from '@shared/devices';
import type { DeviceDefinition } from '@shared/devices';

// =============================================================================
// Types
// =============================================================================

export interface DeviceInstance {
  /** Unique runtime identifier generated on creation (e.g., 'dev-<uuid>') */
  instanceId: string;
  /** Device definition ID used to look up hardware layout ('nanokontrol2', 'akai-midimix') */
  deviceType: string;
  /** User-assigned display name, defaults to the device definition's human-readable name */
  nickname: string;
  /** Whether this device is currently connected to physical hardware */
  connected: boolean;
  /** Whether a connection attempt is in progress */
  connecting: boolean;
  /** The MIDI input port name this device is bound to, or null if unbound */
  inputPort: string | null;
  /** The MIDI output port name this device sends to, or null if unbound */
  outputPort: string | null;
  /** The virtual MIDI port name created for this device's remapped output */
  virtualPort: string;
  /** The currently loaded preset ID for this device, or null if no preset is loaded */
  currentPresetId: string | null;
  /** Ordering index for tab rendering in the UI */
  tabOrder: number;
}

interface DevicesStoreState {
  /** All device instances keyed by instanceId */
  devices: Map<string, DeviceInstance>;
  /** The instanceId of the currently active (focused) device, or null if none */
  activeDeviceId: string | null;
  /** Ordered list of instanceIds controlling tab render order */
  deviceOrder: string[];

  /**
   * Creates and registers a new device instance.
   * The first device added automatically becomes the active device.
   *
   * @param deviceType - Device definition ID (must exist in the registry)
   * @param nickname - Optional user-facing name; defaults to the device definition's name
   * @returns The generated instanceId, or null if the deviceType is unknown
   */
  addDevice: (deviceType: string, nickname?: string) => string | null;

  /**
   * Removes a device instance and cleans up ordering.
   * If the removed device was active, the next device in order becomes active.
   *
   * @param instanceId - The instance to remove
   */
  removeDevice: (instanceId: string) => void;

  /**
   * Sets the active (focused) device by instanceId.
   * No-op if the instanceId does not exist.
   *
   * @param instanceId - The device to make active
   */
  setActiveDevice: (instanceId: string) => void;

  /**
   * Applies a partial update to a device instance's properties.
   * No-op if the instanceId does not exist.
   *
   * @param instanceId - The device to update
   * @param updates - The fields to merge into the device instance
   */
  updateDevice: (instanceId: string, updates: Partial<DeviceInstance>) => void;

  /**
   * Replaces the device ordering with a new arrangement.
   * Only instanceIds that exist in the devices map are kept.
   *
   * @param newOrder - The desired instanceId ordering
   */
  reorderDevices: (newOrder: string[]) => void;

  /**
   * Returns the currently active device instance, or null if none is active.
   */
  getActiveDevice: () => DeviceInstance | null;

  /**
   * Returns the DeviceDefinition for a given instance, or undefined if
   * the instance or its device type is not found.
   */
  getDeviceDefinition: (instanceId: string) => DeviceDefinition | undefined;
}

// =============================================================================
// ID Generation
// =============================================================================

/**
 * Generates a unique device instance ID.
 * Uses crypto.randomUUID when available (modern browsers), falls back to
 * timestamp + random suffix for environments without crypto support (tests).
 */
function generateInstanceId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `dev-${crypto.randomUUID()}`;
  }
  // Fallback for test environments without crypto.randomUUID
  const timestamp = Date.now().toString(36);
  const randomSuffix = Math.random().toString(36).substring(2, 10);
  return `dev-${timestamp}-${randomSuffix}`;
}

// =============================================================================
// Store Implementation
// =============================================================================

export const useDevicesStore = create<DevicesStoreState>((set, get) => ({
  devices: new Map(),
  activeDeviceId: null,
  deviceOrder: [],

  addDevice: (deviceType, nickname) => {
    const definition = getDeviceDefinition(deviceType);
    if (!definition) {
      console.error(`[devices-store] Unknown device type: '${deviceType}'`);
      return null;
    }

    const instanceId = generateInstanceId();
    const displayName = nickname ?? definition.name;

    const instance: DeviceInstance = {
      instanceId,
      deviceType,
      nickname: displayName,
      connected: false,
      connecting: false,
      inputPort: null,
      outputPort: null,
      virtualPort: `nkEditor3 ${displayName} Out`,
      currentPresetId: null,
      tabOrder: get().deviceOrder.length,
    };

    set((state) => {
      const newDevices = new Map(state.devices);
      newDevices.set(instanceId, instance);

      const newOrder = [...state.deviceOrder, instanceId];

      // First device becomes active automatically
      const newActiveId = state.activeDeviceId ?? instanceId;

      return {
        devices: newDevices,
        deviceOrder: newOrder,
        activeDeviceId: newActiveId,
      };
    });

    return instanceId;
  },

  removeDevice: (instanceId) => {
    set((state) => {
      if (!state.devices.has(instanceId)) {
        return state;
      }

      const newDevices = new Map(state.devices);
      newDevices.delete(instanceId);

      const newOrder = state.deviceOrder.filter((id) => id !== instanceId);

      // If the removed device was active, promote the first remaining device
      let newActiveId = state.activeDeviceId;
      if (state.activeDeviceId === instanceId) {
        newActiveId = newOrder.length > 0 ? (newOrder[0] ?? null) : null;
      }

      return {
        devices: newDevices,
        deviceOrder: newOrder,
        activeDeviceId: newActiveId,
      };
    });
  },

  setActiveDevice: (instanceId) => {
    const { devices } = get();
    if (!devices.has(instanceId)) {
      return;
    }
    set({ activeDeviceId: instanceId });
  },

  updateDevice: (instanceId, updates) => {
    set((state) => {
      const existing = state.devices.get(instanceId);
      if (!existing) {
        return state;
      }

      const newDevices = new Map(state.devices);
      newDevices.set(instanceId, { ...existing, ...updates, instanceId });

      return { devices: newDevices };
    });
  },

  reorderDevices: (newOrder) => {
    set((state) => {
      // Only keep instanceIds that actually exist in the devices map
      const validOrder = newOrder.filter((id) => state.devices.has(id));
      return { deviceOrder: validOrder };
    });
  },

  getActiveDevice: () => {
    const { devices, activeDeviceId } = get();
    if (!activeDeviceId) {
      return null;
    }
    return devices.get(activeDeviceId) ?? null;
  },

  getDeviceDefinition: (instanceId) => {
    const { devices } = get();
    const instance = devices.get(instanceId);
    if (!instance) {
      return undefined;
    }
    return getDeviceDefinition(instance.deviceType);
  },
}));
