/**
 * Edit Control Values Dialog Component
 *
 * Modal dialog for editing preset control values, labels, and CC targets as JSON.
 * Features syntax highlighting, validation, and error display.
 *
 * The comprehensive JSON structure includes:
 * - Control values (knob/slider 0-127, button booleans)
 * - Labels for each control
 * - Output CC numbers and MIDI channels
 */

import React, { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import { usePresetStore } from '../../stores/preset';
import type { Preset, ControlValues } from '@shared/ipc-protocol';
import type { MappingConfig } from '@shared/types';

interface EditControlValuesDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** The preset to edit control values for. Dialog will not render if null. */
  preset: Preset | null;
}

// Syntax highlighting colors
const COLORS = {
  key: '#9cdcfe',      // Blue - property names
  string: '#ce9178',   // Orange - string values
  number: '#b5cea8',   // Green - numbers
  boolean: '#569cd6',  // Blue - true/false
  null: '#569cd6',     // Blue - null
  bracket: '#ffd700',  // Gold - brackets
  colon: '#d4d4d4',    // Gray - colons
  comma: '#d4d4d4',    // Gray - commas
};

/**
 * Applies syntax highlighting to JSON string
 */
function highlightJSON(json: string): React.ReactNode {
  const lines = json.split('\n');

  return lines.map((line, lineIndex) => {
    const parts: React.ReactNode[] = [];
    let remaining = line;
    let partIndex = 0;

    while (remaining.length > 0) {
      const whitespaceMatch = remaining.match(/^(\s+)/);
      if (whitespaceMatch && whitespaceMatch[1]) {
        parts.push(<span key={`${lineIndex}-${partIndex++}`}>{whitespaceMatch[1]}</span>);
        remaining = remaining.slice(whitespaceMatch[1].length);
        continue;
      }

      const keyMatch = remaining.match(/^("(?:[^"\\]|\\.)*")(\s*:)/);
      if (keyMatch) {
        parts.push(
          <span key={`${lineIndex}-${partIndex++}`} style={{ color: COLORS.key }}>
            {keyMatch[1]}
          </span>
        );
        parts.push(
          <span key={`${lineIndex}-${partIndex++}`} style={{ color: COLORS.colon }}>
            {keyMatch[2]}
          </span>
        );
        remaining = remaining.slice(keyMatch[0].length);
        continue;
      }

      const stringMatch = remaining.match(/^("(?:[^"\\]|\\.)*")/);
      if (stringMatch && stringMatch[1]) {
        parts.push(
          <span key={`${lineIndex}-${partIndex++}`} style={{ color: COLORS.string }}>
            {stringMatch[1]}
          </span>
        );
        remaining = remaining.slice(stringMatch[1].length);
        continue;
      }

      const numberMatch = remaining.match(/^(-?\d+\.?\d*(?:[eE][+-]?\d+)?)/);
      if (numberMatch && numberMatch[1]) {
        parts.push(
          <span key={`${lineIndex}-${partIndex++}`} style={{ color: COLORS.number }}>
            {numberMatch[1]}
          </span>
        );
        remaining = remaining.slice(numberMatch[1].length);
        continue;
      }

      const boolNullMatch = remaining.match(/^(true|false|null)(?![a-zA-Z0-9_])/);
      if (boolNullMatch && boolNullMatch[1]) {
        parts.push(
          <span key={`${lineIndex}-${partIndex++}`} style={{ color: COLORS.boolean }}>
            {boolNullMatch[1]}
          </span>
        );
        remaining = remaining.slice(boolNullMatch[1].length);
        continue;
      }

      const bracketMatch = remaining.match(/^([{}\[\]])/);
      if (bracketMatch) {
        parts.push(
          <span key={`${lineIndex}-${partIndex++}`} style={{ color: COLORS.bracket }}>
            {bracketMatch[1]}
          </span>
        );
        remaining = remaining.slice(1);
        continue;
      }

      if (remaining[0] === ',') {
        parts.push(
          <span key={`${lineIndex}-${partIndex++}`} style={{ color: COLORS.comma }}>
            ,
          </span>
        );
        remaining = remaining.slice(1);
        continue;
      }

      parts.push(<span key={`${lineIndex}-${partIndex++}`}>{remaining[0]}</span>);
      remaining = remaining.slice(1);
    }

    return (
      <div key={lineIndex} style={{ minHeight: '1.625em', lineHeight: '1.625' }}>
        {parts.length > 0 ? parts : '\u00A0'}
      </div>
    );
  });
}

/**
 * Builds a comprehensive JSON object from preset data,
 * merging controlValues, labels, and mapping CC info.
 */
function buildComprehensiveJson(preset: Preset): object {
  const cv = preset.controlValues;
  // buildComprehensiveJson only handles nK2-shaped MappingConfig presets
  const mapping = preset.mapping as MappingConfig;

  const tracks = [];
  for (let i = 0; i < 8; i++) {
    const trackCv = cv?.tracks[i];
    const trackMap = mapping.tracks[i];

    tracks.push({
      knob: {
        value: trackCv?.knob ?? 0,
        label: trackCv?.knobLabel ?? trackMap?.knob.label ?? null,
        outputCC: trackMap?.knob.outputCC ?? 0,
        channel: trackMap?.knob.channel ?? 1,
      },
      slider: {
        value: trackCv?.slider ?? 0,
        label: trackCv?.sliderLabel ?? trackMap?.slider.label ?? null,
        outputCC: trackMap?.slider.outputCC ?? 0,
        channel: trackMap?.slider.channel ?? 1,
      },
      solo: {
        active: trackCv?.solo ?? false,
        label: trackCv?.soloLabel ?? trackMap?.solo.label ?? null,
        outputCC: trackMap?.solo.outputCC ?? 0,
        channel: trackMap?.solo.channel ?? 1,
      },
      mute: {
        active: trackCv?.mute ?? false,
        label: trackCv?.muteLabel ?? trackMap?.mute.label ?? null,
        outputCC: trackMap?.mute.outputCC ?? 0,
        channel: trackMap?.mute.channel ?? 1,
      },
      rec: {
        active: trackCv?.rec ?? false,
        label: trackCv?.recLabel ?? trackMap?.rec.label ?? null,
        outputCC: trackMap?.rec.outputCC ?? 0,
        channel: trackMap?.rec.channel ?? 1,
      },
    });
  }

  const transportKeys = [
    'play', 'stop', 'rewind', 'forward', 'record', 'cycle',
    'track_left', 'track_right', 'marker_set', 'marker_left', 'marker_right',
  ] as const;

  const labelKeyMap: Record<string, string> = {
    play: 'playLabel', stop: 'stopLabel', rewind: 'rewindLabel',
    forward: 'forwardLabel', record: 'recordLabel', cycle: 'cycleLabel',
    track_left: 'track_leftLabel', track_right: 'track_rightLabel',
    marker_set: 'marker_setLabel', marker_left: 'marker_leftLabel',
    marker_right: 'marker_rightLabel',
  };

  const transport: Record<string, object> = {};
  for (const key of transportKeys) {
    const transportCv = cv?.transport;
    const transportMap = mapping.transport[key];
    const labelKey = labelKeyMap[key] as keyof typeof transportCv;

    transport[key] = {
      active: transportCv?.[key as keyof typeof transportCv] ?? false,
      label: (transportCv && labelKey ? transportCv[labelKey] : undefined) ?? transportMap?.label ?? null,
      outputCC: transportMap?.outputCC ?? 0,
      channel: transportMap?.channel ?? 1,
    };
  }

  return { tracks, transport };
}

/**
 * Parses the comprehensive JSON back into separate controlValues and mapping updates.
 */
function parseComprehensiveJson(
  json: object,
  originalMapping: MappingConfig
): { controlValues: ControlValues; mapping: MappingConfig } | { error: string } {
  const data = json as {
    tracks?: Array<{
      knob?: { value?: number; label?: string | null; outputCC?: number; channel?: number };
      slider?: { value?: number; label?: string | null; outputCC?: number; channel?: number };
      solo?: { active?: boolean; label?: string | null; outputCC?: number; channel?: number };
      mute?: { active?: boolean; label?: string | null; outputCC?: number; channel?: number };
      rec?: { active?: boolean; label?: string | null; outputCC?: number; channel?: number };
    }>;
    transport?: Record<string, { active?: boolean; label?: string | null; outputCC?: number; channel?: number }>;
  };

  if (!data.tracks || !Array.isArray(data.tracks) || data.tracks.length !== 8) {
    return { error: 'Expected 8 tracks in tracks array' };
  }
  if (!data.transport || typeof data.transport !== 'object') {
    return { error: 'Expected transport object' };
  }

  // Deep clone original mapping to apply CC/channel changes
  const mapping: MappingConfig = JSON.parse(JSON.stringify(originalMapping));

  const cvTracks = [];
  for (let i = 0; i < 8; i++) {
    const t = data.tracks[i];
    if (!t) return { error: `Track ${i} is missing` };

    const knobVal = t.knob?.value ?? 0;
    const sliderVal = t.slider?.value ?? 0;
    if (typeof knobVal !== 'number' || knobVal < 0 || knobVal > 127) {
      return { error: `Track ${i + 1} knob value must be 0-127` };
    }
    if (typeof sliderVal !== 'number' || sliderVal < 0 || sliderVal > 127) {
      return { error: `Track ${i + 1} slider value must be 0-127` };
    }

    cvTracks.push({
      knob: knobVal,
      knobLabel: t.knob?.label ?? undefined,
      slider: sliderVal,
      sliderLabel: t.slider?.label ?? undefined,
      solo: t.solo?.active ?? false,
      soloLabel: t.solo?.label ?? undefined,
      mute: t.mute?.active ?? false,
      muteLabel: t.mute?.label ?? undefined,
      rec: t.rec?.active ?? false,
      recLabel: t.rec?.label ?? undefined,
    });

    // Update mapping CC/channel/label
    const mt = mapping.tracks[i];
    if (mt) {
      if (t.knob?.outputCC !== undefined) mt.knob.outputCC = t.knob.outputCC;
      if (t.knob?.channel !== undefined) mt.knob.channel = t.knob.channel;
      mt.knob.label = t.knob?.label ?? undefined;
      if (t.slider?.outputCC !== undefined) mt.slider.outputCC = t.slider.outputCC;
      if (t.slider?.channel !== undefined) mt.slider.channel = t.slider.channel;
      mt.slider.label = t.slider?.label ?? undefined;
      if (t.solo?.outputCC !== undefined) mt.solo.outputCC = t.solo.outputCC;
      if (t.solo?.channel !== undefined) mt.solo.channel = t.solo.channel;
      mt.solo.label = t.solo?.label ?? undefined;
      if (t.mute?.outputCC !== undefined) mt.mute.outputCC = t.mute.outputCC;
      if (t.mute?.channel !== undefined) mt.mute.channel = t.mute.channel;
      mt.mute.label = t.mute?.label ?? undefined;
      if (t.rec?.outputCC !== undefined) mt.rec.outputCC = t.rec.outputCC;
      if (t.rec?.channel !== undefined) mt.rec.channel = t.rec.channel;
      mt.rec.label = t.rec?.label ?? undefined;
    }
  }

  const transportKeys = [
    'play', 'stop', 'rewind', 'forward', 'record', 'cycle',
    'track_left', 'track_right', 'marker_set', 'marker_left', 'marker_right',
  ] as const;

  const labelKeyMap: Record<string, string> = {
    play: 'playLabel', stop: 'stopLabel', rewind: 'rewindLabel',
    forward: 'forwardLabel', record: 'recordLabel', cycle: 'cycleLabel',
    track_left: 'track_leftLabel', track_right: 'track_rightLabel',
    marker_set: 'marker_setLabel', marker_left: 'marker_leftLabel',
    marker_right: 'marker_rightLabel',
  };

  const cvTransport: ControlValues['transport'] = {
    play: false, playLabel: undefined,
    stop: false, stopLabel: undefined,
    rewind: false, rewindLabel: undefined,
    forward: false, forwardLabel: undefined,
    record: false, recordLabel: undefined,
    cycle: false, cycleLabel: undefined,
    track_left: false, track_leftLabel: undefined,
    track_right: false, track_rightLabel: undefined,
    marker_set: false, marker_setLabel: undefined,
    marker_left: false, marker_leftLabel: undefined,
    marker_right: false, marker_rightLabel: undefined,
  };
  for (const key of transportKeys) {
    const entry = data.transport[key];
    cvTransport[key] = entry?.active ?? false;
    const labelField = labelKeyMap[key] as keyof typeof cvTransport;
    if (labelField) {
      cvTransport[labelField] = (entry?.label ?? undefined) as never;
    }

    // Update mapping
    const mt = mapping.transport[key];
    if (mt && entry) {
      if (entry.outputCC !== undefined) mt.outputCC = entry.outputCC;
      if (entry.channel !== undefined) mt.channel = entry.channel;
      mt.label = entry.label ?? undefined;
    }
  }

  return {
    controlValues: {
      tracks: cvTracks,
      transport: cvTransport,
    },
    mapping,
  };
}

/**
 * Validates the comprehensive JSON structure.
 */
function validateComprehensiveJson(jsonText: string): string | null {
  if (!jsonText.trim()) return 'JSON content is required';

  let parsed;
  try {
    parsed = JSON.parse(jsonText);
  } catch {
    return 'Invalid JSON syntax';
  }

  if (!parsed.tracks || !Array.isArray(parsed.tracks)) return 'Missing tracks array';
  if (parsed.tracks.length !== 8) return `Expected 8 tracks, got ${parsed.tracks.length}`;
  if (!parsed.transport || typeof parsed.transport !== 'object') return 'Missing transport object';

  // Validate each track
  for (let i = 0; i < 8; i++) {
    const t = parsed.tracks[i];
    if (!t) return `Track ${i + 1} is missing`;
    for (const ctrl of ['knob', 'slider']) {
      if (t[ctrl]?.value !== undefined) {
        const v = t[ctrl].value;
        if (typeof v !== 'number' || v < 0 || v > 127) {
          return `Track ${i + 1} ${ctrl} value must be 0-127`;
        }
      }
      if (t[ctrl]?.outputCC !== undefined) {
        const cc = t[ctrl].outputCC;
        if (typeof cc !== 'number' || cc < 0 || cc > 127) {
          return `Track ${i + 1} ${ctrl} outputCC must be 0-127`;
        }
      }
      if (t[ctrl]?.channel !== undefined) {
        const ch = t[ctrl].channel;
        if (typeof ch !== 'number' || ch < 1 || ch > 16) {
          return `Track ${i + 1} ${ctrl} channel must be 1-16`;
        }
      }
    }
    for (const ctrl of ['solo', 'mute', 'rec']) {
      if (t[ctrl]?.outputCC !== undefined) {
        const cc = t[ctrl].outputCC;
        if (typeof cc !== 'number' || cc < 0 || cc > 127) {
          return `Track ${i + 1} ${ctrl} outputCC must be 0-127`;
        }
      }
    }
  }

  return null;
}

export function EditControlValuesDialog({ isOpen, onClose, preset }: EditControlValuesDialogProps): React.ReactElement | null {
  const [jsonText, setJsonText] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [validationError, setValidationError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const preRef = useRef<HTMLPreElement>(null);

  const savePreset = usePresetStore((state) => state.savePreset);
  const setCurrentPreset = usePresetStore((state) => state.setCurrentPreset);
  const currentPreset = usePresetStore((state) => state.currentPreset);

  // Initialize JSON text when preset changes or dialog opens
  useEffect(() => {
    if (preset && isOpen) {
      const comprehensive = buildComprehensiveJson(preset);
      setJsonText(JSON.stringify(comprehensive, null, 2));
      setError(null);
      setValidationError(null);
      requestAnimationFrame(() => {
        if (textareaRef.current && preRef.current) {
          textareaRef.current.scrollTop = 0;
          textareaRef.current.scrollLeft = 0;
          preRef.current.scrollTop = 0;
          preRef.current.scrollLeft = 0;
        }
      });
    }
  }, [preset, preset?.metadata.id, isOpen]);

  // Handle Escape key
  useEffect(() => {
    if (!isOpen) return;
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !isSubmitting) onClose();
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, isSubmitting, onClose]);

  // Validate JSON on change
  useEffect(() => {
    setValidationError(validateComprehensiveJson(jsonText));
  }, [jsonText]);

  // Sync scroll
  const handleScroll = useCallback(() => {
    if (textareaRef.current && preRef.current) {
      preRef.current.scrollTop = textareaRef.current.scrollTop;
      preRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  // Tab indentation
  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Tab') {
      e.preventDefault();
      const textarea = e.currentTarget;
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      const newText = jsonText.substring(0, start) + '  ' + jsonText.substring(end);
      setJsonText(newText);
      setTimeout(() => {
        textarea.selectionStart = textarea.selectionEnd = start + 2;
      }, 0);
    }
  }, [jsonText]);

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!preset) {
      setError('No preset selected');
      return;
    }

    if (validationError) {
      setError('Please fix validation errors before saving');
      return;
    }

    let parsed;
    try {
      parsed = JSON.parse(jsonText);
    } catch {
      setError('Invalid JSON syntax');
      return;
    }

    // parseComprehensiveJson expects nK2-shaped MappingConfig
    const result = parseComprehensiveJson(parsed, preset.mapping as MappingConfig);
    if ('error' in result) {
      setError(result.error);
      return;
    }

    setIsSubmitting(true);

    try {
      const updatedPreset: Preset = {
        ...preset,
        metadata: {
          ...preset.metadata,
          modifiedAt: new Date().toISOString(),
        },
        controlValues: result.controlValues,
        mapping: result.mapping,
      };

      const success = await savePreset(updatedPreset, true);

      if (success) {
        if (currentPreset && currentPreset.metadata.id === preset.metadata.id) {
          setCurrentPreset(updatedPreset);
        }
        onClose();
      } else {
        setError('Failed to save preset');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setIsSubmitting(false);
    }
  }, [preset, jsonText, validationError, savePreset, currentPreset, setCurrentPreset, onClose]);

  const handleClose = useCallback(() => {
    if (!isSubmitting) {
      setError(null);
      setValidationError(null);
      onClose();
    }
  }, [isSubmitting, onClose]);

  const handleFormat = useCallback(() => {
    try {
      const parsed = JSON.parse(jsonText);
      setJsonText(JSON.stringify(parsed, null, 2));
      requestAnimationFrame(() => {
        if (textareaRef.current && preRef.current) {
          textareaRef.current.scrollTop = 0;
          textareaRef.current.scrollLeft = 0;
          preRef.current.scrollTop = 0;
          preRef.current.scrollLeft = 0;
        }
      });
    } catch {
      // Can't format invalid JSON
    }
  }, [jsonText]);

  const highlightedContent = useMemo(() => highlightJSON(jsonText), [jsonText]);

  if (!isOpen || !preset) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/60" onClick={handleClose} />

      {/* Dialog */}
      <div className="relative bg-nk-dark border border-nk-border rounded-lg shadow-xl w-full max-w-4xl mx-4 max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-nk-border shrink-0">
          <div>
            <h2 className="text-lg font-semibold text-gray-200">Edit Preset Configuration</h2>
            <p className="text-sm text-gray-500">{preset.metadata.name}</p>
          </div>
          <button
            className="text-gray-500 hover:text-gray-300 text-xl leading-none"
            onClick={handleClose}
            disabled={isSubmitting}
            aria-label="Close dialog"
          >
            x
          </button>
        </div>

        {/* Content */}
        <div className="p-4 flex-1 overflow-hidden flex flex-col min-h-0">
          {error && (
            <div className="text-red-400 text-sm bg-red-900/20 border border-red-800 rounded px-3 py-2 mb-4 shrink-0">
              {error}
            </div>
          )}

          <div className="text-sm text-gray-500 mb-3 shrink-0">
            Edit control values, labels, output CC numbers, and MIDI channels below.
            Values for knobs/sliders must be 0-127. CC numbers must be 0-127. Channels must be 1-16.
            Labels can be null or a string.
          </div>

          {/* JSON Editor */}
          <div className="relative flex-1 rounded-lg border border-nk-border overflow-hidden bg-[#1e1e1e]" style={{ minHeight: '400px' }}>
            {/* Highlighted code (background) */}
            <pre
              ref={preRef}
              className="absolute inset-0 p-4 m-0 overflow-auto pointer-events-none font-mono text-sm leading-relaxed whitespace-pre"
              style={{
                fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
                color: '#d4d4d4',
              }}
              aria-hidden="true"
            >
              <code>{highlightedContent}</code>
            </pre>

            {/* Textarea (foreground, transparent) */}
            <textarea
              ref={textareaRef}
              className="absolute inset-0 p-4 m-0 resize-none font-mono text-sm leading-relaxed bg-transparent caret-white outline-none whitespace-pre overflow-auto"
              style={{
                fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
                color: 'transparent',
                caretColor: 'white',
              }}
              value={jsonText}
              onChange={(e) => setJsonText(e.target.value)}
              onScroll={handleScroll}
              onKeyDown={handleKeyDown}
              disabled={isSubmitting}
              spellCheck={false}
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
            />
          </div>

          {/* Validation error */}
          {validationError && (
            <div className="mt-3 text-sm text-yellow-400 bg-yellow-900/20 border border-yellow-800 rounded px-3 py-2 shrink-0 max-h-24 overflow-auto">
              <div className="font-medium mb-1">Validation Errors:</div>
              <pre className="whitespace-pre-wrap text-xs">{validationError}</pre>
            </div>
          )}

          {/* Actions */}
          <div className="flex justify-between items-center pt-4 shrink-0">
            <button
              type="button"
              className="btn btn-ghost text-sm"
              onClick={handleFormat}
              disabled={isSubmitting}
            >
              Format JSON
            </button>
            <div className="flex gap-2">
              <button
                type="button"
                className="btn btn-ghost"
                onClick={handleClose}
                disabled={isSubmitting}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={handleSubmit}
                disabled={isSubmitting || !!validationError}
              >
                {isSubmitting ? 'Saving...' : 'Save Changes'}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
