//! `nkEditor4` desktop entry point.

use eframe::egui;
use nkeditor::devices::{get_all_device_definitions, get_device_definition, DeviceDefinition};
use nkeditor::led_controller::MultiDeviceLedManager;
use nkeditor::mapping_engine::{DeviceMappingConfig, MultiDeviceMappingEngine};
use nkeditor::midi_io::MultiDeviceManager;
use nkeditor::theme::ThemeSettings;
use nkeditor::ui_editor::{render_controller_workspace, ControllerEditorUiState};

/// Root application state for the `nkEditor4` desktop application.
pub struct NkEditorApp {
    pub theme_settings: ThemeSettings,
    pub devices: Vec<DeviceDefinition>,
    pub active_device_index: usize,
    pub device_manager: MultiDeviceManager,
    pub mapping_engine: MultiDeviceMappingEngine,
    pub led_manager: MultiDeviceLedManager,
    pub ui_state: ControllerEditorUiState,
    theme_dirty: bool,
}

impl Default for NkEditorApp {
    fn default() -> Self {
        let mut device_manager = MultiDeviceManager::new();
        let _ = device_manager.poll_host_hardware();
        let mut mapping_engine = MultiDeviceMappingEngine::new();
        let mut led_manager = MultiDeviceLedManager::new();
        let mut ui_state = ControllerEditorUiState::default();
        for (key, session) in &device_manager.sessions {
            if let Some(ref def_id) = session.definition_id {
                if let Some(def) = get_device_definition(def_id) {
                    mapping_engine.register_device_pipeline(
                        key.clone(),
                        DeviceMappingConfig::from_device_definition(&def),
                    );
                    let _ = led_manager.ensure_controller(key, def_id);
                    ui_state.select_device(key.clone(), def_id);
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
            ui_state,
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

        if render_controller_workspace(
            ctx,
            &mut self.ui_state,
            &mut self.theme_settings,
            &self.device_manager,
            &mut self.mapping_engine,
            &mut self.led_manager,
        ) {
            self.theme_dirty = true;
        }
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
