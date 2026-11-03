/**
 * nkEditor4 External Node.js & Konduktiva JavaScript SDK (`js/nkeditor-sdk.js`)
 *
 * Zero-dependency EventEmitter + Promise UDP OSC client (`NkEditorClient`),
 * Konduktiva `MusicalEnvironment` binder, hardware specifications, and theme/curve
 * utilities synchronized with the pure Rust `nkeditor` desktop application.
 */

import { EventEmitter } from 'node:events';
import dgram from 'node:dgram';
import {
  encodeOscMessage,
  decodeOscPacket,
  encodeOscBundle,
} from './osc-codec.js';

export const DEFAULT_OSC_SERVER_PORT = 9000;
export const DEFAULT_OSC_CLIENT_PORT = 9001;

export const YAAE_THEMES = Object.freeze([
  'default',
  'light',
  'dark',
  'solarized_dark',
  'monokai',
  'dracula',
  'nord',
  'gruvbox_dark',
  'light_grayscale',
  'dark_grayscale',
  'high_contrast_bw',
  'high_contrast_color',
  'high_contrast_light',
  'cream_low_contrast',
  'medium_contrast',
]);

export const TWOBOWLS_BACKGROUND_PRESETS = Object.freeze({
  white: '#FFFFFF',
  cream: '#F8F2E4',
  black: '#000000',
  dark_grey: '#181818',
});

export const NANOKONTROL2_SPEC = Object.freeze({
  id: 'nanokontrol2',
  name: 'nanoKONTROL2',
  manufacturer: 'Korg',
  usbVendorId: 0x0944,
  usbProductId: 0x0117,
  trackCount: 8,
  knobs: Object.freeze([16, 17, 18, 19, 20, 21, 22, 23]),
  sliders: Object.freeze([0, 1, 2, 3, 4, 5, 6, 7]),
  solo: Object.freeze([32, 33, 34, 35, 36, 37, 38, 39]),
  mute: Object.freeze([48, 49, 50, 51, 52, 53, 54, 55]),
  rec: Object.freeze([64, 65, 66, 67, 68, 69, 70, 71]),
  transport: Object.freeze({
    play: 41,
    stop: 42,
    rewind: 43,
    forward: 44,
    record: 45,
    cycle: 46,
    track_left: 58,
    track_right: 59,
    marker_set: 60,
    marker_left: 61,
    marker_right: 62,
  }),
  ledCount: 28,
});

export const AKAI_MIDIMIX_SPEC = Object.freeze({
  id: 'akai-midimix',
  name: 'MIDI Mix',
  manufacturer: 'Akai',
  usbVendorId: 0x09e8,
  usbProductId: 0x0031,
  trackCount: 8,
  knobRow1: Object.freeze([16, 20, 24, 28, 46, 50, 54, 58]),
  knobRow2: Object.freeze([17, 21, 25, 29, 47, 51, 55, 59]),
  knobRow3: Object.freeze([18, 22, 26, 30, 48, 52, 56, 60]),
  faders: Object.freeze([19, 23, 27, 31, 49, 53, 57, 61]),
  muteNotes: Object.freeze([1, 4, 7, 10, 13, 16, 19, 22]),
  soloNotes: Object.freeze([2, 5, 8, 11, 14, 17, 20, 23]),
  recArmNotes: Object.freeze([3, 6, 9, 12, 15, 18, 21, 24]),
  master: Object.freeze({
    master_fader: 62,
    bank_left: 25,
    bank_right: 26,
    solo_master: 27,
  }),
  ledCount: 24,
});

export const NORD_DRUM_2_SPEC = Object.freeze({
  tone: Object.freeze({
    SPECTRA: 30,
    PITCH_MSB: 31,
    WAVE: 46,
    TIMBRE_DECAY: 47,
    PUNCH: 48,
    DECAY_TYPE: 49,
    DECAY: 50,
    DECAY_LO: 51,
    TIMBRE: 52,
    TIMBRE_ENV: 53,
    BEND_AMOUNT: 54,
    BEND_TIME: 55,
  }),
  noise: Object.freeze({
    FILTER_FREQ: 14,
    FILTER_TYPE: 15,
    FILTER_ENV: 16,
    RESONANCE: 17,
    ATTACK_RATE: 18,
    ATK_MODE: 19,
    DECAY_TYPE: 20,
    DECAY: 21,
    DECAY_LO: 22,
  }),
  click: Object.freeze({
    LEVEL: 56,
    TYPE: 57,
  }),
  mix: Object.freeze({
    LEVEL: 7,
    PAN: 10,
    BALANCE: 58,
  }),
  fx: Object.freeze({
    DIST_AMOUNT: 23,
    DIST_TYPE: 24,
    EQ_FREQ: 25,
    EQ_GAIN: 26,
    ECHO_FEEDBACK: 27,
    ECHO_AMOUNT: 28,
  }),
  channels: Object.freeze({
    PAD_1: 1,
    PAD_2: 2,
    PAD_3: 3,
    PAD_4: 4,
    PAD_5: 5,
    PAD_6: 6,
    GLOBAL_FX: 10,
  }),
});

export const OSC_ENDPOINTS = Object.freeze({
  status: '/nkeditor/status',
  deviceList: '/nkeditor/device/list',
  deviceSelect: '/nkeditor/device/select',
  presetLoad: '/nkeditor/preset/load',
  presetSave: '/nkeditor/preset/save',
  controlSet: '/nkeditor/control/set',
  ledSet: '/nkeditor/led/set',
  themeSet: '/nkeditor/theme/set',
  layerSelect: '/nkeditor/layer/select',
  op8State: '/nkeditor/op8/state',
  op8Form: '/nkeditor/op8/form',
  op8Timbre: '/nkeditor/op8/timbre',
  op8SnapshotSave: '/nkeditor/op8/snapshot/save',
  op8LaneDensity: (index) => `/nkeditor/op8/lane/${index}/density`,
  op8LaneMute: (index) => `/nkeditor/op8/lane/${index}/mute`,
  op8LaneSolo: (index) => `/nkeditor/op8/lane/${index}/solo`,
});

export const OP8_FAMILY_ORDER = Object.freeze([
  'kick',
  'snare',
  'hat_closed',
  'hat_open',
  'clap',
  'tom',
  'rim',
  'cymbal',
]);

export const RESPONSE_CURVES = Object.freeze([
  'linear',
  'exponential',
  'logarithmic',
  's_curve',
]);

export const SOFT_TAKEOVER_MODES = Object.freeze(['catch', 'jump', 'pickup']);

export const LED_OPERATING_MODES = Object.freeze([
  'internal',
  'external',
  'op8_cockpit',
]);

/**
 * Formats a canonical USB-port-independent HardwareFingerprint string:
 * `vendor_id:product_id:serial:card_name` (e.g. `0944:0117::nanoKONTROL2`).
 */
export function formatHardwareFingerprint({
  vendorId = '0000',
  productId = '0000',
  serial = '',
  cardName = '',
} = {}) {
  const v = String(vendorId || '0000').trim().toLowerCase();
  const p = String(productId || '0000').trim().toLowerCase();
  const s = String(serial || '').trim();
  const c = String(cardName || '').trim();
  return `${v}:${p}:${s}:${c}`;
}

/**
 * Parses a canonical `vendor_id:product_id:serial:card_name` string into its components.
 */
export function parseHardwareFingerprint(str) {
  const parts = String(str).trim().split(':');
  if (parts.length < 4) {
    throw new Error(`Invalid HardwareFingerprint: ${str}`);
  }
  const [vendorId, productId, serial, ...rest] = parts;
  return {
    vendorId: vendorId.toLowerCase(),
    productId: productId.toLowerCase(),
    serial,
    cardName: rest.join(':'),
    usbId: `${vendorId.toLowerCase()}:${productId.toLowerCase()}`,
  };
}

/**
 * Mirrors `ResponseCurve::transform_value` in `src/mapping_engine.rs`.
 */
export function transformContinuousValue(
  rawValue,
  { minValue = 0, maxValue = 127, invert = false, curve = 'linear' } = {}
) {
  const clamped = Math.max(0, Math.min(127, Number(rawValue) || 0));
  let t = clamped / 127.0;
  if (invert) {
    t = 1.0 - t;
  }
  let curved = t;
  if (curve === 'exponential') {
    curved = t * t;
  } else if (curve === 'logarithmic') {
    curved = Math.log(1.0 + 9.0 * t) / Math.log(10.0);
  } else if (curve === 's_curve') {
    curved = t * t * (3.0 - 2.0 * t);
  }
  const low = Math.min(minValue, maxValue);
  const high = Math.max(minValue, maxValue);
  return Math.round(Math.max(0, Math.min(127, low + curved * (high - low))));
}

/**
 * Parses a dot-notated control path (e.g. `"track1.knob"`, `"track3.slider"`, `"transport.play"`)
 * into `{ section, track, controlName, category: 'knob' | 'slider' | 'button' }`.
 */
export function classifyControlPath(controlPath) {
  const str = String(controlPath || '').trim();
  const [prefix, controlName = ''] = str.split('.');
  const trackMatch = /^track(\d+)$/i.exec(prefix);
  const track = trackMatch ? Number(trackMatch[1]) : null;

  let category = 'button';
  if (controlName.startsWith('knob')) {
    category = 'knob';
  } else if (
    controlName === 'slider' ||
    controlName === 'fader' ||
    controlName === 'master_fader'
  ) {
    category = 'slider';
  }

  return {
    controlPath: str,
    section: track !== null ? `track${track}` : prefix,
    track,
    controlName,
    category,
  };
}

// =============================================================================
// NkEditorClient: Bidirectional UDP OSC Client & Konduktiva Bridge
// =============================================================================

/**
 * Bidirectional UDP OSC client for communicating with the Rust `nkEditor4` app.
 *
 * Emits:
 * - `'osc'` (`{ addr, args, typedArgs, rinfo }`) on any incoming OSC message
 * - `'knob'` (`{ deviceKey, controlPath, track, controlName, value, channel, midiNumber }`)
 * - `'slider'` (`{ deviceKey, controlPath, track, controlName, value, channel, midiNumber }`)
 * - `'button'` (`{ deviceKey, controlPath, buttonId, isOn, value }`)
 * - `'op8Action'` (`{ action, address, data }`)
 */
export class NkEditorClient extends EventEmitter {
  constructor({
    serverHost = '127.0.0.1',
    serverPort = DEFAULT_OSC_SERVER_PORT,
    listenHost = '127.0.0.1',
    listenPort = DEFAULT_OSC_CLIENT_PORT,
    requestTimeoutMs = 1500,
  } = {}) {
    super();
    this.serverHost = serverHost;
    this.serverPort = serverPort;
    this.listenHost = listenHost;
    this.listenPort = listenPort;
    this.requestTimeoutMs = requestTimeoutMs;
    this.socket = null;
    this.boundAddress = null;
  }

  /**
   * Binds the local UDP listener socket for receiving telemetry and query replies from Rust.
   */
  async connect() {
    if (this.socket) {
      return this.boundAddress;
    }

    const sock = dgram.createSocket('udp4');
    this.socket = sock;

    sock.on('message', (msgBuf, rinfo) => {
      try {
        const packets = decodeOscPacket(msgBuf);
        for (const pkt of packets) {
          this._handleIncomingOscMessage(pkt, rinfo);
        }
      } catch (err) {
        this.emit('error', err);
      }
    });

    await new Promise((resolve, reject) => {
      const onError = (err) => {
        sock.off('listening', onListening);
        reject(err);
      };
      const onListening = () => {
        sock.off('error', onError);
        resolve();
      };
      sock.once('error', onError);
      sock.once('listening', onListening);
      sock.bind(this.listenPort, this.listenHost);
    });

    this.boundAddress = sock.address();
    this.listenPort = this.boundAddress.port;
    return this.boundAddress;
  }

  /**
   * Closes the UDP socket.
   */
  async close() {
    if (!this.socket) {
      return;
    }
    const sock = this.socket;
    this.socket = null;
    this.boundAddress = null;
    await new Promise((resolve) => {
      try {
        sock.close(() => resolve());
      } catch {
        resolve();
      }
    });
  }

  /**
   * Sends a single OSC message to `serverHost:serverPort`.
   */
  async sendOsc(addr, args = []) {
    if (!this.socket) {
      await this.connect();
    }
    const encoded = encodeOscMessage(addr, args);
    await new Promise((resolve, reject) => {
      this.socket.send(
        encoded,
        0,
        encoded.length,
        this.serverPort,
        this.serverHost,
        (err) => (err ? reject(err) : resolve())
      );
    });
  }

  /**
   * Sends an OSC Bundle containing multiple `{ addr, args }` messages.
   */
  async sendOscBundle(messages) {
    if (!this.socket) {
      await this.connect();
    }
    const encoded = encodeOscBundle(messages);
    await new Promise((resolve, reject) => {
      this.socket.send(
        encoded,
        0,
        encoded.length,
        this.serverPort,
        this.serverHost,
        (err) => (err ? reject(err) : resolve())
      );
    });
  }

  /**
   * Sends an OSC request and awaits the matching reply on `expectedReplyAddr`.
   */
  async requestOsc(
    addr,
    args = [],
    expectedReplyAddr = addr,
    timeoutMs = this.requestTimeoutMs
  ) {
    if (!this.socket) {
      await this.connect();
    }

    return new Promise((resolve, reject) => {
      let timer = null;
      const onMessage = (pkt) => {
        if (pkt.addr === expectedReplyAddr) {
          clearTimeout(timer);
          this.off('osc', onMessage);
          resolve(pkt);
        }
      };

      timer = setTimeout(() => {
        this.off('osc', onMessage);
        reject(
          new Error(`OSC request timeout (${timeoutMs}ms) waiting for ${expectedReplyAddr}`)
        );
      }, timeoutMs);

      this.on('osc', onMessage);
      this.sendOsc(addr, args).catch((err) => {
        clearTimeout(timer);
        this.off('osc', onMessage);
        reject(err);
      });
    });
  }

  // ---------------------------------------------------------------------------
  // High-Level Promise API (Status, Devices, Presets, Controls, LEDs, Themes)
  // ---------------------------------------------------------------------------

  async queryStatus() {
    const reply = await this.requestOsc(OSC_ENDPOINTS.status);
    const jsonStr = reply.args[0];
    return typeof jsonStr === 'string' ? JSON.parse(jsonStr) : jsonStr;
  }

  async queryDeviceList() {
    const reply = await this.requestOsc(OSC_ENDPOINTS.deviceList);
    const jsonStr = reply.args[0];
    return typeof jsonStr === 'string' ? JSON.parse(jsonStr) : jsonStr;
  }

  async selectDevice(deviceId) {
    await this.sendOsc(OSC_ENDPOINTS.deviceSelect, [String(deviceId)]);
  }

  async loadPreset(presetName) {
    await this.sendOsc(OSC_ENDPOINTS.presetLoad, [String(presetName)]);
  }

  async savePreset(presetName) {
    await this.sendOsc(OSC_ENDPOINTS.presetSave, [String(presetName)]);
  }

  async setControl(trackOrPath, controlNameOrValue, maybeValue) {
    if (maybeValue !== undefined) {
      await this.sendOsc(OSC_ENDPOINTS.controlSet, [
        String(trackOrPath),
        String(controlNameOrValue),
        { type: 'i', value: Number(maybeValue) },
      ]);
    } else {
      await this.sendOsc(OSC_ENDPOINTS.controlSet, [
        String(trackOrPath),
        { type: 'i', value: Number(controlNameOrValue) },
      ]);
    }
  }

  async setLed(buttonId, isOn) {
    await this.sendOsc(OSC_ENDPOINTS.ledSet, [
      String(buttonId),
      { type: 'i', value: isOn ? 1 : 0 },
    ]);
  }

  async setTheme(themeName, backgroundPreset) {
    const args = [String(themeName)];
    if (backgroundPreset) {
      args.push(String(backgroundPreset));
    }
    await this.sendOsc(OSC_ENDPOINTS.themeSet, args);
  }

  async selectLayer(page) {
    await this.sendOsc(OSC_ENDPOINTS.layerSelect, [String(page)]);
  }

  // ---------------------------------------------------------------------------
  // High-Level 8opPerc Performance Cockpit API
  // ---------------------------------------------------------------------------

  async queryOp8State() {
    const reply = await this.requestOsc(OSC_ENDPOINTS.op8State);
    const jsonStr = reply.args[0];
    return typeof jsonStr === 'string' ? JSON.parse(jsonStr) : jsonStr;
  }

  async setOp8State(stateObj) {
    const json =
      typeof stateObj === 'string' ? stateObj : JSON.stringify(stateObj);
    await this.sendOsc(OSC_ENDPOINTS.op8State, [json]);
  }

  async navigateOp8Form(directionOrName) {
    await this.sendOsc(OSC_ENDPOINTS.op8Form, [String(directionOrName)]);
  }

  async rerollOp8Timbre(laneOrAll = 'all', timbreName) {
    const args = [String(laneOrAll)];
    if (timbreName) {
      args.push(String(timbreName));
    }
    await this.sendOsc(OSC_ENDPOINTS.op8Timbre, args);
  }

  async setOp8LaneDensity(laneIndex, density) {
    await this.sendOsc(OSC_ENDPOINTS.op8LaneDensity(laneIndex), [
      { type: 'f', value: Number(density) },
    ]);
  }

  async setOp8LaneMute(laneIndex, muted) {
    await this.sendOsc(OSC_ENDPOINTS.op8LaneMute(laneIndex), [
      { type: 'i', value: muted ? 1 : 0 },
    ]);
  }

  async setOp8LaneSolo(laneIndex, soloed) {
    await this.sendOsc(OSC_ENDPOINTS.op8LaneSolo(laneIndex), [
      { type: 'i', value: soloed ? 1 : 0 },
    ]);
  }

  async saveOp8Snapshot(label) {
    const args = label ? [String(label)] : [];
    await this.sendOsc(OSC_ENDPOINTS.op8SnapshotSave, args);
  }

  // ---------------------------------------------------------------------------
  // Real-Time Controller Gesture Subscriptions
  // ---------------------------------------------------------------------------

  onKnob(handler) {
    this.on('knob', handler);
    return () => this.off('knob', handler);
  }

  onSlider(handler) {
    this.on('slider', handler);
    return () => this.off('slider', handler);
  }

  onButton(handler) {
    this.on('button', handler);
    return () => this.off('button', handler);
  }

  onOp8Action(handler) {
    this.on('op8Action', handler);
    return () => this.off('op8Action', handler);
  }

  // ---------------------------------------------------------------------------
  // Konduktiva MusicalEnvironment Live Binding
  // ---------------------------------------------------------------------------

  /**
   * Binds a Konduktiva `MusicalEnvironment` (or compatible live rig object) to this
   * `NkEditorClient` so controller gestures and `/nkeditor/op8/*` actions automatically
   * update the environment's voice lanes, density ladders, mutes/solos, and form navigation.
   */
  bindKonduktivaEnvironment(env, hooks = {}) {
    if (!env || typeof env !== 'object') {
      throw new Error('bindKonduktivaEnvironment requires a MusicalEnvironment object');
    }

    if (!env.op8VoiceLanes) {
      env.op8VoiceLanes = OP8_FAMILY_ORDER.map((family, index) => ({
        index,
        family,
        density: 1.0,
        muted: false,
        soloed: false,
        level: 100,
      }));
    }

    const unsubKnob = this.onKnob((ev) => {
      if (ev.track >= 1 && ev.track <= 8) {
        const lane = env.op8VoiceLanes[ev.track - 1];
        if (lane) {
          // Map 0..127 -> 0.1..5.0 density multiplier
          const density = Number((0.1 + (ev.value / 127.0) * 4.9).toFixed(3));
          lane.density = density;
          if (typeof hooks.onLaneDensityChange === 'function') {
            hooks.onLaneDensityChange(lane.index, density, ev);
          }
        }
      }
    });

    const unsubSlider = this.onSlider((ev) => {
      if (ev.track >= 1 && ev.track <= 8) {
        const lane = env.op8VoiceLanes[ev.track - 1];
        if (lane) {
          lane.level = ev.value;
          if (typeof hooks.onLaneLevelChange === 'function') {
            hooks.onLaneLevelChange(lane.index, ev.value, ev);
          }
        }
      }
    });

    const unsubButton = this.onButton((ev) => {
      if (typeof hooks.onButton === 'function') {
        hooks.onButton(ev);
      }
    });

    const unsubOp8 = this.onOp8Action((ev) => {
      if (ev.action === 'state' && ev.data && Array.isArray(ev.data.lanes)) {
        env.activeForm = ev.data.active_form_name;
        env.bpm = ev.data.bpm;
        env.globalDensity = ev.data.global_density;
        env.feelScale = ev.data.feel_scale;
        ev.op8VoiceLanes = ev.data.lanes.map((l) => ({ ...l }));
      } else if (ev.action === 'laneMute') {
        const lane = env.op8VoiceLanes[ev.data.laneIndex];
        if (lane) {
          lane.muted = ev.data.muted;
        }
      } else if (ev.action === 'laneSolo') {
        const lane = env.op8VoiceLanes[ev.data.laneIndex];
        if (lane) {
          lane.soloed = ev.data.soloed;
        }
      } else if (ev.action === 'laneDensity') {
        const lane = env.op8VoiceLanes[ev.data.laneIndex];
        if (lane) {
          lane.density = ev.data.density;
        }
      }
      if (typeof hooks.onOp8Action === 'function') {
        hooks.onOp8Action(ev);
      }
    });

    return {
      unbind() {
        unsubKnob();
        unsubSlider();
        unsubButton();
        unsubOp8();
      },
    };
  }

  _handleIncomingOscMessage(pkt, rinfo) {
    this.emit('osc', { ...pkt, rinfo });

    const { addr, args } = pkt;

    if (addr === OSC_ENDPOINTS.controlSet) {
      // Rust notify_control_change sends: [device_key, control_path, output_value, output_channel, output_number]
      // While echo of /nkeditor/control/set sends: [control_path, value]
      let deviceKey = 'default';
      let controlPath = '';
      let value = 0;
      let channel = 1;
      let midiNumber = 0;

      if (args.length >= 5) {
        deviceKey = String(args[0]);
        controlPath = String(args[1]);
        value = Number(args[2]) || 0;
        channel = Number(args[3]) || 1;
        midiNumber = Number(args[4]) || 0;
      } else if (args.length >= 2) {
        controlPath = String(args[0]);
        value = Number(args[1]) || 0;
      }

      const info = classifyControlPath(controlPath);
      const payload = {
        deviceKey,
        controlPath: info.controlPath,
        track: info.track,
        controlName: info.controlName,
        value,
        channel,
        midiNumber,
      };

      if (info.category === 'knob') {
        this.emit('knob', payload);
      } else if (info.category === 'slider') {
        this.emit('slider', payload);
      } else {
        this.emit('button', {
          ...payload,
          buttonId: info.controlPath,
          isOn: value >= 64,
        });
      }
    } else if (addr === OSC_ENDPOINTS.ledSet) {
      const buttonId = String(args[0] ?? '');
      const isOn = Boolean(args[1]);
      const deviceKey = String(args[2] ?? 'default');
      const info = classifyControlPath(buttonId);
      this.emit('button', {
        deviceKey,
        controlPath: info.controlPath,
        track: info.track,
        controlName: info.controlName,
        buttonId,
        isOn,
        value: isOn ? 127 : 0,
      });
    } else if (addr.startsWith('/nkeditor/op8/')) {
      if (addr === OSC_ENDPOINTS.op8State) {
        let parsed = args[0];
        if (typeof parsed === 'string') {
          try {
            parsed = JSON.parse(parsed);
          } catch {
            // keep string
          }
        }
        this.emit('op8Action', {
          action: 'state',
          address: addr,
          data: parsed,
        });
      } else if (addr === OSC_ENDPOINTS.op8SnapshotSave) {
        this.emit('op8Action', {
          action: 'snapshotSave',
          address: addr,
          data: { label: args[0] },
        });
      } else {
        const laneMatch = /^\/nkeditor\/op8\/lane\/(\d+)\/(density|mute|solo)$/.exec(
          addr
        );
        if (laneMatch) {
          const laneIndex = Number(laneMatch[1]);
          const prop = laneMatch[2];
          if (prop === 'density') {
            this.emit('op8Action', {
              action: 'laneDensity',
              address: addr,
              data: { laneIndex, density: Number(args[0]) },
            });
          } else if (prop === 'mute') {
            this.emit('op8Action', {
              action: 'laneMute',
              address: addr,
              data: { laneIndex, muted: Boolean(args[0]) },
            });
          } else if (prop === 'solo') {
            this.emit('op8Action', {
              action: 'laneSolo',
              address: addr,
              data: { laneIndex, soloed: Boolean(args[0]) },
            });
          }
        }
      }
    }
  }
}
