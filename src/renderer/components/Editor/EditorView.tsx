/**
 * Editor View Component
 *
 * Main editor view showing the visual representation of the nanoKONTROL2.
 * Displays 8 tracks with knobs, sliders, and buttons, plus transport controls.
 *
 * Users can click on any control to edit its CC mapping. Changes are stored
 * in memory until explicitly saved via the Save button.
 */

import React, { useCallback, useEffect, useState } from 'react';
import { Track } from './Track';
import { Transport } from './Transport';
import { useControlsStore } from '../../stores/controls';
import { usePresetStore } from '../../stores/preset';
import { useUndoStore } from '../../stores/undo';
import { NewPresetDialog } from '../Librarian/NewPresetDialog';
import type { ControlValues, Preset } from '@shared/ipc-protocol';

interface EditorViewProps {
  /** When provided, this editor is operating in multi-device mode for a specific instance */
  instanceId?: string;
}

export function EditorView({ instanceId: _instanceId }: EditorViewProps = {}): React.ReactElement {
  // instanceId is accepted for forward compatibility with multi-device mode.
  // When provided, the editor could read per-device state from deviceControls
  // instead of the legacy tracks/transport state. The legacy single-device
  // interface remains the primary path until full migration is complete.
  void _instanceId;
  const tracks = useControlsStore((state) => state.tracks);
  const transport = useControlsStore((state) => state.transport);
  const currentPreset = usePresetStore((state) => state.currentPreset);
  const hasUnsavedChanges = usePresetStore((state) => state.hasUnsavedChanges);
  const saveCurrentPreset = usePresetStore((state) => state.saveCurrentPreset);
  const setCurrentPreset = usePresetStore((state) => state.setCurrentPreset);
  const markUnsavedChanges = usePresetStore((state) => state.markUnsavedChanges);
  const applyLabelsFromMapping = useControlsStore((state) => state.applyLabelsFromMapping);
  const randomizeKnobsAndSliders = useControlsStore((state) => state.randomizeKnobsAndSliders);
  const randomizeSelectedKnobsAndSliders = useControlsStore((state) => state.randomizeSelectedKnobsAndSliders);
  const restoreFromControlValues = useControlsStore((state) => state.restoreFromControlValues);
  const captureControlValues = useControlsStore((state) => state.captureControlValues);
  const selectAllControls = useControlsStore((state) => state.selectAllControls);
  const deselectAllControls = useControlsStore((state) => state.deselectAllControls);
  const getSelectedCount = useControlsStore((state) => state.getSelectedCount);
  const loading = usePresetStore((state) => state.loading);

  // Undo store
  const pushSnapshot = useUndoStore((state) => state.pushSnapshot);
  const popSnapshot = useUndoStore((state) => state.popSnapshot);
  const canUndo = useUndoStore((state) => state.canUndo);
  const getUndoCount = useUndoStore((state) => state.getUndoCount);

  const [isNewPresetDialogOpen, setIsNewPresetDialogOpen] = useState(false);
  const [copyLabelsFeedback, setCopyLabelsFeedback] = useState<'idle' | 'success' | 'error'>('idle');
  const [pasteLabelsFeedback, setPasteLabelsFeedback] = useState<'idle' | 'success' | 'error'>('idle');
  const [randomizeFeedback, setRandomizeFeedback] = useState<'idle' | 'success'>('idle');
  const [undoFeedback, setUndoFeedback] = useState<'idle' | 'success'>('idle');

  // Compute selected count
  const selectedCount = getSelectedCount();

  const handleSave = useCallback(async () => {
    const success = await saveCurrentPreset();
    if (!success) {
      console.error('[EditorView] Failed to save preset');
    }
  }, [saveCurrentPreset]);

  // Extract labels from current UI state
  const extractLabels = useCallback(() => {
    const labels: {
      tracks: Array<{
        knob?: string;
        slider?: string;
        solo?: string;
        mute?: string;
        rec?: string;
      }>;
      transport: {
        play?: string;
        stop?: string;
        rewind?: string;
        forward?: string;
        record?: string;
        cycle?: string;
        track_left?: string;
        track_right?: string;
        marker_set?: string;
        marker_left?: string;
        marker_right?: string;
      };
    } = {
      tracks: [],
      transport: {},
    };

    // Extract from tracks
    for (const track of tracks) {
      labels.tracks.push({
        knob: track.knob.label,
        slider: track.slider.label,
        solo: track.solo.label,
        mute: track.mute.label,
        rec: track.rec.label,
      });
    }

    // Extract from transport
    labels.transport = {
      play: transport.play.label,
      stop: transport.stop.label,
      rewind: transport.rewind.label,
      forward: transport.forward.label,
      record: transport.record.label,
      cycle: transport.cycle.label,
      track_left: transport.track_left.label,
      track_right: transport.track_right.label,
      marker_set: transport.marker_set.label,
      marker_left: transport.marker_left.label,
      marker_right: transport.marker_right.label,
    };

    return labels;
  }, [tracks, transport]);

  // Copy labels to clipboard
  const handleCopyLabels = useCallback(async () => {
    try {
      const labels = extractLabels();
      await navigator.clipboard.writeText(JSON.stringify(labels, null, 2));
      setCopyLabelsFeedback('success');
      setTimeout(() => setCopyLabelsFeedback('idle'), 1500);
    } catch {
      setCopyLabelsFeedback('error');
      setTimeout(() => setCopyLabelsFeedback('idle'), 2000);
    }
  }, [extractLabels]);

  // Paste labels from clipboard
  const handlePasteLabels = useCallback(async () => {
    if (!currentPreset) return;

    try {
      const clipboardText = await navigator.clipboard.readText();
      const labels = JSON.parse(clipboardText);

      // Validate structure
      if (!labels.tracks || !Array.isArray(labels.tracks) || labels.tracks.length !== 8) {
        throw new Error('Invalid labels format: expected 8 tracks');
      }

      // Build updated control values with new labels
      const currentCv = currentPreset.controlValues ?? {
        tracks: Array(8).fill(null).map(() => ({
          knob: 0,
          slider: 0,
          solo: false,
          mute: false,
          rec: false,
        })),
        transport: {
          play: false,
          stop: false,
          rewind: false,
          forward: false,
          record: false,
          cycle: false,
          track_left: false,
          track_right: false,
          marker_set: false,
          marker_left: false,
          marker_right: false,
        },
      };

      const updatedCv: ControlValues = {
        tracks: currentCv.tracks.map((track, i) => ({
          ...track,
          knobLabel: labels.tracks[i]?.knob,
          sliderLabel: labels.tracks[i]?.slider,
          soloLabel: labels.tracks[i]?.solo,
          muteLabel: labels.tracks[i]?.mute,
          recLabel: labels.tracks[i]?.rec,
        })),
        transport: {
          ...currentCv.transport,
          playLabel: labels.transport?.play,
          stopLabel: labels.transport?.stop,
          rewindLabel: labels.transport?.rewind,
          forwardLabel: labels.transport?.forward,
          recordLabel: labels.transport?.record,
          cycleLabel: labels.transport?.cycle,
          track_leftLabel: labels.transport?.track_left,
          track_rightLabel: labels.transport?.track_right,
          marker_setLabel: labels.transport?.marker_set,
          marker_leftLabel: labels.transport?.marker_left,
          marker_rightLabel: labels.transport?.marker_right,
        },
      };

      // Update mapping config labels as well
      const updatedMapping = JSON.parse(JSON.stringify(currentPreset.mapping));
      for (let i = 0; i < 8; i++) {
        if (updatedMapping.tracks[i]) {
          updatedMapping.tracks[i].knob.label = labels.tracks[i]?.knob;
          updatedMapping.tracks[i].slider.label = labels.tracks[i]?.slider;
          updatedMapping.tracks[i].solo.label = labels.tracks[i]?.solo;
          updatedMapping.tracks[i].mute.label = labels.tracks[i]?.mute;
          updatedMapping.tracks[i].rec.label = labels.tracks[i]?.rec;
        }
      }
      updatedMapping.transport.play.label = labels.transport?.play;
      updatedMapping.transport.stop.label = labels.transport?.stop;
      updatedMapping.transport.rewind.label = labels.transport?.rewind;
      updatedMapping.transport.forward.label = labels.transport?.forward;
      updatedMapping.transport.record.label = labels.transport?.record;
      updatedMapping.transport.cycle.label = labels.transport?.cycle;
      updatedMapping.transport.track_left.label = labels.transport?.track_left;
      updatedMapping.transport.track_right.label = labels.transport?.track_right;
      updatedMapping.transport.marker_set.label = labels.transport?.marker_set;
      updatedMapping.transport.marker_left.label = labels.transport?.marker_left;
      updatedMapping.transport.marker_right.label = labels.transport?.marker_right;

      // Update the current preset in memory
      const updatedPreset: Preset = {
        ...currentPreset,
        mapping: updatedMapping,
        controlValues: updatedCv,
      };

      setCurrentPreset(updatedPreset);
      applyLabelsFromMapping(updatedMapping);
      markUnsavedChanges();

      setPasteLabelsFeedback('success');
      setTimeout(() => setPasteLabelsFeedback('idle'), 1500);
    } catch {
      setPasteLabelsFeedback('error');
      setTimeout(() => setPasteLabelsFeedback('idle'), 2000);
    }
  }, [currentPreset, setCurrentPreset, applyLabelsFromMapping, markUnsavedChanges]);

  // Randomize all knob and slider values
  const handleRandomizeAll = useCallback(async () => {
    if (!currentPreset) return;

    try {
      // Save current state for undo before randomizing
      const currentValues = captureControlValues();
      pushSnapshot('Randomize All', currentValues);

      await randomizeKnobsAndSliders(currentPreset.mapping);
      markUnsavedChanges();
      setRandomizeFeedback('success');
      setTimeout(() => setRandomizeFeedback('idle'), 1000);
    } catch (error) {
      console.error('[EditorView] Randomize failed:', error);
    }
  }, [currentPreset, randomizeKnobsAndSliders, markUnsavedChanges, captureControlValues, pushSnapshot]);

  // Randomize only selected knob and slider values
  const handleRandomizeSelected = useCallback(async () => {
    if (!currentPreset || selectedCount === 0) return;

    try {
      // Save current state for undo before randomizing
      const currentValues = captureControlValues();
      pushSnapshot('Randomize Selected', currentValues);

      await randomizeSelectedKnobsAndSliders(currentPreset.mapping);
      markUnsavedChanges();
      setRandomizeFeedback('success');
      setTimeout(() => setRandomizeFeedback('idle'), 1000);
    } catch (error) {
      console.error('[EditorView] Randomize selected failed:', error);
    }
  }, [currentPreset, selectedCount, randomizeSelectedKnobsAndSliders, markUnsavedChanges, captureControlValues, pushSnapshot]);

  // Undo last change
  const handleUndo = useCallback(async () => {
    if (!currentPreset || !canUndo()) return;

    try {
      const snapshot = popSnapshot();
      if (snapshot) {
        await restoreFromControlValues(snapshot.controlValues, currentPreset.mapping);
        markUnsavedChanges();
        setUndoFeedback('success');
        setTimeout(() => setUndoFeedback('idle'), 1000);
      }
    } catch (error) {
      console.error('[EditorView] Undo failed:', error);
    }
  }, [currentPreset, canUndo, popSnapshot, restoreFromControlValues, markUnsavedChanges]);

  // Keyboard shortcuts: Ctrl+S to save, Ctrl+Z to undo
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 's') {
        event.preventDefault();
        if (currentPreset && hasUnsavedChanges && !loading) {
          handleSave();
        }
      }
      if ((event.ctrlKey || event.metaKey) && event.key === 'z') {
        event.preventDefault();
        if (currentPreset && canUndo() && !loading) {
          handleUndo();
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [currentPreset, hasUnsavedChanges, loading, handleSave, canUndo, handleUndo]);

  return (
    <div className="h-full p-4">
      <div className="card p-4 h-full flex flex-col">
        {/* Controller header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-gray-200">nanoKONTROL2 Editor</h2>
            {currentPreset && (
              <span className="text-sm text-gray-400">
                {currentPreset.metadata.name}
                {hasUnsavedChanges && <span className="text-yellow-400 ml-1">*</span>}
              </span>
            )}
          </div>
          <div className="flex items-center gap-4">
            <div className="flex flex-col items-end gap-0.5">
              <div className="text-sm text-gray-500">
                Click any control to edit its mapping
              </div>
              <div className="text-xs text-gray-600">
                Changes auto-detected • Ctrl+S to save • Ctrl+Z to undo
              </div>
            </div>
            {/* Copy/Paste Labels buttons */}
            <div className="flex gap-1">
              <button
                type="button"
                onClick={handleCopyLabels}
                disabled={loading}
                className={`btn btn-ghost text-sm px-2 py-1 ${
                  copyLabelsFeedback === 'success' ? 'text-green-400 bg-green-900/20' :
                  copyLabelsFeedback === 'error' ? 'text-red-400 bg-red-900/20' : ''
                }`}
                title="Copy all control labels to clipboard"
              >
                {copyLabelsFeedback === 'success' ? 'Copied!' :
                 copyLabelsFeedback === 'error' ? 'Error' :
                 'Copy Labels'}
              </button>
              <button
                type="button"
                onClick={handlePasteLabels}
                disabled={loading || !currentPreset}
                className={`btn btn-ghost text-sm px-2 py-1 ${
                  pasteLabelsFeedback === 'success' ? 'text-green-400 bg-green-900/20' :
                  pasteLabelsFeedback === 'error' ? 'text-red-400 bg-red-900/20' : ''
                } ${!currentPreset ? 'opacity-50 cursor-not-allowed' : ''}`}
                title={currentPreset ? 'Paste labels from clipboard' : 'Load a preset first'}
              >
                {pasteLabelsFeedback === 'success' ? 'Pasted!' :
                 pasteLabelsFeedback === 'error' ? 'Error' :
                 'Paste Labels'}
              </button>
            </div>
            {/* Selection controls */}
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={selectAllControls}
                disabled={loading || !currentPreset}
                className={`btn btn-ghost text-xs px-2 py-1 ${!currentPreset ? 'opacity-50 cursor-not-allowed' : ''}`}
                title="Select all knobs and sliders"
              >
                Select All
              </button>
              <button
                type="button"
                onClick={deselectAllControls}
                disabled={loading || !currentPreset || selectedCount === 0}
                className={`btn btn-ghost text-xs px-2 py-1 ${(!currentPreset || selectedCount === 0) ? 'opacity-50 cursor-not-allowed' : ''}`}
                title="Deselect all controls"
              >
                Deselect
              </button>
              {selectedCount > 0 && (
                <span className="text-xs text-nk-accent ml-1">
                  {selectedCount} selected
                </span>
              )}
            </div>
            {/* Randomize buttons */}
            <div className="flex gap-1">
              <button
                type="button"
                onClick={handleRandomizeAll}
                disabled={loading || !currentPreset}
                className={`btn btn-ghost text-sm px-2 py-1 ${
                  randomizeFeedback === 'success' ? 'text-green-400 bg-green-900/20' : ''
                } ${!currentPreset ? 'opacity-50 cursor-not-allowed' : ''}`}
                title={currentPreset ? 'Randomize all knob and slider values' : 'Load a preset first'}
              >
                {randomizeFeedback === 'success' ? 'Randomized!' : 'Randomize All'}
              </button>
              <button
                type="button"
                onClick={handleRandomizeSelected}
                disabled={loading || !currentPreset || selectedCount === 0}
                className={`btn btn-ghost text-sm px-2 py-1 ${
                  randomizeFeedback === 'success' ? 'text-green-400 bg-green-900/20' : ''
                } ${(!currentPreset || selectedCount === 0) ? 'opacity-50 cursor-not-allowed' : ''}`}
                title={selectedCount > 0 ? `Randomize ${selectedCount} selected controls` : 'Select controls first'}
              >
                Randomize Selected
              </button>
            </div>
            {/* Undo button */}
            <button
              type="button"
              onClick={handleUndo}
              disabled={loading || !currentPreset || !canUndo()}
              className={`btn btn-ghost text-sm px-2 py-1 ${
                undoFeedback === 'success' ? 'text-green-400 bg-green-900/20' : ''
              } ${(!currentPreset || !canUndo()) ? 'opacity-50 cursor-not-allowed' : ''}`}
              title={canUndo() ? `Undo (${getUndoCount()} steps available) - Ctrl+Z` : 'Nothing to undo'}
            >
              {undoFeedback === 'success' ? 'Undone!' : `Undo${canUndo() ? ` (${getUndoCount()})` : ''}`}
            </button>
            <button
              type="button"
              onClick={() => setIsNewPresetDialogOpen(true)}
              disabled={loading}
              className="btn btn-ghost text-sm"
              title="Save current mapping configuration as a new preset"
            >
              Save As New...
            </button>
            {currentPreset && (
              <button
                type="button"
                onClick={handleSave}
                disabled={!hasUnsavedChanges || loading}
                className={`btn text-sm ${
                  hasUnsavedChanges && !loading
                    ? 'btn-primary'
                    : 'btn-ghost opacity-50 cursor-not-allowed'
                }`}
                title={
                  hasUnsavedChanges
                    ? 'Save mapping changes (Ctrl+S)'
                    : 'No mapping changes to save'
                }
              >
                {loading ? 'Saving...' : 'Save Preset'}
              </button>
            )}
          </div>
        </div>

        {/* No preset loaded warning */}
        {!currentPreset && (
          <div className="mb-4 p-3 bg-yellow-900/20 border border-yellow-800 rounded text-sm text-yellow-400">
            No preset loaded. Load a preset from the Librarian to edit mappings.
          </div>
        )}

        {/* Controller visualization */}
        <div className="flex-1 bg-nk-darker rounded-lg p-4 overflow-auto">
          {/* Track strips - flex-shrink-0 prevents compression in flex container */}
          <div className="flex gap-4 justify-center mb-6 flex-shrink-0">
            {tracks.map((track, index) => (
              <Track
                key={index}
                trackNumber={index + 1}
                track={track}
              />
            ))}
          </div>

          {/* Transport section */}
          <Transport />
        </div>
      </div>

      {/* New Preset Dialog - saves current mapping as a new preset without sending MIDI */}
      <NewPresetDialog
        isOpen={isNewPresetDialogOpen}
        onClose={() => setIsNewPresetDialogOpen(false)}
      />
    </div>
  );
}
