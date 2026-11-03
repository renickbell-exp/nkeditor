/**
 * MidiMix Master Section Component
 *
 * Renders the master section of the Akai MidiMix:
 * - Master fader
 * - Bank Left / Bank Right buttons
 * - Solo master button
 *
 * Reads and writes state through the generic per-device section controls.
 */

import React, { useCallback } from 'react';
import { Slider } from '../Slider';
import { useControlsStore } from '../../../stores/controls';
import type { GenericControlValue } from '../../../stores/controls';

interface MidiMixMasterSectionProps {
  instanceId: string;
  sectionState: Record<string, GenericControlValue>;
}

export function MidiMixMasterSection({
  instanceId,
  sectionState,
}: MidiMixMasterSectionProps): React.ReactElement {
  const updateSectionControl = useControlsStore((state) => state.updateSectionControl);
  const updateSectionButton = useControlsStore((state) => state.updateSectionButton);

  const masterFader = sectionState['master_fader'];
  const bankLeft = sectionState['bank_left'];
  const bankRight = sectionState['bank_right'];
  const soloMaster = sectionState['solo_master'];

  const handleFaderChange = useCallback(
    (value: number) => {
      updateSectionControl(instanceId, 'master', 'master_fader', value);
    },
    [instanceId, updateSectionControl],
  );

  const handleButtonToggle = useCallback(
    (controlName: string) => {
      const currentActive = sectionState[controlName]?.active ?? false;
      updateSectionButton(instanceId, 'master', controlName, !currentActive);
    },
    [instanceId, sectionState, updateSectionButton],
  );

  return (
    <div className="flex flex-col items-center gap-3 p-3 bg-nk-dark rounded-lg border border-nk-border flex-shrink-0">
      {/* Section label */}
      <div className="text-xs font-medium text-gray-500">Master</div>

      {/* Master fader */}
      <Slider
        value={masterFader?.value ?? 0}
        label={masterFader?.label ?? 'Master'}
        controlId="master.master_fader"
        onValueChange={handleFaderChange}
      />

      {/* Bank and solo buttons */}
      <div className="flex flex-col gap-2 mt-2">
        <MasterButton
          label={bankLeft?.label ?? 'Bank L'}
          active={bankLeft?.active ?? false}
          onClick={() => handleButtonToggle('bank_left')}
          title="Bank Left"
        />
        <MasterButton
          label={bankRight?.label ?? 'Bank R'}
          active={bankRight?.active ?? false}
          onClick={() => handleButtonToggle('bank_right')}
          title="Bank Right"
        />
        <MasterButton
          label={soloMaster?.label ?? 'Solo'}
          active={soloMaster?.active ?? false}
          onClick={() => handleButtonToggle('solo_master')}
          title="Solo Master"
          accentWhenActive
        />
      </div>
    </div>
  );
}

// =============================================================================
// Master Button (no LED, simpler than the shared Button)
// =============================================================================

interface MasterButtonProps {
  label: string;
  active: boolean;
  onClick: () => void;
  title: string;
  accentWhenActive?: boolean;
}

function MasterButton({
  label,
  active,
  onClick,
  title,
  accentWhenActive = false,
}: MasterButtonProps): React.ReactElement {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      className={`w-14 h-7 rounded text-xs font-medium transition-all ${
        active
          ? accentWhenActive
            ? 'bg-yellow-600 text-white shadow-inner'
            : 'bg-gray-600 text-white shadow-inner'
          : 'bg-gray-700 hover:bg-gray-600 text-gray-400 shadow'
      }`}
    >
      {label}
    </button>
  );
}
