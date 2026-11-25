use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use super::file_log::FileLog;
use super::vk::{KeyShortcut, KeySynthesizer, VkHelper, SYNTHETIC_EXTRA_INFO};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::GetCurrentThreadId;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
  VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
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

pub enum RecordRequest {
  Shortcut(Box<dyn FnOnce(Option<KeyShortcut>, bool) + Send>),
  SingleKey(Box<dyn FnOnce(Option<u16>) + Send>),
}

type KeyHandler = Arc<dyn Fn(&LowLevelKeyEvent) -> HookDecision + Send + Sync>;

static HUB_INSTANCE: OnceLock<Arc<KeyboardHookHub>> = OnceLock::new();

pub struct KeyboardHookHub {
  naga_handler: RwLock<Option<KeyHandler>>,
  hyper_handler: RwLock<Option<KeyHandler>>,
  recorder: Mutex<Option<RecordRequest>>,
  swallowed_record_vk: AtomicU16,
  started: AtomicBool,
  thread_id: AtomicU32,
  join_handle: Mutex<Option<JoinHandle<()>>>,
}

impl KeyboardHookHub {
  pub fn new_shared() -> Arc<Self> {
    let hub = Arc::new(Self {
      naga_handler: RwLock::new(None),
      hyper_handler: RwLock::new(None),
      recorder: Mutex::new(None),
      swallowed_record_vk: AtomicU16::new(0),
      started: AtomicBool::new(false),
      thread_id: AtomicU32::new(0),
      join_handle: Mutex::new(None),
    });
    let _ = HUB_INSTANCE.set(Arc::clone(&hub));
    hub
  }

  pub fn set_naga_handler<F>(&self, handler: F)
  where
    F: Fn(&LowLevelKeyEvent) -> HookDecision + Send + Sync + 'static,
  {
    *self.naga_handler.write().unwrap() = Some(Arc::new(handler));
  }

  pub fn clear_naga_handler(&self) {
    *self.naga_handler.write().unwrap() = None;
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

  pub fn begin_recording(&self, request: RecordRequest) {
    self.swallowed_record_vk.store(0, Ordering::SeqCst);
    *self.recorder.lock().unwrap() = Some(request);
  }

  pub fn cancel_recording(&self) {
    *self.recorder.lock().unwrap() = None;
    self.swallowed_record_vk.store(0, Ordering::SeqCst);
  }

  pub fn start(&self) {
    if self.started.swap(true, Ordering::SeqCst) {
      return;
    }

    #[cfg(target_os = "windows")]
    {
      let handle = thread::Builder::new()
        .name("xtools-kbd-hook".to_string())
        .spawn(|| {
          Self::run_hook_loop();
        })
        .expect("failed to spawn xtools-kbd-hook thread");
      *self.join_handle.lock().unwrap() = Some(handle);
    }
  }

  pub fn stop(&self) {
    if !self.started.swap(false, Ordering::SeqCst) {
      return;
    }
    self.cancel_recording();

    #[cfg(target_os = "windows")]
    {
      let tid = self.thread_id.load(Ordering::SeqCst);
      if tid != 0 {
        unsafe {
          let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
      }
      if let Some(handle) = self.join_handle.lock().unwrap().take() {
        let _ = handle.join();
      }
    }
    LOG.info("unified keyboard hook stopped");
  }

  fn now_millis() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|d| d.as_millis() as u64)
      .unwrap_or(0)
  }

  #[cfg(target_os = "windows")]
  fn run_hook_loop() {
    unsafe {
      let tid = GetCurrentThreadId();
      if let Some(hub) = HUB_INSTANCE.get() {
        hub.thread_id.store(tid, Ordering::SeqCst);
      }

      let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(Self::hook_proc), None, 0) {
        Ok(h) => h,
        Err(err) => {
          LOG.error(&format!("SetWindowsHookExW failed: {}", err));
          return;
        }
      };

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

    if is_up && hub.swallowed_record_vk.load(Ordering::SeqCst) == vk {
      hub.swallowed_record_vk.store(0, Ordering::SeqCst);
      return LRESULT(1);
    }

    if let Some(decision) = hub.try_handle_recording(vk, is_down, is_up) {
      return match decision {
        HookDecision::Swallow => LRESULT(1),
        HookDecision::Pass => CallNextHookEx(None, code, wparam, lparam),
      };
    }

    let ev = LowLevelKeyEvent {
      vk,
      scan_code: kbd.scanCode,
      is_down,
      is_up,
      is_extended: (kbd.flags.0 & 0x01) != 0,
      timestamp_ms: Self::now_millis(),
    };

    if let Some(handler) = hub.naga_handler.read().unwrap().as_ref().cloned() {
      if handler(&ev) == HookDecision::Swallow {
        return LRESULT(1);
      }
    }

    if let Some(handler) = hub.hyper_handler.read().unwrap().as_ref().cloned() {
      if handler(&ev) == HookDecision::Swallow {
        return LRESULT(1);
      }
    }

    CallNextHookEx(None, code, wparam, lparam)
  }

  #[cfg(target_os = "windows")]
  fn try_handle_recording(&self, vk: u16, is_down: bool, is_up: bool) -> Option<HookDecision> {
    let mut guard = self.recorder.lock().unwrap();
    if guard.is_none() {
      return None;
    }

    // Pass modifier transitions through so GetAsyncKeyState stays accurate.
    if VkHelper::is_modifier_vk(vk) {
      return Some(HookDecision::Pass);
    }

    // Ignore CapsLock during shortcut/sentinel capture so user doesn't bind Caps by accident.
    if vk == VK_CAPITAL.0 {
      return Some(HookDecision::Pass);
    }

    if !is_down {
      return Some(if is_up {
        HookDecision::Pass
      } else {
        HookDecision::Swallow
      });
    }

    let request = guard.take()?;
    self.swallowed_record_vk.store(vk, Ordering::SeqCst);
    drop(guard);

    let ctrl = KeySynthesizer::is_key_down(VK_CONTROL.0);
    let alt = KeySynthesizer::is_key_down(VK_MENU.0);
    let shift = KeySynthesizer::is_key_down(VK_SHIFT.0);
    let win = KeySynthesizer::is_key_down(VK_LWIN.0) || KeySynthesizer::is_key_down(VK_RWIN.0);

    thread::spawn(move || match request {
      RecordRequest::Shortcut(cb) => {
        if vk == VK_ESCAPE.0 && !ctrl && !alt && !shift && !win {
          cb(None, false);
        } else if (vk == VK_BACK.0 || vk == VK_DELETE.0) && !ctrl && !alt && !shift && !win {
          cb(None, true);
        } else {
          cb(
            Some(KeyShortcut {
              vk,
              ctrl,
              alt,
              shift,
              win,
            }),
            false,
          );
        }
      }
      RecordRequest::SingleKey(cb) => {
        if vk == VK_ESCAPE.0 {
          cb(None);
        } else {
          cb(Some(vk));
        }
      }
    });

    Some(HookDecision::Swallow)
  }
}

impl Drop for KeyboardHookHub {
  fn drop(&mut self) {
    self.stop();
  }
}
