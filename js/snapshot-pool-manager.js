/**
 * 8opPerc Timbre Snapshot & Scene Morphing Manager (`js/snapshot-pool-manager.js`).
 *
 * Companion Node.js & Konduktiva module matching `src/snapshot_pool.rs`:
 * 1. Manages durable 8-voice snapshots in JSONL preset pools
 *    (`nanokontrol2-random-preset-pool.jsonl` and legacy `midimix-random-preset-pool.jsonl`)
 *    plus named timbre-state `.json` files.
 * 2. Supports rehearsal snapshot browsing, star ratings (`0..=5`), favorites, and
 *    per-lane voice lock masks (e.g., locking `kick` and `snare` while loading
 *    `hat_closed`, `clap`, `tom`, etc. from another snapshot).
 * 3. Smoothly morphs continuous per-lane density multipliers (`0.1..=5.0`), macro/gain
 *    sliders (`0..=127`), global density (`0.1..=5.0`), and rhythmic feel (`0.0..=1.0`)
 *    between Scene A and Scene B over a configurable number of musical beats.
 */

import fs from 'node:fs';
import path from 'node:path';
import {
  FAMILY_ORDER,
  DEFAULT_BOOT_PATCHES,
  MIN_DENSITY_SCALE,
  MAX_DENSITY_SCALE,
  TIMBRE_BANK_SIZE,
  timbrePoolIndex,
  resolveLaneIndex,
} from './op8-nanokontrol2-mapper.js';

export const DEFAULT_NK2_POOL_FILENAME = 'nanokontrol2-random-preset-pool.jsonl';
export const LEGACY_MIDIMIX_POOL_FILENAME = 'midimix-random-preset-pool.jsonl';

/**
 * Computes the canonical pipe-delimited signature for an 8-voice timbre array.
 */
export function timbreSetSignature(timbres) {
  return Array.isArray(timbres) ? timbres.join('|') : '';
}

/**
 * Normalizes a raw snapshot or JSONL record (from either `nanokontrol2-random-preset-pool.jsonl`,
 * legacy `midimix-random-preset-pool.jsonl`, or a named timbre-state `.json` file) into a
 * canonical 8-voice scene record.
 */
export function normalizeSnapshotRecord(raw = {}, fallbackIndex = 0) {
  const id = String(raw.id || raw.setId || `snap-${String(fallbackIndex + 1).padStart(4, '0')}`);
  const label = String(raw.label || raw.name || id);
  const controller = String(raw.controller || 'korg-nanokontrol2');
  const savedAt = String(raw.savedAt || raw.saved_at || new Date().toISOString());

  const timbres = DEFAULT_BOOT_PATCHES.slice();
  if (Array.isArray(raw.timbres)) {
    for (let i = 0; i < 8; i += 1) {
      if (raw.timbres[i]) timbres[i] = String(raw.timbres[i]);
    }
  } else if (raw.timbres && typeof raw.timbres === 'object') {
    for (const [k, v] of Object.entries(raw.timbres)) {
      const idx = resolveLaneIndex(k);
      if (idx >= 0 && v) {
        timbres[idx] = String(v);
      }
    }
  }

  const signature = String(raw.signature || timbreSetSignature(timbres));
  const poolBankIndex =
    typeof raw.poolBankIndex === 'number'
      ? ((raw.poolBankIndex % TIMBRE_BANK_SIZE) + TIMBRE_BANK_SIZE) % TIMBRE_BANK_SIZE
      : typeof raw.pool_bank_index === 'number'
      ? ((raw.pool_bank_index % TIMBRE_BANK_SIZE) + TIMBRE_BANK_SIZE) % TIMBRE_BANK_SIZE
      : timbrePoolIndex(signature, TIMBRE_BANK_SIZE);

  const rating = Math.max(0, Math.min(5, Math.round(Number(raw.rating ?? 0))));
  const favorite = Boolean(raw.favorite);
  const tags = Array.isArray(raw.tags) ? raw.tags.map(String) : [];

  const densities = new Array(8).fill(1.0);
  if (Array.isArray(raw.densities)) {
    for (let i = 0; i < 8; i += 1) {
      if (raw.densities[i] != null) {
        densities[i] = Math.max(
          MIN_DENSITY_SCALE,
          Math.min(MAX_DENSITY_SCALE, Number(raw.densities[i]) || 1.0)
        );
      }
    }
  }

  const gains = new Array(8).fill(100);
  if (Array.isArray(raw.gains)) {
    for (let i = 0; i < 8; i += 1) {
      if (raw.gains[i] != null) {
        gains[i] = Math.max(0, Math.min(127, Math.round(Number(raw.gains[i]))));
      }
    }
  }

  const mutes = new Array(8).fill(false);
  if (Array.isArray(raw.mutes)) {
    for (let i = 0; i < 8; i += 1) mutes[i] = Boolean(raw.mutes[i]);
  }

  const solos = new Array(8).fill(false);
  if (Array.isArray(raw.solos)) {
    for (let i = 0; i < 8; i += 1) solos[i] = Boolean(raw.solos[i]);
  }

  const globalDensity = Math.max(
    MIN_DENSITY_SCALE,
    Math.min(MAX_DENSITY_SCALE, Number(raw.globalDensity ?? raw.global_density ?? 1.0))
  );
  const feelScale = Math.max(
    0.0,
    Math.min(1.0, Number(raw.feelScale ?? raw.feel_scale ?? 0.5))
  );
  const activeFormName = String(raw.activeFormName || raw.active_form_name || 'form-0001');

  return {
    id,
    label,
    controller,
    savedAt,
    signature,
    poolBankIndex,
    rating,
    favorite,
    tags,
    timbres,
    densities,
    gains,
    mutes,
    solos,
    globalDensity,
    feelScale,
    activeFormName,
  };
}

/**
 * Serializes a normalized snapshot record into a single JSONL line with a
 * family-keyed `timbres` map (compatible with both `nanokontrol2-random-preset-pool.jsonl`
 * and `midimix-random-preset-pool.jsonl`).
 */
export function formatSnapshotJsonlLine(snapshot) {
  const norm = normalizeSnapshotRecord(snapshot);
  const timbresMap = Object.fromEntries(
    FAMILY_ORDER.map((fam, idx) => [fam, norm.timbres[idx]])
  );
  return JSON.stringify({
    ...norm,
    timbres: timbresMap,
  });
}

/**
 * Manages 8-voice snapshots, JSONL preset pools, named timbre-state `.json` files,
 * per-lane voice lock masks, and beat-synchronized scene morphing.
 */
export class SnapshotPoolManager {
  constructor() {
    this.snapshots = [];
    this.activeSnapshotId = '';
    this.laneLocks = new Array(8).fill(false);
    this.queueTimbresOnRecall = false;
    this.morphState = {
      sceneAId: '',
      sceneBId: '',
      durationBeats: 8.0,
      progress: 0.0,
      timbreSwitchPoint: 0.5,
      active: false,
      startBeat: null,
    };
  }

  /**
   * Captures the current state of an `Op8PerformanceModel` into the snapshot pool.
   */
  captureFromModel(model, options = {}) {
    const idx = this.snapshots.length + 1;
    const id = options.id || `snap-${String(idx).padStart(4, '0')}`;
    const label = options.label || `Snapshot ${String(idx).padStart(4, '0')}`;
    const controller = options.controller || 'korg-nanokontrol2';

    const record = normalizeSnapshotRecord(
      {
        id,
        label,
        controller,
        rating: options.rating ?? 0,
        favorite: Boolean(options.favorite),
        tags: options.tags || [],
        timbres: model.lanes.map((l) => l.activeTimbre),
        densities: model.lanes.map((l) => l.densityMultiplier),
        gains: model.lanes.map((l) => l.gainCc),
        mutes: model.lanes.map((l) => l.muted),
        solos: model.lanes.map((l) => l.soloed),
        globalDensity: model.globalDensity,
        feelScale: model.feel.scale,
        activeFormName: model.formBank.activeFormName,
      },
      idx - 1
    );

    this.upsertSnapshot(record);
    this.activeSnapshotId = record.id;
    return record;
  }

  /**
   * Inserts or updates a snapshot record by `id` or `signature`.
   */
  upsertSnapshot(rawRecord) {
    const norm = normalizeSnapshotRecord(rawRecord, this.snapshots.length);
    const existingIdx = this.snapshots.findIndex(
      (s) => s.id === norm.id || s.signature === norm.signature
    );
    if (existingIdx >= 0) {
      this.snapshots[existingIdx] = norm;
    } else {
      this.snapshots.push(norm);
    }
    return norm;
  }

  /**
   * Finds a snapshot by `id` or case-insensitive `label`.
   */
  getSnapshot(idOrLabel) {
    const target = String(idOrLabel || '').toLowerCase();
    return (
      this.snapshots.find(
        (s) => s.id.toLowerCase() === target || s.label.toLowerCase() === target
      ) || null
    );
  }

  /**
   * Sets the rehearsal star rating (`0..=5`) on a snapshot.
   */
  setRating(idOrLabel, rating) {
    const snap = this.getSnapshot(idOrLabel);
    if (!snap) return null;
    snap.rating = Math.max(0, Math.min(5, Math.round(Number(rating) || 0)));
    return snap.rating;
  }

  /**
   * Toggles the `favorite` star on a snapshot.
   */
  toggleFavorite(idOrLabel) {
    const snap = this.getSnapshot(idOrLabel);
    if (!snap) return null;
    snap.favorite = !snap.favorite;
    return snap.favorite;
  }

  /**
   * Locks or unlocks a specific voice lane (`0..7` or family name like `'kick'`, `'snare'`).
   */
  setLaneLock(laneOrFamily, locked) {
    const idx = resolveLaneIndex(laneOrFamily);
    if (idx >= 0 && idx < 8) {
      this.laneLocks[idx] = Boolean(locked);
      return true;
    }
    return false;
  }

  /**
   * Imports all snapshots from a JSONL string (`nanokontrol2-random-preset-pool.jsonl`
   * or legacy `midimix-random-preset-pool.jsonl`).
   */
  importJsonlString(jsonlContent) {
    const lines = String(jsonlContent || '')
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter((l) => l.length > 0 && !l.startsWith('#'));

    let count = 0;
    for (const line of lines) {
      const parsed = JSON.parse(line);
      this.upsertSnapshot(parsed);
      count += 1;
    }
    return count;
  }

  /**
   * Loads snapshots from a JSONL file on disk.
   */
  loadJsonlFile(filePath) {
    if (!fs.existsSync(filePath)) return 0;
    const content = fs.readFileSync(filePath, 'utf8');
    return this.importJsonlString(content);
  }

  /**
   * Appends a snapshot to a JSONL preset pool file on disk.
   */
  appendSnapshotToJsonlFile(filePath, snapshot) {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    const line = formatSnapshotJsonlLine(snapshot);
    fs.appendFileSync(filePath, `${line}\n`, 'utf8');
    return line;
  }

  /**
   * Saves a named timbre-state `.json` file to disk.
   */
  saveNamedSceneJsonFile(filePath, idOrLabel) {
    const snap = this.getSnapshot(idOrLabel);
    if (!snap) throw new Error(`Snapshot not found: ${idOrLabel}`);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    const json = JSON.stringify(snap, null, 2);
    fs.writeFileSync(filePath, json, 'utf8');
    return snap;
  }

  /**
   * Loads a named timbre-state `.json` file from disk into the pool.
   */
  loadNamedSceneJsonFile(filePath) {
    const raw = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    return this.upsertSnapshot(raw);
  }

  /**
   * Recalls a snapshot into an `Op8PerformanceModel`, respecting `this.laneLocks`
   * (so locked lanes such as `kick` and `snare` keep their existing timbres and densities).
   */
  recallSnapshotIntoModel(idOrLabel, model) {
    const snap = this.getSnapshot(idOrLabel);
    if (!snap) return null;
    this.activeSnapshotId = snap.id;

    const updatedLanes = [];
    for (let i = 0; i < 8; i += 1) {
      const locked = Boolean(this.laneLocks[i] || model.lanes[i]?.timbreLocked);
      if (locked) continue;

      if (this.queueTimbresOnRecall) {
        model.lanes[i].queuedTimbre = snap.timbres[i];
      } else {
        model.lanes[i].activeTimbre = snap.timbres[i];
        model.lanes[i].queuedTimbre = null;
      }
      model.lanes[i].densityMultiplier = snap.densities[i];
      model.lanes[i].gainCc = snap.gains[i];
      model.lanes[i].gainNormalized = snap.gains[i] / 127.0;
      model.lanes[i].muted = snap.mutes[i];
      model.lanes[i].soloed = snap.solos[i];
      updatedLanes.push(i);
    }

    model.globalDensity = snap.globalDensity;
    model.feel.scale = snap.feelScale;
    model.feel.effectiveSwing = model.feel.authoredSwing * snap.feelScale;
    model.feel.effectiveHumanize = model.feel.authoredHumanize * snap.feelScale;
    return updatedLanes;
  }

  /**
   * Starts a beat-synchronized morph between `sceneAId` and `sceneBId` over `durationBeats`.
   */
  startMorph(sceneAId, sceneBId, durationBeats = 8.0, nowBeat = 0.0) {
    this.morphState.sceneAId = sceneAId;
    this.morphState.sceneBId = sceneBId;
    this.morphState.durationBeats = Math.max(0.25, Number(durationBeats) || 8.0);
    this.morphState.progress = 0.0;
    this.morphState.startBeat = Number(nowBeat) || 0.0;
    this.morphState.active = true;
    return this.morphState;
  }

  /**
   * Advances the active scene morph by `deltaBeats` and applies the interpolated
   * parameters to `model` (if provided).
   */
  advanceMorphBeats(deltaBeats, model = null) {
    if (this.morphState.active) {
      const step = Math.max(0, Number(deltaBeats) || 0) / this.morphState.durationBeats;
      this.morphState.progress = Math.min(1.0, Math.max(0.0, this.morphState.progress + step));
      if (this.morphState.progress >= 1.0) {
        this.morphState.active = false;
      }
    }
    return this.evaluateMorphFrame(this.morphState.progress, model);
  }

  /**
   * Evaluates the interpolated scene frame at `progress in [0.0, 1.0]` and optionally
   * applies it to `model`, respecting `this.laneLocks`.
   */
  evaluateMorphFrame(progress, model = null) {
    const t = Math.max(0.0, Math.min(1.0, Number(progress) || 0.0));
    this.morphState.progress = t;
    const sceneA = this.getSnapshot(this.morphState.sceneAId);
    const sceneB = this.getSnapshot(this.morphState.sceneBId);
    if (!sceneA || !sceneB) return null;

    const densities = new Array(8);
    const gains = new Array(8);
    const timbres = new Array(8);

    for (let i = 0; i < 8; i += 1) {
      const locked = Boolean(this.laneLocks[i] || model?.lanes?.[i]?.timbreLocked);
      if (locked) {
        densities[i] = model ? model.lanes[i].densityMultiplier : sceneA.densities[i];
        gains[i] = model ? model.lanes[i].gainCc : sceneA.gains[i];
        timbres[i] = model ? model.lanes[i].activeTimbre : sceneA.timbres[i];
      } else {
        densities[i] =
          sceneA.densities[i] + (sceneB.densities[i] - sceneA.densities[i]) * t;
        gains[i] = Math.round(sceneA.gains[i] + (sceneB.gains[i] - sceneA.gains[i]) * t);
        timbres[i] =
          t >= this.morphState.timbreSwitchPoint ? sceneB.timbres[i] : sceneA.timbres[i];

        if (model) {
          model.lanes[i].densityMultiplier = densities[i];
          model.lanes[i].gainCc = gains[i];
          model.lanes[i].gainNormalized = gains[i] / 127.0;
          model.lanes[i].activeTimbre = timbres[i];
        }
      }
    }

    const globalDensity =
      sceneA.globalDensity + (sceneB.globalDensity - sceneA.globalDensity) * t;
    const feelScale = sceneA.feelScale + (sceneB.feelScale - sceneA.feelScale) * t;

    if (model) {
      model.globalDensity = globalDensity;
      model.feel.scale = feelScale;
      model.feel.effectiveSwing = model.feel.authoredSwing * feelScale;
      model.feel.effectiveHumanize = model.feel.authoredHumanize * feelScale;
    }

    return {
      progress: t,
      densities,
      gains,
      globalDensity,
      feelScale,
      timbres,
    };
  }
}
