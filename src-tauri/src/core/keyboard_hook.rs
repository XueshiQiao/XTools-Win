use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::file_log::FileLog;
use super::vk::SYNTHETIC_EXTRA_INFO;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::GetCurrentThreadId;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
  TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
  WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const LOG: FileLog = FileLog::new("KeyboardHookHub");

#[derive(Debug, Clone, Copy)]
pub struct LowLevelKeyEvent {
  pub vk: u16,
  pub scan_code: u32,
  pub is_down: bool,
  pub is_up: bool,
  pub is_extended: bool,
  pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDecision {
  Pass,
  Swallow,
}

type KeyHandler = Arc<dyn Fn(&LowLevelKeyEvent) -> HookDecision + Send + Sync>;

static HUB_INSTANCE: OnceLock<Arc<KeyboardHookHub>> = OnceLock::new();

pub struct KeyboardHookHub {
  hyper_handler: RwLock<Option<KeyHandler>>,
  started: AtomicBool,
  thread_id: AtomicU32,
  join_handle: Mutex<Option<JoinHandle<()>>>,
  lifecycle_lock: Mutex<()>,
}

impl KeyboardHookHub {
  pub fn new_shared() -> Arc<Self> {
    let hub = Arc::new(Self {
      hyper_handler: RwLock::new(None),
      started: AtomicBool::new(false),
      thread_id: AtomicU32::new(0),
      join_handle: Mutex::new(None),
      lifecycle_lock: Mutex::new(()),
    });
    let _ = HUB_INSTANCE.set(Arc::clone(&hub));
    hub
  }

  pub fn set_hyper_handler<F>(&self, handler: F)
  where
    F: Fn(&LowLevelKeyEvent) -> HookDecision + Send + Sync + 'static,
  {
    *self.hyper_handler.write().unwrap() = Some(Arc::new(handler));
  }

  pub fn clear_hyper_handler(&self) {
    *self.hyper_handler.write().unwrap() = None;
  }

  pub fn is_running(&self) -> bool {
    self.started.load(Ordering::SeqCst)
  }

  pub fn start(&self) {
    let _guard = self.lifecycle_lock.lock().unwrap();
    if self.started.swap(true, Ordering::SeqCst) {
      return;
    }

    #[cfg(target_os = "windows")]
    {
      let (ready_tx, ready_rx) = mpsc::channel::<u32>();
      let handle = thread::Builder::new()
        .name("xtools-kbd-hook".to_string())
        .spawn(move || {
          Self::run_hook_loop(ready_tx);
        })
        .expect("failed to spawn xtools-kbd-hook thread");
      *self.join_handle.lock().unwrap() = Some(handle);

      match ready_rx.recv_timeout(Duration::from_millis(500)) {
        Ok(0) => {
          if let Some(h) = self.join_handle.lock().unwrap().take() {
            let _ = h.join();
          }
          self.started.store(false, Ordering::SeqCst);
        }
        Ok(_) | Err(_) => {}
      }
    }
  }

  pub fn stop(&self) {
    let _guard = self.lifecycle_lock.lock().unwrap();
    if !self.started.swap(false, Ordering::SeqCst) {
      return;
    }

    #[cfg(target_os = "windows")]
    {
      let tid = self.thread_id.swap(0, Ordering::SeqCst);
      if tid != 0 {
        unsafe {
          let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
      }
      if let Some(handle) = self.join_handle.lock().unwrap().take() {
        let _ = handle.join();
      }
    }
    LOG.info("unified WH_KEYBOARD_LL hook stopped");
  }

  fn now_millis() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|d| d.as_millis() as u64)
      .unwrap_or(0)
  }

  #[cfg(target_os = "windows")]
  fn run_hook_loop(ready_tx: mpsc::Sender<u32>) {
    unsafe {
      let tid = GetCurrentThreadId();
      let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(Self::hook_proc), None, 0) {
        Ok(h) => h,
        Err(err) => {
          LOG.error(&format!("SetWindowsHookExW failed: {}", err));
          if let Some(hub) = HUB_INSTANCE.get() {
            hub.started.store(false, Ordering::SeqCst);
          }
          let _ = ready_tx.send(0);
          return;
        }
      };

      if let Some(hub) = HUB_INSTANCE.get() {
        hub.thread_id.store(tid, Ordering::SeqCst);
      }
      let _ = ready_tx.send(tid);
      LOG.info("unified WH_KEYBOARD_LL hook installed");

      let mut msg = MSG::default();
      loop {
        let ret = GetMessageW(&mut msg, None, 0, 0);
        if ret.0 <= 0 {
          break;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
      }

      let _ = UnhookWindowsHookEx(hook);
      if let Some(hub) = HUB_INSTANCE.get() {
        hub.thread_id.store(0, Ordering::SeqCst);
        hub.started.store(false, Ordering::SeqCst);
      }
    }
  }

  #[cfg(target_os = "windows")]
  unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
      return CallNextHookEx(None, code, wparam, lparam);
    }

    let kbd = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let is_injected = (kbd.flags.0 & 0x10) != 0;
    if is_injected || kbd.dwExtraInfo == SYNTHETIC_EXTRA_INFO {
      return CallNextHookEx(None, code, wparam, lparam);
    }

    let msg = wparam.0 as u32;
    let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
    let vk = kbd.vkCode as u16;

    let Some(hub) = HUB_INSTANCE.get() else {
      return CallNextHookEx(None, code, wparam, lparam);
    };

    let ev = LowLevelKeyEvent {
      vk,
      scan_code: kbd.scanCode,
      is_down,
      is_up,
      is_extended: (kbd.flags.0 & 0x01) != 0,
      timestamp_ms: Self::now_millis(),
    };

    if let Some(handler) = hub.hyper_handler.read().unwrap().as_ref().cloned() {
      if handler(&ev) == HookDecision::Swallow {
        return LRESULT(1);
      }
    }

    CallNextHookEx(None, code, wparam, lparam)
  }
}

impl Drop for KeyboardHookHub {
  fn drop(&mut self) {
    self.stop();
  }
}
