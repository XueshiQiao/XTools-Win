use std::fs;
use std::path::PathBuf;

use super::file_log::FileLog;

#[cfg(target_os = "windows")]
use windows::{
  core::{w, HSTRING, PCWSTR},
  Data::Xml::Dom::XmlDocument,
  UI::Notifications::{NotificationSetting, ToastNotification, ToastNotificationManager},
  Win32::Foundation::ERROR_SUCCESS,
  Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
  Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
  },
  Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID,
};

const LOG: FileLog = FileLog::new("Notifier");
const AUMID: &str = "me.xueshi.xtools";
const APP_LOGO_BYTES: &[u8] = include_bytes!("../../icons/app_logo.png");

/// Sends native Windows Toast notifications via WinRT (`ToastNotificationManager`),
/// matching macOS XTools' `ROGNotifier`.
pub struct WindowsNotifier;

impl WindowsNotifier {
  pub fn new() -> Self {
    Self
  }

  /// Registers our AUMID in HKCU and associates it with the current process so
  /// WinRT toasts display with XTools' name & icon even when launched unbundled
  /// or in portable mode.
  pub fn init() {
    #[cfg(target_os = "windows")]
    unsafe {
      let _ = SetCurrentProcessExplicitAppUserModelID(w!("me.xueshi.xtools"));

      let icon_path = Self::ensure_toast_icon();
      let mut hkey = HKEY::default();
      let status = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        w!(r"Software\Classes\AppUserModelId\me.xueshi.xtools"),
        None,
        PCWSTR::null(),
        REG_OPTION_NON_VOLATILE,
        KEY_WRITE,
        None,
        &mut hkey,
        None,
      );
      if status == ERROR_SUCCESS {
        Self::set_reg_sz(hkey, "DisplayName", "XTools");
        if let Some(path) = icon_path {
          let path_str = path.to_string_lossy();
          Self::set_reg_sz(hkey, "IconUri", &path_str);
        }
        let _ = RegCloseKey(hkey);
      } else {
        LOG.warn(&format!("Failed to register AUMID in HKCU: {:?}", status));
      }
    }
  }

  pub fn is_authorized(&self) -> bool {
    #[cfg(target_os = "windows")]
    {
      Self::ensure_com();
      let aumid = HSTRING::from(AUMID);
      let Ok(notifier) = ToastNotificationManager::CreateToastNotifierWithId(&aumid) else {
        return true;
      };
      match notifier.Setting() {
        Ok(setting) => setting == NotificationSetting::Enabled,
        Err(_) => true,
      }
    }
    #[cfg(not(target_os = "windows"))]
    {
      true
    }
  }

  pub fn post(&self, title: &str, body: &str, play_sound: bool) {
    LOG.info(&format!("notify: {} — {}", title, body));
    #[cfg(target_os = "windows")]
    {
      Self::ensure_com();
      let audio_tag = if play_sound {
        ""
      } else {
        r#"<audio silent="true"/>"#
      };
      let xml_text = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{}</toast>"#,
        Self::escape_xml(title),
        Self::escape_xml(body),
        audio_tag
      );
      let Ok(doc) = XmlDocument::new() else {
        LOG.error("Failed to create XmlDocument for Toast");
        return;
      };
      if let Err(err) = doc.LoadXml(&HSTRING::from(xml_text)) {
        LOG.error(&format!("Failed to load Toast XML: {}", err));
        return;
      }
      let toast = match ToastNotification::CreateToastNotification(&doc) {
        Ok(t) => t,
        Err(err) => {
          LOG.error(&format!("Failed to create ToastNotification: {}", err));
          return;
        }
      };
      let aumid = HSTRING::from(AUMID);
      match ToastNotificationManager::CreateToastNotifierWithId(&aumid) {
        Ok(notifier) => {
          if let Err(err) = notifier.Show(&toast) {
            LOG.error(&format!("Toast delivery failed: {}", err));
          }
        }
        Err(err) => {
          LOG.error(&format!("CreateToastNotifierWithId failed: {}", err));
        }
      }
    }
  }

  fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
      match ch {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&apos;"),
        _ => out.push(ch),
      }
    }
    out
  }

  #[cfg(target_os = "windows")]
  fn ensure_com() {
    unsafe {
      let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
  }

  #[cfg(target_os = "windows")]
  unsafe fn set_reg_sz(hkey: HKEY, name: &str, value: &str) {
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let val_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = std::slice::from_raw_parts(val_w.as_ptr() as *const u8, val_w.len() * 2);
    let _ = RegSetValueExW(
      hkey,
      PCWSTR(name_w.as_ptr()),
      None,
      REG_SZ,
      Some(bytes),
    );
  }

  fn ensure_toast_icon() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("XTools");
    let _ = fs::create_dir_all(&dir);
    let icon_path = dir.join("toast_icon.png");
    if !icon_path.exists() {
      if fs::write(&icon_path, APP_LOGO_BYTES).is_err() {
        return None;
      }
    }
    Some(icon_path)
  }
}
