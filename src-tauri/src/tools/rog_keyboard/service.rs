use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::hid_device::{RogError, RogHidDevice};
use super::monitor::{MonitorEvent, RogKeyboardMonitor};
use super::protocol::{RogBatteryInfo, RogLink, RogModel, RogProfile, RogProfileNaming};
use crate::core::file_log::FileLog;
use crate::core::notifier::WindowsNotifier;
use crate::core::preferences::PreferencesStore;
use crate::core::tool::XTool;

const LOG: FileLog = FileLog::new("ROGKeyboard");
const BATTERY_POLL_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchReason {
  AutoReturn,
  StartupAuto,
  Windows,
}

impl SwitchReason {
  fn is_automatic(self) -> bool {
    matches!(self, Self::AutoReturn | Self::StartupAuto)
  }

  fn role_label(self, is_zh: bool) -> &'static str {
    let _ = self;
    if is_zh { "Windows 档" } else { "Windows Profile" }
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct RogKeyboardSnapshot {
  pub is_connected: bool,
  pub model_name: String,
  pub link: Option<RogLink>,
  pub firmware_version: Option<String>,
  pub profile_count: usize,
  pub current_profile: Option<usize>,
  pub battery: Option<RogBatteryInfo>,
  pub is_busy: bool,
  pub status_message: Option<String>,
  pub error_message: Option<String>,
  pub notifications_authorized: bool,
  pub windows_profile: usize,
  pub auto_switch_enabled: bool,
  pub sound_enabled: bool,
}

impl RogKeyboardSnapshot {
  pub fn from_prefs(prefs: &PreferencesStore) -> Self {
    let cfg = prefs.get().rog_keyboard;
    Self {
      is_connected: false,
      model_name: RogModel::DISPLAY_NAME.to_string(),
      link: None,
      firmware_version: None,
      profile_count: 6,
      current_profile: None,
      battery: None,
      is_busy: false,
      status_message: None,
      error_message: None,
      notifications_authorized: true,
      windows_profile: cfg.windows_profile,
      auto_switch_enabled: cfg.auto_switch_enabled,
      sound_enabled: cfg.sound_enabled,
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RogSettingsPatch {
  pub windows_profile: Option<usize>,
  pub auto_switch_enabled: Option<bool>,
  pub sound_enabled: Option<bool>,
}

enum WorkItem {
  StartupCheck,
  Refresh,
  BatteryPoll,
  ApplyProfile {
    slot: usize,
    reason: SwitchReason,
  },
  Detached,
  Shutdown,
}

type StateObserver = Arc<dyn Fn(&RogKeyboardSnapshot) + Send + Sync>;

/// Coordinates the ROG Falcata HID connection, background arrival/reconnect monitor,
/// periodic battery telemetry, preferences, and Windows Toast announcements.
pub struct RogKeyboardService {
  prefs: Arc<PreferencesStore>,
  notifier: Arc<WindowsNotifier>,
  monitor: Arc<RogKeyboardMonitor>,
  hid_op_mutex: Arc<Mutex<()>>,
  snapshot: Arc<RwLock<RogKeyboardSnapshot>>,
  observer: Arc<RwLock<Option<StateObserver>>>,
  worker_tx: Mutex<Option<mpsc::Sender<WorkItem>>>,
  worker_handle: Mutex<Option<JoinHandle<()>>>,
  battery_stop_tx: Mutex<Option<mpsc::Sender<()>>>,
  battery_handle: Mutex<Option<JoinHandle<()>>>,
}

impl RogKeyboardService {
  pub fn new(prefs: Arc<PreferencesStore>, notifier: Arc<WindowsNotifier>) -> Arc<Self> {
    let initial = RogKeyboardSnapshot::from_prefs(&prefs);
    Arc::new(Self {
      prefs,
      notifier,
      monitor: Arc::new(RogKeyboardMonitor::new()),
      hid_op_mutex: Arc::new(Mutex::new(())),
      snapshot: Arc::new(RwLock::new(initial)),
      observer: Arc::new(RwLock::new(None)),
      worker_tx: Mutex::new(None),
      worker_handle: Mutex::new(None),
      battery_stop_tx: Mutex::new(None),
      battery_handle: Mutex::new(None),
    })
  }

  pub fn set_observer<F>(&self, cb: F)
  where
    F: Fn(&RogKeyboardSnapshot) + Send + Sync + 'static,
  {
    *self.observer.write().unwrap() = Some(Arc::new(cb));
  }

  pub fn snapshot(&self) -> RogKeyboardSnapshot {
    self.snapshot.read().unwrap().clone()
  }

  pub fn refresh(&self) {
    self.enqueue(WorkItem::Refresh);
  }

  pub fn switch_to_windows(&self) {
    let slot = self.prefs.get().rog_keyboard.windows_profile;
    self.enqueue(WorkItem::ApplyProfile {
      slot,
      reason: SwitchReason::Windows,
    });
  }

  pub fn update_settings(&self, patch: RogSettingsPatch) -> Result<RogKeyboardSnapshot, String> {
    let updated_cfg = self.prefs.update(|cfg| {
      if let Some(w) = patch.windows_profile {
        cfg.rog_keyboard.windows_profile = w.max(1);
      }
      if let Some(auto) = patch.auto_switch_enabled {
        cfg.rog_keyboard.auto_switch_enabled = auto;
      }
      if let Some(sound) = patch.sound_enabled {
        cfg.rog_keyboard.sound_enabled = sound;
      }
    })?;

    let snap = self.mutate_snapshot(|s| {
      s.windows_profile = updated_cfg.rog_keyboard.windows_profile;
      s.auto_switch_enabled = updated_cfg.rog_keyboard.auto_switch_enabled;
      s.sound_enabled = updated_cfg.rog_keyboard.sound_enabled;
    });
    Ok(snap)
  }

  pub fn notify_language_changed(&self) {
    let snap = self.snapshot();
    self.emit_snapshot(&snap);
  }

  fn enqueue(&self, item: WorkItem) {
    if let Some(tx) = self.worker_tx.lock().unwrap().as_ref() {
      let _ = tx.send(item);
    }
  }

  fn mutate_snapshot<F>(&self, mutator: F) -> RogKeyboardSnapshot
  where
    F: FnOnce(&mut RogKeyboardSnapshot),
  {
    let snap = {
      let mut guard = self.snapshot.write().unwrap();
      mutator(&mut guard);
      guard.clone()
    };
    self.emit_snapshot(&snap);
    snap
  }

  fn emit_snapshot(&self, snap: &RogKeyboardSnapshot) {
    if let Some(cb) = self.observer.read().unwrap().as_ref() {
      cb(snap);
    }
  }

  fn start_worker(self: &Arc<Self>) {
    let mut handle_guard = self.worker_handle.lock().unwrap();
    if handle_guard.is_some() {
      return;
    }

    let (tx, rx) = mpsc::channel::<WorkItem>();
    *self.worker_tx.lock().unwrap() = Some(tx);

    let svc = Arc::clone(self);
    let handle = thread::Builder::new()
      .name("rog-hid-worker".to_string())
      .spawn(move || {
        while let Ok(item) = rx.recv() {
          match item {
            WorkItem::Shutdown => break,
            WorkItem::StartupCheck => svc.handle_startup_check(),
            WorkItem::Refresh => svc.handle_refresh(),
            WorkItem::BatteryPoll => svc.handle_battery_poll(),
            WorkItem::ApplyProfile { slot, reason } => svc.handle_apply_profile(slot, reason),
            WorkItem::Detached => svc.handle_detached(),
          }
        }
      })
      .expect("failed to spawn rog-hid-worker thread");
    *handle_guard = Some(handle);
  }

  fn start_battery_timer(self: &Arc<Self>) {
    let mut handle_guard = self.battery_handle.lock().unwrap();
    if handle_guard.is_some() {
      return;
    }

    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    *self.battery_stop_tx.lock().unwrap() = Some(stop_tx);

    let svc = Arc::clone(self);
    let handle = thread::Builder::new()
      .name("rog-battery-poll".to_string())
      .spawn(move || {
        loop {
          match stop_rx.recv_timeout(BATTERY_POLL_INTERVAL) {
            Err(mpsc::RecvTimeoutError::Timeout) => {
              if svc.snapshot().is_connected {
                svc.enqueue(WorkItem::BatteryPoll);
              }
            }
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
          }
        }
      })
      .expect("failed to spawn rog-battery-poll thread");
    *handle_guard = Some(handle);
  }

  fn handle_startup_check(&self) {
    let cfg = self.prefs.get().rog_keyboard;
    if cfg.auto_switch_enabled {
      LOG.info("startup check — inspecting connected ROG Falcata");
      self.run_apply_profile(cfg.windows_profile, SwitchReason::StartupAuto, true);
    } else {
      self.handle_refresh();
    }
  }

  fn handle_refresh(&self) {
    let is_zh = self.prefs.effective_is_zh();
    let authorized = self.notifier.is_authorized();
    self.mutate_snapshot(|s| {
      s.is_busy = true;
      s.notifications_authorized = authorized;
    });

    let _hid_guard = self.hid_op_mutex.lock().unwrap();
    let result = (|| -> Result<_, RogError> {
      let dev = RogHidDevice::open_first(&RogModel::ALL_PRODUCT_IDS)?;
      let (link, info) = dev.read_state(Duration::from_secs(2))?;
      let battery = dev.read_battery(Duration::from_secs(1)).ok();
      Ok((link, info, battery))
    })();
    drop(_hid_guard);

    match result {
      Ok((link, info, battery)) => {
        self.mutate_snapshot(|s| {
          s.is_busy = false;
          s.is_connected = true;
          s.link = Some(link);
          s.firmware_version = Some(info.firmware_version);
          s.profile_count = info.profile_count.max(1);
          s.current_profile = Some(info.current_profile);
          s.battery = battery;
          s.error_message = None;
        });
      }
      Err(RogError::NotFound) => {
        self.mutate_snapshot(|s| {
          s.is_busy = false;
          s.is_connected = false;
          s.link = None;
          s.current_profile = None;
          s.battery = None;
          s.error_message = None;
        });
      }
      Err(err) => {
        let msg = err.message(is_zh);
        LOG.error(&msg);
        self.mutate_snapshot(|s| {
          s.is_busy = false;
          s.error_message = Some(msg);
        });
      }
    }
  }

  fn handle_battery_poll(&self) {
    let Ok(_hid_guard) = self.hid_op_mutex.try_lock() else {
      return;
    };
    let result = (|| -> Result<_, RogError> {
      let dev = RogHidDevice::open_first(&RogModel::ALL_PRODUCT_IDS)?;
      dev.read_battery(Duration::from_secs(1))
    })();
    drop(_hid_guard);

    if let Ok(bat) = result {
      let unchanged = self
        .snapshot
        .read()
        .unwrap()
        .battery
        .as_ref()
        == Some(&bat);
      if !unchanged {
        self.mutate_snapshot(|s| {
          s.battery = Some(bat);
        });
      }
    }
  }

  fn handle_apply_profile(&self, slot: usize, reason: SwitchReason) {
    self.run_apply_profile(slot, reason, false);
  }

  fn run_apply_profile(&self, slot: usize, reason: SwitchReason, silent_if_missing_or_same: bool) {
    let is_zh = self.prefs.effective_is_zh();
    let cfg = self.prefs.get().rog_keyboard;
    let authorized = self.notifier.is_authorized();

    self.mutate_snapshot(|s| {
      s.is_busy = true;
      s.notifications_authorized = authorized;
      if !silent_if_missing_or_same {
        s.status_message = None;
      }
    });

    let _hid_guard = self.hid_op_mutex.lock().unwrap();
    let outcome = (|| -> Result<_, RogError> {
      let dev = RogHidDevice::open_first(&RogModel::ALL_PRODUCT_IDS)?;
      let (link, info) = dev.read_state(Duration::from_secs(2))?;
      let count = info.profile_count.max(1);
      if !RogProfile::is_valid(slot, count) {
        return Err(RogError::InvalidProfile(slot));
      }
      if info.current_profile == slot {
        let battery = dev.read_battery(Duration::from_secs(1)).ok();
        return Ok((link, info, None, battery));
      }
      let after = dev.change_profile(slot, link, count)?;
      let battery = dev.read_battery(Duration::from_secs(1)).ok();
      Ok((link, info, Some(after), battery))
    })();
    drop(_hid_guard);

    match outcome {
      Ok((link, before, None, battery)) => {
        let count = before.profile_count.max(1);
        let wanted = RogProfileNaming::label(slot, count, is_zh);
        let role = reason.role_label(is_zh);
        LOG.info(&format!("{:?} — already on {}, nothing to change", reason, wanted));

        if !silent_if_missing_or_same {
          let notify_body = Self::format_unchanged_notify(reason, role, &wanted, is_zh);
          self.notifier.post(Self::notify_title(is_zh), &notify_body, cfg.sound_enabled);
        }

        let status = Self::format_already_on_status(role, &wanted, is_zh);
        self.mutate_snapshot(|s| {
          s.is_busy = false;
          s.is_connected = true;
          s.link = Some(link);
          s.firmware_version = Some(before.firmware_version);
          s.profile_count = count;
          s.current_profile = Some(before.current_profile);
          s.battery = battery;
          s.status_message = Some(status);
          s.error_message = None;
        });
      }
      Ok((link, before, Some(after), battery)) => {
        let count = after.profile_count.max(1);
        let wanted = RogProfileNaming::label(slot, count, is_zh);
        let actual = RogProfileNaming::label(after.current_profile, count, is_zh);
        let landed = after.current_profile == slot;
        let role = reason.role_label(is_zh);

        LOG.info(&format!(
          "{:?} — {} -> {}{}",
          reason,
          RogProfileNaming::label(before.current_profile, count, is_zh),
          actual,
          if landed { String::new() } else { format!(" (asked for {})", wanted) }
        ));

        let notify_body = if landed {
          Self::format_switched_notify(reason, role, &wanted, is_zh)
        } else {
          Self::format_failed_notify(&wanted, &actual, is_zh)
        };
        self.notifier.post(Self::notify_title(is_zh), &notify_body, cfg.sound_enabled);

        let status = if landed {
          Some(Self::format_switched_status(role, &wanted, is_zh))
        } else {
          None
        };
        let error = if landed {
          None
        } else {
          Some(Self::format_failed_notify(&wanted, &actual, is_zh))
        };

        self.mutate_snapshot(|s| {
          s.is_busy = false;
          s.is_connected = true;
          s.link = Some(link);
          s.firmware_version = Some(after.firmware_version);
          s.profile_count = count;
          s.current_profile = Some(after.current_profile);
          s.battery = battery;
          s.status_message = status;
          s.error_message = error;
        });
      }
      Err(RogError::NotFound) => {
        self.mutate_snapshot(|s| {
          s.is_busy = false;
          s.is_connected = false;
          s.link = None;
          s.current_profile = None;
          s.battery = None;
          s.error_message = if silent_if_missing_or_same {
            None
          } else {
            Some(RogError::NotFound.message(is_zh))
          };
        });
      }
      Err(err) => {
        let msg = err.message(is_zh);
        LOG.error(&msg);
        self.mutate_snapshot(|s| {
          s.is_busy = false;
          s.error_message = Some(msg);
        });
      }
    }
  }

  fn handle_detached(&self) {
    let is_zh = self.prefs.effective_is_zh();
    let msg = if is_zh {
      "键盘已离开这台 Windows 电脑。".to_string()
    } else {
      "Keyboard left this PC.".to_string()
    };
    self.mutate_snapshot(|s| {
      s.is_connected = false;
      s.link = None;
      s.current_profile = None;
      s.battery = None;
      s.status_message = Some(msg);
      s.error_message = None;
    });
  }

  fn notify_title(is_zh: bool) -> &'static str {
    if is_zh { "ROG 键盘" } else { "ROG Keyboard" }
  }

  fn format_switched_notify(reason: SwitchReason, role: &str, wanted: &str, is_zh: bool) -> String {
    if reason.is_automatic() {
      if is_zh {
        format!("键盘连上 Windows —— 已切到{}（{}）", role, wanted)
      } else {
        format!("Keyboard connected to Windows — switched to {} ({})", role, wanted)
      }
    } else if is_zh {
      format!("已切到{}（{}）", role, wanted)
    } else {
      format!("Switched to {} ({})", role, wanted)
    }
  }

  fn format_unchanged_notify(reason: SwitchReason, role: &str, wanted: &str, is_zh: bool) -> String {
    if reason.is_automatic() {
      if is_zh {
        format!("键盘连上 Windows —— 本来就是{}（{}）", role, wanted)
      } else {
        format!("Keyboard connected to Windows — already on {} ({})", role, wanted)
      }
    } else if is_zh {
      format!("本来就是{}（{}），没有改动", role, wanted)
    } else {
      format!("Already on {} ({}), nothing changed", role, wanted)
    }
  }

  fn format_failed_notify(wanted: &str, actual: &str, is_zh: bool) -> String {
    if is_zh {
      format!("切换没生效：请求「{}」，键盘停在「{}」。", wanted, actual)
    } else {
      format!("Profile switch did not stick: requested {}, keyboard stayed on {}.", wanted, actual)
    }
  }

  fn format_switched_status(role: &str, wanted: &str, is_zh: bool) -> String {
    if is_zh {
      format!("已切到{}（{}）。", role, wanted)
    } else {
      format!("Switched to {} ({}).", role, wanted)
    }
  }

  fn format_already_on_status(role: &str, wanted: &str, is_zh: bool) -> String {
    if is_zh {
      format!("本来就是{}（{}），没有改动。", role, wanted)
    } else {
      format!("Already on {} ({}), nothing changed.", role, wanted)
    }
  }
}

pub struct RogKeyboardTool {
  service: Arc<RogKeyboardService>,
}

impl RogKeyboardTool {
  pub fn new(service: Arc<RogKeyboardService>) -> Self {
    Self { service }
  }
}

impl XTool for RogKeyboardTool {
  fn id(&self) -> &'static str {
    "rog-keyboard"
  }

  fn activate(&self) {
    self.service.start_worker();
    self.service.start_battery_timer();
    let svc = Arc::clone(&self.service);
    self.service.monitor.start(Arc::clone(&self.service.hid_op_mutex), move |event| {
      match event {
        MonitorEvent::Arrived => {
          let cfg = svc.prefs.get().rog_keyboard;
          if cfg.auto_switch_enabled {
            LOG.info("keyboard connected to Windows — applying Windows profile");
            svc.enqueue(WorkItem::ApplyProfile {
              slot: cfg.windows_profile,
              reason: SwitchReason::AutoReturn,
            });
          } else {
            LOG.info("keyboard connected but auto-switch is off — refreshing only");
            svc.enqueue(WorkItem::Refresh);
          }
        }
        MonitorEvent::Detached => {
          svc.enqueue(WorkItem::Detached);
        }
      }
    });
    self.service.enqueue(WorkItem::StartupCheck);
  }

  fn shutdown(&self) {
    if let Some(tx) = self.service.battery_stop_tx.lock().unwrap().take() {
      let _ = tx.send(());
    }
    if let Some(handle) = self.service.battery_handle.lock().unwrap().take() {
      let _ = handle.join();
    }
    self.service.monitor.stop();
    if let Some(tx) = self.service.worker_tx.lock().unwrap().take() {
      let _ = tx.send(WorkItem::Shutdown);
    }
    if let Some(handle) = self.service.worker_handle.lock().unwrap().take() {
      let _ = handle.join();
    }
  }
}
