/**
 * Korg nanoKONTROL2 Live Form-Batch Rehearsal & Deterministic Dry-Run Demo
 * (`js/audible-form-batch-nanokontrol2-live.js`).
 *
 * Upgrades `/home/renick/claude/8operc-konduktiva/examples/audible-form-batch-midimix-live.js`
 * for the Korg nanoKONTROL2 (`0944:0117`) and `nkEditor4` OSC cockpit:
 * - Track Knobs 1..8 (`CC 16..23`): Per-lane density multiplier (`0.1 ..= 5.0`) latched at 32nd-note boundaries
 * - Track Sliders 1..8 (`CC 0..7`): Per-lane level/decay macro (`0..=127`)
 * - Track Mute Buttons 1..8 (`CC 48..55`): Toggle per-lane voice mute with LED feedback
 * - Track Solo Buttons 1..8 (`CC 32..39`): Toggle per-lane voice solo with priority gating & LED feedback
 * - Track Rec Buttons 1..8 (`CC 64..71`): Queue next-bar timbre reroll for single lane
 * - Cycle (`CC 46`): Queue next full 8-voice timbre set from the 4,096-set reservoir
 * - Track Left / Right (`CC 58 / 59`): Navigate previous / next form in the 4,096-form catalogue
 * - Marker Set (`CC 60`): Atomically capture current 8-voice timbre snapshot into
 *   `nanokontrol2-random-preset-pool.jsonl`
 * - Marker Left / Right (`CC 61 / 62`): Step global density multiplier (`0.1 ..= 5.0`)
 * - Rewind / Forward (`CC 43 / 44`): Step global rhythmic feel scale (`swing` & `humanize`, `CC 64 = 0.5`)
 * - Play / Stop / Record (`CC 41 / 42 / 45`): Transport and WAV capture control
 *
 * Safety Policy:
 * - Defaults to `--dry` (`dryRun: true`) so automated tests and CLI invocations never launch
 *   audible `8opPerc` synthesis unless explicitly cued by the operator with `--live-audio`.
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  PulsedNanoKontrol2Rig,
  DEFAULT_PRESET_POOL_FILENAME,
} from './pulsed-nanokontrol2-rig.js';
import {
  NK2_CC_PLAY,
  NK2_CC_STOP,
  NK2_CC_RECORD,
  NK2_CC_CYCLE,
  NK2_CC_TRACK_LEFT,
  NK2_CC_TRACK_RIGHT,
  NK2_CC_MARKER_SET,
  NK2_CC_MARKER_LEFT,
  NK2_CC_MARKER_RIGHT,
  NK2_CC_REWIND,
  NK2_CC_FORWARD,
} from './op8-nanokontrol2-mapper.js';

export const DEFAULT_BPM = 160;
export const DEFAULT_INITIAL_FORM = 'gqom_drive';
export const DEFAULT_CANDIDATE_COUNT = 100;
export const DEFAULT_AUDITION_COUNT = 10;

/**
 * Runs a complete Korg nanoKONTROL2 `8opPerc` form-batch rehearsal workflow.
 * In `dryRun: true` (default) mode, executes deterministically in memory and
 * writes `nanokontrol2-random-preset-pool.jsonl` to `options.presetPoolPath`
 * without spawning audio processes.
 */
export async function runNanoKontrol2FormBatchRehearsal(options = {}) {
  const dryRun = options.dryRun !== false;
  const rig = new PulsedNanoKontrol2Rig({
    dryRun,
    bpm: options.bpm ?? DEFAULT_BPM,
    form: options.form ?? DEFAULT_INITIAL_FORM,
    seed: options.seed ?? 260729,
    presetPoolPath: options.presetPoolPath,
    oscClient: options.oscClient,
  });

  const events = [];
  rig.on('action', (action) => events.push(action));

  try {
    // 1. Start transport & WAV recording indicator (CC 41 Play, CC 45 Record)
    rig.handleNanoKontrol2Cc(NK2_CC_PLAY, 127);
    rig.handleNanoKontrol2Cc(NK2_CC_RECORD, 127);

    // 2. Advance beat to 0.03 (mid-32nd-note) and adjust Kick (CC 16) & Global Density (CC 62)
    rig.advanceBeatTo(0.03);
    const kickDensityAct = rig.handleNanoKontrol2Cc(16, 127); // 5.0x at next 32nd-note (beat 0.125)
    const globalDensityAct = rig.handleNanoKontrol2Cc(NK2_CC_MARKER_RIGHT, 127);

    // Before beat 0.125, density remains 1.0; at beat 0.125, both latch!
    const preLatchKickDensity = rig.densityState.at(0.1, 'kick');
    rig.advanceBeatTo(0.125);
    const postLatchKickDensity = rig.densityState.at(0.125, 'kick');

    // 3. Mute Hat Open (lane 3, CC 51) and Solo Snare (lane 1, CC 33) + Kick (lane 0, CC 32)
    rig.handleNanoKontrol2Cc(51, 127);
    rig.handleNanoKontrol2Cc(33, 127);
    rig.handleNanoKontrol2Cc(32, 127);
    const soloAudibleMask = rig.model.audibleMask();

    // Un-solo Snare & Kick so all unmuted lanes return to playing
    rig.handleNanoKontrol2Cc(33, 127);
    rig.handleNanoKontrol2Cc(32, 127);

    // 4. Adjust Swing/Humanize Feel via Rewind (CC 43) then back to exact midpoint (CC 44 -> CC 64 = 0.5)
    rig.handleNanoKontrol2Cc(NK2_CC_REWIND, 127);
    const midpointFeelAct = rig.handleNanoKontrol2Cc(NK2_CC_FORWARD, 127);

    // 5. Queue single-lane timbre reroll on Snare (Track 2 Rec, CC 65) for next bar (beat 4.0)
    const snareRerollAct = rig.handleNanoKontrol2Cc(65, 127);
    rig.advanceBeatTo(4.0);
    const snareActiveTimbreAfterBar = rig.players.snare.activeTimbre;

    // 6. Queue full 8-voice timbre set switch via Cycle (CC 46) for next bar (beat 8.0)
    const cycleSetAct = rig.handleNanoKontrol2Cc(NK2_CC_CYCLE, 127);
    rig.advanceBeatTo(8.0);

    // 7. Navigate form bank forward (Track Right CC 59) and backward (Track Left CC 58)
    const nextFormAct = rig.handleNanoKontrol2Cc(NK2_CC_TRACK_RIGHT, 127);
    const prevFormAct = rig.handleNanoKontrol2Cc(NK2_CC_TRACK_LEFT, 127);

    // 8. Capture atomic 8-voice preset snapshot via Marker Set (CC 60)
    const snapshotAct = rig.handleNanoKontrol2Cc(NK2_CC_MARKER_SET, 127);
    const savedPresets = rig.readCapturedPresetPool();

    // 9. Stop transport (CC 42)
    rig.handleNanoKontrol2Cc(NK2_CC_STOP, 127);

    return {
      dryRun,
      hardware: rig.hardware,
      kickDensityPendingBeat: kickDensityAct.pendingBeat,
      globalDensityPendingBeat: globalDensityAct.pendingBeat,
      preLatchKickDensity,
      postLatchKickDensity,
      soloAudibleMask,
      midpointFeelScale: midpointFeelAct.feelScale,
      snareQueuedTargetBeat: snareRerollAct.targetBeat,
      snareActiveTimbreAfterBar,
      cycleSetIndex: cycleSetAct.activeSetIndex,
      nextFormName: nextFormAct.activeFormName,
      prevFormName: prevFormAct.activeFormName,
      capturedSnapshot: snapshotAct.snapshot,
      savedPresets,
      totalActionsEmitted: events.length,
    };
  } finally {
    await rig.stop();
  }
}

// CLI Entrypoint (`node js/audible-form-batch-nanokontrol2-live.js --dry`)
const isMain =
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (isMain) {
  const args = new Set(process.argv.slice(2));
  const dryRun = !args.has('--live-audio') || args.has('--dry');
  runNanoKontrol2FormBatchRehearsal({ dryRun })
    .then((summary) => {
      console.log(
        `[nanokontrol2-live] completed rehearsal (dryRun=${summary.dryRun}, hardwareConnected=${summary.hardware.connected}, actions=${summary.totalActionsEmitted}, savedPresets=${summary.savedPresets.length})`
      );
    })
    .catch((err) => {
      console.error('[nanokontrol2-live] error:', err.message);
      process.exitCode = 1;
    });
}
