use std::thread;
use std::time::Duration;

use super::protocol::{
  NagaBatteryInfo, NagaChargingState, NagaDeviceState, NagaLink, NagaModel, RazerReport,
};
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
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetFeature, HidD_GetHidGuid,
    HidD_GetPreparsedData, HidD_SetFeature, HidP_GetCaps, HIDD_ATTRIBUTES, HIDP_CAPS,
    HIDP_STATUS_SUCCESS, PHIDP_PREPARSED_DATA,
  },
  Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE},
  Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
  },
};

const LOG: FileLog = FileLog::new("RazerNagaHID");

#[derive(Debug, Clone)]
pub enum NagaError {
  NotFound,
  OpenFailed(u32),
  IoFailed(u32),
  Offline,
  Rejected,
  TimedOut,
}

impl NagaError {
  pub fn message(&self, is_zh: bool) -> String {
    match self {
      Self::NotFound => {
        if is_zh {
          "找不到 Razer Naga V3 Pro（请检查 USB 线或 2.4G 接收器连接）。".to_string()
        } else {
          "Razer Naga V3 Pro not found (check USB cable or 2.4G dongle).".to_string()
        }
      }
      Self::OpenFailed(code) => {
        if is_zh {
          format!("无法打开鼠标控制接口（Win32 0x{:08X}）。", code)
        } else {
          format!("Failed to open mouse control interface (Win32 0x{:08X}).", code)
        }
      }
      Self::IoFailed(code) => {
        if is_zh {
          format!("读取鼠标特征报告失败（Win32 0x{:08X}）。", code)
        } else {
          format!("HID feature report I/O failed (Win32 0x{:08X}).", code)
        }
      }
      Self::Offline => {
        if is_zh {
          "鼠标当前未响应（可能已休眠或切换为蓝牙模式）。".to_string()
        } else {
          "Mouse did not respond (it may be asleep or on Bluetooth).".to_string()
        }
      }
      Self::Rejected => {
        if is_zh {
          "鼠标拒绝了查询指令。".to_string()
        } else {
          "Mouse rejected the query command.".to_string()
        }
      }
      Self::TimedOut => {
        if is_zh {
          "等待鼠标回应超时。".to_string()
        } else {
          "Timed out waiting for mouse reply.".to_string()
        }
      }
    }
  }
}

#[derive(Debug, Clone)]
pub struct NagaHidDeviceInfo {
  pub path: String,
  pub product_id: u16,
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

pub struct NagaHidDevice {
  pub product_id: u16,
  #[cfg(target_os = "windows")]
  handle: OwnedHandle,
}

impl NagaHidDevice {
  pub fn discover(product_ids: &[u16]) -> Vec<NagaHidDeviceInfo> {
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
        if !path.to_ascii_lowercase().contains("vid_1532") {
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
        if attrs.VendorID != NagaModel::VENDOR_ID || !product_ids.contains(&attrs.ProductID) {
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
        if caps.UsagePage != NagaModel::CONTROL_USAGE_PAGE
          || caps.Usage != NagaModel::CONTROL_USAGE
          || usize::from(caps.FeatureReportByteLength) != NagaModel::FEATURE_REPORT_LEN
        {
          continue;
        }

        found.push(NagaHidDeviceInfo {
          path,
          product_id: attrs.ProductID,
        });
      }

      let _ = SetupDiDestroyDeviceInfoList(dev_info);
      found.sort_by_key(|d| {
        if d.product_id == NagaModel::PID_WIRED {
          0u8
        } else {
          1u8
        }
      });
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

  pub fn open(info: &NagaHidDeviceInfo) -> Result<Self, NagaError> {
    #[cfg(target_os = "windows")]
    unsafe {
      const GENERIC_RW: u32 = 0x8000_0000 | 0x4000_0000;
      let wide: Vec<u16> = info.path.encode_utf16().chain(std::iter::once(0)).collect();
      let handle = CreateFileW(
        windows::core::PCWSTR(wide.as_ptr()),
        GENERIC_RW,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None,
      )
      .map_err(|e| NagaError::OpenFailed(e.code().0 as u32))?;

      if !OwnedHandle::is_valid(handle) {
        return Err(NagaError::OpenFailed(GetLastError().0));
      }

      Ok(Self {
        product_id: info.product_id,
        handle: OwnedHandle(handle),
      })
    }

    #[cfg(not(target_os = "windows"))]
    {
      let _ = info;
      Err(NagaError::NotFound)
    }
  }

  /// Sends a Razer 90-byte feature report and polls quickly until the response matches
  /// `(tx_id, cmd_class, cmd_id)`, tolerating Razer Synapse's continuous Chroma traffic.
  pub fn exchange(
    &self,
    req: &[u8; NagaModel::FEATURE_REPORT_LEN],
  ) -> Result<[u8; NagaModel::REPORT_PAYLOAD_LEN], NagaError> {
    #[cfg(target_os = "windows")]
    unsafe {
      for _attempt in 0..10 {
        if !HidD_SetFeature(
          self.handle.0,
          req.as_ptr() as *const std::ffi::c_void,
          req.len() as u32,
        ) {
          return Err(NagaError::IoFailed(GetLastError().0));
        }

        for _poll in 0..8 {
          thread::sleep(Duration::from_millis(5));
          let mut resp = [0u8; NagaModel::FEATURE_REPORT_LEN];
          if !HidD_GetFeature(
            self.handle.0,
            resp.as_mut_ptr() as *mut std::ffi::c_void,
            resp.len() as u32,
          ) {
            return Err(NagaError::IoFailed(GetLastError().0));
          }

          let payload: [u8; NagaModel::REPORT_PAYLOAD_LEN] = resp[1..].try_into().unwrap();
          if !RazerReport::matches_request(req, &payload) {
            continue;
          }

          match payload[0] {
            RazerReport::STATUS_BUSY => continue,
            RazerReport::STATUS_SUCCESS => return Ok(payload),
            RazerReport::STATUS_TIMEOUT => return Err(NagaError::Offline),
            RazerReport::STATUS_FAILURE => return Err(NagaError::Rejected),
            _ => continue,
          }
        }

        thread::sleep(Duration::from_millis(8));
      }

      Err(NagaError::TimedOut)
    }

    #[cfg(not(target_os = "windows"))]
    {
      let _ = req;
      Err(NagaError::NotFound)
    }
  }

  pub fn read_device_state(&self) -> Result<NagaDeviceState, NagaError> {
    let link = NagaLink::from_pid(self.product_id).unwrap_or(NagaLink::Wired);

    let bat_payload = self.exchange(&RazerReport::battery_request())?;
    let percent = RazerReport::parse_battery_percent(bat_payload[9]);

    let charging_raw = self
      .exchange(&RazerReport::charging_request())
      .ok()
      .map(|p| p[9]);
    let charging_state = match charging_raw {
      Some(1) => {
        if percent >= 100 {
          NagaChargingState::Full
        } else {
          NagaChargingState::Charging
        }
      }
      Some(0) => NagaChargingState::Discharging,
      _ => NagaChargingState::Unknown,
    };

    let firmware_version = self
      .exchange(&RazerReport::firmware_request())
      .ok()
      .and_then(|p| RazerReport::parse_firmware(&p));

    let dpi = self
      .exchange(&RazerReport::dpi_request())
      .ok()
      .and_then(|p| RazerReport::parse_dpi(&p));

    Ok(NagaDeviceState {
      link,
      firmware_version,
      battery: Some(NagaBatteryInfo {
        percent,
        charging_state,
      }),
      dpi,
    })
  }

  pub fn read_active_state() -> Result<NagaDeviceState, NagaError> {
    let candidates = Self::discover(&NagaModel::ALL_PRODUCT_IDS);
    if candidates.is_empty() {
      return Err(NagaError::NotFound);
    }

    let mut last_err = NagaError::Offline;
    for info in candidates {
      match Self::open(&info) {
        Ok(dev) => match dev.read_device_state() {
          Ok(state) => return Ok(state),
          Err(err) => {
            LOG.info(&format!(
              "probe PID 0x{:04X} skipped: {:?}",
              info.product_id, err
            ));
            last_err = err;
          }
        },
        Err(err) => {
          last_err = err;
        }
      }
    }

    Err(last_err)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[ignore = "Requires live Razer Naga V3 Pro hardware attached to Windows"]
  fn live_read_razer_naga_v3_pro() {
    let candidates = NagaHidDevice::discover(&NagaModel::ALL_PRODUCT_IDS);
    assert!(
      !candidates.is_empty(),
      "No Razer Naga V3 Pro control interface (VID 0x1532, UsagePage 0x0001, Usage 0x0003, Feature 91) found"
    );
    let state =
      NagaHidDevice::read_active_state().expect("Failed to read live Razer Naga V3 Pro state");
    let bat = state.battery.as_ref().expect("Expected battery info");
    println!(
      "LIVE RAZER HARDWARE CHECK: link={:?} firmware={:?} battery={}% ({:?}) dpi={:?}",
      state.link, state.firmware_version, bat.percent, bat.charging_state, state.dpi
    );
    assert!(bat.percent <= 100);
  }
}
