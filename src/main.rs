//! `nkEditor4` desktop and headless CLI entry point (`src/main.rs`).

use eframe::egui;
use nkeditor::devices::{get_all_device_definitions, get_device_definition, DeviceDefinition};
use nkeditor::hardware::discover_connected_midi_hardware;
use nkeditor::led_controller::MultiDeviceLedManager;
use nkeditor::librarian::PresetLibrarian;
use nkeditor::mapping_engine::{DeviceMappingConfig, MultiDeviceMappingEngine};
use nkeditor::midi_io::MultiDeviceManager;
use nkeditor::osc_server::{Op8RigState, OscServer};
use nkeditor::theme::{parse_theme_preference, ThemeSettings, TwoBowlsBackgroundPreset};
use nkeditor::ui_editor::{render_controller_workspace, ControllerEditorUiState};
use nkeditor::verification_harness::{run_full_verification_and_stress_suite, CliLaunchConfig};

/// Root application state for the `nkEditor4` desktop application.
pub struct NkEditorApp {
    pub theme_settings: ThemeSettings,
    pub devices: Vec<DeviceDefinition>,
    pub active_device_index: usize,
    pub device_manager: MultiDeviceManager,
    pub mapping_engine: MultiDeviceMappingEngine,
    pub led_manager: MultiDeviceLedManager,
    pub ui_state: ControllerEditorUiState,
    pub op8_state: Op8RigState,
    pub osc_server: Option<OscServer>,
    theme_dirty: bool,
}

impl Default for NkEditorApp {
    fn default() -> Self {
        Self::from_cli_config(&CliLaunchConfig::default())
    }
}

impl NkEditorApp {
    /// Initializes [`NkEditorApp`] honoring any command-line overrides (`--osc-port`, `--osc-target`,
    /// `--theme`, `--bg`, `--preset`).
    #[must_use]
    pub fn from_cli_config(cli: &CliLaunchConfig) -> Self {
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

        let mut theme_settings = ThemeSettings::default();
        if let Some(ref theme_name) = cli.theme {
            if let Some(pref) = parse_theme_preference(theme_name) {
                theme_settings.preference = pref;
            }
        }
        if let Some(ref bg_name) = cli.background_preset {
            theme_settings.background_preset = TwoBowlsBackgroundPreset::from_name(bg_name);
        }

        if let Some(ref preset_query) = cli.preset {
            let librarian = PresetLibrarian::new();
            if let Some(preset) = librarian
                .presets
                .iter()
                .find(|p| {
                    p.metadata.id.eq_ignore_ascii_case(preset_query)
                        || p.metadata.name.eq_ignore_ascii_case(preset_query)
                })
            {
                let active_key = ui_state.active_device_key.clone();
                mapping_engine.register_device_pipeline(active_key.clone(), preset.mapping.clone());
                if let Some(session) = device_manager.sessions.get_mut(&active_key) {
                    session.active_preset = preset.metadata.id.clone();
                }
            }
        }

        let listen_addr = format!("127.0.0.1:{}", cli.osc_port);
        let client_targets: Vec<std::net::SocketAddr> = cli
            .osc_target
            .parse::<std::net::SocketAddr>()
            .ok()
            .into_iter()
            .collect();
        let osc_server = OscServer::bind(&listen_addr, client_targets).ok();

        Self {
            theme_settings,
            devices: get_all_device_definitions(),
            active_device_index: 0,
            device_manager,
            mapping_engine,
            led_manager,
            ui_state,
            op8_state: Op8RigState::default(),
            osc_server,
            theme_dirty: true,
        }
    }

    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, cli: &CliLaunchConfig) -> Self {
        let app = Self::from_cli_config(cli);
        app.theme_settings.apply_to_ctx(&cc.egui_ctx);
        app
    }
}

impl eframe::App for NkEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(ref mut osc) = self.osc_server {
            let cmds = osc.poll_and_dispatch(
                &mut self.ui_state,
                &mut self.theme_settings,
                &mut self.device_manager,
                &mut self.mapping_engine,
                &mut self.led_manager,
                &mut self.op8_state,
            );
            if !cmds.is_empty() {
                self.theme_dirty = true;
            }
        }

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

    let cli = CliLaunchConfig::parse_from_args(&args);

    if cli.list_devices {
        let devices = discover_connected_midi_hardware();
        if let Ok(json) = serde_json::to_string_pretty(&devices) {
            println!("{json}");
        }
        return Ok(());
    }

    if args.iter().any(|a| a == "--check-init") {
        let app = NkEditorApp::from_cli_config(&cli);
        println!(
            "nkeditor initialized: {} definitions, {} connected hardware controllers, theme={}",
            app.devices.len(),
            app.device_manager.connected_sessions().len(),
            app.theme_settings.preference
        );
        return Ok(());
    }

    if cli.verify_hardware || cli.dry_stress {
        let report = run_full_verification_and_stress_suite();
        if let Ok(json) = serde_json::to_string_pretty(&report) {
            println!("{json}");
        }
        return Ok(());
    }

    if cli.headless_osc {
        let mut app = NkEditorApp::from_cli_config(&cli);
        if let Some(ref mut osc) = app.osc_server {
            let _ = osc.poll_and_dispatch(
                &mut app.ui_state,
                &mut app.theme_settings,
                &mut app.device_manager,
                &mut app.mapping_engine,
                &mut app.led_manager,
                &mut app.op8_state,
            );
            osc.broadcast_op8_state(&app.op8_state);
        }
        let status = serde_json::json!({
            "mode": "headless-osc",
            "osc_port": cli.osc_port,
            "osc_target": cli.osc_target,
            "theme": app.theme_settings.preference.display_name(),
            "background_preset": app.theme_settings.background_preset.map(|p| p.display_name()),
            "active_device": app.ui_state.active_device_key,
            "connected_devices": app.device_manager.connected_sessions().len(),
        });
        if let Ok(json) = serde_json::to_string_pretty(&status) {
            println!("{json}");
        }
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
        Box::new(move |cc| Ok(Box::new(NkEditorApp::new(cc, &cli)))),
    )
}
