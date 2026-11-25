use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use super::protocol::NagaModel;
use crate::core::file_log::FileLog;
use crate::core::vk::{KeyShortcut, KeySynthesizer, VkHelper};

#[cfg(target_os = "windows")]
use windows::{
  core::{w, GUID},
  Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
  Win32::System::LibraryLoader::GetModuleHandleW,
  Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, HRAWINPUT, RAWINPUT,
    RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK, RIDI_DEVICENAME, RID_INPUT, RIM_TYPEKEYBOARD,
  },
  Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT},
  Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    RegisterClassExW, RegisterDeviceNotificationW, TranslateMessage, UnregisterDeviceNotification,
    DBT_DEVICEARRIVAL, DBT_DEVICEREMOVECOMPLETE, DBT_DEVTYP_DEVICEINTERFACE,
    DEVICE_NOTIFY_WINDOW_HANDLE, DEV_BROADCAST_DEVICEINTERFACE_W, DEV_BROADCAST_HDR, HDEVNOTIFY,
    MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_DEVICECHANGE, WM_INPUT,
    WNDCLASSEXW,
  },
};

const LOG: FileLog = FileLog::new("NagaRawMonitor");

#[derive(Debug, Clone, Serialize)]
pub struct NagaCapturedEvent {
  pub id: u64,
  pub vk: u16,
  pub ctrl: bool,
  pub alt: bool,
  pub shift: bool,
  pub win: bool,
  pub display: String,
  pub detail: String,
  pub timestamp_ms: u64,
  pub remapped_to: Option<String>,
}

pub struct RawKeyEvent {
  pub vk: u16,
  pub ctrl: bool,
  pub alt: bool,
  pub shift: bool,
  pub win: bool,
  pub timestamp_ms: u64,
}

#[cfg(target_os = "windows")]
struct WindowContext {
  on_usb_change: Arc<dyn Fn() + Send + Sync>,
  on_razer_key: Arc<dyn Fn(RawKeyEvent) + Send + Sync>,
  device_cache: Mutex<HashMap<isize, bool>>,
}

pub struct NagaRawInputMonitor {
  stop_flag: Arc<AtomicBool>,
  hwnd: Arc<Mutex<Option<isize>>>,
  thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl NagaRawInputMonitor {
  pub fn new() -> Self {
    Self {
      stop_flag: Arc::new(AtomicBool::new(false)),
      hwnd: Arc::new(Mutex::new(None)),
      thread_handle: Mutex::new(None),
    }
  }

  pub fn start<F, K>(&self, on_usb_change: F, on_razer_key: K)
  where
    F: Fn() + Send + Sync + 'static,
    K: Fn(RawKeyEvent) + Send + Sync + 'static,
  {
    let mut slot = self.thread_handle.lock().unwrap();
    if slot.is_some() {
      return;
    }
    self.stop_flag.store(false, Ordering::SeqCst);

    #[cfg(target_os = "windows")]
    {
      let stop = Arc::clone(&self.stop_flag);
      let hwnd_slot = Arc::clone(&self.hwnd);
      let usb_cb = Arc::new(on_usb_change);
      let key_cb = Arc::new(on_razer_key);

      let handle = thread::Builder::new()
        .name("naga-raw-monitor".to_string())
        .spawn(move || {
          Self::run_window_loop(stop, hwnd_slot, usb_cb, key_cb);
        })
        .expect("failed to spawn naga-raw-monitor thread");
      *slot = Some(handle);
    }

    #[cfg(not(target_os = "windows"))]
    let _ = (on_usb_change, on_razer_key);
  }

  pub fn stop(&self) {
    self.stop_flag.store(true, Ordering::SeqCst);
    #[cfg(target_os = "windows")]
    {
      if let Some(raw_hwnd) = self.hwnd.lock().unwrap().take() {
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
    if let Some(handle) = self.thread_handle.lock().unwrap().take() {
      let _ = handle.join();
    }
  }

  #[allow(dead_code)]
  pub fn format_display(vk: u16, ctrl: bool, alt: bool, shift: bool, win: bool) -> (String, String) {
    let sc = KeyShortcut {
      vk,
      ctrl,
      alt,
      shift,
      win,
    };
    (sc.display(), format!("VK 0x{:02X} ({})", vk, VkHelper::vk_name(vk)))
  }

  #[cfg(target_os = "windows")]
  fn now_millis() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|d| d.as_millis() as u64)
      .unwrap_or(0)
  }

  #[cfg(target_os = "windows")]
  fn run_window_loop(
    stop_flag: Arc<AtomicBool>,
    hwnd_slot: Arc<Mutex<Option<isize>>>,
    on_usb_change: Arc<dyn Fn() + Send + Sync>,
    on_razer_key: Arc<dyn Fn(RawKeyEvent) + Send + Sync>,
  ) {
    unsafe {
      const GUID_DEVINTERFACE_HID: GUID = GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030);

      let class_name = w!("XToolsNagaMonitorWindow");
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

      let ctx = Box::new(WindowContext {
        on_usb_change,
        on_razer_key,
        device_cache: Mutex::new(HashMap::new()),
      });
      let ctx_ptr = Box::into_raw(ctx);

      let Ok(hwnd) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class_name,
        w!("XToolsNagaMonitor"),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        None,
        None,
        Some(hinstance),
        Some(ctx_ptr as *const std::ffi::c_void),
      ) else {
        let _ = Box::from_raw(ctx_ptr);
        LOG.error("Failed to create hidden Win32 Raw Input window");
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

      let rid = [RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x06,
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
      }];
      if let Err(err) = RegisterRawInputDevices(&rid, std::mem::size_of::<RAWINPUTDEVICE>() as u32) {
        LOG.warn(&format!("RegisterRawInputDevices failed: {}", err));
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

      if let Some(notify) = dev_notify {
        let _ = UnregisterDeviceNotification(notify);
      }
      *hwnd_slot.lock().unwrap() = None;
      let _ = DestroyWindow(hwnd);
      let _ = Box::from_raw(ctx_ptr);
    }
  }

  #[cfg(target_os = "windows")]
  unsafe fn is_razer_raw_device(ctx: &WindowContext, hdevice: HANDLE) -> bool {
    if hdevice.is_invalid() {
      return false;
    }
    let key = hdevice.0 as isize;
    if let Some(&cached) = ctx.device_cache.lock().unwrap().get(&key) {
      return cached;
    }

    let mut char_count: u32 = 0;
    let header_res = GetRawInputDeviceInfoW(Some(hdevice), RIDI_DEVICENAME, None, &mut char_count);
    if header_res == u32::MAX || char_count == 0 {
      return false;
    }

    let mut name_buf = vec![0u16; char_count as usize];
    let read_res = GetRawInputDeviceInfoW(
      Some(hdevice),
      RIDI_DEVICENAME,
      Some(name_buf.as_mut_ptr() as *mut std::ffi::c_void),
      &mut char_count,
    );
    let is_razer = if read_res != u32::MAX {
      let nul = name_buf.iter().position(|&c| c == 0).unwrap_or(name_buf.len());
      let lower = String::from_utf16_lossy(&name_buf[..nul]).to_ascii_lowercase();
      let wired_vid = format!("vid_{:04x}", NagaModel::VENDOR_ID);
      let bt_vid = format!("vid_{:04x}", NagaModel::BT_VENDOR_ID);
      lower.contains(&wired_vid) || lower.contains(&bt_vid)
    } else {
      false
    };

    ctx.device_cache.lock().unwrap().insert(key, is_razer);
    is_razer
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
      WM_INPUT => {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowContext;
        if !ptr.is_null() {
          let ctx = &*ptr;
          let hraw = HRAWINPUT(lparam.0 as *mut std::ffi::c_void);
          let mut size: u32 = 0;
          let hdr_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
          if GetRawInputData(hraw, RID_INPUT, None, &mut size, hdr_size) != u32::MAX && size > 0 {
            let mut buf = vec![0u8; size as usize];
            let got = GetRawInputData(
              hraw,
              RID_INPUT,
              Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
              &mut size,
              hdr_size,
            );
            if got != u32::MAX && (got as usize) >= std::mem::size_of::<RAWINPUTHEADER>() {
              let raw = &*(buf.as_ptr() as *const RAWINPUT);
              if raw.header.dwType == RIM_TYPEKEYBOARD.0
                && Self::is_razer_raw_device(ctx, raw.header.hDevice)
              {
                let kb = raw.data.keyboard;
                const RI_KEY_BREAK_FLAG: u16 = 0x0001;
                let is_down = (kb.Flags & RI_KEY_BREAK_FLAG) == 0;
                let vk = kb.VKey;
                if is_down && vk != 0 && vk != 0xFF && !VkHelper::is_modifier_vk(vk) {
                  let ctrl = KeySynthesizer::is_key_down(VK_CONTROL.0);
                  let alt = KeySynthesizer::is_key_down(VK_MENU.0);
                  let shift = KeySynthesizer::is_key_down(VK_SHIFT.0);
                  let win =
                    KeySynthesizer::is_key_down(VK_LWIN.0) || KeySynthesizer::is_key_down(VK_RWIN.0);
                  (ctx.on_razer_key)(RawKeyEvent {
                    vk,
                    ctrl,
                    alt,
                    shift,
                    win,
                    timestamp_ms: Self::now_millis(),
                  });
                }
              }
            }
          }
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
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
            if name.to_ascii_lowercase().contains("vid_1532") {
              let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowContext;
              if !ptr.is_null() {
                let ctx = &*ptr;
                ctx.device_cache.lock().unwrap().clear();
                (ctx.on_usb_change)();
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

impl Drop for NagaRawInputMonitor {
  fn drop(&mut self) {
    self.stop();
  }
}
