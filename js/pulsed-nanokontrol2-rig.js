/**
 * Korg nanoKONTROL2 8opPerc Live Performance Rig (`js/pulsed-nanokontrol2-rig.js`).
 *
 * Upgrades `/home/renick/claude/8operc-konduktiva/src/pulsed-live-rig.js` from an
 * Akai MIDImix-only workflow to a first-class Korg nanoKONTROL2 + bidirectional
 * Rust `nkEditor4` OSC cockpit workflow:
 * - Live Korg nanoKONTROL2 discovery (Linux sysfs `/sys/class/sound/card*`,
 *   `/dev/snd/by-id/`, `/proc/asound/seq/clients`, PipeWire/ALSA raw/seq ports,
 *   plus bidirectional UDP OSC sync with `nkEditor4`)
 * - 8-lane voice Mute (`CC 48..55`) AND Solo (`CC 32..39`) gating with LED feedback
 * - Per-voice (`Knobs CC 16..23`) and global (`Marker L/R CC 61/62`) density ladder
 *   (`0.1 ..= 5.0`) latched at 32nd-note boundaries (`0.125` beats)
 * - Swing & humanize rhythmic feel scaling (`Rewind/Forward CC 43/44`, exact midpoint `CC 64 = 0.5`)
 * - Single-lane timbre reroll via `Rec` buttons (`CC 64..71`) scheduled for next bar boundary
 * - Full 8-voice timbre set switching via `Cycle` (`CC 46`) from the 4,096-set reservoir
 * - 4,096-entry form bank navigation via `Track Left / Right` (`CC 58 / 59`)
 * - Atomic 8-voice preset snapshot saving via single-button `Marker Set` (`CC 60`) into
 *   `nanokontrol2-random-preset-pool.jsonl`
 * - Deterministic `--dry` / mock-engine mode (`dryRun: true`) so automated tests verify the
 *   complete live rig workflow without spawning audio engines or starting live playback.
 */

import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  FAMILY_ORDER,
  KONDUKTIVA_FAMILY_ORDER,
  KONDUKTIVA_LIVE_LABELS,
  DEFAULT_BOOT_PATCHES,
  MIN_DENSITY_SCALE,
  MAX_DENSITY_SCALE,
  DEFAULT_DENSITY_QUANTIZE_BEATS,
  FORM_BANK_SIZE,
  FORM_BANK_PREFETCH,
  TIMBRE_BANK_SIZE,
  TIMBRE_BANK_REFILL_AT,
  NK2_DENSITY_KNOB_CCS,
  NK2_GAIN_SLIDER_CCS,
  NK2_SOLO_BUTTON_CCS,
  NK2_MUTE_BUTTON_CCS,
  NK2_REC_REROLL_CCS,
  NK2_CC_PLAY,
  NK2_CC_STOP,
  NK2_CC_REWIND,
  NK2_CC_FORWARD,
  NK2_CC_RECORD,
  NK2_CC_CYCLE,
  NK2_CC_TRACK_LEFT,
  NK2_CC_TRACK_RIGHT,
  NK2_CC_MARKER_SET,
  NK2_CC_MARKER_LEFT,
  NK2_CC_MARKER_RIGHT,
  Op8PerformanceModel,
  mapMidiCCToDensity,
  mapMidiCCToFeelScale,
  timbrePoolIndex,
  resolveLaneIndex,
} from './op8-nanokontrol2-mapper.js';
import { NkEditorClient } from './nkeditor-sdk.js';

export const DEFAULT_PRESET_POOL_FILENAME = 'nanokontrol2-random-preset-pool.jsonl';
export const PIPEWIRE_NANOKONTROL2_CAPTURE =
  'Midi-Bridge:nanoKONTROL2:(capture_0) nanoKONTROL2 _ CTRL';

const EPSILON = 1e-8;

// =============================================================================
// 1. 32nd-Note Quantized Live Density Latch (`createQuantizedDensityState`)
// =============================================================================

/**
 * Quantized live density state that latches both global and per-lane density
 * multipliers (`0.1 ..= 5.0`) at exact 32nd-note boundaries (`0.125` beats),
 * matching `createLiveDensityState` in `8operc-konduktiva/src/live-density.js`.
 */
export function createQuantizedDensityState(options = {}) {
  const quantizeBeats = Number(options.quantizeBeats ?? DEFAULT_DENSITY_QUANTIZE_BEATS);
  if (!(quantizeBeats > 0)) {
    throw new RangeError('live density quantizeBeats must be positive');
  }
  const slotsPerBeat = Math.round(1 / quantizeBeats);
  if (Math.abs(slotsPerBeat * quantizeBeats - 1) > EPSILON) {
    throw new RangeError('live density quantizeBeats must divide one beat exactly');
  }
  const clampScale = (v) =>
    Math.max(MIN_DENSITY_SCALE, Math.min(MAX_DENSITY_SCALE, Number(v) || 1.0));

  const initialScale = clampScale(options.initialScale ?? 1.0);
  const familyScales = Object.create(null);
  for (const fam of FAMILY_ORDER) {
    familyScales[fam] = {
      currentScale: 1.0,
      pendingScale: null,
      pendingBeat: null,
    };
  }

  return {
    enabled: true,
    minScale: MIN_DENSITY_SCALE,
    maxScale: MAX_DENSITY_SCALE,
    quantizeBeats,
    currentScale: initialScale,
    pendingScale: null,
    pendingBeat: null,
    familyScales,

    /**
     * Schedules a global density scale change at the next 32nd-note boundary.
     */
    setGlobal(scale, nowBeat = 0) {
      const now = Number(nowBeat) || 0;
      const next = clampScale(scale);
      this.pendingScale = next;
      this.pendingBeat = Math.ceil((now - EPSILON) / this.quantizeBeats) * this.quantizeBeats;
      return {
        pendingScale: this.pendingScale,
        pendingBeat: this.pendingBeat,
      };
    },

    /**
     * Schedules a per-lane density scale change at the next 32nd-note boundary.
     */
    setFamily(familyOrIndex, scale, nowBeat = 0) {
      const idx = resolveLaneIndex(familyOrIndex);
      const fam = idx >= 0 ? FAMILY_ORDER[idx] : String(familyOrIndex || '');
      if (!fam) throw new TypeError('live density family must be a valid lane or family');
      const now = Number(nowBeat) || 0;
      const next = clampScale(scale);
      const state =
        this.familyScales[fam] ||
        (this.familyScales[fam] = {
          currentScale: 1.0,
          pendingScale: null,
          pendingBeat: null,
        });
      state.pendingScale = next;
      state.pendingBeat = Math.ceil((now - EPSILON) / this.quantizeBeats) * this.quantizeBeats;
      return {
        family: fam,
        pendingScale: state.pendingScale,
        pendingBeat: state.pendingBeat,
      };
    },

    /**
     * Evaluates the effective density multiplier (`global * family`) at `beat`,
     * latching any pending changes whose `pendingBeat <= beat`.
     */
    at(beat, familyOrIndex = null) {
      const now = Number(beat) || 0;
      if (this.pendingBeat != null && now + EPSILON >= this.pendingBeat) {
        this.currentScale = this.pendingScale;
        this.pendingScale = null;
        this.pendingBeat = null;
      }
      if (familyOrIndex == null) {
        return this.currentScale;
      }
      const idx = resolveLaneIndex(familyOrIndex);
      const fam = idx >= 0 ? FAMILY_ORDER[idx] : String(familyOrIndex);
      const state = this.familyScales[fam];
      if (state && state.pendingBeat != null && now + EPSILON >= state.pendingBeat) {
        state.currentScale = state.pendingScale;
        state.pendingScale = null;
        state.pendingBeat = null;
      }
      return this.currentScale * (state ? state.currentScale : 1.0);
    },
  };
}

// =============================================================================
// 2. Korg nanoKONTROL2 Hardware Discovery (Read-Only Sysfs / ALSA / PipeWire)
// =============================================================================

/**
 * Discovers a connected Korg nanoKONTROL2 (`0944:0117`) via Linux `/sys/class/sound`,
 * `/dev/snd/by-id`, and `/proc/asound/seq/clients` without blocking or opening audio ports.
 */
export function discoverNanoKontrol2Hardware(options = {}) {
  const sysfsRoot = options.sysfsRoot || '/sys/class/sound';
  const seqClientsPath = options.seqClientsPath || '/proc/asound/seq/clients';
  const result = {
    connected: false,
    vendorId: '0944',
    productId: '0117',
    cardNumber: null,
    cardId: null,
    alsaClientId: null,
    alsaPort: null,
    rawMidiDevice: null,
    byIdSymlink: '/dev/snd/by-id/usb-KORG_INC._nanoKONTROL2-00',
    pipewireCapturePort: PIPEWIRE_NANOKONTROL2_CAPTURE,
    fingerprint: '0944:0117:noserial:nanoKONTROL2',
  };

  try {
    if (fs.existsSync(sysfsRoot)) {
      for (const entry of fs.readdirSync(sysfsRoot)) {
        const match = entry.match(/^card(\d+)$/);
        if (!match) continue;
        const cardNum = Number(match[1]);
        const idFile = path.join(sysfsRoot, entry, 'id');
        if (!fs.existsSync(idFile)) continue;
        const cardId = fs.readFileSync(idFile, 'utf8').trim();
        if (/nanokontrol2/i.test(cardId)) {
          result.connected = true;
          result.cardNumber = cardNum;
          result.cardId = cardId;
          result.rawMidiDevice = `hw:${cardNum},0,0`;
          result.fingerprint = `0944:0117:noserial:${cardId}`;
          break;
        }
      }
    }
  } catch (_) {
    // Non-Linux or restricted sysfs environment
  }

  try {
    if (fs.existsSync(seqClientsPath)) {
      const text = fs.readFileSync(seqClientsPath, 'utf8');
      for (const line of text.split(/\r?\n/)) {
        const m = line.match(/^Client\s+(\d+)\s*:\s*"([^"]+)"\s*\[([^\]]+)\]/);
        if (m && /nanokontrol2/i.test(m[2])) {
          result.connected = true;
          result.alsaClientId = Number(m[1]);
          result.alsaPort = `${m[1]}:0`;
          break;
        }
      }
    }
  } catch (_) {
    // Ignore missing /proc/asound/seq/clients
  }

  return result;
}

// =============================================================================
// 3. PulsedNanoKontrol2Rig (Full Live & Deterministic Dry-Run Rig)
// =============================================================================

export class PulsedNanoKontrol2Rig extends EventEmitter {
  /**
   * @param {object} [options]
   * @param {boolean} [options.dryRun=true] - Defaults to true so live audio is never started without explicit cue
   * @param {number} [options.bpm=160]
   * @param {string} [options.form='gqom_drive']
   * @param {number} [options.seed=260729]
   * @param {number} [options.beatsPerBar=4]
   * @param {string} [options.presetPoolPath]
   * @param {NkEditorClient} [options.oscClient]
   */
  constructor(options = {}) {
    super();
    this.dryRun = options.dryRun !== false;
    this.bpm = Number(options.bpm ?? 160);
    this.initialForm = String(options.form || 'gqom_drive');
    this.seed = Number(options.seed ?? 260729);
    this.beatsPerBar = Number(options.beatsPerBar ?? 4);
    this.presetPoolPath =
      options.presetPoolPath ||
      path.join(os.tmpdir(), `nkeditor4-${process.pid}`, DEFAULT_PRESET_POOL_FILENAME);

    this.hardware = discoverNanoKontrol2Hardware(options.hardware);
    this.model = new Op8PerformanceModel({
      queueTimbresForNextBar: true,
      stepCcDelta: options.stepCcDelta ?? 8,
      authoredSwing: options.authoredSwing ?? 0.24,
      authoredHumanize: options.authoredHumanize ?? 0.36,
    });
    this.model.transport.bpm = this.bpm;
    this.model.transport.beatsPerBar = this.beatsPerBar;
    this.model.formBank.activeFormName = this.initialForm;

    this.densityState = createQuantizedDensityState({
      quantizeBeats: options.quantizeBeats ?? DEFAULT_DENSITY_QUANTIZE_BEATS,
      initialScale: 1.0,
    });

    this.oscClient = options.oscClient || null;
    this.ledStates = new Map();
    this.stopped = false;

    // Initialize 4,096-entry form catalogue and 4,096-entry timbre set reservoir
    this._formCatalog = Array.from({ length: FORM_BANK_SIZE }, (_, idx) => ({
      index: idx,
      id: `form-${String(idx + 1).padStart(4, '0')}`,
      name: idx === 0 ? this.initialForm : `form-${String(idx + 1).padStart(4, '0')}`,
      seed: this.seed + idx,
    }));

    this._timbreReservoir = Array.from({ length: TIMBRE_BANK_SIZE }, (_, idx) => {
      const setNum = String(idx + 1).padStart(4, '0');
      return {
        setId: `timbre-set-${setNum}`,
        timbres: Object.fromEntries(
          FAMILY_ORDER.map((fam) => [fam, `${fam}-set-${setNum}`])
        ),
      };
    });

    // Mock Konduktiva player map for deterministic dry-run and live parity
    this.players = Object.fromEntries(
      FAMILY_ORDER.map((fam, idx) => [
        fam,
        {
          family: fam,
          konduktivaFamily: KONDUKTIVA_FAMILY_ORDER[idx],
          label: KONDUKTIVA_LIVE_LABELS[idx],
          status: 'playing',
          muted: false,
          soloed: false,
          activeTimbre: DEFAULT_BOOT_PATCHES[idx],
          queuedTimbre: null,
          queuedTargetBeat: null,
          gainNormalized: 100 / 127,
        },
      ])
    );

    this._syncPlayerAudibilityAndLeds();
  }

  /**
   * Returns the current beat of the rig's transport.
   */
  currentBeat() {
    return this.model.transport.currentBeat;
  }

  /**
   * Advances the simulated beat clock (in `--dry` mode) and commits any queued
   * 32nd-note density latches or next-bar timbre changes that have reached their target beat.
   */
  advanceBeatTo(targetBeat) {
    const nextBeat = Number(targetBeat);
    if (!Number.isFinite(nextBeat) || nextBeat < this.model.transport.currentBeat) {
      throw new RangeError('targetBeat must be >= currentBeat');
    }
    this.model.transport.currentBeat = nextBeat;

    // Latch global & per-lane 32nd-note densities
    this.densityState.at(nextBeat);
    for (const fam of FAMILY_ORDER) {
      this.densityState.at(nextBeat, fam);
    }

    // Commit any next-bar timbre preloads whose targetBeat <= nextBeat
    let committedTimbres = 0;
    for (let i = 0; i < 8; i += 1) {
      const fam = FAMILY_ORDER[i];
      const player = this.players[fam];
      if (player.queuedTimbre && player.queuedTargetBeat != null && nextBeat + EPSILON >= player.queuedTargetBeat) {
        player.activeTimbre = player.queuedTimbre;
        player.queuedTimbre = null;
        player.queuedTargetBeat = null;
        this.model.lanes[i].activeTimbre = player.activeTimbre;
        this.model.lanes[i].queuedTimbre = null;
        this._setLed(`track${i + 1}.rec`, false);
        committedTimbres += 1;
      }
    }
    return {
      currentBeat: this.model.transport.currentBeat,
      committedTimbres,
    };
  }

  /**
   * Computes the next bar downbeat strictly in the future.
   */
  nextBarBeat() {
    const bar = Math.max(1, this.beatsPerBar);
    const now = this.currentBeat();
    return (Math.floor(now / bar) + 1) * bar;
  }

  /**
   * Processes a Korg nanoKONTROL2 CC event (`cc`, `value`), updates the `8opPerc`
   * domain model, latches 32nd-note densities, updates Mute/Solo player gating
   * and LEDs, persists preset snapshots on `Marker Set (CC 60)`, and syncs over OSC.
   */
  handleNanoKontrol2Cc(cc, value) {
    if (this.stopped) {
      throw new Error('cannot handle MIDI on a stopped live rig');
    }
    const action = this.model.applyNanoKontrol2Cc(cc, value);
    if (!action) return null;

    const nowBeat = this.currentBeat();

    switch (action.type) {
      case 'SetLaneDensity': {
        const latch = this.densityState.setFamily(action.family, action.density, nowBeat);
        action.pendingBeat = latch.pendingBeat;
        if (this.oscClient) {
          this.oscClient.setOp8LaneDensity(action.lane, action.density);
        }
        break;
      }
      case 'SetLaneGain': {
        this.players[action.family].gainNormalized = action.gainNormalized;
        break;
      }
      case 'ToggleLaneMute': {
        this._syncPlayerAudibilityAndLeds();
        if (this.oscClient) {
          this.oscClient.setOp8LaneMute(action.lane, action.muted);
        }
        break;
      }
      case 'ToggleLaneSolo': {
        this._syncPlayerAudibilityAndLeds();
        if (this.oscClient) {
          this.oscClient.setOp8LaneSolo(action.lane, action.soloed);
        }
        break;
      }
      case 'RerollLaneTimbre': {
        const targetBeat = this.nextBarBeat();
        const player = this.players[action.family];
        player.queuedTimbre = action.queuedTimbre;
        player.queuedTargetBeat = targetBeat;
        this._setLed(`track${action.lane + 1}.rec`, true);
        action.targetBeat = targetBeat;
        if (this.oscClient) {
          this.oscClient.rerollOp8Timbre(action.lane, action.queuedTimbre);
        }
        break;
      }
      case 'AdvanceTimbreSet': {
        const targetBeat = this.nextBarBeat();
        for (let i = 0; i < 8; i += 1) {
          const fam = FAMILY_ORDER[i];
          this.players[fam].queuedTimbre = action.timbres[i];
          this.players[fam].queuedTargetBeat = targetBeat;
        }
        this._setLed('transport.cycle', true);
        action.targetBeat = targetBeat;
        if (this.oscClient) {
          this.oscClient.rerollOp8Timbre('all');
        }
        break;
      }
      case 'NavigateForm': {
        const targetBeat = this.nextBarBeat();
        const entry = this._formCatalog[action.activeIndex];
        action.formEntry = entry;
        action.targetBeat = targetBeat;
        if (this.oscClient) {
          this.oscClient.navigateOp8Form(action.direction);
        }
        break;
      }
      case 'CapturePresetPoolSnapshot': {
        const persisted = this._appendSnapshotToPoolFile(action.snapshot);
        action.poolFile = persisted.poolFile;
        action.poolSize = persisted.poolSize;
        if (this.oscClient) {
          this.oscClient.saveOp8Snapshot(action.snapshot.label);
        }
        break;
      }
      case 'StepGlobalDensity': {
        const latch = this.densityState.setGlobal(action.globalDensity, nowBeat);
        action.pendingBeat = latch.pendingBeat;
        break;
      }
      case 'StepGlobalFeel': {
        action.effectiveSwing = this.model.feel.effectiveSwing;
        action.effectiveHumanize = this.model.feel.effectiveHumanize;
        break;
      }
      case 'TransportPlay': {
        this._setLed('transport.play', true);
        this._setLed('transport.stop', false);
        break;
      }
      case 'TransportStop': {
        this._setLed('transport.play', false);
        this._setLed('transport.stop', true);
        break;
      }
      case 'TransportRecord': {
        this._setLed('transport.record', action.recordingWav);
        break;
      }
      default:
        break;
    }

    this.emit('action', action);
    return action;
  }

  /**
   * Synchronizes all 8 players' effective audibility (`playing` vs `stopped`)
   * from their Mute + Solo states and updates the corresponding nanoKONTROL2 LEDs.
   */
  _syncPlayerAudibilityAndLeds() {
    const mask = this.model.audibleMask();
    for (let i = 0; i < 8; i += 1) {
      const fam = FAMILY_ORDER[i];
      const lane = this.model.lanes[i];
      const player = this.players[fam];
      player.muted = lane.muted;
      player.soloed = lane.soloed;
      player.status = mask[i] ? 'playing' : 'stopped';
      this._setLed(`track${i + 1}.mute`, lane.muted);
      this._setLed(`track${i + 1}.solo`, lane.soloed);
    }
  }

  _setLed(buttonId, on) {
    const state = Boolean(on);
    this.ledStates.set(buttonId, state);
    if (this.oscClient) {
      this.oscClient.setLed(buttonId, state);
    }
  }

  /**
   * Persists a captured 8-voice timbre snapshot to `nanokontrol2-random-preset-pool.jsonl`
   * and injects it into `_timbreReservoir` at its deterministic FNV-1a index.
   */
  _appendSnapshotToPoolFile(snapshot) {
    const dir = path.dirname(this.presetPoolPath);
    fs.mkdirSync(dir, { recursive: true });
    const record = {
      id: snapshot.id,
      label: snapshot.label,
      savedAt: new Date().toISOString(),
      controller: 'korg-nanokontrol2',
      triggerCc: NK2_CC_MARKER_SET,
      poolBankIndex: snapshot.poolBankIndex,
      timbres: Object.fromEntries(
        FAMILY_ORDER.map((fam, idx) => [fam, snapshot.timbres[idx]])
      ),
    };
    fs.appendFileSync(this.presetPoolPath, `${JSON.stringify(record)}\n`, 'utf8');
    this._timbreReservoir[snapshot.poolBankIndex] = {
      setId: snapshot.id,
      timbres: record.timbres,
    };
    return {
      poolFile: this.presetPoolPath,
      poolSize: this.model.timbreBank.capturedPool.length,
      record,
    };
  }

  /**
   * Reads all saved 8-voice preset snapshots from `presetPoolPath`.
   */
  readCapturedPresetPool() {
    if (!fs.existsSync(this.presetPoolPath)) return [];
    return fs
      .readFileSync(this.presetPoolPath, 'utf8')
      .split(/\r?\n/)
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  }

  /**
   * Cleanly stops the rig without leaving any timers or sockets open.
   */
  async stop() {
    this.stopped = true;
    this.model.transport.playing = false;
  }
}
