/**
 * Hardware CC mappings for the Korg nanoKONTROL2.
 *
 * These constants define the default CC assignments for the hardware controller.
 * The nanoKONTROL2 has 8 tracks with knob, slider, and 3 buttons each,
 * plus a transport section with 11 buttons.
 */

/**
 * Hardware CC assignments for the nanoKONTROL2 controller.
 * These are the CC numbers sent by the hardware when controls are operated.
 */
export const HARDWARE_CC = {
  /** Rotary knobs for tracks 1-8 (CC 16-23) */
  KNOBS: [16, 17, 18, 19, 20, 21, 22, 23] as const,

  /** Linear sliders/faders for tracks 1-8 (CC 0-7) */
  SLIDERS: [0, 1, 2, 3, 4, 5, 6, 7] as const,

  /** Solo buttons for tracks 1-8 (CC 32-39) */
  SOLO: [32, 33, 34, 35, 36, 37, 38, 39] as const,

  /** Mute buttons for tracks 1-8 (CC 48-55) */
  MUTE: [48, 49, 50, 51, 52, 53, 54, 55] as const,

  /** Record arm buttons for tracks 1-8 (CC 64-71) */
  REC: [64, 65, 66, 67, 68, 69, 70, 71] as const,

  /** Transport section buttons */
  TRANSPORT: {
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
  } as const,
} as const;

/**
 * LED CC assignments for the nanoKONTROL2.
 * LEDs are controlled by sending CC messages to the hardware output port.
 * LEDs use the same CC numbers as their corresponding buttons.
 *
 * Note: Only buttons with LEDs are included. Knobs and sliders have no LEDs.
 * Transport buttons without LEDs (track_left, track_right, markers) are omitted.
 */
export const LED_CC = {
  /** Solo button LEDs for tracks 1-8 (yellow LEDs) */
  SOLO: [32, 33, 34, 35, 36, 37, 38, 39] as const,

  /** Mute button LEDs for tracks 1-8 (green LEDs) */
  MUTE: [48, 49, 50, 51, 52, 53, 54, 55] as const,

  /** Record button LEDs for tracks 1-8 (red LEDs) */
  REC: [64, 65, 66, 67, 68, 69, 70, 71] as const,

  /** Transport button LEDs (play, stop, record, cycle) */
  TRANSPORT: [41, 42, 45, 46] as const,
} as const;

/**
 * MIDI CC value constants.
 */
export const MIDI_VALUES = {
  /** Value sent/received when a button is pressed or LED should be on */
  ON: 127,
  /** Value sent/received when a button is released or LED should be off */
  OFF: 0,
  /** Minimum valid CC value */
  MIN: 0,
  /** Maximum valid CC value */
  MAX: 127,
} as const;

/**
 * MIDI channel constants.
 */
export const MIDI_CHANNELS = {
  /** Minimum valid channel (user-facing, 1-indexed) */
  MIN: 1,
  /** Maximum valid channel (user-facing, 1-indexed) */
  MAX: 16,
  /** Default channel for mappings if not specified */
  DEFAULT: 1,
} as const;

/**
 * Track constants.
 */
export const TRACK_CONSTANTS = {
  /** Total number of tracks on the nanoKONTROL2 */
  COUNT: 8,
  /** First track number (1-indexed for user display) */
  FIRST: 1,
  /** Last track number (1-indexed for user display) */
  LAST: 8,
} as const;

/**
 * Valid control type names for track sections.
 */
export const TRACK_CONTROL_TYPES = ['knob', 'slider', 'solo', 'mute', 'rec'] as const;

/**
 * Valid control type names for transport section.
 */
export const TRANSPORT_CONTROL_TYPES = [
  'rewind',
  'forward',
  'stop',
  'play',
  'record',
  'cycle',
  'track_left',
  'track_right',
  'marker_set',
  'marker_left',
  'marker_right',
] as const;

/**
 * Button control types that default to toggle behavior.
 */
export const DEFAULT_TOGGLE_CONTROLS = ['solo', 'mute'] as const;

/**
 * Button control types that default to momentary behavior.
 */
export const DEFAULT_MOMENTARY_CONTROLS = [
  'rec',
  'rewind',
  'forward',
  'stop',
  'play',
  'record',
  'cycle',
  'track_left',
  'track_right',
  'marker_set',
  'marker_left',
  'marker_right',
] as const;

/**
 * All LED CC numbers combined for quick lookup.
 * Note: This duplicates LED_CC_NUMBERS in types.ts but is kept here
 * for use within constants.ts utility functions.
 */
const ALL_LED_CCS: readonly number[] = [
  ...LED_CC.SOLO,
  ...LED_CC.MUTE,
  ...LED_CC.REC,
  ...LED_CC.TRANSPORT,
];

/**
 * Check if a CC number corresponds to a button with an LED.
 */
export function hasLedCC(cc: number): boolean {
  return ALL_LED_CCS.includes(cc);
}

/**
 * Check if a control type is a button (has on/off states).
 */
export function isButtonControl(controlType: string): boolean {
  return (
    controlType === 'solo' ||
    controlType === 'mute' ||
    controlType === 'rec' ||
    TRANSPORT_CONTROL_TYPES.includes(controlType as typeof TRANSPORT_CONTROL_TYPES[number])
  );
}

/**
 * Check if a control type is continuous (knob or slider).
 */
export function isContinuousControl(controlType: string): boolean {
  return controlType === 'knob' || controlType === 'slider';
}

/**
 * Get the default button behavior for a control type.
 */
export function getDefaultBehavior(controlType: string): 'toggle' | 'momentary' {
  if (DEFAULT_TOGGLE_CONTROLS.includes(controlType as typeof DEFAULT_TOGGLE_CONTROLS[number])) {
    return 'toggle';
  }
  return 'momentary';
}

/**
 * Derives the control type identifier from a hardware CC number.
 *
 * Maps raw CC numbers from the nanoKONTROL2 hardware to semantic
 * control type strings like "track1.slider" or "transport.play".
 * This enables GUI updates even when no mapping preset is loaded.
 *
 * @param cc - The CC number from hardware (0-127)
 * @returns Control type string (e.g., "track1.knob", "transport.play") or null if not recognized
 */
export function deriveControlTypeFromCC(cc: number): string | null {
  // Check knobs (CC 16-23 -> track1-8.knob)
  const knobIndex = HARDWARE_CC.KNOBS.indexOf(cc as typeof HARDWARE_CC.KNOBS[number]);
  if (knobIndex !== -1) {
    return `track${knobIndex + 1}.knob`;
  }

  // Check sliders (CC 0-7 -> track1-8.slider)
  const sliderIndex = HARDWARE_CC.SLIDERS.indexOf(cc as typeof HARDWARE_CC.SLIDERS[number]);
  if (sliderIndex !== -1) {
    return `track${sliderIndex + 1}.slider`;
  }

  // Check solo buttons (CC 32-39 -> track1-8.solo)
  const soloIndex = HARDWARE_CC.SOLO.indexOf(cc as typeof HARDWARE_CC.SOLO[number]);
  if (soloIndex !== -1) {
    return `track${soloIndex + 1}.solo`;
  }

  // Check mute buttons (CC 48-55 -> track1-8.mute)
  const muteIndex = HARDWARE_CC.MUTE.indexOf(cc as typeof HARDWARE_CC.MUTE[number]);
  if (muteIndex !== -1) {
    return `track${muteIndex + 1}.mute`;
  }

  // Check rec buttons (CC 64-71 -> track1-8.rec)
  const recIndex = HARDWARE_CC.REC.indexOf(cc as typeof HARDWARE_CC.REC[number]);
  if (recIndex !== -1) {
    return `track${recIndex + 1}.rec`;
  }

  // Check transport buttons
  const transportEntries = Object.entries(HARDWARE_CC.TRANSPORT) as [string, number][];
  for (const [name, transportCC] of transportEntries) {
    if (cc === transportCC) {
      // Convert PLAY -> play, TRACK_LEFT -> track_left
      return `transport.${name.toLowerCase()}`;
    }
  }

  // CC not recognized as a nanoKONTROL2 control
  return null;
}
