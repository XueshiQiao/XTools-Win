use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::core::file_log::FileLog;
use crate::core::preferences::PreferencesStore;
use crate::core::tool::XTool;

use super::events::{UsbEventKind, UsbEventLogReader, UsbLiveEvent};
use super::topology::{
  UsbControllerNode, UsbDiagnosticHint, UsbPortNode, UsbPortStatus, UsbTopologyScanner,
};

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

const LOG: FileLog = FileLog::new("UsbInspector");
const RECENT_HIGHLIGHT_TTL: Duration = Duration::from_secs(6);
const MAX_LIVE_EVENTS: usize = 30;

#[derive(Debug, Clone, Serialize)]
pub struct UsbInspectorSnapshot {
  pub controllers: Vec<UsbControllerNode>,
  pub diagnostics: Vec<UsbDiagnosticHint>,
  pub recent_events: Vec<UsbLiveEvent>,
  pub show_empty_ports: bool,
  pub is_scanning: bool,
  pub last_scan_time: Option<String>,
}

enum WorkerMsg {
  Scan { from_hotplug: bool },
  ClearHighlights,
  Shutdown,
}

#[derive(Debug, Clone)]
struct PortOccupant {
  device_name: String,
  instance_id: String,
}

type StateObserver = Arc<dyn Fn(&UsbInspectorSnapshot) + Send + Sync>;

pub struct UsbInspectorService {
  prefs: Arc<PreferencesStore>,
  controllers: RwLock<Vec<UsbControllerNode>>,
  diagnostics: RwLock<Vec<UsbDiagnosticHint>>,
  eventlog_events: RwLock<Vec<UsbLiveEvent>>,
  live_events: Mutex<Vec<UsbLiveEvent>>,
  recently_changed: Mutex<HashMap<String, Instant>>,
  previous_occupants: Mutex<Option<HashMap<String, PortOccupant>>>,
  is_scanning: AtomicBool,
  last_scan_time: RwLock<Option<String>>,
  observer: RwLock<Option<StateObserver>>,
  stop_flag: Arc<AtomicBool>,
  worker_tx: Mutex<Option<mpsc::Sender<WorkerMsg>>>,
  worker_handle: Mutex<Option<JoinHandle<()>>>,
  #[cfg(target_os = "windows")]
  window_hwnd: Arc<Mutex<Option<isize>>>,
  pnp_handle: Mutex<Option<JoinHandle<()>>>,
}

impl UsbInspectorService {
  pub fn new(prefs: Arc<PreferencesStore>) -> Arc<Self> {
    Arc::new(Self {
      prefs,
      controllers: RwLock::new(Vec::new()),
      diagnostics: RwLock::new(Vec::new()),
      eventlog_events: RwLock::new(Vec::new()),
      live_events: Mutex::new(Vec::new()),
      recently_changed: Mutex::new(HashMap::new()),
      previous_occupants: Mutex::new(None),
      is_scanning: AtomicBool::new(false),
      last_scan_time: RwLock::new(None),
      observer: RwLock::new(None),
      stop_flag: Arc::new(AtomicBool::new(false)),
      worker_tx: Mutex::new(None),
      worker_handle: Mutex::new(None),
      #[cfg(target_os = "windows")]
      window_hwnd: Arc::new(Mutex::new(None)),
      pnp_handle: Mutex::new(None),
    })
  }

  pub fn set_observer<F>(&self, cb: F)
  where
    F: Fn(&UsbInspectorSnapshot) + Send + Sync + 'static,
  {
    *self.observer.write().unwrap() = Some(Arc::new(cb));
  }

  pub fn snapshot(&self) -> UsbInspectorSnapshot {
    let cfg = self.prefs.get().usb_inspector;
    let mut controllers = self.controllers.read().unwrap().clone();
    let active_highlights = self.active_highlight_keys();
    Self::apply_config_and_highlights(&mut controllers, &cfg.port_labels, &active_highlights);

    let mut merged_events = self.live_events.lock().unwrap().clone();
    merged_events.extend(self.eventlog_events.read().unwrap().clone());
    merged_events.truncate(MAX_LIVE_EVENTS);

    UsbInspectorSnapshot {
      controllers,
      diagnostics: self.diagnostics.read().unwrap().clone(),
      recent_events: merged_events,
      show_empty_ports: cfg.show_empty_ports,
      is_scanning: self.is_scanning.load(Ordering::SeqCst),
      last_scan_time: self.last_scan_time.read().unwrap().clone(),
    }
  }

  fn apply_config_and_highlights(
    controllers: &mut [UsbControllerNode],
    labels: &std::collections::BTreeMap<String, String>,
    highlights: &HashSet<String>,
  ) {
    fn visit(
      ports: &mut [UsbPortNode],
      labels: &std::collections::BTreeMap<String, String>,
      highlights: &HashSet<String>,
    ) {
      for p in ports {
        p.custom_label = labels.get(&p.port_key).cloned();
        p.recently_changed = highlights.contains(&p.port_key);
        visit(&mut p.hub_children, labels, highlights);
      }
    }
    for c in controllers {
      visit(&mut c.ports, labels, highlights);
    }
  }

  fn active_highlight_keys(&self) -> HashSet<String> {
    let now = Instant::now();
    let mut guard = self.recently_changed.lock().unwrap();
    guard.retain(|_, exp| *exp > now);
    guard.keys().cloned().collect()
  }

  fn emit_snapshot(&self) {
    let snap = self.snapshot();
    if let Some(obs) = self.observer.read().unwrap().as_ref().cloned() {
      obs(&snap);
    }
  }

  pub fn refresh(&self) {
    if let Some(tx) = self.worker_tx.lock().unwrap().as_ref() {
      let _ = tx.send(WorkerMsg::Scan { from_hotplug: false });
    }
  }

  pub fn set_port_label(
    &self,
    port_key: String,
    label: String,
  ) -> Result<UsbInspectorSnapshot, String> {
    self.prefs.update(|cfg| {
      cfg.usb_inspector.set_label(&port_key, &label);
    })?;
    let snap = self.snapshot();
    self.emit_snapshot();
    Ok(snap)
  }

  pub fn set_show_empty_ports(&self, show: bool) -> Result<UsbInspectorSnapshot, String> {
    self.prefs.update(|cfg| {
      cfg.usb_inspector.show_empty_ports = show;
    })?;
    let snap = self.snapshot();
    self.emit_snapshot();
    Ok(snap)
  }

  fn run_scan(&self, from_hotplug: bool) {
    self.is_scanning.store(true, Ordering::SeqCst);
    self.emit_snapshot();

    let cfg = self.prefs.get().usb_inspector;
    let active_highlights = self.active_highlight_keys();
    let outcome = UsbTopologyScanner::scan(&cfg, &active_highlights);

    let new_occupants = Self::collect_occupants(&outcome.controllers);
    let mut newly_changed = Vec::new();

    {
      let mut prev_guard = self.previous_occupants.lock().unwrap();
      if let Some(prev) = prev_guard.as_ref() {
        if from_hotplug {
          let now_ts = UsbEventLogReader::current_local_hh_mm_ss();
          let mut live = self.live_events.lock().unwrap();

          for (pk, new_occ) in &new_occupants {
            let changed = match prev.get(pk) {
              None => true,
              Some(old_occ) => old_occ.instance_id != new_occ.instance_id,
            };
            if changed {
              newly_changed.push(pk.clone());
              live.insert(
                0,
                UsbLiveEvent {
                  timestamp: now_ts.clone(),
                  kind: UsbEventKind::Arrived,
                  device_name: new_occ.device_name.clone(),
                  instance_id: new_occ.instance_id.clone(),
                  port_key: Some(pk.clone()),
                },
              );
            }
          }

          for (pk, old_occ) in prev {
            if !new_occupants.contains_key(pk) {
              newly_changed.push(pk.clone());
              live.insert(
                0,
                UsbLiveEvent {
                  timestamp: now_ts.clone(),
                  kind: UsbEventKind::Removed,
                  device_name: old_occ.device_name.clone(),
                  instance_id: old_occ.instance_id.clone(),
                  port_key: Some(pk.clone()),
                },
              );
            }
          }

          live.truncate(MAX_LIVE_EVENTS);
        }
      }
      *prev_guard = Some(new_occupants);
    }

    if !newly_changed.is_empty() {
      let expiry = Instant::now() + RECENT_HIGHLIGHT_TTL;
      {
        let mut guard = self.recently_changed.lock().unwrap();
        for pk in newly_changed {
          guard.insert(pk, expiry);
        }
      }
      if let Some(tx) = self.worker_tx.lock().unwrap().as_ref().cloned() {
        thread::spawn(move || {
          thread::sleep(RECENT_HIGHLIGHT_TTL + Duration::from_millis(200));
          let _ = tx.send(WorkerMsg::ClearHighlights);
        });
      }
    }

    *self.controllers.write().unwrap() = outcome.controllers;
    *self.diagnostics.write().unwrap() = outcome.diagnostics;
    *self.eventlog_events.write().unwrap() = outcome.recent_events;
    *self.last_scan_time.write().unwrap() = Some(UsbEventLogReader::current_local_hh_mm_ss());
    self.is_scanning.store(false, Ordering::SeqCst);
    self.emit_snapshot();
  }

  fn collect_occupants(controllers: &[UsbControllerNode]) -> HashMap<String, PortOccupant> {
    fn visit(ports: &[UsbPortNode], out: &mut HashMap<String, PortOccupant>) {
      for p in ports {
        if p.status != UsbPortStatus::Empty {
          out.insert(
            p.port_key.clone(),
            PortOccupant {
              device_name: p
                .device_name
                .clone()
                .unwrap_or_else(|| format!("Port {}", p.port_number)),
              instance_id: p.instance_id.clone().unwrap_or_default(),
            },
          );
        }
        visit(&p.hub_children, out);
      }
    }
    let mut map = HashMap::new();
    for c in controllers {
      visit(&c.ports, &mut map);
    }
    map
  }

  fn start_worker(self: &Arc<Self>) {
    let mut guard = self.worker_handle.lock().unwrap();
    if guard.is_some() {
      return;
    }

    self.stop_flag.store(false, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel::<WorkerMsg>();
    *self.worker_tx.lock().unwrap() = Some(tx.clone());

    let svc = Arc::clone(self);
    let handle = thread::Builder::new()
      .name("usb-inspector-worker".to_string())
      .spawn(move || {
        while let Ok(msg) = rx.recv() {
          match msg {
            WorkerMsg::Shutdown => break,
            WorkerMsg::ClearHighlights => {
              let _ = svc.active_highlight_keys();
              svc.emit_snapshot();
            }
            WorkerMsg::Scan { mut from_hotplug } => {
              if from_hotplug {
                thread::sleep(Duration::from_millis(450));
              }
              while let Ok(extra) = rx.try_recv() {
                match extra {
                  WorkerMsg::Shutdown => return,
                  WorkerMsg::Scan { from_hotplug: f } => from_hotplug |= f,
                  WorkerMsg::ClearHighlights => {}
                }
              }
              svc.run_scan(from_hotplug);
            }
          }
        }
      })
      .expect("failed to spawn usb-inspector-worker thread");
    *guard = Some(handle);

    #[cfg(target_os = "windows")]
    {
      let mut pnp_guard = self.pnp_handle.lock().unwrap();
      if pnp_guard.is_none() {
        let stop = Arc::clone(&self.stop_flag);
        let hwnd_slot = Arc::clone(&self.window_hwnd);
        let pnp_tx = tx.clone();
        let pnp = thread::Builder::new()
          .name("usb-pnp-window".to_string())
          .spawn(move || {
            Self::win32_device_listener(stop, hwnd_slot, pnp_tx);
          })
          .expect("failed to spawn usb-pnp-window thread");
        *pnp_guard = Some(pnp);
      }
    }

    let _ = tx.send(WorkerMsg::Scan { from_hotplug: false });
    LOG.info("USB Inspector service started");
  }

  fn stop_worker(&self) {
    self.stop_flag.store(true, Ordering::SeqCst);
    if let Some(tx) = self.worker_tx.lock().unwrap().take() {
      let _ = tx.send(WorkerMsg::Shutdown);
    }
    #[cfg(target_os = "windows")]
    {
      if let Some(raw) = self.window_hwnd.lock().unwrap().take() {
        unsafe {
          let _ = PostMessageW(
            Some(HWND(raw as *mut std::ffi::c_void)),
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0),
          );
        }
      }
    }
    if let Some(h) = self.pnp_handle.lock().unwrap().take() {
      let _ = h.join();
    }
    if let Some(h) = self.worker_handle.lock().unwrap().take() {
      let _ = h.join();
    }
    LOG.info("USB Inspector service stopped");
  }

  #[cfg(target_os = "windows")]
  fn win32_device_listener(
    stop_flag: Arc<AtomicBool>,
    hwnd_slot: Arc<Mutex<Option<isize>>>,
    tx: mpsc::Sender<WorkerMsg>,
  ) {
    unsafe {
      const GUID_DEVINTERFACE_USB_DEVICE: GUID =
        GUID::from_u128(0xa5dcbf10_6530_11d2_901f_00c04fb951ed);
      const GUID_DEVINTERFACE_USB_HUB: GUID =
        GUID::from_u128(0xf18a0e88_c30c_11d0_8815_00a0c906bed8);

      let class_name = w!("XToolsUsbMonitorWindow");
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
        w!("XToolsUsbMonitor"),
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
        LOG.error("Failed to create hidden USB PnP listener window");
        return;
      };

      *hwnd_slot.lock().unwrap() = Some(hwnd.0 as isize);

      let mut notifications: Vec<HDEVNOTIFY> = Vec::new();
      for guid in [GUID_DEVINTERFACE_USB_DEVICE, GUID_DEVINTERFACE_USB_HUB] {
        let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
          dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
          dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
          dbcc_reserved: 0,
          dbcc_classguid: guid,
          dbcc_name: [0],
        };
        if let Ok(notify) = RegisterDeviceNotificationW(
          HANDLE(hwnd.0),
          &filter as *const DEV_BROADCAST_DEVICEINTERFACE_W as *const std::ffi::c_void,
          DEVICE_NOTIFY_WINDOW_HANDLE,
        ) {
          notifications.push(notify);
        }
      }

      let mut msg = MSG::default();
      while !stop_flag.load(Ordering::SeqCst) {
        let ret = GetMessageW(&mut msg, None, 0, 0);
        if ret.0 <= 0 {
          break;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
      }

      for notify in notifications {
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
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const mpsc::Sender<WorkerMsg>;
            if !ptr.is_null() {
              let _ = (*ptr).send(WorkerMsg::Scan { from_hotplug: true });
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

pub struct UsbInspectorTool {
  service: Arc<UsbInspectorService>,
}

impl UsbInspectorTool {
  pub fn new(service: Arc<UsbInspectorService>) -> Self {
    Self { service }
  }
}

impl XTool for UsbInspectorTool {
  fn id(&self) -> &'static str {
    "usb-inspector"
  }

  fn activate(&self) {
    self.service.start_worker();
  }

  fn shutdown(&self) {
    self.service.stop_worker();
  }
}
