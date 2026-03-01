/**
 * Nord Drum 2 CC Reference Tests
 *
 * Validates the structural correctness of the Nord Drum 2 MIDI CC constants.
 * Ensures all values fall within valid MIDI ranges, match the hardware
 * specification, and contain no accidental duplicates within sections.
 */

import { describe, it, expect } from 'vitest';
import {
  NORD_DRUM_2_CC,
  NORD_DRUM_2_FX_CC,
  NORD_DRUM_2_CHANNELS,
} from '../src/shared/nord-drum-2-cc.js';

// =============================================================================
// Helpers
// =============================================================================

/** Minimum valid MIDI CC number */
const MIDI_CC_MIN = 0;

/** Maximum valid MIDI CC number */
const MIDI_CC_MAX = 127;

/** Minimum valid MIDI channel (1-indexed user-facing convention) */
const MIDI_CHANNEL_MIN = 1;

/** Maximum valid MIDI channel */
const MIDI_CHANNEL_MAX = 16;

/**
 * Extracts all numeric values from an object (one level deep).
 * Used to collect CC numbers from a single section like TONE or NOISE.
 */
function extractCCValues(section: Record<string, number>): number[] {
  return Object.values(section);
}

/**
 * Checks that all values in an array are unique.
 * Returns the first duplicate found, or null if all values are unique.
 */
function findFirstDuplicate(values: number[]): number | null {
  const seen = new Set<number>();
  for (const value of values) {
    if (seen.has(value)) {
      return value;
    }
    seen.add(value);
  }
  return null;
}

// =============================================================================
// Object Structure Tests
// =============================================================================

describe('NORD_DRUM_2_CC structure', () => {
  it('should have exactly the four expected section keys', () => {
    const sectionKeys = Object.keys(NORD_DRUM_2_CC).sort();
    expect(sectionKeys).toEqual(['CLICK', 'MIX', 'NOISE', 'TONE']);
  });

  it('should have 12 parameters in TONE section', () => {
    const toneKeys = Object.keys(NORD_DRUM_2_CC.TONE);
    expect(toneKeys).toHaveLength(12);
  });

  it('should have 9 parameters in NOISE section', () => {
    const noiseKeys = Object.keys(NORD_DRUM_2_CC.NOISE);
    expect(noiseKeys).toHaveLength(9);
  });

  it('should have 2 parameters in CLICK section', () => {
    const clickKeys = Object.keys(NORD_DRUM_2_CC.CLICK);
    expect(clickKeys).toHaveLength(2);
  });

  it('should have 3 parameters in MIX section', () => {
    const mixKeys = Object.keys(NORD_DRUM_2_CC.MIX);
    expect(mixKeys).toHaveLength(3);
  });
});

describe('NORD_DRUM_2_FX_CC structure', () => {
  it('should have exactly 6 FX parameters', () => {
    const fxKeys = Object.keys(NORD_DRUM_2_FX_CC);
    expect(fxKeys).toHaveLength(6);
  });
});

// =============================================================================
// MIDI Range Validation
// =============================================================================

describe('MIDI CC range validation', () => {
  describe('per-pad CC values', () => {
    const allSections = {
      TONE: NORD_DRUM_2_CC.TONE,
      NOISE: NORD_DRUM_2_CC.NOISE,
      CLICK: NORD_DRUM_2_CC.CLICK,
      MIX: NORD_DRUM_2_CC.MIX,
    };

    for (const [sectionName, section] of Object.entries(allSections)) {
      it(`should have all ${sectionName} CCs in valid range (${MIDI_CC_MIN}-${MIDI_CC_MAX})`, () => {
        const ccValues = extractCCValues(section);
        for (const ccValue of ccValues) {
          expect(ccValue).toBeGreaterThanOrEqual(MIDI_CC_MIN);
          expect(ccValue).toBeLessThanOrEqual(MIDI_CC_MAX);
        }
      });
    }
  });

  describe('global FX CC values', () => {
    it('should have all FX CCs in valid range (0-127)', () => {
      const fxValues = extractCCValues(NORD_DRUM_2_FX_CC);
      for (const ccValue of fxValues) {
        expect(ccValue).toBeGreaterThanOrEqual(MIDI_CC_MIN);
        expect(ccValue).toBeLessThanOrEqual(MIDI_CC_MAX);
      }
    });
  });
});

// =============================================================================
// Duplicate Detection
// =============================================================================

describe('no duplicate CCs within sections', () => {
  it('should have unique CCs in TONE section', () => {
    const toneCCs = extractCCValues(NORD_DRUM_2_CC.TONE);
    const duplicate = findFirstDuplicate(toneCCs);
    expect(duplicate).toBeNull();
  });

  it('should have unique CCs in NOISE section', () => {
    const noiseCCs = extractCCValues(NORD_DRUM_2_CC.NOISE);
    const duplicate = findFirstDuplicate(noiseCCs);
    expect(duplicate).toBeNull();
  });

  it('should have unique CCs in CLICK section', () => {
    const clickCCs = extractCCValues(NORD_DRUM_2_CC.CLICK);
    const duplicate = findFirstDuplicate(clickCCs);
    expect(duplicate).toBeNull();
  });

  it('should have unique CCs in MIX section', () => {
    const mixCCs = extractCCValues(NORD_DRUM_2_CC.MIX);
    const duplicate = findFirstDuplicate(mixCCs);
    expect(duplicate).toBeNull();
  });

  it('should have unique CCs in FX', () => {
    const fxCCs = extractCCValues(NORD_DRUM_2_FX_CC);
    const duplicate = findFirstDuplicate(fxCCs);
    expect(duplicate).toBeNull();
  });

  it('should have no duplicate CCs across all per-pad sections combined', () => {
    const allPerPadCCs = [
      ...extractCCValues(NORD_DRUM_2_CC.TONE),
      ...extractCCValues(NORD_DRUM_2_CC.NOISE),
      ...extractCCValues(NORD_DRUM_2_CC.CLICK),
      ...extractCCValues(NORD_DRUM_2_CC.MIX),
    ];
    const duplicate = findFirstDuplicate(allPerPadCCs);
    expect(duplicate).toBeNull();
  });
});

// =============================================================================
// Specific CC Value Verification (Hardware Spec Cross-Reference)
// =============================================================================

describe('specific CC values match Nord Drum 2 specification', () => {
  describe('TONE section', () => {
    it('should map Spectra to CC 30', () => {
      expect(NORD_DRUM_2_CC.TONE.SPECTRA).toBe(30);
    });

    it('should map Pitch MSB to CC 31', () => {
      expect(NORD_DRUM_2_CC.TONE.PITCH_MSB).toBe(31);
    });

    it('should map Wave to CC 46', () => {
      expect(NORD_DRUM_2_CC.TONE.WAVE).toBe(46);
    });

    it('should map Decay to CC 50', () => {
      expect(NORD_DRUM_2_CC.TONE.DECAY).toBe(50);
    });

    it('should map Timbre to CC 52', () => {
      expect(NORD_DRUM_2_CC.TONE.TIMBRE).toBe(52);
    });

    it('should map Bend Amount to CC 54', () => {
      expect(NORD_DRUM_2_CC.TONE.BEND_AMOUNT).toBe(54);
    });

    it('should map Bend Time to CC 55', () => {
      expect(NORD_DRUM_2_CC.TONE.BEND_TIME).toBe(55);
    });
  });

  describe('NOISE section', () => {
    it('should map Filter Freq to CC 14', () => {
      expect(NORD_DRUM_2_CC.NOISE.FILTER_FREQ).toBe(14);
    });

    it('should map Resonance to CC 17', () => {
      expect(NORD_DRUM_2_CC.NOISE.RESONANCE).toBe(17);
    });

    it('should map Decay to CC 21', () => {
      expect(NORD_DRUM_2_CC.NOISE.DECAY).toBe(21);
    });
  });

  describe('CLICK section', () => {
    it('should map Level to CC 56', () => {
      expect(NORD_DRUM_2_CC.CLICK.LEVEL).toBe(56);
    });

    it('should map Type to CC 57', () => {
      expect(NORD_DRUM_2_CC.CLICK.TYPE).toBe(57);
    });
  });

  describe('MIX section', () => {
    it('should map Level to CC 7', () => {
      expect(NORD_DRUM_2_CC.MIX.LEVEL).toBe(7);
    });

    it('should map Pan to CC 10', () => {
      expect(NORD_DRUM_2_CC.MIX.PAN).toBe(10);
    });

    it('should map Balance to CC 58', () => {
      expect(NORD_DRUM_2_CC.MIX.BALANCE).toBe(58);
    });
  });

  describe('FX section', () => {
    it('should map Distortion Amount to CC 23', () => {
      expect(NORD_DRUM_2_FX_CC.DIST_AMOUNT).toBe(23);
    });

    it('should map Distortion Type to CC 24', () => {
      expect(NORD_DRUM_2_FX_CC.DIST_TYPE).toBe(24);
    });

    it('should map EQ Frequency to CC 25', () => {
      expect(NORD_DRUM_2_FX_CC.EQ_FREQ).toBe(25);
    });

    it('should map EQ Gain to CC 26', () => {
      expect(NORD_DRUM_2_FX_CC.EQ_GAIN).toBe(26);
    });

    it('should map Echo Feedback to CC 27', () => {
      expect(NORD_DRUM_2_FX_CC.ECHO_FEEDBACK).toBe(27);
    });

    it('should map Echo Amount to CC 28', () => {
      expect(NORD_DRUM_2_FX_CC.ECHO_AMOUNT).toBe(28);
    });
  });
});

// =============================================================================
// Channel Assignment Tests
// =============================================================================

describe('NORD_DRUM_2_CHANNELS', () => {
  it('should assign pads 1-6 to channels 1-6', () => {
    expect(NORD_DRUM_2_CHANNELS.PAD_1).toBe(1);
    expect(NORD_DRUM_2_CHANNELS.PAD_2).toBe(2);
    expect(NORD_DRUM_2_CHANNELS.PAD_3).toBe(3);
    expect(NORD_DRUM_2_CHANNELS.PAD_4).toBe(4);
    expect(NORD_DRUM_2_CHANNELS.PAD_5).toBe(5);
    expect(NORD_DRUM_2_CHANNELS.PAD_6).toBe(6);
  });

  it('should assign global FX to channel 10', () => {
    expect(NORD_DRUM_2_CHANNELS.GLOBAL_FX).toBe(10);
  });

  it('should have all channel values in valid MIDI range (1-16)', () => {
    const channelValues = Object.values(NORD_DRUM_2_CHANNELS);
    for (const channel of channelValues) {
      expect(channel).toBeGreaterThanOrEqual(MIDI_CHANNEL_MIN);
      expect(channel).toBeLessThanOrEqual(MIDI_CHANNEL_MAX);
    }
  });

  it('should have exactly 7 channel entries (6 pads + 1 FX)', () => {
    const channelKeys = Object.keys(NORD_DRUM_2_CHANNELS);
    expect(channelKeys).toHaveLength(7);
  });

  it('should have unique channel assignments for all pads', () => {
    const padChannels = [
      NORD_DRUM_2_CHANNELS.PAD_1,
      NORD_DRUM_2_CHANNELS.PAD_2,
      NORD_DRUM_2_CHANNELS.PAD_3,
      NORD_DRUM_2_CHANNELS.PAD_4,
      NORD_DRUM_2_CHANNELS.PAD_5,
      NORD_DRUM_2_CHANNELS.PAD_6,
    ];
    const duplicate = findFirstDuplicate(padChannels);
    expect(duplicate).toBeNull();
  });

  it('should not have FX channel overlap with any pad channel', () => {
    const padChannels = [
      NORD_DRUM_2_CHANNELS.PAD_1,
      NORD_DRUM_2_CHANNELS.PAD_2,
      NORD_DRUM_2_CHANNELS.PAD_3,
      NORD_DRUM_2_CHANNELS.PAD_4,
      NORD_DRUM_2_CHANNELS.PAD_5,
      NORD_DRUM_2_CHANNELS.PAD_6,
    ];
    expect(padChannels).not.toContain(NORD_DRUM_2_CHANNELS.GLOBAL_FX);
  });
});
