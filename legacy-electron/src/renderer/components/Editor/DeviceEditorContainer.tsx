/**
 * Device Editor Container
 *
 * Routes to the correct device-specific editor based on the active device's
 * deviceType. When no devices are configured, shows a placeholder prompting
 * the user to add a device.
 *
 * This component sits between the DeviceTabBar and the device-specific
 * editors (EditorView for nK2, MidiMixEditor for MidiMix).
 */

import React from 'react';
import { EditorView } from './EditorView';
import { MidiMixEditor } from './MidiMix/MidiMixEditor';
import { useDevicesStore } from '../../stores/devices';

export function DeviceEditorContainer(): React.ReactElement {
  const activeDeviceId = useDevicesStore((state) => state.activeDeviceId);
  const devices = useDevicesStore((state) => state.devices);

  // No devices configured at all
  if (devices.size === 0 || !activeDeviceId) {
    return <NoDevicesPlaceholder />;
  }

  const activeDevice = devices.get(activeDeviceId);
  if (!activeDevice) {
    return <NoDevicesPlaceholder />;
  }

  switch (activeDevice.deviceType) {
    case 'nanokontrol2':
      return <EditorView instanceId={activeDeviceId} />;

    case 'akai-midimix':
      return <MidiMixEditor instanceId={activeDeviceId} />;

    default:
      return (
        <div className="h-full flex items-center justify-center p-8">
          <div className="text-center text-gray-500">
            <p className="text-lg font-medium mb-2">Unsupported device type</p>
            <p className="text-sm">
              No editor is available for device type "{activeDevice.deviceType}".
            </p>
          </div>
        </div>
      );
  }
}

// =============================================================================
// Placeholder for empty state
// =============================================================================

function NoDevicesPlaceholder(): React.ReactElement {
  return (
    <div className="h-full flex items-center justify-center p-8">
      <div className="text-center max-w-md">
        <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-nk-light flex items-center justify-center">
          <svg
            className="w-8 h-8 text-gray-500"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
            />
          </svg>
        </div>
        <h3 className="text-lg font-medium text-gray-300 mb-2">
          No devices configured
        </h3>
        <p className="text-sm text-gray-500 mb-4">
          Add a MIDI device to start editing its control mappings.
          Use the device selector in the header to add your first device.
        </p>
      </div>
    </div>
  );
}
