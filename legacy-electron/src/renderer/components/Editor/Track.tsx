/**
 * Track Component
 *
 * Represents a single track channel on the nanoKONTROL2.
 * Contains a knob, slider, and three buttons (solo, mute, rec).
 *
 * When GUI controls are manipulated, this component:
 * 1. Updates the local store state for UI reflection
 * 2. Sends MIDI CC messages to the virtual output port for DAW control
 */

import React from 'react';
import { Knob } from './Knob';
import { Slider } from './Slider';
import { Button } from './Button';
import type { TrackState } from '../../stores/controls';
import { useControlsStore } from '../../stores/controls';
import { usePresetStore } from '../../stores/preset';
import { HARDWARE_CC } from '@shared/constants';
import type { ElectronAPI } from '../../../main/preload';
import type { MappingEntry, MappingConfig } from '@shared/types';

// Retrieve electronAPI from window for MIDI output
const getElectronAPI = (): ElectronAPI | null => {
  return (window as { electronAPI?: ElectronAPI }).electronAPI ?? null;
};

// Default MIDI channel for GUI-initiated CC messages (0-indexed)
const DEFAULT_MIDI_CHANNEL = 0;

/**
 * Scale a GUI value (0-127) to the configured min/max output range.
 * Formula: min + (value / 127) * (max - min)
 * Result is clamped to 0-127 and rounded to an integer.
 */
function scaleValue(value: number, minValue: number = 0, maxValue: number = 127): number {
  // Handle edge case where minValue > maxValue by swapping
  const min = Math.min(minValue, maxValue);
  const max = Math.max(minValue, maxValue);

  // Apply scaling formula
  const scaled = min + (value / 127) * (max - min);

  // Clamp to 0-127 and round to integer
  return Math.round(Math.max(0, Math.min(127, scaled)));
}

/**
 * Get the MIDI value for a button based on its state and configured on/off values.
 * Uses custom onValue/offValue if configured, otherwise defaults to 127/0.
 * Result is clamped to 0-127.
 */
function getButtonMidiValue(active: boolean, mapping?: MappingEntry): number {
  if (active) {
    const onValue = mapping?.onValue ?? 127;
    return Math.max(0, Math.min(127, onValue));
  } else {
    const offValue = mapping?.offValue ?? 0;
    return Math.max(0, Math.min(127, offValue));
  }
}

interface TrackProps {
  trackNumber: number;
  track: TrackState;
}

export function Track({ trackNumber, track }: TrackProps): React.ReactElement {
  const updateControl = useControlsStore((state) => state.updateControl);
  const updateButton = useControlsStore((state) => state.updateButton);
  const selections = useControlsStore((state) => state.selections);
  const toggleControlSelection = useControlsStore((state) => state.toggleControlSelection);

  const trackIndex = trackNumber - 1;
  const selection = selections[trackIndex];

  const handleKnobChange = (value: number) => {
    // Update local store state for UI
    updateControl(`track${trackNumber}.knob`, value);

    // Send MIDI CC to virtual output port
    const api = getElectronAPI();
    if (api) {
      const ccNumber = HARDWARE_CC.KNOBS[trackNumber - 1];
      if (ccNumber !== undefined) {
        // Get the current mapping to apply min/max scaling
        const currentPreset = usePresetStore.getState().currentPreset;
        const trackIndex = trackNumber - 1;
        // nK2 editor components only handle MappingConfig presets (array-based tracks)
        const trackMapping = Array.isArray(currentPreset?.mapping.tracks)
          ? (currentPreset.mapping as MappingConfig).tracks[trackIndex]
          : undefined;
        const mapping = trackMapping?.knob;

        // Scale value from 0-127 to configured min-max range
        const minVal = mapping?.minValue ?? 0;
        const maxVal = mapping?.maxValue ?? 127;
        const scaledValue = scaleValue(value, minVal, maxVal);

        api.sendCC({ channel: DEFAULT_MIDI_CHANNEL, cc: ccNumber, value: scaledValue });
      }
    }

    // Mark preset as having unsaved changes
    usePresetStore.getState().markUnsavedChanges();
  };

  const handleSliderChange = (value: number) => {
    // Update local store state for UI
    updateControl(`track${trackNumber}.slider`, value);

    // Send MIDI CC to virtual output port
    const api = getElectronAPI();
    if (api) {
      const ccNumber = HARDWARE_CC.SLIDERS[trackNumber - 1];
      if (ccNumber !== undefined) {
        // Get the current mapping to apply min/max scaling
        const currentPreset = usePresetStore.getState().currentPreset;
        const trackIndex = trackNumber - 1;
        // nK2 editor components only handle MappingConfig presets (array-based tracks)
        const sliderTrackMapping = Array.isArray(currentPreset?.mapping.tracks)
          ? (currentPreset.mapping as MappingConfig).tracks[trackIndex]
          : undefined;
        const mapping = sliderTrackMapping?.slider;

        // Scale value from 0-127 to configured min-max range
        const minVal = mapping?.minValue ?? 0;
        const maxVal = mapping?.maxValue ?? 127;
        const scaledValue = scaleValue(value, minVal, maxVal);

        api.sendCC({ channel: DEFAULT_MIDI_CHANNEL, cc: ccNumber, value: scaledValue });
      }
    }

    // Mark preset as having unsaved changes
    usePresetStore.getState().markUnsavedChanges();
  };

  const handleButtonClick = (buttonType: 'solo' | 'mute' | 'rec') => {
    const currentActive = track[buttonType].active;
    const newActive = !currentActive;

    // Update local store state for UI
    updateButton(`track${trackNumber}.${buttonType}`, newActive);

    // Send MIDI CC to virtual output port
    const api = getElectronAPI();
    if (api) {
      const ccArrayKey = buttonType.toUpperCase() as keyof typeof HARDWARE_CC;
      const ccArray = HARDWARE_CC[ccArrayKey] as readonly number[];
      const ccNumber = ccArray[trackNumber - 1];
      if (ccNumber !== undefined) {
        // Get the current mapping to apply custom on/off values
        const currentPreset = usePresetStore.getState().currentPreset;
        const trackIndex = trackNumber - 1;
        // nK2 editor components only handle MappingConfig presets (array-based tracks)
        const buttonTrackMapping = Array.isArray(currentPreset?.mapping.tracks)
          ? (currentPreset.mapping as MappingConfig).tracks[trackIndex]
          : undefined;
        const mapping = buttonTrackMapping?.[buttonType];

        // Use custom on/off values if configured, otherwise default to 127/0
        const midiValue = getButtonMidiValue(newActive, mapping);
        api.sendCC({ channel: DEFAULT_MIDI_CHANNEL, cc: ccNumber, value: midiValue });
      }
    }

    // Mark preset as having unsaved changes
    usePresetStore.getState().markUnsavedChanges();
  };

  return (
    <div className="flex flex-col items-center gap-3 p-3 bg-nk-dark rounded-lg border border-nk-border flex-shrink-0">
      {/* Track number label */}
      <div className="text-xs font-medium text-gray-500">
        Track {trackNumber}
      </div>

      {/* Knob */}
      <Knob
        value={track.knob.value}
        label={track.knob.label}
        controlId={`track${trackNumber}.knob`}
        onValueChange={handleKnobChange}
        selected={selection?.knob ?? false}
        onToggleSelection={() => toggleControlSelection(trackIndex, 'knob')}
      />

      {/* Slider */}
      <Slider
        value={track.slider.value}
        label={track.slider.label}
        controlId={`track${trackNumber}.slider`}
        onValueChange={handleSliderChange}
        selected={selection?.slider ?? false}
        onToggleSelection={() => toggleControlSelection(trackIndex, 'slider')}
      />

      {/* Buttons */}
      <div className="flex flex-col gap-2">
        <Button
          type="solo"
          active={track.solo.active}
          label={track.solo.label}
          controlId={`track${trackNumber}.solo`}
          onClick={() => handleButtonClick('solo')}
        />
        <Button
          type="mute"
          active={track.mute.active}
          label={track.mute.label}
          controlId={`track${trackNumber}.mute`}
          onClick={() => handleButtonClick('mute')}
        />
        <Button
          type="rec"
          active={track.rec.active}
          label={track.rec.label}
          controlId={`track${trackNumber}.rec`}
          onClick={() => handleButtonClick('rec')}
        />
      </div>
    </div>
  );
}
