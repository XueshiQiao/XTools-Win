use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::core::file_log::FileLog;
use crate::core::preferences::PreferencesStore;
use crate::core::tool::XTool;

use super::config::NagaPlateLayout;
use super::hid_device::{NagaError, NagaHidDevice};
use super::protocol::{
  NagaBatteryInfo, NagaDeviceState, NagaDpi, NagaDpiStages, NagaHardwareAction, NagaHardwareButton,
  NagaLink, NagaModel, NagaProfileInfo,
};
use super::raw_input::NagaDeviceMonitor;

const LOG: FileLog = FileLog::new("RazerNaga");
const BATTERY_POLL_SECS: u64 = 60;

#[derive(Debug, Clone, Serialize)]
pub struct NagaSnapshot {
  pub is_connected: bool,
  pub model_name: String,
  pub link: Option<NagaLink>,
  pub firmware_version: Option<String>,
  pub battery: Option<NagaBatteryInfo>,
  pub dpi: Option<NagaDpi>,
  pub dpi_stages: Option<NagaDpiStages>,
  pub polling_rate_hz: Option<u16>,
  pub profile_info: Option<NagaProfileInfo>,
  pub plate_layout: NagaPlateLayout,
  pub buttons: Vec<NagaHardwareButton>,
  pub is_busy: bool,
  pub status_message: Option<String>,
  pub error_message: Option<String>,
}

struct RuntimeData {
  is_connected: bool,
  link: Option<NagaLink>,
  firmware_version: Option<String>,
  battery: Option<NagaBatteryInfo>,
  dpi: Option<NagaDpi>,
  dpi_stages: Option<NagaDpiStages>,
  polling_rate_hz: Option<u16>,
  profile_info: Option<NagaProfileInfo>,
  buttons: Vec<NagaHardwareButton>,
  is_busy: bool,
  status_message: Option<String>,
  error_message: Option<String>,
}

impl Default for RuntimeData {
  fn default() -> Self {
    Self {
      is_connected: false,
      link: None,
      firmware_version: None,
      battery: None,
      dpi: None,
      dpi_stages: None,
      polling_rate_hz: None,
      profile_info: None,
      buttons: Vec::new(),
      is_busy: false,
      status_message: None,
      error_message: None,
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
  device_monitor: NagaDeviceMonitor,
  hid_mutex: Arc<Mutex<()>>,
  runtime: RwLock<RuntimeData>,
  poll_stop: AtomicBool,
  poll_tx: Mutex<Option<mpsc::Sender<PollCommand>>>,
  poll_thread: Mutex<Option<JoinHandle<()>>>,
  observer: RwLock<Option<SnapshotObserver>>,
}

impl NagaService {
  pub fn new(prefs: Arc<PreferencesStore>) -> Arc<Self> {
    Arc::new(Self {
      prefs,
      device_monitor: NagaDeviceMonitor::new(),
      hid_mutex: Arc::new(Mutex::new(())),
      runtime: RwLock::new(RuntimeData::default()),
      poll_stop: AtomicBool::new(false),
      poll_tx: Mutex::new(None),
      poll_thread: Mutex::new(None),
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
      dpi_stages: rt.dpi_stages.clone(),
      polling_rate_hz: rt.polling_rate_hz,
      profile_info: rt.profile_info.clone(),
      plate_layout: cfg.plate_layout,
      buttons: rt.buttons.clone(),
      is_busy: rt.is_busy,
      status_message: rt.status_message.clone(),
      error_message: rt.error_message.clone(),
    }
  }

  fn emit_snapshot(&self) {
    let snap = self.snapshot();
    if let Some(obs) = self.observer.read().unwrap().as_ref().cloned() {
      obs(&snap);
    }
  }

  pub fn start(self: &Arc<Self>) {
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
    self.device_monitor.start(move || {
      let _ = usb_tx.send(PollCommand::UsbChanged);
    });

    let _ = tx.send(PollCommand::RefreshNow);
    LOG.info("Razer Naga V3 Pro service started");
  }

  pub fn stop(&self) {
    self.device_monitor.stop();
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

  fn apply_device_state(
    rt: &mut RuntimeData,
    state: NagaDeviceState,
    status_message: Option<String>,
  ) {
    rt.is_connected = true;
    rt.link = Some(state.link);
    rt.firmware_version = state.firmware_version;
    rt.battery = state.battery;
    rt.dpi = state.dpi;
    rt.dpi_stages = state.dpi_stages;
    rt.polling_rate_hz = state.polling_rate_hz;
    rt.profile_info = state.profile_info;
    rt.buttons = state.buttons;
    rt.status_message = status_message;
    rt.error_message = None;
  }

  fn apply_error(rt: &mut RuntimeData, err: &NagaError, is_zh: bool) {
    match err {
      NagaError::NotFound | NagaError::Offline => {
        rt.is_connected = false;
        rt.link = None;
        rt.battery = None;
        rt.dpi = None;
        rt.dpi_stages = None;
        rt.polling_rate_hz = None;
        rt.profile_info = None;
        rt.buttons.clear();
        rt.error_message = None;
      }
      _ => {
        rt.error_message = Some(err.message(is_zh));
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

    let plate_layout = self.prefs.get().razer_naga.plate_layout;
    let result = NagaHidDevice::read_active_state(plate_layout);
    let is_zh = self.prefs.effective_is_zh();

    {
      let mut rt = self.runtime.write().unwrap();
      rt.is_busy = false;
      match result {
        Ok(state) => Self::apply_device_state(&mut rt, state, None),
        Err(ref err) => Self::apply_error(&mut rt, err, is_zh),
      }
    }

    self.emit_snapshot();
  }

  fn run_hardware_mutation<F>(
    &self,
    success_message: Option<String>,
    op: F,
  ) -> Result<NagaSnapshot, String>
  where
    F: FnOnce(&NagaHidDevice, u8, NagaPlateLayout) -> Result<(), NagaError>,
  {
    let is_zh = self.prefs.effective_is_zh();
    let plate_layout = self.prefs.get().razer_naga.plate_layout;

    let result: Result<NagaDeviceState, NagaError> = {
      let _hid_guard = self.hid_mutex.lock().unwrap();
      {
        let mut rt = self.runtime.write().unwrap();
        rt.is_busy = true;
        rt.error_message = None;
      }
      self.emit_snapshot();

      let outcome = NagaHidDevice::open_active().and_then(|dev| {
        let active_profile = dev
          .read_profile_info()
          .map(|p| p.active_profile)
          .unwrap_or(1);
        op(&dev, active_profile, plate_layout)?;
        dev.read_device_state(plate_layout)
      });

      let mut rt = self.runtime.write().unwrap();
      rt.is_busy = false;
      match &outcome {
        Ok(state) => Self::apply_device_state(&mut rt, state.clone(), success_message),
        Err(err) => {
          rt.status_message = None;
          match err {
            NagaError::NotFound | NagaError::Offline => {
              Self::apply_error(&mut rt, err, is_zh);
              rt.error_message = Some(err.message(is_zh));
            }
            _ => {
              rt.error_message = Some(err.message(is_zh));
            }
          }
        }
      }
      outcome
    };

    let snap = self.snapshot();
    self.emit_snapshot();
    result.map(|_| snap).map_err(|e| e.message(is_zh))
  }

  pub fn set_plate_layout(&self, layout: NagaPlateLayout) -> Result<NagaSnapshot, String> {
    self.prefs.update(|cfg| {
      cfg.razer_naga.plate_layout = layout;
    })?;

    if self.runtime.read().unwrap().is_connected {
      let _ = self.run_hardware_mutation(None, |_dev, _profile, _layout| Ok(()));
    } else {
      self.emit_snapshot();
    }
    Ok(self.snapshot())
  }

  pub fn set_active_profile(&self, profile: u8) -> Result<NagaSnapshot, String> {
    let is_zh = self.prefs.effective_is_zh();
    let msg = if is_zh {
      format!("已切换到板载配置文件 {}。", profile)
    } else {
      format!("Switched to onboard Profile {}.", profile)
    };
    let snap = self.run_hardware_mutation(Some(msg), |dev, _active, _layout| {
      dev.set_active_profile(profile)
    })?;
    LOG.info(&format!("switched active onboard profile to {}", profile));
    Ok(snap)
  }

  pub fn delete_profile(&self, profile: u8) -> Result<NagaSnapshot, String> {
    let is_zh = self.prefs.effective_is_zh();
    let msg = if is_zh {
      format!("已删除板载配置文件 {}。", profile)
    } else {
      format!("Deleted onboard Profile {}.", profile)
    };
    let snap = self.run_hardware_mutation(Some(msg), |dev, _active, _layout| {
      dev.delete_profile(profile)
    })?;
    LOG.info(&format!("deleted onboard profile {}", profile));
    Ok(snap)
  }

  pub fn set_button_action(
    &self,
    button_id: u8,
    action: NagaHardwareAction,
  ) -> Result<NagaSnapshot, String> {
    let is_zh = self.prefs.effective_is_zh();
    let resolved = action.resolve()?;
    let msg = if is_zh {
      "侧键映射已写入鼠标板载内存。".to_string()
    } else {
      "Button mapping saved to mouse onboard memory.".to_string()
    };
    let snap = self.run_hardware_mutation(Some(msg), |dev, active_profile, _layout| {
      let readback = dev.set_button_action(active_profile, button_id, &resolved)?;
      LOG.info(&format!(
        "onboard button 0x{:02X} on profile {} set -> {:?}",
        button_id, active_profile, readback
      ));
      Ok(())
    })?;
    Ok(snap)
  }

  pub fn reset_plate_defaults(&self) -> Result<NagaSnapshot, String> {
    let is_zh = self.prefs.effective_is_zh();
    let msg = if is_zh {
      "当前侧面板按键已恢复出厂默认并写入板载内存。".to_string()
    } else {
      "Current side plate buttons restored to factory defaults.".to_string()
    };
    self.run_hardware_mutation(Some(msg), |dev, active_profile, layout| {
      for &button_id in layout.button_ids() {
        let def = layout.factory_default_action(button_id);
        dev.set_button_action(active_profile, button_id, &def)?;
      }
      LOG.info(&format!(
        "restored {:?} plate factory defaults on profile {}",
        layout, active_profile
      ));
      Ok(())
    })
  }

  pub fn set_dpi_stages(
    &self,
    active_stage: u8,
    stages: Vec<NagaDpi>,
  ) -> Result<NagaSnapshot, String> {
    let is_zh = self.prefs.effective_is_zh();
    let msg = if is_zh {
      "DPI 档位已写入鼠标板载内存。".to_string()
    } else {
      "DPI stages saved to mouse onboard memory.".to_string()
    };
    self.run_hardware_mutation(Some(msg), |dev, active_profile, _layout| {
      dev.set_dpi_stages(active_profile, active_stage, &stages)?;
      LOG.info(&format!(
        "set DPI stages on profile {}: active={}, stages={:?}",
        active_profile, active_stage, stages
      ));
      Ok(())
    })
  }

  pub fn set_polling_rate(&self, hz: u16) -> Result<NagaSnapshot, String> {
    let is_zh = self.prefs.effective_is_zh();
    let msg = if is_zh {
      format!("轮询率已设为 {} Hz。", hz)
    } else {
      format!("Polling rate set to {} Hz.", hz)
    };
    self.run_hardware_mutation(Some(msg), |dev, _active, _layout| {
      dev.set_polling_rate(hz)?;
      LOG.info(&format!("set polling rate -> {} Hz", hz));
      Ok(())
    })
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
