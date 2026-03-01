/**
 * Device Tab Bar Component
 *
 * Horizontal tab strip below the header showing one tab per device instance.
 * Provides visual connection status, active device highlighting, and
 * the ability to remove devices via a close button on hover.
 *
 * This is a second-level tab bar within the Editor view, separate from
 * the primary Editor/Librarian/Settings navigation tabs in the header.
 */

import React, { useState, useCallback } from 'react';
import { useDevicesStore } from '../stores/devices';
import { useControlsStore } from '../stores/controls';

export function DeviceTabBar(): React.ReactElement {
  const devices = useDevicesStore((state) => state.devices);
  const deviceOrder = useDevicesStore((state) => state.deviceOrder);
  const activeDeviceId = useDevicesStore((state) => state.activeDeviceId);
  const setActiveDevice = useDevicesStore((state) => state.setActiveDevice);
  const removeDevice = useDevicesStore((state) => state.removeDevice);

  const [confirmingRemoval, setConfirmingRemoval] = useState<string | null>(null);

  const handleTabClick = useCallback(
    (instanceId: string) => {
      setActiveDevice(instanceId);
    },
    [setActiveDevice],
  );

  const handleRemoveClick = useCallback(
    (event: React.MouseEvent, instanceId: string) => {
      event.stopPropagation();

      if (confirmingRemoval === instanceId) {
        // Second click: actually remove
        removeDevice(instanceId);
        useControlsStore.getState().removeDeviceState(instanceId);
        setConfirmingRemoval(null);
      } else {
        // First click: ask for confirmation
        setConfirmingRemoval(instanceId);
        // Auto-cancel confirmation after 3 seconds
        setTimeout(() => {
          setConfirmingRemoval((current) =>
            current === instanceId ? null : current,
          );
        }, 3000);
      }
    },
    [confirmingRemoval, removeDevice],
  );

  const orderedDevices = deviceOrder
    .map((id) => devices.get(id))
    .filter((device): device is NonNullable<typeof device> => device != null);

  if (orderedDevices.length === 0) {
    return <></>;
  }

  return (
    <div className="flex items-center gap-1 px-4 py-1 bg-nk-dark border-b border-nk-border overflow-x-auto">
      {orderedDevices.map((device) => {
        const isActive = device.instanceId === activeDeviceId;
        const isConfirming = confirmingRemoval === device.instanceId;

        return (
          <div
            key={device.instanceId}
            onClick={() => handleTabClick(device.instanceId)}
            className={`group flex items-center gap-2 px-3 py-1.5 rounded-t cursor-pointer transition-colors border-b-2 ${
              isActive
                ? 'bg-nk-light border-nk-accent text-white'
                : 'bg-transparent border-transparent text-gray-400 hover:text-gray-200 hover:bg-nk-light/50'
            }`}
          >
            {/* Connection indicator */}
            <span
              className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
                device.connected ? 'bg-green-400' : 'bg-red-400'
              }`}
            />

            {/* Device name */}
            <span className="text-xs font-medium truncate max-w-[120px]">
              {device.nickname}
            </span>

            {/* Close/remove button -- visible on hover or when confirming */}
            <button
              type="button"
              onClick={(e) => handleRemoveClick(e, device.instanceId)}
              className={`ml-1 flex-shrink-0 transition-opacity ${
                isConfirming
                  ? 'opacity-100'
                  : 'opacity-0 group-hover:opacity-100'
              }`}
              title={isConfirming ? 'Click again to confirm removal' : 'Remove device'}
              aria-label={isConfirming ? 'Confirm remove device' : 'Remove device'}
            >
              {isConfirming ? (
                <span className="text-xs text-red-400 font-medium">Remove?</span>
              ) : (
                <svg
                  className="w-3 h-3 text-gray-500 hover:text-gray-300"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              )}
            </button>
          </div>
        );
      })}
    </div>
  );
}
