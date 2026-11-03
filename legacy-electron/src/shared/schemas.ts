/**
 * Zod Schemas for Validation
 *
 * Provides runtime validation for presets, configurations, and IPC messages.
 */

import { z } from 'zod';
import type { DeviceMappingConfig, MappingEntry as SharedMappingEntry } from './types.js';
import { getDeviceDefinition } from './devices/registry.js';
import type { DeviceDefinition, ControlDef } from './devices/device-definition.js';
import { NORD_DRUM_2_CC, NORD_DRUM_2_FX_CC, NORD_DRUM_2_CHANNELS } from './nord-drum-2-cc.js';

// =============================================================================
// MIDI Value Schemas
// =============================================================================

/** Valid CC number (0-127) */
export const ccNumberSchema = z.number().int().min(0).max(127);

/** Valid MIDI channel (1-16, user-facing) */
export const midiChannelSchema = z.number().int().min(1).max(16);

/** Valid MIDI value (0-127) */
export const midiValueSchema = z.number().int().min(0).max(127);

// =============================================================================
// Mapping Schemas
// =============================================================================

/** Button behavior: toggle or momentary */
export const buttonBehaviorSchema = z.enum(['toggle', 'momentary']);

/** MIDI message type: 'cc' for Control Change, 'note' for Note On/Off */
export const midiMessageTypeSchema = z.enum(['cc', 'note']);

/** Single mapping entry */
export const mappingEntrySchema = z.object({
  inputCC: ccNumberSchema,
  outputCC: ccNumberSchema,
  channel: midiChannelSchema,
  behavior: buttonBehaviorSchema.optional(),
  label: z.string().max(50).optional(),
  /** Minimum output value for continuous controls (0-127, default 0) */
  minValue: z.number().int().min(0).max(127).optional(),
  /** Maximum output value for continuous controls (0-127, default 127) */
  maxValue: z.number().int().min(0).max(127).optional(),
  /** CC value sent when button is ON (0-127, default 127) */
  onValue: z.number().int().min(0).max(127).optional(),
  /** CC value sent when button is OFF (0-127, default 0) */
  offValue: z.number().int().min(0).max(127).optional(),
  /** MIDI message type ('cc' or 'note'). Defaults to 'cc' if omitted. */
  messageType: midiMessageTypeSchema.optional(),
});

/** Track mapping (5 controls) */
export const trackMappingSchema = z.object({
  knob: mappingEntrySchema,
  slider: mappingEntrySchema,
  solo: mappingEntrySchema,
  mute: mappingEntrySchema,
  rec: mappingEntrySchema,
});

/** Transport mapping (11 controls) */
export const transportMappingSchema = z.object({
  rewind: mappingEntrySchema,
  forward: mappingEntrySchema,
  stop: mappingEntrySchema,
  play: mappingEntrySchema,
  record: mappingEntrySchema,
  cycle: mappingEntrySchema,
  track_left: mappingEntrySchema,
  track_right: mappingEntrySchema,
  marker_set: mappingEntrySchema,
  marker_left: mappingEntrySchema,
  marker_right: mappingEntrySchema,
});

/** Complete mapping configuration */
export const mappingConfigSchema = z.object({
  tracks: z.array(trackMappingSchema).length(8),
  transport: transportMappingSchema,
});

/** DeviceMappingConfig: generic mapping for non-nK2 devices */
export const deviceMappingConfigSchema = z.object({
  deviceId: z.string().min(1),
  tracks: z.record(z.string(), z.record(z.string(), mappingEntrySchema)),
  sections: z.record(z.string(), z.record(z.string(), mappingEntrySchema)),
});

// =============================================================================
// Control Values Schemas (for preset state storage)
// =============================================================================

/** Track control values */
export const trackControlValuesSchema = z.object({
  knob: midiValueSchema,
  knobLabel: z.string().max(50).optional(),
  slider: midiValueSchema,
  sliderLabel: z.string().max(50).optional(),
  solo: z.boolean(),
  soloLabel: z.string().max(50).optional(),
  mute: z.boolean(),
  muteLabel: z.string().max(50).optional(),
  rec: z.boolean(),
  recLabel: z.string().max(50).optional(),
});

/** Transport control values (all button states) */
export const transportControlValuesSchema = z.object({
  play: z.boolean(),
  playLabel: z.string().max(50).optional(),
  stop: z.boolean(),
  stopLabel: z.string().max(50).optional(),
  rewind: z.boolean(),
  rewindLabel: z.string().max(50).optional(),
  forward: z.boolean(),
  forwardLabel: z.string().max(50).optional(),
  record: z.boolean(),
  recordLabel: z.string().max(50).optional(),
  cycle: z.boolean(),
  cycleLabel: z.string().max(50).optional(),
  track_left: z.boolean(),
  track_leftLabel: z.string().max(50).optional(),
  track_right: z.boolean(),
  track_rightLabel: z.string().max(50).optional(),
  marker_set: z.boolean(),
  marker_setLabel: z.string().max(50).optional(),
  marker_left: z.boolean(),
  marker_leftLabel: z.string().max(50).optional(),
  marker_right: z.boolean(),
  marker_rightLabel: z.string().max(50).optional(),
});

/** Complete control values for a preset */
export const controlValuesSchema = z.object({
  tracks: z.array(trackControlValuesSchema).length(8),
  transport: transportControlValuesSchema,
});

// =============================================================================
// Preset Schemas
// =============================================================================

/** Preset metadata */
export const presetMetadataSchema = z.object({
  id: z.string().min(1).max(100),
  name: z.string().min(1).max(100),
  description: z.string().max(500).optional(),
  author: z.string().max(100).optional(),
  /** Group/folder for organizing presets (max 50 characters) */
  group: z.string().max(50).optional(),
  createdAt: z.string().datetime(),
  modifiedAt: z.string().datetime(),
  tags: z.array(z.string().max(30)).max(10),
  favorite: z.boolean(),
});

/** Full preset (metadata + mapping + optional control values) */
export const presetSchema = z.object({
  metadata: presetMetadataSchema,
  mapping: z.union([mappingConfigSchema, deviceMappingConfigSchema]),
  controlValues: controlValuesSchema.optional(),
});

// =============================================================================
// Configuration Schemas
// =============================================================================

/** Soft takeover mode */
export const softTakeoverModeSchema = z.enum(['catch', 'jump', 'pickup']);

/** Theme setting */
export const themeSchema = z.enum(['light', 'dark', 'system']);

/** Value display format */
export const valueDisplaySchema = z.enum(['decimal', 'hex']);

/** Window bounds */
export const windowBoundsSchema = z.object({
  x: z.number().int(),
  y: z.number().int(),
  width: z.number().int().min(800),
  height: z.number().int().min(600),
});

/** LED mode */
export const ledModeSchema = z.enum(['internal', 'external']);

/** Application configuration */
/** Schema for custom theme colors */
export const themeColorsSchema = z.object({
  background: z.string(),
  backgroundDarker: z.string(),
  surface: z.string(),
  border: z.string(),
  accent: z.string(),
  solo: z.string(),
  mute: z.string(),
  rec: z.string(),
});

/** Schema for a saved user theme */
export const userThemeSchema = z.object({
  name: z.string().max(30),
  colors: themeColorsSchema,
});

export const appConfigSchema = z.object({
  softTakeoverMode: softTakeoverModeSchema,
  softTakeoverThreshold: z.number().int().min(1).max(20),
  theme: themeSchema,
  valueDisplay: valueDisplaySchema,
  autoConnect: z.boolean(),
  transmitButtonsOnLoad: z.boolean(),
  globalMidiChannel: z.number().int().min(1).max(16),
  ledMode: ledModeSchema,
  recentPresets: z.array(z.string()).max(10),
  quickAccessSlots: z.array(z.string().nullable()).length(5),
  windowBounds: windowBoundsSchema.optional(),
  customThemeColors: themeColorsSchema.optional(),
  userThemes: z.array(userThemeSchema).optional(),
  uiScale: z.number().int().min(50).max(200).optional(),
  fontFamily: z.string().max(50).optional(),
  fontSize: z.number().int().min(10).max(24).optional(),
});

// =============================================================================
// IPC Message Schemas
// =============================================================================

/** CC message from MIDI */
export const ccMessageSchema = z.object({
  channel: z.number().int().min(0).max(15),
  controller: ccNumberSchema,
  value: midiValueSchema,
});

/** Send CC request */
export const sendCCRequestSchema = z.object({
  channel: z.number().int().min(0).max(15),
  cc: ccNumberSchema,
  value: midiValueSchema,
});

/** Connect request */
export const connectRequestSchema = z.object({
  inputPort: z.string().optional(),
  outputPort: z.string().optional(),
});

/** Save preset request */
export const savePresetRequestSchema = z.object({
  preset: presetSchema,
  overwrite: z.boolean().optional(),
});

/** Load preset request */
export const loadPresetRequestSchema = z.object({
  id: z.string().min(1),
});

/** Delete preset request */
export const deletePresetRequestSchema = z.object({
  id: z.string().min(1),
});

/** Update config request */
export const updateConfigRequestSchema = z.object({
  updates: appConfigSchema.partial(),
});

// =============================================================================
// Type Exports (inferred from schemas)
// =============================================================================

export type MappingEntry = z.infer<typeof mappingEntrySchema>;
export type TrackMapping = z.infer<typeof trackMappingSchema>;
export type TransportMapping = z.infer<typeof transportMappingSchema>;
export type MappingConfig = z.infer<typeof mappingConfigSchema>;
export type DeviceMappingConfigSchema = z.infer<typeof deviceMappingConfigSchema>;
export type TrackControlValues = z.infer<typeof trackControlValuesSchema>;
export type TransportControlValues = z.infer<typeof transportControlValuesSchema>;
export type ControlValues = z.infer<typeof controlValuesSchema>;
export type PresetMetadata = z.infer<typeof presetMetadataSchema>;
export type Preset = z.infer<typeof presetSchema>;
export type AppConfig = z.infer<typeof appConfigSchema>;
export type CCMessage = z.infer<typeof ccMessageSchema>;

// =============================================================================
// Validation Helpers
// =============================================================================

/**
 * Validates a preset and returns typed result.
 * Throws ZodError if validation fails.
 */
export function validatePreset(data: unknown): Preset {
  return presetSchema.parse(data);
}

/**
 * Validates a mapping configuration.
 * Throws ZodError if validation fails.
 */
export function validateMappingConfig(data: unknown): MappingConfig {
  return mappingConfigSchema.parse(data);
}

/**
 * Validates app configuration.
 * Throws ZodError if validation fails.
 */
export function validateAppConfig(data: unknown): AppConfig {
  return appConfigSchema.parse(data);
}

/**
 * Safe validation that returns success/error result.
 */
export function safeValidatePreset(data: unknown): { success: true; data: Preset } | { success: false; error: string } {
  const result = presetSchema.safeParse(data);
  if (result.success) {
    return { success: true, data: result.data };
  }
  return { success: false, error: result.error.message };
}

/**
 * Type guard: checks if a mapping is a DeviceMappingConfig (has deviceId).
 */
export function isDeviceMappingConfig(
  mapping: MappingConfig | z.infer<typeof deviceMappingConfigSchema>
): mapping is z.infer<typeof deviceMappingConfigSchema> {
  return 'deviceId' in mapping && typeof (mapping as Record<string, unknown>).deviceId === 'string';
}

/**
 * Type guard: checks if a mapping is a standard MappingConfig (tracks is an array).
 */
export function isMappingConfig(
  mapping: MappingConfig | z.infer<typeof deviceMappingConfigSchema>
): mapping is MappingConfig {
  return Array.isArray((mapping as Record<string, unknown>).tracks);
}

/**
 * Creates default app configuration.
 */
export function createDefaultAppConfig(): AppConfig {
  return {
    softTakeoverMode: 'catch',
    softTakeoverThreshold: 3,
    theme: 'dark',
    valueDisplay: 'decimal',
    autoConnect: true,
    transmitButtonsOnLoad: true,
    globalMidiChannel: 1,
    ledMode: 'internal',
    recentPresets: [],
    quickAccessSlots: [null, null, null, null, null],
  };
}

/**
 * Creates default preset metadata.
 * @param name - The name of the preset
 * @param group - Optional group/folder for the preset
 */
export function createDefaultPresetMetadata(name: string, group?: string): PresetMetadata {
  const now = new Date().toISOString();
  const metadata: PresetMetadata = {
    id: generatePresetId(),
    name,
    createdAt: now,
    modifiedAt: now,
    tags: [],
    favorite: false,
  };

  // Only include group if provided to avoid undefined in JSON
  if (group) {
    metadata.group = group;
  }

  return metadata;
}

/**
 * Generates a unique preset ID.
 */
function generatePresetId(): string {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).substring(2, 8);
  return `preset_${timestamp}_${random}`;
}

/**
 * Creates default track control values (all at zero/off).
 */
export function createDefaultTrackControlValues(): TrackControlValues {
  return {
    knob: 0,
    slider: 0,
    solo: false,
    mute: false,
    rec: false,
  };
}

/**
 * Creates default transport control values (all buttons off).
 */
export function createDefaultTransportControlValues(): TransportControlValues {
  return {
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
  };
}

/**
 * Creates default control values for all tracks and transport.
 * All continuous controls are set to 0, all buttons are off.
 */
export function createDefaultControlValues(): ControlValues {
  return {
    tracks: Array.from({ length: 8 }, () => createDefaultTrackControlValues()),
    transport: createDefaultTransportControlValues(),
  };
}

/**
 * Creates the default (identity) mapping configuration for nanoKONTROL2.
 * Input CCs match output CCs - a pass-through configuration.
 */
export function createDefaultMappingConfig(): MappingConfig {
  // Hardware CC assignments from constants (as const for tuple typing)
  const KNOBS = [16, 17, 18, 19, 20, 21, 22, 23] as const;
  const SLIDERS = [0, 1, 2, 3, 4, 5, 6, 7] as const;
  const SOLO = [32, 33, 34, 35, 36, 37, 38, 39] as const;
  const MUTE = [48, 49, 50, 51, 52, 53, 54, 55] as const;
  const REC = [64, 65, 66, 67, 68, 69, 70, 71] as const;
  const TRANSPORT = {
    PLAY: 41,
    STOP: 42,
    REWIND: 43,
    FORWARD: 44,
    RECORD: 45,
    CYCLE: 46,
    TRACK_LEFT: 58,
    TRACK_RIGHT: 59,
    MARKER_SET: 60,
    MARKER_LEFT: 61,
    MARKER_RIGHT: 62,
  } as const;

  const tracks: TrackMapping[] = [];

  for (let i = 0; i < 8; i++) {
    // These indices are guaranteed to be valid (0-7)
    const knobCC = KNOBS[i as 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7];
    const sliderCC = SLIDERS[i as 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7];
    const soloCC = SOLO[i as 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7];
    const muteCC = MUTE[i as 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7];
    const recCC = REC[i as 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7];

    tracks.push({
      knob: { inputCC: knobCC, outputCC: knobCC, channel: 1 },
      slider: { inputCC: sliderCC, outputCC: sliderCC, channel: 1 },
      solo: { inputCC: soloCC, outputCC: soloCC, channel: 1, behavior: 'toggle' },
      mute: { inputCC: muteCC, outputCC: muteCC, channel: 1, behavior: 'toggle' },
      rec: { inputCC: recCC, outputCC: recCC, channel: 1, behavior: 'momentary' },
    });
  }

  const transport: TransportMapping = {
    rewind: { inputCC: TRANSPORT.REWIND, outputCC: TRANSPORT.REWIND, channel: 1, behavior: 'momentary' },
    forward: { inputCC: TRANSPORT.FORWARD, outputCC: TRANSPORT.FORWARD, channel: 1, behavior: 'momentary' },
    stop: { inputCC: TRANSPORT.STOP, outputCC: TRANSPORT.STOP, channel: 1, behavior: 'momentary' },
    play: { inputCC: TRANSPORT.PLAY, outputCC: TRANSPORT.PLAY, channel: 1, behavior: 'momentary' },
    record: { inputCC: TRANSPORT.RECORD, outputCC: TRANSPORT.RECORD, channel: 1, behavior: 'momentary' },
    cycle: { inputCC: TRANSPORT.CYCLE, outputCC: TRANSPORT.CYCLE, channel: 1, behavior: 'toggle' },
    track_left: { inputCC: TRANSPORT.TRACK_LEFT, outputCC: TRANSPORT.TRACK_LEFT, channel: 1, behavior: 'momentary' },
    track_right: { inputCC: TRANSPORT.TRACK_RIGHT, outputCC: TRANSPORT.TRACK_RIGHT, channel: 1, behavior: 'momentary' },
    marker_set: { inputCC: TRANSPORT.MARKER_SET, outputCC: TRANSPORT.MARKER_SET, channel: 1, behavior: 'momentary' },
    marker_left: { inputCC: TRANSPORT.MARKER_LEFT, outputCC: TRANSPORT.MARKER_LEFT, channel: 1, behavior: 'momentary' },
    marker_right: { inputCC: TRANSPORT.MARKER_RIGHT, outputCC: TRANSPORT.MARKER_RIGHT, channel: 1, behavior: 'momentary' },
  };

  return { tracks, transport };
}

// =============================================================================
// Device-Aware Default Mapping Factories
// =============================================================================

/**
 * Creates an identity mapping for any registered device.
 *
 * For 'nanokontrol2': returns the standard MappingConfig shape (backward compat).
 * For other devices: returns a DeviceMappingConfig with control names and
 * message types derived from the device definition.
 *
 * @param deviceId - Registered device identifier (e.g., 'nanokontrol2', 'akai-midimix')
 * @returns A MappingConfig (nK2) or DeviceMappingConfig (other devices)
 * @throws Error if the deviceId is not found in the device registry
 */
export function createDefaultMappingForDevice(deviceId: string): MappingConfig | DeviceMappingConfig {
  if (deviceId === 'nanokontrol2') {
    return createDefaultMappingConfig();
  }

  const device = getDeviceDefinition(deviceId);
  if (!device) {
    throw new Error(
      `Unknown device: '${deviceId}'. Cannot create default mapping for an unregistered device.`
    );
  }

  return buildDeviceMappingConfig(device);
}

/**
 * Builds a DeviceMappingConfig from a device definition using identity mapping.
 * Every control's outputCC equals its inputCC (hardware MIDI number).
 */
function buildDeviceMappingConfig(device: DeviceDefinition): DeviceMappingConfig {
  const tracks: Record<string, Record<string, SharedMappingEntry>> = {};

  for (let trackIndex = 0; trackIndex < device.trackCount; trackIndex++) {
    const trackNumber = String(trackIndex + 1);
    const layout = device.trackLayouts[trackIndex];
    if (!layout) {
      continue;
    }

    const trackEntries: Record<string, SharedMappingEntry> = {};
    const allControls: ControlDef[] = [...layout.continuousControls, ...layout.buttonControls];

    for (const control of allControls) {
      const entry: SharedMappingEntry = {
        inputCC: control.midiNumber,
        outputCC: control.midiNumber,
        channel: control.channel,
        messageType: control.messageType,
      };

      if (control.kind === 'button') {
        entry.behavior = control.defaultBehavior;
      }

      trackEntries[control.name] = entry;
    }

    tracks[trackNumber] = trackEntries;
  }

  const sections: Record<string, Record<string, SharedMappingEntry>> = {};

  for (const section of device.extraSections) {
    const sectionEntries: Record<string, SharedMappingEntry> = {};

    for (const control of section.controls) {
      const entry: SharedMappingEntry = {
        inputCC: control.midiNumber,
        outputCC: control.midiNumber,
        channel: control.channel,
        messageType: control.messageType,
      };

      if (control.kind === 'button') {
        entry.behavior = control.defaultBehavior;
      }

      sectionEntries[control.name] = entry;
    }

    sections[section.name] = sectionEntries;
  }

  return {
    deviceId: device.id,
    tracks,
    sections,
  };
}

/**
 * Creates a new preset with default (identity) mapping and default control values.
 * @param name - The name of the preset
 * @param description - Optional description of the preset
 * @param group - Optional group/folder for the preset
 */
export function createDefaultPreset(name: string, description?: string, group?: string): Preset {
  const metadata = createDefaultPresetMetadata(name, group);
  if (description) {
    metadata.description = description;
  }
  return {
    metadata,
    mapping: createDefaultMappingConfig(),
    controlValues: createDefaultControlValues(),
  };
}

// =============================================================================
// Sample Preset Factories
// =============================================================================

/**
 * Valid track indices for nanoKONTROL2 (8 tracks: 0-7).
 */
type TrackIndex = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7;

/**
 * Safely gets a track from the preset, asserting it exists.
 * This helper exists because TypeScript cannot infer that tracks[] always has 8 elements.
 */
function getTrack(preset: Preset, index: TrackIndex): TrackMapping {
  const mapping = preset.mapping;
  if (!Array.isArray(mapping.tracks)) {
    throw new Error('getTrack() requires a MappingConfig preset (array-based tracks), not a DeviceMappingConfig');
  }
  const track = (mapping as MappingConfig).tracks[index];
  if (!track) {
    throw new Error(`Track ${index} not found - preset mapping is malformed`);
  }
  return track;
}

/**
 * Creates the "Default" preset - identity mapping (CC in = CC out).
 * This is a pass-through configuration for users who want unmodified signals.
 */
function createDefaultSamplePreset(): Preset {
  const preset = createDefaultPreset(
    'Default',
    'Identity mapping - input CCs pass through unchanged. Use as a starting point for custom configurations.',
    'Factory'
  );
  preset.metadata.tags = ['factory', 'default', 'identity'];
  return preset;
}

/**
 * Creates the "Synth Lead" preset - mappings optimized for synthesizer control.
 * Knobs map to common synth parameters: filter, resonance, envelope, LFO.
 */
function createSynthLeadPreset(): Preset {
  const preset = createDefaultPreset(
    'Synth Lead',
    'Optimized for synthesizer lead patches. Knobs control filter cutoff, resonance, envelope, and modulation parameters.',
    'Factory'
  );
  preset.metadata.tags = ['factory', 'synth', 'lead', 'performance'];

  // Remap knobs to common synth CC destinations
  const synthKnobCCs = [74, 71, 73, 72, 91, 93, 1, 11] as const; // Filter, Res, Attack, Release, Reverb, Chorus, Mod, Expression
  const synthKnobLabels = ['Filter', 'Resonance', 'Attack', 'Release', 'Reverb', 'Chorus', 'Mod Wheel', 'Expression'] as const;

  const trackIndices: TrackIndex[] = [0, 1, 2, 3, 4, 5, 6, 7];
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.knob.outputCC = synthKnobCCs[i];
    track.knob.label = synthKnobLabels[i];
  }

  // Sliders for oscillator levels and master
  const synthSliderCCs = [16, 17, 18, 19, 20, 21, 22, 7] as const; // OSC levels + Master volume
  const synthSliderLabels = ['OSC 1', 'OSC 2', 'OSC 3', 'Sub', 'Noise', 'Aux 1', 'Aux 2', 'Master'] as const;

  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.slider.outputCC = synthSliderCCs[i];
    track.slider.label = synthSliderLabels[i];
  }

  return preset;
}

/**
 * Creates the "Drums" preset - mappings optimized for drum machines.
 * Knobs control tuning/decay, sliders control levels, buttons trigger/mute.
 */
function createDrumsPreset(): Preset {
  const preset = createDefaultPreset(
    'Drums',
    'Optimized for drum machines and samplers. Sliders control individual drum levels, knobs adjust tuning and decay parameters.',
    'Factory'
  );
  preset.metadata.tags = ['factory', 'drums', 'percussion', 'beats'];

  // Knobs for drum parameter control (pan positions)
  const drumKnobLabels = ['Kick Tune', 'Snare Tune', 'HH Decay', 'Tom Tune', 'Perc Tune', 'Cymbal', 'FX 1', 'FX 2'] as const;
  const drumKnobCCs = [10, 42, 43, 44, 45, 46, 47, 48] as const;

  const trackIndices: TrackIndex[] = [0, 1, 2, 3, 4, 5, 6, 7];
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.knob.outputCC = drumKnobCCs[i];
    track.knob.label = drumKnobLabels[i];
  }

  // Sliders for drum levels (using standard mixer CCs)
  const drumSliderLabels = ['Kick', 'Snare', 'HiHat', 'Tom 1', 'Tom 2', 'Perc', 'FX', 'Master'] as const;

  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.slider.label = drumSliderLabels[i];
  }

  // Solo buttons become drum triggers (momentary)
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.solo.behavior = 'momentary';
    track.solo.label = 'Trigger';
  }

  return preset;
}

/**
 * Creates the "DAW Transport" preset - focus on transport and marker controls.
 * Transport buttons mapped to standard DAW functions, tracks for mixing.
 */
function createDAWTransportPreset(): Preset {
  const preset = createDefaultPreset(
    'DAW Transport',
    'Focused on DAW control with standard transport mappings. Compatible with most DAWs using Mackie Control protocol.',
    'Factory'
  );
  preset.metadata.tags = ['factory', 'daw', 'transport', 'mixing'];

  // createDefaultPreset always produces a MappingConfig; narrow the union for transport access
  const dawMapping = preset.mapping as MappingConfig;

  // Standard Mackie Control-style transport mappings
  dawMapping.transport.play.outputCC = 94;
  dawMapping.transport.play.label = 'Play';
  dawMapping.transport.stop.outputCC = 93;
  dawMapping.transport.stop.label = 'Stop';
  dawMapping.transport.record.outputCC = 95;
  dawMapping.transport.record.label = 'Record';
  dawMapping.transport.rewind.outputCC = 91;
  dawMapping.transport.rewind.label = 'Rewind';
  dawMapping.transport.forward.outputCC = 92;
  dawMapping.transport.forward.label = 'Fast Fwd';
  dawMapping.transport.cycle.outputCC = 86;
  dawMapping.transport.cycle.label = 'Loop';

  // Track labels for DAW mixing
  const dawTrackLabels = ['Ch 1', 'Ch 2', 'Ch 3', 'Ch 4', 'Ch 5', 'Ch 6', 'Ch 7', 'Master'] as const;

  const trackIndices: TrackIndex[] = [0, 1, 2, 3, 4, 5, 6, 7];
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.slider.label = dawTrackLabels[i];
    track.knob.label = `Pan ${i + 1}`;
    track.solo.label = 'Solo';
    track.mute.label = 'Mute';
    track.rec.label = 'Arm';
  }

  return preset;
}

/**
 * Creates the "DJ Mixer" preset - mappings for DJ-style control.
 * Sliders for channel faders, knobs for EQ, buttons for cue/sync.
 */
function createDJMixerPreset(): Preset {
  const preset = createDefaultPreset(
    'DJ Mixer',
    'DJ-style layout with channel faders, 3-band EQ on knobs, and cue/sync controls. Works well with DJ software.',
    'Factory'
  );
  preset.metadata.tags = ['factory', 'dj', 'mixer', 'live'];

  // First 4 tracks for Deck A, last 4 for Deck B style layout
  // Knobs become EQ controls
  const djKnobLabels = ['A Hi', 'A Mid', 'A Lo', 'A Filter', 'B Hi', 'B Mid', 'B Lo', 'B Filter'] as const;
  const djKnobCCs = [22, 23, 24, 25, 26, 27, 28, 29] as const;

  const trackIndices: TrackIndex[] = [0, 1, 2, 3, 4, 5, 6, 7];
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.knob.outputCC = djKnobCCs[i];
    track.knob.label = djKnobLabels[i];
  }

  // Sliders for channel and crossfader
  const djSliderLabels = ['Deck A', 'A Gain', 'X-Fade', 'Master', 'Deck B', 'B Gain', 'Cue Mix', 'Cue Vol'] as const;

  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.slider.label = djSliderLabels[i];
  }

  // Solo buttons for cue/pfl
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.solo.label = 'Cue';
    track.solo.behavior = 'toggle';
  }

  // Mute buttons for play/pause on decks
  const track0 = getTrack(preset, 0);
  track0.mute.label = 'A Play';
  track0.mute.behavior = 'toggle';
  const track4 = getTrack(preset, 4);
  track4.mute.label = 'B Play';
  track4.mute.behavior = 'toggle';

  // Rec buttons for sync
  track0.rec.label = 'A Sync';
  track4.rec.label = 'B Sync';

  return preset;
}

/**
 * Creates the "Nord Drum 2" preset for nanoKONTROL2.
 * Maps 16 parameters to control the Nord Drum 2 Desktop percussion synthesizer.
 * Tracks 1-6 control individual pads (channels 1-6) with Tone Spectra on knobs
 * and Tone Decay on sliders. Tracks 7-8 control global FX on channel 10.
 */
function createNordDrum2NK2Preset(): Preset {
  const preset = createDefaultPreset(
    'Nord Drum 2',
    'Nord Drum 2 Desktop control. Knobs: Tone Spectra, Sliders: Tone Decay. Tracks 1-6 = Pads, Tracks 7-8 = Global FX (ch10).',
    'Nord Drum 2'
  );
  preset.metadata.tags = ['factory', 'nord-drum-2', 'percussion', 'synthesizer'];

  // Pad channels (1-indexed as used by MappingEntry.channel)
  const padChannels = [
    NORD_DRUM_2_CHANNELS.PAD_1,
    NORD_DRUM_2_CHANNELS.PAD_2,
    NORD_DRUM_2_CHANNELS.PAD_3,
    NORD_DRUM_2_CHANNELS.PAD_4,
    NORD_DRUM_2_CHANNELS.PAD_5,
    NORD_DRUM_2_CHANNELS.PAD_6,
  ] as const;

  // Tracks 0-5: individual pads (narrowed to 6-element tuple indices)
  type PadIndex = 0 | 1 | 2 | 3 | 4 | 5;
  const padTrackIndices: PadIndex[] = [0, 1, 2, 3, 4, 5];
  for (const i of padTrackIndices) {
    const track = getTrack(preset, i);
    const padNum = i + 1;

    // Knob -> Tone Spectra
    track.knob.outputCC = NORD_DRUM_2_CC.TONE.SPECTRA;
    track.knob.channel = padChannels[i];
    track.knob.label = `Pad${padNum} Spectra`;

    // Slider -> Tone Decay
    track.slider.outputCC = NORD_DRUM_2_CC.TONE.DECAY;
    track.slider.channel = padChannels[i];
    track.slider.label = `Pad${padNum} Decay`;

    // Solo button -> momentary trigger
    track.solo.channel = padChannels[i];
    track.solo.behavior = 'momentary';
    track.solo.label = 'Trigger';

    // Mute button -> toggle mute
    track.mute.channel = padChannels[i];
    track.mute.behavior = 'toggle';
    track.mute.label = 'Mute';

    // Rec button -> keep default
    track.rec.channel = padChannels[i];
    track.rec.label = 'Rec';
  }

  // Track 6: Global FX - Echo
  const fxTrack1 = getTrack(preset, 6);
  fxTrack1.knob.outputCC = NORD_DRUM_2_FX_CC.ECHO_AMOUNT;
  fxTrack1.knob.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack1.knob.label = 'Echo Amount';
  fxTrack1.slider.outputCC = NORD_DRUM_2_FX_CC.ECHO_FEEDBACK;
  fxTrack1.slider.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack1.slider.label = 'Echo Feedbk';
  fxTrack1.solo.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack1.solo.behavior = 'momentary';
  fxTrack1.solo.label = 'Trigger';
  fxTrack1.mute.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack1.mute.behavior = 'toggle';
  fxTrack1.mute.label = 'Mute';
  fxTrack1.rec.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack1.rec.label = 'Rec';

  // Track 7: Global FX - Distortion & EQ
  const fxTrack2 = getTrack(preset, 7);
  fxTrack2.knob.outputCC = NORD_DRUM_2_FX_CC.DIST_AMOUNT;
  fxTrack2.knob.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack2.knob.label = 'Dist Amount';
  fxTrack2.slider.outputCC = NORD_DRUM_2_FX_CC.EQ_GAIN;
  fxTrack2.slider.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack2.slider.label = 'EQ Gain';
  fxTrack2.solo.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack2.solo.behavior = 'momentary';
  fxTrack2.solo.label = 'Trigger';
  fxTrack2.mute.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack2.mute.behavior = 'toggle';
  fxTrack2.mute.label = 'Mute';
  fxTrack2.rec.channel = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  fxTrack2.rec.label = 'Rec';

  return preset;
}

// =============================================================================
// MidiMix Sample Preset Factories
// =============================================================================

/**
 * CC number tables from the Akai MidiMix hardware definition.
 * Duplicated here as literal arrays to avoid coupling preset factories
 * to the device definition module at runtime. These are hardware constants
 * that will never change.
 */
const MIDIMIX_KNOB_ROW1 = [16, 20, 24, 28, 46, 50, 54, 58] as const;
const MIDIMIX_KNOB_ROW2 = [17, 21, 25, 29, 47, 51, 55, 59] as const;
const MIDIMIX_KNOB_ROW3 = [18, 22, 26, 30, 48, 52, 56, 60] as const;
const MIDIMIX_FADERS = [19, 23, 27, 31, 49, 53, 57, 61] as const;
const MIDIMIX_MUTE_NOTES = [1, 4, 7, 10, 13, 16, 19, 22] as const;
const MIDIMIX_SOLO_NOTES = [2, 5, 8, 11, 14, 17, 20, 23] as const;
const MIDIMIX_REC_ARM_NOTES = [3, 6, 9, 12, 15, 18, 21, 24] as const;

/**
 * Builds a nK2-shaped MappingConfig from MidiMix hardware assignments.
 * The MidiMix has more controls per track than nK2, so we map:
 *   knob1 -> knob slot, fader -> slider slot, mute -> mute, solo -> solo, rec_arm -> rec.
 * Extra knobs (knob2, knob3) and the master section cannot be represented
 * in the nK2 MappingConfig shape, so they are omitted from preset data.
 */
function createMidiMixIdentityMapping(): MappingConfig {
  const tracks: TrackMapping[] = [];

  for (let i = 0; i < 8; i++) {
    const idx = i as TrackIndex;
    tracks.push({
      knob: {
        inputCC: MIDIMIX_KNOB_ROW1[idx],
        outputCC: MIDIMIX_KNOB_ROW1[idx],
        channel: 1,
        messageType: 'cc',
      },
      slider: {
        inputCC: MIDIMIX_FADERS[idx],
        outputCC: MIDIMIX_FADERS[idx],
        channel: 1,
        messageType: 'cc',
      },
      solo: {
        inputCC: MIDIMIX_SOLO_NOTES[idx],
        outputCC: MIDIMIX_SOLO_NOTES[idx],
        channel: 1,
        behavior: 'toggle',
        messageType: 'note',
      },
      mute: {
        inputCC: MIDIMIX_MUTE_NOTES[idx],
        outputCC: MIDIMIX_MUTE_NOTES[idx],
        channel: 1,
        behavior: 'toggle',
        messageType: 'note',
      },
      rec: {
        inputCC: MIDIMIX_REC_ARM_NOTES[idx],
        outputCC: MIDIMIX_REC_ARM_NOTES[idx],
        channel: 1,
        behavior: 'momentary',
        messageType: 'note',
      },
    });
  }

  // MidiMix has no transport section; provide empty transport with zeros
  const emptyTransportEntry = (behavior: 'toggle' | 'momentary'): MappingEntry => ({
    inputCC: 0,
    outputCC: 0,
    channel: 1,
    behavior,
  });

  const transport: TransportMapping = {
    rewind: emptyTransportEntry('momentary'),
    forward: emptyTransportEntry('momentary'),
    stop: emptyTransportEntry('momentary'),
    play: emptyTransportEntry('momentary'),
    record: emptyTransportEntry('momentary'),
    cycle: emptyTransportEntry('toggle'),
    track_left: emptyTransportEntry('momentary'),
    track_right: emptyTransportEntry('momentary'),
    marker_set: emptyTransportEntry('momentary'),
    marker_left: emptyTransportEntry('momentary'),
    marker_right: emptyTransportEntry('momentary'),
  };

  return { tracks, transport };
}

/**
 * Creates the "Default MidiMix" preset - identity mapping for the Akai MidiMix.
 * All controls map to their hardware MIDI numbers with correct message types.
 */
function createDefaultMidiMixPreset(): Preset {
  const metadata = createDefaultPresetMetadata('Default MidiMix', 'MidiMix Factory');
  metadata.description = 'Identity mapping for the Akai MidiMix. All controls pass through unchanged with correct CC/Note message types.';
  metadata.tags = ['factory', 'midimix', 'default', 'identity'];

  return {
    metadata,
    mapping: createMidiMixIdentityMapping(),
    controlValues: createDefaultControlValues(),
  };
}

/**
 * Creates the "MidiMix Synth" preset.
 * Knobs mapped to filter/resonance/envelope parameters, faders to oscillator levels.
 */
function createMidiMixSynthPreset(): Preset {
  const preset: Preset = {
    metadata: createDefaultPresetMetadata('MidiMix Synth', 'MidiMix Factory'),
    mapping: createMidiMixIdentityMapping(),
    controlValues: createDefaultControlValues(),
  };
  preset.metadata.description = 'Synth control layout for MidiMix. Knobs control filter cutoff, resonance, and envelope. Faders control oscillator and mix levels.';
  preset.metadata.tags = ['factory', 'midimix', 'synth', 'performance'];

  // Remap knob row 1 to common synth filter/mod CCs
  const synthKnobCCs = [74, 71, 73, 72, 91, 93, 1, 11] as const;
  const synthKnobLabels = ['Filter', 'Resonance', 'Attack', 'Release', 'Reverb', 'Chorus', 'Mod Wheel', 'Expression'] as const;

  const trackIndices: TrackIndex[] = [0, 1, 2, 3, 4, 5, 6, 7];
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.knob.outputCC = synthKnobCCs[i];
    track.knob.label = synthKnobLabels[i];
  }

  // Remap faders to oscillator levels
  const synthSliderLabels = ['OSC 1', 'OSC 2', 'Sub', 'Noise', 'Filter Env', 'Amp Env', 'FX Send', 'Master'] as const;
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.slider.label = synthSliderLabels[i];
  }

  return preset;
}

/**
 * Creates the "MidiMix DJ" preset.
 * Knobs mapped to 3-band EQ per channel, faders to channel volume.
 */
function createMidiMixDJPreset(): Preset {
  const preset: Preset = {
    metadata: createDefaultPresetMetadata('MidiMix DJ', 'MidiMix Factory'),
    mapping: createMidiMixIdentityMapping(),
    controlValues: createDefaultControlValues(),
  };
  preset.metadata.description = 'DJ-style layout for MidiMix. Knob row 1 as EQ High, faders as channel volumes, buttons as cue/sync controls.';
  preset.metadata.tags = ['factory', 'midimix', 'dj', 'live'];

  // Remap knobs to EQ-style CCs (high-frequency band)
  const eqHighCCs = [22, 23, 24, 25, 26, 27, 28, 29] as const;
  const eqLabels = ['Ch1 Hi', 'Ch2 Hi', 'Ch3 Hi', 'Ch4 Hi', 'Ch5 Hi', 'Ch6 Hi', 'Ch7 Hi', 'Ch8 Hi'] as const;

  const trackIndices: TrackIndex[] = [0, 1, 2, 3, 4, 5, 6, 7];
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.knob.outputCC = eqHighCCs[i];
    track.knob.label = eqLabels[i];
  }

  // Label faders as channel volumes
  const volLabels = ['Ch1 Vol', 'Ch2 Vol', 'Ch3 Vol', 'Ch4 Vol', 'Ch5 Vol', 'Ch6 Vol', 'Ch7 Vol', 'Ch8 Vol'] as const;
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.slider.label = volLabels[i];
  }

  // Label mute buttons as cue
  for (const i of trackIndices) {
    const track = getTrack(preset, i);
    track.mute.label = 'Cue';
    track.solo.label = 'PFL';
  }

  return preset;
}

/**
 * Creates the "MidiMix Nord Drum 2" preset using DeviceMappingConfig.
 * Maps 32 continuous controls across all 3 knob rows and 8 faders to control
 * the Nord Drum 2 Desktop. Tracks 1-6 = Pads (channels 1-6),
 * Tracks 7-8 = Global FX (channel 10).
 */
function createNordDrum2MidiMixPreset(): Preset {
  const padChannels = [
    NORD_DRUM_2_CHANNELS.PAD_1,
    NORD_DRUM_2_CHANNELS.PAD_2,
    NORD_DRUM_2_CHANNELS.PAD_3,
    NORD_DRUM_2_CHANNELS.PAD_4,
    NORD_DRUM_2_CHANNELS.PAD_5,
    NORD_DRUM_2_CHANNELS.PAD_6,
  ] as const;

  const tracks: Record<string, Record<string, MappingEntry>> = {};

  // Tracks 1-6: individual pads (channels 1-6)
  type PadIndex = 0 | 1 | 2 | 3 | 4 | 5;
  for (let i = 0; i < 6; i++) {
    const idx = i as TrackIndex;
    const padNum = i + 1;
    const ch = padChannels[i as PadIndex];

    tracks[String(padNum)] = {
      knob1: {
        inputCC: MIDIMIX_KNOB_ROW1[idx],
        outputCC: NORD_DRUM_2_CC.TONE.SPECTRA,
        channel: ch,
        messageType: 'cc',
        label: `Pad${padNum} Spectra`,
      },
      knob2: {
        inputCC: MIDIMIX_KNOB_ROW2[idx],
        outputCC: NORD_DRUM_2_CC.NOISE.FILTER_FREQ,
        channel: ch,
        messageType: 'cc',
        label: `Pad${padNum} NoiseFlt`,
      },
      knob3: {
        inputCC: MIDIMIX_KNOB_ROW3[idx],
        outputCC: NORD_DRUM_2_CC.TONE.TIMBRE,
        channel: ch,
        messageType: 'cc',
        label: `Pad${padNum} Timbre`,
      },
      fader: {
        inputCC: MIDIMIX_FADERS[idx],
        outputCC: NORD_DRUM_2_CC.MIX.LEVEL,
        channel: ch,
        messageType: 'cc',
        label: `Pad${padNum} Level`,
      },
      mute: {
        inputCC: MIDIMIX_MUTE_NOTES[idx],
        outputCC: MIDIMIX_MUTE_NOTES[idx],
        channel: ch,
        behavior: 'toggle',
        messageType: 'note',
        label: 'Mute',
      },
      solo: {
        inputCC: MIDIMIX_SOLO_NOTES[idx],
        outputCC: MIDIMIX_SOLO_NOTES[idx],
        channel: ch,
        behavior: 'momentary',
        messageType: 'note',
        label: 'Trigger',
      },
      rec_arm: {
        inputCC: MIDIMIX_REC_ARM_NOTES[idx],
        outputCC: MIDIMIX_REC_ARM_NOTES[idx],
        channel: ch,
        behavior: 'momentary',
        messageType: 'note',
        label: 'Rec',
      },
    };
  }

  // Track 7: Global FX - Echo + EQ Freq
  const fxCh = NORD_DRUM_2_CHANNELS.GLOBAL_FX;
  tracks['7'] = {
    knob1: {
      inputCC: MIDIMIX_KNOB_ROW1[6],
      outputCC: NORD_DRUM_2_FX_CC.ECHO_AMOUNT,
      channel: fxCh,
      messageType: 'cc',
      label: 'Echo Amt',
    },
    knob2: {
      inputCC: MIDIMIX_KNOB_ROW2[6],
      outputCC: NORD_DRUM_2_FX_CC.ECHO_FEEDBACK,
      channel: fxCh,
      messageType: 'cc',
      label: 'Echo FB',
    },
    knob3: {
      inputCC: MIDIMIX_KNOB_ROW3[6],
      outputCC: NORD_DRUM_2_FX_CC.EQ_FREQ,
      channel: fxCh,
      messageType: 'cc',
      label: 'EQ Freq',
    },
    fader: {
      inputCC: MIDIMIX_FADERS[6],
      outputCC: NORD_DRUM_2_FX_CC.EQ_GAIN,
      channel: fxCh,
      messageType: 'cc',
      label: 'EQ Gain',
    },
    mute: {
      inputCC: MIDIMIX_MUTE_NOTES[6],
      outputCC: MIDIMIX_MUTE_NOTES[6],
      channel: fxCh,
      behavior: 'toggle',
      messageType: 'note',
      label: 'Mute',
    },
    solo: {
      inputCC: MIDIMIX_SOLO_NOTES[6],
      outputCC: MIDIMIX_SOLO_NOTES[6],
      channel: fxCh,
      behavior: 'momentary',
      messageType: 'note',
      label: 'Trigger',
    },
    rec_arm: {
      inputCC: MIDIMIX_REC_ARM_NOTES[6],
      outputCC: MIDIMIX_REC_ARM_NOTES[6],
      channel: fxCh,
      behavior: 'momentary',
      messageType: 'note',
      label: 'Rec',
    },
  };

  // Track 8: Global FX - Distortion + misc
  tracks['8'] = {
    knob1: {
      inputCC: MIDIMIX_KNOB_ROW1[7],
      outputCC: NORD_DRUM_2_FX_CC.DIST_AMOUNT,
      channel: fxCh,
      messageType: 'cc',
      label: 'Dist Amt',
    },
    knob2: {
      inputCC: MIDIMIX_KNOB_ROW2[7],
      outputCC: NORD_DRUM_2_FX_CC.DIST_TYPE,
      channel: fxCh,
      messageType: 'cc',
      label: 'Dist Type',
    },
    knob3: {
      inputCC: MIDIMIX_KNOB_ROW3[7],
      outputCC: NORD_DRUM_2_CC.TONE.DECAY,
      channel: fxCh,
      messageType: 'cc',
      label: 'Tone Dcy',
    },
    fader: {
      inputCC: MIDIMIX_FADERS[7],
      outputCC: NORD_DRUM_2_CC.TONE.PUNCH,
      channel: fxCh,
      messageType: 'cc',
      label: 'Punch',
    },
    mute: {
      inputCC: MIDIMIX_MUTE_NOTES[7],
      outputCC: MIDIMIX_MUTE_NOTES[7],
      channel: fxCh,
      behavior: 'toggle',
      messageType: 'note',
      label: 'Mute',
    },
    solo: {
      inputCC: MIDIMIX_SOLO_NOTES[7],
      outputCC: MIDIMIX_SOLO_NOTES[7],
      channel: fxCh,
      behavior: 'momentary',
      messageType: 'note',
      label: 'Trigger',
    },
    rec_arm: {
      inputCC: MIDIMIX_REC_ARM_NOTES[7],
      outputCC: MIDIMIX_REC_ARM_NOTES[7],
      channel: fxCh,
      behavior: 'momentary',
      messageType: 'note',
      label: 'Rec',
    },
  };

  const mapping: z.infer<typeof deviceMappingConfigSchema> = {
    deviceId: 'akai-midimix',
    tracks,
    sections: {},
  };

  const metadata = createDefaultPresetMetadata('MidiMix Nord Drum 2', 'Nord Drum 2');
  metadata.description = 'Nord Drum 2 Desktop control for Akai MidiMix. 32 parameters: Knob1=Spectra, Knob2=Noise Filter, Knob3=Timbre, Faders=Level. Tracks 1-6=Pads, 7-8=Global FX (ch10).';
  metadata.tags = ['factory', 'nord-drum-2', 'midimix', 'percussion', 'synthesizer'];

  return {
    metadata,
    mapping,
  };
}

/**
 * Creates all sample presets for first-run initialization.
 * Returns an array of presets ready to be saved.
 * Includes both nanoKONTROL2 and MidiMix factory presets.
 */
export function createSamplePresets(): Preset[] {
  return [
    createDefaultSamplePreset(),
    createSynthLeadPreset(),
    createDrumsPreset(),
    createDAWTransportPreset(),
    createDJMixerPreset(),
    createNordDrum2NK2Preset(),
    createDefaultMidiMixPreset(),
    createMidiMixSynthPreset(),
    createMidiMixDJPreset(),
    createNordDrum2MidiMixPreset(),
  ];
}

// =============================================================================
// Duplicate Name Generation
// =============================================================================

/**
 * Generates a name for a duplicated preset.
 *
 * Rules:
 * - If name doesn't end in " copy" or " copy N", append " copy"
 * - If name ends in " copy", append " 2"
 * - If name ends in " copy N", increment N
 *
 * @param originalName - The original preset name
 * @param existingNames - Array of existing preset names to check for conflicts
 * @returns A unique name for the duplicate
 */
export function generateDuplicateName(originalName: string, existingNames: string[]): string {
  // Regex to match " copy" or " copy N" at the end
  const copyPattern = / copy( \d+)?$/i;
  const match = originalName.match(copyPattern);

  let baseName: string;
  let copyNumber: number;

  if (match) {
    // Name already has " copy" suffix
    baseName = originalName.replace(copyPattern, '');
    if (match[1]) {
      // Has " copy N" - increment the number
      copyNumber = parseInt(match[1].trim(), 10) + 1;
    } else {
      // Has " copy" - start at 2
      copyNumber = 2;
    }
  } else {
    // No copy suffix - add " copy"
    baseName = originalName;
    copyNumber = 0; // Will be " copy" (no number)
  }

  // Generate the candidate name
  let candidateName = copyNumber === 0 ? `${baseName} copy` : `${baseName} copy ${copyNumber}`;

  // Check for conflicts and increment if needed
  const existingNamesLower = existingNames.map(n => n.toLowerCase());
  while (existingNamesLower.includes(candidateName.toLowerCase())) {
    copyNumber = copyNumber === 0 ? 2 : copyNumber + 1;
    candidateName = `${baseName} copy ${copyNumber}`;
  }

  return candidateName;
}
