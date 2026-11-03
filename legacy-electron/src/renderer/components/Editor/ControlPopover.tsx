/**
 * Control Popover Component
 *
 * A popover that appears when clicking on a control (knob, slider, button).
 * Allows editing the output CC number, output channel, and label for the control.
 *
 * Changes are stored in the current preset's mapping configuration and can be
 * persisted by saving the preset.
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import { usePresetStore } from '../../stores/preset';
import { useControlsStore } from '../../stores/controls';
import { useSettingsStore } from '../../stores/settings';
import type { MappingEntry, MappingConfig, ButtonBehavior, TrackMapping, TransportMapping } from '@shared/types';
import type { Preset } from '@shared/ipc-protocol';

// =============================================================================
// Types
// =============================================================================

type ControlKind = 'knob' | 'slider' | 'button';

interface ControlPopoverProps {
  /** Whether the popover is visible */
  isOpen: boolean;
  /** Callback to close the popover */
  onClose: () => void;
  /** The control identifier (e.g., "track1.knob", "transport.play") */
  controlId: string;
  /** The kind of control (determines available options) */
  controlKind: ControlKind;
  /** Position for the popover relative to viewport */
  anchorPosition: { x: number; y: number };
}

interface ParsedControlId {
  section: 'track' | 'transport';
  trackNumber?: number;
  controlType: string;
}

// =============================================================================
// Helpers
// =============================================================================

/**
 * Parses a control ID string into its components.
 * Examples: "track1.knob" -> { section: 'track', trackNumber: 1, controlType: 'knob' }
 *           "transport.play" -> { section: 'transport', controlType: 'play' }
 */
function parseControlId(controlId: string): ParsedControlId | null {
  const parts = controlId.split('.');
  if (parts.length !== 2) return null;

  const [section, controlType] = parts;
  if (!section || !controlType) return null;

  if (section.startsWith('track')) {
    const trackNumber = parseInt(section.replace('track', ''), 10);
    if (isNaN(trackNumber) || trackNumber < 1 || trackNumber > 8) return null;
    return { section: 'track', trackNumber, controlType };
  }

  if (section === 'transport') {
    return { section: 'transport', controlType };
  }

  return null;
}

/**
 * Gets the mapping entry for a control from the current preset.
 */
function getMappingEntry(
  controlId: string,
  preset: Preset | null
): MappingEntry | null {
  if (!preset) return null;
  if (!Array.isArray(preset.mapping.tracks)) return null;

  const parsed = parseControlId(controlId);
  if (!parsed) return null;

  // These editor components only handle nK2-shaped MappingConfig presets
  const mapping = preset.mapping as MappingConfig;

  if (parsed.section === 'track' && parsed.trackNumber !== undefined) {
    const trackIndex = parsed.trackNumber - 1;
    const track = mapping.tracks[trackIndex];
    if (!track) return null;
    const controlKey = parsed.controlType as keyof TrackMapping;
    return track[controlKey] ?? null;
  }

  if (parsed.section === 'transport') {
    const transportKey = parsed.controlType as keyof TransportMapping;
    return mapping.transport[transportKey] ?? null;
  }

  return null;
}

/**
 * Validates a CC number (0-127).
 */
function isValidCC(value: number): boolean {
  return Number.isInteger(value) && value >= 0 && value <= 127;
}

/**
 * Validates a MIDI channel (1-16).
 */
function isValidChannel(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 16;
}

// =============================================================================
// Component
// =============================================================================

export function ControlPopover({
  isOpen,
  onClose,
  controlId,
  controlKind,
  anchorPosition,
}: ControlPopoverProps): React.ReactElement | null {
  const currentPreset = usePresetStore((state) => state.currentPreset);
  const globalMidiChannel = useSettingsStore((state) => state.config?.globalMidiChannel ?? 1);

  // Local form state
  const [outputCC, setOutputCC] = useState<string>('');
  const [channel, setChannel] = useState<string>('');
  const [label, setLabel] = useState<string>('');
  const [behavior, setBehavior] = useState<ButtonBehavior>('toggle');
  const [minValue, setMinValue] = useState<number>(0);
  const [maxValue, setMaxValue] = useState<number>(127);
  const [onValue, setOnValue] = useState<number>(127);
  const [offValue, setOffValue] = useState<number>(0);
  const [errors, setErrors] = useState<{ outputCC?: string; channel?: string }>({});
  const [hasChanges, setHasChanges] = useState(false);

  const popoverRef = useRef<HTMLDivElement>(null);
  const outputCCInputRef = useRef<HTMLInputElement>(null);

  // Initialize form state when popover opens or control changes
  useEffect(() => {
    if (isOpen && currentPreset) {
      const entry = getMappingEntry(controlId, currentPreset);
      if (entry) {
        setOutputCC(String(entry.outputCC));
        // Use entry's channel if set, otherwise fall back to global MIDI channel
        setChannel(String(entry.channel ?? globalMidiChannel));
        setLabel(entry.label ?? '');
        setBehavior(entry.behavior ?? 'toggle');
        setMinValue(entry.minValue ?? 0);
        setMaxValue(entry.maxValue ?? 127);
        setOnValue(entry.onValue ?? 127);
        setOffValue(entry.offValue ?? 0);
        setErrors({});
        setHasChanges(false);
      } else {
        // No existing entry - use global MIDI channel as default
        setChannel(String(globalMidiChannel));
        setErrors({});
        setHasChanges(false);
      }
    }
  }, [isOpen, controlId, currentPreset, globalMidiChannel]);

  // Focus the output CC input when popover opens
  useEffect(() => {
    if (!isOpen || !outputCCInputRef.current) {
      return;
    }

    // Small delay to ensure the popover is rendered
    const timeout = setTimeout(() => {
      outputCCInputRef.current?.focus();
      outputCCInputRef.current?.select();
    }, 50);

    return () => clearTimeout(timeout);
  }, [isOpen]);

  // Close on escape key
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

  // Close when clicking outside
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(event.target as Node)) {
        onClose();
      }
    };

    // Delay adding the listener to avoid immediate close from the opening click
    const timeout = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 100);

    return () => {
      clearTimeout(timeout);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen, onClose]);

  // Handle save - updates the current preset in memory
  const handleSave = useCallback(() => {
    const newErrors: { outputCC?: string; channel?: string } = {};
    const ccValue = parseInt(outputCC, 10);
    const channelValue = parseInt(channel, 10);

    if (!isValidCC(ccValue)) {
      newErrors.outputCC = 'CC must be 0-127';
    }
    if (!isValidChannel(channelValue)) {
      newErrors.channel = 'Channel must be 1-16';
    }

    if (Object.keys(newErrors).length > 0) {
      setErrors(newErrors);
      return;
    }

    if (!currentPreset) {
      console.warn('[ControlPopover] No current preset to update');
      onClose();
      return;
    }

    const parsed = parseControlId(controlId);
    if (!parsed) {
      console.warn('[ControlPopover] Invalid control ID:', controlId);
      onClose();
      return;
    }

    // Create updated mapping entry
    const updatedEntry: MappingEntry = {
      inputCC: getMappingEntry(controlId, currentPreset)?.inputCC ?? ccValue,
      outputCC: ccValue,
      channel: channelValue,
      label: label.trim() || undefined,
      behavior: controlKind === 'button' ? behavior : undefined,
      // Only include min/max for continuous controls (knob/slider)
      minValue: controlKind !== 'button' ? minValue : undefined,
      maxValue: controlKind !== 'button' ? maxValue : undefined,
      // Only include on/off values for buttons
      onValue: controlKind === 'button' ? onValue : undefined,
      offValue: controlKind === 'button' ? offValue : undefined,
    };

    // Create a deep copy of the current preset with the updated mapping
    const updatedPreset = JSON.parse(JSON.stringify(currentPreset));

    if (parsed.section === 'track' && parsed.trackNumber !== undefined) {
      const trackIndex = parsed.trackNumber - 1;
      updatedPreset.mapping.tracks[trackIndex][parsed.controlType] = updatedEntry;
    } else if (parsed.section === 'transport') {
      updatedPreset.mapping.transport[parsed.controlType] = updatedEntry;
    }

    // Update the modified timestamp
    updatedPreset.metadata.modifiedAt = new Date().toISOString();

    // Update the store with the modified preset (in-memory only, not persisted)
    // This marks the preset as having unsaved changes
    usePresetStore.getState().setCurrentPreset(updatedPreset);

    // Sync labels to the controls store so the Editor GUI reflects the updated labels
    useControlsStore.getState().applyLabelsFromMapping(updatedPreset.mapping);

    // Also apply the updated mapping to the MIDI manager so output CC changes take effect immediately
    const api = (window as { electronAPI?: { applyMapping: (req: { mapping: typeof updatedPreset.mapping }) => Promise<{ success: boolean }> } }).electronAPI;
    if (api) {
      api.applyMapping({ mapping: updatedPreset.mapping }).catch((err) => {
        console.error('[ControlPopover] Failed to apply mapping:', err);
      });
    }

    onClose();
  }, [outputCC, channel, label, behavior, minValue, maxValue, onValue, offValue, currentPreset, controlId, controlKind, onClose]);

  // Handle input changes
  const handleOutputCCChange = (value: string) => {
    setOutputCC(value);
    setHasChanges(true);
    setErrors((prev) => ({ ...prev, outputCC: undefined }));
  };

  const handleChannelChange = (value: string) => {
    setChannel(value);
    setHasChanges(true);
    setErrors((prev) => ({ ...prev, channel: undefined }));
  };

  const handleLabelChange = (value: string) => {
    setLabel(value);
    setHasChanges(true);
  };

  const handleBehaviorChange = (value: ButtonBehavior) => {
    setBehavior(value);
    setHasChanges(true);
  };

  // Handle enter key to save
  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Enter' && hasChanges) {
      event.preventDefault();
      handleSave();
    }
  };

  if (!isOpen) return null;

  // Parse control ID for display
  const parsed = parseControlId(controlId);
  const displayName = parsed
    ? parsed.section === 'track'
      ? `Track ${parsed.trackNumber} ${parsed.controlType.charAt(0).toUpperCase() + parsed.controlType.slice(1)}`
      : `Transport ${parsed.controlType.replace('_', ' ').replace(/\b\w/g, (c) => c.toUpperCase())}`
    : controlId;

  // Calculate position to keep popover within viewport
  const popoverWidth = 280;
  // Buttons have behavior + on/off values, knobs/sliders have min/max range
  const popoverHeight = controlKind === 'button' ? 420 : 340;
  const padding = 16;

  let left = anchorPosition.x;
  let top = anchorPosition.y + 10;

  // Adjust if would overflow right edge
  if (left + popoverWidth + padding > window.innerWidth) {
    left = window.innerWidth - popoverWidth - padding;
  }

  // Adjust if would overflow bottom edge
  if (top + popoverHeight + padding > window.innerHeight) {
    top = anchorPosition.y - popoverHeight - 10;
  }

  // Ensure minimum distance from edges
  left = Math.max(padding, left);
  top = Math.max(padding, top);

  const entry = getMappingEntry(controlId, currentPreset);

  return (
    <div
      ref={popoverRef}
      className="fixed z-50 bg-nk-dark border border-nk-border rounded-lg shadow-xl"
      style={{
        left: `${left}px`,
        top: `${top}px`,
        width: `${popoverWidth}px`,
      }}
      onKeyDown={handleKeyDown}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-nk-border">
        <h3 className="text-sm font-semibold text-gray-200">{displayName}</h3>
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

      {/* Content */}
      <div className="p-4 space-y-4">
        {!currentPreset ? (
          <div className="text-sm text-gray-500">
            No preset loaded. Load a preset first to edit mappings.
          </div>
        ) : (
          <>
            {/* Input CC (read-only info) */}
            <div className="flex items-center justify-between">
              <span className="text-xs text-gray-500">Input CC (hardware):</span>
              <span className="text-xs font-mono text-gray-400">{entry?.inputCC ?? '-'}</span>
            </div>

            {/* Output CC */}
            <div>
              <label htmlFor="outputCC" className="block text-xs font-medium text-gray-400 mb-1">
                Output CC (to DAW)
              </label>
              <input
                ref={outputCCInputRef}
                id="outputCC"
                type="number"
                min={0}
                max={127}
                value={outputCC}
                onChange={(e) => handleOutputCCChange(e.target.value)}
                className={`w-full px-3 py-2 bg-nk-darker border rounded text-sm font-mono text-gray-200 focus:outline-none focus:ring-1 ${
                  errors.outputCC
                    ? 'border-red-500 focus:ring-red-500'
                    : 'border-nk-border focus:ring-nk-accent'
                }`}
              />
              {errors.outputCC && (
                <p className="mt-1 text-xs text-red-400">{errors.outputCC}</p>
              )}
            </div>

            {/* Channel */}
            <div>
              <label htmlFor="channel" className="block text-xs font-medium text-gray-400 mb-1">
                MIDI Channel
              </label>
              <input
                id="channel"
                type="number"
                min={1}
                max={16}
                value={channel}
                onChange={(e) => handleChannelChange(e.target.value)}
                className={`w-full px-3 py-2 bg-nk-darker border rounded text-sm font-mono text-gray-200 focus:outline-none focus:ring-1 ${
                  errors.channel
                    ? 'border-red-500 focus:ring-red-500'
                    : 'border-nk-border focus:ring-nk-accent'
                }`}
              />
              {errors.channel && (
                <p className="mt-1 text-xs text-red-400">{errors.channel}</p>
              )}
            </div>

            {/* Label */}
            <div>
              <label htmlFor="label" className="block text-xs font-medium text-gray-400 mb-1">
                Label (optional)
              </label>
              <input
                id="label"
                type="text"
                maxLength={50}
                value={label}
                onChange={(e) => handleLabelChange(e.target.value)}
                placeholder="e.g., Volume, Pan, Filter..."
                className="w-full px-3 py-2 bg-nk-darker border border-nk-border rounded text-sm text-gray-200 placeholder-gray-600 focus:outline-none focus:ring-1 focus:ring-nk-accent"
              />
            </div>

            {/* Min/Max Value Range (only for continuous controls) */}
            {(controlKind === 'knob' || controlKind === 'slider') && (
              <div className="space-y-2">
                <label className="text-xs font-medium text-gray-400">Output Range</label>
                <div className="flex gap-2 items-center">
                  <input
                    type="number"
                    min={0}
                    max={127}
                    value={minValue}
                    onChange={(e) => {
                      setMinValue(parseInt(e.target.value) || 0);
                      setHasChanges(true);
                    }}
                    className="w-16 px-2 py-1.5 bg-nk-darker border border-nk-border rounded text-sm font-mono text-gray-200 text-center focus:outline-none focus:ring-1 focus:ring-nk-accent"
                    placeholder="Min"
                  />
                  <span className="text-gray-500 text-sm">to</span>
                  <input
                    type="number"
                    min={0}
                    max={127}
                    value={maxValue}
                    onChange={(e) => {
                      setMaxValue(parseInt(e.target.value) || 127);
                      setHasChanges(true);
                    }}
                    className="w-16 px-2 py-1.5 bg-nk-darker border border-nk-border rounded text-sm font-mono text-gray-200 text-center focus:outline-none focus:ring-1 focus:ring-nk-accent"
                    placeholder="Max"
                  />
                </div>
                <p className="text-xs text-gray-500">
                  Scales 0-127 input to this output range
                </p>
              </div>
            )}

            {/* Behavior (buttons only) */}
            {controlKind === 'button' && (
              <>
                <div>
                  <label className="block text-xs font-medium text-gray-400 mb-2">
                    Button Behavior
                  </label>
                  <div className="flex gap-3">
                    <label className="flex items-center gap-2 cursor-pointer">
                      <input
                        type="radio"
                        name="behavior"
                        value="toggle"
                        checked={behavior === 'toggle'}
                        onChange={() => handleBehaviorChange('toggle')}
                        className="text-nk-accent focus:ring-nk-accent"
                      />
                      <span className="text-sm text-gray-300">Toggle</span>
                    </label>
                    <label className="flex items-center gap-2 cursor-pointer">
                      <input
                        type="radio"
                        name="behavior"
                        value="momentary"
                        checked={behavior === 'momentary'}
                        onChange={() => handleBehaviorChange('momentary')}
                        className="text-nk-accent focus:ring-nk-accent"
                      />
                      <span className="text-sm text-gray-300">Momentary</span>
                    </label>
                  </div>
                </div>

                {/* On/Off Values for buttons */}
                <div className="space-y-2">
                  <label className="text-xs font-medium text-gray-400">Button Output Values</label>
                  <div className="flex gap-2 items-center">
                    <div className="flex-1">
                      <label className="block text-xs text-gray-500 mb-1">Off</label>
                      <input
                        type="number"
                        min={0}
                        max={127}
                        value={offValue}
                        onChange={(e) => {
                          setOffValue(parseInt(e.target.value) || 0);
                          setHasChanges(true);
                        }}
                        className="w-full px-2 py-1.5 bg-nk-darker border border-nk-border rounded text-sm font-mono text-gray-200 text-center focus:outline-none focus:ring-1 focus:ring-nk-accent"
                      />
                    </div>
                    <div className="flex-1">
                      <label className="block text-xs text-gray-500 mb-1">On</label>
                      <input
                        type="number"
                        min={0}
                        max={127}
                        value={onValue}
                        onChange={(e) => {
                          setOnValue(parseInt(e.target.value) || 127);
                          setHasChanges(true);
                        }}
                        className="w-full px-2 py-1.5 bg-nk-darker border border-nk-border rounded text-sm font-mono text-gray-200 text-center focus:outline-none focus:ring-1 focus:ring-nk-accent"
                      />
                    </div>
                  </div>
                  <p className="text-xs text-gray-500">
                    CC values sent for button states
                  </p>
                </div>
              </>
            )}
          </>
        )}
      </div>

      {/* Footer */}
      {currentPreset && (
        <div className="flex justify-end gap-2 px-4 py-3 border-t border-nk-border">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-gray-400 hover:text-gray-200 transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={!hasChanges}
            className={`px-3 py-1.5 text-sm rounded transition-colors ${
              hasChanges
                ? 'bg-nk-accent text-white hover:bg-opacity-80'
                : 'bg-gray-700 text-gray-500 cursor-not-allowed'
            }`}
          >
            Apply
          </button>
        </div>
      )}
    </div>
  );
}
