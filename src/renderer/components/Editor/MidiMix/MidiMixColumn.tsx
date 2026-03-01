/**
 * MidiMix Column Component
 *
 * Renders a single channel strip for the Akai MidiMix:
 * 3 knobs stacked vertically, then 3 buttons (mute/solo/rec_arm),
 * then 1 fader at the bottom.
 *
 * Uses the shared Knob, Button, and Slider primitives, adapting
 * their control IDs to the generic per-device state format.
 */

import React, { useCallback } from 'react';
import { Knob } from '../Knob';
import { Button } from '../Button';
import { Slider } from '../Slider';
import { useControlsStore } from '../../../stores/controls';
import type { GenericControlValue } from '../../../stores/controls';

interface MidiMixColumnProps {
  instanceId: string;
  trackIndex: number;
  trackState: Record<string, GenericControlValue>;
}

export function MidiMixColumn({
  instanceId,
  trackIndex,
  trackState,
}: MidiMixColumnProps): React.ReactElement {
  const updateDeviceControl = useControlsStore((state) => state.updateDeviceControl);
  const updateDeviceButton = useControlsStore((state) => state.updateDeviceButton);

  const trackNumber = trackIndex + 1;

  const handleKnobChange = useCallback(
    (controlName: string, value: number) => {
      updateDeviceControl(instanceId, trackIndex, controlName, value);
    },
    [instanceId, trackIndex, updateDeviceControl],
  );

  const handleButtonToggle = useCallback(
    (controlName: string) => {
      const currentActive = trackState[controlName]?.active ?? false;
      updateDeviceButton(instanceId, trackIndex, controlName, !currentActive);
    },
    [instanceId, trackIndex, trackState, updateDeviceButton],
  );

  const handleFaderChange = useCallback(
    (value: number) => {
      updateDeviceControl(instanceId, trackIndex, 'fader', value);
    },
    [instanceId, trackIndex, updateDeviceControl],
  );

  const knob1 = trackState['knob1'];
  const knob2 = trackState['knob2'];
  const knob3 = trackState['knob3'];
  const mute = trackState['mute'];
  const solo = trackState['solo'];
  const recArm = trackState['rec_arm'];
  const fader = trackState['fader'];

  return (
    <div className="flex flex-col items-center gap-3 p-3 bg-nk-dark rounded-lg border border-nk-border flex-shrink-0">
      {/* Track number label */}
      <div className="text-xs font-medium text-gray-500">
        Track {trackNumber}
      </div>

      {/* Three knobs stacked vertically */}
      <Knob
        value={knob1?.value ?? 0}
        label={knob1?.label ?? `K1`}
        controlId={`track${trackNumber}.knob1`}
        onValueChange={(value) => handleKnobChange('knob1', value)}
      />
      <Knob
        value={knob2?.value ?? 0}
        label={knob2?.label ?? `K2`}
        controlId={`track${trackNumber}.knob2`}
        onValueChange={(value) => handleKnobChange('knob2', value)}
      />
      <Knob
        value={knob3?.value ?? 0}
        label={knob3?.label ?? `K3`}
        controlId={`track${trackNumber}.knob3`}
        onValueChange={(value) => handleKnobChange('knob3', value)}
      />

      {/* Three buttons: mute, solo, rec_arm */}
      <div className="flex flex-col gap-2">
        <Button
          type="mute"
          active={mute?.active ?? false}
          label={mute?.label ?? 'M'}
          controlId={`track${trackNumber}.mute`}
          onClick={() => handleButtonToggle('mute')}
        />
        <Button
          type="solo"
          active={solo?.active ?? false}
          label={solo?.label ?? 'S'}
          controlId={`track${trackNumber}.solo`}
          onClick={() => handleButtonToggle('solo')}
        />
        <Button
          type="rec"
          active={recArm?.active ?? false}
          label={recArm?.label ?? 'R'}
          controlId={`track${trackNumber}.rec_arm`}
          onClick={() => handleButtonToggle('rec_arm')}
        />
      </div>

      {/* Fader */}
      <Slider
        value={fader?.value ?? 0}
        label={fader?.label ?? 'Fader'}
        controlId={`track${trackNumber}.fader`}
        onValueChange={handleFaderChange}
      />
    </div>
  );
}
