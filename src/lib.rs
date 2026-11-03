//! `nkEditor4` — Pure Rust `egui` MIDI Controller Remapper & `8opPerc` Performance Cockpit.
//!
//! Core library exposing the visual theme engine ([`theme`]), hardware device
//! definitions ([`devices`]), port-independent USB/ALSA hardware discovery
//! ([`hardware`]), simultaneous multi-device MIDI controller management
//! ([`midi_io`]), and the multi-device MIDI remapping engine ([`mapping_engine`]).

pub mod devices;
pub mod hardware;
pub mod mapping_engine;
pub mod midi_io;
pub mod theme;
