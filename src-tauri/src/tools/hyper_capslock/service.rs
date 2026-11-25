use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::core::file_log::FileLog;
use crate::core::keyboard_hook::KeyboardHookHub;
use crate::core::notifier::WindowsNotifier;
use crate::core::preferences::PreferencesStore;
use crate::core::process_watcher::{ProcessEnumerator, RunningProcessInfo};
use crate::core::tool::XTool;

use super::config::{
  add_auto_pause_process, default_action_mappings, normalize_mappings, remove_auto_pause_process,
  remove_mapping, upsert_mapping, validate_action, ActionConfig, ActionMappingEntry, Trigger,
};
use super::engine::HyperCapslockEngine;

const LOG: FileLog = FileLog::new("HyperCapslock");
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Serialize)]
pub struct HyperCapslockSnapshot {
  /// Effective paused state (`manual_paused || auto_paused`).
  pub paused: bool,
  pub manual_paused: bool,
  pub auto_paused: bool,
  pub matched_processes: Vec<String>,
  pub auto_pause_enabled: bool,
  pub auto_pause_processes: Vec<String>,
  pub auto_pause_notify_sound: bool,
  pub mappings: Vec<ActionMappingEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoPauseNotification {
  Paused { processes: Vec<String> },
  Resumed { processes: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileOutcome {
  pub effective_paused: bool,
  pub state_changed: bool,
  pub notification: Option<AutoPauseNotification>,
}

/// Pure state machine deciding when watched processes should pause/resume HyperCapslock.
#[derive(Debug, Default)]
pub struct AutoPauseState {
  matched_processes: Vec<String>,
  auto_paused: bool,
  trigger_processes: Vec<String>,
  manual_override_active: bool,
}

impl AutoPauseState {
  pub fn is_auto_paused(&self) -> bool {
    self.auto_paused
  }

  pub fn matched_processes(&self) -> &[String] {
    &self.matched_processes
  }

  pub fn reconcile(
    &mut self,
    manual_paused: bool,
    auto_pause_enabled: bool,
    next_matched: Vec<String>,
  ) -> ReconcileOutcome {
    let prev_auto_paused = self.auto_paused;
    let prev_matched = self.matched_processes.clone();
    let mut notification = None;
    let effective_paused;

    if !auto_pause_enabled || next_matched.is_empty() {
      self.matched_processes.clear();
      self.manual_override_active = false;
      if self.auto_paused {
        self.auto_paused = false;
        let exited = std::mem::take(&mut self.trigger_processes);
        if !manual_paused {
          notification = Some(AutoPauseNotification::Resumed { processes: exited });
        }
      } else {
        self.trigger_processes.clear();
      }
      effective_paused = manual_paused;
    } else {
      self.matched_processes = next_matched.clone();
      if manual_paused {
        self.auto_paused = false;
        self.trigger_processes.clear();
        effective_paused = true;
      } else if self.manual_override_active {
        self.auto_paused = false;
        self.trigger_processes.clear();
        effective_paused = false;
      } else {
        if !self.auto_paused {
          self.auto_paused = true;
          self.trigger_processes = next_matched.clone();
          notification = Some(AutoPauseNotification::Paused {
            processes: next_matched,
          });
        } else {
          self.trigger_processes = next_matched;
        }
        effective_paused = true;
      }
    }

    ReconcileOutcome {
      effective_paused,
      state_changed: prev_auto_paused != self.auto_paused || prev_matched != self.matched_processes,
      notification,
    }
  }

  /// Records an explicit user pause/resume; returns the new effective paused state.
  pub fn on_manual_set_paused(&mut self, paused: bool) -> bool {
    self.auto_paused = false;
    self.trigger_processes.clear();
    if paused {
      self.manual_override_active = false;
      true
    } else {
      self.manual_override_active = !self.matched_processes.is_empty();
      false
    }
  }
}

enum WatcherSignal {
  CheckNow,
  Shutdown,
}

type SnapshotObserver = Arc<dyn Fn(&HyperCapslockSnapshot) + Send + Sync>;

pub struct HyperCapslockService {
  prefs: Arc<PreferencesStore>,
  hook: Arc<KeyboardHookHub>,
  notifier: Arc<WindowsNotifier>,
  engine: Arc<HyperCapslockEngine>,
  engine_paused: AtomicBool,
  auto_pause: Mutex<AutoPauseState>,
  observer: RwLock<Option<SnapshotObserver>>,
  watcher_tx: Mutex<Option<mpsc::Sender<WatcherSignal>>>,
  watcher_handle: Mutex<Option<JoinHandle<()>>>,
}

impl HyperCapslockService {
  pub fn new(
    prefs: Arc<PreferencesStore>,
    hook: Arc<KeyboardHookHub>,
    notifier: Arc<WindowsNotifier>,
  ) -> Arc<Self> {
    let cfg = prefs.get().hyper_capslock;
    let engine = HyperCapslockEngine::new(cfg.paused, cfg.mappings);
    Arc::new(Self {
      prefs,
      hook,
      notifier,
      engine,
      engine_paused: AtomicBool::new(cfg.paused),
      auto_pause: Mutex::new(AutoPauseState::default()),
      observer: RwLock::new(None),
      watcher_tx: Mutex::new(None),
      watcher_handle: Mutex::new(None),
    })
  }

  pub fn set_observer<F>(&self, obs: F)
  where
    F: Fn(&HyperCapslockSnapshot) + Send + Sync + 'static,
  {
    *self.observer.write().unwrap() = Some(Arc::new(obs));
  }

  pub fn snapshot(&self) -> HyperCapslockSnapshot {
    let cfg = self.prefs.get().hyper_capslock;
    let (auto_paused, matched_processes) = {
      let state = self.auto_pause.lock().unwrap();
      (state.is_auto_paused(), state.matched_processes().to_vec())
    };
    HyperCapslockSnapshot {
      paused: cfg.paused || auto_paused,
      manual_paused: cfg.paused,
      auto_paused,
      matched_processes,
      auto_pause_enabled: cfg.auto_pause_enabled,
      auto_pause_processes: cfg.auto_pause_processes,
      auto_pause_notify_sound: cfg.auto_pause_notify_sound,
      mappings: cfg.mappings,
    }
  }

  fn emit_snapshot(&self, snap: &HyperCapslockSnapshot) {
    if let Some(obs) = self.observer.read().unwrap().as_ref().cloned() {
      obs(snap);
    }
  }

  fn apply_engine_paused(&self, paused: bool) {
    if self.engine_paused.swap(paused, Ordering::SeqCst) != paused {
      self.engine.set_paused(paused);
      if paused {
        self.hook.stop();
      } else {
        self.hook.start();
      }
    }
  }

  fn wake_watcher(&self) {
    if let Some(tx) = self.watcher_tx.lock().unwrap().as_ref() {
      let _ = tx.send(WatcherSignal::CheckNow);
    }
  }

  /// Enumerates processes, reconciles auto-pause state, and applies engine/notification effects.
  fn check_processes_now(&self) {
    let cfg = self.prefs.get().hyper_capslock;
    let next_matched = if cfg.auto_pause_enabled && !cfg.auto_pause_processes.is_empty() {
      let running = ProcessEnumerator::running_process_names_lowercase();
      ProcessEnumerator::find_matched_processes(&cfg.auto_pause_processes, &running)
    } else {
      Vec::new()
    };

    let outcome = {
      let mut state = self.auto_pause.lock().unwrap();
      let fresh = self.prefs.get().hyper_capslock;
      state.reconcile(fresh.paused, fresh.auto_pause_enabled, next_matched)
    };

    if outcome.state_changed || outcome.notification.is_some() {
      self.apply_engine_paused(outcome.effective_paused);
      let snap = self.snapshot();
      self.emit_snapshot(&snap);
    }
    if let Some(notification) = outcome.notification {
      self.post_notification(notification);
    }
  }

  fn post_notification(&self, notification: AutoPauseNotification) {
    let is_zh = self.prefs.effective_is_zh();
    let sound = self.prefs.get().hyper_capslock.auto_pause_notify_sound;
    let (title, body) = Self::format_notification(&notification, is_zh);
    LOG.info(&format!("auto-pause: {}", body));
    let notifier = Arc::clone(&self.notifier);
    thread::spawn(move || {
      notifier.post(title, &body, sound);
    });
  }

  fn format_notification(notification: &AutoPauseNotification, is_zh: bool) -> (&'static str, String) {
    let title = "HyperCapslock";
    let body = match notification {
      AutoPauseNotification::Paused { processes } => {
        if is_zh {
          format!("检测到进程 {} 正在运行，已自动暂停", processes.join("、"))
        } else {
          format!("Auto-paused because {} is running", processes.join(", "))
        }
      }
      AutoPauseNotification::Resumed { processes } => {
        if processes.is_empty() {
          if is_zh {
            "冲突进程已结束，已自动恢复运行".to_string()
          } else {
            "Watched processes exited — auto-resumed".to_string()
          }
        } else if is_zh {
          format!("进程 {} 已结束，已自动恢复运行", processes.join("、"))
        } else {
          format!("{} exited — auto-resumed", processes.join(", "))
        }
      }
    };
    (title, body)
  }

  /// Re-evaluates processes, wakes the watcher thread, and emits + returns a fresh snapshot.
  fn finish_mutation(&self) -> HyperCapslockSnapshot {
    self.check_processes_now();
    self.wake_watcher();
    let snap = self.snapshot();
    self.emit_snapshot(&snap);
    snap
  }

  pub fn start(self: &Arc<Self>) {
    let engine = Arc::clone(&self.engine);
    self.hook.set_hyper_handler(move |ev| engine.handle_event(ev));

    // Evaluate processes first so the hook stays uninstalled if a watched process is already running.
    self.check_processes_now();
    if !self.engine_paused.load(Ordering::SeqCst) && !self.hook.is_running() {
      self.hook.start();
    }

    let mut handle_guard = self.watcher_handle.lock().unwrap();
    if handle_guard.is_none() {
      let (tx, rx) = mpsc::channel::<WatcherSignal>();
      *self.watcher_tx.lock().unwrap() = Some(tx);
      let svc = Arc::clone(self);
      let handle = thread::Builder::new()
        .name("hyper-process-watcher".to_string())
        .spawn(move || loop {
          let cfg = svc.prefs.get().hyper_capslock;
          let polling = cfg.auto_pause_enabled && !cfg.auto_pause_processes.is_empty();
          let signal = if polling {
            match rx.recv_timeout(PROCESS_POLL_INTERVAL) {
              Ok(signal) => Some(signal),
              Err(mpsc::RecvTimeoutError::Timeout) => Some(WatcherSignal::CheckNow),
              Err(mpsc::RecvTimeoutError::Disconnected) => None,
            }
          } else {
            rx.recv().ok()
          };
          match signal {
            Some(WatcherSignal::CheckNow) => svc.check_processes_now(),
            Some(WatcherSignal::Shutdown) | None => break,
          }
        })
        .expect("failed to spawn hyper-process-watcher thread");
      *handle_guard = Some(handle);
    }
    drop(handle_guard);
    LOG.info("HyperCapslock started");
  }

  pub fn stop(&self) {
    self.hook.clear_hyper_handler();
    self.hook.stop();
    if let Some(tx) = self.watcher_tx.lock().unwrap().take() {
      let _ = tx.send(WatcherSignal::Shutdown);
    }
    if let Some(handle) = self.watcher_handle.lock().unwrap().take() {
      let _ = handle.join();
    }
    LOG.info("HyperCapslock stopped");
  }

  pub fn set_paused(&self, paused: bool) -> Result<HyperCapslockSnapshot, String> {
    let effective = {
      let mut state = self.auto_pause.lock().unwrap();
      self.prefs.update(|cfg| {
        cfg.hyper_capslock.paused = paused;
      })?;
      state.on_manual_set_paused(paused)
    };
    self.apply_engine_paused(effective);
    LOG.info(if paused {
      "HyperCapslock paused"
    } else {
      "HyperCapslock resumed"
    });
    let snap = self.snapshot();
    self.emit_snapshot(&snap);
    Ok(snap)
  }

  pub fn set_auto_pause_enabled(&self, enabled: bool) -> Result<HyperCapslockSnapshot, String> {
    self.prefs.update(|cfg| {
      cfg.hyper_capslock.auto_pause_enabled = enabled;
    })?;
    Ok(self.finish_mutation())
  }

  pub fn set_auto_pause_notify_sound(
    &self,
    enabled: bool,
  ) -> Result<HyperCapslockSnapshot, String> {
    self.prefs.update(|cfg| {
      cfg.hyper_capslock.auto_pause_notify_sound = enabled;
    })?;
    Ok(self.finish_mutation())
  }

  pub fn add_auto_pause_process(&self, raw: String) -> Result<HyperCapslockSnapshot, String> {
    let mut result: Result<String, String> = Ok(String::new());
    self.prefs.update(|cfg| {
      result = add_auto_pause_process(&mut cfg.hyper_capslock.auto_pause_processes, &raw);
    })?;
    result?;
    Ok(self.finish_mutation())
  }

  pub fn remove_auto_pause_process(&self, raw: String) -> Result<HyperCapslockSnapshot, String> {
    self.prefs.update(|cfg| {
      remove_auto_pause_process(&mut cfg.hyper_capslock.auto_pause_processes, &raw);
    })?;
    Ok(self.finish_mutation())
  }

  pub fn list_running_processes() -> Vec<RunningProcessInfo> {
    ProcessEnumerator::list_selectable_processes()
  }

  pub fn upsert_mapping(
    &self,
    trigger: Trigger,
    action: ActionConfig,
  ) -> Result<HyperCapslockSnapshot, String> {
    validate_action(&action)?;
    let next = self.prefs.update(|cfg| {
      upsert_mapping(&mut cfg.hyper_capslock.mappings, ActionMappingEntry { trigger, action });
      normalize_mappings(&mut cfg.hyper_capslock.mappings);
    })?;
    self.engine.update_mappings(next.hyper_capslock.mappings);
    let snap = self.snapshot();
    self.emit_snapshot(&snap);
    Ok(snap)
  }

  pub fn remove_mapping(&self, trigger: Trigger) -> Result<HyperCapslockSnapshot, String> {
    let next = self.prefs.update(|cfg| {
      remove_mapping(&mut cfg.hyper_capslock.mappings, &trigger);
    })?;
    self.engine.update_mappings(next.hyper_capslock.mappings);
    let snap = self.snapshot();
    self.emit_snapshot(&snap);
    Ok(snap)
  }

  pub fn reset_defaults(&self) -> Result<HyperCapslockSnapshot, String> {
    let next = self.prefs.update(|cfg| {
      cfg.hyper_capslock.mappings = default_action_mappings();
    })?;
    self.engine.update_mappings(next.hyper_capslock.mappings);
    let snap = self.snapshot();
    self.emit_snapshot(&snap);
    Ok(snap)
  }

  pub fn import_mappings(
    &self,
    mut mappings: Vec<ActionMappingEntry>,
  ) -> Result<HyperCapslockSnapshot, String> {
    if mappings.is_empty() {
      return Err("Imported mappings list cannot be empty".to_string());
    }
    for entry in &mappings {
      validate_action(&entry.action)?;
    }
    normalize_mappings(&mut mappings);

    let next = self.prefs.update(|cfg| {
      cfg.hyper_capslock.mappings = mappings;
    })?;
    self.engine.update_mappings(next.hyper_capslock.mappings);
    let snap = self.snapshot();
    self.emit_snapshot(&snap);
    Ok(snap)
  }

  pub fn export_mappings_json(&self) -> Result<String, String> {
    let mappings = self.prefs.get().hyper_capslock.mappings;
    serde_json::to_string_pretty(&mappings)
      .map_err(|e| format!("Failed to serialize mappings: {}", e))
  }
}

pub struct HyperCapslockTool {
  service: Arc<HyperCapslockService>,
}

impl HyperCapslockTool {
  pub fn new(service: Arc<HyperCapslockService>) -> Self {
    Self { service }
  }
}

impl XTool for HyperCapslockTool {
  fn id(&self) -> &'static str {
    "hyper-capslock"
  }

  fn activate(&self) {
    self.service.start();
  }

  fn shutdown(&self) {
    self.service.stop();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
  }

  #[test]
  fn process_appears_then_exits_pauses_then_resumes_with_notifications() {
    let mut st = AutoPauseState::default();

    let idle = st.reconcile(false, true, vec![]);
    assert!(!idle.effective_paused);
    assert!(!idle.state_changed);
    assert_eq!(idle.notification, None);

    let paused = st.reconcile(false, true, names(&["steam.exe"]));
    assert!(paused.effective_paused);
    assert!(paused.state_changed);
    assert_eq!(
      paused.notification,
      Some(AutoPauseNotification::Paused {
        processes: names(&["steam.exe"])
      })
    );

    let steady = st.reconcile(false, true, names(&["steam.exe"]));
    assert!(steady.effective_paused);
    assert!(!steady.state_changed);
    assert_eq!(steady.notification, None);

    let resumed = st.reconcile(false, true, vec![]);
    assert!(!resumed.effective_paused);
    assert!(resumed.state_changed);
    assert_eq!(
      resumed.notification,
      Some(AutoPauseNotification::Resumed {
        processes: names(&["steam.exe"])
      })
    );
  }

  #[test]
  fn overlapping_processes_emit_single_pause_and_single_resume() {
    let mut st = AutoPauseState::default();
    let mut notifications = Vec::new();

    for matched in [
      names(&["steam.exe"]),
      names(&["steam.exe", "cs2.exe"]),
      names(&["steam.exe"]),
      vec![],
    ] {
      let outcome = st.reconcile(false, true, matched);
      if let Some(n) = outcome.notification {
        notifications.push(n);
      }
    }

    assert_eq!(
      notifications,
      vec![
        AutoPauseNotification::Paused {
          processes: names(&["steam.exe"])
        },
        AutoPauseNotification::Resumed {
          processes: names(&["steam.exe"])
        },
      ]
    );
  }

  #[test]
  fn manual_pause_is_respected_and_never_auto_resumed() {
    let mut st = AutoPauseState::default();

    let started = st.reconcile(true, true, names(&["cs2.exe"]));
    assert!(started.effective_paused);
    assert_eq!(started.notification, None);
    assert!(!st.is_auto_paused());

    let exited = st.reconcile(true, true, vec![]);
    assert!(exited.effective_paused);
    assert_eq!(exited.notification, None);
  }

  #[test]
  fn manual_resume_overrides_until_process_exits_then_auto_pause_works_again() {
    let mut st = AutoPauseState::default();
    st.reconcile(false, true, names(&["cs2.exe"]));
    assert!(st.is_auto_paused());

    assert!(!st.on_manual_set_paused(false));
    let still_running = st.reconcile(false, true, names(&["cs2.exe"]));
    assert!(!still_running.effective_paused);
    assert_eq!(still_running.notification, None);
    assert!(!st.is_auto_paused());

    let exited = st.reconcile(false, true, vec![]);
    assert!(!exited.effective_paused);
    assert_eq!(exited.notification, None);

    let again = st.reconcile(false, true, names(&["cs2.exe"]));
    assert!(again.effective_paused);
    assert_eq!(
      again.notification,
      Some(AutoPauseNotification::Paused {
        processes: names(&["cs2.exe"])
      })
    );
  }

  #[test]
  fn disabling_auto_pause_while_auto_paused_resumes_immediately() {
    let mut st = AutoPauseState::default();
    st.reconcile(false, true, names(&["cs2.exe"]));

    let outcome = st.reconcile(false, false, names(&["cs2.exe"]));
    assert!(!outcome.effective_paused);
    assert!(outcome.state_changed);
    assert_eq!(
      outcome.notification,
      Some(AutoPauseNotification::Resumed {
        processes: names(&["cs2.exe"])
      })
    );
    assert!(st.matched_processes().is_empty());
  }

  #[test]
  fn notification_text_is_localized() {
    let paused = AutoPauseNotification::Paused {
      processes: names(&["steam.exe", "cs2.exe"]),
    };
    let (_, zh) = HyperCapslockService::format_notification(&paused, true);
    assert_eq!(zh, "检测到进程 steam.exe、cs2.exe 正在运行，已自动暂停");
    let (_, en) = HyperCapslockService::format_notification(&paused, false);
    assert_eq!(en, "Auto-paused because steam.exe, cs2.exe is running");
  }
}
