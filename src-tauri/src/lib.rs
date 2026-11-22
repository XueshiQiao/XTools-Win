mod core;
mod tools;
mod tray;

use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};

use core::autostart::AutoStartManager;
use core::file_log::{self, FileLog};
use core::notifier::WindowsNotifier;
use core::preferences::PreferencesStore;
use core::tool::ToolRegistry;
use tools::rog_keyboard::service::{
  RogKeyboardService, RogKeyboardSnapshot, RogKeyboardTool, RogSettingsPatch,
};
use tray::TrayController;

const LOG: FileLog = FileLog::new("App");
const ROG_STATE_EVENT: &str = "rog-keyboard://state-changed";

pub struct AppRuntimeState {
  pub prefs: Arc<PreferencesStore>,
  pub autostart: Arc<AutoStartManager>,
  pub rog_service: Arc<RogKeyboardService>,
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
fn rog_switch_to_mac(state: State<'_, Arc<AppRuntimeState>>) {
  state.rog_service.switch_to_mac();
}

#[tauri::command]
fn rog_update_settings(
  state: State<'_, Arc<AppRuntimeState>>,
  patch: RogSettingsPatch,
) -> Result<RogKeyboardSnapshot, String> {
  state.rog_service.update_settings(patch)
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
  let snap = state.rog_service.snapshot();
  let is_zh = state.prefs.effective_is_zh();
  state.tray.refresh_labels(&snap, is_zh);
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
      let rog_service = RogKeyboardService::new(Arc::clone(&prefs), Arc::clone(&notifier));
      let rog_tool = Arc::new(RogKeyboardTool::new(Arc::clone(&rog_service)));
      let registry = Arc::new(ToolRegistry::new(vec![rog_tool]));

      let initial_snap = rog_service.snapshot();
      let is_zh = prefs.effective_is_zh();
      let tray = TrayController::build(
        app.handle(),
        Arc::clone(&rog_service),
        &initial_snap,
        is_zh,
      )?;

      let app_handle: AppHandle = app.handle().clone();
      let tray_obs = Arc::clone(&tray);
      let prefs_obs = Arc::clone(&prefs);
      rog_service.set_observer(move |snap| {
        let is_zh = prefs_obs.effective_is_zh();
        tray_obs.refresh_labels(snap, is_zh);
        let _ = app_handle.emit(ROG_STATE_EVENT, snap);
      });

      let version = app.package_info().version.to_string();
      let runtime = Arc::new(AppRuntimeState {
        prefs,
        autostart,
        rog_service,
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
      rog_switch_to_mac,
      rog_update_settings,
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
      }
    }
  });
}
