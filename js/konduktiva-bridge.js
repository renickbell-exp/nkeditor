/**
 * Konduktiva MusicalEnvironment & Rhythm-Map Ladder Bridge (`js/konduktiva-bridge.js`).
 *
 * Connects Konduktiva `MusicalEnvironment` players and `QuantizedMap` rhythm-map ladders
 * (`/home/renick/claude/konduktiva` and `/home/renick/claude/8operc-konduktiva`) to the
 * Rust `nkEditor4` egui cockpit over UDP OSC:
 * - Extracts per-player `QuantizedMap` IOI (inter-onset interval) rhythm rows (`onsetBeats`,
 *   `ioiValues`), `maskMap` flags, `velocityMap` values, and sparse-to-dense ladder rung
 *   positions (`selectedRungIndex`, `totalRungs`, `nativeRate`, `targetRate`).
 * - Streams real-time rhythm row updates, density graph positions, and per-hit note trigger
 *   events (`beat`, `note`, `velocity`, `gateMs`, `timbre`, `ioi`, `variationDepth`) over UDP OSC.
 */

import { EventEmitter } from 'node:events';
import {
  FAMILY_ORDER,
  KONDUKTIVA_FAMILY_ORDER,
  DEFAULT_BOOT_PATCHES,
  MIN_DENSITY_SCALE,
  MAX_DENSITY_SCALE,
  resolveLaneIndex,
} from './op8-nanokontrol2-mapper.js';
import { encodeOscMessage } from './osc-codec.js';

export const VARIATION_DEPTH_RANGES = Object.freeze({
  kicks: [0.45, 0.95],
  snares: [0.82, 1.45],
  hats: [0.72, 1.55],
  toms: [0.82, 1.5],
  bells: [0.78, 1.5],
  metals: [0.7, 1.28],
  noise: [0.75, 1.48],
  fx: [0.72, 1.5],
});

/**
 * Builds a deterministic 5-rung Konduktiva-style `QuantizedMap` rhythm ladder for a voice lane
 * over `keyspan` beats (default `4` beats), ordered from sparse (rung 0) to dense (rung 4).
 */
export function buildRhythmMapLadderData(laneOrFamily, keyspan = 4) {
  const idx = Math.max(0, resolveLaneIndex(laneOrFamily));
  const span = Math.max(1, Number(keyspan) || 4);

  // Base onset patterns per lane (in a 4-beat bar) scaled to `span`
  const baseOnsetsByLane = [
    [0.0, 1.5, 2.0, 3.25], // kick
    [1.0, 3.0], // snare
    [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5], // hat_closed
    [0.75, 2.75], // hat_open
    [1.0, 2.5, 3.0], // clap / bells
    [0.5, 1.75, 3.5], // tom / metals
    [0.25, 1.25, 2.25, 3.25], // rim / noise
    [0.0, 2.0], // cymbal / fx
  ];

  const base = baseOnsetsByLane[idx] || [0.0, 1.0, 2.0, 3.0];
  const scale = span / 4.0;
  const nativeKeys = base.map((b) => Number((b * scale).toFixed(4)));

  // Build 5 rungs from sparse (0.25x) to ultra-dense (16th/32nd subdivisions)
  const rungSubdivisions = [
    nativeKeys.filter((_, i) => i % 2 === 0), // Rung 0: Sparse
    nativeKeys.slice(0, Math.max(1, Math.ceil(nativeKeys.length * 0.75))), // Rung 1: Light
    nativeKeys.slice(), // Rung 2: Native authored pattern
    subdivideOnsets(nativeKeys, span, 0.5), // Rung 3: Dense (8th/16th)
    subdivideOnsets(nativeKeys, span, 0.25), // Rung 4: Maximum density (16th/32nd)
  ];

  const rateKeys = [];
  const rows = [];

  for (const rawKeys of rungSubdivisions) {
    const keys = Array.from(new Set(rawKeys)).sort((a, b) => a - b);
    const values = keys.map((k, i) => {
      const next = i + 1 < keys.length ? keys[i + 1] : keys[0] + span;
      return Number((next - k).toFixed(4));
    });
    const rate = Number((keys.length / span).toFixed(4));
    rateKeys.push(rate);
    rows.push({
      keyspan: span,
      keys,
      values,
    });
  }

  return {
    keyspan: span,
    keys: rateKeys,
    values: rows,
    nativeRate: rateKeys[2],
  };
}

function subdivideOnsets(keys, span, step) {
  const set = new Set(keys);
  for (const k of keys) {
    const next = (k + step) % span;
    set.add(Number(next.toFixed(4)));
  }
  return Array.from(set).sort((a, b) => a - b);
}

/**
 * Selects the active rhythm row (`{ selectedRungIndex, totalRungs, targetRate, row }`)
 * from a Konduktiva `rhythmMap` ladder given `nativeRate` and `effectiveDensity`.
 */
export function selectLadderRung(rhythmMap, nativeRate = 1.0, effectiveDensity = 1.0) {
  const rows = Array.isArray(rhythmMap?.values) ? rhythmMap.values : [];
  const rates = Array.isArray(rhythmMap?.keys) ? rhythmMap.keys : [];
  const totalRungs = rows.length;
  if (totalRungs === 0) {
    return {
      selectedRungIndex: 0,
      totalRungs: 1,
      targetRate: nativeRate * effectiveDensity,
      row: { keyspan: 4, keys: [0], values: [4] },
    };
  }

  const clampedDensity = Math.max(
    MIN_DENSITY_SCALE,
    Math.min(MAX_DENSITY_SCALE, Number(effectiveDensity) || 1.0)
  );
  const targetRate = Math.max(0.05, Number(nativeRate || 1.0) * clampedDensity);

  // Pick the rung whose rate key is closest to targetRate
  let bestIdx = 0;
  let bestDist = Infinity;
  for (let i = 0; i < totalRungs; i += 1) {
    const r = Number(rates[i] ?? (i + 1) * 0.5);
    const dist = Math.abs(r - targetRate);
    if (dist < bestDist) {
      bestDist = dist;
      bestIdx = i;
    }
  }

  return {
    selectedRungIndex: bestIdx,
    totalRungs,
    targetRate: Number(targetRate.toFixed(4)),
    row: rows[bestIdx],
  };
}

/**
 * Computes the deterministic per-hit variation depth at `beat` for a Konduktiva family,
 * matching `variationDepths` in `8operc-konduktiva/src/pulsed-core-rhythm.js`.
 */
export function computeVariationDepth(konduktivaFamily, beat, keyspan = 4, stepIndex = 0) {
  const [low, high] = VARIATION_DEPTH_RANGES[konduktivaFamily] || [0.75, 1.25];
  const spread = high - low;
  const familyPhase =
    Array.from(String(konduktivaFamily)).reduce((sum, ch) => sum + ch.charCodeAt(0), 0) * 0.173;
  const phase = ((Number(beat) || 0) / Math.max(1, keyspan)) * Math.PI * 2 + familyPhase;
  const contour =
    0.5 + 0.32 * Math.sin(phase) + 0.18 * Math.sin(phase * 2.618 + stepIndex * 0.71);
  return Number(Math.max(low, Math.min(high, low + spread * contour)).toFixed(4));
}

/**
 * Extracts a structured telemetry snapshot of a single voice lane's IOI rhythm row,
 * mask flags, velocity values, and density ladder position from a Konduktiva
 * `MusicalEnvironment` (or `Op8PerformanceModel`).
 */
export function extractPlayerRhythmRow(envOrModel, laneOrFamily, options = {}) {
  const idx = Math.max(0, resolveLaneIndex(laneOrFamily));
  const family = FAMILY_ORDER[idx];
  const konduktivaFamily = KONDUKTIVA_FAMILY_ORDER[idx];
  const keyspan = Number(options.keyspan ?? 4);

  // Resolve density and mute/solo from either an Op8PerformanceModel or Konduktiva env
  const laneModel = envOrModel?.lanes?.[idx] || null;
  const player =
    envOrModel?.players?.[family] ||
    envOrModel?.players?.[konduktivaFamily] ||
    null;

  const laneDensity = Number(
    options.laneDensity ?? laneModel?.densityMultiplier ?? player?.densityMultiplier ?? 1.0
  );
  const globalDensity = Number(
    options.globalDensity ?? envOrModel?.globalDensity ?? 1.0
  );
  const effectiveDensity = Number((laneDensity * globalDensity).toFixed(4));

  const ladder =
    player?.rhythmMap && Array.isArray(player.rhythmMap.values)
      ? player.rhythmMap
      : buildRhythmMapLadderData(idx, keyspan);

  const nativeRate = Number(ladder.nativeRate ?? ladder.keys?.[2] ?? 1.0);
  const { selectedRungIndex, totalRungs, targetRate, row } = selectLadderRung(
    ladder,
    nativeRate,
    effectiveDensity
  );

  const onsetBeats = Array.from(row.keys || []);
  const ioiValues = Array.from(row.values || []);
  const isMuted = Boolean(laneModel?.muted ?? player?.muted ?? false);
  const maskFlags = onsetBeats.map((_, stepIdx) => {
    if (isMuted) return true;
    if (Array.isArray(player?.maskMap?.values) && player.maskMap.values[stepIdx] != null) {
      return Boolean(player.maskMap.values[stepIdx]);
    }
    return false;
  });

  const velocities = onsetBeats.map((_, stepIdx) => {
    if (Array.isArray(player?.velocityMap?.values) && player.velocityMap.values[stepIdx] != null) {
      return Math.max(0, Math.min(127, Math.round(Number(player.velocityMap.values[stepIdx]))));
    }
    return stepIdx === 0 ? 118 : 96;
  });

  const beat = Number(options.beat ?? envOrModel?.transport?.currentBeat ?? 0);
  const variationDepth = computeVariationDepth(konduktivaFamily, beat, row.keyspan || keyspan, 0);

  return {
    laneIndex: idx,
    family,
    konduktivaFamily,
    keyspan: Number(row.keyspan || keyspan),
    onsetBeats,
    ioiValues,
    maskFlags,
    velocities,
    selectedRungIndex,
    totalRungs,
    nativeRate,
    effectiveDensity,
    targetRate,
    variationDepth,
    activeTimbre: String(
      laneModel?.activeTimbre ?? player?.activeTimbre ?? DEFAULT_BOOT_PATCHES[idx]
    ),
  };
}

/**
 * Bridge connecting a Konduktiva `MusicalEnvironment` and `Op8PerformanceModel`
 * to the Rust `nkEditor4` egui cockpit over UDP OSC.
 */
export class KonduktivaBridge extends EventEmitter {
  constructor(options = {}) {
    super();
    this.env = options.env || null;
    this.model = options.model || null;
    this.oscClient = options.oscClient || null;
    this.keyspan = Number(options.keyspan ?? 4);
    this.laneRows = FAMILY_ORDER.map((_, idx) =>
      extractPlayerRhythmRow(this.model || this.env, idx, { keyspan: this.keyspan })
    );
    this.triggerHistory = [];
    this.maxTriggerHistory = Number(options.maxTriggerHistory ?? 64);
  }

  /**
   * Recomputes all 8 voice lanes' IOI rhythm rows from the current density ladder state
   * and optionally broadcasts them over UDP OSC to the Rust egui cockpit.
   */
  syncAllLanes(nowBeat = 0) {
    const source = this.model || this.env;
    for (let i = 0; i < 8; i += 1) {
      this.laneRows[i] = extractPlayerRhythmRow(source, i, {
        keyspan: this.keyspan,
        beat: nowBeat,
      });
      this._sendOscRhythmRow(this.laneRows[i]);
    }
    this.emit('rhythmSync', this.laneRows);
    return this.laneRows;
  }

  /**
   * Updates a single lane's rhythm row when its density knob or global density changes.
   */
  syncLane(laneOrFamily, nowBeat = 0) {
    const idx = resolveLaneIndex(laneOrFamily);
    if (idx < 0 || idx >= 8) return null;
    const row = extractPlayerRhythmRow(this.model || this.env, idx, {
      keyspan: this.keyspan,
      beat: nowBeat,
    });
    this.laneRows[idx] = row;
    this._sendOscRhythmRow(row);
    this.emit('laneRhythm', row);
    return row;
  }

  /**
   * Records and streams a real-time Konduktiva note trigger event (`8operc` player fire).
   */
  emitNoteTrigger(laneOrFamily, trigger = {}) {
    const idx = resolveLaneIndex(laneOrFamily);
    if (idx < 0 || idx >= 8) return null;
    const row = this.laneRows[idx];
    const beat = Number(trigger.beat ?? 0);
    const localBeat = ((beat % row.keyspan) + row.keyspan) % row.keyspan;

    // Find matching onset step in the active IOI row
    let stepIndex = 0;
    for (let i = 0; i < row.onsetBeats.length; i += 1) {
      if (row.onsetBeats[i] <= localBeat + 1e-4) {
        stepIndex = i;
      }
    }

    const event = {
      laneIndex: idx,
      family: FAMILY_ORDER[idx],
      konduktivaFamily: KONDUKTIVA_FAMILY_ORDER[idx],
      beat,
      stepIndex,
      ioi: Number(trigger.ioi ?? row.ioiValues[stepIndex] ?? 0.5),
      note: Number(trigger.note ?? 36 + idx),
      velocity: Number(trigger.velocity ?? row.velocities[stepIndex] ?? 100),
      gateMs: Number(trigger.gateMs ?? 150),
      timbre: String(trigger.timbre ?? row.activeTimbre),
      variationDepth: computeVariationDepth(
        KONDUKTIVA_FAMILY_ORDER[idx],
        beat,
        row.keyspan,
        stepIndex
      ),
    };

    this.triggerHistory.push(event);
    if (this.triggerHistory.length > this.maxTriggerHistory) {
      this.triggerHistory.shift();
    }

    this._sendOscTrigger(event);
    this.emit('noteTrigger', event);
    return event;
  }

  /**
   * Steps the bridge to `beat`, automatically firing note triggers for any unmasked
   * IOI onsets in `[prevBeat, beat]` so rehearsals and tests can verify end-to-end streaming.
   */
  stepBeatWindow(prevBeat, nextBeat) {
    const start = Number(prevBeat) || 0;
    const end = Number(nextBeat) || 0;
    if (end < start) return [];

    const fired = [];
    for (let idx = 0; idx < 8; idx += 1) {
      const row = this.laneRows[idx];
      const span = row.keyspan;
      const startBar = Math.floor(start / span);
      const endBar = Math.floor(end / span);

      for (let bar = startBar; bar <= endBar; bar += 1) {
        const barOffset = bar * span;
        for (let s = 0; s < row.onsetBeats.length; s += 1) {
          if (row.maskFlags[s]) continue;
          const absBeat = barOffset + row.onsetBeats[s];
          if (absBeat >= start - 1e-6 && absBeat <= end + 1e-6) {
            fired.push(
              this.emitNoteTrigger(idx, {
                beat: Number(absBeat.toFixed(4)),
                ioi: row.ioiValues[s],
                velocity: row.velocities[s],
                timbre: row.activeTimbre,
              })
            );
          }
        }
      }
    }
    return fired;
  }

  _sendOscRhythmRow(row) {
    if (!this.oscClient) return;
    const addr = `/nkeditor/konduktiva/lane/${row.laneIndex}/rhythm`;
    const args = [JSON.stringify(row)];
    if (typeof this.oscClient.sendOsc === 'function') {
      void this.oscClient.sendOsc(addr, args);
    } else if (typeof this.oscClient.sendRaw === 'function') {
      this.oscClient.sendRaw(encodeOscMessage(addr, args));
    }
  }

  _sendOscTrigger(event) {
    if (!this.oscClient) return;
    const addr = `/nkeditor/konduktiva/lane/${event.laneIndex}/trigger`;
    const args = [
      { type: 'f', value: event.beat },
      { type: 'i', value: event.note },
      { type: 'i', value: event.velocity },
      { type: 'f', value: event.ioi },
      String(event.timbre),
    ];
    if (typeof this.oscClient.sendOsc === 'function') {
      void this.oscClient.sendOsc(addr, args);
    } else if (typeof this.oscClient.sendRaw === 'function') {
      this.oscClient.sendRaw(encodeOscMessage(addr, args));
    }
  }
}
