/**
 * 8opPerc Performance Domain Model & Korg nanoKONTROL2 / MIDImix Mapper (`js/op8-nanokontrol2-mapper.js`).
 *
 * Mirrors `src/op8_model.rs` and `/home/renick/claude/8operc-konduktiva/src/pulsed-live-rig.js`:
 * - 8 percussion voice lanes (`FAMILY_ORDER` & `KONDUKTIVA_FAMILY_ORDER`)
 * - Per-lane density multipliers (`0.1 ..= 5.0`), macro/gain sliders (`0..=127`), Mute & Solo states,
 *   active & queued timbre names
 * - Global density multiplier (`0.1 ..= 5.0`)
 * - Global rhythmic feel scale (`swing` & `humanize` `0.0 ..= 1.0` with exact midpoint `CC 64 = 0.5`)
 * - 4,096-entry form bank state and 4,096-entry timbre bank state
 * - Native Korg nanoKONTROL2 Performance Mapping (`Knobs CC 16..23`, `Sliders CC 0..7`, `Solo CC 32..39`,
 *   `Mute CC 48..55`, `Rec CC 64..71`, `Track L/R CC 58/59`, `Cycle CC 46`, `Marker Set CC 60`,
 *   `Marker L/R CC 61/62`, `Rewind/Forward CC 43/44`, `Play/Stop/Rec CC 41/42/45`)
 * - MIDImix Emulation Mode translating nanoKONTROL2 CCs into factory Akai MIDI Mix Notes/CCs
 *   (including the 32-CC `Send All` burst on `Marker Set CC 60`).
 */

import { EventEmitter } from 'node:events';

// =============================================================================
// 1. Canonical Constants & Family Tables
// =============================================================================

export const MIN_DENSITY_SCALE = 0.1;
export const MAX_DENSITY_SCALE = 5.0;
export const DEFAULT_DENSITY_QUANTIZE_BEATS = 0.125;
export const FORM_BANK_SIZE = 4096;
export const FORM_BANK_PREFETCH = 4;
export const TIMBRE_BANK_SIZE = 4096;
export const TIMBRE_BANK_REFILL_AT = 1024;

export const FAMILY_ORDER = Object.freeze([
  'kick',
  'snare',
  'hat_closed',
  'hat_open',
  'clap',
  'tom',
  'rim',
  'cymbal',
]);

export const KONDUKTIVA_FAMILY_ORDER = Object.freeze([
  'kicks',
  'snares',
  'hats',
  'toms',
  'bells',
  'metals',
  'noise',
  'fx',
]);

export const KONDUKTIVA_LIVE_LABELS = Object.freeze([
  'kicks',
  'snares',
  'hats',
  'toms',
  'percussion',
  'alt-toms',
  'noise',
  'fx',
]);

export const DEFAULT_BOOT_PATCHES = Object.freeze([
  'kick-sub',
  'snare',
  'closed-hat',
  'open-hat',
  'clap',
  'tom',
  'rim',
  'cymbal',
]);

// Korg nanoKONTROL2 CC assignments
export const NK2_DENSITY_KNOB_CCS = Object.freeze([16, 17, 18, 19, 20, 21, 22, 23]);
export const NK2_GAIN_SLIDER_CCS = Object.freeze([0, 1, 2, 3, 4, 5, 6, 7]);
export const NK2_SOLO_BUTTON_CCS = Object.freeze([32, 33, 34, 35, 36, 37, 38, 39]);
export const NK2_MUTE_BUTTON_CCS = Object.freeze([48, 49, 50, 51, 52, 53, 54, 55]);
export const NK2_REC_REROLL_CCS = Object.freeze([64, 65, 66, 67, 68, 69, 70, 71]);

export const NK2_CC_PLAY = 41;
export const NK2_CC_STOP = 42;
export const NK2_CC_REWIND = 43;
export const NK2_CC_FORWARD = 44;
export const NK2_CC_RECORD = 45;
export const NK2_CC_CYCLE = 46;
export const NK2_CC_TRACK_LEFT = 58;
export const NK2_CC_TRACK_RIGHT = 59;
export const NK2_CC_MARKER_SET = 60;
export const NK2_CC_MARKER_LEFT = 61;
export const NK2_CC_MARKER_RIGHT = 62;

// Factory Akai MIDImix Note & CC assignments
export const MIDIMIX_MUTE_NOTES = Object.freeze([1, 4, 7, 10, 13, 16, 19, 22]);
export const MIDIMIX_SOLO_NOTES = Object.freeze([2, 5, 8, 11, 14, 17, 20, 23]);
export const MIDIMIX_REC_ARM_NOTES = Object.freeze([3, 6, 9, 12, 15, 18, 21, 24]);
export const MIDIMIX_VOICE_DENSITY_CCS = Object.freeze([17, 21, 25, 29, 47, 51, 55, 59]);
export const MIDIMIX_FADER_CCS = Object.freeze([19, 23, 27, 31, 49, 53, 57, 61]);
export const MIDIMIX_FEEL_CC = 58;
export const MIDIMIX_GLOBAL_DENSITY_CC = 61;
export const MIDIMIX_BANK_LEFT_NOTE = 25;
export const MIDIMIX_BANK_RIGHT_NOTE = 26;
export const MIDIMIX_TIMBRE_SET_NOTE = 27;
export const MIDIMIX_SEND_ALL_CCS = Object.freeze([
  ...Array.from({ length: 16 }, (_, i) => 16 + i),
  ...Array.from({ length: 16 }, (_, i) => 46 + i),
]);

// =============================================================================
// 2. Scaling & Hash Functions
// =============================================================================

/**
 * Exponential MIDI CC (0..127) to density multiplier (0.1..5.0) ladder,
 * matching `mapMidiCCToDensity` in `8operc-konduktiva/src/live-density.js`.
 */
export function mapMidiCCToDensity(value) {
  const cc = Number(value);
  if (!Number.isInteger(cc) || cc < 0 || cc > 127) {
    throw new RangeError('MIDI CC value must be an integer from 0 to 127');
  }
  return MIN_DENSITY_SCALE * Math.pow(MAX_DENSITY_SCALE / MIN_DENSITY_SCALE, cc / 127);
}

/**
 * Inverse of `mapMidiCCToDensity`: maps a density multiplier (0.1..5.0) back to CC (0..127).
 */
export function mapDensityToMidiCC(density) {
  const clamped = Math.max(MIN_DENSITY_SCALE, Math.min(MAX_DENSITY_SCALE, Number(density) || 1.0));
  const ratio = clamped / MIN_DENSITY_SCALE;
  const maxRatio = MAX_DENSITY_SCALE / MIN_DENSITY_SCALE;
  const normalized = Math.log(ratio) / Math.log(maxRatio);
  return Math.max(0, Math.min(127, Math.round(normalized * 127)));
}

/**
 * Positive-slope MIDI CC (0..127) to feel scale (0.0..1.0) with exact midpoint `CC 64 = 0.5`:
 * - `CC 0   -> 0.0`
 * - `CC 64  -> 0.5`
 * - `CC 127 -> 1.0`
 */
export function mapMidiCCToFeelScale(value) {
  const cc = Number(value);
  if (!Number.isInteger(cc) || cc < 0 || cc > 127) {
    throw new RangeError('Feel controller value must be an integer from 0 to 127');
  }
  return cc <= 64 ? cc / 128 : 0.5 + (cc - 64) / 126;
}

/**
 * Inverted feel scale used by legacy `pulsed-live-rig.js` (`CC 0 = 1.0`, `CC 64 = 0.5`, `CC 127 = 0.0`).
 */
export function mapMidiCCToMidimixFeelScale(value) {
  const cc = Number(value);
  if (!Number.isInteger(cc) || cc < 0 || cc > 127) {
    throw new RangeError('MIDImix feel controller value must be an integer from 0 to 127');
  }
  return cc <= 64 ? 1 - cc / 128 : (127 - cc) / 126;
}

/**
 * Inverse of `mapMidiCCToFeelScale` (`0.0 -> 0`, `0.5 -> 64`, `1.0 -> 127`).
 */
export function mapFeelScaleToMidiCC(scale) {
  const s = Math.max(0, Math.min(1, Number(scale) || 0));
  if (s <= 0.5) {
    return Math.max(0, Math.min(64, Math.round(s * 128)));
  }
  return Math.max(64, Math.min(127, Math.round(64 + (s - 0.5) * 126)));
}

/**
 * Deterministic FNV-1a 32-bit pool index matching `timbrePoolIndex` in `pulsed-live-rig.js`.
 */
export function timbrePoolIndex(id, length = TIMBRE_BANK_SIZE) {
  let hash = 2166136261;
  const text = String(id);
  for (let i = 0; i < text.length; i += 1) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) % Math.max(1, length);
}

/**
 * Resolves a lane index (0..7) or family name (`"kick"`, `"kicks"`, `"percussion"`, `"alt-toms"`, etc.).
 */
export function resolveLaneIndex(ident) {
  if (typeof ident === 'number' && Number.isInteger(ident) && ident >= 0 && ident < 8) {
    return ident;
  }
  const norm = String(ident || '').trim().toLowerCase();
  if (/^[0-7]$/.test(norm)) {
    return Number(norm);
  }
  const trackMatch = norm.match(/^track([1-8])$/);
  if (trackMatch) {
    return Number(trackMatch[1]) - 1;
  }
  for (let i = 0; i < 8; i += 1) {
    if (
      FAMILY_ORDER[i] === norm ||
      KONDUKTIVA_FAMILY_ORDER[i] === norm ||
      KONDUKTIVA_LIVE_LABELS[i] === norm
    ) {
      return i;
    }
  }
  if (norm === 'closed-hat' || norm === 'closed_hat') return 2;
  if (norm === 'open-hat' || norm === 'open_hat') return 3;
  return -1;
}

// =============================================================================
// 3. Op8PerformanceModel & NanoKontrol2Op8Mapper
// =============================================================================

export class Op8PerformanceModel extends EventEmitter {
  /**
   * @param {object} [options]
   * @param {boolean} [options.queueTimbresForNextBar=true]
   * @param {number} [options.stepCcDelta=8]
   * @param {number} [options.authoredSwing=0.24]
   * @param {number} [options.authoredHumanize=0.36]
   */
  constructor(options = {}) {
    super();
    this.queueTimbresForNextBar = options.queueTimbresForNextBar !== false;
    this.stepCcDelta = Number(options.stepCcDelta ?? 8);

    const defaultDensityCc = mapDensityToMidiCC(1.0);
    this.lanes = FAMILY_ORDER.map((family, index) => ({
      index,
      family,
      konduktivaFamily: KONDUKTIVA_FAMILY_ORDER[index],
      liveLabel: KONDUKTIVA_LIVE_LABELS[index],
      densityMultiplier: 1.0,
      densityCc: defaultDensityCc,
      gainCc: 100,
      gainNormalized: 100 / 127,
      muted: false,
      soloed: false,
      activeTimbre: DEFAULT_BOOT_PATCHES[index],
      queuedTimbre: null,
      rerollCount: 0,
    }));

    this.globalDensityMultiplier = 1.0;
    this.globalDensityCc = defaultDensityCc;

    const authoredSwing = Number(options.authoredSwing ?? 0.24);
    const authoredHumanize = Number(options.authoredHumanize ?? 0.36);
    this.feel = {
      scale: 0.5,
      feelCc: 64,
      authoredSwing,
      authoredHumanize,
      effectiveSwing: authoredSwing * 0.5,
      effectiveHumanize: authoredHumanize * 0.5,
    };

    this.formBank = {
      totalEntries: FORM_BANK_SIZE,
      activeIndex: 0,
      activeFormName: 'form-0001',
      prefetchWindow: FORM_BANK_PREFETCH,
      history: [0],
      historyCursor: 0,
    };

    this.timbreBank = {
      totalEntries: TIMBRE_BANK_SIZE,
      refillThreshold: TIMBRE_BANK_REFILL_AT,
      activeSetIndex: 0,
      remainingSets: TIMBRE_BANK_SIZE,
      capturedPool: [],
    };

    this.transport = {
      playing: false,
      recordingWav: false,
      bpm: 132.0,
      currentBeat: 0.0,
      beatsPerBar: 4,
    };
  }

  anyLaneSoloed() {
    return this.lanes.some((l) => l.soloed);
  }

  isLaneAudible(index) {
    const lane = this.lanes[index];
    if (!lane || lane.muted) return false;
    return this.anyLaneSoloed() ? lane.soloed : true;
  }

  audibleMask() {
    return this.lanes.map((_, i) => this.isLaneAudible(i));
  }

  effectiveLaneDensity(index) {
    const lane = this.lanes[index];
    if (!lane) return this.globalDensityMultiplier;
    return this.globalDensityMultiplier * lane.densityMultiplier;
  }

  setLaneDensityFromCc(index, cc) {
    const lane = this.lanes[index];
    if (!lane) return null;
    const clamped = Math.max(0, Math.min(127, Math.round(Number(cc))));
    lane.densityCc = clamped;
    lane.densityMultiplier = mapMidiCCToDensity(clamped);
    return lane.densityMultiplier;
  }

  setLaneGainFromCc(index, cc) {
    const lane = this.lanes[index];
    if (!lane) return null;
    const clamped = Math.max(0, Math.min(127, Math.round(Number(cc))));
    lane.gainCc = clamped;
    lane.gainNormalized = clamped / 127;
    return lane.gainNormalized;
  }

  setGlobalDensityFromCc(cc) {
    const clamped = Math.max(0, Math.min(127, Math.round(Number(cc))));
    this.globalDensityCc = clamped;
    this.globalDensityMultiplier = mapMidiCCToDensity(clamped);
    return this.globalDensityMultiplier;
  }

  setFeelFromCc(cc) {
    const clamped = Math.max(0, Math.min(127, Math.round(Number(cc))));
    this.feel.feelCc = clamped;
    this.feel.scale = mapMidiCCToFeelScale(clamped);
    this.feel.effectiveSwing = this.feel.authoredSwing * this.feel.scale;
    this.feel.effectiveHumanize = this.feel.authoredHumanize * this.feel.scale;
    return this.feel.scale;
  }

  navigateForm(direction) {
    const total = Math.max(1, this.formBank.totalEntries);
    const dir = String(direction || 'next').toLowerCase();
    if (dir === 'previous' || dir === 'prev' || dir === '-1') {
      this.formBank.activeIndex = (this.formBank.activeIndex + total - 1) % total;
    } else {
      this.formBank.activeIndex = (this.formBank.activeIndex + 1) % total;
    }
    this.formBank.activeFormName = `form-${String(this.formBank.activeIndex + 1).padStart(4, '0')}`;
    this.formBank.history.push(this.formBank.activeIndex);
    this.formBank.historyCursor = this.formBank.history.length - 1;
    return {
      direction: dir === 'previous' || dir === 'prev' || dir === '-1' ? 'previous' : 'next',
      activeIndex: this.formBank.activeIndex,
      activeFormName: this.formBank.activeFormName,
    };
  }

  rerollLaneTimbre(index, explicitTimbreName = null) {
    const lane = this.lanes[index];
    if (!lane) return null;
    lane.rerollCount += 1;
    const nextName =
      explicitTimbreName || `${lane.family}-var-${String(lane.rerollCount).padStart(4, '0')}`;
    if (this.queueTimbresForNextBar) {
      lane.queuedTimbre = nextName;
    } else {
      lane.activeTimbre = nextName;
      lane.queuedTimbre = null;
    }
    return {
      lane: index,
      family: lane.family,
      activeTimbre: lane.activeTimbre,
      queuedTimbre: lane.queuedTimbre,
      rerollCount: lane.rerollCount,
    };
  }

  advanceFullTimbreSet() {
    const total = Math.max(1, this.timbreBank.totalEntries);
    this.timbreBank.activeSetIndex = (this.timbreBank.activeSetIndex + 1) % total;
    if (this.timbreBank.remainingSets > 0) {
      this.timbreBank.remainingSets -= 1;
    }
    if (this.timbreBank.remainingSets <= this.timbreBank.refillThreshold) {
      this.timbreBank.remainingSets = this.timbreBank.totalEntries;
    }
    const setNum = String(this.timbreBank.activeSetIndex + 1).padStart(4, '0');
    const timbres = this.lanes.map((lane) => {
      lane.rerollCount += 1;
      const nextName = `${lane.family}-set-${setNum}`;
      if (this.queueTimbresForNextBar) {
        lane.queuedTimbre = nextName;
      } else {
        lane.activeTimbre = nextName;
        lane.queuedTimbre = null;
      }
      return lane.queuedTimbre || lane.activeTimbre;
    });
    return {
      activeSetIndex: this.timbreBank.activeSetIndex,
      timbres,
    };
  }

  commitAllQueuedTimbres() {
    let committed = 0;
    for (const lane of this.lanes) {
      if (lane.queuedTimbre != null) {
        lane.activeTimbre = lane.queuedTimbre;
        lane.queuedTimbre = null;
        committed += 1;
      }
    }
    return committed;
  }

  capturePresetSnapshot(label = null) {
    const serial = this.timbreBank.capturedPool.length + 1;
    const id = `preset-${String(serial).padStart(4, '0')}`;
    const resolvedLabel = label && String(label).trim() ? String(label).trim() : id;
    const timbres = this.lanes.map((l) => l.queuedTimbre || l.activeTimbre);
    const signature = timbres.join('|');
    const poolBankIndex = timbrePoolIndex(`${id}:${signature}`, this.timbreBank.totalEntries);
    const snapshot = {
      id,
      label: resolvedLabel,
      signature,
      timbres,
      poolBankIndex,
    };
    this.timbreBank.capturedPool.push(snapshot);
    return snapshot;
  }

  /**
   * Applies a native Korg nanoKONTROL2 CC message (`cc`, `value`) and emits an `'action'` event.
   */
  applyNanoKontrol2Cc(cc, value) {
    const c = Number(cc);
    const v = Math.max(0, Math.min(127, Math.round(Number(value))));

    // 1. Track Knobs 1..8 (CC 16..23) -> voice density
    if (c >= 16 && c <= 23) {
      const lane = c - 16;
      const density = this.setLaneDensityFromCc(lane, v);
      const action = {
        type: 'SetLaneDensity',
        lane,
        family: this.lanes[lane].family,
        ccValue: v,
        density,
        effectiveDensity: this.effectiveLaneDensity(lane),
      };
      this.emit('action', action);
      return action;
    }

    // 2. Track Sliders 1..8 (CC 0..7) -> voice gain/macro
    if (c >= 0 && c <= 7) {
      const lane = c;
      const gainNormalized = this.setLaneGainFromCc(lane, v);
      const action = {
        type: 'SetLaneGain',
        lane,
        family: this.lanes[lane].family,
        ccValue: v,
        gainNormalized,
      };
      this.emit('action', action);
      return action;
    }

    // Buttons only trigger on press (v > 0)
    if (v === 0) return null;

    // 3. Track Solo Buttons 1..8 (CC 32..39)
    if (c >= 32 && c <= 39) {
      const lane = c - 32;
      this.lanes[lane].soloed = !this.lanes[lane].soloed;
      const action = {
        type: 'ToggleLaneSolo',
        lane,
        family: this.lanes[lane].family,
        soloed: this.lanes[lane].soloed,
        audibleMask: this.audibleMask(),
      };
      this.emit('action', action);
      return action;
    }

    // 4. Track Mute Buttons 1..8 (CC 48..55)
    if (c >= 48 && c <= 55) {
      const lane = c - 48;
      this.lanes[lane].muted = !this.lanes[lane].muted;
      const action = {
        type: 'ToggleLaneMute',
        lane,
        family: this.lanes[lane].family,
        muted: this.lanes[lane].muted,
        audibleMask: this.audibleMask(),
      };
      this.emit('action', action);
      return action;
    }

    // 5. Track Rec Buttons 1..8 (CC 64..71) -> single-lane timbre reroll
    if (c >= 64 && c <= 71) {
      const lane = c - 64;
      const reroll = this.rerollLaneTimbre(lane);
      const action = {
        type: 'RerollLaneTimbre',
        ...reroll,
      };
      this.emit('action', action);
      return action;
    }

    switch (c) {
      case NK2_CC_TRACK_LEFT: {
        const nav = this.navigateForm('previous');
        const action = { type: 'NavigateForm', ...nav };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_TRACK_RIGHT: {
        const nav = this.navigateForm('next');
        const action = { type: 'NavigateForm', ...nav };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_CYCLE: {
        const setInfo = this.advanceFullTimbreSet();
        const action = { type: 'AdvanceTimbreSet', ...setInfo };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_MARKER_SET: {
        const snapshot = this.capturePresetSnapshot();
        const action = { type: 'CapturePresetPoolSnapshot', snapshot };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_MARKER_LEFT: {
        const nextCc = Math.max(0, Math.min(127, this.globalDensityCc - this.stepCcDelta));
        const globalDensity = this.setGlobalDensityFromCc(nextCc);
        const action = {
          type: 'StepGlobalDensity',
          deltaCc: -this.stepCcDelta,
          ccValue: nextCc,
          globalDensity,
        };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_MARKER_RIGHT: {
        const nextCc = Math.max(0, Math.min(127, this.globalDensityCc + this.stepCcDelta));
        const globalDensity = this.setGlobalDensityFromCc(nextCc);
        const action = {
          type: 'StepGlobalDensity',
          deltaCc: this.stepCcDelta,
          ccValue: nextCc,
          globalDensity,
        };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_REWIND: {
        const nextCc = Math.max(0, Math.min(127, this.feel.feelCc - this.stepCcDelta));
        const feelScale = this.setFeelFromCc(nextCc);
        const action = {
          type: 'StepGlobalFeel',
          deltaCc: -this.stepCcDelta,
          ccValue: nextCc,
          feelScale,
        };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_FORWARD: {
        const nextCc = Math.max(0, Math.min(127, this.feel.feelCc + this.stepCcDelta));
        const feelScale = this.setFeelFromCc(nextCc);
        const action = {
          type: 'StepGlobalFeel',
          deltaCc: this.stepCcDelta,
          ccValue: nextCc,
          feelScale,
        };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_PLAY: {
        this.transport.playing = true;
        const action = { type: 'TransportPlay', playing: true };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_STOP: {
        this.transport.playing = false;
        const action = { type: 'TransportStop', playing: false };
        this.emit('action', action);
        return action;
      }
      case NK2_CC_RECORD: {
        this.transport.recordingWav = !this.transport.recordingWav;
        const action = { type: 'TransportRecord', recordingWav: this.transport.recordingWav };
        this.emit('action', action);
        return action;
      }
      default:
        return null;
    }
  }

  /**
   * Translates a Korg nanoKONTROL2 CC message into factory Akai MIDImix Note/CC messages
   * compatible with `pulsed-live-rig.js`.
   */
  translateNanoKontrol2ToMidimix(cc, value) {
    const c = Number(cc);
    const v = Math.max(0, Math.min(127, Math.round(Number(value))));

    if (c >= 16 && c <= 23) {
      const lane = c - 16;
      const controller = MIDIMIX_VOICE_DENSITY_CCS[lane];
      return [{ kind: 'controlchange', channel: 1, controller, value: v }];
    }
    if (c >= 0 && c <= 7) {
      const lane = c;
      const controller = MIDIMIX_FADER_CCS[lane];
      return [{ kind: 'controlchange', channel: 1, controller, value: v }];
    }
    if (c >= 48 && c <= 55) {
      const lane = c - 48;
      const note = MIDIMIX_MUTE_NOTES[lane];
      return [{ kind: v > 0 ? 'noteon' : 'noteoff', channel: 1, data1: note, data2: v }];
    }
    if (c >= 32 && c <= 39) {
      const lane = c - 32;
      const note = MIDIMIX_SOLO_NOTES[lane];
      return [{ kind: v > 0 ? 'noteon' : 'noteoff', channel: 1, data1: note, data2: v }];
    }
    if (c >= 64 && c <= 71) {
      const lane = c - 64;
      const note = MIDIMIX_REC_ARM_NOTES[lane];
      return [{ kind: v > 0 ? 'noteon' : 'noteoff', channel: 1, data1: note, data2: v }];
    }

    switch (c) {
      case NK2_CC_TRACK_LEFT:
        return [
          {
            kind: v > 0 ? 'noteon' : 'noteoff',
            channel: 1,
            data1: MIDIMIX_BANK_LEFT_NOTE,
            data2: v,
          },
        ];
      case NK2_CC_TRACK_RIGHT:
        return [
          {
            kind: v > 0 ? 'noteon' : 'noteoff',
            channel: 1,
            data1: MIDIMIX_BANK_RIGHT_NOTE,
            data2: v,
          },
        ];
      case NK2_CC_CYCLE:
        return [
          {
            kind: v > 0 ? 'noteon' : 'noteoff',
            channel: 1,
            data1: MIDIMIX_TIMBRE_SET_NOTE,
            data2: v,
          },
        ];
      case NK2_CC_MARKER_SET:
        if (v === 0) return [];
        return MIDIMIX_SEND_ALL_CCS.map((ctrl) => {
          const laneIdx = MIDIMIX_VOICE_DENSITY_CCS.indexOf(ctrl);
          let ctrlVal = 64;
          if (laneIdx >= 0) ctrlVal = this.lanes[laneIdx].densityCc;
          else if (ctrl === MIDIMIX_GLOBAL_DENSITY_CC) ctrlVal = this.globalDensityCc;
          else if (ctrl === MIDIMIX_FEEL_CC) ctrlVal = this.feel.feelCc;
          return { kind: 'controlchange', channel: 1, controller: ctrl, value: ctrlVal };
        });
      case NK2_CC_MARKER_LEFT:
      case NK2_CC_MARKER_RIGHT:
        if (v === 0) return [];
        return [
          {
            kind: 'controlchange',
            channel: 1,
            controller: MIDIMIX_GLOBAL_DENSITY_CC,
            value: this.globalDensityCc,
          },
        ];
      case NK2_CC_REWIND:
      case NK2_CC_FORWARD:
        if (v === 0) return [];
        return [
          {
            kind: 'controlchange',
            channel: 1,
            controller: MIDIMIX_FEEL_CC,
            value: this.feel.feelCc,
          },
        ];
      default:
        return [];
    }
  }
}
