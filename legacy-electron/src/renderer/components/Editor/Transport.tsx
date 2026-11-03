/**
 * Transport Component
 *
 * Transport section controls for the nanoKONTROL2.
 * Contains playback controls and marker/track navigation buttons.
 *
 * When GUI controls are manipulated, this component:
 * 1. Updates the local store state for UI reflection
 * 2. Sends MIDI CC messages to the virtual output port for DAW control
 */

import React, { useState, useCallback } from 'react';
import { useControlsStore } from '../../stores/controls';
import { usePresetStore } from '../../stores/preset';
import { HARDWARE_CC } from '@shared/constants';
import { ControlPopover } from './ControlPopover';
import type { ElectronAPI } from '../../../main/preload';
import type { MappingEntry, MappingConfig, TransportControlType } from '@shared/types';

// Retrieve electronAPI from window for MIDI output
const getElectronAPI = (): ElectronAPI | null => {
  return (window as { electronAPI?: ElectronAPI }).electronAPI ?? null;
};

// Default MIDI channel for GUI-initiated CC messages (0-indexed)
const DEFAULT_MIDI_CHANNEL = 0;

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

// Map transport control names to their CC numbers
const TRANSPORT_CC_MAP: Record<string, number> = {
  play: HARDWARE_CC.TRANSPORT.PLAY,
  stop: HARDWARE_CC.TRANSPORT.STOP,
  rewind: HARDWARE_CC.TRANSPORT.REWIND,
  forward: HARDWARE_CC.TRANSPORT.FORWARD,
  record: HARDWARE_CC.TRANSPORT.RECORD,
  cycle: HARDWARE_CC.TRANSPORT.CYCLE,
  track_left: HARDWARE_CC.TRANSPORT.TRACK_LEFT,
  track_right: HARDWARE_CC.TRANSPORT.TRACK_RIGHT,
  marker_set: HARDWARE_CC.TRANSPORT.MARKER_SET,
  marker_left: HARDWARE_CC.TRANSPORT.MARKER_LEFT,
  marker_right: HARDWARE_CC.TRANSPORT.MARKER_RIGHT,
};

export function Transport(): React.ReactElement {
  const transport = useControlsStore((state) => state.transport);
  const updateButton = useControlsStore((state) => state.updateButton);

  const handleTransportClick = (controlName: string) => {
    const controlKey = controlName as keyof typeof transport;
    const currentActive = transport[controlKey]?.active ?? false;
    const newActive = !currentActive;

    // Update local store state for UI
    updateButton(`transport.${controlName}`, newActive);

    // Send MIDI CC to virtual output port
    const api = getElectronAPI();
    if (api) {
      const ccNumber = TRANSPORT_CC_MAP[controlName];
      if (ccNumber !== undefined) {
        // Get the current mapping to apply custom on/off values
        const currentPreset = usePresetStore.getState().currentPreset;
        // nK2 editor components only handle MappingConfig presets
        const nk2Mapping = currentPreset?.mapping && 'transport' in currentPreset.mapping
          ? (currentPreset.mapping as MappingConfig)
          : undefined;
        const mapping = nk2Mapping?.transport[controlName as TransportControlType];

        // Use custom on/off values if configured, otherwise default to 127/0
        const midiValue = getButtonMidiValue(newActive, mapping);
        api.sendCC({ channel: DEFAULT_MIDI_CHANNEL, cc: ccNumber, value: midiValue });
      }
    }

    // Mark preset as having unsaved changes
    usePresetStore.getState().markUnsavedChanges();
  };

  return (
    <div className="flex items-center justify-center gap-6 p-4 bg-nk-dark rounded-lg border border-nk-border">
      {/* Track navigation */}
      <div className="flex flex-col items-center gap-2">
        <span className="text-xs text-gray-500">Track</span>
        <div className="flex gap-2">
          <TransportButton
            label="<"
            active={transport.track_left.active}
            controlId="transport.track_left"
            onClick={() => handleTransportClick('track_left')}
          />
          <TransportButton
            label=">"
            active={transport.track_right.active}
            controlId="transport.track_right"
            onClick={() => handleTransportClick('track_right')}
          />
        </div>
      </div>

      {/* Marker controls */}
      <div className="flex flex-col items-center gap-2">
        <span className="text-xs text-gray-500">Marker</span>
        <div className="flex gap-2">
          <TransportButton
            label="<"
            active={transport.marker_left.active}
            controlId="transport.marker_left"
            onClick={() => handleTransportClick('marker_left')}
          />
          <TransportButton
            label="Set"
            active={transport.marker_set.active}
            controlId="transport.marker_set"
            onClick={() => handleTransportClick('marker_set')}
          />
          <TransportButton
            label=">"
            active={transport.marker_right.active}
            controlId="transport.marker_right"
            onClick={() => handleTransportClick('marker_right')}
          />
        </div>
      </div>

      {/* Playback controls */}
      <div className="flex flex-col items-center gap-2">
        <span className="text-xs text-gray-500">Transport</span>
        <div className="flex gap-2">
          <TransportButton
            label="<<"
            active={transport.rewind.active}
            controlId="transport.rewind"
            onClick={() => handleTransportClick('rewind')}
          />
          <TransportButton
            label=">>"
            active={transport.forward.active}
            controlId="transport.forward"
            onClick={() => handleTransportClick('forward')}
          />
          <TransportButton
            label="[]"
            active={transport.stop.active}
            controlId="transport.stop"
            hasLed
            onClick={() => handleTransportClick('stop')}
          />
          <TransportButton
            label=">"
            active={transport.play.active}
            controlId="transport.play"
            hasLed
            onClick={() => handleTransportClick('play')}
          />
          <TransportButton
            label="O"
            active={transport.record.active}
            controlId="transport.record"
            hasLed
            ledColor="red"
            onClick={() => handleTransportClick('record')}
          />
        </div>
      </div>

      {/* Cycle button */}
      <div className="flex flex-col items-center gap-2">
        <span className="text-xs text-gray-500">Cycle</span>
        <TransportButton
          label="@"
          active={transport.cycle.active}
          controlId="transport.cycle"
          hasLed
          onClick={() => handleTransportClick('cycle')}
        />
      </div>
    </div>
  );
}

// =============================================================================
// Transport Button
// =============================================================================

interface TransportButtonProps {
  label: string;
  active: boolean;
  controlId: string;
  hasLed?: boolean;
  ledColor?: 'yellow' | 'red';
  onClick?: () => void;
}

function TransportButton({
  label,
  active,
  controlId,
  hasLed = false,
  ledColor = 'yellow',
  onClick,
}: TransportButtonProps): React.ReactElement {
  const [popoverOpen, setPopoverOpen] = useState(false);
  const [popoverPosition, setPopoverPosition] = useState({ x: 0, y: 0 });

  const openPopover = useCallback((x: number, y: number) => {
    setPopoverPosition({ x, y });
    setPopoverOpen(true);
  }, []);

  const closePopover = useCallback(() => {
    setPopoverOpen(false);
  }, []);

  const handleClick = (e: React.MouseEvent) => {
    // Ctrl+click or meta+click opens the popover instead of toggling
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      openPopover(e.clientX, e.clientY);
      return;
    }

    // Regular click toggles the button
    if (onClick) {
      onClick();
    }
  };

  // Right-click opens the popover for CC assignment editing
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    openPopover(e.clientX, e.clientY);
  };

  const getLedClass = () => {
    if (!active) return 'led-off';
    return ledColor === 'red' ? 'led-rec' : 'led-solo';
  };

  return (
    <>
      <div
        className="flex flex-col items-center gap-1 cursor-pointer"
        title={`${controlId}: ${active ? 'ON' : 'OFF'} (right-click to edit mapping)`}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        {/* Render LED or invisible placeholder to maintain consistent button alignment */}
        {hasLed ? (
          <div className={`led ${getLedClass()}`} />
        ) : (
          <div className="led invisible" />
        )}
        <button
          className={`w-10 h-8 rounded text-xs font-medium transition-all ${
            active
              ? 'bg-gray-600 shadow-inner text-white'
              : 'bg-gray-700 hover:bg-gray-600 shadow text-gray-400'
          }`}
          type="button"
        >
          {label}
        </button>
      </div>

      {popoverOpen && (
        <ControlPopover
          isOpen={popoverOpen}
          onClose={closePopover}
          controlId={controlId}
          controlKind="button"
          anchorPosition={popoverPosition}
        />
      )}
    </>
  );
}
