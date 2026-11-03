import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  YAAE_THEMES,
  TWOBOWLS_BACKGROUND_PRESETS,
  NANOKONTROL2_SPEC,
  AKAI_MIDIMIX_SPEC,
  NORD_DRUM_2_SPEC,
  OSC_ENDPOINTS,
  RESPONSE_CURVES,
  SOFT_TAKEOVER_MODES,
  formatHardwareFingerprint,
  parseHardwareFingerprint,
  transformContinuousValue,
} from './nkeditor-sdk.js';

describe('nkEditor4 Companion JavaScript SDK (Epochs 1-3)', () => {
  it('supports continuous response curves, min/max scaling, and polarity inversion matching Rust mapping_engine', () => {
    assert.deepEqual(RESPONSE_CURVES, ['linear', 'exponential', 'logarithmic', 's_curve']);
    assert.deepEqual(SOFT_TAKEOVER_MODES, ['catch', 'jump', 'pickup']);
    assert.equal(transformContinuousValue(0, { minValue: 20, maxValue: 100 }), 20);
    assert.equal(transformContinuousValue(127, { minValue: 20, maxValue: 100 }), 100);
    assert.equal(transformContinuousValue(0, { minValue: 20, maxValue: 100, invert: true }), 100);
    assert.equal(transformContinuousValue(64, { curve: 's_curve' }), 64);
    assert.ok(transformContinuousValue(64, { curve: 'exponential' }) < 64);
    assert.ok(transformContinuousValue(64, { curve: 'logarithmic' }) > 64);
  });
  it('formats and parses USB-port-independent HardwareFingerprints', () => {
    const fp = formatHardwareFingerprint({
      vendorId: '0944',
      productId: '0117',
      serial: '',
      cardName: 'nanoKONTROL2',
    });
    assert.equal(fp, '0944:0117::nanoKONTROL2');
    const parsed = parseHardwareFingerprint(fp);
    assert.deepEqual(parsed, {
      vendorId: '0944',
      productId: '0117',
      serial: '',
      cardName: 'nanoKONTROL2',
      usbId: '0944:0117',
    });
  });

  it('exposes all 15 yaae-theme preference identifiers and 4 twoBowls background presets', () => {
    assert.equal(YAAE_THEMES.length, 15);
    assert.ok(YAAE_THEMES.includes('default'));
    assert.ok(YAAE_THEMES.includes('solarized_dark'));
    assert.ok(YAAE_THEMES.includes('high_contrast_bw'));
    assert.ok(YAAE_THEMES.includes('cream_low_contrast'));
    assert.ok(YAAE_THEMES.includes('medium_contrast'));

    assert.deepEqual(TWOBOWLS_BACKGROUND_PRESETS, {
      white: '#FFFFFF',
      cream: '#F8F2E4',
      black: '#000000',
      dark_grey: '#181818',
    });
  });

  it('matches Korg nanoKONTROL2 CC, transport, and LED hardware layout', () => {
    assert.equal(NANOKONTROL2_SPEC.id, 'nanokontrol2');
    assert.equal(NANOKONTROL2_SPEC.usbVendorId, 0x0944);
    assert.equal(NANOKONTROL2_SPEC.usbProductId, 0x0117);
    assert.equal(NANOKONTROL2_SPEC.trackCount, 8);
    assert.deepEqual(NANOKONTROL2_SPEC.knobs, [16, 17, 18, 19, 20, 21, 22, 23]);
    assert.deepEqual(NANOKONTROL2_SPEC.sliders, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert.deepEqual(NANOKONTROL2_SPEC.solo, [32, 33, 34, 35, 36, 37, 38, 39]);
    assert.deepEqual(NANOKONTROL2_SPEC.mute, [48, 49, 50, 51, 52, 53, 54, 55]);
    assert.deepEqual(NANOKONTROL2_SPEC.rec, [64, 65, 66, 67, 68, 69, 70, 71]);
    assert.equal(Object.keys(NANOKONTROL2_SPEC.transport).length, 11);
    assert.equal(NANOKONTROL2_SPEC.ledCount, 28);
  });

  it('matches Akai MIDI Mix CC/Note and LED hardware layout', () => {
    assert.equal(AKAI_MIDIMIX_SPEC.id, 'akai-midimix');
    assert.equal(AKAI_MIDIMIX_SPEC.trackCount, 8);
    assert.deepEqual(AKAI_MIDIMIX_SPEC.knobRow1, [16, 20, 24, 28, 46, 50, 54, 58]);
    assert.deepEqual(AKAI_MIDIMIX_SPEC.knobRow2, [17, 21, 25, 29, 47, 51, 55, 59]);
    assert.deepEqual(AKAI_MIDIMIX_SPEC.knobRow3, [18, 22, 26, 30, 48, 52, 56, 60]);
    assert.deepEqual(AKAI_MIDIMIX_SPEC.faders, [19, 23, 27, 31, 49, 53, 57, 61]);
    assert.deepEqual(AKAI_MIDIMIX_SPEC.muteNotes, [1, 4, 7, 10, 13, 16, 19, 22]);
    assert.deepEqual(AKAI_MIDIMIX_SPEC.soloNotes, [2, 5, 8, 11, 14, 17, 20, 23]);
    assert.deepEqual(AKAI_MIDIMIX_SPEC.recArmNotes, [3, 6, 9, 12, 15, 18, 21, 24]);
    assert.equal(AKAI_MIDIMIX_SPEC.master.master_fader, 62);
    assert.equal(AKAI_MIDIMIX_SPEC.ledCount, 24);
  });

  it('matches Nord Drum 2 Desktop synthesis and global FX CC constants', () => {
    assert.equal(NORD_DRUM_2_SPEC.tone.SPECTRA, 30);
    assert.equal(NORD_DRUM_2_SPEC.tone.DECAY, 50);
    assert.equal(NORD_DRUM_2_SPEC.noise.FILTER_FREQ, 14);
    assert.equal(NORD_DRUM_2_SPEC.click.LEVEL, 56);
    assert.equal(NORD_DRUM_2_SPEC.mix.LEVEL, 7);
    assert.equal(NORD_DRUM_2_SPEC.fx.DIST_AMOUNT, 23);
    assert.equal(NORD_DRUM_2_SPEC.channels.GLOBAL_FX, 10);
    assert.equal(OSC_ENDPOINTS.status, '/nkeditor/status');
  });
});
