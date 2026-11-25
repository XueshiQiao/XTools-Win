mod core;
mod tools;
mod tray;

use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};

use core::autostart::AutoStartManager;
use core::file_log::{self, FileLog};
use core::keyboard_hook::KeyboardHookHub;
use core::notifier::WindowsNotifier;
use core::preferences::PreferencesStore;
use core::process_watcher::RunningProcessInfo;
use core::tool::{ToolRegistry, XTool};
use tools::hyper_capslock::config::{ActionConfig, ActionMappingEntry, Trigger};
use tools::hyper_capslock::service::{
  HyperCapslockService, HyperCapslockSnapshot, HyperCapslockTool,
};
use tools::razer_naga::config::NagaPlateLayout;
use tools::razer_naga::protocol::{NagaDpi, NagaHardwareAction};
use tools::razer_naga::service::{NagaService, NagaSnapshot, NagaTool};
use tools::rog_keyboard::service::{
  RogKeyboardService, RogKeyboardSnapshot, RogKeyboardTool, RogSettingsPatch,
};
use tray::TrayController;

const LOG: FileLog = FileLog::new("App");
const ROG_STATE_EVENT: &str = "rog-keyboard://state-changed";
const NAGA_STATE_EVENT: &str = "razer-naga://state-changed";
const HYPER_STATE_EVENT: &str = "hyper-capslock://state-changed";

pub struct AppRuntimeState {
  pub prefs: Arc<PreferencesStore>,
  pub autostart: Arc<AutoStartManager>,
  pub keyboard_hook: Arc<KeyboardHookHub>,
  pub rog_service: Arc<RogKeyboardService>,
  pub naga_service: Arc<NagaService>,
  pub hyper_service: Arc<HyperCapslockService>,
  pub tray: Arc<TrayController>,
  pub registry: Arc<ToolRegistry>,
  pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneralSettingsPayload {
  pub launch_at_login: bool,
  pub language: Option<String>,
  pub effective_language: String,
  pub version: String,
  pub log_path: String,
}

impl AppRuntimeState {
  fn build_general_settings(&self) -> GeneralSettingsPayload {
    let cfg = self.prefs.get();
    let effective = if self.prefs.effective_is_zh() {
      "zh-Hans".to_string()
    } else {
      "en".to_string()
    };
    GeneralSettingsPayload {
      launch_at_login: self.autostart.is_enabled(),
      language: cfg.language,
      effective_language: effective,
      version: self.version.clone(),
      log_path: file_log::log_file_path().to_string_lossy().to_string(),
    }
  }
}

#[tauri::command]
fn rog_get_state(state: State<'_, Arc<AppRuntimeState>>) -> RogKeyboardSnapshot {
  state.rog_service.snapshot()
}

#[tauri::command]
fn rog_refresh(state: State<'_, Arc<AppRuntimeState>>) {
  state.rog_service.refresh();
}

#[tauri::command]
fn rog_switch_to_windows(state: State<'_, Arc<AppRuntimeState>>) {
  state.rog_service.switch_to_windows();
}

#[tauri::command]
fn rog_update_settings(
  state: State<'_, Arc<AppRuntimeState>>,
  patch: RogSettingsPatch,
) -> Result<RogKeyboardSnapshot, String> {
  state.rog_service.update_settings(patch)
}

#[tauri::command]
fn hyper_get_state(state: State<'_, Arc<AppRuntimeState>>) -> HyperCapslockSnapshot {
  state.hyper_service.snapshot()
}

#[tauri::command]
fn hyper_set_paused(
  state: State<'_, Arc<AppRuntimeState>>,
  paused: bool,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.set_paused(paused)
}

#[tauri::command]
fn hyper_set_auto_pause_enabled(
  state: State<'_, Arc<AppRuntimeState>>,
  enabled: bool,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.set_auto_pause_enabled(enabled)
}

#[tauri::command]
fn hyper_set_auto_pause_notify_sound(
  state: State<'_, Arc<AppRuntimeState>>,
  enabled: bool,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.set_auto_pause_notify_sound(enabled)
}

#[tauri::command]
fn hyper_add_auto_pause_process(
  state: State<'_, Arc<AppRuntimeState>>,
  name: String,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.add_auto_pause_process(name)
}

#[tauri::command]
fn hyper_remove_auto_pause_process(
  state: State<'_, Arc<AppRuntimeState>>,
  name: String,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.remove_auto_pause_process(name)
}

#[tauri::command]
async fn hyper_list_running_processes() -> Vec<RunningProcessInfo> {
  tauri::async_runtime::spawn_blocking(HyperCapslockService::list_running_processes)
    .await
    .unwrap_or_default()
}

#[tauri::command]
fn hyper_upsert_mapping(
  state: State<'_, Arc<AppRuntimeState>>,
  trigger: Trigger,
  action: ActionConfig,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.upsert_mapping(trigger, action)
}

#[tauri::command]
fn hyper_remove_mapping(
  state: State<'_, Arc<AppRuntimeState>>,
  trigger: Trigger,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.remove_mapping(trigger)
}

#[tauri::command]
fn hyper_reset_defaults(
  state: State<'_, Arc<AppRuntimeState>>,
) -> Result<HyperCapslockSnapshot, String> {
  state.hyper_service.reset_defaults()
}

#[tauri::command]
fn hyper_import_mappings(
  state: State<'_, Arc<AppRuntimeState>>,
  mappings_json: String,
) -> Result<HyperCapslockSnapshot, String> {
  let mappings: Vec<ActionMappingEntry> = serde_json::from_str(&mappings_json)
    .map_err(|e| format!("Invalid mappings JSON: {}", e))?;
  state.hyper_service.import_mappings(mappings)
}

#[tauri::command]
fn hyper_export_mappings(state: State<'_, Arc<AppRuntimeState>>) -> Result<String, String> {
  state.hyper_service.export_mappings_json()
}

#[tauri::command]
fn naga_get_state(state: State<'_, Arc<AppRuntimeState>>) -> NagaSnapshot {
  state.naga_service.snapshot()
}

#[tauri::command]
fn naga_refresh(state: State<'_, Arc<AppRuntimeState>>) {
  state.naga_service.refresh();
}

#[tauri::command]
fn naga_set_plate_layout(
  state: State<'_, Arc<AppRuntimeState>>,
  layout: NagaPlateLayout,
) -> Result<NagaSnapshot, String> {
  state.naga_service.set_plate_layout(layout)
}

#[tauri::command]
fn naga_set_active_profile(
  state: State<'_, Arc<AppRuntimeState>>,
  profile: u8,
) -> Result<NagaSnapshot, String> {
  state.naga_service.set_active_profile(profile)
}

#[tauri::command]
fn naga_delete_profile(
  state: State<'_, Arc<AppRuntimeState>>,
  profile: u8,
) -> Result<NagaSnapshot, String> {
  state.naga_service.delete_profile(profile)
}

#[tauri::command]
fn naga_set_button_action(
  state: State<'_, Arc<AppRuntimeState>>,
  button_id: u8,
  action: NagaHardwareAction,
) -> Result<NagaSnapshot, String> {
  state.naga_service.set_button_action(button_id, action)
}

#[tauri::command]
fn naga_reset_plate_defaults(state: State<'_, Arc<AppRuntimeState>>) -> Result<NagaSnapshot, String> {
  state.naga_service.reset_plate_defaults()
}

#[tauri::command]
fn naga_set_dpi_stages(
  state: State<'_, Arc<AppRuntimeState>>,
  active_stage: u8,
  stages: Vec<NagaDpi>,
) -> Result<NagaSnapshot, String> {
  state.naga_service.set_dpi_stages(active_stage, stages)
}

#[tauri::command]
fn naga_set_polling_rate(
  state: State<'_, Arc<AppRuntimeState>>,
  hz: u16,
) -> Result<NagaSnapshot, String> {
  state.naga_service.set_polling_rate(hz)
}

#[tauri::command]
fn app_get_general_settings(state: State<'_, Arc<AppRuntimeState>>) -> GeneralSettingsPayload {
  state.build_general_settings()
}

#[tauri::command]
fn app_set_launch_at_login(
  state: State<'_, Arc<AppRuntimeState>>,
  enabled: bool,
) -> Result<GeneralSettingsPayload, String> {
  state.autostart.set_enabled(enabled)?;
  Ok(state.build_general_settings())
}

#[tauri::command]
fn app_set_language(
  state: State<'_, Arc<AppRuntimeState>>,
  language: Option<String>,
) -> Result<GeneralSettingsPayload, String> {
  state.prefs.update(|cfg| {
    cfg.language = language;
  })?;
  let rog_snap = state.rog_service.snapshot();
  let naga_snap = state.naga_service.snapshot();
  let hyper_snap = state.hyper_service.snapshot();
  let is_zh = state.prefs.effective_is_zh();
  state
    .tray
    .refresh_all(&rog_snap, &naga_snap, &hyper_snap, is_zh);
  state.rog_service.notify_language_changed();
  Ok(state.build_general_settings())
}

#[tauri::command]
fn app_reveal_log_file() -> Result<(), String> {
  file_log::reveal_in_explorer()
}

#[cfg(target_os = "windows")]
fn apply_windows_chrome(window: &tauri::WebviewWindow) {
  use windows::Win32::Foundation::HWND;
  use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWCP_ROUND, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE,
  };

  if let Ok(raw) = window.hwnd() {
    let hwnd = HWND(raw.0);
    unsafe {
      let dark: i32 = 1;
      let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark as *const i32 as *const std::ffi::c_void,
        std::mem::size_of::<i32>() as u32,
      );
      let corner = DWMWCP_ROUND.0;
      let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &corner as *const i32 as *const std::ffi::c_void,
        std::mem::size_of::<i32>() as u32,
      );
    }
  }
}

pub fn run() {
  WindowsNotifier::init();
  LOG.info("XTools starting");

  let app = tauri::Builder::default()
    .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
      TrayController::focus_main_window(app);
    }))
    .plugin(tauri_plugin_opener::init())
    .setup(|app| {
      let prefs = Arc::new(PreferencesStore::load());
      let autostart = Arc::new(AutoStartManager::new());
      let notifier = Arc::new(WindowsNotifier::new());
      let keyboard_hook = KeyboardHookHub::new_shared();

      let rog_service = RogKeyboardService::new(Arc::clone(&prefs), Arc::clone(&notifier));
      let naga_service = NagaService::new(Arc::clone(&prefs));
      let hyper_service = HyperCapslockService::new(
        Arc::clone(&prefs),
        Arc::clone(&keyboard_hook),
        Arc::clone(&notifier),
      );

      let tools: Vec<Arc<dyn XTool>> = vec![
        Arc::new(RogKeyboardTool::new(Arc::clone(&rog_service))),
        Arc::new(NagaTool::new(Arc::clone(&naga_service))),
        Arc::new(HyperCapslockTool::new(Arc::clone(&hyper_service))),
      ];
      let registry = Arc::new(ToolRegistry::new(tools));

      let initial_rog = rog_service.snapshot();
      let initial_naga = naga_service.snapshot();
      let initial_hyper = hyper_service.snapshot();
      let is_zh = prefs.effective_is_zh();
      let tray = TrayController::build(
        app.handle(),
        Arc::clone(&rog_service),
        Arc::clone(&naga_service),
        Arc::clone(&hyper_service),
        &initial_rog,
        &initial_naga,
        &initial_hyper,
        is_zh,
      )?;

      let app_rog: AppHandle = app.handle().clone();
      let tray_rog = Arc::clone(&tray);
      let prefs_rog = Arc::clone(&prefs);
      rog_service.set_observer(move |snap| {
        let is_zh = prefs_rog.effective_is_zh();
        tray_rog.refresh_rog(snap, is_zh);
        let _ = app_rog.emit(ROG_STATE_EVENT, snap);
      });

      let app_naga: AppHandle = app.handle().clone();
      let tray_naga = Arc::clone(&tray);
      let prefs_naga = Arc::clone(&prefs);
      naga_service.set_observer(move |snap| {
        let is_zh = prefs_naga.effective_is_zh();
        tray_naga.refresh_naga(snap, is_zh);
        let _ = app_naga.emit(NAGA_STATE_EVENT, snap);
      });

      let app_hyper: AppHandle = app.handle().clone();
      let tray_hyper = Arc::clone(&tray);
      let prefs_hyper = Arc::clone(&prefs);
      hyper_service.set_observer(move |snap| {
        let is_zh = prefs_hyper.effective_is_zh();
        tray_hyper.refresh_hyper(snap, is_zh);
        let _ = app_hyper.emit(HYPER_STATE_EVENT, snap);
      });

      let version = app.package_info().version.to_string();
      let runtime = Arc::new(AppRuntimeState {
        prefs,
        autostart,
        keyboard_hook,
        rog_service,
        naga_service,
        hyper_service,
        tray,
        registry: Arc::clone(&registry),
        version,
      });
      app.manage(runtime);

      if let Some(main_win) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        apply_windows_chrome(&main_win);

        let start_minimized = std::env::args().any(|arg| arg == "--minimized");
        if !start_minimized {
          let _ = main_win.show();
          let _ = main_win.set_focus();
        }
      }

      registry.activate_all();
      Ok(())
    })
    .on_window_event(|window, event| {
      if window.label() == "main" {
        if let WindowEvent::CloseRequested { api, .. } = event {
          api.prevent_close();
          let _ = window.hide();
        }
      }
    })
    .invoke_handler(tauri::generate_handler![
      rog_get_state,
      rog_refresh,
      rog_switch_to_windows,
      rog_update_settings,
      hyper_get_state,
      hyper_set_paused,
      hyper_set_auto_pause_enabled,
      hyper_set_auto_pause_notify_sound,
      hyper_add_auto_pause_process,
      hyper_remove_auto_pause_process,
      hyper_list_running_processes,
      hyper_upsert_mapping,
      hyper_remove_mapping,
      hyper_reset_defaults,
      hyper_import_mappings,
      hyper_export_mappings,
      naga_get_state,
      naga_refresh,
      naga_set_plate_layout,
      naga_set_active_profile,
      naga_delete_profile,
      naga_set_button_action,
      naga_reset_plate_defaults,
      naga_set_dpi_stages,
      naga_set_polling_rate,
      app_get_general_settings,
      app_set_launch_at_login,
      app_set_language,
      app_reveal_log_file,
    ])
    .build(tauri::generate_context!())
    .expect("error while building XTools");

  app.run(|app_handle, event| {
    if let RunEvent::Exit = event {
      if let Some(state) = app_handle.try_state::<Arc<AppRuntimeState>>() {
        state.registry.shutdown_all();
        state.keyboard_hook.stop();
      }
    }
  });
}
