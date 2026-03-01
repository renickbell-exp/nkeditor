/**
 * MidiMix Editor Component
 *
 * Full editor view for the Akai MidiMix controller.
 * Layout mirrors the physical hardware:
 * - 8 columns with 3 knobs, 3 buttons (mute/solo/rec_arm), and 1 fader each
 * - Master section on the right: master fader, bank L/R buttons, solo button
 *
 * Reads per-device control state from the controls store using the
 * provided instanceId.
 */

import React from 'react';
import { MidiMixColumn } from './MidiMixColumn';
import { MidiMixMasterSection } from './MidiMixMasterSection';
import { useControlsStore } from '../../../stores/controls';

interface MidiMixEditorProps {
  instanceId: string;
}

export function MidiMixEditor({ instanceId }: MidiMixEditorProps): React.ReactElement {
  const deviceState = useControlsStore((state) => state.getDeviceState(instanceId));

  if (!deviceState) {
    return (
      <div className="h-full flex items-center justify-center p-8">
        <div className="text-center text-gray-500">
          <p className="text-lg font-medium mb-2">Device not initialized</p>
          <p className="text-sm">
            Control state for this MidiMix has not been set up yet.
          </p>
        </div>
      </div>
    );
  }

  const masterSection = deviceState.sections['master'] ?? {};

  return (
    <div className="h-full p-4">
      <div className="card p-4 h-full flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-gray-200">Akai MIDI Mix Editor</h2>
          </div>
          <div className="text-sm text-gray-500">
            Click any control to edit its mapping
          </div>
        </div>

        {/* Controller visualization */}
        <div className="flex-1 bg-nk-darker rounded-lg p-4 overflow-auto">
          <div className="flex gap-4 justify-center flex-shrink-0">
            {/* 8 track columns */}
            {deviceState.tracks.map((trackState, trackIndex) => (
              <MidiMixColumn
                key={trackIndex}
                instanceId={instanceId}
                trackIndex={trackIndex}
                trackState={trackState}
              />
            ))}

            {/* Separator */}
            <div className="w-px bg-nk-border self-stretch" />

            {/* Master section */}
            <MidiMixMasterSection
              instanceId={instanceId}
              sectionState={masterSection}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
