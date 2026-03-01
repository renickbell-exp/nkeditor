/**
 * Undo Store
 *
 * Manages undo history for control value changes.
 * Stores snapshots of control state that can be restored.
 * Maximum of 20 undo steps per device.
 *
 * Multi-device support:
 * Undo stacks are maintained per device instance, keyed by instanceId.
 * Each device has its own independent undo history so that undoing on one
 * device does not affect another.
 *
 * Backward compatibility:
 * The original push/pop/clear/canUndo methods without an instanceId parameter
 * delegate to a "default" device stack (using the key '__default__'). This
 * allows existing UI code to continue working unchanged during the transition.
 */

import { create } from 'zustand';
import type { ControlValues } from '@shared/ipc-protocol';

/** Key used for the legacy single-device undo stack */
const DEFAULT_DEVICE_KEY = '__default__';

interface UndoSnapshot {
  timestamp: number;
  description: string;
  controlValues: ControlValues;
}

interface UndoStore {
  /** Legacy: single undo stack (delegates to default device key) */
  undoStack: UndoSnapshot[];
  /** Maximum number of undo steps per device */
  maxUndoSteps: number;

  /** Per-device undo stacks keyed by instanceId */
  deviceUndoStacks: Map<string, UndoSnapshot[]>;

  // =========================================================================
  // Per-device undo operations
  // =========================================================================

  /**
   * Pushes a snapshot onto a specific device's undo stack.
   * Call this BEFORE making changes to preserve the previous state.
   *
   * @param instanceId - The device instance to push onto
   * @param description - Human-readable description of the action being undone
   * @param controlValues - The control state to preserve
   */
  pushDeviceSnapshot: (instanceId: string, description: string, controlValues: ControlValues) => void;

  /**
   * Pops and returns the most recent snapshot from a device's undo stack.
   * Returns null if the stack is empty.
   */
  popDeviceSnapshot: (instanceId: string) => UndoSnapshot | null;

  /**
   * Peeks at the most recent snapshot for a device without removing it.
   */
  peekDeviceSnapshot: (instanceId: string) => UndoSnapshot | null;

  /**
   * Returns the number of available undo steps for a device.
   */
  getDeviceUndoCount: (instanceId: string) => number;

  /**
   * Clears the undo stack for a specific device.
   */
  clearDeviceUndoStack: (instanceId: string) => void;

  /**
   * Checks if undo is available for a specific device.
   */
  canDeviceUndo: (instanceId: string) => boolean;

  /**
   * Removes the undo stack for a device instance entirely.
   * Should be called when a device instance is removed.
   */
  removeDeviceStack: (instanceId: string) => void;

  // =========================================================================
  // Legacy single-device operations (backward compat)
  // =========================================================================

  /**
   * Push a new snapshot onto the undo stack.
   * Call this BEFORE making changes to preserve the previous state.
   */
  pushSnapshot: (description: string, controlValues: ControlValues) => void;

  /**
   * Pop and return the most recent snapshot from the undo stack.
   * Returns null if stack is empty.
   */
  popSnapshot: () => UndoSnapshot | null;

  /**
   * Peek at the most recent snapshot without removing it.
   */
  peekSnapshot: () => UndoSnapshot | null;

  /**
   * Get the number of available undo steps.
   */
  getUndoCount: () => number;

  /**
   * Clear the entire undo stack.
   */
  clearUndoStack: () => void;

  /**
   * Check if undo is available.
   */
  canUndo: () => boolean;
}

export const useUndoStore = create<UndoStore>((set, get) => ({
  undoStack: [],
  maxUndoSteps: 20,
  deviceUndoStacks: new Map(),

  // =========================================================================
  // Per-device undo operations
  // =========================================================================

  pushDeviceSnapshot: (instanceId, description, controlValues) => {
    const snapshot: UndoSnapshot = {
      timestamp: Date.now(),
      description,
      controlValues: JSON.parse(JSON.stringify(controlValues)),
    };

    set((state) => {
      const newStacks = new Map(state.deviceUndoStacks);
      const existingStack = newStacks.get(instanceId) ?? [];
      const newStack = [snapshot, ...existingStack];

      // Trim to max size
      if (newStack.length > state.maxUndoSteps) {
        newStack.pop();
      }

      newStacks.set(instanceId, newStack);

      // Also update legacy undoStack if this is the default device
      const legacyUpdate = instanceId === DEFAULT_DEVICE_KEY
        ? { undoStack: newStack }
        : {};

      return { deviceUndoStacks: newStacks, ...legacyUpdate };
    });
  },

  popDeviceSnapshot: (instanceId) => {
    const stacks = get().deviceUndoStacks;
    const stack = stacks.get(instanceId);
    if (!stack || stack.length === 0) {
      return null;
    }

    const [snapshot, ...rest] = stack;

    set((state) => {
      const newStacks = new Map(state.deviceUndoStacks);
      newStacks.set(instanceId, rest);

      const legacyUpdate = instanceId === DEFAULT_DEVICE_KEY
        ? { undoStack: rest }
        : {};

      return { deviceUndoStacks: newStacks, ...legacyUpdate };
    });

    return snapshot ?? null;
  },

  peekDeviceSnapshot: (instanceId) => {
    const stack = get().deviceUndoStacks.get(instanceId);
    if (!stack || stack.length === 0) {
      return null;
    }
    return stack[0] ?? null;
  },

  getDeviceUndoCount: (instanceId) => {
    const stack = get().deviceUndoStacks.get(instanceId);
    return stack?.length ?? 0;
  },

  clearDeviceUndoStack: (instanceId) => {
    set((state) => {
      const newStacks = new Map(state.deviceUndoStacks);
      newStacks.set(instanceId, []);

      const legacyUpdate = instanceId === DEFAULT_DEVICE_KEY
        ? { undoStack: [] }
        : {};

      return { deviceUndoStacks: newStacks, ...legacyUpdate };
    });
  },

  canDeviceUndo: (instanceId) => {
    const stack = get().deviceUndoStacks.get(instanceId);
    return (stack?.length ?? 0) > 0;
  },

  removeDeviceStack: (instanceId) => {
    set((state) => {
      const newStacks = new Map(state.deviceUndoStacks);
      newStacks.delete(instanceId);
      return { deviceUndoStacks: newStacks };
    });
  },

  // =========================================================================
  // Legacy single-device operations (delegate to default device key)
  // =========================================================================

  pushSnapshot: (description: string, controlValues: ControlValues) => {
    const snapshot: UndoSnapshot = {
      timestamp: Date.now(),
      description,
      // Deep clone to prevent reference issues
      controlValues: JSON.parse(JSON.stringify(controlValues)),
    };

    set((state) => {
      const newStack = [snapshot, ...state.undoStack];
      // Trim to max size
      if (newStack.length > state.maxUndoSteps) {
        newStack.pop();
      }

      // Also update the per-device map for the default key
      const newStacks = new Map(state.deviceUndoStacks);
      newStacks.set(DEFAULT_DEVICE_KEY, newStack);

      return { undoStack: newStack, deviceUndoStacks: newStacks };
    });
  },

  popSnapshot: () => {
    const { undoStack } = get();
    if (undoStack.length === 0) {
      return null;
    }

    const [snapshot, ...rest] = undoStack;

    set((state) => {
      const newStacks = new Map(state.deviceUndoStacks);
      newStacks.set(DEFAULT_DEVICE_KEY, rest);
      return { undoStack: rest, deviceUndoStacks: newStacks };
    });

    return snapshot ?? null;
  },

  peekSnapshot: () => {
    const { undoStack } = get();
    return undoStack[0] ?? null;
  },

  getUndoCount: () => {
    return get().undoStack.length;
  },

  clearUndoStack: () => {
    set((state) => {
      const newStacks = new Map(state.deviceUndoStacks);
      newStacks.set(DEFAULT_DEVICE_KEY, []);
      return { undoStack: [], deviceUndoStacks: newStacks };
    });
  },

  canUndo: () => {
    return get().undoStack.length > 0;
  },
}));
