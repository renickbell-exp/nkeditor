/**
 * Regression Tests (Phase 6)
 *
 * Verifies backward compatibility after the multi-device refactoring.
 * Ensures that all pre-existing nK2 constants, types, and APIs remain
 * accessible and correct. These tests act as a safety net against
 * unintended breaking changes to the public surface area.
 */

import { describe, it, expect } from 'vitest';
import {
  HARDWARE_CC,
  LED_CC,
  MIDI_VALUES,
  MIDI_CHANNELS,
  TRACK_CONSTANTS,
  TRACK_CONTROL_TYPES,
  TRANSPORT_CONTROL_TYPES,
  DEFAULT_TOGGLE_CONTROLS,
  DEFAULT_MOMENTARY_CONTROLS,
  hasLedCC,
  isButtonControl,
  isContinuousControl,
  getDefaultBehavior,
  deriveControlTypeFromCC,
} from '../src/shared/constants.js';

import type {
  MappingEntry,
  TrackMapping,
  TransportMapping,
  MappingConfig,
  ConnectionStatus,
  MidiPorts,
  ButtonState,
  LedState,
  ParseResult,
  DeviceMappingConfig,
  CCMessage,
  NanoKontrol2Ports,
  ButtonBehavior,
  ControlType,
  ContinuousControlType,
  ButtonControlType,
  TransportControlType,
  GenericTrackMapping,
  GenericSectionMapping,
  NoteMessage,
} from '../src/shared/types.js';

import {
  LED_CC_NUMBERS,
  hasLed,
  MIDI_VALUE_ON,
  MIDI_VALUE_OFF,
  DEFAULT_VIRTUAL_PORT_NAME,
  DEFAULT_HOTPLUG_INTERVAL_MS,
  HOTPLUG_DEBOUNCE_MS,
} from '../src/shared/types.js';

import { NANOKONTROL2_DEFINITION } from '../src/shared/devices/nanokontrol2.js';
import { createDefaultMappingConfig } from '../src/shared/schemas.js';

// =============================================================================
// HARDWARE_CC Constants Accessibility
// =============================================================================

describe('HARDWARE_CC constants are still accessible', () => {
  it('should export HARDWARE_CC.KNOBS with 8 CC numbers', () => {
    expect(HARDWARE_CC.KNOBS).toHaveLength(8);
    expect(HARDWARE_CC.KNOBS).toEqual([16, 17, 18, 19, 20, 21, 22, 23]);
  });

  it('should export HARDWARE_CC.SLIDERS with 8 CC numbers', () => {
    expect(HARDWARE_CC.SLIDERS).toHaveLength(8);
    expect(HARDWARE_CC.SLIDERS).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
  });

  it('should export HARDWARE_CC.SOLO with 8 CC numbers', () => {
    expect(HARDWARE_CC.SOLO).toHaveLength(8);
    expect(HARDWARE_CC.SOLO).toEqual([32, 33, 34, 35, 36, 37, 38, 39]);
  });

  it('should export HARDWARE_CC.MUTE with 8 CC numbers', () => {
    expect(HARDWARE_CC.MUTE).toHaveLength(8);
    expect(HARDWARE_CC.MUTE).toEqual([48, 49, 50, 51, 52, 53, 54, 55]);
  });

  it('should export HARDWARE_CC.REC with 8 CC numbers', () => {
    expect(HARDWARE_CC.REC).toHaveLength(8);
    expect(HARDWARE_CC.REC).toEqual([64, 65, 66, 67, 68, 69, 70, 71]);
  });

  it('should export HARDWARE_CC.TRANSPORT with all 11 transport CC numbers', () => {
    expect(HARDWARE_CC.TRANSPORT.PLAY).toBe(41);
    expect(HARDWARE_CC.TRANSPORT.STOP).toBe(42);
    expect(HARDWARE_CC.TRANSPORT.REWIND).toBe(43);
    expect(HARDWARE_CC.TRANSPORT.FORWARD).toBe(44);
    expect(HARDWARE_CC.TRANSPORT.RECORD).toBe(45);
    expect(HARDWARE_CC.TRANSPORT.CYCLE).toBe(46);
    expect(HARDWARE_CC.TRANSPORT.TRACK_LEFT).toBe(58);
    expect(HARDWARE_CC.TRANSPORT.TRACK_RIGHT).toBe(59);
    expect(HARDWARE_CC.TRANSPORT.MARKER_SET).toBe(60);
    expect(HARDWARE_CC.TRANSPORT.MARKER_LEFT).toBe(61);
    expect(HARDWARE_CC.TRANSPORT.MARKER_RIGHT).toBe(62);
  });
});

describe('LED_CC constants are still accessible', () => {
  it('should export LED_CC.SOLO', () => {
    expect(LED_CC.SOLO).toEqual([32, 33, 34, 35, 36, 37, 38, 39]);
  });

  it('should export LED_CC.MUTE', () => {
    expect(LED_CC.MUTE).toEqual([48, 49, 50, 51, 52, 53, 54, 55]);
  });

  it('should export LED_CC.REC', () => {
    expect(LED_CC.REC).toEqual([64, 65, 66, 67, 68, 69, 70, 71]);
  });

  it('should export LED_CC.TRANSPORT', () => {
    expect(LED_CC.TRANSPORT).toEqual([41, 42, 45, 46]);
  });
});

// =============================================================================
// nK2 Device Definition Matches HARDWARE_CC Constants
// =============================================================================

describe('nK2 device definition matches HARDWARE_CC constants exactly', () => {
  it('knob MIDI numbers match HARDWARE_CC.KNOBS', () => {
    const knobCCs = NANOKONTROL2_DEFINITION.trackLayouts.map((layout) => {
      const knob = layout.continuousControls.find((c) => c.name === 'knob');
      return knob!.midiNumber;
    });
    expect(knobCCs).toEqual([...HARDWARE_CC.KNOBS]);
  });

  it('slider MIDI numbers match HARDWARE_CC.SLIDERS', () => {
    const sliderCCs = NANOKONTROL2_DEFINITION.trackLayouts.map((layout) => {
      const slider = layout.continuousControls.find((c) => c.name === 'slider');
      return slider!.midiNumber;
    });
    expect(sliderCCs).toEqual([...HARDWARE_CC.SLIDERS]);
  });

  it('solo MIDI numbers match HARDWARE_CC.SOLO', () => {
    const soloCCs = NANOKONTROL2_DEFINITION.trackLayouts.map((layout) => {
      const solo = layout.buttonControls.find((b) => b.name === 'solo');
      return solo!.midiNumber;
    });
    expect(soloCCs).toEqual([...HARDWARE_CC.SOLO]);
  });

  it('mute MIDI numbers match HARDWARE_CC.MUTE', () => {
    const muteCCs = NANOKONTROL2_DEFINITION.trackLayouts.map((layout) => {
      const mute = layout.buttonControls.find((b) => b.name === 'mute');
      return mute!.midiNumber;
    });
    expect(muteCCs).toEqual([...HARDWARE_CC.MUTE]);
  });

  it('rec MIDI numbers match HARDWARE_CC.REC', () => {
    const recCCs = NANOKONTROL2_DEFINITION.trackLayouts.map((layout) => {
      const rec = layout.buttonControls.find((b) => b.name === 'rec');
      return rec!.midiNumber;
    });
    expect(recCCs).toEqual([...HARDWARE_CC.REC]);
  });

  it('transport MIDI numbers match HARDWARE_CC.TRANSPORT', () => {
    const transportSection = NANOKONTROL2_DEFINITION.extraSections.find(
      (s) => s.name === 'transport'
    );
    expect(transportSection).toBeDefined();
    if (!transportSection) return;

    const byName = new Map(
      transportSection.controls.map((c) => [c.name, c.midiNumber])
    );

    expect(byName.get('play')).toBe(HARDWARE_CC.TRANSPORT.PLAY);
    expect(byName.get('stop')).toBe(HARDWARE_CC.TRANSPORT.STOP);
    expect(byName.get('rewind')).toBe(HARDWARE_CC.TRANSPORT.REWIND);
    expect(byName.get('forward')).toBe(HARDWARE_CC.TRANSPORT.FORWARD);
    expect(byName.get('record')).toBe(HARDWARE_CC.TRANSPORT.RECORD);
    expect(byName.get('cycle')).toBe(HARDWARE_CC.TRANSPORT.CYCLE);
    expect(byName.get('track_left')).toBe(HARDWARE_CC.TRANSPORT.TRACK_LEFT);
    expect(byName.get('track_right')).toBe(HARDWARE_CC.TRANSPORT.TRACK_RIGHT);
    expect(byName.get('marker_set')).toBe(HARDWARE_CC.TRANSPORT.MARKER_SET);
    expect(byName.get('marker_left')).toBe(HARDWARE_CC.TRANSPORT.MARKER_LEFT);
    expect(byName.get('marker_right')).toBe(HARDWARE_CC.TRANSPORT.MARKER_RIGHT);
  });
});

// =============================================================================
// Default MappingConfig Shape
// =============================================================================

describe('default MappingConfig shape', () => {
  const mapping = createDefaultMappingConfig();

  it('should have exactly 8 tracks', () => {
    expect(mapping.tracks).toHaveLength(8);
  });

  it('should have 11 transport controls', () => {
    const transportKeys = Object.keys(mapping.transport);
    expect(transportKeys).toHaveLength(11);
    expect(transportKeys).toContain('play');
    expect(transportKeys).toContain('stop');
    expect(transportKeys).toContain('rewind');
    expect(transportKeys).toContain('forward');
    expect(transportKeys).toContain('record');
    expect(transportKeys).toContain('cycle');
    expect(transportKeys).toContain('track_left');
    expect(transportKeys).toContain('track_right');
    expect(transportKeys).toContain('marker_set');
    expect(transportKeys).toContain('marker_left');
    expect(transportKeys).toContain('marker_right');
  });

  it('each track should have knob, slider, solo, mute, rec', () => {
    for (const track of mapping.tracks) {
      expect(track.knob).toBeDefined();
      expect(track.slider).toBeDefined();
      expect(track.solo).toBeDefined();
      expect(track.mute).toBeDefined();
      expect(track.rec).toBeDefined();
    }
  });

  it('each track control should have inputCC, outputCC, channel', () => {
    for (const track of mapping.tracks) {
      for (const controlName of ['knob', 'slider', 'solo', 'mute', 'rec'] as const) {
        const control = track[controlName];
        expect(typeof control.inputCC).toBe('number');
        expect(typeof control.outputCC).toBe('number');
        expect(typeof control.channel).toBe('number');
      }
    }
  });
});

// =============================================================================
// Type Exports Still Exist
// =============================================================================

describe('pre-existing type exports from types.ts', () => {
  // These tests verify at the type level that these types are importable.
  // If any type is removed, the test file will fail to compile.

  it('MappingEntry should be usable as a type', () => {
    const entry: MappingEntry = {
      inputCC: 16,
      outputCC: 74,
      channel: 1,
    };
    expect(entry.inputCC).toBe(16);
  });

  it('TrackMapping should have knob, slider, solo, mute, rec', () => {
    const track: TrackMapping = {
      knob: { inputCC: 0, outputCC: 0, channel: 1 },
      slider: { inputCC: 0, outputCC: 0, channel: 1 },
      solo: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
      mute: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
      rec: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
    };
    expect(track.knob).toBeDefined();
    expect(track.slider).toBeDefined();
    expect(track.solo).toBeDefined();
    expect(track.mute).toBeDefined();
    expect(track.rec).toBeDefined();
  });

  it('TransportMapping should have all 11 controls', () => {
    const transport: TransportMapping = {
      rewind: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      forward: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      stop: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      play: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      record: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      cycle: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
      track_left: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      track_right: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      marker_set: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      marker_left: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      marker_right: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
    };
    expect(Object.keys(transport)).toHaveLength(11);
  });

  it('MappingConfig should have tracks and transport', () => {
    const config: MappingConfig = {
      tracks: [],
      transport: {
        rewind: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        forward: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        stop: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        play: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        record: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        cycle: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'toggle' },
        track_left: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        track_right: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        marker_set: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        marker_left: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
        marker_right: { inputCC: 0, outputCC: 0, channel: 1, behavior: 'momentary' },
      },
    };
    expect(config.tracks).toBeDefined();
    expect(config.transport).toBeDefined();
  });

  it('DeviceMappingConfig should have deviceId, tracks, sections', () => {
    const deviceConfig: DeviceMappingConfig = {
      deviceId: 'test',
      tracks: {},
      sections: {},
    };
    expect(deviceConfig.deviceId).toBe('test');
  });

  it('ConnectionStatus type should accept valid values', () => {
    const statuses: ConnectionStatus[] = ['disconnected', 'connecting', 'connected', 'error'];
    expect(statuses).toHaveLength(4);
  });

  it('ParseResult type should accept success and failure shapes', () => {
    const successResult: ParseResult = {
      success: true,
      config: createDefaultMappingConfig(),
      deviceId: 'nanokontrol2',
    };
    expect(successResult.success).toBe(true);

    const failureResult: ParseResult = {
      success: false,
      error: 'test error',
    };
    expect(failureResult.success).toBe(false);
  });

  it('NoteMessage type should be importable', () => {
    const noteMsg: NoteMessage = { channel: 0, note: 60, velocity: 127 };
    expect(noteMsg.note).toBe(60);
  });

  it('GenericTrackMapping should be a Record of MappingEntry', () => {
    const genericTrack: GenericTrackMapping = {
      knob1: { inputCC: 16, outputCC: 16, channel: 1 },
      fader: { inputCC: 19, outputCC: 19, channel: 1 },
    };
    expect(genericTrack['knob1']?.inputCC).toBe(16);
  });
});

// =============================================================================
// LED_CC_NUMBERS Matches nK2 Device Definition
// =============================================================================

describe('LED_CC_NUMBERS matches nK2 device definition', () => {
  it('should contain exactly the same CC numbers as the nK2 LED definitions', () => {
    const nk2LedMidiNumbers = NANOKONTROL2_DEFINITION.leds.map(
      (led) => led.midiNumber
    );

    // LED_CC_NUMBERS is ordered: solo(8) + mute(8) + rec(8) + transport(4) = 28
    expect(LED_CC_NUMBERS).toHaveLength(28);

    // Every number in LED_CC_NUMBERS should appear in the nK2 LED definitions
    for (const ccNumber of LED_CC_NUMBERS) {
      expect(nk2LedMidiNumbers).toContain(ccNumber);
    }

    // Every nK2 LED MIDI number should appear in LED_CC_NUMBERS
    for (const midiNumber of nk2LedMidiNumbers) {
      expect(LED_CC_NUMBERS as readonly number[]).toContain(midiNumber);
    }
  });
});

// =============================================================================
// hasLed() Function
// =============================================================================

describe('hasLed() function', () => {
  it('should return true for solo button CCs (32-39)', () => {
    for (let cc = 32; cc <= 39; cc++) {
      expect(hasLed(cc)).toBe(true);
    }
  });

  it('should return true for mute button CCs (48-55)', () => {
    for (let cc = 48; cc <= 55; cc++) {
      expect(hasLed(cc)).toBe(true);
    }
  });

  it('should return true for rec button CCs (64-71)', () => {
    for (let cc = 64; cc <= 71; cc++) {
      expect(hasLed(cc)).toBe(true);
    }
  });

  it('should return true for transport LEDs (play=41, stop=42, record=45, cycle=46)', () => {
    expect(hasLed(41)).toBe(true);
    expect(hasLed(42)).toBe(true);
    expect(hasLed(45)).toBe(true);
    expect(hasLed(46)).toBe(true);
  });

  it('should return false for transport buttons without LEDs (rewind=43, forward=44)', () => {
    expect(hasLed(43)).toBe(false);
    expect(hasLed(44)).toBe(false);
  });

  it('should return false for knob CCs (16-23)', () => {
    for (let cc = 16; cc <= 23; cc++) {
      expect(hasLed(cc)).toBe(false);
    }
  });

  it('should return false for slider CCs (0-7)', () => {
    for (let cc = 0; cc <= 7; cc++) {
      expect(hasLed(cc)).toBe(false);
    }
  });

  it('should return false for arbitrary non-assigned CCs', () => {
    expect(hasLed(100)).toBe(false);
    expect(hasLed(127)).toBe(false);
    expect(hasLed(8)).toBe(false);
  });
});

// =============================================================================
// Utility Function Regressions
// =============================================================================

describe('utility function regressions', () => {
  it('MIDI_VALUES should have ON=127, OFF=0', () => {
    expect(MIDI_VALUES.ON).toBe(127);
    expect(MIDI_VALUES.OFF).toBe(0);
    expect(MIDI_VALUES.MIN).toBe(0);
    expect(MIDI_VALUES.MAX).toBe(127);
  });

  it('MIDI_CHANNELS should have MIN=1, MAX=16, DEFAULT=1', () => {
    expect(MIDI_CHANNELS.MIN).toBe(1);
    expect(MIDI_CHANNELS.MAX).toBe(16);
    expect(MIDI_CHANNELS.DEFAULT).toBe(1);
  });

  it('TRACK_CONSTANTS should have COUNT=8', () => {
    expect(TRACK_CONSTANTS.COUNT).toBe(8);
    expect(TRACK_CONSTANTS.FIRST).toBe(1);
    expect(TRACK_CONSTANTS.LAST).toBe(8);
  });

  it('TRACK_CONTROL_TYPES should have 5 entries', () => {
    expect(TRACK_CONTROL_TYPES).toEqual(['knob', 'slider', 'solo', 'mute', 'rec']);
  });

  it('TRANSPORT_CONTROL_TYPES should have 11 entries', () => {
    expect(TRANSPORT_CONTROL_TYPES).toHaveLength(11);
  });

  it('isButtonControl should identify buttons correctly', () => {
    expect(isButtonControl('solo')).toBe(true);
    expect(isButtonControl('mute')).toBe(true);
    expect(isButtonControl('rec')).toBe(true);
    expect(isButtonControl('play')).toBe(true);
    expect(isButtonControl('knob')).toBe(false);
    expect(isButtonControl('slider')).toBe(false);
  });

  it('isContinuousControl should identify continuous controls correctly', () => {
    expect(isContinuousControl('knob')).toBe(true);
    expect(isContinuousControl('slider')).toBe(true);
    expect(isContinuousControl('solo')).toBe(false);
    expect(isContinuousControl('play')).toBe(false);
  });

  it('getDefaultBehavior should return correct defaults', () => {
    expect(getDefaultBehavior('solo')).toBe('toggle');
    expect(getDefaultBehavior('mute')).toBe('toggle');
    expect(getDefaultBehavior('rec')).toBe('momentary');
    expect(getDefaultBehavior('play')).toBe('momentary');
  });

  it('hasLedCC should match hasLed from types.ts', () => {
    // Both functions should agree on all CC numbers
    for (let cc = 0; cc <= 127; cc++) {
      expect(hasLedCC(cc)).toBe(hasLed(cc));
    }
  });

  it('MIDI_VALUE_ON and MIDI_VALUE_OFF from types.ts should be correct', () => {
    expect(MIDI_VALUE_ON).toBe(127);
    expect(MIDI_VALUE_OFF).toBe(0);
  });

  it('DEFAULT_VIRTUAL_PORT_NAME should be nkEditor3 Out', () => {
    expect(DEFAULT_VIRTUAL_PORT_NAME).toBe('nkEditor3 Out');
  });

  it('deriveControlTypeFromCC should map known CCs correctly', () => {
    // Knobs
    expect(deriveControlTypeFromCC(16)).toBe('track1.knob');
    expect(deriveControlTypeFromCC(23)).toBe('track8.knob');

    // Sliders
    expect(deriveControlTypeFromCC(0)).toBe('track1.slider');
    expect(deriveControlTypeFromCC(7)).toBe('track8.slider');

    // Solo
    expect(deriveControlTypeFromCC(32)).toBe('track1.solo');

    // Mute
    expect(deriveControlTypeFromCC(48)).toBe('track1.mute');

    // Rec
    expect(deriveControlTypeFromCC(64)).toBe('track1.rec');

    // Transport
    expect(deriveControlTypeFromCC(41)).toBe('transport.play');
    expect(deriveControlTypeFromCC(42)).toBe('transport.stop');

    // Unknown
    expect(deriveControlTypeFromCC(100)).toBeNull();
  });
});
