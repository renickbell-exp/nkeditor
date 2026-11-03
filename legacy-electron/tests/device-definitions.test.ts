/**
 * Device Definition Tests
 *
 * Validates the structural correctness of all device definitions and ensures
 * they match the actual hardware specifications. Cross-references the nK2
 * definition against the existing HARDWARE_CC and LED_CC constants to prevent
 * drift between the legacy constants and the new device abstraction layer.
 */

import { describe, it, expect } from 'vitest';
import { HARDWARE_CC, LED_CC } from '../src/shared/constants.js';
import { NANOKONTROL2_DEFINITION } from '../src/shared/devices/nanokontrol2.js';
import { AKAI_MIDIMIX_DEFINITION } from '../src/shared/devices/akai-midimix.js';
import {
  getDeviceDefinition,
  getAllDeviceDefinitions,
  identifyDevice,
  getDefaultDevice,
} from '../src/shared/devices/registry.js';
import type {
  ButtonControlDef,
  ContinuousControlDef,
  ControlDef,
} from '../src/shared/devices/device-definition.js';

// =============================================================================
// Helper Functions
// =============================================================================

function isButtonControl(control: ControlDef): control is ButtonControlDef {
  return control.kind === 'button';
}

function isContinuousControl(control: ControlDef): control is ContinuousControlDef {
  return control.kind === 'continuous';
}

// =============================================================================
// nanoKONTROL2 Definition Tests
// =============================================================================

describe('nanoKONTROL2 definition', () => {
  const nk2 = NANOKONTROL2_DEFINITION;

  it('should have the correct device metadata', () => {
    expect(nk2.id).toBe('nanokontrol2');
    expect(nk2.name).toBe('nanoKONTROL2');
    expect(nk2.manufacturer).toBe('Korg');
    expect(nk2.virtualPortName).toBe('nkEditor3 Out');
  });

  it('should have exactly 8 tracks', () => {
    expect(nk2.trackCount).toBe(8);
    expect(nk2.trackLayouts).toHaveLength(8);
  });

  describe('track layouts', () => {
    it('should have exactly 1 knob, 1 slider, and 3 buttons per track', () => {
      for (let trackIndex = 0; trackIndex < 8; trackIndex++) {
        const layout = nk2.trackLayouts[trackIndex]!;

        // 2 continuous controls: knob + slider
        expect(layout.continuousControls).toHaveLength(2);
        expect(layout.buttonControls).toHaveLength(3);

        const knobs = layout.continuousControls.filter((c) => c.visualType === 'knob');
        const sliders = layout.continuousControls.filter((c) => c.visualType === 'slider');
        expect(knobs).toHaveLength(1);
        expect(sliders).toHaveLength(1);

        const buttonNames = layout.buttonControls.map((b) => b.name);
        expect(buttonNames).toContain('solo');
        expect(buttonNames).toContain('mute');
        expect(buttonNames).toContain('rec');
      }
    });

    it('should have all controls using CC messageType', () => {
      for (const layout of nk2.trackLayouts) {
        for (const control of layout.continuousControls) {
          expect(control.messageType).toBe('cc');
        }
        for (const button of layout.buttonControls) {
          expect(button.messageType).toBe('cc');
        }
      }
    });

    it('should have all controls on channel 1', () => {
      for (const layout of nk2.trackLayouts) {
        for (const control of [...layout.continuousControls, ...layout.buttonControls]) {
          expect(control.channel).toBe(1);
        }
      }
    });
  });

  describe('CC number cross-reference with HARDWARE_CC constants', () => {
    it('should match HARDWARE_CC.KNOBS exactly', () => {
      const knobCCs = nk2.trackLayouts.map((layout) => {
        const knob = layout.continuousControls.find((c) => c.visualType === 'knob');
        return knob!.midiNumber;
      });
      expect(knobCCs).toEqual([...HARDWARE_CC.KNOBS]);
    });

    it('should match HARDWARE_CC.SLIDERS exactly', () => {
      const sliderCCs = nk2.trackLayouts.map((layout) => {
        const slider = layout.continuousControls.find((c) => c.visualType === 'slider');
        return slider!.midiNumber;
      });
      expect(sliderCCs).toEqual([...HARDWARE_CC.SLIDERS]);
    });

    it('should match HARDWARE_CC.SOLO exactly', () => {
      const soloCCs = nk2.trackLayouts.map((layout) => {
        const solo = layout.buttonControls.find((b) => b.name === 'solo');
        return solo!.midiNumber;
      });
      expect(soloCCs).toEqual([...HARDWARE_CC.SOLO]);
    });

    it('should match HARDWARE_CC.MUTE exactly', () => {
      const muteCCs = nk2.trackLayouts.map((layout) => {
        const mute = layout.buttonControls.find((b) => b.name === 'mute');
        return mute!.midiNumber;
      });
      expect(muteCCs).toEqual([...HARDWARE_CC.MUTE]);
    });

    it('should match HARDWARE_CC.REC exactly', () => {
      const recCCs = nk2.trackLayouts.map((layout) => {
        const rec = layout.buttonControls.find((b) => b.name === 'rec');
        return rec!.midiNumber;
      });
      expect(recCCs).toEqual([...HARDWARE_CC.REC]);
    });

    it('should match HARDWARE_CC.TRANSPORT CC numbers', () => {
      const transportSection = nk2.extraSections.find((s) => s.name === 'transport')!;
      const transportByName = new Map(
        transportSection.controls.map((c) => [c.name, c.midiNumber]),
      );

      expect(transportByName.get('play')).toBe(HARDWARE_CC.TRANSPORT.PLAY);
      expect(transportByName.get('stop')).toBe(HARDWARE_CC.TRANSPORT.STOP);
      expect(transportByName.get('rewind')).toBe(HARDWARE_CC.TRANSPORT.REWIND);
      expect(transportByName.get('forward')).toBe(HARDWARE_CC.TRANSPORT.FORWARD);
      expect(transportByName.get('record')).toBe(HARDWARE_CC.TRANSPORT.RECORD);
      expect(transportByName.get('cycle')).toBe(HARDWARE_CC.TRANSPORT.CYCLE);
      expect(transportByName.get('track_left')).toBe(HARDWARE_CC.TRANSPORT.TRACK_LEFT);
      expect(transportByName.get('track_right')).toBe(HARDWARE_CC.TRANSPORT.TRACK_RIGHT);
      expect(transportByName.get('marker_set')).toBe(HARDWARE_CC.TRANSPORT.MARKER_SET);
      expect(transportByName.get('marker_left')).toBe(HARDWARE_CC.TRANSPORT.MARKER_LEFT);
      expect(transportByName.get('marker_right')).toBe(HARDWARE_CC.TRANSPORT.MARKER_RIGHT);
    });
  });

  describe('transport section', () => {
    it('should have exactly 11 transport controls', () => {
      const transportSection = nk2.extraSections.find((s) => s.name === 'transport');
      expect(transportSection).toBeDefined();
      expect(transportSection!.controls).toHaveLength(11);
    });

    it('should have all transport controls as buttons', () => {
      const transportSection = nk2.extraSections.find((s) => s.name === 'transport')!;
      for (const control of transportSection.controls) {
        expect(isButtonControl(control)).toBe(true);
      }
    });

    it('should have cycle as toggle and all others as momentary', () => {
      const transportSection = nk2.extraSections.find((s) => s.name === 'transport')!;
      for (const control of transportSection.controls) {
        if (isButtonControl(control)) {
          if (control.name === 'cycle') {
            expect(control.defaultBehavior).toBe('toggle');
          } else {
            expect(control.defaultBehavior).toBe('momentary');
          }
        }
      }
    });

    it('should have LEDs only on play, stop, record, and cycle', () => {
      const transportSection = nk2.extraSections.find((s) => s.name === 'transport')!;
      const buttonsWithLeds = transportSection.controls
        .filter((c): c is ButtonControlDef => isButtonControl(c) && c.hasLed)
        .map((b) => b.name)
        .sort();

      expect(buttonsWithLeds).toEqual(['cycle', 'play', 'record', 'stop']);
    });
  });

  describe('LED definitions', () => {
    it('should have exactly 28 LEDs (8 solo + 8 mute + 8 rec + 4 transport)', () => {
      expect(nk2.leds).toHaveLength(28);
    });

    it('should match LED_CC.SOLO numbers', () => {
      const soloLedNumbers = nk2.leds
        .filter((led) => led.buttonName.endsWith('.solo'))
        .map((led) => led.midiNumber);
      expect(soloLedNumbers).toEqual([...LED_CC.SOLO]);
    });

    it('should match LED_CC.MUTE numbers', () => {
      const muteLedNumbers = nk2.leds
        .filter((led) => led.buttonName.endsWith('.mute'))
        .map((led) => led.midiNumber);
      expect(muteLedNumbers).toEqual([...LED_CC.MUTE]);
    });

    it('should match LED_CC.REC numbers', () => {
      const recLedNumbers = nk2.leds
        .filter((led) => led.buttonName.endsWith('.rec'))
        .map((led) => led.midiNumber);
      expect(recLedNumbers).toEqual([...LED_CC.REC]);
    });

    it('should match LED_CC.TRANSPORT numbers', () => {
      const transportLedNames = ['transport.play', 'transport.stop', 'transport.record', 'transport.cycle'];
      const transportLedNumbers = nk2.leds
        .filter((led) => transportLedNames.includes(led.buttonName))
        .map((led) => led.midiNumber);
      expect(transportLedNumbers).toEqual([...LED_CC.TRANSPORT]);
    });

    it('should use CC-based LED protocol', () => {
      expect(nk2.ledProtocol.messageType).toBe('cc');
      expect(nk2.ledProtocol.channel).toBe(1);
      expect(nk2.ledProtocol.onValue).toBe(127);
      expect(nk2.ledProtocol.offValue).toBe(0);
    });
  });

  describe('button behaviors', () => {
    it('should set solo and mute to toggle', () => {
      for (const layout of nk2.trackLayouts) {
        const solo = layout.buttonControls.find((b) => b.name === 'solo')!;
        const mute = layout.buttonControls.find((b) => b.name === 'mute')!;
        expect(solo.defaultBehavior).toBe('toggle');
        expect(mute.defaultBehavior).toBe('toggle');
      }
    });

    it('should set rec to momentary', () => {
      for (const layout of nk2.trackLayouts) {
        const rec = layout.buttonControls.find((b) => b.name === 'rec')!;
        expect(rec.defaultBehavior).toBe('momentary');
      }
    });

    it('should have hasLed true for all track buttons', () => {
      for (const layout of nk2.trackLayouts) {
        for (const button of layout.buttonControls) {
          expect(button.hasLed).toBe(true);
        }
      }
    });
  });
});

// =============================================================================
// Akai MidiMix Definition Tests
// =============================================================================

describe('Akai MidiMix definition', () => {
  const midimix = AKAI_MIDIMIX_DEFINITION;

  it('should have the correct device metadata', () => {
    expect(midimix.id).toBe('akai-midimix');
    expect(midimix.name).toBe('MIDI Mix');
    expect(midimix.manufacturer).toBe('Akai');
  });

  it('should have exactly 8 tracks', () => {
    expect(midimix.trackCount).toBe(8);
    expect(midimix.trackLayouts).toHaveLength(8);
  });

  describe('track layouts', () => {
    it('should have exactly 3 knobs, 1 fader, and 3 buttons per track', () => {
      for (let trackIndex = 0; trackIndex < 8; trackIndex++) {
        const layout = midimix.trackLayouts[trackIndex]!;

        // 4 continuous controls: 3 knobs + 1 fader
        expect(layout.continuousControls).toHaveLength(4);
        expect(layout.buttonControls).toHaveLength(3);

        const knobs = layout.continuousControls.filter((c) => c.visualType === 'knob');
        const faders = layout.continuousControls.filter((c) => c.visualType === 'fader');
        expect(knobs).toHaveLength(3);
        expect(faders).toHaveLength(1);

        const buttonNames = layout.buttonControls.map((b) => b.name);
        expect(buttonNames).toContain('mute');
        expect(buttonNames).toContain('solo');
        expect(buttonNames).toContain('rec_arm');
      }
    });

    it('should have buttons using note messageType', () => {
      for (const layout of midimix.trackLayouts) {
        for (const button of layout.buttonControls) {
          expect(button.messageType).toBe('note');
        }
      }
    });

    it('should have knobs and faders using cc messageType', () => {
      for (const layout of midimix.trackLayouts) {
        for (const control of layout.continuousControls) {
          expect(control.messageType).toBe('cc');
        }
      }
    });

    it('should have all controls on channel 1', () => {
      for (const layout of midimix.trackLayouts) {
        for (const control of [...layout.continuousControls, ...layout.buttonControls]) {
          expect(control.channel).toBe(1);
        }
      }
    });
  });

  describe('CC/Note number assignments', () => {
    const expectedKnob1CCs = [16, 20, 24, 28, 46, 50, 54, 58];
    const expectedKnob2CCs = [17, 21, 25, 29, 47, 51, 55, 59];
    const expectedKnob3CCs = [18, 22, 26, 30, 48, 52, 56, 60];
    const expectedFaderCCs = [19, 23, 27, 31, 49, 53, 57, 61];
    const expectedMuteNotes = [1, 4, 7, 10, 13, 16, 19, 22];
    const expectedSoloNotes = [2, 5, 8, 11, 14, 17, 20, 23];
    const expectedRecArmNotes = [3, 6, 9, 12, 15, 18, 21, 24];

    it('should have correct knob row 1 CC numbers', () => {
      const knob1CCs = midimix.trackLayouts.map((layout) => {
        const knob1 = layout.continuousControls.find((c) => c.name === 'knob1');
        return knob1!.midiNumber;
      });
      expect(knob1CCs).toEqual(expectedKnob1CCs);
    });

    it('should have correct knob row 2 CC numbers', () => {
      const knob2CCs = midimix.trackLayouts.map((layout) => {
        const knob2 = layout.continuousControls.find((c) => c.name === 'knob2');
        return knob2!.midiNumber;
      });
      expect(knob2CCs).toEqual(expectedKnob2CCs);
    });

    it('should have correct knob row 3 CC numbers', () => {
      const knob3CCs = midimix.trackLayouts.map((layout) => {
        const knob3 = layout.continuousControls.find((c) => c.name === 'knob3');
        return knob3!.midiNumber;
      });
      expect(knob3CCs).toEqual(expectedKnob3CCs);
    });

    it('should have correct fader CC numbers', () => {
      const faderCCs = midimix.trackLayouts.map((layout) => {
        const fader = layout.continuousControls.find((c) => c.visualType === 'fader');
        return fader!.midiNumber;
      });
      expect(faderCCs).toEqual(expectedFaderCCs);
    });

    it('should have correct mute Note numbers', () => {
      const muteNotes = midimix.trackLayouts.map((layout) => {
        const mute = layout.buttonControls.find((b) => b.name === 'mute');
        return mute!.midiNumber;
      });
      expect(muteNotes).toEqual(expectedMuteNotes);
    });

    it('should have correct solo Note numbers', () => {
      const soloNotes = midimix.trackLayouts.map((layout) => {
        const solo = layout.buttonControls.find((b) => b.name === 'solo');
        return solo!.midiNumber;
      });
      expect(soloNotes).toEqual(expectedSoloNotes);
    });

    it('should have correct rec arm Note numbers', () => {
      const recArmNotes = midimix.trackLayouts.map((layout) => {
        const recArm = layout.buttonControls.find((b) => b.name === 'rec_arm');
        return recArm!.midiNumber;
      });
      expect(recArmNotes).toEqual(expectedRecArmNotes);
    });
  });

  describe('master section', () => {
    it('should have a master extra section', () => {
      const masterSection = midimix.extraSections.find((s) => s.name === 'master');
      expect(masterSection).toBeDefined();
    });

    it('should have 4 controls in master section', () => {
      const masterSection = midimix.extraSections.find((s) => s.name === 'master')!;
      expect(masterSection.controls).toHaveLength(4);
    });

    it('should have master_fader as CC 62 with fader visualType', () => {
      const masterSection = midimix.extraSections.find((s) => s.name === 'master')!;
      const masterFader = masterSection.controls.find((c) => c.name === 'master_fader');
      expect(masterFader).toBeDefined();
      expect(isContinuousControl(masterFader!)).toBe(true);
      if (isContinuousControl(masterFader!)) {
        expect(masterFader!.midiNumber).toBe(62);
        expect(masterFader!.visualType).toBe('fader');
        expect(masterFader!.messageType).toBe('cc');
      }
    });

    it('should have bank_left as Note 25, momentary, no LED', () => {
      const masterSection = midimix.extraSections.find((s) => s.name === 'master')!;
      const bankLeft = masterSection.controls.find((c) => c.name === 'bank_left');
      expect(bankLeft).toBeDefined();
      expect(isButtonControl(bankLeft!)).toBe(true);
      if (isButtonControl(bankLeft!)) {
        expect(bankLeft!.midiNumber).toBe(25);
        expect(bankLeft!.messageType).toBe('note');
        expect(bankLeft!.defaultBehavior).toBe('momentary');
        expect(bankLeft!.hasLed).toBe(false);
      }
    });

    it('should have bank_right as Note 26, momentary, no LED', () => {
      const masterSection = midimix.extraSections.find((s) => s.name === 'master')!;
      const bankRight = masterSection.controls.find((c) => c.name === 'bank_right');
      expect(bankRight).toBeDefined();
      expect(isButtonControl(bankRight!)).toBe(true);
      if (isButtonControl(bankRight!)) {
        expect(bankRight!.midiNumber).toBe(26);
        expect(bankRight!.messageType).toBe('note');
        expect(bankRight!.defaultBehavior).toBe('momentary');
        expect(bankRight!.hasLed).toBe(false);
      }
    });

    it('should have solo_master as Note 27, toggle, no LED', () => {
      const masterSection = midimix.extraSections.find((s) => s.name === 'master')!;
      const soloMaster = masterSection.controls.find((c) => c.name === 'solo_master');
      expect(soloMaster).toBeDefined();
      expect(isButtonControl(soloMaster!)).toBe(true);
      if (isButtonControl(soloMaster!)) {
        expect(soloMaster!.midiNumber).toBe(27);
        expect(soloMaster!.messageType).toBe('note');
        expect(soloMaster!.defaultBehavior).toBe('toggle');
        expect(soloMaster!.hasLed).toBe(false);
      }
    });
  });

  describe('LED definitions', () => {
    it('should have exactly 24 LEDs (8 mute + 8 solo + 8 rec_arm)', () => {
      expect(midimix.leds).toHaveLength(24);
    });

    it('should use note-based LED protocol', () => {
      expect(midimix.ledProtocol.messageType).toBe('note');
      expect(midimix.ledProtocol.channel).toBe(1);
      expect(midimix.ledProtocol.onValue).toBe(127);
      expect(midimix.ledProtocol.offValue).toBe(0);
    });

    it('should have 8 mute LEDs, 8 solo LEDs, and 8 rec_arm LEDs', () => {
      const muteLeds = midimix.leds.filter((led) => led.buttonName.endsWith('.mute'));
      const soloLeds = midimix.leds.filter((led) => led.buttonName.endsWith('.solo'));
      const recArmLeds = midimix.leds.filter((led) => led.buttonName.endsWith('.rec_arm'));

      expect(muteLeds).toHaveLength(8);
      expect(soloLeds).toHaveLength(8);
      expect(recArmLeds).toHaveLength(8);
    });
  });

  describe('button behaviors', () => {
    it('should set mute and solo to toggle', () => {
      for (const layout of midimix.trackLayouts) {
        const mute = layout.buttonControls.find((b) => b.name === 'mute')!;
        const solo = layout.buttonControls.find((b) => b.name === 'solo')!;
        expect(mute.defaultBehavior).toBe('toggle');
        expect(solo.defaultBehavior).toBe('toggle');
      }
    });

    it('should set rec_arm to momentary', () => {
      for (const layout of midimix.trackLayouts) {
        const recArm = layout.buttonControls.find((b) => b.name === 'rec_arm')!;
        expect(recArm.defaultBehavior).toBe('momentary');
      }
    });

    it('should have hasLed true for all track buttons', () => {
      for (const layout of midimix.trackLayouts) {
        for (const button of layout.buttonControls) {
          expect(button.hasLed).toBe(true);
        }
      }
    });
  });
});

// =============================================================================
// Device Registry Tests
// =============================================================================

describe('device registry', () => {
  it('should contain both devices', () => {
    const allDevices = getAllDeviceDefinitions();
    expect(allDevices).toHaveLength(2);

    const deviceIds = allDevices.map((d) => d.id).sort();
    expect(deviceIds).toEqual(['akai-midimix', 'nanokontrol2']);
  });

  it('should retrieve nanoKONTROL2 by ID', () => {
    const device = getDeviceDefinition('nanokontrol2');
    expect(device).toBeDefined();
    expect(device!.name).toBe('nanoKONTROL2');
  });

  it('should retrieve MidiMix by ID', () => {
    const device = getDeviceDefinition('akai-midimix');
    expect(device).toBeDefined();
    expect(device!.name).toBe('MIDI Mix');
  });

  it('should return undefined for unknown device ID', () => {
    const device = getDeviceDefinition('unknown-device');
    expect(device).toBeUndefined();
  });

  describe('identifyDevice', () => {
    it('should identify nanoKONTROL2 from port name', () => {
      const device = identifyDevice('nanoKONTROL2:nanoKONTROL2 MIDI 1 24:0');
      expect(device).not.toBeNull();
      expect(device!.id).toBe('nanokontrol2');
    });

    it('should identify nanoKONTROL2 with space separator', () => {
      const device = identifyDevice('nanoKONTROL2 MIDI 1');
      expect(device).not.toBeNull();
      expect(device!.id).toBe('nanokontrol2');
    });

    it('should identify nanoKONTROL2 case-insensitively', () => {
      const device = identifyDevice('nanokontrol2: test port');
      expect(device).not.toBeNull();
      expect(device!.id).toBe('nanokontrol2');
    });

    it('should identify MidiMix from system port name', () => {
      const device = identifyDevice('MIDI Mix MIDI 1');
      expect(device).not.toBeNull();
      expect(device!.id).toBe('akai-midimix');
    });

    it('should identify MidiMix with colon separator', () => {
      const device = identifyDevice('MIDI Mix: test');
      expect(device).not.toBeNull();
      expect(device!.id).toBe('akai-midimix');
    });

    it('should return null for unknown device', () => {
      const device = identifyDevice('Unknown Device');
      expect(device).toBeNull();
    });

    it('should return null for empty string', () => {
      const device = identifyDevice('');
      expect(device).toBeNull();
    });
  });

  describe('getDefaultDevice', () => {
    it('should return nanoKONTROL2 as the default device', () => {
      const device = getDefaultDevice();
      expect(device.id).toBe('nanokontrol2');
      expect(device.name).toBe('nanoKONTROL2');
    });
  });
});

// =============================================================================
// Structural Consistency Tests
// =============================================================================

describe('structural consistency', () => {
  const allDevices = getAllDeviceDefinitions();

  for (const device of allDevices) {
    describe(`${device.name}`, () => {
      it('should have trackLayouts length matching trackCount', () => {
        expect(device.trackLayouts).toHaveLength(device.trackCount);
      });

      it('should have matching control counts across all tracks', () => {
        if (device.trackLayouts.length === 0) return;

        const firstTrack = device.trackLayouts[0]!;
        const expectedContinuousCount = firstTrack.continuousControls.length;
        const expectedButtonCount = firstTrack.buttonControls.length;

        for (let trackIndex = 0; trackIndex < device.trackLayouts.length; trackIndex++) {
          const layout = device.trackLayouts[trackIndex]!;
          expect(layout.continuousControls).toHaveLength(expectedContinuousCount);
          expect(layout.buttonControls).toHaveLength(expectedButtonCount);
        }
      });

      it('should have no duplicate MIDI numbers within the same message type and channel', () => {
        const seen = new Map<string, string>();

        // Collect all controls from tracks
        for (let trackIndex = 0; trackIndex < device.trackLayouts.length; trackIndex++) {
          const layout = device.trackLayouts[trackIndex]!;
          const allControls: ControlDef[] = [
            ...layout.continuousControls,
            ...layout.buttonControls,
          ];

          for (const control of allControls) {
            const key = `${control.messageType}:ch${control.channel}:${control.midiNumber}`;
            const label = `track${trackIndex + 1}.${control.name}`;
            if (seen.has(key)) {
              // Fail with a clear message about the duplicate
              expect.fail(
                `Duplicate MIDI assignment ${key}: '${seen.get(key)}' and '${label}'`,
              );
            }
            seen.set(key, label);
          }
        }

        // Collect all controls from extra sections
        for (const section of device.extraSections) {
          for (const control of section.controls) {
            const key = `${control.messageType}:ch${control.channel}:${control.midiNumber}`;
            const label = `${section.name}.${control.name}`;
            if (seen.has(key)) {
              expect.fail(
                `Duplicate MIDI assignment ${key}: '${seen.get(key)}' and '${label}'`,
              );
            }
            seen.set(key, label);
          }
        }
      });

      it('should have LED definitions only for buttons with hasLed: true', () => {
        // Build set of all button names that have LEDs
        const buttonsWithLeds = new Set<string>();

        for (let trackIndex = 0; trackIndex < device.trackLayouts.length; trackIndex++) {
          const layout = device.trackLayouts[trackIndex]!;
          for (const button of layout.buttonControls) {
            if (button.hasLed) {
              buttonsWithLeds.add(`track${trackIndex + 1}.${button.name}`);
            }
          }
        }

        for (const section of device.extraSections) {
          for (const control of section.controls) {
            if (isButtonControl(control) && control.hasLed) {
              buttonsWithLeds.add(`${section.name}.${control.name}`);
            }
          }
        }

        // Every LED definition must reference a button with hasLed: true
        for (const led of device.leds) {
          expect(buttonsWithLeds.has(led.buttonName)).toBe(true);
        }

        // Every button with hasLed: true must have an LED definition
        const ledButtonNames = new Set(device.leds.map((led) => led.buttonName));
        for (const buttonName of buttonsWithLeds) {
          expect(ledButtonNames.has(buttonName)).toBe(true);
        }
      });

      it('should have valid MIDI numbers (0-127) on all controls', () => {
        for (const layout of device.trackLayouts) {
          for (const control of [...layout.continuousControls, ...layout.buttonControls]) {
            expect(control.midiNumber).toBeGreaterThanOrEqual(0);
            expect(control.midiNumber).toBeLessThanOrEqual(127);
          }
        }

        for (const section of device.extraSections) {
          for (const control of section.controls) {
            expect(control.midiNumber).toBeGreaterThanOrEqual(0);
            expect(control.midiNumber).toBeLessThanOrEqual(127);
          }
        }
      });

      it('should have valid MIDI channels (1-16) on all controls', () => {
        for (const layout of device.trackLayouts) {
          for (const control of [...layout.continuousControls, ...layout.buttonControls]) {
            expect(control.channel).toBeGreaterThanOrEqual(1);
            expect(control.channel).toBeLessThanOrEqual(16);
          }
        }

        for (const section of device.extraSections) {
          for (const control of section.controls) {
            expect(control.channel).toBeGreaterThanOrEqual(1);
            expect(control.channel).toBeLessThanOrEqual(16);
          }
        }
      });

      it('should have at least one port pattern for auto-detection', () => {
        expect(device.portPatterns.length).toBeGreaterThanOrEqual(1);
      });
    });
  }
});
