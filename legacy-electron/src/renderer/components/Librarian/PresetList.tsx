/**
 * Preset List Component
 *
 * Displays a list of presets with selection, management options, and expandable details.
 * Each preset shows a chevron toggle to expand/collapse full metadata details.
 */

import React, { useState, useCallback } from 'react';
import { usePresetStore } from '../../stores/preset';
import type { PresetMetadata, ControlValues, Preset } from '@shared/ipc-protocol';
import type { MappingConfig } from '@shared/types';
import type { ElectronAPI } from '../../../main/preload';

// Get the electron API from window
const getElectronAPI = (): ElectronAPI | null => {
  return (window as { electronAPI?: ElectronAPI }).electronAPI ?? null;
};

interface PresetListProps {
  presets: PresetMetadata[];
  compact?: boolean;
  onEditPreset?: (preset: PresetMetadata) => void;
  onEditControlValues?: (preset: Preset) => void;
  onDuplicatePreset?: (preset: PresetMetadata) => void;
}

export function PresetList({ presets, compact = false, onEditPreset, onEditControlValues, onDuplicatePreset }: PresetListProps): React.ReactElement {
  const selectedId = usePresetStore((state) => state.selectedId);
  const selectPreset = usePresetStore((state) => state.selectPreset);
  const loadPreset = usePresetStore((state) => state.loadPreset);
  const deletePreset = usePresetStore((state) => state.deletePreset);
  const toggleFavorite = usePresetStore((state) => state.toggleFavorite);

  // Track which presets have expanded details
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  if (presets.length === 0) {
    return (
      <div className="flex items-center justify-center h-32 text-gray-500">
        No presets found
      </div>
    );
  }

  const handleDelete = (id: string, name: string) => {
    if (window.confirm(`Delete preset "${name}"? This cannot be undone.`)) {
      deletePreset(id);
    }
  };

  const toggleExpanded = (id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  return (
    <div className="space-y-1">
      {presets.map((preset) => (
        <PresetItem
          key={preset.id}
          preset={preset}
          selected={preset.id === selectedId}
          expanded={expandedIds.has(preset.id)}
          compact={compact}
          onSelect={() => selectPreset(preset.id)}
          onLoad={() => loadPreset(preset.id)}
          onDelete={() => handleDelete(preset.id, preset.name)}
          onToggleFavorite={() => toggleFavorite(preset.id)}
          onToggleExpanded={() => toggleExpanded(preset.id)}
          onEdit={onEditPreset ? () => onEditPreset(preset) : undefined}
          onEditControlValues={onEditControlValues}
          onDuplicate={onDuplicatePreset ? () => onDuplicatePreset(preset) : undefined}
        />
      ))}
    </div>
  );
}

// =============================================================================
// Chevron Icon Component
// =============================================================================

interface ChevronIconProps {
  expanded: boolean;
  className?: string;
}

function ChevronIcon({ expanded, className = '' }: ChevronIconProps): React.ReactElement {
  return (
    <svg
      className={`w-4 h-4 transition-transform duration-200 ${expanded ? 'rotate-90' : ''} ${className}`}
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
        d="M9 5l7 7-7 7"
      />
    </svg>
  );
}

// =============================================================================
// Preset Item
// =============================================================================

interface PresetItemProps {
  preset: PresetMetadata;
  selected: boolean;
  expanded: boolean;
  compact: boolean;
  onSelect: () => void;
  onLoad: () => Promise<boolean>;
  onDelete: () => void;
  onToggleFavorite: () => void;
  onToggleExpanded: () => void;
  onEdit?: () => void;
  onEditControlValues?: (preset: Preset) => void;
  onDuplicate?: () => void;
}

// Loading state for the Load button
type LoadStatus = 'idle' | 'loading' | 'success' | 'error';

// State for JSON preview within an expanded preset
interface JsonPreviewState {
  loading: boolean;
  controlValues: ControlValues | null;
  fullPreset: Preset | null;
  error: string | null;
}

function PresetItem({
  preset,
  selected,
  expanded,
  compact,
  onSelect,
  onLoad,
  onDelete,
  onToggleFavorite,
  onToggleExpanded,
  onEdit,
  onEditControlValues,
  onDuplicate,
}: PresetItemProps): React.ReactElement {
  // State for JSON preview
  const [jsonPreview, setJsonPreview] = useState<JsonPreviewState>({
    loading: false,
    controlValues: null,
    fullPreset: null,
    error: null,
  });
  const [showJson, setShowJson] = useState(false);
  // State for Load button feedback
  const [loadStatus, setLoadStatus] = useState<LoadStatus>('idle');
  // State for copy/paste feedback
  const [copyLabelsFeedback, setCopyLabelsFeedback] = useState<'idle' | 'success' | 'error'>('idle');
  const [pasteLabelsFeedback, setPasteLabelsFeedback] = useState<'idle' | 'success' | 'error'>('idle');

  // Handle load with visual feedback
  const handleLoad = async () => {
    if (loadStatus === 'loading') return; // Prevent double-clicks

    setLoadStatus('loading');
    try {
      const success = await onLoad();
      if (success) {
        setLoadStatus('success');
        // Reset to idle after showing success briefly
        setTimeout(() => setLoadStatus('idle'), 1500);
      } else {
        setLoadStatus('error');
        // Reset to idle after showing error briefly
        setTimeout(() => setLoadStatus('idle'), 2000);
      }
    } catch {
      setLoadStatus('error');
      setTimeout(() => setLoadStatus('idle'), 2000);
    }
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString();
  };

  const formatDateTime = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleString();
  };

  // Load full preset to get controlValues for JSON preview
  const handleShowJson = async () => {
    if (showJson) {
      // Toggle off
      setShowJson(false);
      return;
    }

    // Always refetch fresh data from disk to avoid showing stale cached data
    // Load the full preset
    const api = getElectronAPI();
    if (!api) {
      setJsonPreview({ loading: false, controlValues: null, fullPreset: null, error: 'API not available' });
      return;
    }

    setJsonPreview({ loading: true, controlValues: null, fullPreset: null, error: null });

    try {
      const response = await api.loadPreset({ id: preset.id });
      if (response.success && response.preset) {
        setJsonPreview({
          loading: false,
          controlValues: response.preset.controlValues ?? null,
          fullPreset: response.preset,
          error: null,
        });
        setShowJson(true);
      } else {
        setJsonPreview({
          loading: false,
          controlValues: null,
          fullPreset: null,
          error: response.error ?? 'Failed to load preset',
        });
      }
    } catch (err) {
      setJsonPreview({
        loading: false,
        controlValues: null,
        fullPreset: null,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  };

  // Extract labels from control values or mapping
  const extractLabels = useCallback((fullPreset: Preset) => {
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

    // Extract from control values first (takes precedence)
    const cv = fullPreset.controlValues;
    // Label extraction only works with nK2-shaped MappingConfig presets
    const mapping = fullPreset.mapping as MappingConfig;

    for (let i = 0; i < 8; i++) {
      const trackCv = cv?.tracks[i];
      const trackMapping = mapping.tracks[i];
      labels.tracks.push({
        knob: trackCv?.knobLabel ?? trackMapping?.knob.label,
        slider: trackCv?.sliderLabel ?? trackMapping?.slider.label,
        solo: trackCv?.soloLabel ?? trackMapping?.solo.label,
        mute: trackCv?.muteLabel ?? trackMapping?.mute.label,
        rec: trackCv?.recLabel ?? trackMapping?.rec.label,
      });
    }

    labels.transport = {
      play: cv?.transport.playLabel ?? mapping.transport.play.label,
      stop: cv?.transport.stopLabel ?? mapping.transport.stop.label,
      rewind: cv?.transport.rewindLabel ?? mapping.transport.rewind.label,
      forward: cv?.transport.forwardLabel ?? mapping.transport.forward.label,
      record: cv?.transport.recordLabel ?? mapping.transport.record.label,
      cycle: cv?.transport.cycleLabel ?? mapping.transport.cycle.label,
      track_left: cv?.transport.track_leftLabel ?? mapping.transport.track_left.label,
      track_right: cv?.transport.track_rightLabel ?? mapping.transport.track_right.label,
      marker_set: cv?.transport.marker_setLabel ?? mapping.transport.marker_set.label,
      marker_left: cv?.transport.marker_leftLabel ?? mapping.transport.marker_left.label,
      marker_right: cv?.transport.marker_rightLabel ?? mapping.transport.marker_right.label,
    };

    return labels;
  }, []);

  // Copy labels to clipboard
  const handleCopyLabels = useCallback(async () => {
    if (!jsonPreview.fullPreset) return;

    try {
      const labels = extractLabels(jsonPreview.fullPreset);
      await navigator.clipboard.writeText(JSON.stringify(labels, null, 2));
      setCopyLabelsFeedback('success');
      setTimeout(() => setCopyLabelsFeedback('idle'), 1500);
    } catch {
      setCopyLabelsFeedback('error');
      setTimeout(() => setCopyLabelsFeedback('idle'), 2000);
    }
  }, [jsonPreview.fullPreset, extractLabels]);

  // Paste labels from clipboard
  const handlePasteLabels = useCallback(async () => {
    if (!jsonPreview.fullPreset) return;

    const api = getElectronAPI();
    if (!api) return;

    try {
      const clipboardText = await navigator.clipboard.readText();
      const labels = JSON.parse(clipboardText);

      // Validate structure
      if (!labels.tracks || !Array.isArray(labels.tracks) || labels.tracks.length !== 8) {
        throw new Error('Invalid labels format: expected 8 tracks');
      }

      // Build updated control values with new labels
      const currentCv = jsonPreview.fullPreset.controlValues ?? {
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

      // Also update mapping config labels for consistency
      const updatedMapping = JSON.parse(JSON.stringify(jsonPreview.fullPreset.mapping));
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

      // Save the preset with updated labels (both controlValues and mapping)
      const updatedPreset: Preset = {
        ...jsonPreview.fullPreset,
        metadata: {
          ...jsonPreview.fullPreset.metadata,
          modifiedAt: new Date().toISOString(),
        },
        mapping: updatedMapping,
        controlValues: updatedCv,
      };

      const response = await api.savePreset({ preset: updatedPreset, overwrite: true });
      if (response.success) {
        // Update local state
        setJsonPreview({
          ...jsonPreview,
          controlValues: updatedCv,
          fullPreset: updatedPreset,
        });
        setPasteLabelsFeedback('success');
        setTimeout(() => setPasteLabelsFeedback('idle'), 1500);
      } else {
        throw new Error(response.error ?? 'Failed to save preset');
      }
    } catch {
      setPasteLabelsFeedback('error');
      setTimeout(() => setPasteLabelsFeedback('idle'), 2000);
    }
  }, [jsonPreview]);

  return (
    <div
      className={`
        rounded transition-colors border
        ${selected ? 'bg-nk-accent/20 border-nk-accent' : 'hover:bg-nk-light border-transparent'}
      `}
    >
      {/* Main row */}
      <div
        className="flex items-center gap-2 p-2 cursor-pointer"
        onClick={onSelect}
        onDoubleClick={handleLoad}
      >
        {/* Expand/collapse chevron */}
        {!compact && (
          <button
            className="text-gray-500 hover:text-gray-300 p-0.5"
            onClick={(e) => {
              e.stopPropagation();
              onToggleExpanded();
            }}
            title={expanded ? 'Collapse details' : 'Expand details'}
            aria-label={expanded ? 'Collapse details' : 'Expand details'}
          >
            <ChevronIcon expanded={expanded} />
          </button>
        )}

        {/* Favorite star */}
        <button
          className={`text-lg ${preset.favorite ? 'text-yellow-400' : 'text-gray-600 hover:text-gray-400'}`}
          onClick={(e) => {
            e.stopPropagation();
            onToggleFavorite();
          }}
          title={preset.favorite ? 'Remove from favorites' : 'Add to favorites'}
        >
          {preset.favorite ? '\u2605' : '\u2606'}
        </button>

        {/* Preset info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-gray-200 truncate">
              {preset.name}
            </span>
            {/* Group badge */}
            {!compact && preset.group && (
              <span
                className="px-1.5 py-0.5 bg-nk-accent/20 border border-nk-accent/40 rounded text-xs text-nk-accent"
                title={`Group: ${preset.group}`}
              >
                {preset.group}
              </span>
            )}
            {/* Tags (collapsed view - show first 2) */}
            {!compact && !expanded && preset.tags.length > 0 && (
              <div className="flex gap-1">
                {preset.tags.slice(0, 2).map((tag) => (
                  <span
                    key={tag}
                    className="px-1.5 py-0.5 bg-nk-border rounded text-xs text-gray-400"
                  >
                    {tag}
                  </span>
                ))}
                {preset.tags.length > 2 && (
                  <span className="text-xs text-gray-500">+{preset.tags.length - 2}</span>
                )}
              </div>
            )}
          </div>
          {/* Description (collapsed view - truncated) */}
          {!compact && !expanded && preset.description && (
            <p className="text-xs text-gray-500 truncate">{preset.description}</p>
          )}
        </div>

        {/* Date (collapsed view) */}
        {!compact && !expanded && (
          <span className="text-xs text-gray-600">
            {formatDate(preset.modifiedAt)}
          </span>
        )}

        {/* Action buttons */}
        <div className="flex gap-1">
          <button
            className={`btn btn-ghost text-xs py-1 px-2 min-w-[52px] transition-colors ${
              loadStatus === 'success' ? 'text-green-400 bg-green-900/20' :
              loadStatus === 'error' ? 'text-red-400 bg-red-900/20' :
              loadStatus === 'loading' ? 'text-gray-400' : ''
            }`}
            onClick={(e) => {
              e.stopPropagation();
              handleLoad();
            }}
            disabled={loadStatus === 'loading'}
            title="Load preset"
          >
            {loadStatus === 'loading' ? (
              <span className="flex items-center gap-1">
                <span className="animate-spin inline-block w-3 h-3 border border-gray-400 border-t-transparent rounded-full" />
              </span>
            ) : loadStatus === 'success' ? (
              <span title="Loaded successfully">{'\u2713'}</span>
            ) : loadStatus === 'error' ? (
              <span title="Load failed">{'\u2717'}</span>
            ) : (
              'Load'
            )}
          </button>
          {onEdit && (
            <button
              className="btn btn-ghost text-xs py-1 px-2"
              onClick={(e) => {
                e.stopPropagation();
                onEdit();
              }}
              title="Edit preset metadata"
            >
              Edit
            </button>
          )}
          {onDuplicate && (
            <button
              className="btn btn-ghost text-xs py-1 px-2"
              onClick={(e) => {
                e.stopPropagation();
                onDuplicate();
              }}
              title="Duplicate preset"
            >
              Duplicate
            </button>
          )}
          <button
            className="btn btn-ghost text-xs py-1 px-2 text-red-400 hover:text-red-300 hover:bg-red-900/20"
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
            title="Delete preset"
          >
            Delete
          </button>
        </div>
      </div>

      {/* Expanded details panel */}
      {!compact && expanded && (
        <div className="px-4 pb-3 pt-1 ml-6 border-t border-nk-border/50 space-y-2">
          {/* Full description */}
          {preset.description && (
            <div>
              <span className="text-xs font-medium text-gray-500">Description:</span>
              <p className="text-sm text-gray-300 mt-0.5">{preset.description}</p>
            </div>
          )}

          {/* All tags */}
          {preset.tags.length > 0 && (
            <div>
              <span className="text-xs font-medium text-gray-500">Tags:</span>
              <div className="flex flex-wrap gap-1 mt-1">
                {preset.tags.map((tag) => (
                  <span
                    key={tag}
                    className="px-2 py-0.5 bg-nk-border rounded text-xs text-gray-300"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Group/folder */}
          {preset.group && (
            <div>
              <span className="text-xs font-medium text-gray-500">Group:</span>
              <span className="text-sm text-gray-300 ml-2">{preset.group}</span>
            </div>
          )}

          {/* Dates */}
          <div className="grid grid-cols-2 gap-4 text-xs">
            <div>
              <span className="font-medium text-gray-500">Created:</span>
              <span className="text-gray-400 ml-2">{formatDateTime(preset.createdAt)}</span>
            </div>
            <div>
              <span className="font-medium text-gray-500">Modified:</span>
              <span className="text-gray-400 ml-2">{formatDateTime(preset.modifiedAt)}</span>
            </div>
          </div>

          {/* Author if present */}
          {preset.author && (
            <div className="text-xs">
              <span className="font-medium text-gray-500">Author:</span>
              <span className="text-gray-400 ml-2">{preset.author}</span>
            </div>
          )}

          {/* Preset ID (for debugging/reference) */}
          <div className="text-xs text-gray-600 truncate">
            ID: {preset.id}
          </div>

          {/* JSON Debug View */}
          <div className="mt-3 pt-2 border-t border-nk-border/50">
            <button
              className="text-xs text-nk-accent hover:text-nk-accent/80 flex items-center gap-1"
              onClick={(e) => {
                e.stopPropagation();
                handleShowJson();
              }}
              disabled={jsonPreview.loading}
            >
              {jsonPreview.loading ? (
                <span className="animate-spin inline-block w-3 h-3 border border-nk-accent border-t-transparent rounded-full" />
              ) : (
                <span>{showJson ? '\u25BC' : '\u25B6'}</span>
              )}
              <span>{jsonPreview.loading ? 'Loading control values...' : 'Control Values JSON'}</span>
            </button>

            {showJson && (
              <div className="mt-2">
                {jsonPreview.error ? (
                  <div className="text-xs text-red-400 bg-red-900/20 p-2 rounded flex items-center justify-between">
                    <span>Error: {jsonPreview.error}</span>
                    <button
                      className="text-red-300 hover:text-red-200 underline ml-2"
                      onClick={(e) => {
                        e.stopPropagation();
                        // Reset and retry
                        setJsonPreview({ loading: false, controlValues: null, fullPreset: null, error: null });
                        setShowJson(false);
                        // Trigger reload after state reset
                        setTimeout(() => handleShowJson(), 50);
                      }}
                    >
                      Retry
                    </button>
                  </div>
                ) : jsonPreview.controlValues === null ? (
                  <div className="text-xs text-gray-500 italic bg-nk-dark/50 p-2 rounded">
                    No control values stored in this preset (older preset format or preset created before control value capture was implemented)
                  </div>
                ) : (
                  <div>
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-xs text-gray-500">
                        {jsonPreview.controlValues.tracks?.length || 0} tracks, transport controls
                      </span>
                      <div className="flex gap-2">
                        <button
                          className="text-xs text-gray-400 hover:text-gray-300"
                          onClick={(e) => {
                            e.stopPropagation();
                            navigator.clipboard.writeText(JSON.stringify(jsonPreview.controlValues, null, 2));
                          }}
                          title="Copy JSON to clipboard"
                        >
                          Copy JSON
                        </button>
                        {onEditControlValues && jsonPreview.fullPreset && (
                          <button
                            className="text-xs text-nk-accent hover:text-nk-accent/80"
                            onClick={(e) => {
                              e.stopPropagation();
                              onEditControlValues(jsonPreview.fullPreset!);
                            }}
                            title="Edit control values JSON"
                          >
                            Edit JSON
                          </button>
                        )}
                      </div>
                    </div>
                    <pre className="text-xs bg-nk-dark p-2 rounded overflow-auto max-h-64 text-gray-300 font-mono">
                      {JSON.stringify(jsonPreview.controlValues, null, 2)}
                    </pre>
                    {/* Copy/Paste Labels buttons */}
                    <div className="flex gap-2 mt-2 pt-2 border-t border-nk-border/50">
                      <button
                        className={`text-xs px-2 py-1 rounded transition-colors ${
                          copyLabelsFeedback === 'success' ? 'bg-green-900/30 text-green-400' :
                          copyLabelsFeedback === 'error' ? 'bg-red-900/30 text-red-400' :
                          'bg-nk-light text-gray-300 hover:bg-nk-border'
                        }`}
                        onClick={(e) => {
                          e.stopPropagation();
                          handleCopyLabels();
                        }}
                        title="Copy all labels to clipboard"
                      >
                        {copyLabelsFeedback === 'success' ? 'Copied!' :
                         copyLabelsFeedback === 'error' ? 'Error' :
                         'Copy Labels'}
                      </button>
                      <button
                        className={`text-xs px-2 py-1 rounded transition-colors ${
                          pasteLabelsFeedback === 'success' ? 'bg-green-900/30 text-green-400' :
                          pasteLabelsFeedback === 'error' ? 'bg-red-900/30 text-red-400' :
                          'bg-nk-light text-gray-300 hover:bg-nk-border'
                        }`}
                        onClick={(e) => {
                          e.stopPropagation();
                          handlePasteLabels();
                        }}
                        title="Paste labels from clipboard"
                      >
                        {pasteLabelsFeedback === 'success' ? 'Pasted!' :
                         pasteLabelsFeedback === 'error' ? 'Error' :
                         'Paste Labels'}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
