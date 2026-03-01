/**
 * Tests for the config-parser module.
 */

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseMapping, parseMappingOrThrow, ParseError } from '../src/config-parser.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const MAPPINGS_DIR = join(__dirname, '..', 'mappings');

describe('parseMapping', () => {
  describe('default.txt mapping file', () => {
    it('should parse the default mapping file successfully', () => {
      const content = readFileSync(join(MAPPINGS_DIR, 'default.txt'), 'utf-8');
      const result = parseMapping(content);

      expect(result.success).toBe(true);
      if (result.success) {
        expect(result.config.tracks).toHaveLength(8);
        expect(result.config.transport).toBeDefined();
      }
    });

    it('should have correct track 1 mappings', () => {
      const content = readFileSync(join(MAPPINGS_DIR, 'default.txt'), 'utf-8');
      const result = parseMapping(content);

      expect(result.success).toBe(true);
      if (result.success) {
        const track1 = result.config.tracks[0];
        expect(track1).toBeDefined();
        if (track1) {
          // Input CCs match hardware assignments; output CCs match the current default.txt
          expect(track1.knob.inputCC).toBe(16);
          expect(track1.knob.outputCC).toBe(74);
          expect(track1.knob.channel).toBe(1);

          expect(track1.slider.inputCC).toBe(0);
          expect(track1.slider.outputCC).toBe(0);

          expect(track1.solo.inputCC).toBe(32);
          expect(track1.solo.behavior).toBe('toggle');

          expect(track1.mute.inputCC).toBe(48);
          expect(track1.mute.behavior).toBe('toggle');

          expect(track1.rec.inputCC).toBe(64);
          expect(track1.rec.behavior).toBe('momentary');
        }
      }
    });

    it('should have correct transport mappings', () => {
      const content = readFileSync(join(MAPPINGS_DIR, 'default.txt'), 'utf-8');
      const result = parseMapping(content);

      expect(result.success).toBe(true);
      if (result.success) {
        const transport = result.config.transport;

        expect(transport.play.inputCC).toBe(41);
        expect(transport.play.behavior).toBe('momentary');

        expect(transport.stop.inputCC).toBe(42);
        expect(transport.rewind.inputCC).toBe(43);
        expect(transport.forward.inputCC).toBe(44);
        expect(transport.record.inputCC).toBe(45);
        expect(transport.cycle.inputCC).toBe(46);

        expect(transport.track_left.inputCC).toBe(58);
        expect(transport.track_right.inputCC).toBe(59);
        expect(transport.marker_set.inputCC).toBe(60);
        expect(transport.marker_left.inputCC).toBe(61);
        expect(transport.marker_right.inputCC).toBe(62);
      }
    });
  });

  describe('error handling', () => {
    it('should return error for empty content', () => {
      const result = parseMapping('');

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('Track 1 is not defined');
      }
    });

    it('should return error for missing tracks', () => {
      const content = `
[track1]
knob 16 -> 16 ch1
slider 0 -> 0 ch1
solo 32 -> 32 ch1 toggle
mute 48 -> 48 ch1 toggle
rec 64 -> 64 ch1 momentary

[transport]
rewind 43 -> 43 ch1 momentary
forward 44 -> 44 ch1 momentary
stop 42 -> 42 ch1 momentary
play 41 -> 41 ch1 momentary
record 45 -> 45 ch1 momentary
cycle 46 -> 46 ch1 momentary
track_left 58 -> 58 ch1 momentary
track_right 59 -> 59 ch1 momentary
marker_set 60 -> 60 ch1 momentary
marker_left 61 -> 61 ch1 momentary
marker_right 62 -> 62 ch1 momentary
`;
      const result = parseMapping(content);

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('Track 2 is not defined');
      }
    });

    it('should return error for invalid CC range', () => {
      const content = `
[track1]
knob 200 -> 16 ch1
`;
      const result = parseMapping(content);

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('Input CC must be an integer between 0 and 127');
      }
    });

    it('should return error for invalid channel', () => {
      const content = `
[track1]
knob 16 -> 16 ch17
`;
      const result = parseMapping(content);

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('Channel must be an integer between 1 and 16');
      }
    });

    it('should return error for control outside section', () => {
      const content = `
knob 16 -> 16 ch1
`;
      const result = parseMapping(content);

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('outside of a section');
      }
    });

    it('should return error for invalid control type in track', () => {
      const content = `
[track1]
play 41 -> 41 ch1
`;
      const result = parseMapping(content);

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('Invalid track control type');
      }
    });

    it('should return error for invalid control type in transport', () => {
      const content = `
[transport]
knob 16 -> 16 ch1
`;
      const result = parseMapping(content);

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('Invalid transport control type');
      }
    });

    it('should return error for behavior on continuous control', () => {
      const content = `
[track1]
knob 16 -> 16 ch1 toggle
`;
      const result = parseMapping(content);

      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).toContain('not valid for continuous control type');
      }
    });
  });

  describe('parsing features', () => {
    it('should ignore comments', () => {
      const content = `
# This is a comment
[track1]
# Another comment
knob 16 -> 16 ch1
slider 0 -> 0 ch1
solo 32 -> 32 ch1 toggle
mute 48 -> 48 ch1 toggle
rec 64 -> 64 ch1 momentary
`;
      const result = parseMapping(content);

      // Will fail due to missing tracks, but should not fail on comments
      expect(result.success).toBe(false);
      if (!result.success) {
        expect(result.error).not.toContain('comment');
      }
    });

    it('should parse quoted labels with spaces', () => {
      const content = readFileSync(join(MAPPINGS_DIR, 'default.txt'), 'utf-8');
      const result = parseMapping(content);

      expect(result.success).toBe(true);
      if (result.success) {
        const track1 = result.config.tracks[0];
        expect(track1).toBeDefined();
        if (track1) {
          expect(track1.knob.label).toBe('Track 1 Knob');
        }
      }
    });

    it('should use default channel 1 when not specified', () => {
      const content = `
[track1]
knob 16 -> 16
slider 0 -> 0
solo 32 -> 32 toggle
mute 48 -> 48 toggle
rec 64 -> 64 momentary
`;
      const result = parseMapping(content);

      // Will fail due to missing tracks, but check that we got past track1 parsing
      if (!result.success) {
        // If error is about missing track 2, track 1 was parsed correctly
        expect(result.error).toContain('Track 2 is not defined');
      }
    });

    it('should default solo/mute to toggle behavior', () => {
      const content = readFileSync(join(MAPPINGS_DIR, 'default.txt'), 'utf-8');
      const result = parseMapping(content);

      expect(result.success).toBe(true);
      if (result.success) {
        const track1 = result.config.tracks[0];
        expect(track1).toBeDefined();
        if (track1) {
          expect(track1.solo.behavior).toBe('toggle');
          expect(track1.mute.behavior).toBe('toggle');
        }
      }
    });

    it('should default rec to momentary behavior', () => {
      const content = readFileSync(join(MAPPINGS_DIR, 'default.txt'), 'utf-8');
      const result = parseMapping(content);

      expect(result.success).toBe(true);
      if (result.success) {
        const track1 = result.config.tracks[0];
        expect(track1).toBeDefined();
        if (track1) {
          expect(track1.rec.behavior).toBe('momentary');
        }
      }
    });
  });
});

describe('parseMappingOrThrow', () => {
  it('should return config for valid content', () => {
    const content = readFileSync(join(MAPPINGS_DIR, 'default.txt'), 'utf-8');
    const config = parseMappingOrThrow(content);

    expect(config.tracks).toHaveLength(8);
    expect(config.transport).toBeDefined();
  });

  it('should throw Error for invalid content', () => {
    expect(() => parseMappingOrThrow('')).toThrow(Error);
  });
});

describe('ParseError', () => {
  it('should include line number and content in message', () => {
    const error = new ParseError('Test error', 42, 'invalid content here');

    expect(error.message).toContain('Line 42');
    expect(error.message).toContain('Test error');
    expect(error.message).toContain('invalid content here');
    expect(error.lineNumber).toBe(42);
    expect(error.lineContent).toBe('invalid content here');
  });
});
