/**
 * Store Tests for Multi-Device Architecture (Phase 3)
 *
 * Validates the devices store, the generic per-device control state in the
 * controls store, the per-device undo stacks in the undo store, and the
 * device-aware preset filtering in the preset store.
 *
 * These tests run in a Node/Vitest environment without Electron APIs.
 * The stores are designed to function without the Electron API for exactly
 * this kind of headless testing.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useDevicesStore } from '../src/renderer/stores/devices';
import { useControlsStore } from '../src/renderer/stores/controls';
import { useUndoStore } from '../src/renderer/stores/undo';
import { usePresetStore } from '../src/renderer/stores/preset';
import { useConnectionStore } from '../src/renderer/stores/connection';
import { NANOKONTROL2_DEFINITION } from '../src/shared/devices/nanokontrol2';
import { AKAI_MIDIMIX_DEFINITION } from '../src/shared/devices/akai-midimix';

// =============================================================================
// Test Helpers
// =============================================================================

/**
 * Resets all Zustand stores to their initial state before each test.
 * Without this, state leaks between tests because Zustand stores are singletons.
 */
function resetAllStores(): void {
  // Reset devices store
  useDevicesStore.setState({
    devices: new Map(),
    activeDeviceId: null,
    deviceOrder: [],
  });

  // Reset controls store (both legacy and multi-device state)
  useControlsStore.setState({
    deviceControls: new Map(),
    tracks: Array(8).fill(null).map(() => ({
      knob: { value: 0 },
      slider: { value: 0 },
      solo: { active: false },
      mute: { active: false },
      rec: { active: false },
    })),
    transport: {
      rewind: { active: false },
      forward: { active: false },
      stop: { active: false },
      play: { active: false },
      record: { active: false },
      cycle: { active: false },
      track_left: { active: false },
      track_right: { active: false },
      marker_set: { active: false },
      marker_left: { active: false },
      marker_right: { active: false },
    },
    selections: Array(8).fill(null).map(() => ({
      knob: false,
      slider: false,
    })),
  });

  // Reset undo store
  useUndoStore.setState({
    undoStack: [],
    deviceUndoStacks: new Map(),
  });

  // Reset preset store
  usePresetStore.setState({
    presets: [],
    selectedId: null,
    currentPreset: null,
    abSlotA: null,
    abSlotB: null,
    abActive: 'A' as const,
    loading: false,
    error: null,
    hasUnsavedChanges: false,
    presetDeviceTypes: new Map(),
  });
}

// =============================================================================
// Devices Store Tests
// =============================================================================

describe('devices store', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('should start with empty state', () => {
    const state = useDevicesStore.getState();
    expect(state.devices.size).toBe(0);
    expect(state.activeDeviceId).toBeNull();
    expect(state.deviceOrder).toHaveLength(0);
  });

  describe('addDevice', () => {
    it('should create an instance with correct defaults for nanokontrol2', () => {
      const instanceId = useDevicesStore.getState().addDevice('nanokontrol2');
      expect(instanceId).not.toBeNull();

      const state = useDevicesStore.getState();
      const device = state.devices.get(instanceId!);
      expect(device).toBeDefined();
      expect(device!.deviceType).toBe('nanokontrol2');
      expect(device!.nickname).toBe('nanoKONTROL2');
      expect(device!.connected).toBe(false);
      expect(device!.connecting).toBe(false);
      expect(device!.inputPort).toBeNull();
      expect(device!.outputPort).toBeNull();
      expect(device!.virtualPort).toBe('nkEditor3 nanoKONTROL2 Out');
      expect(device!.currentPresetId).toBeNull();
      expect(device!.tabOrder).toBe(0);
    });

    it('should create an instance with correct defaults for akai-midimix', () => {
      const instanceId = useDevicesStore.getState().addDevice('akai-midimix');
      expect(instanceId).not.toBeNull();

      const device = useDevicesStore.getState().devices.get(instanceId!);
      expect(device).toBeDefined();
      expect(device!.deviceType).toBe('akai-midimix');
      expect(device!.nickname).toBe('MIDI Mix');
      expect(device!.virtualPort).toBe('nkEditor3 MIDI Mix Out');
    });

    it('should use custom nickname when provided', () => {
      const instanceId = useDevicesStore.getState().addDevice('nanokontrol2', 'My nK2');
      const device = useDevicesStore.getState().devices.get(instanceId!);
      expect(device!.nickname).toBe('My nK2');
      expect(device!.virtualPort).toBe('nkEditor3 My nK2 Out');
    });

    it('should return null for unknown device type', () => {
      const instanceId = useDevicesStore.getState().addDevice('unknown-device');
      expect(instanceId).toBeNull();
      expect(useDevicesStore.getState().devices.size).toBe(0);
    });

    it('should add to deviceOrder array', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2');
      const id2 = useDevicesStore.getState().addDevice('akai-midimix');

      const state = useDevicesStore.getState();
      expect(state.deviceOrder).toHaveLength(2);
      expect(state.deviceOrder[0]).toBe(id1);
      expect(state.deviceOrder[1]).toBe(id2);
    });

    it('should set first added device as active automatically', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2');
      expect(useDevicesStore.getState().activeDeviceId).toBe(id1);

      // Second device should NOT change active
      useDevicesStore.getState().addDevice('akai-midimix');
      expect(useDevicesStore.getState().activeDeviceId).toBe(id1);
    });

    it('should generate unique instance IDs', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2');
      const id2 = useDevicesStore.getState().addDevice('nanokontrol2');
      expect(id1).not.toBe(id2);
    });

    it('should assign incrementing tabOrder values', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2');
      const id2 = useDevicesStore.getState().addDevice('akai-midimix');

      const dev1 = useDevicesStore.getState().devices.get(id1!);
      const dev2 = useDevicesStore.getState().devices.get(id2!);
      expect(dev1!.tabOrder).toBe(0);
      expect(dev2!.tabOrder).toBe(1);
    });
  });

  describe('removeDevice', () => {
    it('should remove from devices map', () => {
      const instanceId = useDevicesStore.getState().addDevice('nanokontrol2')!;
      expect(useDevicesStore.getState().devices.has(instanceId)).toBe(true);

      useDevicesStore.getState().removeDevice(instanceId);
      expect(useDevicesStore.getState().devices.has(instanceId)).toBe(false);
      expect(useDevicesStore.getState().devices.size).toBe(0);
    });

    it('should remove from deviceOrder', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;
      const id2 = useDevicesStore.getState().addDevice('akai-midimix')!;

      useDevicesStore.getState().removeDevice(id1);

      const state = useDevicesStore.getState();
      expect(state.deviceOrder).toHaveLength(1);
      expect(state.deviceOrder[0]).toBe(id2);
    });

    it('should promote next device to active when active device is removed', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;
      const id2 = useDevicesStore.getState().addDevice('akai-midimix')!;

      expect(useDevicesStore.getState().activeDeviceId).toBe(id1);

      useDevicesStore.getState().removeDevice(id1);
      expect(useDevicesStore.getState().activeDeviceId).toBe(id2);
    });

    it('should set activeDeviceId to null when last device is removed', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;
      useDevicesStore.getState().removeDevice(id1);
      expect(useDevicesStore.getState().activeDeviceId).toBeNull();
    });

    it('should not change activeDeviceId when non-active device is removed', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;
      const id2 = useDevicesStore.getState().addDevice('akai-midimix')!;

      // id1 is active
      useDevicesStore.getState().removeDevice(id2);
      expect(useDevicesStore.getState().activeDeviceId).toBe(id1);
    });

    it('should be a no-op for non-existent instanceId', () => {
      useDevicesStore.getState().addDevice('nanokontrol2');
      const sizeBefore = useDevicesStore.getState().devices.size;

      useDevicesStore.getState().removeDevice('non-existent-id');
      expect(useDevicesStore.getState().devices.size).toBe(sizeBefore);
    });
  });

  describe('setActiveDevice', () => {
    it('should update activeDeviceId', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;
      const id2 = useDevicesStore.getState().addDevice('akai-midimix')!;

      expect(useDevicesStore.getState().activeDeviceId).toBe(id1);

      useDevicesStore.getState().setActiveDevice(id2);
      expect(useDevicesStore.getState().activeDeviceId).toBe(id2);
    });

    it('should be a no-op for non-existent instanceId', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;
      useDevicesStore.getState().setActiveDevice('non-existent');
      expect(useDevicesStore.getState().activeDeviceId).toBe(id1);
    });
  });

  describe('updateDevice', () => {
    it('should merge partial updates into device instance', () => {
      const instanceId = useDevicesStore.getState().addDevice('nanokontrol2')!;

      useDevicesStore.getState().updateDevice(instanceId, {
        connected: true,
        inputPort: 'nanoKONTROL2:nanoKONTROL2 MIDI 1',
        nickname: 'Studio nK2',
      });

      const device = useDevicesStore.getState().devices.get(instanceId);
      expect(device!.connected).toBe(true);
      expect(device!.inputPort).toBe('nanoKONTROL2:nanoKONTROL2 MIDI 1');
      expect(device!.nickname).toBe('Studio nK2');
      // Other fields should remain unchanged
      expect(device!.deviceType).toBe('nanokontrol2');
      expect(device!.connecting).toBe(false);
    });

    it('should not allow overwriting instanceId', () => {
      const instanceId = useDevicesStore.getState().addDevice('nanokontrol2')!;

      // Attempting to overwrite instanceId should be prevented
      useDevicesStore.getState().updateDevice(instanceId, {
        instanceId: 'hacked-id',
      } as Partial<any>);

      const device = useDevicesStore.getState().devices.get(instanceId);
      expect(device!.instanceId).toBe(instanceId);
    });
  });

  describe('reorderDevices', () => {
    it('should change the device order', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;
      const id2 = useDevicesStore.getState().addDevice('akai-midimix')!;

      useDevicesStore.getState().reorderDevices([id2, id1]);

      const state = useDevicesStore.getState();
      expect(state.deviceOrder[0]).toBe(id2);
      expect(state.deviceOrder[1]).toBe(id1);
    });

    it('should filter out non-existent instanceIds', () => {
      const id1 = useDevicesStore.getState().addDevice('nanokontrol2')!;

      useDevicesStore.getState().reorderDevices([id1, 'non-existent']);
      expect(useDevicesStore.getState().deviceOrder).toEqual([id1]);
    });
  });

  describe('getActiveDevice', () => {
    it('should return the active device instance', () => {
      useDevicesStore.getState().addDevice('nanokontrol2');
      const activeDevice = useDevicesStore.getState().getActiveDevice();
      expect(activeDevice).not.toBeNull();
      expect(activeDevice!.deviceType).toBe('nanokontrol2');
    });

    it('should return null when no devices exist', () => {
      expect(useDevicesStore.getState().getActiveDevice()).toBeNull();
    });
  });

  describe('getDeviceDefinition', () => {
    it('should return the device definition for a valid instance', () => {
      const instanceId = useDevicesStore.getState().addDevice('nanokontrol2')!;
      const def = useDevicesStore.getState().getDeviceDefinition(instanceId);
      expect(def).toBeDefined();
      expect(def!.id).toBe('nanokontrol2');
    });

    it('should return undefined for non-existent instance', () => {
      expect(useDevicesStore.getState().getDeviceDefinition('non-existent')).toBeUndefined();
    });
  });
});

// =============================================================================
// Controls Store - Multi-Device Tests
// =============================================================================

describe('controls store (multi-device)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  describe('initializeDevice', () => {
    it('should create correct state for nanoKONTROL2', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState).toBeDefined();

      // 8 tracks
      expect(deviceState!.tracks).toHaveLength(8);

      // Each track should have: knob, slider (continuous) + solo, mute, rec (buttons)
      for (let trackIndex = 0; trackIndex < 8; trackIndex++) {
        const track = deviceState!.tracks[trackIndex]!;

        // Continuous controls
        expect(track['knob']).toBeDefined();
        expect(track['knob']!.value).toBe(0);
        expect(track['slider']).toBeDefined();
        expect(track['slider']!.value).toBe(0);

        // Button controls
        expect(track['solo']).toBeDefined();
        expect(track['solo']!.active).toBe(false);
        expect(track['solo']!.value).toBe(0);
        expect(track['mute']).toBeDefined();
        expect(track['mute']!.active).toBe(false);
        expect(track['rec']).toBeDefined();
        expect(track['rec']!.active).toBe(false);
      }

      // Transport section
      expect(deviceState!.sections['transport']).toBeDefined();
      const transport = deviceState!.sections['transport']!;
      expect(transport['play']).toBeDefined();
      expect(transport['play']!.active).toBe(false);
      expect(transport['stop']).toBeDefined();
      expect(transport['rewind']).toBeDefined();
      expect(transport['forward']).toBeDefined();
      expect(transport['record']).toBeDefined();
      expect(transport['cycle']).toBeDefined();
      expect(transport['track_left']).toBeDefined();
      expect(transport['track_right']).toBeDefined();
      expect(transport['marker_set']).toBeDefined();
      expect(transport['marker_left']).toBeDefined();
      expect(transport['marker_right']).toBeDefined();
    });

    it('should create correct state for Akai MidiMix', () => {
      const instanceId = 'test-midimix';
      useControlsStore.getState().initializeDevice(instanceId, AKAI_MIDIMIX_DEFINITION);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState).toBeDefined();

      // 8 tracks
      expect(deviceState!.tracks).toHaveLength(8);

      // Each track should have: knob1, knob2, knob3, fader (continuous) + mute, solo, rec_arm (buttons)
      for (let trackIndex = 0; trackIndex < 8; trackIndex++) {
        const track = deviceState!.tracks[trackIndex]!;

        // Continuous controls
        expect(track['knob1']).toBeDefined();
        expect(track['knob1']!.value).toBe(0);
        expect(track['knob2']).toBeDefined();
        expect(track['knob2']!.value).toBe(0);
        expect(track['knob3']).toBeDefined();
        expect(track['knob3']!.value).toBe(0);
        expect(track['fader']).toBeDefined();
        expect(track['fader']!.value).toBe(0);

        // Button controls
        expect(track['mute']).toBeDefined();
        expect(track['mute']!.active).toBe(false);
        expect(track['solo']).toBeDefined();
        expect(track['solo']!.active).toBe(false);
        expect(track['rec_arm']).toBeDefined();
        expect(track['rec_arm']!.active).toBe(false);
      }

      // Master section
      expect(deviceState!.sections['master']).toBeDefined();
      const master = deviceState!.sections['master']!;
      expect(master['master_fader']).toBeDefined();
      expect(master['master_fader']!.value).toBe(0);
      expect(master['bank_left']).toBeDefined();
      expect(master['bank_left']!.active).toBe(false);
      expect(master['bank_right']).toBeDefined();
      expect(master['bank_right']!.active).toBe(false);
      expect(master['solo_master']).toBeDefined();
      expect(master['solo_master']!.active).toBe(false);
    });
  });

  describe('updateDeviceControl', () => {
    it('should change value for the correct device and control', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceControl(instanceId, 0, 'knob', 64);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[0]!['knob']!.value).toBe(64);
    });

    it('should not affect other controls on the same track', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceControl(instanceId, 0, 'knob', 64);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[0]!['slider']!.value).toBe(0);
    });

    it('should not affect other tracks', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceControl(instanceId, 0, 'knob', 64);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[1]!['knob']!.value).toBe(0);
    });

    it('should be a no-op for non-existent device', () => {
      // Should not throw
      useControlsStore.getState().updateDeviceControl('non-existent', 0, 'knob', 64);
      expect(useControlsStore.getState().getDeviceState('non-existent')).toBeUndefined();
    });

    it('should be a no-op for invalid track index', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceControl(instanceId, 99, 'knob', 64);

      // All tracks should remain at 0
      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      for (let i = 0; i < 8; i++) {
        expect(deviceState!.tracks[i]!['knob']!.value).toBe(0);
      }
    });

    it('should be a no-op for non-existent control name', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceControl(instanceId, 0, 'nonexistent', 64);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[0]!['knob']!.value).toBe(0);
    });
  });

  describe('updateDeviceButton', () => {
    it('should change active state for the correct button', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceButton(instanceId, 2, 'solo', true);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[2]!['solo']!.active).toBe(true);
      expect(deviceState!.tracks[2]!['solo']!.value).toBe(127);
    });

    it('should set value to 0 when deactivated', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceButton(instanceId, 0, 'solo', true);
      useControlsStore.getState().updateDeviceButton(instanceId, 0, 'solo', false);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[0]!['solo']!.active).toBe(false);
      expect(deviceState!.tracks[0]!['solo']!.value).toBe(0);
    });

    it('should not affect other buttons on the same track', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateDeviceButton(instanceId, 0, 'solo', true);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[0]!['mute']!.active).toBe(false);
      expect(deviceState!.tracks[0]!['rec']!.active).toBe(false);
    });
  });

  describe('updateSectionControl', () => {
    it('should update a section continuous control', () => {
      const instanceId = 'test-midimix';
      useControlsStore.getState().initializeDevice(instanceId, AKAI_MIDIMIX_DEFINITION);

      useControlsStore.getState().updateSectionControl(instanceId, 'master', 'master_fader', 100);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.sections['master']!['master_fader']!.value).toBe(100);
    });

    it('should be a no-op for non-existent section', () => {
      const instanceId = 'test-midimix';
      useControlsStore.getState().initializeDevice(instanceId, AKAI_MIDIMIX_DEFINITION);

      useControlsStore.getState().updateSectionControl(instanceId, 'nonexistent', 'control', 50);
      // Should not throw, device state should remain unchanged
      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.sections['master']!['master_fader']!.value).toBe(0);
    });
  });

  describe('updateSectionButton', () => {
    it('should update a section button state', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().updateSectionButton(instanceId, 'transport', 'play', true);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.sections['transport']!['play']!.active).toBe(true);
      expect(deviceState!.sections['transport']!['play']!.value).toBe(127);
    });
  });

  describe('device state isolation', () => {
    it('should maintain completely independent state between two devices', () => {
      const nk2Id = 'test-nk2';
      const mixId = 'test-midimix';

      useControlsStore.getState().initializeDevice(nk2Id, NANOKONTROL2_DEFINITION);
      useControlsStore.getState().initializeDevice(mixId, AKAI_MIDIMIX_DEFINITION);

      // Change nK2 track 0 knob
      useControlsStore.getState().updateDeviceControl(nk2Id, 0, 'knob', 100);
      // Change MidiMix track 0 knob1
      useControlsStore.getState().updateDeviceControl(mixId, 0, 'knob1', 50);

      // Verify nK2 state is independent
      const nk2State = useControlsStore.getState().getDeviceState(nk2Id);
      expect(nk2State!.tracks[0]!['knob']!.value).toBe(100);

      // Verify MidiMix state is independent
      const mixState = useControlsStore.getState().getDeviceState(mixId);
      expect(mixState!.tracks[0]!['knob1']!.value).toBe(50);

      // Verify cross-device values are NOT affected
      // nK2 should not have knob1
      expect(nk2State!.tracks[0]!['knob1']).toBeUndefined();
      // MidiMix should not have 'knob' (it has 'knob1', 'knob2', 'knob3')
      expect(mixState!.tracks[0]!['knob']).toBeUndefined();
    });

    it('should not affect device B when modifying device A', () => {
      const deviceA = 'device-a';
      const deviceB = 'device-b';

      useControlsStore.getState().initializeDevice(deviceA, NANOKONTROL2_DEFINITION);
      useControlsStore.getState().initializeDevice(deviceB, NANOKONTROL2_DEFINITION);

      // Modify every track on device A
      for (let i = 0; i < 8; i++) {
        useControlsStore.getState().updateDeviceControl(deviceA, i, 'knob', 127);
        useControlsStore.getState().updateDeviceControl(deviceA, i, 'slider', 64);
        useControlsStore.getState().updateDeviceButton(deviceA, i, 'solo', true);
      }

      // Device B should be completely untouched
      const deviceBState = useControlsStore.getState().getDeviceState(deviceB);
      for (let i = 0; i < 8; i++) {
        expect(deviceBState!.tracks[i]!['knob']!.value).toBe(0);
        expect(deviceBState!.tracks[i]!['slider']!.value).toBe(0);
        expect(deviceBState!.tracks[i]!['solo']!.active).toBe(false);
      }
    });
  });

  describe('removeDeviceState', () => {
    it('should clean up state for a removed device', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      expect(useControlsStore.getState().getDeviceState(instanceId)).toBeDefined();

      useControlsStore.getState().removeDeviceState(instanceId);

      expect(useControlsStore.getState().getDeviceState(instanceId)).toBeUndefined();
    });

    it('should not affect other devices when removing one', () => {
      const id1 = 'device-1';
      const id2 = 'device-2';

      useControlsStore.getState().initializeDevice(id1, NANOKONTROL2_DEFINITION);
      useControlsStore.getState().initializeDevice(id2, AKAI_MIDIMIX_DEFINITION);

      useControlsStore.getState().updateDeviceControl(id2, 0, 'knob1', 42);

      useControlsStore.getState().removeDeviceState(id1);

      expect(useControlsStore.getState().getDeviceState(id1)).toBeUndefined();
      const remainingState = useControlsStore.getState().getDeviceState(id2);
      expect(remainingState).toBeDefined();
      expect(remainingState!.tracks[0]!['knob1']!.value).toBe(42);
    });
  });

  describe('applyDeviceLabels', () => {
    it('should apply labels to track controls', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().applyDeviceLabels(instanceId, [
        { trackIndex: 0, controlName: 'knob', label: 'Filter Cutoff' },
        { trackIndex: 1, controlName: 'slider', label: 'Volume' },
      ]);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.tracks[0]!['knob']!.label).toBe('Filter Cutoff');
      expect(deviceState!.tracks[1]!['slider']!.label).toBe('Volume');
    });

    it('should apply labels to section controls', () => {
      const instanceId = 'test-nk2';
      useControlsStore.getState().initializeDevice(instanceId, NANOKONTROL2_DEFINITION);

      useControlsStore.getState().applyDeviceLabels(instanceId, [], [
        { sectionName: 'transport', controlName: 'play', label: 'Start' },
      ]);

      const deviceState = useControlsStore.getState().getDeviceState(instanceId);
      expect(deviceState!.sections['transport']!['play']!.label).toBe('Start');
    });
  });

  describe('legacy backward compatibility', () => {
    it('should still have tracks property with 8 default tracks', () => {
      const state = useControlsStore.getState();
      expect(state.tracks).toHaveLength(8);
      for (const track of state.tracks) {
        expect(track.knob.value).toBe(0);
        expect(track.slider.value).toBe(0);
        expect(track.solo.active).toBe(false);
        expect(track.mute.active).toBe(false);
        expect(track.rec.active).toBe(false);
      }
    });

    it('should still have transport property with default state', () => {
      const state = useControlsStore.getState();
      expect(state.transport.play.active).toBe(false);
      expect(state.transport.stop.active).toBe(false);
    });

    it('should still support legacy updateControl', () => {
      useControlsStore.getState().updateControl('track1.knob', 100);
      expect(useControlsStore.getState().tracks[0]!.knob.value).toBe(100);
    });

    it('should still support legacy updateButton', () => {
      useControlsStore.getState().updateButton('track3.solo', true);
      expect(useControlsStore.getState().tracks[2]!.solo.active).toBe(true);
    });

    it('should still support legacy transport button update', () => {
      useControlsStore.getState().updateButton('transport.play', true);
      expect(useControlsStore.getState().transport.play.active).toBe(true);
    });
  });
});

// =============================================================================
// Undo Store - Per-Device Tests
// =============================================================================

describe('undo store (per-device)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  const sampleControlValues = {
    tracks: Array(8).fill(null).map(() => ({
      knob: 0,
      slider: 0,
      solo: false,
      mute: false,
      rec: false,
    })),
    transport: {
      play: false,
      stop: false,
      rewind: false,
      forward: false,
      record: false,
      cycle: false,
      track_left: false,
      track_right: false,
      marker_set: false,
      marker_left: false,
      marker_right: false,
    },
  };

  const modifiedControlValues = {
    ...sampleControlValues,
    tracks: sampleControlValues.tracks.map((track, index) =>
      index === 0 ? { ...track, knob: 64 } : track,
    ),
  };

  describe('per-device stacks are independent', () => {
    it('should maintain separate undo stacks for different devices', () => {
      const store = useUndoStore.getState();

      store.pushDeviceSnapshot('device-a', 'change A1', sampleControlValues);
      store.pushDeviceSnapshot('device-a', 'change A2', modifiedControlValues);
      store.pushDeviceSnapshot('device-b', 'change B1', sampleControlValues);

      expect(store.getDeviceUndoCount('device-a')).toBe(2);
      expect(store.getDeviceUndoCount('device-b')).toBe(1);
    });

    it('should pop from correct device stack without affecting other', () => {
      const store = useUndoStore.getState();

      store.pushDeviceSnapshot('device-a', 'change A', sampleControlValues);
      store.pushDeviceSnapshot('device-b', 'change B', modifiedControlValues);

      const snapshotA = store.popDeviceSnapshot('device-a');
      expect(snapshotA).not.toBeNull();
      expect(snapshotA!.description).toBe('change A');

      // device-b should still have its snapshot
      expect(useUndoStore.getState().getDeviceUndoCount('device-b')).toBe(1);
      // device-a should now be empty
      expect(useUndoStore.getState().getDeviceUndoCount('device-a')).toBe(0);
    });

    it('should clear only the targeted device stack', () => {
      const store = useUndoStore.getState();

      store.pushDeviceSnapshot('device-a', 'change A', sampleControlValues);
      store.pushDeviceSnapshot('device-b', 'change B', sampleControlValues);

      store.clearDeviceUndoStack('device-a');

      expect(useUndoStore.getState().canDeviceUndo('device-a')).toBe(false);
      expect(useUndoStore.getState().canDeviceUndo('device-b')).toBe(true);
    });
  });

  describe('pushDeviceSnapshot', () => {
    it('should add snapshot to the correct device stack', () => {
      useUndoStore.getState().pushDeviceSnapshot('device-a', 'test push', sampleControlValues);

      expect(useUndoStore.getState().getDeviceUndoCount('device-a')).toBe(1);
      expect(useUndoStore.getState().getDeviceUndoCount('device-b')).toBe(0);
    });

    it('should deep clone control values to prevent reference issues', () => {
      const mutableValues = JSON.parse(JSON.stringify(sampleControlValues));
      useUndoStore.getState().pushDeviceSnapshot('device-a', 'test', mutableValues);

      // Mutate the original
      mutableValues.tracks[0].knob = 999;

      // The snapshot should NOT be affected
      const snapshot = useUndoStore.getState().peekDeviceSnapshot('device-a');
      expect(snapshot!.controlValues.tracks[0]!.knob).toBe(0);
    });

    it('should respect maxUndoSteps limit', () => {
      const maxSteps = useUndoStore.getState().maxUndoSteps;

      // Push more than maxUndoSteps snapshots
      for (let i = 0; i < maxSteps + 5; i++) {
        useUndoStore.getState().pushDeviceSnapshot('device-a', `step ${i}`, sampleControlValues);
      }

      expect(useUndoStore.getState().getDeviceUndoCount('device-a')).toBe(maxSteps);
    });
  });

  describe('popDeviceSnapshot', () => {
    it('should return most recent snapshot first (LIFO)', () => {
      const store = useUndoStore.getState();

      store.pushDeviceSnapshot('device-a', 'first', sampleControlValues);
      store.pushDeviceSnapshot('device-a', 'second', modifiedControlValues);

      const popped = useUndoStore.getState().popDeviceSnapshot('device-a');
      expect(popped!.description).toBe('second');

      const poppedAgain = useUndoStore.getState().popDeviceSnapshot('device-a');
      expect(poppedAgain!.description).toBe('first');
    });

    it('should return null for empty stack', () => {
      const result = useUndoStore.getState().popDeviceSnapshot('non-existent');
      expect(result).toBeNull();
    });
  });

  describe('peekDeviceSnapshot', () => {
    it('should return the top snapshot without removing it', () => {
      useUndoStore.getState().pushDeviceSnapshot('device-a', 'peek test', sampleControlValues);

      const peeked = useUndoStore.getState().peekDeviceSnapshot('device-a');
      expect(peeked!.description).toBe('peek test');

      // Count should remain the same
      expect(useUndoStore.getState().getDeviceUndoCount('device-a')).toBe(1);
    });
  });

  describe('canDeviceUndo', () => {
    it('should return false for empty/non-existent device stack', () => {
      expect(useUndoStore.getState().canDeviceUndo('non-existent')).toBe(false);
    });

    it('should return true when snapshots exist', () => {
      useUndoStore.getState().pushDeviceSnapshot('device-a', 'test', sampleControlValues);
      expect(useUndoStore.getState().canDeviceUndo('device-a')).toBe(true);
    });
  });

  describe('removeDeviceStack', () => {
    it('should completely remove the undo stack for a device', () => {
      useUndoStore.getState().pushDeviceSnapshot('device-a', 'test', sampleControlValues);
      expect(useUndoStore.getState().getDeviceUndoCount('device-a')).toBe(1);

      useUndoStore.getState().removeDeviceStack('device-a');
      expect(useUndoStore.getState().getDeviceUndoCount('device-a')).toBe(0);
      expect(useUndoStore.getState().canDeviceUndo('device-a')).toBe(false);
    });
  });

  describe('legacy backward compatibility', () => {
    it('should still support pushSnapshot without instanceId', () => {
      useUndoStore.getState().pushSnapshot('legacy push', sampleControlValues);
      expect(useUndoStore.getState().undoStack).toHaveLength(1);
      expect(useUndoStore.getState().canUndo()).toBe(true);
    });

    it('should still support popSnapshot without instanceId', () => {
      useUndoStore.getState().pushSnapshot('legacy pop', sampleControlValues);
      const snapshot = useUndoStore.getState().popSnapshot();
      expect(snapshot).not.toBeNull();
      expect(snapshot!.description).toBe('legacy pop');
      expect(useUndoStore.getState().undoStack).toHaveLength(0);
    });

    it('should still support clearUndoStack without instanceId', () => {
      useUndoStore.getState().pushSnapshot('to be cleared', sampleControlValues);
      useUndoStore.getState().clearUndoStack();
      expect(useUndoStore.getState().undoStack).toHaveLength(0);
      expect(useUndoStore.getState().canUndo()).toBe(false);
    });

    it('should still support peekSnapshot without instanceId', () => {
      useUndoStore.getState().pushSnapshot('peek me', sampleControlValues);
      const peeked = useUndoStore.getState().peekSnapshot();
      expect(peeked!.description).toBe('peek me');
      expect(useUndoStore.getState().getUndoCount()).toBe(1);
    });
  });
});

// =============================================================================
// Preset Store - Device Type Tests
// =============================================================================

describe('preset store (device-aware)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  describe('setPresetDeviceType', () => {
    it('should associate a device type with a preset', () => {
      usePresetStore.getState().setPresetDeviceType('preset-1', 'akai-midimix');
      expect(usePresetStore.getState().getPresetDeviceType('preset-1')).toBe('akai-midimix');
    });

    it('should overwrite previous device type', () => {
      usePresetStore.getState().setPresetDeviceType('preset-1', 'nanokontrol2');
      usePresetStore.getState().setPresetDeviceType('preset-1', 'akai-midimix');
      expect(usePresetStore.getState().getPresetDeviceType('preset-1')).toBe('akai-midimix');
    });
  });

  describe('getPresetDeviceType', () => {
    it('should default to nanokontrol2 for untagged presets', () => {
      expect(usePresetStore.getState().getPresetDeviceType('unknown-preset')).toBe('nanokontrol2');
    });
  });

  describe('getPresetsForDevice', () => {
    it('should filter presets by device type', () => {
      // Set up some presets in the store
      const mockPresets: any[] = [
        { id: 'preset-1', name: 'NK2 Preset', tags: [], createdAt: '', modifiedAt: '', favorite: false },
        { id: 'preset-2', name: 'MidiMix Preset', tags: [], createdAt: '', modifiedAt: '', favorite: false },
        { id: 'preset-3', name: 'Another NK2', tags: [], createdAt: '', modifiedAt: '', favorite: false },
      ];

      usePresetStore.setState({ presets: mockPresets });
      usePresetStore.getState().setPresetDeviceType('preset-2', 'akai-midimix');
      // preset-1 and preset-3 have no explicit type, so default to 'nanokontrol2'

      const nk2Presets = usePresetStore.getState().getPresetsForDevice('nanokontrol2');
      expect(nk2Presets).toHaveLength(2);
      expect(nk2Presets.map(p => p.id).sort()).toEqual(['preset-1', 'preset-3']);

      const mixPresets = usePresetStore.getState().getPresetsForDevice('akai-midimix');
      expect(mixPresets).toHaveLength(1);
      expect(mixPresets[0]!.id).toBe('preset-2');
    });

    it('should return empty array for device type with no presets', () => {
      const presets = usePresetStore.getState().getPresetsForDevice('akai-midimix');
      expect(presets).toHaveLength(0);
    });

    it('should treat all legacy presets as nanokontrol2', () => {
      const mockPresets: any[] = [
        { id: 'legacy-1', name: 'Old Preset 1', tags: [], createdAt: '', modifiedAt: '', favorite: false },
        { id: 'legacy-2', name: 'Old Preset 2', tags: [], createdAt: '', modifiedAt: '', favorite: false },
      ];

      usePresetStore.setState({ presets: mockPresets });

      const nk2Presets = usePresetStore.getState().getPresetsForDevice('nanokontrol2');
      expect(nk2Presets).toHaveLength(2);
    });
  });

  describe('legacy backward compatibility', () => {
    it('should still have all existing properties', () => {
      const state = usePresetStore.getState();
      expect(state.presets).toBeDefined();
      expect(state.selectedId).toBeNull();
      expect(state.currentPreset).toBeNull();
      expect(state.abSlotA).toBeNull();
      expect(state.abSlotB).toBeNull();
      expect(state.abActive).toBe('A');
      expect(state.loading).toBe(false);
      expect(state.error).toBeNull();
      expect(state.hasUnsavedChanges).toBe(false);
    });

    it('should still support selectPreset', () => {
      usePresetStore.getState().selectPreset('test-id');
      expect(usePresetStore.getState().selectedId).toBe('test-id');
    });

    it('should still support markUnsavedChanges', () => {
      usePresetStore.getState().markUnsavedChanges();
      expect(usePresetStore.getState().hasUnsavedChanges).toBe(true);
    });

    it('should still support getAllTags', () => {
      const mockPresets: any[] = [
        { id: '1', name: 'P1', tags: ['ambient', 'pad'], createdAt: '', modifiedAt: '', favorite: false },
        { id: '2', name: 'P2', tags: ['bass', 'ambient'], createdAt: '', modifiedAt: '', favorite: false },
      ];
      usePresetStore.setState({ presets: mockPresets });

      const tags = usePresetStore.getState().getAllTags();
      expect(tags).toEqual(['ambient', 'bass', 'pad']);
    });

    it('should still support getAllGroups', () => {
      const mockPresets: any[] = [
        { id: '1', name: 'P1', tags: [], group: 'Synths', createdAt: '', modifiedAt: '', favorite: false },
        { id: '2', name: 'P2', tags: [], group: 'Drums', createdAt: '', modifiedAt: '', favorite: false },
        { id: '3', name: 'P3', tags: [], createdAt: '', modifiedAt: '', favorite: false },
      ];
      usePresetStore.setState({ presets: mockPresets });

      const groups = usePresetStore.getState().getAllGroups();
      expect(groups).toEqual(['Drums', 'Synths']);
    });
  });
});

// =============================================================================
// Connection Store - Multi-Device Routing Tests
// =============================================================================

describe('connection store (multi-device)', () => {
  beforeEach(() => {
    resetAllStores();

    // Also reset connection store state relevant to multi-device
    useConnectionStore.setState({
      portDeviceMap: new Map(),
      connected: false,
      connecting: false,
      inputPort: null,
      outputPort: null,
      virtualPort: 'nkEditor3 Out',
      midiAvailable: false,
      availablePorts: { inputs: [], outputs: [] },
    });
  });

  it('should start with empty port-to-device map', () => {
    expect(useConnectionStore.getState().portDeviceMap.size).toBe(0);
  });

  it('should register a port for a device', () => {
    useConnectionStore.getState().registerPortForDevice('nanoKONTROL2:MIDI 1', 'device-a');

    expect(useConnectionStore.getState().getDeviceForPort('nanoKONTROL2:MIDI 1')).toBe('device-a');
  });

  it('should return null for unregistered port', () => {
    expect(useConnectionStore.getState().getDeviceForPort('unknown-port')).toBeNull();
  });

  it('should unregister a port', () => {
    useConnectionStore.getState().registerPortForDevice('port1', 'device-a');
    useConnectionStore.getState().unregisterPort('port1');

    expect(useConnectionStore.getState().getDeviceForPort('port1')).toBeNull();
  });

  it('should unregister all ports for a device', () => {
    useConnectionStore.getState().registerPortForDevice('port1', 'device-a');
    useConnectionStore.getState().registerPortForDevice('port2', 'device-a');
    useConnectionStore.getState().registerPortForDevice('port3', 'device-b');

    useConnectionStore.getState().unregisterAllPortsForDevice('device-a');

    expect(useConnectionStore.getState().getDeviceForPort('port1')).toBeNull();
    expect(useConnectionStore.getState().getDeviceForPort('port2')).toBeNull();
    // device-b's port should remain
    expect(useConnectionStore.getState().getDeviceForPort('port3')).toBe('device-b');
  });

  it('should preserve legacy connection state properties', () => {
    const state = useConnectionStore.getState();

    expect(state.connected).toBe(false);
    expect(state.connecting).toBe(false);
    expect(state.inputPort).toBeNull();
    expect(state.outputPort).toBeNull();
    expect(state.virtualPort).toBe('nkEditor3 Out');
    expect(state.midiAvailable).toBe(false);
    expect(state.availablePorts.inputs).toHaveLength(0);
    expect(state.availablePorts.outputs).toHaveLength(0);
  });
});
