use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::hid_device::RogHidDevice;
use super::protocol::RogModel;
use crate::core::file_log::FileLog;

#[cfg(target_os = "windows")]
use windows::{
  core::{w, GUID},
  Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
  Win32::System::LibraryLoader::GetModuleHandleW,
  Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    RegisterClassExW, RegisterDeviceNotificationW, TranslateMessage, UnregisterDeviceNotification,
    DBT_DEVICEARRIVAL, DBT_DEVICEREMOVECOMPLETE, DBT_DEVTYP_DEVICEINTERFACE,
    DEVICE_NOTIFY_WINDOW_HANDLE, DEV_BROADCAST_DEVICEINTERFACE_W, DEV_BROADCAST_HDR, HDEVNOTIFY,
    MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_DEVICECHANGE, WNDCLASSEXW,
  },
};

const LOG: FileLog = FileLog::new("ROGKeyboardMonitor");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorEvent {
  Arrived,
  Detached,
}

/// Pure, unit-testable state tracker for 2.4 GHz wireless dongle reconnections.
/// Over 2.4 GHz, flipping the keyboard switch away from Windows leaves the USB
/// receiver plugged in, so Windows PnP emits no arrival/removal events; probing the
/// dongle catches offline -> online transitions with 2-failure hysteresis so a single
/// transient packet miss does not flicker connection state.
pub struct WirelessPresenceTracker {
  online: bool,
  consecutive_failures: u8,
}

impl WirelessPresenceTracker {
  pub fn new(initial_online: bool) -> Self {
    Self {
      online: initial_online,
      consecutive_failures: 0,
    }
  }

  pub fn is_online(&self) -> bool {
    self.online
  }

  pub fn seed(&mut self, online: bool) {
    self.online = online;
    self.consecutive_failures = 0;
  }

  pub fn on_probe_success(&mut self) -> Option<MonitorEvent> {
    self.consecutive_failures = 0;
    if !self.online {
      self.online = true;
      Some(MonitorEvent::Arrived)
    } else {
      None
    }
  }

  pub fn on_probe_failure(&mut self) -> Option<MonitorEvent> {
    if !self.online {
      return None;
    }
    self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    if self.consecutive_failures >= 2 {
      self.online = false;
      self.consecutive_failures = 0;
      Some(MonitorEvent::Detached)
    } else {
      None
    }
  }
}

enum CoordMsg {
  UsbChange,
  Shutdown,
}

pub struct RogKeyboardMonitor {
  stop_flag: Arc<AtomicBool>,
  coord_tx: Mutex<Option<mpsc::Sender<CoordMsg>>>,
  #[cfg(target_os = "windows")]
  window_hwnd: Arc<Mutex<Option<isize>>>,
  threads: Mutex<Vec<JoinHandle<()>>>,
}

impl RogKeyboardMonitor {
  pub fn new() -> Self {
    Self {
      stop_flag: Arc::new(AtomicBool::new(false)),
      coord_tx: Mutex::new(None),
      #[cfg(target_os = "windows")]
      window_hwnd: Arc::new(Mutex::new(None)),
      threads: Mutex::new(Vec::new()),
    }
  }

  pub fn start<F>(&self, hid_op_mutex: Arc<Mutex<()>>, on_event: F)
  where
    F: Fn(MonitorEvent) + Send + Sync + 'static,
  {
    let mut threads = self.threads.lock().unwrap();
    if !threads.is_empty() {
      return;
    }
    self.stop_flag.store(false, Ordering::SeqCst);

    let (tx, rx) = mpsc::channel::<CoordMsg>();
    *self.coord_tx.lock().unwrap() = Some(tx.clone());

    let stop_coord = Arc::clone(&self.stop_flag);
    let callback = Arc::new(on_event);
    let coord_handle = thread::Builder::new()
      .name("rog-monitor-coord".to_string())
      .spawn(move || {
        Self::coordinator_loop(rx, stop_coord, hid_op_mutex, callback);
      })
      .expect("failed to spawn rog-monitor-coord thread");
    threads.push(coord_handle);

    #[cfg(target_os = "windows")]
    {
      let stop_win = Arc::clone(&self.stop_flag);
      let hwnd_slot = Arc::clone(&self.window_hwnd);
      let win_tx = tx;
      let win_handle = thread::Builder::new()
        .name("rog-pnp-window".to_string())
        .spawn(move || {
          Self::win32_device_listener(stop_win, hwnd_slot, win_tx);
        })
        .expect("failed to spawn rog-pnp-window thread");
      threads.push(win_handle);
    }

    LOG.info("monitoring started");
  }

  pub fn stop(&self) {
    self.stop_flag.store(true, Ordering::SeqCst);
    if let Some(tx) = self.coord_tx.lock().unwrap().take() {
      let _ = tx.send(CoordMsg::Shutdown);
    }
    #[cfg(target_os = "windows")]
    {
      if let Some(raw_hwnd) = self.window_hwnd.lock().unwrap().take() {
        unsafe {
          let _ = PostMessageW(
            Some(HWND(raw_hwnd as *mut std::ffi::c_void)),
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0),
          );
        }
      }
    }
    let mut threads = self.threads.lock().unwrap();
    for handle in threads.drain(..) {
      let _ = handle.join();
    }
    LOG.info("monitoring stopped");
  }

  fn coordinator_loop(
    rx: mpsc::Receiver<CoordMsg>,
    stop_flag: Arc<AtomicBool>,
    hid_op_mutex: Arc<Mutex<()>>,
    on_event: Arc<dyn Fn(MonitorEvent) + Send + Sync>,
  ) {
    let (mut wired_present, mut dongle_present) = Self::scan_presence();
    let initial_wireless = if !wired_present && dongle_present {
      Self::probe_dongle_once(&hid_op_mutex)
    } else {
      false
    };
    let mut tracker = WirelessPresenceTracker::new(initial_wireless);
    let mut pending_usb_arrival: Option<Instant> = None;

    LOG.info(&format!(
      "initial topology: wired={} dongle={} wireless_online={}",
      wired_present, dongle_present, initial_wireless
    ));

    while !stop_flag.load(Ordering::SeqCst) {
      let wait_duration = if let Some(deadline) = pending_usb_arrival {
        deadline
          .saturating_duration_since(Instant::now())
          .min(Duration::from_millis(250))
      } else if !wired_present && dongle_present {
        if tracker.is_online() {
          Duration::from_millis(4000)
        } else {
          Duration::from_millis(2500)
        }
      } else {
        Duration::from_secs(30)
      };

      match rx.recv_timeout(wait_duration) {
        Ok(CoordMsg::Shutdown) => break,
        Ok(CoordMsg::UsbChange) => {
          // Drain duplicate bursts within 80ms
          thread::sleep(Duration::from_millis(80));
          while rx.try_recv().is_ok() {}

          let (new_wired, new_dongle) = Self::scan_presence();
          if !wired_present && new_wired {
            LOG.info("USB wired ROG Falcata arrived — waiting 800ms settle delay");
            pending_usb_arrival = Some(Instant::now() + Duration::from_millis(800));
            tracker.seed(false);
          } else if wired_present && !new_wired {
            pending_usb_arrival = None;
            tracker.seed(false);
            if !new_dongle {
              LOG.info("USB wired ROG Falcata removed and no 2.4G dongle present");
              on_event(MonitorEvent::Detached);
            } else {
              LOG.info("USB wired ROG Falcata removed; 2.4G dongle present, tracking wireless");
            }
          } else if !new_wired && !dongle_present && new_dongle {
            LOG.info("2.4G dongle arrived — waiting 800ms settle delay");
            tracker.seed(false);
            pending_usb_arrival = Some(Instant::now() + Duration::from_millis(800));
          } else if !new_wired && dongle_present && !new_dongle {
            pending_usb_arrival = None;
            if tracker.is_online() {
              tracker.seed(false);
              on_event(MonitorEvent::Detached);
            }
          }
          wired_present = new_wired;
          dongle_present = new_dongle;
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
          if let Some(deadline) = pending_usb_arrival {
            if Instant::now() >= deadline {
              pending_usb_arrival = None;
              let (new_wired, new_dongle) = Self::scan_presence();
              wired_present = new_wired;
              dongle_present = new_dongle;
              if wired_present {
                LOG.info("USB wired settle window elapsed -> MonitorEvent::Arrived");
                on_event(MonitorEvent::Arrived);
              } else if dongle_present {
                if Self::probe_dongle_once(&hid_op_mutex) {
                  tracker.seed(true);
                  LOG.info("2.4G dongle settle probe succeeded -> MonitorEvent::Arrived");
                  on_event(MonitorEvent::Arrived);
                }
              }
            }
            continue;
          }

          if !wired_present && dongle_present {
            if let Ok(_guard) = hid_op_mutex.try_lock() {
              let ok = Self::probe_dongle_unlocked();
              drop(_guard);
              let ev = if ok {
                tracker.on_probe_success()
              } else {
                tracker.on_probe_failure()
              };
              if let Some(event) = ev {
                if event == MonitorEvent::Arrived {
                  LOG.info("2.4G wireless keyboard reconnected — waiting 500ms settle");
                  thread::sleep(Duration::from_millis(500));
                } else {
                  LOG.info("2.4G wireless keyboard went offline");
                }
                if !stop_flag.load(Ordering::SeqCst) {
                  on_event(event);
                }
              }
            }
          }
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => break,
      }
    }
  }

  fn scan_presence() -> (bool, bool) {
    let devices = RogHidDevice::discover(&RogModel::ALL_PRODUCT_IDS);
    let wired = devices.iter().any(|d| d.product_id == RogModel::PID_WIRED);
    let dongle = devices.iter().any(|d| d.product_id == RogModel::PID_WIRELESS);
    (wired, dongle)
  }

  fn probe_dongle_once(hid_op_mutex: &Arc<Mutex<()>>) -> bool {
    let Ok(_guard) = hid_op_mutex.try_lock() else {
      return false;
    };
    Self::probe_dongle_unlocked()
  }

  fn probe_dongle_unlocked() -> bool {
    let Ok(dev) = RogHidDevice::open_first(&[RogModel::PID_WIRELESS]) else {
      return false;
    };
    dev.read_state(Duration::from_millis(600)).is_ok()
  }

  #[cfg(target_os = "windows")]
  fn win32_device_listener(
    stop_flag: Arc<AtomicBool>,
    hwnd_slot: Arc<Mutex<Option<isize>>>,
    tx: mpsc::Sender<CoordMsg>,
  ) {
    unsafe {
      // GUID_DEVINTERFACE_HID {4D1E55B2-F16F-11CF-88CB-001111000030}
      const GUID_DEVINTERFACE_HID: GUID = GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030);

      let class_name = w!("XToolsRogMonitorWindow");
      let hmodule = GetModuleHandleW(None).unwrap_or_default();
      let hinstance = HINSTANCE(hmodule.0);

      let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(Self::wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
      };
      let _ = RegisterClassExW(&wc);

      let tx_box = Box::new(tx);
      let tx_ptr = Box::into_raw(tx_box);

      let Ok(hwnd) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class_name,
        w!("XToolsRogMonitor"),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        None,
        None,
        Some(hinstance),
        Some(tx_ptr as *const std::ffi::c_void),
      ) else {
        let _ = Box::from_raw(tx_ptr);
        LOG.error("Failed to create hidden Win32 device listener window");
        return;
      };

      *hwnd_slot.lock().unwrap() = Some(hwnd.0 as isize);

      let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
        dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
        dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
        dbcc_reserved: 0,
        dbcc_classguid: GUID_DEVINTERFACE_HID,
        dbcc_name: [0],
      };
      let dev_notify: Option<HDEVNOTIFY> = RegisterDeviceNotificationW(
        HANDLE(hwnd.0),
        &filter as *const DEV_BROADCAST_DEVICEINTERFACE_W as *const std::ffi::c_void,
        DEVICE_NOTIFY_WINDOW_HANDLE,
      )
      .ok();

      let mut msg = MSG::default();
      while !stop_flag.load(Ordering::SeqCst) {
        let ret = GetMessageW(&mut msg, None, 0, 0);
        if ret.0 <= 0 {
          break;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
      }

      if let Some(notify) = dev_notify {
        let _ = UnregisterDeviceNotification(notify);
      }
      *hwnd_slot.lock().unwrap() = None;
      let _ = DestroyWindow(hwnd);
      let _ = Box::from_raw(tx_ptr);
    }
  }

  #[cfg(target_os = "windows")]
  unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
  ) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
      GetWindowLongPtrW, PostQuitMessage, SetWindowLongPtrW, CREATESTRUCTW, GWLP_USERDATA,
      WM_CREATE,
    };

    match msg {
      WM_CREATE => {
        let cs = &*(lparam.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        LRESULT(0)
      }
      WM_DEVICECHANGE => {
        let event = wparam.0 as u32;
        if (event == DBT_DEVICEARRIVAL || event == DBT_DEVICEREMOVECOMPLETE) && lparam.0 != 0 {
          let hdr = &*(lparam.0 as *const DEV_BROADCAST_HDR);
          if hdr.dbch_devicetype == DBT_DEVTYP_DEVICEINTERFACE {
            let iface = &*(lparam.0 as *const DEV_BROADCAST_DEVICEINTERFACE_W);
            let name_ptr = iface.dbcc_name.as_ptr();
            let mut len = 0usize;
            while len < 512 && *name_ptr.add(len) != 0 {
              len += 1;
            }
            let name = String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, len));
            if name.to_ascii_lowercase().contains("vid_0b05") {
              let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const mpsc::Sender<CoordMsg>;
              if !ptr.is_null() {
                let _ = (*ptr).send(CoordMsg::UsbChange);
              }
            }
          }
        }
        LRESULT(1)
      }
      WM_CLOSE => {
        let _ = DestroyWindow(hwnd);
        LRESULT(0)
      }
      WM_DESTROY => {
        PostQuitMessage(0);
        LRESULT(0)
      }
      _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
  }
}

impl Drop for RogKeyboardMonitor {
  fn drop(&mut self) {
    self.stop();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wireless_tracker_arrives_on_first_success_and_ignores_duplicates() {
    let mut tracker = WirelessPresenceTracker::new(false);
    assert_eq!(tracker.on_probe_success(), Some(MonitorEvent::Arrived));
    assert!(tracker.is_online());
    assert_eq!(tracker.on_probe_success(), None);
  }

  #[test]
  fn wireless_tracker_detaches_only_after_two_consecutive_failures() {
    let mut tracker = WirelessPresenceTracker::new(true);
    assert_eq!(tracker.on_probe_failure(), None);
    assert!(tracker.is_online());
    // Single success clears failure count
    assert_eq!(tracker.on_probe_success(), None);
    assert_eq!(tracker.on_probe_failure(), None);
    assert_eq!(tracker.on_probe_failure(), Some(MonitorEvent::Detached));
    assert!(!tracker.is_online());
    // Additional failures while offline stay silent
    assert_eq!(tracker.on_probe_failure(), None);
  }
}
