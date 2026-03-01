/**
 * Akai MidiMix Device Definition
 *
 * Declares the complete hardware layout of the Akai MidiMix:
 * - 8 tracks, each with 3 knobs, 1 fader, and 3 buttons (mute/solo/rec_arm)
 * - Master section with 1 fader and 3 buttons (bank_left, bank_right, solo_master)
 * - Knobs and faders communicate via CC messages on channel 1
 * - Buttons communicate via Note messages on channel 1
 * - LEDs are controlled by sending Note messages back to the device
 *
 * The MidiMix uses a non-contiguous CC/Note numbering scheme where tracks are
 * grouped into two banks of 4, with a gap in the MIDI number sequence between
 * tracks 4 and 5.
 */

import type {
  DeviceDefinition,
  TrackLayout,
  ContinuousControlDef,
  ButtonControlDef,
  ExtraControlsSection,
  ControlDef,
  LedDef,
} from './device-definition.js';

// =============================================================================
// Hardware CC/Note Number Tables
// =============================================================================

/**
 * Knob CC numbers are arranged in 3 rows across 8 tracks.
 * The numbering has a gap between tracks 4 and 5 due to the MidiMix's
 * internal bank architecture.
 */
const KNOB_ROW1_CCS = [16, 20, 24, 28, 46, 50, 54, 58] as const;
const KNOB_ROW2_CCS = [17, 21, 25, 29, 47, 51, 55, 59] as const;
const KNOB_ROW3_CCS = [18, 22, 26, 30, 48, 52, 56, 60] as const;

/** Fader CC numbers for tracks 1-8 */
const FADER_CCS = [19, 23, 27, 31, 49, 53, 57, 61] as const;

/**
 * Button Note numbers. The MidiMix sends Note On/Off messages for buttons
 * rather than CC messages. Notes are numbered in groups of 3 per track
 * (mute, solo, rec_arm).
 */
const MUTE_NOTES = [1, 4, 7, 10, 13, 16, 19, 22] as const;
const SOLO_NOTES = [2, 5, 8, 11, 14, 17, 20, 23] as const;
const REC_ARM_NOTES = [3, 6, 9, 12, 15, 18, 21, 24] as const;

// =============================================================================
// Track Layout Builder
// =============================================================================

/**
 * Builds the TrackLayout for a single MidiMix track.
 * Each track has 3 knobs (rows 1-3), 1 fader, and 3 buttons.
 */
function buildTrackLayout(trackIndex: number): TrackLayout {
  const trackNumber = trackIndex + 1;

  const knob1: ContinuousControlDef = {
    kind: 'continuous',
    name: 'knob1',
    displayName: `Track ${trackNumber} Knob 1`,
    midiNumber: KNOB_ROW1_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    visualType: 'knob',
  };

  const knob2: ContinuousControlDef = {
    kind: 'continuous',
    name: 'knob2',
    displayName: `Track ${trackNumber} Knob 2`,
    midiNumber: KNOB_ROW2_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    visualType: 'knob',
  };

  const knob3: ContinuousControlDef = {
    kind: 'continuous',
    name: 'knob3',
    displayName: `Track ${trackNumber} Knob 3`,
    midiNumber: KNOB_ROW3_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    visualType: 'knob',
  };

  const fader: ContinuousControlDef = {
    kind: 'continuous',
    name: 'fader',
    displayName: `Track ${trackNumber} Fader`,
    midiNumber: FADER_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    visualType: 'fader',
  };

  const mute: ButtonControlDef = {
    kind: 'button',
    name: 'mute',
    displayName: `Track ${trackNumber} Mute`,
    midiNumber: MUTE_NOTES[trackIndex]!,
    channel: 1,
    messageType: 'note',
    defaultBehavior: 'toggle',
    hasLed: true,
  };

  const solo: ButtonControlDef = {
    kind: 'button',
    name: 'solo',
    displayName: `Track ${trackNumber} Solo`,
    midiNumber: SOLO_NOTES[trackIndex]!,
    channel: 1,
    messageType: 'note',
    defaultBehavior: 'toggle',
    hasLed: true,
  };

  const recArm: ButtonControlDef = {
    kind: 'button',
    name: 'rec_arm',
    displayName: `Track ${trackNumber} Rec Arm`,
    midiNumber: REC_ARM_NOTES[trackIndex]!,
    channel: 1,
    messageType: 'note',
    defaultBehavior: 'momentary',
    hasLed: true,
  };

  return {
    continuousControls: [knob1, knob2, knob3, fader],
    buttonControls: [mute, solo, recArm],
  };
}

// =============================================================================
// Master Section
// =============================================================================

function buildMasterSection(): ExtraControlsSection {
  const masterFader: ContinuousControlDef = {
    kind: 'continuous',
    name: 'master_fader',
    displayName: 'Master Fader',
    midiNumber: 62,
    channel: 1,
    messageType: 'cc',
    visualType: 'fader',
  };

  const bankLeft: ButtonControlDef = {
    kind: 'button',
    name: 'bank_left',
    displayName: 'Bank Left',
    midiNumber: 25,
    channel: 1,
    messageType: 'note',
    defaultBehavior: 'momentary',
    hasLed: false,
  };

  const bankRight: ButtonControlDef = {
    kind: 'button',
    name: 'bank_right',
    displayName: 'Bank Right',
    midiNumber: 26,
    channel: 1,
    messageType: 'note',
    defaultBehavior: 'momentary',
    hasLed: false,
  };

  const soloMaster: ButtonControlDef = {
    kind: 'button',
    name: 'solo_master',
    displayName: 'Solo Master',
    midiNumber: 27,
    channel: 1,
    messageType: 'note',
    defaultBehavior: 'toggle',
    hasLed: false,
  };

  const controls: ControlDef[] = [masterFader, bankLeft, bankRight, soloMaster];

  return {
    name: 'master',
    displayName: 'Master',
    controls,
  };
}

// =============================================================================
// LED Definitions
// =============================================================================

/**
 * Builds the LED definitions for the MidiMix.
 * Only the per-track mute, solo, and rec_arm buttons have LEDs.
 * Master section buttons have no LEDs.
 * Total: 8 mute + 8 solo + 8 rec_arm = 24 LEDs
 */
function buildLedDefinitions(): LedDef[] {
  const leds: LedDef[] = [];

  for (let trackIndex = 0; trackIndex < 8; trackIndex++) {
    const trackNumber = trackIndex + 1;
    leds.push(
      { buttonName: `track${trackNumber}.mute`, midiNumber: MUTE_NOTES[trackIndex]! },
      { buttonName: `track${trackNumber}.solo`, midiNumber: SOLO_NOTES[trackIndex]! },
      { buttonName: `track${trackNumber}.rec_arm`, midiNumber: REC_ARM_NOTES[trackIndex]! },
    );
  }

  return leds;
}

// =============================================================================
// Device Definition Export
// =============================================================================

const trackLayouts: TrackLayout[] = Array.from({ length: 8 }, (_, index) =>
  buildTrackLayout(index),
);

export const AKAI_MIDIMIX_DEFINITION: DeviceDefinition = {
  id: 'akai-midimix',
  name: 'MIDI Mix',
  manufacturer: 'Akai',
  portPatterns: [/^MIDI Mix[:\s]/i, /^MIDI Mix MIDI/i],
  trackCount: 8,
  trackLayouts: trackLayouts,
  extraSections: [buildMasterSection()],
  ledProtocol: {
    messageType: 'note',
    channel: 1,
    onValue: 127,
    offValue: 0,
  },
  leds: buildLedDefinitions(),
};
