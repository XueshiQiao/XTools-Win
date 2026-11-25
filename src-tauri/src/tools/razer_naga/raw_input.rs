use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

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

const LOG: FileLog = FileLog::new("NagaDeviceMonitor");

#[cfg(target_os = "windows")]
struct WindowContext {
  on_usb_change: Arc<dyn Fn() + Send + Sync>,
}

/// Lightweight hidden Win32 window listening solely for Razer USB arrival/removal (`WM_DEVICECHANGE`).
pub struct NagaDeviceMonitor {
  stop_flag: Arc<AtomicBool>,
  hwnd: Arc<Mutex<Option<isize>>>,
  thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl NagaDeviceMonitor {
  pub fn new() -> Self {
    Self {
      stop_flag: Arc::new(AtomicBool::new(false)),
      hwnd: Arc::new(Mutex::new(None)),
      thread_handle: Mutex::new(None),
    }
  }

  pub fn start<F>(&self, on_usb_change: F)
  where
    F: Fn() + Send + Sync + 'static,
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

      let handle = thread::Builder::new()
        .name("naga-usb-monitor".to_string())
        .spawn(move || {
          Self::run_window_loop(stop, hwnd_slot, usb_cb);
        })
        .expect("failed to spawn naga-usb-monitor thread");
      *slot = Some(handle);
    }

    #[cfg(not(target_os = "windows"))]
    let _ = on_usb_change;
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

  #[cfg(target_os = "windows")]
  fn run_window_loop(
    stop_flag: Arc<AtomicBool>,
    hwnd_slot: Arc<Mutex<Option<isize>>>,
    on_usb_change: Arc<dyn Fn() + Send + Sync>,
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

      let ctx = Box::new(WindowContext { on_usb_change });
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
        LOG.error("Failed to create hidden Win32 USB monitor window");
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
      let _ = Box::from_raw(ctx_ptr);
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
            if name.to_ascii_lowercase().contains("vid_1532") {
              let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowContext;
              if !ptr.is_null() {
                let ctx = &*ptr;
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

impl Drop for NagaDeviceMonitor {
  fn drop(&mut self) {
    self.stop();
  }
}
