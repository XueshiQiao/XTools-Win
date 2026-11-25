use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::core::file_log::FileLog;
use crate::core::keyboard_hook::{HookDecision, KeyboardHookHub, LowLevelKeyEvent, RecordRequest};
use crate::core::preferences::PreferencesStore;
use crate::core::tool::XTool;
use crate::core::vk::{KeyShortcut, KeySynthesizer, VkHelper};

use super::config::{NagaButtonMapping, NagaPlateLayout};
use super::hid_device::{NagaError, NagaHidDevice};
use super::protocol::{NagaBatteryInfo, NagaDpi, NagaLink, NagaModel};
use super::raw_input::{NagaCapturedEvent, NagaRawInputMonitor, RawKeyEvent};

const LOG: FileLog = FileLog::new("RazerNaga");
const MAX_CAPTURES: usize = 30;
const BATTERY_POLL_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NagaRecordMode {
  Target,
  Sentinel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NagaRecordingState {
  pub button_index: u8,
  pub mode: NagaRecordMode,
}

#[derive(Debug, Clone, Serialize)]
pub struct NagaSnapshot {
  pub is_connected: bool,
  pub model_name: String,
  pub link: Option<NagaLink>,
  pub firmware_version: Option<String>,
  pub battery: Option<NagaBatteryInfo>,
  pub dpi: Option<NagaDpi>,
  pub is_busy: bool,
  pub error_message: Option<String>,
  pub enabled: bool,
  pub plate_layout: NagaPlateLayout,
  pub mappings: Vec<NagaButtonMapping>,
  pub recording: Option<NagaRecordingState>,
  pub captures: Vec<NagaCapturedEvent>,
}

struct RuntimeData {
  is_connected: bool,
  link: Option<NagaLink>,
  firmware_version: Option<String>,
  battery: Option<NagaBatteryInfo>,
  dpi: Option<NagaDpi>,
  is_busy: bool,
  error_message: Option<String>,
  recording: Option<NagaRecordingState>,
  captures: Vec<NagaCapturedEvent>,
  next_capture_id: u64,
}

impl Default for RuntimeData {
  fn default() -> Self {
    Self {
      is_connected: false,
      link: None,
      firmware_version: None,
      battery: None,
      dpi: None,
      is_busy: false,
      error_message: None,
      recording: None,
      captures: Vec::new(),
      next_capture_id: 1,
    }
  }
}

enum PollCommand {
  RefreshNow,
  UsbChanged,
  Shutdown,
}

type SnapshotObserver = Arc<dyn Fn(&NagaSnapshot) + Send + Sync>;

pub struct NagaService {
  prefs: Arc<PreferencesStore>,
  hook: Arc<KeyboardHookHub>,
  raw_monitor: NagaRawInputMonitor,
  hid_mutex: Arc<Mutex<()>>,
  runtime: RwLock<RuntimeData>,
  pressed_sentinels: Mutex<HashSet<u16>>,
  poll_stop: AtomicBool,
  poll_tx: Mutex<Option<mpsc::Sender<PollCommand>>>,
  poll_thread: Mutex<Option<JoinHandle<()>>>,
  record_gen: AtomicU64,
  observer: RwLock<Option<SnapshotObserver>>,
}

impl NagaService {
  pub fn new(prefs: Arc<PreferencesStore>, hook: Arc<KeyboardHookHub>) -> Arc<Self> {
    Arc::new(Self {
      prefs,
      hook,
      raw_monitor: NagaRawInputMonitor::new(),
      hid_mutex: Arc::new(Mutex::new(())),
      runtime: RwLock::new(RuntimeData::default()),
      pressed_sentinels: Mutex::new(HashSet::new()),
      poll_stop: AtomicBool::new(false),
      poll_tx: Mutex::new(None),
      poll_thread: Mutex::new(None),
      record_gen: AtomicU64::new(0),
      observer: RwLock::new(None),
    })
  }

  pub fn set_observer<F>(&self, obs: F)
  where
    F: Fn(&NagaSnapshot) + Send + Sync + 'static,
  {
    *self.observer.write().unwrap() = Some(Arc::new(obs));
  }

  pub fn snapshot(&self) -> NagaSnapshot {
    let cfg = self.prefs.get().razer_naga;
    let rt = self.runtime.read().unwrap();
    NagaSnapshot {
      is_connected: rt.is_connected,
      model_name: NagaModel::DISPLAY_NAME.to_string(),
      link: rt.link,
      firmware_version: rt.firmware_version.clone(),
      battery: rt.battery.clone(),
      dpi: rt.dpi,
      is_busy: rt.is_busy,
      error_message: rt.error_message.clone(),
      enabled: cfg.enabled,
      plate_layout: cfg.plate_layout,
      mappings: cfg.mappings,
      recording: rt.recording.clone(),
      captures: rt.captures.clone(),
    }
  }

  fn emit_snapshot(&self) {
    let snap = self.snapshot();
    if let Some(obs) = self.observer.read().unwrap().as_ref().cloned() {
      obs(&snap);
    }
  }

  pub fn start(self: &Arc<Self>) {
    let hook_self = Arc::clone(self);
    self.hook.set_naga_handler(move |ev| hook_self.handle_hook_event(ev));
    self.hook.start();

    let (tx, rx) = mpsc::channel::<PollCommand>();
    *self.poll_tx.lock().unwrap() = Some(tx.clone());
    self.poll_stop.store(false, Ordering::SeqCst);

    let poll_self = Arc::clone(self);
    let poll_handle = thread::Builder::new()
      .name("naga-poll".to_string())
      .spawn(move || {
        poll_self.poll_loop(rx);
      })
      .expect("failed to spawn naga-poll thread");
    *self.poll_thread.lock().unwrap() = Some(poll_handle);

    let usb_tx = tx.clone();
    let raw_self = Arc::clone(self);
    self.raw_monitor.start(
      move || {
        let _ = usb_tx.send(PollCommand::UsbChanged);
      },
      move |ev| {
        raw_self.record_raw_capture(ev);
      },
    );

    let _ = tx.send(PollCommand::RefreshNow);
    LOG.info("Razer Naga V3 Pro service started");
  }

  pub fn stop(&self) {
    self.cancel_recording_internal(false);
    self.raw_monitor.stop();
    self.hook.clear_naga_handler();
    self.poll_stop.store(true, Ordering::SeqCst);
    if let Some(tx) = self.poll_tx.lock().unwrap().take() {
      let _ = tx.send(PollCommand::Shutdown);
    }
    if let Some(handle) = self.poll_thread.lock().unwrap().take() {
      let _ = handle.join();
    }
    LOG.info("Razer Naga V3 Pro service stopped");
  }

  pub fn refresh(&self) {
    if let Some(tx) = self.poll_tx.lock().unwrap().as_ref() {
      let _ = tx.send(PollCommand::RefreshNow);
    }
  }

  fn poll_loop(&self, rx: mpsc::Receiver<PollCommand>) {
    while !self.poll_stop.load(Ordering::SeqCst) {
      match rx.recv_timeout(Duration::from_secs(BATTERY_POLL_SECS)) {
        Ok(PollCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
        Ok(PollCommand::UsbChanged) => {
          thread::sleep(Duration::from_millis(600));
          while rx.try_recv().is_ok() {}
          self.poll_hardware_once();
        }
        Ok(PollCommand::RefreshNow) | Err(mpsc::RecvTimeoutError::Timeout) => {
          self.poll_hardware_once();
        }
      }
    }
  }

  fn poll_hardware_once(&self) {
    let Ok(_hid_guard) = self.hid_mutex.try_lock() else {
      return;
    };
    {
      let mut rt = self.runtime.write().unwrap();
      rt.is_busy = true;
    }
    self.emit_snapshot();

    let result = NagaHidDevice::read_active_state();
    let is_zh = self.prefs.effective_is_zh();

    {
      let mut rt = self.runtime.write().unwrap();
      rt.is_busy = false;
      match result {
        Ok(state) => {
          rt.is_connected = true;
          rt.link = Some(state.link);
          rt.firmware_version = state.firmware_version;
          rt.battery = state.battery;
          rt.dpi = state.dpi;
          rt.error_message = None;
        }
        Err(NagaError::NotFound) | Err(NagaError::Offline) => {
          rt.is_connected = false;
          rt.link = None;
          rt.battery = None;
          rt.dpi = None;
          rt.error_message = None;
        }
        Err(err) => {
          rt.error_message = Some(err.message(is_zh));
        }
      }
    }

    self.emit_snapshot();
  }

  fn handle_hook_event(&self, ev: &LowLevelKeyEvent) -> HookDecision {
    let cfg = self.prefs.get().razer_naga;
    if !cfg.enabled {
      return HookDecision::Pass;
    }

    if ev.is_up {
      let was_swallowed = self.pressed_sentinels.lock().unwrap().remove(&ev.vk);
      return if was_swallowed {
        HookDecision::Swallow
      } else {
        HookDecision::Pass
      };
    }

    if !ev.is_down {
      return HookDecision::Pass;
    }

    let Some(mapping) = cfg.mappings.iter().find(|m| m.sentinel_vk == ev.vk) else {
      return HookDecision::Pass;
    };

    if !mapping.enabled {
      return HookDecision::Pass;
    }

    // Intercept configured F13-F24 sentinels (even unbound) or any key with an active target.
    if mapping.target.is_none() && !VkHelper::is_sentinel_fn_vk(ev.vk) {
      return HookDecision::Pass;
    }

    let is_first_press = self.pressed_sentinels.lock().unwrap().insert(ev.vk);
    if !is_first_press {
      // Swallow autorepeat without re-emitting target shortcut repeatedly.
      return HookDecision::Swallow;
    }

    if let Some(target) = &mapping.target {
      KeySynthesizer::send_shortcut(target);
      let remapped = target.display();
      self.append_capture(
        RawKeyEvent {
          vk: ev.vk,
          ctrl: false,
          alt: false,
          shift: false,
          win: false,
          timestamp_ms: ev.timestamp_ms,
        },
        Some(remapped.clone()),
      );
      LOG.info(&format!(
        "button {} ({}) -> {}",
        mapping.index,
        VkHelper::vk_name(ev.vk),
        remapped
      ));
    } else {
      self.append_capture(
        RawKeyEvent {
          vk: ev.vk,
          ctrl: false,
          alt: false,
          shift: false,
          win: false,
          timestamp_ms: ev.timestamp_ms,
        },
        None,
      );
    }

    HookDecision::Swallow
  }

  fn record_raw_capture(&self, ev: RawKeyEvent) {
    self.append_capture(ev, None);
  }

  fn append_capture(&self, ev: RawKeyEvent, remapped_to: Option<String>) {
    let (display, detail) =
      NagaRawInputMonitor::format_display(ev.vk, ev.ctrl, ev.alt, ev.shift, ev.win);
    let mut changed = false;

    {
      let mut rt = self.runtime.write().unwrap();
      // Deduplicate identical hook + Raw Input arrival within 80ms; upgrade remapped_to if known.
      if let Some(latest) = rt.captures.first_mut() {
        if latest.vk == ev.vk && ev.timestamp_ms.saturating_sub(latest.timestamp_ms) <= 80 {
          if remapped_to.is_some() && latest.remapped_to.is_none() {
            latest.remapped_to = remapped_to;
            changed = true;
          }
        } else {
          let id = rt.next_capture_id;
          rt.next_capture_id = rt.next_capture_id.saturating_add(1);
          rt.captures.insert(
            0,
            NagaCapturedEvent {
              id,
              vk: ev.vk,
              ctrl: ev.ctrl,
              alt: ev.alt,
              shift: ev.shift,
              win: ev.win,
              display,
              detail,
              timestamp_ms: ev.timestamp_ms,
              remapped_to,
            },
          );
          if rt.captures.len() > MAX_CAPTURES {
            rt.captures.truncate(MAX_CAPTURES);
          }
          changed = true;
        }
      } else {
        let id = rt.next_capture_id;
        rt.next_capture_id = rt.next_capture_id.saturating_add(1);
        rt.captures.push(NagaCapturedEvent {
          id,
          vk: ev.vk,
          ctrl: ev.ctrl,
          alt: ev.alt,
          shift: ev.shift,
          win: ev.win,
          display,
          detail,
          timestamp_ms: ev.timestamp_ms,
          remapped_to,
        });
        changed = true;
      }
    }

    if changed {
      self.emit_snapshot();
    }
  }

  pub fn set_enabled(&self, enabled: bool) -> Result<NagaSnapshot, String> {
    self.prefs.update(|cfg| {
      cfg.razer_naga.enabled = enabled;
    })?;
    self.pressed_sentinels.lock().unwrap().clear();
    let snap = self.snapshot();
    self.emit_snapshot();
    Ok(snap)
  }

  pub fn set_plate_layout(&self, layout: NagaPlateLayout) -> Result<NagaSnapshot, String> {
    self.prefs.update(|cfg| {
      cfg.razer_naga.set_plate_layout(layout);
    })?;
    self.pressed_sentinels.lock().unwrap().clear();
    let snap = self.snapshot();
    self.emit_snapshot();
    Ok(snap)
  }

  pub fn set_button_shortcut(
    &self,
    button_index: u8,
    shortcut: Option<KeyShortcut>,
  ) -> Result<NagaSnapshot, String> {
    self.prefs.update(|cfg| {
      if let Some(m) = cfg
        .razer_naga
        .mappings
        .iter_mut()
        .find(|m| m.index == button_index)
      {
        m.target = shortcut;
      }
    })?;
    let snap = self.snapshot();
    self.emit_snapshot();
    Ok(snap)
  }

  pub fn set_button_sentinel(&self, button_index: u8, sentinel_vk: u16) -> Result<NagaSnapshot, String> {
    self.prefs.update(|cfg| {
      if let Some(m) = cfg
        .razer_naga
        .mappings
        .iter_mut()
        .find(|m| m.index == button_index)
      {
        m.sentinel_vk = sentinel_vk;
      }
    })?;
    let snap = self.snapshot();
    self.emit_snapshot();
    Ok(snap)
  }

  pub fn set_button_enabled(&self, button_index: u8, enabled: bool) -> Result<NagaSnapshot, String> {
    self.prefs.update(|cfg| {
      if let Some(m) = cfg
        .razer_naga
        .mappings
        .iter_mut()
        .find(|m| m.index == button_index)
      {
        m.enabled = enabled;
      }
    })?;
    let snap = self.snapshot();
    self.emit_snapshot();
    Ok(snap)
  }

  pub fn begin_recording(
    self: &Arc<Self>,
    button_index: u8,
    mode: NagaRecordMode,
  ) -> NagaSnapshot {
    let already_same = {
      let rt = self.runtime.read().unwrap();
      rt.recording.as_ref().map(|r| (r.button_index, r.mode)) == Some((button_index, mode))
    };
    if already_same {
      return self.cancel_recording();
    }

    let gen = self.record_gen.fetch_add(1, Ordering::SeqCst) + 1;
    {
      let mut rt = self.runtime.write().unwrap();
      rt.recording = Some(NagaRecordingState { button_index, mode });
    }
    self.emit_snapshot();

    let svc = Arc::clone(self);
    match mode {
      NagaRecordMode::Target => {
        self.hook.begin_recording(RecordRequest::Shortcut(Box::new(
          move |shortcut_opt, is_clear| {
            svc.finish_target_recording(gen, button_index, shortcut_opt, is_clear);
          },
        )));
      }
      NagaRecordMode::Sentinel => {
        self.hook.begin_recording(RecordRequest::SingleKey(Box::new(
          move |vk_opt| {
            svc.finish_sentinel_recording(gen, button_index, vk_opt);
          },
        )));
      }
    }

    // 8-second safety timeout so recording can never stay stuck
    let timeout_svc = Arc::clone(self);
    thread::spawn(move || {
      thread::sleep(Duration::from_secs(8));
      if timeout_svc.record_gen.load(Ordering::SeqCst) == gen {
        timeout_svc.cancel_recording_internal(true);
      }
    });

    self.snapshot()
  }

  fn finish_target_recording(
    &self,
    gen: u64,
    button_index: u8,
    shortcut_opt: Option<KeyShortcut>,
    is_clear: bool,
  ) {
    if self.record_gen.load(Ordering::SeqCst) != gen {
      return;
    }
    self.record_gen.fetch_add(1, Ordering::SeqCst);
    {
      let mut rt = self.runtime.write().unwrap();
      rt.recording = None;
    }
    if is_clear {
      let _ = self.set_button_shortcut(button_index, None);
      return;
    }
    if let Some(sc) = shortcut_opt {
      let _ = self.set_button_shortcut(button_index, Some(sc));
      return;
    }
    self.emit_snapshot();
  }

  fn finish_sentinel_recording(&self, gen: u64, button_index: u8, vk_opt: Option<u16>) {
    if self.record_gen.load(Ordering::SeqCst) != gen {
      return;
    }
    self.record_gen.fetch_add(1, Ordering::SeqCst);
    {
      let mut rt = self.runtime.write().unwrap();
      rt.recording = None;
    }
    if let Some(vk) = vk_opt {
      let _ = self.set_button_sentinel(button_index, vk);
      return;
    }
    self.emit_snapshot();
  }

  pub fn cancel_recording(&self) -> NagaSnapshot {
    self.cancel_recording_internal(true);
    self.snapshot()
  }

  fn cancel_recording_internal(&self, notify: bool) {
    self.record_gen.fetch_add(1, Ordering::SeqCst);
    self.hook.cancel_recording();
    let changed = {
      let mut rt = self.runtime.write().unwrap();
      rt.recording.take().is_some()
    };
    if notify && changed {
      self.emit_snapshot();
    }
  }

  pub fn clear_captures(&self) -> NagaSnapshot {
    {
      let mut rt = self.runtime.write().unwrap();
      rt.captures.clear();
    }
    self.emit_snapshot();
    self.snapshot()
  }
}

pub struct NagaTool {
  service: Arc<NagaService>,
}

impl NagaTool {
  pub fn new(service: Arc<NagaService>) -> Self {
    Self { service }
  }
}

impl XTool for NagaTool {
  fn id(&self) -> &'static str {
    "razer-naga"
  }

  fn activate(&self) {
    self.service.start();
  }

  fn shutdown(&self) {
    self.service.stop();
  }
}
