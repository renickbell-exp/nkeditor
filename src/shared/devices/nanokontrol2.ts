/**
 * Korg nanoKONTROL2 Device Definition
 *
 * Declares the complete hardware layout of the nanoKONTROL2:
 * - 8 tracks, each with 1 knob, 1 slider, and 3 buttons (solo/mute/rec)
 * - Transport section with 11 buttons (6 with LEDs)
 * - All controls communicate via CC messages on channel 1
 * - LEDs are controlled by sending CC messages back to the device
 *
 * CC assignments match the factory default configuration of the nanoKONTROL2.
 */

import type {
  DeviceDefinition,
  TrackLayout,
  ContinuousControlDef,
  ButtonControlDef,
  ExtraControlsSection,
  LedDef,
} from './device-definition.js';

// =============================================================================
// Hardware CC Number Tables
// =============================================================================

/** CC numbers for the 8 rotary knobs (tracks 1-8) */
const KNOB_CCS = [16, 17, 18, 19, 20, 21, 22, 23] as const;

/** CC numbers for the 8 linear sliders (tracks 1-8) */
const SLIDER_CCS = [0, 1, 2, 3, 4, 5, 6, 7] as const;

/** CC numbers for the 8 solo buttons (tracks 1-8) */
const SOLO_CCS = [32, 33, 34, 35, 36, 37, 38, 39] as const;

/** CC numbers for the 8 mute buttons (tracks 1-8) */
const MUTE_CCS = [48, 49, 50, 51, 52, 53, 54, 55] as const;

/** CC numbers for the 8 record arm buttons (tracks 1-8) */
const REC_CCS = [64, 65, 66, 67, 68, 69, 70, 71] as const;

// =============================================================================
// Track Layout Builder
// =============================================================================

/**
 * Builds the TrackLayout for a single nK2 track.
 * Each track has exactly 1 knob, 1 slider, and 3 buttons.
 */
function buildTrackLayout(trackIndex: number): TrackLayout {
  const trackNumber = trackIndex + 1;

  const knob: ContinuousControlDef = {
    kind: 'continuous',
    name: 'knob',
    displayName: `Track ${trackNumber} Knob`,
    midiNumber: KNOB_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    visualType: 'knob',
  };

  const slider: ContinuousControlDef = {
    kind: 'continuous',
    name: 'slider',
    displayName: `Track ${trackNumber} Slider`,
    midiNumber: SLIDER_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    visualType: 'slider',
  };

  const solo: ButtonControlDef = {
    kind: 'button',
    name: 'solo',
    displayName: `Track ${trackNumber} Solo`,
    midiNumber: SOLO_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    defaultBehavior: 'toggle',
    hasLed: true,
  };

  const mute: ButtonControlDef = {
    kind: 'button',
    name: 'mute',
    displayName: `Track ${trackNumber} Mute`,
    midiNumber: MUTE_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    defaultBehavior: 'toggle',
    hasLed: true,
  };

  const rec: ButtonControlDef = {
    kind: 'button',
    name: 'rec',
    displayName: `Track ${trackNumber} Rec`,
    midiNumber: REC_CCS[trackIndex]!,
    channel: 1,
    messageType: 'cc',
    defaultBehavior: 'momentary',
    hasLed: true,
  };

  return {
    continuousControls: [knob, slider],
    buttonControls: [solo, mute, rec],
  };
}

// =============================================================================
// Transport Section
// =============================================================================

function buildTransportSection(): ExtraControlsSection {
  const transportControls: ButtonControlDef[] = [
    {
      kind: 'button',
      name: 'play',
      displayName: 'Play',
      midiNumber: 41,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: true,
    },
    {
      kind: 'button',
      name: 'stop',
      displayName: 'Stop',
      midiNumber: 42,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: true,
    },
    {
      kind: 'button',
      name: 'rewind',
      displayName: 'Rewind',
      midiNumber: 43,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: false,
    },
    {
      kind: 'button',
      name: 'forward',
      displayName: 'Forward',
      midiNumber: 44,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: false,
    },
    {
      kind: 'button',
      name: 'record',
      displayName: 'Record',
      midiNumber: 45,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: true,
    },
    {
      kind: 'button',
      name: 'cycle',
      displayName: 'Cycle',
      midiNumber: 46,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'toggle',
      hasLed: true,
    },
    {
      kind: 'button',
      name: 'track_left',
      displayName: 'Track Left',
      midiNumber: 58,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: false,
    },
    {
      kind: 'button',
      name: 'track_right',
      displayName: 'Track Right',
      midiNumber: 59,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: false,
    },
    {
      kind: 'button',
      name: 'marker_set',
      displayName: 'Marker Set',
      midiNumber: 60,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: false,
    },
    {
      kind: 'button',
      name: 'marker_left',
      displayName: 'Marker Left',
      midiNumber: 61,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: false,
    },
    {
      kind: 'button',
      name: 'marker_right',
      displayName: 'Marker Right',
      midiNumber: 62,
      channel: 1,
      messageType: 'cc',
      defaultBehavior: 'momentary',
      hasLed: false,
    },
  ];

  return {
    name: 'transport',
    displayName: 'Transport',
    controls: transportControls,
  };
}

// =============================================================================
// LED Definitions
// =============================================================================

/**
 * Builds the LED definitions for the nK2.
 * LEDs share CC numbers with their corresponding buttons.
 * Only buttons with physical LEDs are included:
 * - 8 solo LEDs + 8 mute LEDs + 8 rec LEDs = 24 track LEDs
 * - 4 transport LEDs (play, stop, record, cycle) = 4 transport LEDs
 * - Total: 28 LEDs
 */
function buildLedDefinitions(): LedDef[] {
  const leds: LedDef[] = [];

  for (let trackIndex = 0; trackIndex < 8; trackIndex++) {
    const trackNumber = trackIndex + 1;
    leds.push(
      { buttonName: `track${trackNumber}.solo`, midiNumber: SOLO_CCS[trackIndex]! },
      { buttonName: `track${trackNumber}.mute`, midiNumber: MUTE_CCS[trackIndex]! },
      { buttonName: `track${trackNumber}.rec`, midiNumber: REC_CCS[trackIndex]! },
    );
  }

  // Transport LEDs: only play (41), stop (42), record (45), cycle (46)
  leds.push(
    { buttonName: 'transport.play', midiNumber: 41 },
    { buttonName: 'transport.stop', midiNumber: 42 },
    { buttonName: 'transport.record', midiNumber: 45 },
    { buttonName: 'transport.cycle', midiNumber: 46 },
  );

  return leds;
}

// =============================================================================
// Device Definition Export
// =============================================================================

const trackLayouts: TrackLayout[] = Array.from({ length: 8 }, (_, index) =>
  buildTrackLayout(index),
);

export const NANOKONTROL2_DEFINITION: DeviceDefinition = {
  id: 'nanokontrol2',
  name: 'nanoKONTROL2',
  manufacturer: 'Korg',
  portPatterns: [/^nanokontrol2[:\s]/i],
  trackCount: 8,
  trackLayouts: trackLayouts,
  extraSections: [buildTransportSection()],
  ledProtocol: {
    messageType: 'cc',
    channel: 1,
    onValue: 127,
    offValue: 0,
  },
  leds: buildLedDefinitions(),
  virtualPortName: 'nkEditor3 Out',
};
