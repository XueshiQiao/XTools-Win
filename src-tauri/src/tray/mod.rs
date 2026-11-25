mod battery_icon;

use std::sync::{Arc, Mutex};
use tauri::{
  image::Image,
  menu::{Menu, MenuItem, PredefinedMenuItem},
  tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
  AppHandle, Manager, Wry,
};

use battery_icon::BatteryIconRenderer;
use crate::tools::hyper_capslock::service::{HyperCapslockService, HyperCapslockSnapshot};
use crate::tools::razer_naga::protocol::{NagaBatteryInfo, NagaChargingState, NagaLink, NagaModel};
use crate::tools::razer_naga::service::{NagaService, NagaSnapshot};
use crate::tools::rog_keyboard::protocol::{
  RogBatteryInfo, RogChargingState, RogModel, RogProfileNaming,
};
use crate::tools::rog_keyboard::service::{RogKeyboardService, RogKeyboardSnapshot};

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../icons/32x32.png");

struct MainTrayItems {
  rog_status_item: MenuItem<Wry>,
  rog_battery_item: MenuItem<Wry>,
  rog_to_windows_item: MenuItem<Wry>,
  naga_status_item: MenuItem<Wry>,
  naga_refresh_item: MenuItem<Wry>,
  hyper_toggle_item: MenuItem<Wry>,
  open_item: MenuItem<Wry>,
  quit_item: MenuItem<Wry>,
}

struct RogBatteryTrayItems {
  battery_item: MenuItem<Wry>,
  profile_item: MenuItem<Wry>,
  refresh_item: MenuItem<Wry>,
  to_windows_item: MenuItem<Wry>,
  open_item: MenuItem<Wry>,
  quit_item: MenuItem<Wry>,
}

struct NagaBatteryTrayItems {
  battery_item: MenuItem<Wry>,
  detail_item: MenuItem<Wry>,
  refresh_item: MenuItem<Wry>,
  open_item: MenuItem<Wry>,
  quit_item: MenuItem<Wry>,
}

/// Manages Windows notification-area icons:
/// 1) `main`: XTools brand tray icon + tool status & quick toggles.
/// 2) `rog_battery`: dedicated numeric battery badge tray icon visible while ROG Falcata is connected.
/// 3) `naga_battery`: dedicated numeric battery badge tray icon visible while Razer Naga V3 Pro is connected.
pub struct TrayController {
  main_items: Mutex<MainTrayItems>,
  rog_battery_items: Mutex<RogBatteryTrayItems>,
  rog_battery_tray: TrayIcon<Wry>,
  rog_last_badge: Mutex<Option<(u8, RogChargingState)>>,
  rog_last_visible: Mutex<bool>,
  naga_battery_items: Mutex<NagaBatteryTrayItems>,
  naga_battery_tray: TrayIcon<Wry>,
  naga_last_badge: Mutex<Option<(u8, NagaChargingState)>>,
  naga_last_visible: Mutex<bool>,
}

impl TrayController {
  pub fn build(
    app: &AppHandle,
    rog_service: Arc<RogKeyboardService>,
    naga_service: Arc<NagaService>,
    hyper_service: Arc<HyperCapslockService>,
    initial_rog: &RogKeyboardSnapshot,
    initial_naga: &NagaSnapshot,
    initial_hyper: &HyperCapslockSnapshot,
    is_zh: bool,
  ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
    let version = app.package_info().version.to_string();
    let title_item =
      MenuItem::with_id(app, "title", format!("XTools v{}", version), false, None::<&str>)?;

    let rog_status_item = MenuItem::with_id(
      app,
      "rog_status",
      Self::rog_status_text(initial_rog, is_zh),
      false,
      None::<&str>,
    )?;
    let rog_battery_item = MenuItem::with_id(
      app,
      "rog_battery_status",
      Self::rog_battery_text(initial_rog, is_zh),
      false,
      None::<&str>,
    )?;
    let rog_to_windows_item = MenuItem::with_id(
      app,
      "rog_switch_windows",
      Self::windows_action_text(is_zh),
      true,
      None::<&str>,
    )?;

    let naga_status_item = MenuItem::with_id(
      app,
      "naga_status",
      Self::naga_main_text(initial_naga, is_zh),
      false,
      None::<&str>,
    )?;
    let naga_refresh_item = MenuItem::with_id(
      app,
      "naga_main_refresh",
      Self::naga_refresh_text(is_zh),
      true,
      None::<&str>,
    )?;

    let hyper_toggle_item = MenuItem::with_id(
      app,
      "hyper_main_toggle",
      Self::hyper_toggle_text(initial_hyper, is_zh),
      true,
      None::<&str>,
    )?;

    let open_item =
      MenuItem::with_id(app, "open_main", Self::open_text(is_zh), true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", Self::quit_text(is_zh), true, None::<&str>)?;

    let main_menu = Menu::with_items(
      app,
      &[
        &title_item,
        &PredefinedMenuItem::separator(app)?,
        &rog_status_item,
        &rog_battery_item,
        &rog_to_windows_item,
        &PredefinedMenuItem::separator(app)?,
        &naga_status_item,
        &naga_refresh_item,
        &PredefinedMenuItem::separator(app)?,
        &hyper_toggle_item,
        &PredefinedMenuItem::separator(app)?,
        &open_item,
        &PredefinedMenuItem::separator(app)?,
        &quit_item,
      ],
    )?;

    let main_icon = Image::from_bytes(TRAY_ICON_BYTES)?;
    let rog_for_menu = Arc::clone(&rog_service);
    let naga_for_menu = Arc::clone(&naga_service);
    let hyper_for_menu = Arc::clone(&hyper_service);

    // IMPORTANT: In Tauri 2, `on_menu_event` registers a global app-wide listener.
    // Register ONLY ONE handler here covering all tray menus' distinct item IDs.
    TrayIconBuilder::with_id("main")
      .icon(main_icon)
      .tooltip("XTools")
      .menu(&main_menu)
      .show_menu_on_left_click(false)
      .on_menu_event(move |app_handle, event| match event.id.as_ref() {
        "rog_switch_windows" | "bat_switch_windows" => rog_for_menu.switch_to_windows(),
        "bat_refresh" => rog_for_menu.refresh(),
        "naga_main_refresh" | "naga_bat_refresh" => naga_for_menu.refresh(),
        "hyper_main_toggle" => {
          let next_paused = !hyper_for_menu.snapshot().paused;
          let _ = hyper_for_menu.set_paused(next_paused);
        }
        "open_main" | "bat_open_main" | "naga_bat_open_main" => {
          Self::focus_main_window(app_handle);
        }
        "quit" | "bat_quit" | "naga_bat_quit" => app_handle.exit(0),
        _ => {}
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
      Self::rog_battery_text(initial_rog, is_zh),
      false,
      None::<&str>,
    )?;
    let bat_profile_item = MenuItem::with_id(
      app,
      "bat_profile_status",
      Self::rog_status_text(initial_rog, is_zh),
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

    let rog_bat_menu = Menu::with_items(
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

    let initial_rog_bat_icon = BatteryIconRenderer::render(&RogBatteryInfo {
      percent: 100,
      charging_state: RogChargingState::Unknown,
      left_percent: None,
      left_mv: None,
      right_percent: None,
      right_mv: None,
    });

    let rog_battery_tray = TrayIconBuilder::with_id("rog_battery")
      .icon(initial_rog_bat_icon)
      .tooltip(RogModel::DISPLAY_NAME)
      .menu(&rog_bat_menu)
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
    let _ = rog_battery_tray.set_visible(false);

    // Build dedicated Razer Naga battery tray icon (Tray #3)
    let naga_bat_status_item = MenuItem::with_id(
      app,
      "naga_bat_status",
      Self::naga_battery_text(initial_naga, is_zh),
      false,
      None::<&str>,
    )?;
    let naga_bat_detail_item = MenuItem::with_id(
      app,
      "naga_bat_detail",
      Self::naga_detail_text(initial_naga, is_zh),
      false,
      None::<&str>,
    )?;
    let naga_bat_refresh_item = MenuItem::with_id(
      app,
      "naga_bat_refresh",
      Self::refresh_battery_text(is_zh),
      true,
      None::<&str>,
    )?;
    let naga_bat_open_item = MenuItem::with_id(
      app,
      "naga_bat_open_main",
      Self::open_text(is_zh),
      true,
      None::<&str>,
    )?;
    let naga_bat_quit_item = MenuItem::with_id(
      app,
      "naga_bat_quit",
      Self::quit_text(is_zh),
      true,
      None::<&str>,
    )?;

    let naga_bat_menu = Menu::with_items(
      app,
      &[
        &naga_bat_status_item,
        &naga_bat_detail_item,
        &PredefinedMenuItem::separator(app)?,
        &naga_bat_refresh_item,
        &PredefinedMenuItem::separator(app)?,
        &naga_bat_open_item,
        &naga_bat_quit_item,
      ],
    )?;

    let initial_naga_bat_icon = BatteryIconRenderer::render_naga(&NagaBatteryInfo {
      percent: 100,
      charging_state: NagaChargingState::Unknown,
    });

    let naga_battery_tray = TrayIconBuilder::with_id("naga_battery")
      .icon(initial_naga_bat_icon)
      .tooltip(NagaModel::DISPLAY_NAME)
      .menu(&naga_bat_menu)
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
    let _ = naga_battery_tray.set_visible(false);

    Ok(Arc::new(Self {
      main_items: Mutex::new(MainTrayItems {
        rog_status_item,
        rog_battery_item,
        rog_to_windows_item,
        naga_status_item,
        naga_refresh_item,
        hyper_toggle_item,
        open_item,
        quit_item,
      }),
      rog_battery_items: Mutex::new(RogBatteryTrayItems {
        battery_item: bat_status_item,
        profile_item: bat_profile_item,
        refresh_item: bat_refresh_item,
        to_windows_item: bat_windows_item,
        open_item: bat_open_item,
        quit_item: bat_quit_item,
      }),
      rog_battery_tray,
      rog_last_badge: Mutex::new(None),
      rog_last_visible: Mutex::new(false),
      naga_battery_items: Mutex::new(NagaBatteryTrayItems {
        battery_item: naga_bat_status_item,
        detail_item: naga_bat_detail_item,
        refresh_item: naga_bat_refresh_item,
        open_item: naga_bat_open_item,
        quit_item: naga_bat_quit_item,
      }),
      naga_battery_tray,
      naga_last_badge: Mutex::new(None),
      naga_last_visible: Mutex::new(false),
    }))
  }

  pub fn refresh_all(
    &self,
    rog: &RogKeyboardSnapshot,
    naga: &NagaSnapshot,
    hyper: &HyperCapslockSnapshot,
    is_zh: bool,
  ) {
    self.refresh_rog(rog, is_zh);
    self.refresh_naga(naga, is_zh);
    self.refresh_hyper(hyper, is_zh);
  }

  pub fn refresh_rog(&self, snapshot: &RogKeyboardSnapshot, is_zh: bool) {
    let profile_text = Self::rog_status_text(snapshot, is_zh);
    let battery_text = Self::rog_battery_text(snapshot, is_zh);

    if let Ok(items) = self.main_items.lock() {
      let _ = items.rog_status_item.set_text(&profile_text);
      let _ = items.rog_battery_item.set_text(&battery_text);
      let _ = items.rog_to_windows_item.set_text(Self::windows_action_text(is_zh));
      let _ = items.naga_refresh_item.set_text(Self::naga_refresh_text(is_zh));
      let _ = items.open_item.set_text(Self::open_text(is_zh));
      let _ = items.quit_item.set_text(Self::quit_text(is_zh));
    }

    if let Ok(items) = self.rog_battery_items.lock() {
      let _ = items.battery_item.set_text(&battery_text);
      let _ = items.profile_item.set_text(&profile_text);
      let _ = items.refresh_item.set_text(Self::refresh_battery_text(is_zh));
      let _ = items.to_windows_item.set_text(Self::windows_action_text(is_zh));
      let _ = items.open_item.set_text(Self::open_text(is_zh));
      let _ = items.quit_item.set_text(Self::quit_text(is_zh));
    }

    let should_show = snapshot.is_connected && snapshot.battery.is_some();
    if let (true, Some(bat)) = (should_show, &snapshot.battery) {
      let badge_key = (bat.percent, bat.charging_state);
      let needs_icon = {
        let mut last = self.rog_last_badge.lock().unwrap();
        if *last != Some(badge_key) {
          *last = Some(badge_key);
          true
        } else {
          false
        }
      };
      if needs_icon {
        let icon = BatteryIconRenderer::render(bat);
        let _ = self.rog_battery_tray.set_icon(Some(icon));
      }

      let tip = Self::rog_battery_tooltip(snapshot, bat, is_zh);
      let _ = self.rog_battery_tray.set_tooltip(Some(tip));
    }

    let vis_changed = {
      let mut vis = self.rog_last_visible.lock().unwrap();
      if *vis != should_show {
        *vis = should_show;
        true
      } else {
        false
      }
    };
    if vis_changed {
      let _ = self.rog_battery_tray.set_visible(should_show);
    }
  }

  pub fn refresh_naga(&self, snapshot: &NagaSnapshot, is_zh: bool) {
    let main_text = Self::naga_main_text(snapshot, is_zh);
    let bat_text = Self::naga_battery_text(snapshot, is_zh);
    let detail_text = Self::naga_detail_text(snapshot, is_zh);

    if let Ok(items) = self.main_items.lock() {
      let _ = items.naga_status_item.set_text(&main_text);
      let _ = items.naga_refresh_item.set_text(Self::naga_refresh_text(is_zh));
    }

    if let Ok(items) = self.naga_battery_items.lock() {
      let _ = items.battery_item.set_text(&bat_text);
      let _ = items.detail_item.set_text(&detail_text);
      let _ = items.refresh_item.set_text(Self::refresh_battery_text(is_zh));
      let _ = items.open_item.set_text(Self::open_text(is_zh));
      let _ = items.quit_item.set_text(Self::quit_text(is_zh));
    }

    let should_show = snapshot.is_connected && snapshot.battery.is_some();
    if let (true, Some(bat)) = (should_show, &snapshot.battery) {
      let badge_key = (bat.percent, bat.charging_state);
      let needs_icon = {
        let mut last = self.naga_last_badge.lock().unwrap();
        if *last != Some(badge_key) {
          *last = Some(badge_key);
          true
        } else {
          false
        }
      };
      if needs_icon {
        let icon = BatteryIconRenderer::render_naga(bat);
        let _ = self.naga_battery_tray.set_icon(Some(icon));
      }

      let tip = Self::naga_battery_tooltip(snapshot, bat, is_zh);
      let _ = self.naga_battery_tray.set_tooltip(Some(tip));
    }

    let vis_changed = {
      let mut vis = self.naga_last_visible.lock().unwrap();
      if *vis != should_show {
        *vis = should_show;
        true
      } else {
        false
      }
    };
    if vis_changed {
      let _ = self.naga_battery_tray.set_visible(should_show);
    }
  }

  pub fn refresh_hyper(&self, snapshot: &HyperCapslockSnapshot, is_zh: bool) {
    if let Ok(items) = self.main_items.lock() {
      let _ = items
        .hyper_toggle_item
        .set_text(Self::hyper_toggle_text(snapshot, is_zh));
    }
  }

  pub fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
      let _ = window.unminimize();
      let _ = window.show();
      let _ = window.set_focus();
    }
  }

  fn rog_status_text(snapshot: &RogKeyboardSnapshot, is_zh: bool) -> String {
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

  fn rog_battery_text(snapshot: &RogKeyboardSnapshot, is_zh: bool) -> String {
    if !snapshot.is_connected {
      return if is_zh {
        "ROG 电量 —— 未连接".to_string()
      } else {
        "ROG Battery — Not connected".to_string()
      };
    }
    let Some(bat) = &snapshot.battery else {
      return if is_zh {
        "ROG 电量 —— 读取中…".to_string()
      } else {
        "ROG Battery — Reading…".to_string()
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
        format!("ROG 电量 —— {}%", bat.percent)
      } else {
        format!("ROG Battery — {}%", bat.percent)
      }
    } else if is_zh {
      format!("ROG 电量 —— {}%（{}）", bat.percent, parts.join(" · "))
    } else {
      format!("ROG Battery — {}% ({})", bat.percent, parts.join(" · "))
    }
  }

  fn rog_battery_tooltip(
    snapshot: &RogKeyboardSnapshot,
    bat: &RogBatteryInfo,
    is_zh: bool,
  ) -> String {
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

  fn naga_main_text(snapshot: &NagaSnapshot, is_zh: bool) -> String {
    if !snapshot.is_connected {
      return if is_zh {
        "Naga 鼠标 —— 未连接".to_string()
      } else {
        "Naga Mouse — Not connected".to_string()
      };
    }
    let Some(bat) = &snapshot.battery else {
      return if is_zh {
        "Naga 鼠标 —— 已连接".to_string()
      } else {
        "Naga Mouse — Connected".to_string()
      };
    };
    let state = Self::naga_charge_label(bat.charging_state, is_zh);
    match state {
      Some(label) => {
        if is_zh {
          format!("Naga 鼠标 —— {}%（{}）", bat.percent, label)
        } else {
          format!("Naga Mouse — {}% ({})", bat.percent, label)
        }
      }
      None => {
        if is_zh {
          format!("Naga 鼠标 —— {}%", bat.percent)
        } else {
          format!("Naga Mouse — {}%", bat.percent)
        }
      }
    }
  }

  fn naga_battery_text(snapshot: &NagaSnapshot, is_zh: bool) -> String {
    if !snapshot.is_connected {
      return if is_zh {
        "鼠标电量 —— 未连接".to_string()
      } else {
        "Mouse Battery — Not connected".to_string()
      };
    }
    let Some(bat) = &snapshot.battery else {
      return if is_zh {
        "鼠标电量 —— 读取中…".to_string()
      } else {
        "Mouse Battery — Reading…".to_string()
      };
    };
    match Self::naga_charge_label(bat.charging_state, is_zh) {
      Some(label) => {
        if is_zh {
          format!("鼠标电量 —— {}%（{}）", bat.percent, label)
        } else {
          format!("Mouse Battery — {}% ({})", bat.percent, label)
        }
      }
      None => {
        if is_zh {
          format!("鼠标电量 —— {}%", bat.percent)
        } else {
          format!("Mouse Battery — {}%", bat.percent)
        }
      }
    }
  }

  fn naga_detail_text(snapshot: &NagaSnapshot, is_zh: bool) -> String {
    let link = match snapshot.link {
      Some(NagaLink::Wired) => {
        if is_zh { "USB 有线" } else { "USB Wired" }
      }
      Some(NagaLink::Wireless24g) => {
        if is_zh { "2.4G 无线" } else { "2.4G Wireless" }
      }
      None => {
        if is_zh { "未连接" } else { "Disconnected" }
      }
    };
    let mut parts = vec![link.to_string()];
    if let Some(prof) = &snapshot.profile_info {
      parts.push(if is_zh {
        format!("配置文件 {}", prof.active_profile)
      } else {
        format!("Profile {}", prof.active_profile)
      });
    }
    if let Some(dpi) = snapshot.dpi {
      parts.push(format!("{} DPI", dpi.x));
    }
    parts.join(" · ")
  }

  fn naga_battery_tooltip(
    snapshot: &NagaSnapshot,
    bat: &NagaBatteryInfo,
    is_zh: bool,
  ) -> String {
    let charge = Self::naga_charge_label(bat.charging_state, is_zh);
    let bat_part = match charge {
      Some(label) => format!("{}%（{}）", bat.percent, label),
      None => format!("{}%", bat.percent),
    };
    let mut parts = vec![NagaModel::DISPLAY_NAME.to_string(), bat_part];
    if let Some(prof) = &snapshot.profile_info {
      parts.push(if is_zh {
        format!("配置文件 {}", prof.active_profile)
      } else {
        format!("Profile {}", prof.active_profile)
      });
    }
    if let Some(dpi) = snapshot.dpi {
      parts.push(format!("{} DPI", dpi.x));
    }
    parts.join(" · ")
  }

  fn naga_charge_label(state: NagaChargingState, is_zh: bool) -> Option<&'static str> {
    match state {
      NagaChargingState::Charging => Some(if is_zh { "充电中" } else { "Charging" }),
      NagaChargingState::Full => Some(if is_zh { "已充满" } else { "Full" }),
      _ => None,
    }
  }

  fn hyper_toggle_text(snapshot: &HyperCapslockSnapshot, is_zh: bool) -> String {
    if snapshot.auto_paused {
      let processes = snapshot.matched_processes.join(", ");
      return if is_zh {
        format!("HyperCapslock —— 已自动暂停（{}）", processes)
      } else {
        format!("HyperCapslock — Auto-paused ({})", processes)
      };
    }
    match (snapshot.paused, is_zh) {
      (false, true) => "HyperCapslock —— 运行中（点击暂停）",
      (true, true) => "HyperCapslock —— 已暂停（点击恢复）",
      (false, false) => "HyperCapslock — Running (Pause)",
      (true, false) => "HyperCapslock — Paused (Resume)",
    }
    .to_string()
  }

  fn naga_refresh_text(is_zh: bool) -> &'static str {
    if is_zh {
      "刷新 Naga 电量"
    } else {
      "Refresh Naga Battery"
    }
  }

  fn refresh_battery_text(is_zh: bool) -> &'static str {
    if is_zh {
      "刷新状态与电量"
    } else {
      "Refresh Status & Battery"
    }
  }

  fn windows_action_text(is_zh: bool) -> &'static str {
    if is_zh {
      "切到 Windows 档"
    } else {
      "Switch to Windows Profile"
    }
  }

  fn open_text(is_zh: bool) -> &'static str {
    if is_zh {
      "打开 XTools…"
    } else {
      "Open XTools…"
    }
  }

  fn quit_text(is_zh: bool) -> &'static str {
    if is_zh {
      "退出 XTools"
    } else {
      "Quit XTools"
    }
  }
}
