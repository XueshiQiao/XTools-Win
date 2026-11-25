use std::thread;
use std::time::Duration;

use super::config::NagaPlateLayout;
use super::protocol::{
  is_allowed_writable_button, NagaBatteryInfo, NagaChargingState, NagaDeviceState, NagaDpi,
  NagaHardwareAction, NagaHardwareButton, NagaLink, NagaModel, NagaProfileInfo, RazerReport,
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
  InvalidParam(String),
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
          format!("读取或写入鼠标 HID 报告失败（Win32 0x{:08X}）。", code)
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
          "鼠标拒绝了该指令。".to_string()
        } else {
          "Mouse rejected the command.".to_string()
        }
      }
      Self::TimedOut => {
        if is_zh {
          "等待鼠标回应超时。".to_string()
        } else {
          "Timed out waiting for mouse reply.".to_string()
        }
      }
      Self::InvalidParam(msg) => msg.clone(),
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

  pub fn open_active() -> Result<Self, NagaError> {
    let candidates = Self::discover(&NagaModel::ALL_PRODUCT_IDS);
    if candidates.is_empty() {
      return Err(NagaError::NotFound);
    }

    let mut last_err = NagaError::Offline;
    for info in candidates {
      match Self::open(&info) {
        Ok(dev) => match dev.exchange(&RazerReport::battery_request()) {
          Ok(_) => return Ok(dev),
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

  fn read_active_profile_slot(&self) -> u8 {
    self
      .exchange(&RazerReport::active_profile_request())
      .ok()
      .map(|p| p[8])
      .filter(|&p| (1..=NagaModel::MAX_PROFILES).contains(&p))
      .unwrap_or(1)
  }

  pub fn read_profile_info(&self) -> Option<NagaProfileInfo> {
    let (max_profiles, mut available_profiles) = self
      .exchange(&RazerReport::profile_list_request())
      .ok()
      .and_then(|p| RazerReport::parse_profile_list(&p))
      .unwrap_or((NagaModel::MAX_PROFILES, vec![1]));

    let active_profile = self.read_active_profile_slot();
    if !available_profiles.contains(&active_profile) {
      available_profiles.push(active_profile);
      available_profiles.sort_unstable();
    }

    Some(NagaProfileInfo {
      max_profiles,
      active_profile,
      available_profiles,
    })
  }

  pub fn read_buttons(&self, profile: u8, plate_layout: NagaPlateLayout) -> Vec<NagaHardwareButton> {
    let ids = plate_layout.button_ids();
    let mut buttons = Vec::with_capacity(ids.len());
    for (idx, &button_id) in ids.iter().enumerate() {
      let action = match self.exchange(&RazerReport::get_button_request(profile, button_id, 0x00)) {
        Ok(payload) => RazerReport::parse_button_action(&payload),
        Err(err) => {
          LOG.warn(&format!(
            "get_button(profile={}, btn=0x{:02X}) failed: {:?}",
            profile, button_id, err
          ));
          plate_layout.factory_default_action(button_id)
        }
      };
      buttons.push(NagaHardwareButton {
        index: (idx + 1) as u8,
        button_id,
        action,
      });
    }
    buttons
  }

  pub fn read_device_state(&self, plate_layout: NagaPlateLayout) -> Result<NagaDeviceState, NagaError> {
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

    let polling_rate_hz = self
      .exchange(&RazerReport::polling_rate_get_request())
      .ok()
      .and_then(|p| RazerReport::parse_polling_rate(&p));

    let profile_info = self.read_profile_info();
    let active_profile = profile_info.as_ref().map(|p| p.active_profile).unwrap_or(1);

    let dpi_stages = self
      .exchange(&RazerReport::dpi_stages_get_request(active_profile))
      .ok()
      .and_then(|p| RazerReport::parse_dpi_stages(&p))
      .or_else(|| {
        self
          .exchange(&RazerReport::dpi_stages_get_request(0x00))
          .ok()
          .and_then(|p| RazerReport::parse_dpi_stages(&p))
      });

    let buttons = self.read_buttons(active_profile, plate_layout);

    Ok(NagaDeviceState {
      link,
      firmware_version,
      battery: Some(NagaBatteryInfo {
        percent,
        charging_state,
      }),
      dpi,
      dpi_stages,
      polling_rate_hz,
      profile_info,
      buttons,
    })
  }

  pub fn read_active_state(plate_layout: NagaPlateLayout) -> Result<NagaDeviceState, NagaError> {
    let dev = Self::open_active()?;
    dev.read_device_state(plate_layout)
  }

  pub fn set_active_profile(&self, profile: u8) -> Result<(), NagaError> {
    if !(1..=NagaModel::MAX_PROFILES).contains(&profile) {
      return Err(NagaError::InvalidParam(format!(
        "Invalid profile slot: {}",
        profile
      )));
    }
    let available = self
      .exchange(&RazerReport::profile_list_request())
      .ok()
      .and_then(|p| RazerReport::parse_profile_list(&p))
      .map(|(_, list)| list)
      .unwrap_or_else(|| vec![1]);

    if !available.contains(&profile) {
      self.exchange(&RazerReport::create_profile_request(profile))?;
    }
    self.exchange(&RazerReport::set_active_profile_request(profile))?;
    Ok(())
  }

  pub fn delete_profile(&self, profile: u8) -> Result<(), NagaError> {
    if !(2..=NagaModel::MAX_PROFILES).contains(&profile) {
      return Err(NagaError::InvalidParam(format!(
        "Cannot delete profile slot {}",
        profile
      )));
    }
    if self.read_active_profile_slot() == profile {
      self.exchange(&RazerReport::set_active_profile_request(1))?;
    }
    self.exchange(&RazerReport::delete_profile_request(profile))?;
    Ok(())
  }

  pub fn set_button_action(
    &self,
    profile: u8,
    button_id: u8,
    action: &NagaHardwareAction,
  ) -> Result<NagaHardwareAction, NagaError> {
    if !(1..=NagaModel::MAX_PROFILES).contains(&profile) {
      return Err(NagaError::InvalidParam(format!(
        "Invalid profile slot: {}",
        profile
      )));
    }
    if !is_allowed_writable_button(button_id) {
      return Err(NagaError::InvalidParam(format!(
        "Button 0x{:02X} is not writable",
        button_id
      )));
    }

    let req = RazerReport::set_button_request(profile, button_id, 0x00, action)
      .map_err(NagaError::InvalidParam)?;
    self.exchange(&req)?;

    if self.read_active_profile_slot() == profile {
      let live_req = RazerReport::set_button_request(0x00, button_id, 0x00, action)
        .map_err(NagaError::InvalidParam)?;
      let _ = self.exchange(&live_req);
    }

    let readback = self.exchange(&RazerReport::get_button_request(profile, button_id, 0x00))?;
    Ok(RazerReport::parse_button_action(&readback))
  }

  pub fn set_dpi_stages(
    &self,
    profile: u8,
    active_stage: u8,
    stages: &[NagaDpi],
  ) -> Result<(), NagaError> {
    if !(1..=NagaModel::MAX_PROFILES).contains(&profile) {
      return Err(NagaError::InvalidParam(format!(
        "Invalid profile slot: {}",
        profile
      )));
    }
    if stages.is_empty() || stages.len() > NagaModel::MAX_DPI_STAGES {
      return Err(NagaError::InvalidParam(format!(
        "DPI stage count must be 1..={}",
        NagaModel::MAX_DPI_STAGES
      )));
    }

    let clamped: Vec<NagaDpi> = stages
      .iter()
      .map(|s| NagaDpi {
        x: s.x.clamp(NagaModel::DPI_MIN, NagaModel::DPI_MAX),
        y: s.y.clamp(NagaModel::DPI_MIN, NagaModel::DPI_MAX),
      })
      .collect();
    let active = active_stage.clamp(1, clamped.len() as u8);

    self.exchange(&RazerReport::dpi_stages_set_request(
      profile, active, &clamped,
    ))?;

    if self.read_active_profile_slot() == profile {
      let _ = self.exchange(&RazerReport::dpi_stages_set_request(
        0x00, active, &clamped,
      ));
      let active_dpi = clamped[usize::from(active - 1)];
      let _ = self.exchange(&RazerReport::set_dpi_request(0x00, active_dpi));
    }
    Ok(())
  }

  pub fn set_polling_rate(&self, hz: u16) -> Result<(), NagaError> {
    let req = RazerReport::polling_rate_set_request(hz)
      .ok_or_else(|| NagaError::InvalidParam(format!("Unsupported polling rate: {} Hz", hz)))?;
    self.exchange(&req)?;
    Ok(())
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
    let state = NagaHidDevice::read_active_state(NagaPlateLayout::SixButton)
      .expect("Failed to read live Razer Naga V3 Pro state");
    let bat = state.battery.as_ref().expect("Expected battery info");
    println!(
      "LIVE RAZER HARDWARE CHECK: link={:?} firmware={:?} battery={}% ({:?}) dpi={:?} poll={:?} profiles={:?} stages={:?} buttons={:?}",
      state.link,
      state.firmware_version,
      bat.percent,
      bat.charging_state,
      state.dpi,
      state.polling_rate_hz,
      state.profile_info,
      state.dpi_stages,
      state.buttons
    );
    assert!(bat.percent <= 100);
  }

  #[test]
  #[ignore = "Requires live Razer Naga V3 Pro hardware; temporarily toggles button 0x55 and restores it"]
  fn live_naga_onboard_roundtrip() {
    let dev = NagaHidDevice::open_active().expect("Failed to open active Naga V3 Pro");
    let before = dev
      .read_device_state(NagaPlateLayout::SixButton)
      .expect("Failed to read initial state");
    let active_profile = before
      .profile_info
      .as_ref()
      .map(|p| p.active_profile)
      .unwrap_or(1);

    let btn_id = 0x55u8; // 6-button side plate #6
    let orig_payload = dev
      .exchange(&RazerReport::get_button_request(active_profile, btn_id, 0x00))
      .expect("Failed to read original button 0x55");
    let orig_action = RazerReport::parse_button_action(&orig_payload);
    println!("Original button 0x55 on profile {}: {:?}", active_profile, orig_action);

    let temp_w = NagaHardwareAction::Keyboard {
      vk: b'W' as u16,
      hid_code: 0x1A,
      ctrl: false,
      shift: false,
      alt: false,
      win: false,
    };

    let readback_p = dev
      .set_button_action(active_profile, btn_id, &temp_w)
      .expect("Failed to write W to button 0x55");
    assert_eq!(readback_p, temp_w);

    let live_payload = dev
      .exchange(&RazerReport::get_button_request(0x00, btn_id, 0x00))
      .expect("Failed to read live RAM slot 0 for button 0x55");
    assert_eq!(RazerReport::parse_button_action(&live_payload), temp_w);
    println!("Verified write & live RAM readback of W on button 0x55");

    // Restore original button mapping
    let restored = dev
      .set_button_action(active_profile, btn_id, &orig_action)
      .expect("Failed to restore original button 0x55");
    assert_eq!(restored, orig_action);
    println!("Restored original button 0x55: {:?}", restored);

    // Verify DPI stages write-back with exact current stages (no change to user settings)
    if let Some(stages) = before.dpi_stages.clone() {
      dev
        .set_dpi_stages(active_profile, stages.active_stage, &stages.stages)
        .expect("Failed to roundtrip DPI stages");
      let after_stages = dev
        .exchange(&RazerReport::dpi_stages_get_request(active_profile))
        .ok()
        .and_then(|p| RazerReport::parse_dpi_stages(&p))
        .expect("Failed to re-read DPI stages");
      assert_eq!(after_stages, stages);
      println!("Verified DPI stages roundtrip: {:?}", after_stages);
    }

    // Verify polling rate write-back with exact current Hz
    if let Some(hz) = before.polling_rate_hz {
      dev
        .set_polling_rate(hz)
        .expect("Failed to roundtrip polling rate");
      let after_poll = dev
        .exchange(&RazerReport::polling_rate_get_request())
        .ok()
        .and_then(|p| RazerReport::parse_polling_rate(&p));
      assert_eq!(after_poll, Some(hz));
      println!("Verified polling rate roundtrip: {:?} Hz", after_poll);
    }
  }
}
