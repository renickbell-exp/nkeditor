/**
 * Device Selector Component
 *
 * Dropdown control in the header for selecting the active device.
 * Displays configured devices with connection status indicators and
 * provides an "Add Device..." option to open the AddDeviceDialog.
 */

import React, { useState, useRef, useEffect, useCallback } from 'react';
import { useDevicesStore } from '../stores/devices';
import { AddDeviceDialog } from './AddDeviceDialog';

export function DeviceSelector(): React.ReactElement {
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const devices = useDevicesStore((state) => state.devices);
  const deviceOrder = useDevicesStore((state) => state.deviceOrder);
  const activeDeviceId = useDevicesStore((state) => state.activeDeviceId);
  const setActiveDevice = useDevicesStore((state) => state.setActiveDevice);

  const activeDevice = activeDeviceId ? devices.get(activeDeviceId) : null;

  // Close dropdown on outside click
  useEffect(() => {
    if (!isDropdownOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsDropdownOpen(false);
      }
    };

    const timeoutId = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 50);

    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isDropdownOpen]);

  const handleSelectDevice = useCallback(
    (instanceId: string) => {
      setActiveDevice(instanceId);
      setIsDropdownOpen(false);
    },
    [setActiveDevice],
  );

  const handleAddDevice = useCallback(() => {
    setIsDropdownOpen(false);
    setIsAddDialogOpen(true);
  }, []);

  const orderedDevices = deviceOrder
    .map((id) => devices.get(id))
    .filter((device): device is NonNullable<typeof device> => device != null);

  return (
    <>
      <div ref={dropdownRef} className="relative">
        <button
          type="button"
          onClick={() => setIsDropdownOpen((prev) => !prev)}
          className="flex items-center gap-2 px-3 py-1.5 bg-nk-darker border border-nk-border rounded text-sm text-gray-300 hover:border-gray-500 transition-colors"
        >
          {activeDevice ? (
            <>
              <ConnectionDot connected={activeDevice.connected} />
              <span className="truncate max-w-[160px]">{activeDevice.nickname}</span>
            </>
          ) : (
            <span className="text-gray-500">No device</span>
          )}
          <svg className="w-3 h-3 text-gray-500 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
          </svg>
        </button>

        {isDropdownOpen && (
          <div className="absolute top-full right-0 mt-1 w-56 bg-nk-dark border border-nk-border rounded-lg shadow-xl z-40 py-1">
            {orderedDevices.map((device) => (
              <button
                key={device.instanceId}
                type="button"
                onClick={() => handleSelectDevice(device.instanceId)}
                className={`w-full flex items-center gap-2 px-3 py-2 text-sm text-left transition-colors ${
                  device.instanceId === activeDeviceId
                    ? 'bg-nk-accent/15 text-white'
                    : 'text-gray-300 hover:bg-nk-light'
                }`}
              >
                <ConnectionDot connected={device.connected} />
                <div className="flex-1 truncate">
                  <span className="block truncate">{device.nickname}</span>
                  <span className="text-xs text-gray-500">{device.deviceType}</span>
                </div>
              </button>
            ))}

            {orderedDevices.length > 0 && (
              <div className="border-t border-nk-border my-1" />
            )}

            <button
              type="button"
              onClick={handleAddDevice}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-400 hover:text-white hover:bg-nk-light transition-colors"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
              Add Device...
            </button>
          </div>
        )}
      </div>

      <AddDeviceDialog
        isOpen={isAddDialogOpen}
        onClose={() => setIsAddDialogOpen(false)}
      />
    </>
  );
}

// =============================================================================
// Sub-components
// =============================================================================

interface ConnectionDotProps {
  connected: boolean;
}

function ConnectionDot({ connected }: ConnectionDotProps): React.ReactElement {
  return (
    <span
      className={`w-2 h-2 rounded-full flex-shrink-0 ${
        connected ? 'bg-green-400' : 'bg-red-400'
      }`}
    />
  );
}
