/**
 * Nord Drum 2 MIDI CC Reference Constants
 *
 * Defines all verified MIDI CC assignments for the Nord Drum 2 Desktop
 * percussion synthesizer. These constants serve as the canonical reference
 * for building mapping presets that target the Nord Drum 2 as a destination.
 *
 * Architecture:
 *   The Nord Drum 2 exposes 6 independent drum pads, each assigned to its
 *   own MIDI channel (factory default: channels 1-6). Every pad responds to
 *   the same set of CC numbers for its synthesis parameters (tone, noise,
 *   click, and mix sections). Global effects (distortion, EQ, echo) are
 *   controlled on a dedicated channel (factory default: channel 10).
 *
 * Usage:
 *   Import `NORD_DRUM_2_CC` for per-pad synthesis parameters, paired with
 *   `NORD_DRUM_2_CHANNELS` to target the correct pad. Import
 *   `NORD_DRUM_2_FX_CC` for global effect parameters on the FX channel.
 *
 *   Example - set Pad 3 tone decay to 100:
 *     sendCC(NORD_DRUM_2_CHANNELS.PAD_3, NORD_DRUM_2_CC.TONE.DECAY, 100)
 *
 *   Example - set global distortion amount to 64:
 *     sendCC(NORD_DRUM_2_CHANNELS.GLOBAL_FX, NORD_DRUM_2_FX_CC.DIST_AMOUNT, 64)
 *
 * Sources:
 *   - Nord Drum 2 MIDI Implementation Chart (Nord / Clavia DMI AB)
 *   - Verified against hardware behavior on firmware v1.x
 */

// =============================================================================
// Per-Channel CC Assignments (same CC numbers on each pad's channel)
// =============================================================================

/**
 * Per-pad MIDI CC numbers for the Nord Drum 2, organized by synthesis section.
 *
 * These CC numbers are identical across all 6 pad channels. The target pad
 * is determined by the MIDI channel, not the CC number.
 *
 * Sections:
 *   - TONE: Oscillator pitch, waveform, timbre, decay, and pitch bend
 *   - NOISE: Noise generator with filter, envelope, and decay controls
 *   - CLICK: Transient click layer for attack definition
 *   - MIX: Output level, pan, and tone/noise balance
 */
export const NORD_DRUM_2_CC = {
  /** Tone oscillator parameters - pitch, waveform, timbre, and decay shaping */
  TONE: {
    /** Spectral character of the tone oscillator (CC 30) */
    SPECTRA: 30,
    /** Pitch coarse tuning, most significant byte (CC 31) */
    PITCH_MSB: 31,
    /** Waveform selection for the tone oscillator (CC 46) */
    WAVE: 46,
    /** Rate at which timbre decays over the note duration (CC 47) */
    TIMBRE_DECAY: 47,
    /** Initial transient emphasis on the tone attack (CC 48) */
    PUNCH: 48,
    /** Decay curve shape: linear, exponential, or gated (CC 49) */
    DECAY_TYPE: 49,
    /** Primary decay time for the tone oscillator (CC 50) */
    DECAY: 50,
    /** Secondary decay time for the low-frequency component (CC 51) */
    DECAY_LO: 51,
    /** Harmonic content / timbral character of the oscillator (CC 52) */
    TIMBRE: 52,
    /** Envelope modulation depth applied to the timbre parameter (CC 53) */
    TIMBRE_ENV: 53,
    /** Pitch bend sweep range in semitones (CC 54) */
    BEND_AMOUNT: 54,
    /** Duration of the pitch bend sweep from attack to resting pitch (CC 55) */
    BEND_TIME: 55,
  },

  /** Noise generator parameters - filter, envelope, and decay shaping */
  NOISE: {
    /** Cutoff frequency of the noise filter (CC 14) */
    FILTER_FREQ: 14,
    /** Filter type selection: LP, HP, BP (CC 15) */
    FILTER_TYPE: 15,
    /** Envelope modulation depth applied to the noise filter (CC 16) */
    FILTER_ENV: 16,
    /** Resonance / Q of the noise filter (CC 17) */
    RESONANCE: 17,
    /** Attack time or rate for the noise envelope (CC 18) */
    ATTACK_RATE: 18,
    /** Attack mode selector (CC 19) */
    ATK_MODE: 19,
    /** Decay curve shape for the noise component (CC 20) */
    DECAY_TYPE: 20,
    /** Primary decay time for the noise generator (CC 21) */
    DECAY: 21,
    /** Secondary decay time for the low-frequency noise component (CC 22) */
    DECAY_LO: 22,
  },

  /** Click transient parameters - attack layer for percussive definition */
  CLICK: {
    /** Volume level of the click transient layer (CC 56) */
    LEVEL: 56,
    /** Click waveform / character selection (CC 57) */
    TYPE: 57,
  },

  /** Mix output parameters - level, pan, and tone/noise balance */
  MIX: {
    /** Overall output level for this pad (CC 7) */
    LEVEL: 7,
    /** Stereo pan position for this pad (CC 10) */
    PAN: 10,
    /** Balance between tone and noise components (CC 58) */
    BALANCE: 58,
  },
} as const;

// =============================================================================
// Global FX CC Assignments (channel 10)
// =============================================================================

/**
 * Global effects MIDI CC numbers for the Nord Drum 2.
 *
 * These parameters are sent on the dedicated FX channel (default channel 10)
 * and affect the combined output of all pads.
 */
export const NORD_DRUM_2_FX_CC = {
  /** Distortion drive amount (CC 23) */
  DIST_AMOUNT: 23,
  /** Distortion algorithm / type selection (CC 24) */
  DIST_TYPE: 24,
  /** EQ center / cutoff frequency (CC 25) */
  EQ_FREQ: 25,
  /** EQ boost or cut amount (CC 26) */
  EQ_GAIN: 26,
  /** Echo regeneration / feedback amount (CC 27) */
  ECHO_FEEDBACK: 27,
  /** Echo wet/dry mix level (CC 28) */
  ECHO_AMOUNT: 28,
} as const;

// =============================================================================
// Channel Assignments
// =============================================================================

/**
 * MIDI channel assignments for the Nord Drum 2.
 *
 * Factory defaults assign pads 1-6 to channels 1-6 respectively.
 * Global effects are controlled on channel 10. All channels use
 * 1-based numbering consistent with the MIDI specification's
 * user-facing convention (internally 0-indexed in the protocol).
 */
export const NORD_DRUM_2_CHANNELS = {
  /** Pad 1 MIDI channel (factory default: 1) */
  PAD_1: 1,
  /** Pad 2 MIDI channel (factory default: 2) */
  PAD_2: 2,
  /** Pad 3 MIDI channel (factory default: 3) */
  PAD_3: 3,
  /** Pad 4 MIDI channel (factory default: 4) */
  PAD_4: 4,
  /** Pad 5 MIDI channel (factory default: 5) */
  PAD_5: 5,
  /** Pad 6 MIDI channel (factory default: 6) */
  PAD_6: 6,
  /** Global FX MIDI channel (factory default: 10) */
  GLOBAL_FX: 10,
} as const;
