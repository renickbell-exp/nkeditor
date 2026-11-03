/**
 * nkEditor4 External Node.js / Konduktiva JavaScript SDK
 *
 * Provides hardware constants, theme definitions, Nord Drum 2 CC tables,
 * and bidirectional OSC address space constants synchronized with the pure
 * Rust `nkeditor` desktop application (`src/theme.rs` & `src/devices.rs`).
 */

export const YAAE_THEMES = Object.freeze([
  'default',
  'light',
  'dark',
  'solarized_dark',
  'monokai',
  'dracula',
  'nord',
  'gruvbox_dark',
  'light_grayscale',
  'dark_grayscale',
  'high_contrast_bw',
  'high_contrast_color',
  'high_contrast_light',
  'cream_low_contrast',
  'medium_contrast',
]);

export const TWOBOWLS_BACKGROUND_PRESETS = Object.freeze({
  white: '#FFFFFF',
  cream: '#F8F2E4',
  black: '#000000',
  dark_grey: '#181818',
});

export const NANOKONTROL2_SPEC = Object.freeze({
  id: 'nanokontrol2',
  name: 'nanoKONTROL2',
  manufacturer: 'Korg',
  usbVendorId: 0x0944,
  usbProductId: 0x0117,
  trackCount: 8,
  knobs: Object.freeze([16, 17, 18, 19, 20, 21, 22, 23]),
  sliders: Object.freeze([0, 1, 2, 3, 4, 5, 6, 7]),
  solo: Object.freeze([32, 33, 34, 35, 36, 37, 38, 39]),
  mute: Object.freeze([48, 49, 50, 51, 52, 53, 54, 55]),
  rec: Object.freeze([64, 65, 66, 67, 68, 69, 70, 71]),
  transport: Object.freeze({
    play: 41,
    stop: 42,
    rewind: 43,
    forward: 44,
    record: 45,
    cycle: 46,
    track_left: 58,
    track_right: 59,
    marker_set: 60,
    marker_left: 61,
    marker_right: 62,
  }),
  ledCount: 28,
});

export const AKAI_MIDIMIX_SPEC = Object.freeze({
  id: 'akai-midimix',
  name: 'MIDI Mix',
  manufacturer: 'Akai',
  usbVendorId: 0x09e8,
  usbProductId: 0x0031,
  trackCount: 8,
  knobRow1: Object.freeze([16, 20, 24, 28, 46, 50, 54, 58]),
  knobRow2: Object.freeze([17, 21, 25, 29, 47, 51, 55, 59]),
  knobRow3: Object.freeze([18, 22, 26, 30, 48, 52, 56, 60]),
  faders: Object.freeze([19, 23, 27, 31, 49, 53, 57, 61]),
  muteNotes: Object.freeze([1, 4, 7, 10, 13, 16, 19, 22]),
  soloNotes: Object.freeze([2, 5, 8, 11, 14, 17, 20, 23]),
  recArmNotes: Object.freeze([3, 6, 9, 12, 15, 18, 21, 24]),
  master: Object.freeze({
    master_fader: 62,
    bank_left: 25,
    bank_right: 26,
    solo_master: 27,
  }),
  ledCount: 24,
});

export const NORD_DRUM_2_SPEC = Object.freeze({
  tone: Object.freeze({
    SPECTRA: 30,
    PITCH_MSB: 31,
    WAVE: 46,
    TIMBRE_DECAY: 47,
    PUNCH: 48,
    DECAY_TYPE: 49,
    DECAY: 50,
    DECAY_LO: 51,
    TIMBRE: 52,
    TIMBRE_ENV: 53,
    BEND_AMOUNT: 54,
    BEND_TIME: 55,
  }),
  noise: Object.freeze({
    FILTER_FREQ: 14,
    FILTER_TYPE: 15,
    FILTER_ENV: 16,
    RESONANCE: 17,
    ATTACK_RATE: 18,
    ATK_MODE: 19,
    DECAY_TYPE: 20,
    DECAY: 21,
    DECAY_LO: 22,
  }),
  click: Object.freeze({
    LEVEL: 56,
    TYPE: 57,
  }),
  mix: Object.freeze({
    LEVEL: 7,
    PAN: 10,
    BALANCE: 58,
  }),
  fx: Object.freeze({
    DIST_AMOUNT: 23,
    DIST_TYPE: 24,
    EQ_FREQ: 25,
    EQ_GAIN: 26,
    ECHO_FEEDBACK: 27,
    ECHO_AMOUNT: 28,
  }),
  channels: Object.freeze({
    PAD_1: 1,
    PAD_2: 2,
    PAD_3: 3,
    PAD_4: 4,
    PAD_5: 5,
    PAD_6: 6,
    GLOBAL_FX: 10,
  }),
});

export const OSC_ENDPOINTS = Object.freeze({
  status: '/nkeditor/status',
  deviceList: '/nkeditor/device/list',
  deviceSelect: '/nkeditor/device/select',
  presetLoad: '/nkeditor/preset/load',
  presetSave: '/nkeditor/preset/save',
  controlSet: '/nkeditor/control/set',
  ledSet: '/nkeditor/led/set',
  themeSet: '/nkeditor/theme/set',
  op8State: '/nkeditor/op8/state',
  op8Form: '/nkeditor/op8/form',
  op8Timbre: '/nkeditor/op8/timbre',
  op8SnapshotSave: '/nkeditor/op8/snapshot/save',
});

/**
 * Formats a canonical USB-port-independent HardwareFingerprint string:
 * `vendor_id:product_id:serial:card_name` (e.g. `0944:0117::nanoKONTROL2`).
 */
export function formatHardwareFingerprint({
  vendorId = '0000',
  productId = '0000',
  serial = '',
  cardName = '',
} = {}) {
  const v = String(vendorId || '0000').trim().toLowerCase();
  const p = String(productId || '0000').trim().toLowerCase();
  const s = String(serial || '').trim();
  const c = String(cardName || '').trim();
  return `${v}:${p}:${s}:${c}`;
}

/**
 * Parses a canonical `vendor_id:product_id:serial:card_name` string into its components.
 */
export function parseHardwareFingerprint(str) {
  const parts = String(str).trim().split(':');
  if (parts.length < 4) {
    throw new Error(`Invalid HardwareFingerprint: ${str}`);
  }
  const [vendorId, productId, serial, ...rest] = parts;
  return {
    vendorId: vendorId.toLowerCase(),
    productId: productId.toLowerCase(),
    serial,
    cardName: rest.join(':'),
    usbId: `${vendorId.toLowerCase()}:${productId.toLowerCase()}`,
  };
}

export const RESPONSE_CURVES = Object.freeze(['linear', 'exponential', 'logarithmic', 's_curve']);

export const SOFT_TAKEOVER_MODES = Object.freeze(['catch', 'jump', 'pickup']);

/**
 * Mirrors `ResponseCurve::transform_value` in `src/mapping_engine.rs`.
 */
export function transformContinuousValue(
  rawValue,
  { minValue = 0, maxValue = 127, invert = false, curve = 'linear' } = {}
) {
  const clamped = Math.max(0, Math.min(127, Number(rawValue) || 0));
  let t = clamped / 127.0;
  if (invert) {
    t = 1.0 - t;
  }
  let curved = t;
  if (curve === 'exponential') {
    curved = t * t;
  } else if (curve === 'logarithmic') {
    curved = Math.log(1.0 + 9.0 * t) / Math.log(10.0);
  } else if (curve === 's_curve') {
    curved = t * t * (3.0 - 2.0 * t);
  }
  const low = Math.min(minValue, maxValue);
  const high = Math.max(minValue, maxValue);
  return Math.round(Math.max(0, Math.min(127, low + curved * (high - low))));
}


