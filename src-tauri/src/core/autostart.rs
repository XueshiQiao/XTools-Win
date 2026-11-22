use super::file_log::FileLog;

#[cfg(target_os = "windows")]
use windows::{
  core::w,
  Win32::Foundation::ERROR_SUCCESS,
  Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
  },
};

const LOG: FileLog = FileLog::new("AutoStart");
const VALUE_NAME: &str = "XTools";

/// Manages Windows launch-at-login via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
pub struct AutoStartManager;

impl AutoStartManager {
  pub fn new() -> Self {
    Self
  }

  pub fn is_enabled(&self) -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
      let mut hkey = HKEY::default();
      let status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!(r"Software\Microsoft\Windows\CurrentVersion\Run"),
        None,
        KEY_READ,
        &mut hkey,
      );
      if status != ERROR_SUCCESS {
        return false;
      }
      let value_wide: Vec<u16> = VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
      let mut data_len: u32 = 0;
      let query_status = RegQueryValueExW(
        hkey,
        windows::core::PCWSTR(value_wide.as_ptr()),
        None,
        None,
        None,
        Some(&mut data_len),
      );
      let _ = RegCloseKey(hkey);
      query_status == ERROR_SUCCESS && data_len > 0
    }
    #[cfg(not(target_os = "windows"))]
    {
      false
    }
  }

  pub fn set_enabled(&self, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    unsafe {
      let mut hkey = HKEY::default();
      let status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!(r"Software\Microsoft\Windows\CurrentVersion\Run"),
        None,
        KEY_WRITE,
        &mut hkey,
      );
      if status != ERROR_SUCCESS {
        return Err(format!("Failed to open Run registry key: {:?}", status));
      }

      let value_wide: Vec<u16> = VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
      let result = if enabled {
        let exe = std::env::current_exe()
          .map_err(|e| format!("Failed to locate executable: {}", e))?;
        let command = format!("\"{}\" --minimized", exe.display());
        let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
        let err = RegSetValueExW(
          hkey,
          windows::core::PCWSTR(value_wide.as_ptr()),
          None,
          REG_SZ,
          Some(bytes),
        );
        if err == ERROR_SUCCESS {
          LOG.info(&format!("Enabled launch at login: {}", command));
          Ok(())
        } else {
          Err(format!("Failed to write Run registry value: {:?}", err))
        }
      } else {
        let err = RegDeleteValueW(hkey, windows::core::PCWSTR(value_wide.as_ptr()));
        if err == ERROR_SUCCESS || err.0 == 2 {
          LOG.info("Disabled launch at login");
          Ok(())
        } else {
          Err(format!("Failed to delete Run registry value: {:?}", err))
        }
      };

      let _ = RegCloseKey(hkey);
      result
    }
    #[cfg(not(target_os = "windows"))]
    {
      let _ = enabled;
      Ok(())
    }
  }
}
