/**
 * Config Preset Tests (Phase 6)
 *
 * Validates the device-aware config parser extensions and preset creation
 * factories introduced in Phase 5. Covers:
 * - Parsing configs with [device akai-midimix] header
 * - Parsing note:N syntax for Note-based controls
 * - Backward compatibility: configs without [device] header default to nanokontrol2
 * - createDefaultMappingForDevice() for both nK2 and MidiMix
 * - createSamplePresets() includes MidiMix factory presets
 * - Unknown device ID throws an error
 */

import { describe, it, expect } from 'vitest';
import { parseMapping, parseMappingOrThrow, ParseError } from '../src/config-parser.js';
import {
  createDefaultMappingConfig,
  createDefaultMappingForDevice,
  createSamplePresets,
} from '../src/shared/schemas.js';
import type { MappingConfig, DeviceMappingConfig } from '../src/shared/types.js';

// =============================================================================
// Device Header Parsing
// =============================================================================

describe('device header parsing', () => {
  it('should parse a config with [device akai-midimix] header', () => {
    const content = `
[device akai-midimix]

[track1]
knob1   16 -> 74 ch1 "Filter Cutoff"
knob2   17 -> 71 ch1 "Resonance"
knob3   18 -> 73 ch1 "Attack"
fader   19 -> 19 ch1 "Volume"
mute    note:1 -> note:1 ch1 toggle "Mute"
solo    note:2 -> note:2 ch1 toggle "Solo"
rec_arm note:3 -> note:3 ch1 momentary "Rec Arm"
`;
    const result = parseMapping(content);

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.deviceId).toBe('akai-midimix');
      // The parser pads to 8 tracks minimum for MappingConfig shape
      expect(result.config.tracks).toHaveLength(8);

      const track1 = result.config.tracks[0];
      expect(track1).toBeDefined();
      if (track1) {
        // knob1 maps to the knob slot in MappingConfig
        expect(track1.knob.inputCC).toBe(16);
        expect(track1.knob.outputCC).toBe(74);
        // fader maps to the slider slot
        expect(track1.slider.inputCC).toBe(19);
        expect(track1.slider.outputCC).toBe(19);
        // mute uses note messageType
        expect(track1.mute.inputCC).toBe(1);
        expect(track1.mute.messageType).toBe('note');
        expect(track1.mute.behavior).toBe('toggle');
        // solo uses note messageType
        expect(track1.solo.inputCC).toBe(2);
        expect(track1.solo.messageType).toBe('note');
        // rec_arm maps to the rec slot
        expect(track1.rec.inputCC).toBe(3);
        expect(track1.rec.messageType).toBe('note');
        expect(track1.rec.behavior).toBe('momentary');
      }
    }
  });

  it('should parse note:N syntax correctly for both input and output', () => {
    const content = `
[device akai-midimix]

[track1]
knob1   16 -> 16 ch1
knob2   17 -> 17 ch1
knob3   18 -> 18 ch1
fader   19 -> 19 ch1
mute    note:1 -> note:10 ch1 toggle
solo    note:2 -> note:20 ch1 toggle
rec_arm note:3 -> note:30 ch1 momentary
`;
    const result = parseMapping(content);

    expect(result.success).toBe(true);
    if (result.success) {
      const track1 = result.config.tracks[0];
      expect(track1).toBeDefined();
      if (track1) {
        expect(track1.mute.inputCC).toBe(1);
        expect(track1.mute.outputCC).toBe(10);
        expect(track1.mute.messageType).toBe('note');

        expect(track1.solo.inputCC).toBe(2);
        expect(track1.solo.outputCC).toBe(20);
        expect(track1.solo.messageType).toBe('note');

        expect(track1.rec.inputCC).toBe(3);
        expect(track1.rec.outputCC).toBe(30);
        expect(track1.rec.messageType).toBe('note');
      }
    }
  });

  it('should default to nanokontrol2 when no [device] header is present', () => {
    const content = `
[track1]
knob    16 -> 16 ch1
slider   0 ->  0 ch1
solo    32 -> 32 ch1 toggle
mute    48 -> 48 ch1 toggle
rec     64 -> 64 ch1 momentary
`;
    const result = parseMapping(content);

    // Will fail because only 1 track is defined, but deviceId should default
    if (result.success) {
      expect(result.deviceId).toBe('nanokontrol2');
    } else {
      // The error should be about missing tracks, not about device identification
      expect(result.error).toContain('Track 2 is not defined');
    }
  });

  it('should return error for unknown device in [device] header', () => {
    const content = `
[device unknown-controller]

[track1]
knob 16 -> 16 ch1
`;
    const result = parseMapping(content);

    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error).toContain('Unknown device');
      expect(result.error).toContain('unknown-controller');
    }
  });

  it('should parse [device] header case-insensitively', () => {
    const content = `
[Device Akai-MidiMix]

[track1]
knob1 16 -> 16 ch1
`;
    const result = parseMapping(content);

    // Should recognize the device (the parser lowercases the ID)
    if (result.success) {
      expect(result.deviceId).toBe('akai-midimix');
    }
    // If it fails, it should NOT be because of an unknown device
    if (!result.success) {
      expect(result.error).not.toContain('Unknown device');
    }
  });

  it('should recognize MidiMix master section', () => {
    const content = `
[device akai-midimix]

[track1]
knob1   16 -> 16 ch1
knob2   17 -> 17 ch1
knob3   18 -> 18 ch1
fader   19 -> 19 ch1
mute    note:1 -> note:1 ch1 toggle
solo    note:2 -> note:2 ch1 toggle
rec_arm note:3 -> note:3 ch1 momentary

[master]
master_fader 62 -> 62 ch1
bank_left    note:25 -> note:25 ch1 momentary
bank_right   note:26 -> note:26 ch1 momentary
solo_master  note:27 -> note:27 ch1 toggle
`;
    const result = parseMapping(content);

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.deviceId).toBe('akai-midimix');
    }
  });
});

// =============================================================================
// Default Mapping Factories
// =============================================================================

describe('createDefaultMappingForDevice', () => {
  it('should return MappingConfig for nanokontrol2', () => {
    const mapping = createDefaultMappingForDevice('nanokontrol2');

    // nK2 returns the standard MappingConfig shape
    expect('tracks' in mapping).toBe(true);
    expect('transport' in mapping).toBe(true);

    const nk2Mapping = mapping as MappingConfig;
    expect(nk2Mapping.tracks).toHaveLength(8);

    // Verify it matches the identity mapping from createDefaultMappingConfig
    const identityMapping = createDefaultMappingConfig();
    expect(nk2Mapping.tracks).toEqual(identityMapping.tracks);
    expect(nk2Mapping.transport).toEqual(identityMapping.transport);
  });

  it('should return DeviceMappingConfig for akai-midimix', () => {
    const mapping = createDefaultMappingForDevice('akai-midimix');

    // MidiMix returns a DeviceMappingConfig with deviceId
    expect('deviceId' in mapping).toBe(true);

    const midimixMapping = mapping as DeviceMappingConfig;
    expect(midimixMapping.deviceId).toBe('akai-midimix');

    // Should have 8 tracks keyed by "1" through "8"
    const trackKeys = Object.keys(midimixMapping.tracks);
    expect(trackKeys).toHaveLength(8);
    expect(trackKeys).toContain('1');
    expect(trackKeys).toContain('8');

    // Each track should have controls: knob1, knob2, knob3, fader, mute, solo, rec_arm
    const track1 = midimixMapping.tracks['1'];
    expect(track1).toBeDefined();
    if (track1) {
      expect('knob1' in track1).toBe(true);
      expect('knob2' in track1).toBe(true);
      expect('knob3' in track1).toBe(true);
      expect('fader' in track1).toBe(true);
      expect('mute' in track1).toBe(true);
      expect('solo' in track1).toBe(true);
      expect('rec_arm' in track1).toBe(true);
    }

    // Should have sections (master section)
    expect('sections' in midimixMapping).toBe(true);
    const sectionKeys = Object.keys(midimixMapping.sections);
    expect(sectionKeys).toContain('master');

    const masterSection = midimixMapping.sections['master'];
    expect(masterSection).toBeDefined();
    if (masterSection) {
      expect('master_fader' in masterSection).toBe(true);
      expect('bank_left' in masterSection).toBe(true);
      expect('bank_right' in masterSection).toBe(true);
      expect('solo_master' in masterSection).toBe(true);
    }
  });

  it('should have correct message types in MidiMix mapping', () => {
    const mapping = createDefaultMappingForDevice('akai-midimix') as DeviceMappingConfig;
    const track1 = mapping.tracks['1'];

    expect(track1).toBeDefined();
    if (track1) {
      // Knobs and faders use CC
      expect(track1['knob1']?.messageType).toBe('cc');
      expect(track1['knob2']?.messageType).toBe('cc');
      expect(track1['knob3']?.messageType).toBe('cc');
      expect(track1['fader']?.messageType).toBe('cc');

      // Buttons use Note
      expect(track1['mute']?.messageType).toBe('note');
      expect(track1['solo']?.messageType).toBe('note');
      expect(track1['rec_arm']?.messageType).toBe('note');
    }
  });

  it('should use identity mapping for MidiMix (inputCC equals outputCC)', () => {
    const mapping = createDefaultMappingForDevice('akai-midimix') as DeviceMappingConfig;
    const track1 = mapping.tracks['1'];

    expect(track1).toBeDefined();
    if (track1) {
      // Each control's outputCC should equal its inputCC
      for (const controlName of Object.keys(track1)) {
        const entry = track1[controlName];
        expect(entry).toBeDefined();
        if (entry) {
          expect(entry.outputCC).toBe(entry.inputCC);
        }
      }
    }
  });

  it('should throw for unknown device ID', () => {
    expect(() => createDefaultMappingForDevice('nonexistent-device')).toThrow(
      /Unknown device.*nonexistent-device/
    );
  });
});

// =============================================================================
// Sample Presets
// =============================================================================

describe('createSamplePresets', () => {
  it('should return an array of presets', () => {
    const presets = createSamplePresets();
    expect(Array.isArray(presets)).toBe(true);
    expect(presets.length).toBeGreaterThan(0);
  });

  it('should include nK2 factory presets', () => {
    const presets = createSamplePresets();
    const presetNames = presets.map((preset) => preset.metadata.name);

    expect(presetNames).toContain('Default');
    expect(presetNames).toContain('Synth Lead');
    expect(presetNames).toContain('Drums');
    expect(presetNames).toContain('DAW Transport');
    expect(presetNames).toContain('DJ Mixer');
  });

  it('should include MidiMix factory presets', () => {
    const presets = createSamplePresets();
    const presetNames = presets.map((preset) => preset.metadata.name);

    expect(presetNames).toContain('Default MidiMix');
    expect(presetNames).toContain('MidiMix Synth');
    expect(presetNames).toContain('MidiMix DJ');
  });

  it('should have 8 total factory presets (5 nK2 + 3 MidiMix)', () => {
    const presets = createSamplePresets();
    expect(presets).toHaveLength(8);
  });

  it('should have valid metadata on all presets', () => {
    const presets = createSamplePresets();

    for (const preset of presets) {
      expect(preset.metadata.id).toBeTruthy();
      expect(preset.metadata.name).toBeTruthy();
      expect(preset.metadata.createdAt).toBeTruthy();
      expect(preset.metadata.modifiedAt).toBeTruthy();
      expect(Array.isArray(preset.metadata.tags)).toBe(true);
      expect(typeof preset.metadata.favorite).toBe('boolean');
    }
  });

  it('should have MidiMix presets in the MidiMix Factory group', () => {
    const presets = createSamplePresets();
    const midimixPresets = presets.filter(
      (preset) => preset.metadata.group === 'MidiMix Factory'
    );

    expect(midimixPresets).toHaveLength(3);
  });

  it('should have nK2 presets in the Factory group', () => {
    const presets = createSamplePresets();
    const nk2Presets = presets.filter(
      (preset) => preset.metadata.group === 'Factory'
    );

    expect(nk2Presets).toHaveLength(5);
  });

  it('should have valid mapping structure in MidiMix presets', () => {
    const presets = createSamplePresets();
    const midimixPresets = presets.filter(
      (preset) => preset.metadata.group === 'MidiMix Factory'
    );

    for (const preset of midimixPresets) {
      // MidiMix presets use the nK2-shaped MappingConfig for compatibility
      expect(preset.mapping.tracks).toHaveLength(8);
      expect(preset.mapping.transport).toBeDefined();

      // Buttons in MidiMix presets should have note messageType
      const track1 = preset.mapping.tracks[0];
      expect(track1).toBeDefined();
      if (track1) {
        expect(track1.mute.messageType).toBe('note');
        expect(track1.solo.messageType).toBe('note');
        expect(track1.rec.messageType).toBe('note');
      }
    }
  });
});

// =============================================================================
// createDefaultMappingConfig (nK2 identity mapping)
// =============================================================================

describe('createDefaultMappingConfig', () => {
  it('should return a MappingConfig with 8 tracks', () => {
    const mapping = createDefaultMappingConfig();
    expect(mapping.tracks).toHaveLength(8);
  });

  it('should have a transport section with 11 controls', () => {
    const mapping = createDefaultMappingConfig();
    const transportKeys = Object.keys(mapping.transport);
    expect(transportKeys).toHaveLength(11);
  });

  it('should use identity mapping (inputCC equals outputCC for all controls)', () => {
    const mapping = createDefaultMappingConfig();

    for (const track of mapping.tracks) {
      expect(track.knob.inputCC).toBe(track.knob.outputCC);
      expect(track.slider.inputCC).toBe(track.slider.outputCC);
      expect(track.solo.inputCC).toBe(track.solo.outputCC);
      expect(track.mute.inputCC).toBe(track.mute.outputCC);
      expect(track.rec.inputCC).toBe(track.rec.outputCC);
    }

    const transport = mapping.transport;
    expect(transport.play.inputCC).toBe(transport.play.outputCC);
    expect(transport.stop.inputCC).toBe(transport.stop.outputCC);
    expect(transport.rewind.inputCC).toBe(transport.rewind.outputCC);
    expect(transport.forward.inputCC).toBe(transport.forward.outputCC);
    expect(transport.record.inputCC).toBe(transport.record.outputCC);
    expect(transport.cycle.inputCC).toBe(transport.cycle.outputCC);
  });

  it('should have all controls on channel 1', () => {
    const mapping = createDefaultMappingConfig();

    for (const track of mapping.tracks) {
      expect(track.knob.channel).toBe(1);
      expect(track.slider.channel).toBe(1);
      expect(track.solo.channel).toBe(1);
      expect(track.mute.channel).toBe(1);
      expect(track.rec.channel).toBe(1);
    }
  });
});
