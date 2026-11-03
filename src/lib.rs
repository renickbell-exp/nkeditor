//! `nkEditor4` — Pure Rust `egui` MIDI Controller Remapper & `8opPerc` Performance Cockpit.
//!
//! Core library exposing the visual theme engine ([`theme`]), hardware device
//! definitions ([`devices`]), port-independent USB/ALSA hardware discovery
//! ([`hardware`]), simultaneous multi-device MIDI controller management
//! ([`midi_io`]), the multi-device MIDI remapping engine ([`mapping_engine`]),
//! the hardware LED controller ([`led_controller`]), the interactive
//! `egui` controller surface and visual mapping inspector ([`ui_editor`]),
//! and the bidirectional UDP OSC server and telemetry broadcaster ([`osc_server`]).

pub mod config_parser;
pub mod devices;
pub mod hardware;
pub mod layer_manager;
pub mod led_controller;
pub mod librarian;
pub mod mapping_engine;
pub mod midi_io;
pub mod op8_cockpit;
pub mod op8_model;
pub mod osc_server;
pub mod snapshot_pool;
pub mod theme;
pub mod ui_editor;
