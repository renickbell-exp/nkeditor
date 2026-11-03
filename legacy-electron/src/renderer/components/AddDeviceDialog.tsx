/**
 * Add Device Dialog
 *
 * Modal dialog for selecting and adding a new MIDI device instance.
 * Presents all supported device types from the device registry and
 * allows the user to assign an optional nickname before creation.
 *
 * On confirmation, creates a new device instance in the devices store
 * and initializes its control state in the controls store.
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import { getAllDeviceDefinitions, getDeviceDefinition } from '@shared/devices';
import { useDevicesStore } from '../stores/devices';
import { useControlsStore } from '../stores/controls';
import type { DeviceDefinition } from '@shared/devices';

interface AddDeviceDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export function AddDeviceDialog({ isOpen, onClose }: AddDeviceDialogProps): React.ReactElement | null {
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);
  const [nickname, setNickname] = useState('');
  const [availableDevices, setAvailableDevices] = useState<DeviceDefinition[]>([]);
  const dialogRef = useRef<HTMLDivElement>(null);

  // Load available devices when dialog opens
  useEffect(() => {
    if (isOpen) {
      setAvailableDevices(getAllDeviceDefinitions());
      setSelectedDeviceId(null);
      setNickname('');
    }
  }, [isOpen]);

  // Close on Escape key
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  // Close on backdrop click
  const handleBackdropClick = useCallback(
    (event: React.MouseEvent) => {
      if (dialogRef.current && !dialogRef.current.contains(event.target as Node)) {
        onClose();
      }
    },
    [onClose],
  );

  const handleConfirm = useCallback(() => {
    if (!selectedDeviceId) return;

    const trimmedNickname = nickname.trim();
    const instanceId = useDevicesStore
      .getState()
      .addDevice(selectedDeviceId, trimmedNickname || undefined);

    if (instanceId) {
      const deviceDef = getDeviceDefinition(selectedDeviceId);
      if (deviceDef) {
        useControlsStore.getState().initializeDevice(instanceId, deviceDef);
      }
    }

    onClose();
  }, [selectedDeviceId, nickname, onClose]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={handleBackdropClick}
    >
      <div
        ref={dialogRef}
        className="w-[420px] bg-nk-dark border border-nk-border rounded-lg shadow-xl"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-nk-border">
          <h2 className="text-base font-semibold text-gray-200">Add Device</h2>
          <button
            type="button"
            onClick={onClose}
            className="text-gray-500 hover:text-gray-300 transition-colors"
            aria-label="Close"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Device list */}
        <div className="p-5 space-y-4">
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-2">
              Select Device Type
            </label>
            <div className="space-y-2">
              {availableDevices.map((device) => (
                <button
                  key={device.id}
                  type="button"
                  onClick={() => setSelectedDeviceId(device.id)}
                  className={`w-full flex items-center gap-3 px-4 py-3 rounded border transition-colors text-left ${
                    selectedDeviceId === device.id
                      ? 'border-nk-accent bg-nk-accent/10 text-white'
                      : 'border-nk-border bg-nk-darker text-gray-300 hover:border-gray-500'
                  }`}
                >
                  <div className="flex-1">
                    <div className="text-sm font-medium">{device.name}</div>
                    <div className="text-xs text-gray-500">{device.manufacturer}</div>
                  </div>
                  <div className="text-xs text-gray-500">
                    {device.trackCount} tracks
                  </div>
                </button>
              ))}
            </div>
          </div>

          {/* Nickname input */}
          <div>
            <label htmlFor="deviceNickname" className="block text-xs font-medium text-gray-400 mb-1">
              Nickname (optional)
            </label>
            <input
              id="deviceNickname"
              type="text"
              maxLength={40}
              value={nickname}
              onChange={(e) => setNickname(e.target.value)}
              placeholder="e.g., Studio nK2, Live MidiMix..."
              className="w-full px-3 py-2 bg-nk-darker border border-nk-border rounded text-sm text-gray-200 placeholder-gray-600 focus:outline-none focus:ring-1 focus:ring-nk-accent"
            />
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 px-5 py-4 border-t border-nk-border">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200 transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleConfirm}
            disabled={!selectedDeviceId}
            className={`px-4 py-2 text-sm rounded transition-colors ${
              selectedDeviceId
                ? 'bg-nk-accent text-white hover:bg-opacity-80'
                : 'bg-gray-700 text-gray-500 cursor-not-allowed'
            }`}
          >
            Add Device
          </button>
        </div>
      </div>
    </div>
  );
}
