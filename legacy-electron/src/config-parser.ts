/**
 * Parser for human-readable mapping configuration files.
 *
 * The mapping file format supports:
 * - Comments: lines starting with #
 * - Blank lines: ignored
 * - Device header: [device <id>] (optional, defaults to nanokontrol2)
 * - Sections: [trackN], [transport], [master], or other device-defined sections
 * - Control mappings: controlType inputCC -> outputCC chN [behavior] ["label"]
 * - Note syntax: note:N for controls that use MIDI Note messages instead of CC
 *
 * Example (nanoKONTROL2 - backward compatible):
 * ```
 * [track1]
 * knob    16 -> 16 ch1 "Filter Cutoff"
 * slider   0 ->  0 ch1 "Volume"
 * solo    32 -> 32 ch1 toggle "Solo"
 * ```
 *
 * Example (Akai MidiMix):
 * ```
 * [device akai-midimix]
 *
 * [track1]
 * knob1   16 -> 74 ch1 "Filter Cutoff"
 * fader   19 -> 19 ch1 "Volume"
 * mute    note:1 -> note:1 ch1 toggle "Mute"
 *
 * [master]
 * master_fader 62 -> 62 ch1 "Master"
 * ```
 */

import type {
  MappingConfig,
  MappingEntry,
  TrackMapping,
  TransportMapping,
  ControlType,
  TransportControlType,
  ButtonBehavior,
  ParseResult,
  MidiMessageType,
} from './types.js';

import {
  MIDI_VALUES,
  MIDI_CHANNELS,
  TRACK_CONSTANTS,
  TRACK_CONTROL_TYPES,
  TRANSPORT_CONTROL_TYPES,
  getDefaultBehavior,
  isContinuousControl,
} from './constants.js';

import { getDeviceDefinition } from './shared/devices/registry.js';
import type { DeviceDefinition, ControlDef } from './shared/devices/device-definition.js';

// =============================================================================
// Constants
// =============================================================================

const DEFAULT_DEVICE_ID = 'nanokontrol2';

// =============================================================================
// Parsing Error Class
// =============================================================================

/**
 * Error thrown when parsing fails with line number context.
 */
export class ParseError extends Error {
  constructor(
    message: string,
    public readonly lineNumber: number,
    public readonly lineContent: string
  ) {
    super(`Line ${lineNumber}: ${message}\n  > ${lineContent}`);
    this.name = 'ParseError';
  }
}

// =============================================================================
// Internal Types
// =============================================================================

type SectionType = 'track' | 'transport' | 'extra';

interface CurrentSection {
  type: SectionType;
  trackNumber?: number; // 1-8 for track sections
  /** Name of the extra section (e.g., 'master') when type is 'extra' */
  sectionName?: string;
}

interface PartialTrackMapping {
  knob?: MappingEntry;
  slider?: MappingEntry;
  solo?: MappingEntry;
  mute?: MappingEntry;
  rec?: MappingEntry;
}

interface PartialTransportMapping {
  rewind?: MappingEntry;
  forward?: MappingEntry;
  stop?: MappingEntry;
  play?: MappingEntry;
  record?: MappingEntry;
  cycle?: MappingEntry;
  track_left?: MappingEntry;
  track_right?: MappingEntry;
  marker_set?: MappingEntry;
  marker_left?: MappingEntry;
  marker_right?: MappingEntry;
}

/**
 * Holds parsed MIDI number along with its message type.
 * Allows distinguishing between CC and Note values parsed from config lines.
 */
interface ParsedMidiNumber {
  value: number;
  messageType: MidiMessageType;
}

// =============================================================================
// Device-Aware Validation Helpers
// =============================================================================

/**
 * Extracts all valid control type names for track sections from a device definition.
 * Returns the set of control names found across all track layouts.
 */
function getDeviceTrackControlTypes(device: DeviceDefinition): string[] {
  const controlNames = new Set<string>();
  for (const layout of device.trackLayouts) {
    for (const control of layout.continuousControls) {
      controlNames.add(control.name);
    }
    for (const button of layout.buttonControls) {
      controlNames.add(button.name);
    }
  }
  return Array.from(controlNames);
}

/**
 * Extracts all valid control type names for a named extra section from a device definition.
 */
function getDeviceExtraSectionControlTypes(device: DeviceDefinition, sectionName: string): string[] {
  const section = device.extraSections.find(s => s.name === sectionName);
  if (!section) {
    return [];
  }
  return section.controls.map(c => c.name);
}

/**
 * Finds the control definition for a given control name in a device's track layout.
 * Returns undefined if no matching control is found.
 */
function findTrackControlDef(device: DeviceDefinition, controlName: string): ControlDef | undefined {
  const firstLayout = device.trackLayouts[0];
  if (!firstLayout) {
    return undefined;
  }
  const allControls: ControlDef[] = [...firstLayout.continuousControls, ...firstLayout.buttonControls];
  return allControls.find(c => c.name === controlName);
}

/**
 * Finds the control definition for a given control name in a device's extra section.
 */
function findExtraSectionControlDef(device: DeviceDefinition, sectionName: string, controlName: string): ControlDef | undefined {
  const section = device.extraSections.find(s => s.name === sectionName);
  if (!section) {
    return undefined;
  }
  return section.controls.find(c => c.name === controlName);
}

/**
 * Determines whether a control is continuous based on the device definition.
 * Falls back to the legacy isContinuousControl() for nK2 compatibility.
 */
function isDeviceContinuousControl(controlName: string, device: DeviceDefinition | null, sectionName?: string): boolean {
  if (!device) {
    return isContinuousControl(controlName);
  }

  let controlDef: ControlDef | undefined;
  if (sectionName) {
    controlDef = findExtraSectionControlDef(device, sectionName, controlName);
  } else {
    controlDef = findTrackControlDef(device, controlName);
  }

  if (controlDef) {
    return controlDef.kind === 'continuous';
  }

  // Fallback for nK2 legacy control types
  return isContinuousControl(controlName);
}

/**
 * Gets the default behavior for a control based on the device definition.
 * Falls back to the legacy getDefaultBehavior() for nK2 compatibility.
 */
function getDeviceDefaultBehavior(controlName: string, device: DeviceDefinition | null, sectionName?: string): 'toggle' | 'momentary' {
  if (!device) {
    return getDefaultBehavior(controlName);
  }

  let controlDef: ControlDef | undefined;
  if (sectionName) {
    controlDef = findExtraSectionControlDef(device, sectionName, controlName);
  } else {
    controlDef = findTrackControlDef(device, controlName);
  }

  if (controlDef && controlDef.kind === 'button') {
    return controlDef.defaultBehavior;
  }

  return getDefaultBehavior(controlName);
}

// =============================================================================
// Validation Helpers
// =============================================================================

/**
 * Validates that a MIDI number is within the valid range (0-127).
 */
function validateMidiRange(midiNumber: number, fieldName: string, lineNumber: number, lineContent: string): void {
  if (!Number.isInteger(midiNumber) || midiNumber < MIDI_VALUES.MIN || midiNumber > MIDI_VALUES.MAX) {
    throw new ParseError(
      `${fieldName} must be an integer between ${MIDI_VALUES.MIN} and ${MIDI_VALUES.MAX}, got: ${midiNumber}`,
      lineNumber,
      lineContent
    );
  }
}

/**
 * Validates that a channel number is within the valid range (1-16).
 */
function validateChannelRange(channel: number, lineNumber: number, lineContent: string): void {
  if (!Number.isInteger(channel) || channel < MIDI_CHANNELS.MIN || channel > MIDI_CHANNELS.MAX) {
    throw new ParseError(
      `Channel must be an integer between ${MIDI_CHANNELS.MIN} and ${MIDI_CHANNELS.MAX}, got: ${channel}`,
      lineNumber,
      lineContent
    );
  }
}

/**
 * Validates that a control type is valid for a track section.
 */
function isValidTrackControlType(controlType: string): controlType is ControlType {
  return (TRACK_CONTROL_TYPES as readonly string[]).includes(controlType);
}

/**
 * Validates that a control type is valid for a transport section.
 */
function isValidTransportControlType(controlType: string): controlType is TransportControlType {
  return (TRANSPORT_CONTROL_TYPES as readonly string[]).includes(controlType);
}

/**
 * Validates that a behavior is valid.
 */
function isValidBehavior(behavior: string): behavior is ButtonBehavior {
  return behavior === 'toggle' || behavior === 'momentary';
}

// =============================================================================
// MIDI Number Parsing
// =============================================================================

/**
 * Parses a MIDI number string that may use note:N syntax.
 *
 * - Plain number (e.g., "16") -> { value: 16, messageType: 'cc' }
 * - Note prefix (e.g., "note:1") -> { value: 1, messageType: 'note' }
 *
 * Returns null if the string cannot be parsed as a valid MIDI number.
 */
function parseMidiNumber(raw: string): ParsedMidiNumber | null {
  const notePrefixMatch = raw.match(/^note:(\d+)$/i);
  if (notePrefixMatch) {
    const noteStr = notePrefixMatch[1];
    if (noteStr === undefined) {
      return null;
    }
    const noteValue = parseInt(noteStr, 10);
    if (isNaN(noteValue)) {
      return null;
    }
    return { value: noteValue, messageType: 'note' };
  }

  const ccValue = parseInt(raw, 10);
  if (isNaN(ccValue)) {
    return null;
  }
  return { value: ccValue, messageType: 'cc' };
}

// =============================================================================
// Device Header Scanning
// =============================================================================

/**
 * Scans the config content for a [device <id>] header line.
 * Returns the device ID if found, or null if no device header is present.
 * Throws ParseError if the device header has invalid syntax.
 */
function scanForDeviceHeader(lines: string[]): { deviceId: string; lineNumber: number } | null {
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line === undefined) {
      continue;
    }
    const trimmed = line.trim();

    // Skip empty lines and comments
    if (trimmed === '' || trimmed.startsWith('#')) {
      continue;
    }

    // Check for [device ...] header
    const deviceMatch = trimmed.match(/^\[device\s+(.+)\]$/i);
    if (deviceMatch) {
      const deviceId = deviceMatch[1]?.trim().toLowerCase();
      if (!deviceId) {
        throw new ParseError('Missing device ID in [device] header', i + 1, line);
      }
      return { deviceId, lineNumber: i + 1 };
    }
  }
  return null;
}

// =============================================================================
// Line Parsing
// =============================================================================

/**
 * Parses a section header line like [track1], [transport], [master], etc.
 * Returns null if the line is not a section header.
 *
 * When a device definition is provided, extra section names from that device
 * are accepted in addition to the standard [trackN] and [transport] sections.
 */
function parseSectionHeader(
  line: string,
  lineNumber: number,
  device: DeviceDefinition | null
): CurrentSection | null {
  const trimmed = line.trim();

  if (!trimmed.startsWith('[') || !trimmed.endsWith(']')) {
    return null;
  }

  const sectionName = trimmed.slice(1, -1).toLowerCase();

  // Skip [device ...] headers - they are handled separately
  if (sectionName.startsWith('device ')) {
    return null;
  }

  if (sectionName === 'transport') {
    return { type: 'transport' };
  }

  // Match track1 through track8
  const trackMatch = sectionName.match(/^track(\d+)$/);
  if (trackMatch) {
    const capturedNumber = trackMatch[1];
    if (capturedNumber === undefined) {
      throw new ParseError('Failed to parse track number', lineNumber, line);
    }
    const trackNumber = parseInt(capturedNumber, 10);
    const maxTrack = device ? device.trackCount : TRACK_CONSTANTS.LAST;
    if (trackNumber >= TRACK_CONSTANTS.FIRST && trackNumber <= maxTrack) {
      return { type: 'track', trackNumber };
    }
    throw new ParseError(
      `Invalid track number: ${trackNumber}. Must be between ${TRACK_CONSTANTS.FIRST} and ${maxTrack}`,
      lineNumber,
      line
    );
  }

  // Check if the section name matches any extra section in the device definition
  if (device) {
    const extraSection = device.extraSections.find(s => s.name === sectionName);
    if (extraSection) {
      return { type: 'extra', sectionName };
    }
  }

  // Build a list of valid section names for the error message
  const validSections = ['[trackN] (1-8)', '[transport]'];
  if (device) {
    for (const extra of device.extraSections) {
      if (extra.name !== 'transport') {
        validSections.push(`[${extra.name}]`);
      }
    }
  }

  throw new ParseError(
    `Unknown section: ${sectionName}. Expected one of: ${validSections.join(', ')}`,
    lineNumber,
    line
  );
}

/**
 * Parses a control mapping line.
 *
 * Format: controlType inputMidi -> outputMidi [chN] [behavior] ["label"]
 *
 * MIDI numbers can be plain integers (CC) or note:N (Note messages).
 *
 * Examples:
 * - knob 16 -> 16 ch1 "Filter Cutoff"
 * - solo 32 -> 32 ch1 toggle "Solo Button"
 * - mute note:1 -> note:1 ch1 toggle "Mute"
 */
function parseControlLine(
  line: string,
  lineNumber: number,
  currentSection: CurrentSection | null,
  device: DeviceDefinition | null,
  isLegacyNk2: boolean
): { controlType: string; entry: MappingEntry } {
  if (!currentSection) {
    throw new ParseError(
      'Control mapping found outside of a section. Add a [trackN] or [transport] header first',
      lineNumber,
      line
    );
  }

  const trimmed = line.trim();

  // Extract quoted label first (if present) to avoid issues with spaces
  let label: string | undefined;
  let lineWithoutLabel = trimmed;

  const labelMatch = trimmed.match(/"([^"]*)"\s*$/);
  if (labelMatch) {
    const capturedLabel = labelMatch[1];
    if (capturedLabel !== undefined) {
      label = capturedLabel;
      lineWithoutLabel = trimmed.slice(0, trimmed.lastIndexOf('"' + label + '"')).trim();
    }
  }

  // Split remaining line by whitespace
  const parts = lineWithoutLabel.split(/\s+/);

  if (parts.length < 4) {
    throw new ParseError(
      'Invalid control mapping format. Expected: controlType inputCC -> outputCC [chN] [behavior] ["label"]',
      lineNumber,
      line
    );
  }

  const controlTypeRaw = parts[0];
  const inputMidiStr = parts[1];
  const arrow = parts[2];
  const outputMidiStr = parts[3];

  // Validate all required parts exist
  if (controlTypeRaw === undefined || inputMidiStr === undefined || arrow === undefined || outputMidiStr === undefined) {
    throw new ParseError(
      'Invalid control mapping format. Expected: controlType inputCC -> outputCC [chN] [behavior] ["label"]',
      lineNumber,
      line
    );
  }

  const controlType = controlTypeRaw.toLowerCase();

  // Validate arrow
  if (arrow !== '->') {
    throw new ParseError(
      `Expected '->' between input and output CC, got: ${arrow}`,
      lineNumber,
      line
    );
  }

  // Parse MIDI numbers (supporting note:N syntax)
  const inputMidi = parseMidiNumber(inputMidiStr);
  const outputMidi = parseMidiNumber(outputMidiStr);

  if (!inputMidi) {
    throw new ParseError(`Invalid input MIDI number: ${inputMidiStr}`, lineNumber, line);
  }
  if (!outputMidi) {
    throw new ParseError(`Invalid output MIDI number: ${outputMidiStr}`, lineNumber, line);
  }

  validateMidiRange(inputMidi.value, 'Input CC', lineNumber, line);
  validateMidiRange(outputMidi.value, 'Output CC', lineNumber, line);

  // Parse optional channel and behavior from remaining parts
  let channel: number = MIDI_CHANNELS.DEFAULT;
  let behavior: ButtonBehavior | undefined;

  for (let i = 4; i < parts.length; i++) {
    const part = parts[i];
    if (part === undefined) {
      continue;
    }

    // Channel: chN or chNN
    const channelMatch = part.match(/^ch(\d+)$/i);
    if (channelMatch) {
      const channelStr = channelMatch[1];
      if (channelStr !== undefined) {
        channel = parseInt(channelStr, 10);
        validateChannelRange(channel, lineNumber, line);
      }
      continue;
    }

    // Behavior: toggle or momentary
    const partLower = part.toLowerCase();
    if (isValidBehavior(partLower)) {
      behavior = partLower;
      continue;
    }

    // Unknown token
    throw new ParseError(
      `Unknown token: ${part}. Expected chN (channel) or toggle/momentary (behavior)`,
      lineNumber,
      line
    );
  }

  // Validate control type based on section and device
  const sectionName = currentSection.sectionName;
  if (isLegacyNk2) {
    // Use legacy validation for nK2 configs (preserves existing error messages exactly)
    if (currentSection.type === 'track') {
      if (!isValidTrackControlType(controlType)) {
        throw new ParseError(
          `Invalid track control type: ${controlType}. Expected one of: ${TRACK_CONTROL_TYPES.join(', ')}`,
          lineNumber,
          line
        );
      }
    } else if (currentSection.type === 'transport') {
      if (!isValidTransportControlType(controlType)) {
        throw new ParseError(
          `Invalid transport control type: ${controlType}. Expected one of: ${TRANSPORT_CONTROL_TYPES.join(', ')}`,
          lineNumber,
          line
        );
      }
    }
  } else if (device) {
    // Device-aware validation
    if (currentSection.type === 'track') {
      const validTypes = getDeviceTrackControlTypes(device);
      if (!validTypes.includes(controlType)) {
        throw new ParseError(
          `Invalid track control type: ${controlType}. Expected one of: ${validTypes.join(', ')}`,
          lineNumber,
          line
        );
      }
    } else if (currentSection.type === 'transport') {
      const validTypes = getDeviceExtraSectionControlTypes(device, 'transport');
      if (!validTypes.includes(controlType)) {
        throw new ParseError(
          `Invalid transport control type: ${controlType}. Expected one of: ${validTypes.join(', ')}`,
          lineNumber,
          line
        );
      }
    } else if (currentSection.type === 'extra' && sectionName) {
      const validTypes = getDeviceExtraSectionControlTypes(device, sectionName);
      if (!validTypes.includes(controlType)) {
        throw new ParseError(
          `Invalid ${sectionName} control type: ${controlType}. Expected one of: ${validTypes.join(', ')}`,
          lineNumber,
          line
        );
      }
    }
  }

  // Determine if the control is continuous
  const isContinuous = isDeviceContinuousControl(controlType, device, sectionName);

  // Apply default behavior for buttons if not specified
  if (!isContinuous && behavior === undefined) {
    behavior = getDeviceDefaultBehavior(controlType, device, sectionName);
  }

  // Continuous controls should not have behavior
  if (isContinuous && behavior !== undefined) {
    throw new ParseError(
      `Behavior (${behavior}) is not valid for continuous control type: ${controlType}`,
      lineNumber,
      line
    );
  }

  // Determine the effective messageType from the parsed MIDI numbers.
  // If either input or output used note: prefix, the entry uses 'note'.
  // For legacy nK2 configs (no note: prefix), we omit messageType to preserve backward compat.
  const hasNotePrefix = inputMidi.messageType === 'note' || outputMidi.messageType === 'note';

  const entry: MappingEntry = {
    inputCC: inputMidi.value,
    outputCC: outputMidi.value,
    channel,
  };

  if (behavior !== undefined) {
    entry.behavior = behavior;
  }

  if (label !== undefined) {
    entry.label = label;
  }

  // Set messageType when note: syntax is used, or when the device definition
  // indicates this control uses Note messages. Omitted for plain CC to preserve
  // byte-identical backward compatibility with nK2 configs.
  if (hasNotePrefix) {
    entry.messageType = 'note';
  } else if (!isLegacyNk2 && device) {
    // For non-legacy configs, check the device definition for the expected message type
    let controlDef: ControlDef | undefined;
    if (sectionName) {
      controlDef = findExtraSectionControlDef(device, sectionName, controlType);
    } else {
      controlDef = findTrackControlDef(device, controlType);
    }
    if (controlDef && controlDef.messageType === 'note') {
      entry.messageType = 'note';
    } else if (controlDef && controlDef.messageType === 'cc') {
      // Explicitly set 'cc' for device-aware configs to be unambiguous
      entry.messageType = 'cc';
    }
  }

  return { controlType, entry };
}

// =============================================================================
// Config Building
// =============================================================================

/**
 * Creates an empty partial track mapping.
 */
function createEmptyPartialTrack(): PartialTrackMapping {
  return {};
}

/**
 * Validates that a partial track mapping has all required controls.
 */
function validateTrackMapping(
  partial: PartialTrackMapping,
  trackNumber: number
): TrackMapping {
  const missing: string[] = [];

  if (!partial.knob) missing.push('knob');
  if (!partial.slider) missing.push('slider');
  if (!partial.solo) missing.push('solo');
  if (!partial.mute) missing.push('mute');
  if (!partial.rec) missing.push('rec');

  if (missing.length > 0) {
    throw new Error(
      `Track ${trackNumber} is missing required controls: ${missing.join(', ')}`
    );
  }

  // TypeScript requires explicit checks even after the above validation
  if (!partial.knob || !partial.slider || !partial.solo || !partial.mute || !partial.rec) {
    throw new Error(`Track ${trackNumber} validation failed unexpectedly`);
  }

  return {
    knob: partial.knob,
    slider: partial.slider,
    solo: partial.solo,
    mute: partial.mute,
    rec: partial.rec,
  };
}

/**
 * Validates that a partial transport mapping has all required controls.
 */
function validateTransportMapping(partial: PartialTransportMapping): TransportMapping {
  const missing: string[] = [];

  if (!partial.rewind) missing.push('rewind');
  if (!partial.forward) missing.push('forward');
  if (!partial.stop) missing.push('stop');
  if (!partial.play) missing.push('play');
  if (!partial.record) missing.push('record');
  if (!partial.cycle) missing.push('cycle');
  if (!partial.track_left) missing.push('track_left');
  if (!partial.track_right) missing.push('track_right');
  if (!partial.marker_set) missing.push('marker_set');
  if (!partial.marker_left) missing.push('marker_left');
  if (!partial.marker_right) missing.push('marker_right');

  if (missing.length > 0) {
    throw new Error(
      `Transport section is missing required controls: ${missing.join(', ')}`
    );
  }

  // TypeScript requires explicit checks even after the above validation
  if (
    !partial.rewind ||
    !partial.forward ||
    !partial.stop ||
    !partial.play ||
    !partial.record ||
    !partial.cycle ||
    !partial.track_left ||
    !partial.track_right ||
    !partial.marker_set ||
    !partial.marker_left ||
    !partial.marker_right
  ) {
    throw new Error('Transport validation failed unexpectedly');
  }

  return {
    rewind: partial.rewind,
    forward: partial.forward,
    stop: partial.stop,
    play: partial.play,
    record: partial.record,
    cycle: partial.cycle,
    track_left: partial.track_left,
    track_right: partial.track_right,
    marker_set: partial.marker_set,
    marker_left: partial.marker_left,
    marker_right: partial.marker_right,
  };
}

// =============================================================================
// Main Parser
// =============================================================================

/**
 * Parses a mapping configuration file content and returns a ParseResult.
 *
 * Supports multi-device configs via the optional [device <id>] header.
 * When no device header is present, defaults to nanokontrol2 and uses
 * the legacy nK2-specific parsing path for exact backward compatibility.
 *
 * @param content - The raw string content of the mapping file
 * @returns ParseResult indicating success with config and deviceId, or failure with error message
 *
 * @example
 * ```typescript
 * const content = fs.readFileSync('mappings/default.txt', 'utf-8');
 * const result = parseMapping(content);
 * if (result.success) {
 *   console.log('Device:', result.deviceId);
 *   console.log('Loaded', result.config.tracks.length, 'tracks');
 * } else {
 *   console.error('Parse error:', result.error);
 * }
 * ```
 */
export function parseMapping(content: string): ParseResult {
  try {
    const lines = content.split('\n');

    // Scan for device header before main parsing
    const deviceHeader = scanForDeviceHeader(lines);
    const deviceId = deviceHeader ? deviceHeader.deviceId : DEFAULT_DEVICE_ID;

    // Load device definition when an explicit header is present
    let device: DeviceDefinition | null = null;
    if (deviceHeader) {
      const foundDevice = getDeviceDefinition(deviceId);
      if (!foundDevice) {
        return {
          success: false,
          error: `Unknown device: '${deviceId}'. No device definition found for this identifier`,
        };
      }
      device = foundDevice;
    }

    // When no [device] header is present and the default is nK2,
    // use the legacy code path for byte-identical backward compatibility
    const isLegacyNk2 = !deviceHeader;

    // For non-nK2 devices, we still use the nK2 MappingConfig shape.
    // The device definition guides validation, but the output is always
    // MappingConfig (tracks[] + transport) since that is the shared type.
    // Future phases will introduce a more flexible DeviceMappingConfig.

    // Initialize tracking structures
    const trackCount = device ? device.trackCount : TRACK_CONSTANTS.COUNT;
    const tracks: Map<number, PartialTrackMapping> = new Map();
    for (let i = 1; i <= trackCount; i++) {
      tracks.set(i, createEmptyPartialTrack());
    }

    // For device-aware configs, use a generic record for track data
    // to support control names beyond the nK2 set
    const deviceTracks: Map<number, Record<string, MappingEntry>> = new Map();
    if (!isLegacyNk2) {
      for (let i = 1; i <= trackCount; i++) {
        deviceTracks.set(i, {});
      }
    }

    const transport: PartialTransportMapping = {};
    // Generic storage for extra section controls (e.g., master)
    const extraSections: Map<string, Record<string, MappingEntry>> = new Map();
    let currentSection: CurrentSection | null = null;
    let hasTransportSection = false;
    const definedTracks = new Set<number>();
    // Track which extra sections have been defined
    const definedExtraSections = new Set<string>();

    // Parse each line
    for (let i = 0; i < lines.length; i++) {
      const lineNumber = i + 1; // 1-indexed for error messages
      const line = lines[i];

      // Handle undefined line (should not happen, but TypeScript requires check)
      if (line === undefined) {
        continue;
      }

      const trimmed = line.trim();

      // Skip empty lines and comments
      if (trimmed === '' || trimmed.startsWith('#')) {
        continue;
      }

      // Skip [device ...] header lines (already processed above)
      if (/^\[device\s+/i.test(trimmed)) {
        continue;
      }

      // Check for section header
      const sectionHeader = parseSectionHeader(trimmed, lineNumber, device);
      if (sectionHeader) {
        currentSection = sectionHeader;
        if (sectionHeader.type === 'transport') {
          hasTransportSection = true;
        } else if (sectionHeader.type === 'track' && sectionHeader.trackNumber !== undefined) {
          definedTracks.add(sectionHeader.trackNumber);
        } else if (sectionHeader.type === 'extra' && sectionHeader.sectionName) {
          definedExtraSections.add(sectionHeader.sectionName);
          if (!extraSections.has(sectionHeader.sectionName)) {
            extraSections.set(sectionHeader.sectionName, {});
          }
        }
        continue;
      }

      // Parse control mapping line
      const { controlType, entry } = parseControlLine(line, lineNumber, currentSection, device, isLegacyNk2);

      if (currentSection === null) {
        // This should not happen due to check in parseControlLine, but TypeScript needs it
        throw new ParseError('Internal error: no current section', lineNumber, line);
      }

      // Store the mapping in the appropriate structure
      if (currentSection.type === 'track' && currentSection.trackNumber !== undefined) {
        if (isLegacyNk2) {
          const trackMapping = tracks.get(currentSection.trackNumber);
          if (!trackMapping) {
            throw new ParseError(
              `Internal error: track ${currentSection.trackNumber} not initialized`,
              lineNumber,
              line
            );
          }

          // Type-safe assignment for track controls
          const trackControl = controlType as ControlType;

          // Check for duplicate control definitions
          if (trackMapping[trackControl] !== undefined) {
            throw new ParseError(
              `Duplicate ${controlType} definition in track${currentSection.trackNumber}`,
              lineNumber,
              line
            );
          }

          trackMapping[trackControl] = entry;
        } else {
          // Device-aware track storage
          const trackData = deviceTracks.get(currentSection.trackNumber);
          if (!trackData) {
            throw new ParseError(
              `Internal error: track ${currentSection.trackNumber} not initialized`,
              lineNumber,
              line
            );
          }

          if (trackData[controlType] !== undefined) {
            throw new ParseError(
              `Duplicate ${controlType} definition in track${currentSection.trackNumber}`,
              lineNumber,
              line
            );
          }

          trackData[controlType] = entry;
        }
      } else if (currentSection.type === 'transport') {
        if (isLegacyNk2) {
          // Type-safe assignment for transport controls
          const transportControl = controlType as TransportControlType;

          // Check for duplicate control definitions
          if (transport[transportControl] !== undefined) {
            throw new ParseError(
              `Duplicate ${controlType} definition in transport section`,
              lineNumber,
              line
            );
          }

          transport[transportControl] = entry;
        } else {
          // Device-aware transport storage
          const transportSection = extraSections.get('transport') ?? {};
          if (transportSection[controlType] !== undefined) {
            throw new ParseError(
              `Duplicate ${controlType} definition in transport section`,
              lineNumber,
              line
            );
          }
          transportSection[controlType] = entry;
          extraSections.set('transport', transportSection);
        }
      } else if (currentSection.type === 'extra' && currentSection.sectionName) {
        const sectionData = extraSections.get(currentSection.sectionName);
        if (!sectionData) {
          throw new ParseError(
            `Internal error: section ${currentSection.sectionName} not initialized`,
            lineNumber,
            line
          );
        }

        if (sectionData[controlType] !== undefined) {
          throw new ParseError(
            `Duplicate ${controlType} definition in ${currentSection.sectionName} section`,
            lineNumber,
            line
          );
        }

        sectionData[controlType] = entry;
      }
    }

    // === Legacy nK2 validation path ===
    if (isLegacyNk2) {
      // Validate all tracks are defined
      for (let i = 1; i <= TRACK_CONSTANTS.COUNT; i++) {
        if (!definedTracks.has(i)) {
          return {
            success: false,
            error: `Track ${i} is not defined. All 8 tracks (track1-track8) must be present`,
          };
        }
      }

      // Validate transport section is defined
      if (!hasTransportSection) {
        return {
          success: false,
          error: 'Transport section is not defined. A [transport] section is required',
        };
      }

      // Validate and build final track mappings
      const finalTracks: TrackMapping[] = [];
      for (let i = 1; i <= TRACK_CONSTANTS.COUNT; i++) {
        const partial = tracks.get(i);
        if (!partial) {
          return {
            success: false,
            error: `Internal error: track ${i} not found`,
          };
        }

        try {
          finalTracks.push(validateTrackMapping(partial, i));
        } catch (err) {
          return {
            success: false,
            error: err instanceof Error ? err.message : String(err),
          };
        }
      }

      // Validate and build final transport mapping
      let finalTransport: TransportMapping;
      try {
        finalTransport = validateTransportMapping(transport);
      } catch (err) {
        return {
          success: false,
          error: err instanceof Error ? err.message : String(err),
        };
      }

      return {
        success: true,
        config: {
          tracks: finalTracks,
          transport: finalTransport,
        },
        deviceId,
      };
    }

    // === Device-aware validation path ===
    // Build MappingConfig from device track data.
    // We map device control names to the nK2 MappingConfig shape where possible,
    // storing extra controls in a best-fit manner.
    const finalTracks: TrackMapping[] = [];
    for (let i = 1; i <= trackCount; i++) {
      const trackData = deviceTracks.get(i);
      if (!trackData || !definedTracks.has(i)) {
        // Not all tracks need to be present for non-nK2 devices at this stage;
        // build a default entry for missing tracks
        continue;
      }

      // Map device control names to nK2 TrackMapping fields.
      // This is a best-effort mapping for the shared MappingConfig type.
      const mappedTrack: PartialTrackMapping = {};

      // Map continuous controls: knob/knob1 -> knob, slider/fader -> slider
      if (trackData['knob']) {
        mappedTrack.knob = trackData['knob'];
      } else if (trackData['knob1']) {
        mappedTrack.knob = trackData['knob1'];
      }

      if (trackData['slider']) {
        mappedTrack.slider = trackData['slider'];
      } else if (trackData['fader']) {
        mappedTrack.slider = trackData['fader'];
      }

      // Map button controls
      if (trackData['solo']) {
        mappedTrack.solo = trackData['solo'];
      }
      if (trackData['mute']) {
        mappedTrack.mute = trackData['mute'];
      }
      if (trackData['rec']) {
        mappedTrack.rec = trackData['rec'];
      } else if (trackData['rec_arm']) {
        mappedTrack.rec = trackData['rec_arm'];
      }

      // Only require the controls that were provided; partial tracks are allowed
      // for device-aware configs since devices have different control sets
      finalTracks.push({
        knob: mappedTrack.knob ?? { inputCC: 0, outputCC: 0, channel: 1 },
        slider: mappedTrack.slider ?? { inputCC: 0, outputCC: 0, channel: 1 },
        solo: mappedTrack.solo ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
        mute: mappedTrack.mute ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
        rec: mappedTrack.rec ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      });
    }

    // Build transport from extra sections or the transport section
    const transportData = extraSections.get('transport');
    const finalTransport: TransportMapping = {
      rewind: transportData?.['rewind'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      forward: transportData?.['forward'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      stop: transportData?.['stop'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      play: transportData?.['play'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      record: transportData?.['record'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      cycle: transportData?.['cycle'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
      track_left: transportData?.['track_left'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      track_right: transportData?.['track_right'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      marker_set: transportData?.['marker_set'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      marker_left: transportData?.['marker_left'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      marker_right: transportData?.['marker_right'] ?? { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
    };

    // Pad tracks array to 8 if needed (MappingConfig requires exactly 8)
    while (finalTracks.length < 8) {
      finalTracks.push({
        knob: { inputCC: 0, outputCC: 0, channel: 1 },
        slider: { inputCC: 0, outputCC: 0, channel: 1 },
        solo: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
        mute: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
        rec: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      });
    }

    return {
      success: true,
      config: {
        tracks: finalTracks,
        transport: finalTransport,
      },
      deviceId,
    };
  } catch (err) {
    if (err instanceof ParseError) {
      return {
        success: false,
        error: err.message,
      };
    }
    return {
      success: false,
      error: err instanceof Error ? err.message : String(err),
    };
  }
}

/**
 * Convenience function that parses and throws on error.
 * Use this when you want exceptions rather than result objects.
 */
export function parseMappingOrThrow(content: string): MappingConfig {
  const result = parseMapping(content);
  if (result.success) {
    return result.config;
  }
  throw new Error(result.error);
}
