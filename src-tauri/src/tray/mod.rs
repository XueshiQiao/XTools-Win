mod battery_icon;

use std::sync::{Arc, Mutex};
use tauri::{
  image::Image,
  menu::{Menu, MenuItem, PredefinedMenuItem},
  tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
  AppHandle, Manager, Wry,
};

use battery_icon::BatteryIconRenderer;
use crate::tools::rog_keyboard::protocol::{
  RogBatteryInfo, RogChargingState, RogModel, RogProfileNaming,
};
use crate::tools::rog_keyboard::service::{RogKeyboardService, RogKeyboardSnapshot};

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../icons/32x32.png");

struct MainTrayItems {
  status_item: MenuItem<Wry>,
  battery_item: MenuItem<Wry>,
  to_windows_item: MenuItem<Wry>,
  open_item: MenuItem<Wry>,
  quit_item: MenuItem<Wry>,
}

struct BatteryTrayItems {
  battery_item: MenuItem<Wry>,
  profile_item: MenuItem<Wry>,
  refresh_item: MenuItem<Wry>,
  to_windows_item: MenuItem<Wry>,
  open_item: MenuItem<Wry>,
  quit_item: MenuItem<Wry>,
}

/// Manages dual Windows notification-area icons:
/// 1) `main`: XTools brand tray icon + tool quick switches.
/// 2) `rog_battery`: dedicated numeric battery badge tray icon visible while
///    the ROG Falcata is connected (Option B).
pub struct TrayController {
  main_items: Mutex<MainTrayItems>,
  battery_items: Mutex<BatteryTrayItems>,
  battery_tray: TrayIcon<Wry>,
  last_badge: Mutex<Option<(u8, RogChargingState)>>,
  last_visible: Mutex<bool>,
}

impl TrayController {
  pub fn build(
    app: &AppHandle,
    rog_service: Arc<RogKeyboardService>,
    initial_snapshot: &RogKeyboardSnapshot,
    is_zh: bool,
  ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
    let version = app.package_info().version.to_string();
    let title_item = MenuItem::with_id(app, "title", format!("XTools v{}", version), false, None::<&str>)?;

    let status_item = MenuItem::with_id(
      app,
      "rog_status",
      Self::status_text(initial_snapshot, is_zh),
      false,
      None::<&str>,
    )?;
    let main_battery_item = MenuItem::with_id(
      app,
      "rog_battery_status",
      Self::battery_text(initial_snapshot, is_zh),
      false,
      None::<&str>,
    )?;
    let to_windows_item = MenuItem::with_id(
      app,
      "rog_switch_windows",
      Self::windows_action_text(is_zh),
      true,
      None::<&str>,
    )?;
    let open_item = MenuItem::with_id(
      app,
      "open_main",
      Self::open_text(is_zh),
      true,
      None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(
      app,
      "quit",
      Self::quit_text(is_zh),
      true,
      None::<&str>,
    )?;

    let main_menu = Menu::with_items(
      app,
      &[
        &title_item,
        &PredefinedMenuItem::separator(app)?,
        &status_item,
        &main_battery_item,
        &to_windows_item,
        &PredefinedMenuItem::separator(app)?,
        &open_item,
        &PredefinedMenuItem::separator(app)?,
        &quit_item,
      ],
    )?;

    let main_icon = Image::from_bytes(TRAY_ICON_BYTES)?;
    let rog_for_menu = Arc::clone(&rog_service);

    // IMPORTANT: In Tauri 2, `on_menu_event` registers a global app-wide listener.
    // Register ONLY ONE handler here covering both tray menus' distinct item IDs.
    TrayIconBuilder::with_id("main")
      .icon(main_icon)
      .tooltip("XTools")
      .menu(&main_menu)
      .show_menu_on_left_click(false)
      .on_menu_event(move |app_handle, event| {
        match event.id.as_ref() {
          "rog_switch_windows" | "bat_switch_windows" => rog_for_menu.switch_to_windows(),
          "bat_refresh" => rog_for_menu.refresh(),
          "open_main" | "bat_open_main" => Self::focus_main_window(app_handle),
          "quit" | "bat_quit" => app_handle.exit(0),
          _ => {}
        }
      })
      .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
          button: MouseButton::Left,
          button_state: MouseButtonState::Up,
          ..
        } = event
        {
          Self::focus_main_window(tray.app_handle());
        }
      })
      .build(app)?;

    // Build dedicated ROG battery tray icon (Tray #2)
    let bat_status_item = MenuItem::with_id(
      app,
      "bat_status",
      Self::battery_text(initial_snapshot, is_zh),
      false,
      None::<&str>,
    )?;
    let bat_profile_item = MenuItem::with_id(
      app,
      "bat_profile_status",
      Self::status_text(initial_snapshot, is_zh),
      false,
      None::<&str>,
    )?;
    let bat_refresh_item = MenuItem::with_id(
      app,
      "bat_refresh",
      Self::refresh_battery_text(is_zh),
      true,
      None::<&str>,
    )?;
    let bat_windows_item = MenuItem::with_id(
      app,
      "bat_switch_windows",
      Self::windows_action_text(is_zh),
      true,
      None::<&str>,
    )?;
    let bat_open_item = MenuItem::with_id(
      app,
      "bat_open_main",
      Self::open_text(is_zh),
      true,
      None::<&str>,
    )?;
    let bat_quit_item = MenuItem::with_id(
      app,
      "bat_quit",
      Self::quit_text(is_zh),
      true,
      None::<&str>,
    )?;

    let bat_menu = Menu::with_items(
      app,
      &[
        &bat_status_item,
        &bat_profile_item,
        &PredefinedMenuItem::separator(app)?,
        &bat_refresh_item,
        &bat_windows_item,
        &PredefinedMenuItem::separator(app)?,
        &bat_open_item,
        &bat_quit_item,
      ],
    )?;

    let placeholder_battery = RogBatteryInfo {
      percent: 100,
      charging_state: RogChargingState::Unknown,
      left_percent: None,
      left_mv: None,
      right_percent: None,
      right_mv: None,
    };
    let initial_bat_icon = BatteryIconRenderer::render(&placeholder_battery);

    let battery_tray = TrayIconBuilder::with_id("rog_battery")
      .icon(initial_bat_icon)
      .tooltip(RogModel::DISPLAY_NAME)
      .menu(&bat_menu)
      .show_menu_on_left_click(false)
      .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
          button: MouseButton::Left,
          button_state: MouseButtonState::Up,
          ..
        } = event
        {
          Self::focus_main_window(tray.app_handle());
        }
      })
      .build(app)?;

    let _ = battery_tray.set_visible(false);

    Ok(Arc::new(Self {
      main_items: Mutex::new(MainTrayItems {
        status_item,
        battery_item: main_battery_item,
        to_windows_item,
        open_item,
        quit_item,
      }),
      battery_items: Mutex::new(BatteryTrayItems {
        battery_item: bat_status_item,
        profile_item: bat_profile_item,
        refresh_item: bat_refresh_item,
        to_windows_item: bat_windows_item,
        open_item: bat_open_item,
        quit_item: bat_quit_item,
      }),
      battery_tray,
      last_badge: Mutex::new(None),
      last_visible: Mutex::new(false),
    }))
  }

  pub fn refresh_labels(&self, snapshot: &RogKeyboardSnapshot, is_zh: bool) {
    let profile_text = Self::status_text(snapshot, is_zh);
    let battery_text = Self::battery_text(snapshot, is_zh);

    if let Ok(items) = self.main_items.lock() {
      let _ = items.status_item.set_text(&profile_text);
      let _ = items.battery_item.set_text(&battery_text);
      let _ = items.to_windows_item.set_text(Self::windows_action_text(is_zh));
      let _ = items.open_item.set_text(Self::open_text(is_zh));
      let _ = items.quit_item.set_text(Self::quit_text(is_zh));
    }

    if let Ok(items) = self.battery_items.lock() {
      let _ = items.battery_item.set_text(&battery_text);
      let _ = items.profile_item.set_text(&profile_text);
      let _ = items.refresh_item.set_text(Self::refresh_battery_text(is_zh));
      let _ = items.to_windows_item.set_text(Self::windows_action_text(is_zh));
      let _ = items.open_item.set_text(Self::open_text(is_zh));
      let _ = items.quit_item.set_text(Self::quit_text(is_zh));
    }

    // Update dedicated ROG battery tray icon outside any held mutex locks.
    let should_show = snapshot.is_connected && snapshot.battery.is_some();
    if let (true, Some(bat)) = (should_show, &snapshot.battery) {
      let badge_key = (bat.percent, bat.charging_state);
      let needs_icon = {
        let mut last = self.last_badge.lock().unwrap();
        if *last != Some(badge_key) {
          *last = Some(badge_key);
          true
        } else {
          false
        }
      };
      if needs_icon {
        let icon = BatteryIconRenderer::render(bat);
        let _ = self.battery_tray.set_icon(Some(icon));
      }

      let tip = Self::battery_tooltip(snapshot, bat, is_zh);
      let _ = self.battery_tray.set_tooltip(Some(tip));
    }

    let vis_changed = {
      let mut vis = self.last_visible.lock().unwrap();
      if *vis != should_show {
        *vis = should_show;
        true
      } else {
        false
      }
    };
    if vis_changed {
      let _ = self.battery_tray.set_visible(should_show);
    }
  }

  pub fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
      let _ = window.unminimize();
      let _ = window.show();
      let _ = window.set_focus();
    }
  }

  fn status_text(snapshot: &RogKeyboardSnapshot, is_zh: bool) -> String {
    if snapshot.is_connected {
      if let Some(current) = snapshot.current_profile {
        let label = RogProfileNaming::label(current, snapshot.profile_count, is_zh);
        return if is_zh {
          format!("ROG 键盘 —— 当前 {}", label)
        } else {
          format!("ROG Keyboard — {}", label)
        };
      }
    }
    if is_zh {
      "ROG 键盘 —— 未连接".to_string()
    } else {
      "ROG Keyboard — Not connected".to_string()
    }
  }

  fn battery_text(snapshot: &RogKeyboardSnapshot, is_zh: bool) -> String {
    if !snapshot.is_connected {
      return if is_zh {
        "电量 —— 未连接".to_string()
      } else {
        "Battery — Not connected".to_string()
      };
    }
    let Some(bat) = &snapshot.battery else {
      return if is_zh {
        "电量 —— 读取中…".to_string()
      } else {
        "Battery — Reading…".to_string()
      };
    };

    let mut parts = Vec::new();
    if let (Some(l), Some(r)) = (bat.left_percent, bat.right_percent) {
      parts.push(if is_zh {
        format!("左 {}% · 右 {}%", l, r)
      } else {
        format!("L {}% · R {}%", l, r)
      });
    }
    match bat.charging_state {
      RogChargingState::Charging => parts.push(if is_zh { "充电中" } else { "Charging" }.to_string()),
      RogChargingState::Full => parts.push(if is_zh { "已充满" } else { "Full" }.to_string()),
      _ => {}
    }
    if parts.is_empty() {
      if is_zh {
        format!("电量 —— {}%", bat.percent)
      } else {
        format!("Battery — {}%", bat.percent)
      }
    } else if is_zh {
      format!("电量 —— {}%（{}）", bat.percent, parts.join(" · "))
    } else {
      format!("Battery — {}% ({})", bat.percent, parts.join(" · "))
    }
  }

  fn battery_tooltip(snapshot: &RogKeyboardSnapshot, bat: &RogBatteryInfo, is_zh: bool) -> String {
    let mut extras = Vec::new();
    if let (Some(l), Some(r)) = (bat.left_percent, bat.right_percent) {
      extras.push(if is_zh {
        format!("左 {}% / 右 {}%", l, r)
      } else {
        format!("L {}% / R {}%", l, r)
      });
    }
    match bat.charging_state {
      RogChargingState::Charging => extras.push(if is_zh { "充电中" } else { "Charging" }.to_string()),
      RogChargingState::Full => extras.push(if is_zh { "已充满" } else { "Full" }.to_string()),
      _ => {}
    }
    let profile = snapshot
      .current_profile
      .map(|slot| RogProfileNaming::label(slot, snapshot.profile_count, is_zh));

    let bat_part = if extras.is_empty() {
      format!("{}%", bat.percent)
    } else {
      format!("{}%（{}）", bat.percent, extras.join(" · "))
    };
    match profile {
      Some(p) => format!("{} · {} · {}", RogModel::DISPLAY_NAME, bat_part, p),
      None => format!("{} · {}", RogModel::DISPLAY_NAME, bat_part),
    }
  }

  fn refresh_battery_text(is_zh: bool) -> &'static str {
    if is_zh { "刷新状态与电量" } else { "Refresh Status & Battery" }
  }

  fn windows_action_text(is_zh: bool) -> &'static str {
    if is_zh { "切到 Windows 档" } else { "Switch to Windows Profile" }
  }

  fn open_text(is_zh: bool) -> &'static str {
    if is_zh { "打开 XTools…" } else { "Open XTools…" }
  }

  fn quit_text(is_zh: bool) -> &'static str {
    if is_zh { "退出 XTools" } else { "Quit XTools" }
  }
}
