//! `nkEditor4` desktop entry point.

use eframe::egui;
use nkeditor::devices::{get_all_device_definitions, get_device_definition, DeviceDefinition};
use nkeditor::led_controller::MultiDeviceLedManager;
use nkeditor::mapping_engine::{DeviceMappingConfig, MultiDeviceMappingEngine};
use nkeditor::midi_io::MultiDeviceManager;
use nkeditor::theme::ThemeSettings;

/// Root application state for the `nkEditor4` desktop application.
pub struct NkEditorApp {
    pub theme_settings: ThemeSettings,
    pub devices: Vec<DeviceDefinition>,
    pub active_device_index: usize,
    pub device_manager: MultiDeviceManager,
    pub mapping_engine: MultiDeviceMappingEngine,
    pub led_manager: MultiDeviceLedManager,
    theme_dirty: bool,
}

impl Default for NkEditorApp {
    fn default() -> Self {
        let mut device_manager = MultiDeviceManager::new();
        let _ = device_manager.poll_host_hardware();
        let mut mapping_engine = MultiDeviceMappingEngine::new();
        let mut led_manager = MultiDeviceLedManager::new();
        for (key, session) in &device_manager.sessions {
            if let Some(ref def_id) = session.definition_id {
                if let Some(def) = get_device_definition(def_id) {
                    mapping_engine.register_device_pipeline(
                        key.clone(),
                        DeviceMappingConfig::from_device_definition(&def),
                    );
                    let _ = led_manager.ensure_controller(key, def_id);
                }
            }
        }
        Self {
            theme_settings: ThemeSettings::default(),
            devices: get_all_device_definitions(),
            active_device_index: 0,
            device_manager,
            mapping_engine,
            led_manager,
            theme_dirty: true,
        }
    }
}

impl NkEditorApp {
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let app = Self::default();
        app.theme_settings.apply_to_ctx(&cc.egui_ctx);
        app
    }
}

impl eframe::App for NkEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.theme_dirty {
            self.theme_settings.apply_to_ctx(ctx);
            self.theme_dirty = false;
        }

        egui::TopBottomPanel::top("nkeditor_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("nkEditor4");
                ui.separator();
                for (idx, dev) in self.devices.iter().enumerate() {
                    if ui
                        .selectable_label(
                            self.active_device_index == idx,
                            format!("{} {}", dev.manufacturer, dev.name),
                        )
                        .clicked()
                    {
                        self.active_device_index = idx;
                    }
                }
                ui.separator();
                ui.label(format!(
                    "Connected Hardware: {}",
                    self.device_manager.connected_sessions().len()
                ));
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(dev) = self.devices.get(self.active_device_index) {
                ui.label(format!(
                    "Active Controller Definition: {} {} ({} tracks, {} controls, {} LEDs)",
                    dev.manufacturer,
                    dev.name,
                    dev.track_count,
                    dev.total_control_count(),
                    dev.leds.len()
                ));
            }
            ui.separator();
            for session in self.device_manager.connected_sessions() {
                ui.label(format!(
                    "Online: {} [{}] ALSA={} RawMIDI={} Preset={}",
                    session.display_name,
                    session.fingerprint,
                    session.alsa_seq_addr.as_deref().unwrap_or("-"),
                    session.rawmidi_hw_id.as_deref().unwrap_or("-"),
                    session.active_preset
                ));
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("nkeditor {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--check-init") {
        let app = NkEditorApp::default();
        println!(
            "nkeditor initialized: {} definitions, {} connected hardware controllers, theme={}",
            app.devices.len(),
            app.device_manager.connected_sessions().len(),
            app.theme_settings.preference
        );
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("nkEditor4 — MIDI Controller & 8opPerc Performance Cockpit"),
        ..Default::default()
    };

    eframe::run_native(
        "nkEditor4",
        native_options,
        Box::new(|cc| Ok(Box::new(NkEditorApp::new(cc)))),
    )
}
