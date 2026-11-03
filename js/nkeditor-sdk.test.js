import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  YAAE_THEMES,
  TWOBOWLS_BACKGROUND_PRESETS,
  NANOKONTROL2_SPEC,
  AKAI_MIDIMIX_SPEC,
  NORD_DRUM_2_SPEC,
  OSC_ENDPOINTS,
  OP8_FAMILY_ORDER,
  RESPONSE_CURVES,
  SOFT_TAKEOVER_MODES,
  LED_OPERATING_MODES,
  NkEditorClient,
  formatHardwareFingerprint,
  parseHardwareFingerprint,
  transformContinuousValue,
  classifyControlPath,
  encodeOscString,
  decodeOscString,
  encodeOscMessage,
  decodeOscMessage,
  encodeOscBundle,
  decodeOscPacket,
  Op8PerformanceModel,
  mapMidiCCToDensity,
  mapDensityToMidiCC,
  mapMidiCCToFeelScale,
  mapMidiCCToMidimixFeelScale,
  mapFeelScaleToMidiCC,
  resolveLaneIndex,
  PulsedNanoKontrol2Rig,
  createQuantizedDensityState,
  discoverNanoKontrol2Hardware,
  runNanoKontrol2FormBatchRehearsal,
  DEFAULT_PRESET_POOL_FILENAME,
  SnapshotPoolManager,
  LEGACY_MIDIMIX_POOL_FILENAME,
  normalizeSnapshotRecord,
} from './index.js';

describe('nkEditor4 Companion JavaScript SDK & OSC Codec (Epochs 1-7)', () => {
  it('encodes and decodes 4-byte aligned OSC strings, messages, and bundles', () => {
    const strBuf = encodeOscString('/nkeditor/status');
    assert.equal(strBuf.length % 4, 0);
    const decodedStr = decodeOscString(strBuf, 0);
    assert.equal(decodedStr.value, '/nkeditor/status');
    assert.equal(decodedStr.nextOffset, strBuf.length);

    const msgBuf = encodeOscMessage('/nkeditor/op8/lane/3/density', [
      { type: 'f', value: 2.5 },
      'hat_open',
      42,
      true,
      false,
    ]);
    const decodedMsg = decodeOscMessage(msgBuf);
    assert.equal(decodedMsg.addr, '/nkeditor/op8/lane/3/density');
    assert.equal(decodedMsg.args[0], 2.5);
    assert.equal(decodedMsg.args[1], 'hat_open');
    assert.equal(decodedMsg.args[2], 42);
    assert.equal(decodedMsg.args[3], true);
    assert.equal(decodedMsg.args[4], false);

    // Bundle encoding & decoding
    const bundleBuf = encodeOscBundle([
      { addr: '/nkeditor/theme/set', args: ['nord', 'dark_grey'] },
      { addr: '/nkeditor/op8/form', args: ['next'] },
    ]);
    const bundleMsgs = decodeOscPacket(bundleBuf);
    assert.equal(bundleMsgs.length, 2);
    assert.equal(bundleMsgs[0].addr, '/nkeditor/theme/set');
    assert.deepEqual(bundleMsgs[0].args, ['nord', 'dark_grey']);
    assert.equal(bundleMsgs[1].addr, '/nkeditor/op8/form');
    assert.deepEqual(bundleMsgs[1].args, ['next']);
  });

  it('supports NkEditorClient Promise queries, real-time gesture events, and Konduktiva MusicalEnvironment binding', async () => {
    // Set up a mock UDP OSC responder on an ephemeral port
    const mockServer = dgram.createSocket('udp4');
    await new Promise((resolve) => mockServer.bind(0, '127.0.0.1', resolve));
    const serverPort = mockServer.address().port;

    const client = new NkEditorClient({
      serverHost: '127.0.0.1',
      serverPort,
      listenHost: '127.0.0.1',
      listenPort: 0,
    });
    const clientAddr = await client.connect();

    const receivedOnServer = [];
    mockServer.on('message', (buf) => {
      const msgs = decodeOscPacket(buf);
      for (const m of msgs) {
        receivedOnServer.push(m);
        if (m.addr === '/nkeditor/status') {
          const reply = encodeOscMessage('/nkeditor/status', [
            JSON.stringify({
              active_device: '0944:0117::nanoKONTROL2',
              theme: 'Nord',
              bpm: 132.0,
            }),
          ]);
          mockServer.send(reply, clientAddr.port, '127.0.0.1');
        } else if (m.addr === '/nkeditor/device/list') {
          const reply = encodeOscMessage('/nkeditor/device/list', [
            JSON.stringify([
              {
                fingerprint: '0944:0117::nanoKONTROL2',
                display_name: 'nanoKONTROL2',
              },
            ]),
          ]);
          mockServer.send(reply, clientAddr.port, '127.0.0.1');
        }
      }
    });

    // 1. Test Promise status & device list queries
    const status = await client.queryStatus();
    assert.equal(status.active_device, '0944:0117::nanoKONTROL2');
    assert.equal(status.theme, 'Nord');

    const devices = await client.queryDeviceList();
    assert.equal(devices.length, 1);
    assert.equal(devices[0].fingerprint, '0944:0117::nanoKONTROL2');

    // 2. Bind a Konduktiva MusicalEnvironment and test gesture events from server -> client
    const musicalEnv = { name: '8opPerc-Live-Session' };
    const observedKnobs = [];
    const observedSliders = [];
    const observedButtons = [];
    const observedOp8 = [];

    client.onKnob((e) => observedKnobs.push(e));
    client.onSlider((e) => observedSliders.push(e));
    client.onButton((e) => observedButtons.push(e));
    client.onOp8Action((e) => observedOp8.push(e));

    const binding = client.bindKonduktivaEnvironment(musicalEnv);

    // Server broadcasts a knob movement (track1.knob = 127 -> density 5.0), slider movement (track2.slider = 90), and button LED change
    const pktKnob = encodeOscMessage('/nkeditor/control/set', [
      '0944:0117::nanoKONTROL2',
      'track1.knob',
      127,
      1,
      16,
    ]);
    const pktSlider = encodeOscMessage('/nkeditor/control/set', [
      '0944:0117::nanoKONTROL2',
      'track2.slider',
      90,
      1,
      1,
    ]);
    const pktLed = encodeOscMessage('/nkeditor/led/set', [
      'track3.mute',
      1,
      '0944:0117::nanoKONTROL2',
    ]);
    const pktOp8Mute = encodeOscMessage('/nkeditor/op8/lane/2/mute', [1]);

    await new Promise((resolve) => {
      let count = 0;
      const checkDone = () => {
        count += 1;
        if (count >= 4) resolve();
      };
      client.on('osc', checkDone);
      mockServer.send(pktKnob, clientAddr.port, '127.0.0.1');
      mockServer.send(pktSlider, clientAddr.port, '127.0.0.1');
      mockServer.send(pktLed, clientAddr.port, '127.0.0.1');
      mockServer.send(pktOp8Mute, clientAddr.port, '127.0.0.1');
    });

    assert.equal(observedKnobs.length, 1);
    assert.equal(observedKnobs[0].track, 1);
    assert.equal(observedKnobs[0].value, 127);
    assert.equal(musicalEnv.op8VoiceLanes[0].density, 5.0);

    assert.equal(observedSliders.length, 1);
    assert.equal(observedSliders[0].track, 2);
    assert.equal(musicalEnv.op8VoiceLanes[1].level, 90);

    assert.equal(observedButtons.length, 1);
    assert.equal(observedButtons[0].buttonId, 'track3.mute');
    assert.equal(observedButtons[0].isOn, true);

    assert.equal(observedOp8.length, 1);
    assert.equal(musicalEnv.op8VoiceLanes[2].muted, true);

    binding.unbind();
    await client.close();
    await new Promise((resolve) => mockServer.close(resolve));
  });

  it('supports continuous response curves, min/max scaling, polarity inversion, and LED modes', () => {
    assert.deepEqual(RESPONSE_CURVES, [
      'linear',
      'exponential',
      'logarithmic',
      's_curve',
    ]);
    assert.deepEqual(SOFT_TAKEOVER_MODES, ['catch', 'jump', 'pickup']);
    assert.deepEqual(LED_OPERATING_MODES, [
      'internal',
      'external',
      'op8_cockpit',
    ]);
    assert.equal(
      transformContinuousValue(0, { minValue: 20, maxValue: 100 }),
      20
    );
    assert.equal(
      transformContinuousValue(127, { minValue: 20, maxValue: 100 }),
      100
    );
    assert.equal(
      transformContinuousValue(0, {
        minValue: 20,
        maxValue: 100,
        invert: true,
      }),
      100
    );
    assert.equal(transformContinuousValue(64, { curve: 's_curve' }), 64);
    assert.ok(transformContinuousValue(64, { curve: 'exponential' }) < 64);
    assert.ok(transformContinuousValue(64, { curve: 'logarithmic' }) > 64);

    assert.equal(classifyControlPath('track4.knob').category, 'knob');
    assert.equal(classifyControlPath('track4.slider').category, 'slider');
    assert.equal(classifyControlPath('transport.play').category, 'button');
  });

  it('formats and parses USB-port-independent HardwareFingerprints', () => {
    const fp = formatHardwareFingerprint({
      vendorId: '0944',
      productId: '0117',
      serial: '',
      cardName: 'nanoKONTROL2',
    });
    assert.equal(fp, '0944:0117::nanoKONTROL2');
    const parsed = parseHardwareFingerprint(fp);
    assert.deepEqual(parsed, {
      vendorId: '0944',
      productId: '0117',
      serial: '',
      cardName: 'nanoKONTROL2',
      usbId: '0944:0117',
    });
  });

  it('exposes all 15 yaae-theme preference identifiers and 4 twoBowls background presets', () => {
    assert.equal(YAAE_THEMES.length, 15);
    assert.ok(YAAE_THEMES.includes('default'));
    assert.ok(YAAE_THEMES.includes('solarized_dark'));
    assert.ok(YAAE_THEMES.includes('high_contrast_bw'));
    assert.ok(YAAE_THEMES.includes('cream_low_contrast'));
    assert.ok(YAAE_THEMES.includes('medium_contrast'));

    assert.deepEqual(TWOBOWLS_BACKGROUND_PRESETS, {
      white: '#FFFFFF',
      cream: '#F8F2E4',
      black: '#000000',
      dark_grey: '#181818',
    });
  });

  it('matches Korg nanoKONTROL2 CC, transport, and LED hardware layout', () => {
    assert.equal(NANOKONTROL2_SPEC.id, 'nanokontrol2');
    assert.equal(NANOKONTROL2_SPEC.usbVendorId, 0x0944);
    assert.equal(NANOKONTROL2_SPEC.usbProductId, 0x0117);
    assert.equal(NANOKONTROL2_SPEC.trackCount, 8);
    assert.deepEqual(
      NANOKONTROL2_SPEC.knobs,
      [16, 17, 18, 19, 20, 21, 22, 23]
    );
    assert.deepEqual(NANOKONTROL2_SPEC.sliders, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert.deepEqual(NANOKONTROL2_SPEC.solo, [32, 33, 34, 35, 36, 37, 38, 39]);
    assert.deepEqual(NANOKONTROL2_SPEC.mute, [48, 49, 50, 51, 52, 53, 54, 55]);
    assert.deepEqual(NANOKONTROL2_SPEC.rec, [64, 65, 66, 67, 68, 69, 70, 71]);
    assert.equal(Object.keys(NANOKONTROL2_SPEC.transport).length, 11);
    assert.equal(NANOKONTROL2_SPEC.ledCount, 28);
  });

  it('matches Akai MIDI Mix CC/Note and LED hardware layout', () => {
    assert.equal(AKAI_MIDIMIX_SPEC.id, 'akai-midimix');
    assert.equal(AKAI_MIDIMIX_SPEC.trackCount, 8);
    assert.deepEqual(
      AKAI_MIDIMIX_SPEC.knobRow1,
      [16, 20, 24, 28, 46, 50, 54, 58]
    );
    assert.deepEqual(
      AKAI_MIDIMIX_SPEC.knobRow2,
      [17, 21, 25, 29, 47, 51, 55, 59]
    );
    assert.deepEqual(
      AKAI_MIDIMIX_SPEC.knobRow3,
      [18, 22, 26, 30, 48, 52, 56, 60]
    );
    assert.deepEqual(
      AKAI_MIDIMIX_SPEC.faders,
      [19, 23, 27, 31, 49, 53, 57, 61]
    );
    assert.deepEqual(
      AKAI_MIDIMIX_SPEC.muteNotes,
      [1, 4, 7, 10, 13, 16, 19, 22]
    );
    assert.deepEqual(
      AKAI_MIDIMIX_SPEC.soloNotes,
      [2, 5, 8, 11, 14, 17, 20, 23]
    );
    assert.deepEqual(
      AKAI_MIDIMIX_SPEC.recArmNotes,
      [3, 6, 9, 12, 15, 18, 21, 24]
    );
    assert.equal(AKAI_MIDIMIX_SPEC.master.master_fader, 62);
    assert.equal(AKAI_MIDIMIX_SPEC.ledCount, 24);
  });

  it('matches Nord Drum 2 Desktop synthesis and global FX CC constants', () => {
    assert.equal(NORD_DRUM_2_SPEC.tone.SPECTRA, 30);
    assert.equal(NORD_DRUM_2_SPEC.tone.DECAY, 50);
    assert.equal(NORD_DRUM_2_SPEC.noise.FILTER_FREQ, 14);
    assert.equal(NORD_DRUM_2_SPEC.click.LEVEL, 56);
    assert.equal(NORD_DRUM_2_SPEC.mix.LEVEL, 7);
    assert.equal(NORD_DRUM_2_SPEC.fx.DIST_AMOUNT, 23);
    assert.equal(NORD_DRUM_2_SPEC.channels.GLOBAL_FX, 10);
    assert.equal(OSC_ENDPOINTS.status, '/nkeditor/status');
    assert.equal(
      OSC_ENDPOINTS.op8LaneDensity(0),
      '/nkeditor/op8/lane/0/density'
    );
    assert.equal(OSC_ENDPOINTS.op8LaneMute(2), '/nkeditor/op8/lane/2/mute');
    assert.equal(OSC_ENDPOINTS.op8LaneSolo(7), '/nkeditor/op8/lane/7/solo');
    assert.equal(OP8_FAMILY_ORDER.length, 8);
  });

  it('models 8opPerc density ladder, exact CC 64 = 0.5 feel curve, and Native nanoKONTROL2 / MIDImix Emulation mapping', () => {
    assert.ok(Math.abs(mapMidiCCToDensity(0) - 0.1) < 1e-12);
    assert.ok(Math.abs(mapMidiCCToDensity(127) - 5.0) < 1e-12);
    assert.equal(mapDensityToMidiCC(mapMidiCCToDensity(72)), 72);

    assert.equal(mapMidiCCToFeelScale(0), 0.0);
    assert.equal(mapMidiCCToFeelScale(64), 0.5);
    assert.equal(mapMidiCCToFeelScale(127), 1.0);
    assert.equal(mapMidiCCToMidimixFeelScale(64), 0.5);
    assert.equal(mapFeelScaleToMidiCC(0.5), 64);

    assert.equal(resolveLaneIndex('kick'), 0);
    assert.equal(resolveLaneIndex('kicks'), 0);
    assert.equal(resolveLaneIndex('percussion'), 4);
    assert.equal(resolveLaneIndex('alt-toms'), 5);

    const model = new Op8PerformanceModel();
    // Native nanoKONTROL2 Knob 1 (CC 16) -> lane 0 density
    const dAct = model.applyNanoKontrol2Cc(16, 127);
    assert.equal(dAct.type, 'SetLaneDensity');
    assert.equal(dAct.lane, 0);
    assert.ok(Math.abs(dAct.density - 5.0) < 1e-9);

    // Solo lane 1 (CC 33) -> only lane 1 audible
    const sAct = model.applyNanoKontrol2Cc(33, 127);
    assert.equal(sAct.type, 'ToggleLaneSolo');
    assert.deepEqual(
      sAct.audibleMask,
      [false, true, false, false, false, false, false, false]
    );

    // Track Left (CC 58) -> wrap to form 4096 (index 4095)
    const fAct = model.applyNanoKontrol2Cc(58, 127);
    assert.equal(fAct.type, 'NavigateForm');
    assert.equal(fAct.activeIndex, 4095);
    assert.equal(fAct.activeFormName, 'form-4096');

    // Cycle (CC 46) -> advance full 8-voice timbre set
    const cAct = model.applyNanoKontrol2Cc(46, 127);
    assert.equal(cAct.type, 'AdvanceTimbreSet');
    assert.equal(cAct.activeSetIndex, 1);

    // Marker Set (CC 60) -> atomic preset pool snapshot capture
    const snapAct = model.applyNanoKontrol2Cc(60, 127);
    assert.equal(snapAct.type, 'CapturePresetPoolSnapshot');
    assert.equal(snapAct.snapshot.id, 'preset-0001');
    assert.ok(snapAct.snapshot.poolBankIndex >= 0 && snapAct.snapshot.poolBankIndex < 4096);

    // MIDImix Emulation: Marker Set (CC 60) -> 32-CC Send All burst
    const burst = model.translateNanoKontrol2ToMidimix(60, 127);
    assert.equal(burst.length, 32);
    assert.equal(burst[0].controller, 16);
    assert.equal(burst[31].controller, 61);
  });

  it('verifies complete Korg nanoKONTROL2 8opPerc live rig rehearsal in deterministic --dry mode with OSC sync and preset pool JSONL', async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'nkeditor4-epoch9-'));
    const poolPath = path.join(tmpDir, DEFAULT_PRESET_POOL_FILENAME);

    // Create a local UDP OSC server to verify OSC synchronization from PulsedNanoKontrol2Rig
    const oscReceiver = dgram.createSocket('udp4');
    const receivedOscAddresses = [];
    oscReceiver.on('message', (msg) => {
      for (const pkt of decodeOscPacket(msg)) {
        receivedOscAddresses.push(pkt.addr);
      }
    });
    await new Promise((resolve) => oscReceiver.bind(0, '127.0.0.1', resolve));
    const serverPort = oscReceiver.address().port;

    const oscClient = new NkEditorClient({
      serverHost: '127.0.0.1',
      serverPort,
      listenPort: 0,
    });

    try {
      await oscClient.connect();
      const summary = await runNanoKontrol2FormBatchRehearsal({
        dryRun: true,
        presetPoolPath: poolPath,
        oscClient,
      });

      assert.equal(summary.dryRun, true);
      assert.equal(summary.hardware.vendorId, '0944');
      assert.equal(summary.hardware.productId, '0117');

      // 32nd-note density latch verification (beat 0.03 -> latches at beat 0.125)
      assert.equal(summary.kickDensityPendingBeat, 0.125);
      assert.equal(summary.globalDensityPendingBeat, 0.125);
      assert.equal(summary.preLatchKickDensity, 1.0);
      assert.ok(summary.postLatchKickDensity > 5.0); // 5.0x kick * stepped global density

      // Solo + Mute priority verification (Kick + Snare soloed -> only lanes 0 and 1 audible)
      assert.deepEqual(
        summary.soloAudibleMask,
        [true, true, false, false, false, false, false, false]
      );

      // Exact midpoint feel scale after Rewind + Forward (CC 64 = 0.5)
      assert.equal(summary.midpointFeelScale, 0.5);

      // Next-bar single-lane timbre reroll & full 8-voice Cycle timbre set
      assert.equal(summary.snareQueuedTargetBeat, 4.0);
      assert.equal(summary.snareActiveTimbreAfterBar, 'snare-var-0001');
      assert.equal(summary.cycleSetIndex, 1);

      // Form bank navigation (Track Right -> form-0002, Track Left -> form-0001)
      assert.equal(summary.nextFormName, 'form-0002');
      assert.equal(summary.prevFormName, 'form-0001');

      // Atomic preset snapshot saved to nanokontrol2-random-preset-pool.jsonl via Marker Set (CC 60)
      assert.equal(summary.savedPresets.length, 1);
      assert.equal(summary.savedPresets[0].id, 'preset-0001');
      assert.equal(summary.savedPresets[0].triggerCc, 60);
      assert.equal(summary.savedPresets[0].timbres.kick, 'kick-set-0002');
      assert.ok(fs.existsSync(poolPath));

      // Wait briefly for UDP loopback OSC datagrams to arrive
      await new Promise((r) => setTimeout(r, 25));
      assert.ok(receivedOscAddresses.includes('/nkeditor/op8/lane/0/density'));
      assert.ok(receivedOscAddresses.includes('/nkeditor/op8/lane/3/mute'));
      assert.ok(receivedOscAddresses.includes('/nkeditor/op8/lane/1/solo'));
      assert.ok(receivedOscAddresses.includes('/nkeditor/op8/timbre'));
      assert.ok(receivedOscAddresses.includes('/nkeditor/op8/form'));
      assert.ok(receivedOscAddresses.includes('/nkeditor/op8/snapshot/save'));
    } finally {
      await oscClient.close();
      await new Promise((resolve) => oscReceiver.close(resolve));
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('manages JSONL preset pools (nanokontrol2 + legacy midimix), named scene JSON files, voice locks, and beat-synchronized scene morphing', () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'nkeditor4-epoch14-'));
    const nk2PoolPath = path.join(tmpDir, DEFAULT_PRESET_POOL_FILENAME);
    const midimixPoolPath = path.join(tmpDir, LEGACY_MIDIMIX_POOL_FILENAME);
    const namedJsonPath = path.join(tmpDir, 'gqom-overdrive.scene.json');

    try {
      const pool = new SnapshotPoolManager();
      const model = new Op8PerformanceModel();

      // 1. Capture Scene A from live model and persist to nanokontrol2-random-preset-pool.jsonl
      const sceneA = pool.captureFromModel(model, {
        id: 'scene-a',
        label: 'Boot Scene A',
        rating: 4,
        favorite: true,
      });
      pool.appendSnapshotToJsonlFile(nk2PoolPath, sceneA);
      assert.ok(fs.existsSync(nk2PoolPath));

      // 2. Write and load a legacy midimix-random-preset-pool.jsonl file (using KONDUKTIVA_FAMILY_ORDER keys)
      const legacyRecord = {
        id: 'scene-b',
        label: 'MidiMix Heavy Scene B',
        controller: 'akai-midimix',
        timbres: {
          kicks: 'kick-thunder',
          snares: 'snare-anvil',
          hats: 'hat-razor',
          toms: 'hat-splash',
          bells: 'clap-metal',
          metals: 'tom-sub',
          noise: 'rim-steel',
          fx: 'shaker-grit',
        },
        densities: [3.0, 2.5, 4.0, 2.0, 2.2, 3.2, 1.8, 3.6],
        gains: [120, 116, 100, 96, 110, 114, 98, 92],
        globalDensity: 2.5,
        feelScale: 0.8,
      };
      fs.writeFileSync(midimixPoolPath, `${JSON.stringify(legacyRecord)}\n`, 'utf8');
      assert.equal(pool.loadJsonlFile(midimixPoolPath), 1);

      // 3. Save and reload named timbre-state JSON file, verify rating & favorite toggle
      pool.setRating('scene-b', 5);
      assert.equal(pool.toggleFavorite('scene-b'), true);
      pool.saveNamedSceneJsonFile(namedJsonPath, 'scene-b');
      const reloaded = pool.loadNamedSceneJsonFile(namedJsonPath);
      assert.equal(reloaded.rating, 5);
      assert.equal(reloaded.favorite, true);
      assert.equal(reloaded.timbres[4], 'clap-metal');

      // 4. Lock kick (lane 0) and snare (lane 1) while recalling hat/clap/tom from Scene B
      model.lanes[0].activeTimbre = 'kick-locked-anchor';
      model.lanes[0].densityMultiplier = 1.25;
      model.lanes[1].activeTimbre = 'snare-locked-anchor';
      model.lanes[1].densityMultiplier = 1.5;

      pool.setLaneLock('kick', true);
      pool.setLaneLock('snare', true);

      const updatedLanes = pool.recallSnapshotIntoModel('scene-b', model);
      assert.deepEqual(updatedLanes, [2, 3, 4, 5, 6, 7]);
      assert.equal(model.lanes[0].activeTimbre, 'kick-locked-anchor');
      assert.equal(model.lanes[1].activeTimbre, 'snare-locked-anchor');
      assert.equal(model.lanes[2].activeTimbre, 'hat-razor');
      assert.equal(model.lanes[4].activeTimbre, 'clap-metal');
      assert.equal(model.lanes[5].activeTimbre, 'tom-sub');

      // 5. Smoothly morph continuous density/macro parameters from Scene A to Scene B over 8 beats
      pool.startMorph('scene-a', 'scene-b', 8.0, 0.0);
      const midFrame = pool.advanceMorphBeats(4.0, model);
      assert.ok(Math.abs(midFrame.progress - 0.5) < 1e-9);
      // Locked kick stays at 1.25; unlocked hat_closed (lane 2) morphs halfway from 1.0 to 4.0 -> 2.5
      assert.ok(Math.abs(midFrame.densities[0] - 1.25) < 1e-9);
      assert.ok(Math.abs(midFrame.densities[2] - 2.5) < 1e-9);
      assert.ok(Math.abs(midFrame.globalDensity - 1.75) < 1e-9);

      const endFrame = pool.advanceMorphBeats(4.0, model);
      assert.ok(Math.abs(endFrame.progress - 1.0) < 1e-9);
      assert.equal(pool.morphState.active, false);
      assert.ok(Math.abs(endFrame.densities[2] - 4.0) < 1e-9);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
