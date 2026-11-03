/**
 * UI Component Tests for Phase 4 (Multi-Device UI)
 *
 * Validates the structural behavior of the new multi-device UI components:
 * - DeviceSelector renders the device list from the store
 * - DeviceTabBar displays tabs with correct active/connection states
 * - DeviceEditorContainer routes to the correct editor based on device type
 * - MidiMixEditor renders the correct number of columns and master section
 *
 * These tests operate at the store/logic level without a DOM renderer,
 * verifying that the component logic integrates correctly with the
 * Zustand stores created in Phase 3.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useDevicesStore } from '../src/renderer/stores/devices';
import { useControlsStore } from '../src/renderer/stores/controls';
import { NANOKONTROL2_DEFINITION } from '../src/shared/devices/nanokontrol2';
import { AKAI_MIDIMIX_DEFINITION } from '../src/shared/devices/akai-midimix';
import { getAllDeviceDefinitions, getDeviceDefinition } from '../src/shared/devices/registry';

// =============================================================================
// Test Helpers
// =============================================================================

/**
 * Resets all Zustand stores to their initial state before each test.
 * Without this, state leaks between tests because Zustand stores are singletons.
 */
function resetAllStores(): void {
  useDevicesStore.setState({
    devices: new Map(),
    activeDeviceId: null,
    deviceOrder: [],
  });

  useControlsStore.setState({
    deviceControls: new Map(),
    tracks: Array(8)
      .fill(null)
      .map(() => ({
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
    selections: Array(8)
      .fill(null)
      .map(() => ({
        knob: false,
        slider: false,
      })),
  });
}

/**
 * Simulates the full device addition workflow that the AddDeviceDialog performs:
 * adds the device to the devices store and initializes its control state.
 */
function addDeviceWithControls(
  deviceType: string,
  nickname?: string,
): string | null {
  const instanceId = useDevicesStore.getState().addDevice(deviceType, nickname);
  if (instanceId) {
    const deviceDef = getDeviceDefinition(deviceType);
    if (deviceDef) {
      useControlsStore.getState().initializeDevice(instanceId, deviceDef);
    }
  }
  return instanceId;
}

// =============================================================================
// DeviceSelector Tests
// =============================================================================

describe('DeviceSelector (store integration)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('should list all configured devices from the store', () => {
    addDeviceWithControls('nanokontrol2', 'My nK2');
    addDeviceWithControls('akai-midimix', 'Studio Mix');

    const state = useDevicesStore.getState();
    const orderedDevices = state.deviceOrder
      .map((id) => state.devices.get(id))
      .filter((d): d is NonNullable<typeof d> => d != null);

    expect(orderedDevices).toHaveLength(2);
    expect(orderedDevices[0]!.nickname).toBe('My nK2');
    expect(orderedDevices[1]!.nickname).toBe('Studio Mix');
  });

  it('should reflect the active device', () => {
    const id1 = addDeviceWithControls('nanokontrol2')!;
    addDeviceWithControls('akai-midimix');

    expect(useDevicesStore.getState().activeDeviceId).toBe(id1);
  });

  it('should switch active device when selected', () => {
    addDeviceWithControls('nanokontrol2');
    const id2 = addDeviceWithControls('akai-midimix')!;

    useDevicesStore.getState().setActiveDevice(id2);

    expect(useDevicesStore.getState().activeDeviceId).toBe(id2);
  });

  it('should have an empty device list when no devices configured', () => {
    const state = useDevicesStore.getState();
    expect(state.devices.size).toBe(0);
    expect(state.activeDeviceId).toBeNull();
  });

  it('should have access to all supported device definitions for the add dialog', () => {
    const allDefs = getAllDeviceDefinitions();
    expect(allDefs.length).toBeGreaterThanOrEqual(2);

    const deviceIds = allDefs.map((d) => d.id).sort();
    expect(deviceIds).toContain('nanokontrol2');
    expect(deviceIds).toContain('akai-midimix');
  });
});

// =============================================================================
// DeviceTabBar Tests
// =============================================================================

describe('DeviceTabBar (store integration)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('should show tabs in deviceOrder sequence', () => {
    const id1 = addDeviceWithControls('nanokontrol2', 'First')!;
    const id2 = addDeviceWithControls('akai-midimix', 'Second')!;

    const state = useDevicesStore.getState();
    expect(state.deviceOrder).toEqual([id1, id2]);
  });

  it('should correctly identify the active tab', () => {
    const id1 = addDeviceWithControls('nanokontrol2')!;
    const id2 = addDeviceWithControls('akai-midimix')!;

    // id1 is active by default
    expect(useDevicesStore.getState().activeDeviceId).toBe(id1);

    // Switch to id2
    useDevicesStore.getState().setActiveDevice(id2);
    expect(useDevicesStore.getState().activeDeviceId).toBe(id2);
  });

  it('should reflect connection status per device', () => {
    const id1 = addDeviceWithControls('nanokontrol2')!;
    const id2 = addDeviceWithControls('akai-midimix')!;

    useDevicesStore.getState().updateDevice(id1, { connected: true });

    const dev1 = useDevicesStore.getState().devices.get(id1);
    const dev2 = useDevicesStore.getState().devices.get(id2);

    expect(dev1!.connected).toBe(true);
    expect(dev2!.connected).toBe(false);
  });

  it('should handle device removal and promote next device', () => {
    const id1 = addDeviceWithControls('nanokontrol2')!;
    const id2 = addDeviceWithControls('akai-midimix')!;

    // Remove active device
    useDevicesStore.getState().removeDevice(id1);
    useControlsStore.getState().removeDeviceState(id1);

    const state = useDevicesStore.getState();
    expect(state.deviceOrder).toHaveLength(1);
    expect(state.activeDeviceId).toBe(id2);
    expect(useControlsStore.getState().getDeviceState(id1)).toBeUndefined();
  });

  it('should produce an empty tab bar when no devices exist', () => {
    const state = useDevicesStore.getState();
    const orderedDevices = state.deviceOrder
      .map((id) => state.devices.get(id))
      .filter((d): d is NonNullable<typeof d> => d != null);

    expect(orderedDevices).toHaveLength(0);
  });
});

// =============================================================================
// DeviceEditorContainer Tests (routing logic)
// =============================================================================

describe('DeviceEditorContainer (routing logic)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('should route to nK2 editor for nanokontrol2 device type', () => {
    const instanceId = addDeviceWithControls('nanokontrol2')!;

    const activeDevice = useDevicesStore.getState().getActiveDevice();
    expect(activeDevice).not.toBeNull();
    expect(activeDevice!.deviceType).toBe('nanokontrol2');
    expect(activeDevice!.instanceId).toBe(instanceId);
  });

  it('should route to MidiMix editor for akai-midimix device type', () => {
    const instanceId = addDeviceWithControls('akai-midimix')!;

    const activeDevice = useDevicesStore.getState().getActiveDevice();
    expect(activeDevice).not.toBeNull();
    expect(activeDevice!.deviceType).toBe('akai-midimix');
    expect(activeDevice!.instanceId).toBe(instanceId);
  });

  it('should show no-devices state when no devices configured', () => {
    const activeDevice = useDevicesStore.getState().getActiveDevice();
    expect(activeDevice).toBeNull();
    expect(useDevicesStore.getState().devices.size).toBe(0);
  });

  it('should switch editor type when active device changes', () => {
    const nk2Id = addDeviceWithControls('nanokontrol2')!;
    const mixId = addDeviceWithControls('akai-midimix')!;

    // Initially nK2 is active
    let activeDevice = useDevicesStore.getState().getActiveDevice();
    expect(activeDevice!.deviceType).toBe('nanokontrol2');

    // Switch to MidiMix
    useDevicesStore.getState().setActiveDevice(mixId);
    activeDevice = useDevicesStore.getState().getActiveDevice();
    expect(activeDevice!.deviceType).toBe('akai-midimix');

    // Switch back to nK2
    useDevicesStore.getState().setActiveDevice(nk2Id);
    activeDevice = useDevicesStore.getState().getActiveDevice();
    expect(activeDevice!.deviceType).toBe('nanokontrol2');
  });
});

// =============================================================================
// MidiMixEditor Tests (structural validation)
// =============================================================================

describe('MidiMixEditor (structural validation)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('should render correct number of track columns (8)', () => {
    const instanceId = addDeviceWithControls('akai-midimix')!;

    const deviceState = useControlsStore.getState().getDeviceState(instanceId);
    expect(deviceState).toBeDefined();
    expect(deviceState!.tracks).toHaveLength(8);
  });

  it('should have 3 knobs + 1 fader + 3 buttons per track in control state', () => {
    const instanceId = addDeviceWithControls('akai-midimix')!;

    const deviceState = useControlsStore.getState().getDeviceState(instanceId);
    expect(deviceState).toBeDefined();

    for (let trackIndex = 0; trackIndex < 8; trackIndex++) {
      const track = deviceState!.tracks[trackIndex]!;

      // 3 knobs
      expect(track['knob1']).toBeDefined();
      expect(track['knob2']).toBeDefined();
      expect(track['knob3']).toBeDefined();

      // 1 fader
      expect(track['fader']).toBeDefined();

      // 3 buttons
      expect(track['mute']).toBeDefined();
      expect(track['solo']).toBeDefined();
      expect(track['rec_arm']).toBeDefined();

      // All controls start at default values
      expect(track['knob1']!.value).toBe(0);
      expect(track['fader']!.value).toBe(0);
      expect(track['mute']!.active).toBe(false);
    }
  });

  it('should have a master section with fader and 3 buttons', () => {
    const instanceId = addDeviceWithControls('akai-midimix')!;

    const deviceState = useControlsStore.getState().getDeviceState(instanceId);
    expect(deviceState).toBeDefined();

    const master = deviceState!.sections['master'];
    expect(master).toBeDefined();
    expect(master!['master_fader']).toBeDefined();
    expect(master!['bank_left']).toBeDefined();
    expect(master!['bank_right']).toBeDefined();
    expect(master!['solo_master']).toBeDefined();
  });

  it('should update track control values independently', () => {
    const instanceId = addDeviceWithControls('akai-midimix')!;

    // Update knob1 on track 0
    useControlsStore.getState().updateDeviceControl(instanceId, 0, 'knob1', 100);

    const deviceState = useControlsStore.getState().getDeviceState(instanceId);
    expect(deviceState!.tracks[0]!['knob1']!.value).toBe(100);

    // Other knobs on same track unchanged
    expect(deviceState!.tracks[0]!['knob2']!.value).toBe(0);
    expect(deviceState!.tracks[0]!['knob3']!.value).toBe(0);

    // Same knob on different track unchanged
    expect(deviceState!.tracks[1]!['knob1']!.value).toBe(0);
  });

  it('should update master section controls', () => {
    const instanceId = addDeviceWithControls('akai-midimix')!;

    useControlsStore.getState().updateSectionControl(instanceId, 'master', 'master_fader', 80);

    const deviceState = useControlsStore.getState().getDeviceState(instanceId);
    expect(deviceState!.sections['master']!['master_fader']!.value).toBe(80);
  });

  it('should toggle button states correctly', () => {
    const instanceId = addDeviceWithControls('akai-midimix')!;

    useControlsStore.getState().updateDeviceButton(instanceId, 2, 'mute', true);

    const deviceState = useControlsStore.getState().getDeviceState(instanceId);
    expect(deviceState!.tracks[2]!['mute']!.active).toBe(true);
    expect(deviceState!.tracks[2]!['mute']!.value).toBe(127);

    // Toggle off
    useControlsStore.getState().updateDeviceButton(instanceId, 2, 'mute', false);
    const updatedState = useControlsStore.getState().getDeviceState(instanceId);
    expect(updatedState!.tracks[2]!['mute']!.active).toBe(false);
    expect(updatedState!.tracks[2]!['mute']!.value).toBe(0);
  });
});

// =============================================================================
// AddDeviceDialog Integration Tests
// =============================================================================

describe('AddDeviceDialog (workflow integration)', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('should create device and initialize control state in one workflow', () => {
    const instanceId = addDeviceWithControls('akai-midimix', 'Live Mix')!;

    // Device should exist in devices store
    const device = useDevicesStore.getState().devices.get(instanceId);
    expect(device).toBeDefined();
    expect(device!.nickname).toBe('Live Mix');
    expect(device!.deviceType).toBe('akai-midimix');

    // Controls should be initialized
    const controlState = useControlsStore.getState().getDeviceState(instanceId);
    expect(controlState).toBeDefined();
    expect(controlState!.tracks).toHaveLength(8);
    expect(controlState!.sections['master']).toBeDefined();
  });

  it('should not initialize controls for unknown device type', () => {
    const instanceId = useDevicesStore.getState().addDevice('unknown-device');
    expect(instanceId).toBeNull();
    expect(useDevicesStore.getState().devices.size).toBe(0);
  });

  it('should support adding multiple devices of the same type', () => {
    const id1 = addDeviceWithControls('nanokontrol2', 'nK2 #1')!;
    const id2 = addDeviceWithControls('nanokontrol2', 'nK2 #2')!;

    expect(id1).not.toBe(id2);

    const dev1 = useDevicesStore.getState().devices.get(id1);
    const dev2 = useDevicesStore.getState().devices.get(id2);
    expect(dev1!.nickname).toBe('nK2 #1');
    expect(dev2!.nickname).toBe('nK2 #2');

    // Both should have independent control state
    const state1 = useControlsStore.getState().getDeviceState(id1);
    const state2 = useControlsStore.getState().getDeviceState(id2);
    expect(state1).toBeDefined();
    expect(state2).toBeDefined();

    // Modify one, verify the other is unaffected
    useControlsStore.getState().updateDeviceControl(id1, 0, 'knob', 100);
    expect(useControlsStore.getState().getDeviceState(id1)!.tracks[0]!['knob']!.value).toBe(100);
    expect(useControlsStore.getState().getDeviceState(id2)!.tracks[0]!['knob']!.value).toBe(0);
  });
});

// =============================================================================
// EditorView Backward Compatibility Tests
// =============================================================================

describe('EditorView backward compatibility', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('should preserve legacy controls store interface when no instanceId', () => {
    // The legacy EditorView reads from tracks and transport directly
    const state = useControlsStore.getState();
    expect(state.tracks).toHaveLength(8);
    expect(state.transport).toBeDefined();
    expect(state.transport.play.active).toBe(false);
  });

  it('should allow legacy updateControl without instanceId', () => {
    useControlsStore.getState().updateControl('track1.knob', 42);
    expect(useControlsStore.getState().tracks[0]!.knob.value).toBe(42);
  });

  it('should allow legacy updateButton without instanceId', () => {
    useControlsStore.getState().updateButton('track1.solo', true);
    expect(useControlsStore.getState().tracks[0]!.solo.active).toBe(true);
  });
});
