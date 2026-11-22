use std::thread;
use std::time::{Duration, Instant};

use super::protocol::{RogBatteryInfo, RogCommand, RogDeviceInfo, RogLink, RogModel, RogProfile};
use crate::core::file_log::FileLog;

#[cfg(target_os = "windows")]
use windows::{
  core::GUID,
  Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
  },
  Win32::Devices::HumanInterfaceDevice::{
    HidD_FlushQueue, HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetHidGuid,
    HidD_GetPreparsedData, HidP_GetCaps, HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS,
    PHIDP_PREPARSED_DATA,
  },
  Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_IO_PENDING, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
  },
  Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
  },
  Win32::System::Threading::{CreateEventW, WaitForSingleObject},
  Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
};

const LOG: FileLog = FileLog::new("ROGHIDDevice");
const REJECTION_PREFIX: [u8; 2] = [0xFF, 0xAA];

#[derive(Debug, Clone)]
pub enum RogError {
  NotFound,
  OpenFailed(u32),
  WriteFailed(u32),
  ReadFailed(u32),
  TimedOut(String),
  Rejected(String),
  MalformedReply(String),
  InvalidProfile(usize),
}

impl RogError {
  pub fn message(&self, is_zh: bool) -> String {
    match self {
      Self::NotFound => {
        if is_zh {
          "找不到键盘的控制通道——它多半被切到了 Mac、蓝牙，或线拔了。".to_string()
        } else {
          "Could not find the keyboard's control channel — it may be switched to Mac, Bluetooth, or unplugged.".to_string()
        }
      }
      Self::OpenFailed(code) => {
        if is_zh {
          format!("打不开键盘（Win32 0x{:08X}）。", code)
        } else {
          format!("Failed to open the keyboard (Win32 0x{:08X}).", code)
        }
      }
      Self::WriteFailed(code) => {
        if is_zh {
          format!("写入键盘失败（Win32 0x{:08X}）。", code)
        } else {
          format!("Failed to write to the keyboard (Win32 0x{:08X}).", code)
        }
      }
      Self::ReadFailed(code) => {
        if is_zh {
          format!("读取键盘回应失败（Win32 0x{:08X}）。", code)
        } else {
          format!("Failed to read keyboard reply (Win32 0x{:08X}).", code)
        }
      }
      Self::TimedOut(cmd) => {
        if is_zh {
          format!("键盘没有回应命令 [{}]。", cmd)
        } else {
          format!("The keyboard did not respond to [{}].", cmd)
        }
      }
      Self::Rejected(cmd) => {
        if is_zh {
          format!("键盘拒绝了命令 [{}]。", cmd)
        } else {
          format!("The keyboard rejected [{}].", cmd)
        }
      }
      Self::MalformedReply(cmd) => {
        if is_zh {
          format!("命令 [{}] 的回应格式不对。", cmd)
        } else {
          format!("Unexpected reply format for [{}].", cmd)
        }
      }
      Self::InvalidProfile(slot) => {
        if is_zh {
          format!("这把键盘上没有这个配置文件槽位（{}）。", slot)
        } else {
          format!("This keyboard does not have profile slot {}.", slot)
        }
      }
    }
  }
}

#[derive(Debug, Clone)]
pub struct RogHidDeviceInfo {
  pub path: String,
  pub product_id: u16,
  pub input_report_len: usize,
  pub output_report_len: usize,
}

#[cfg(target_os = "windows")]
struct OwnedHandle(HANDLE);

#[cfg(target_os = "windows")]
impl OwnedHandle {
  fn is_valid(h: HANDLE) -> bool {
    !h.is_invalid() && h != INVALID_HANDLE_VALUE
  }
}

#[cfg(target_os = "windows")]
impl Drop for OwnedHandle {
  fn drop(&mut self) {
    if Self::is_valid(self.0) {
      unsafe {
        let _ = CloseHandle(self.0);
      }
    }
  }
}

pub struct RogHidDevice {
  #[allow(dead_code)]
  pub product_id: u16,
  pub input_report_len: usize,
  pub output_report_len: usize,
  #[cfg(target_os = "windows")]
  handle: OwnedHandle,
}

impl RogHidDevice {
  pub fn discover(product_ids: &[u16]) -> Vec<RogHidDeviceInfo> {
    #[cfg(target_os = "windows")]
    unsafe {
      let mut found = Vec::new();
      let hid_guid: GUID = HidD_GetHidGuid();
      let Ok(dev_info) = SetupDiGetClassDevsW(
        Some(&hid_guid),
        windows::core::PCWSTR::null(),
        None,
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
      ) else {
        return found;
      };

      let mut index: u32 = 0;
      loop {
        let mut if_data = SP_DEVICE_INTERFACE_DATA {
          cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
          ..Default::default()
        };
        if SetupDiEnumDeviceInterfaces(dev_info, None, &hid_guid, index, &mut if_data).is_err() {
          break;
        }
        index += 1;

        let mut required_size: u32 = 0;
        let _ = SetupDiGetDeviceInterfaceDetailW(
          dev_info,
          &if_data,
          None,
          0,
          Some(&mut required_size),
          None,
        );
        if required_size < std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 {
          continue;
        }

        let mut buf = vec![0u8; required_size as usize];
        let detail_ptr = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
        (*detail_ptr).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;

        if SetupDiGetDeviceInterfaceDetailW(
          dev_info,
          &if_data,
          Some(detail_ptr),
          required_size,
          None,
          None,
        )
        .is_err()
        {
          continue;
        }

        let path = Self::read_device_path(detail_ptr, required_size as usize);
        if !path.to_ascii_lowercase().contains("vid_0b05") {
          continue;
        }

        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let Ok(meta_raw) = CreateFileW(
          windows::core::PCWSTR(wide.as_ptr()),
          0,
          FILE_SHARE_READ | FILE_SHARE_WRITE,
          None,
          OPEN_EXISTING,
          FILE_FLAGS_AND_ATTRIBUTES(0),
          None,
        ) else {
          continue;
        };
        let meta_handle = OwnedHandle(meta_raw);

        let mut attrs = HIDD_ATTRIBUTES {
          Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32,
          ..Default::default()
        };
        if !HidD_GetAttributes(meta_handle.0, &mut attrs) {
          continue;
        }
        if attrs.VendorID != RogModel::VENDOR_ID || !product_ids.contains(&attrs.ProductID) {
          continue;
        }

        let mut preparsed = PHIDP_PREPARSED_DATA::default();
        if !HidD_GetPreparsedData(meta_handle.0, &mut preparsed) {
          continue;
        }
        let mut caps = HIDP_CAPS::default();
        let caps_status = HidP_GetCaps(preparsed, &mut caps);
        let _ = HidD_FreePreparsedData(preparsed);

        if caps_status != HIDP_STATUS_SUCCESS {
          continue;
        }
        if caps.UsagePage != RogModel::CONTROL_USAGE_PAGE
          || caps.Usage != RogModel::CONTROL_USAGE
          || caps.OutputReportByteLength == 0
          || caps.InputReportByteLength == 0
        {
          continue;
        }

        found.push(RogHidDeviceInfo {
          path,
          product_id: attrs.ProductID,
          input_report_len: usize::from(caps.InputReportByteLength),
          output_report_len: usize::from(caps.OutputReportByteLength),
        });
      }

      let _ = SetupDiDestroyDeviceInfoList(dev_info);
      // Prefer wired PID over wireless dongle if both are simultaneously attached.
      found.sort_by_key(|d| if d.product_id == RogModel::PID_WIRED { 0u8 } else { 1u8 });
      found
    }

    #[cfg(not(target_os = "windows"))]
    {
      let _ = product_ids;
      Vec::new()
    }
  }

  #[cfg(target_os = "windows")]
  unsafe fn read_device_path(
    detail_ptr: *const SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    buf_len: usize,
  ) -> String {
    let base_addr = detail_ptr as usize;
    let path_addr = (&(*detail_ptr).DevicePath) as *const u16 as usize;
    let offset = path_addr.saturating_sub(base_addr);
    let max_u16 = buf_len.saturating_sub(offset) / 2;
    let slice = std::slice::from_raw_parts((*detail_ptr).DevicePath.as_ptr(), max_u16);
    let nul_pos = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..nul_pos])
  }

  pub fn open(info: &RogHidDeviceInfo) -> Result<Self, RogError> {
    #[cfg(target_os = "windows")]
    unsafe {
      // GENERIC_READ (0x80000000) | GENERIC_WRITE (0x40000000)
      const GENERIC_RW: u32 = 0x8000_0000 | 0x4000_0000;
      let wide: Vec<u16> = info.path.encode_utf16().chain(std::iter::once(0)).collect();
      let handle = CreateFileW(
        windows::core::PCWSTR(wide.as_ptr()),
        GENERIC_RW,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED,
        None,
      )
      .map_err(|e| RogError::OpenFailed(e.code().0 as u32))?;

      if !OwnedHandle::is_valid(handle) {
        return Err(RogError::OpenFailed(GetLastError().0));
      }

      Ok(Self {
        product_id: info.product_id,
        input_report_len: info.input_report_len,
        output_report_len: info.output_report_len,
        handle: OwnedHandle(handle),
      })
    }

    #[cfg(not(target_os = "windows"))]
    {
      let _ = info;
      Err(RogError::NotFound)
    }
  }

  pub fn open_first(product_ids: &[u16]) -> Result<Self, RogError> {
    let candidates = Self::discover(product_ids);
    let first = candidates.first().ok_or(RogError::NotFound)?;
    Self::open(first)
  }

  pub fn send(&self, command: &RogCommand, timeout: Duration) -> Result<(), RogError> {
    #[cfg(target_os = "windows")]
    unsafe {
      let report = command.windows_report(self.output_report_len);
      let event = OwnedHandle(
        CreateEventW(None, true, false, windows::core::PCWSTR::null())
          .map_err(|e| RogError::WriteFailed(e.code().0 as u32))?,
      );
      let mut overlapped = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
      };
      let mut written: u32 = 0;

      if let Err(err) = WriteFile(
        self.handle.0,
        Some(&report),
        Some(&mut written),
        Some(&mut overlapped),
      ) {
        let last = GetLastError();
        if last != ERROR_IO_PENDING {
          return Err(RogError::WriteFailed(err.code().0 as u32));
        }
        let wait_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        if WaitForSingleObject(event.0, wait_ms) != WAIT_OBJECT_0 {
          let _ = CancelIoEx(self.handle.0, Some(&overlapped));
          let _ = GetOverlappedResult(self.handle.0, &overlapped, &mut written, true);
          return Err(RogError::TimedOut(command.hex_description()));
        }
        if let Err(res_err) = GetOverlappedResult(self.handle.0, &overlapped, &mut written, true) {
          return Err(RogError::WriteFailed(res_err.code().0 as u32));
        }
      }
      Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
      let _ = (command, timeout);
      Err(RogError::NotFound)
    }
  }

  #[cfg(target_os = "windows")]
  fn read_one_report(&self, timeout: Duration, cmd_desc: &str) -> Result<Vec<u8>, RogError> {
    unsafe {
      let mut buf = vec![0u8; self.input_report_len.max(65)];
      let event = OwnedHandle(
        CreateEventW(None, true, false, windows::core::PCWSTR::null())
          .map_err(|e| RogError::ReadFailed(e.code().0 as u32))?,
      );
      let mut overlapped = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
      };
      let mut read_len: u32 = 0;

      if let Err(err) = ReadFile(
        self.handle.0,
        Some(&mut buf),
        Some(&mut read_len),
        Some(&mut overlapped),
      ) {
        let last = GetLastError();
        if last != ERROR_IO_PENDING {
          return Err(RogError::ReadFailed(err.code().0 as u32));
        }
        let wait_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        if WaitForSingleObject(event.0, wait_ms) != WAIT_OBJECT_0 {
          let _ = CancelIoEx(self.handle.0, Some(&overlapped));
          let _ = GetOverlappedResult(self.handle.0, &overlapped, &mut read_len, true);
          return Err(RogError::TimedOut(cmd_desc.to_string()));
        }
        if let Err(res_err) = GetOverlappedResult(self.handle.0, &overlapped, &mut read_len, true) {
          return Err(RogError::ReadFailed(res_err.code().0 as u32));
        }
      }

      let count = (read_len as usize).min(buf.len());
      if count <= 1 {
        return Ok(Vec::new());
      }
      // Strip leading Windows HID Report ID byte (0x00).
      Ok(buf[1..count].to_vec())
    }
  }

  /// Sends `command` and waits for the reply whose first 4 bytes echo `command.header()`.
  /// Non-matching reports are discarded rather than mistaken for this command's reply.
  pub fn request(&self, command: &RogCommand, timeout: Duration) -> Result<Vec<u8>, RogError> {
    #[cfg(target_os = "windows")]
    {
      unsafe {
        let _ = HidD_FlushQueue(self.handle.0);
      }
      let desc = command.hex_description();
      self.send(command, timeout)?;

      let expected = command.header();
      let deadline = Instant::now() + timeout;
      loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
          return Err(RogError::TimedOut(desc));
        }
        let reply = self.read_one_report(remaining, &desc)?;
        if reply.len() >= 2 && reply[0..2] == REJECTION_PREFIX {
          return Err(RogError::Rejected(desc));
        }
        if reply.len() >= expected.len() && reply[0..expected.len()] == expected {
          return Ok(reply);
        }
      }
    }

    #[cfg(not(target_os = "windows"))]
    {
      let _ = (command, timeout);
      Err(RogError::NotFound)
    }
  }

  pub fn read_link(&self, timeout: Duration) -> Result<RogLink, RogError> {
    let cmd = RogCommand::connection_type();
    let reply = self.request(&cmd, timeout)?;
    if reply.len() < 5 {
      return Err(RogError::MalformedReply(cmd.hex_description()));
    }
    RogLink::from_raw(reply[4]).ok_or_else(|| RogError::MalformedReply(cmd.hex_description()))
  }

  pub fn read_device_info(&self, link: RogLink, timeout: Duration) -> Result<RogDeviceInfo, RogError> {
    let cmd = RogCommand::device_info();
    let reply = self.request(&cmd, timeout)?;
    RogDeviceInfo::from_reply(&reply, link)
      .ok_or_else(|| RogError::MalformedReply(cmd.hex_description()))
  }

  pub fn read_state(&self, timeout: Duration) -> Result<(RogLink, RogDeviceInfo), RogError> {
    let link = self.read_link(timeout)?;
    let info = self.read_device_info(link, timeout)?;
    Ok((link, info))
  }

  pub fn read_battery(&self, timeout: Duration) -> Result<RogBatteryInfo, RogError> {
    let cmd = RogCommand::battery_info();
    let reply = self.request(&cmd, timeout)?;
    RogBatteryInfo::from_reply(&reply)
      .ok_or_else(|| RogError::MalformedReply(cmd.hex_description()))
  }

  /// Switches the active profile and reads back the confirmed state.
  pub fn change_profile(
    &self,
    number: usize,
    link: RogLink,
    profile_count: usize,
  ) -> Result<RogDeviceInfo, RogError> {
    if !RogProfile::is_valid(number, profile_count) {
      return Err(RogError::InvalidProfile(number));
    }
    let timeout = Duration::from_secs(2);
    self.send(&RogCommand::change_profile(number), timeout)?;
    // Firmware reloads its keymap before reporting the updated profile accurately.
    thread::sleep(Duration::from_millis(250));
    let info = self.read_device_info(link, timeout)?;
    if info.current_profile != number {
      LOG.error(&format!(
        "profile switch did not stick: asked {}, got {}",
        number, info.current_profile
      ));
    }
    Ok(info)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[ignore = "Requires live ROG Falcata hardware attached to Windows"]
  fn live_read_rog_falcata() {
    let devices = RogHidDevice::discover(&RogModel::ALL_PRODUCT_IDS);
    assert!(
      !devices.is_empty(),
      "No ROG Falcata control interface (VID 0x0B05, UsagePage 0xFF00) found"
    );
    let dev = RogHidDevice::open(&devices[0]).expect("Failed to open ROG Falcata control HID");
    let (link, info) = dev
      .read_state(Duration::from_secs(2))
      .expect("Failed to read ROG Falcata state");
    let battery = dev
      .read_battery(Duration::from_secs(2))
      .expect("Failed to read ROG Falcata battery");
    println!(
      "LIVE HARDWARE CHECK: pid=0x{:04X} link={:?} firmware={} profiles={} current={} battery={}% ({:?}, L={:?}, R={:?})",
      dev.product_id,
      link,
      info.firmware_version,
      info.profile_count,
      info.current_profile,
      battery.percent,
      battery.charging_state,
      battery.left_percent,
      battery.right_percent
    );
    assert!(info.profile_count >= 1);
    assert!(info.current_profile >= 1);
    assert!(battery.percent <= 100);
  }
}
