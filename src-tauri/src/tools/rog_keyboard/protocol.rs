use serde::{Deserialize, Serialize};

/// ASUS ROG peripherals speak a small framed protocol over a vendor-defined HID
/// collection (usage page 0xFF00, usage 1) that sits alongside the ordinary
/// typing interfaces. A request is:
///
///   [command, subcommand, index_low, index_high, payload...]
///
/// zero-padded to the collection's report size (64 bytes over USB, 20 over the
/// 2.4 GHz dongle; Windows HidD/WriteFile prepends 1 byte Report ID `0x00`, so
/// OutputReportByteLength is 65 or 21). The keyboard echoes the first 4 bytes
/// back on its reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RogCommand {
  pub command: u8,
  pub subcommand: u8,
  pub index: u16,
  pub payload: Vec<u8>,
}

impl RogCommand {
  pub fn new(command: u8, subcommand: u8, index: u16, payload: Vec<u8>) -> Self {
    Self {
      command,
      subcommand,
      index,
      payload,
    }
  }

  /// Firmware version, on-board profile count and the currently active profile.
  pub fn device_info() -> Self {
    Self::new(0x12, 0x00, 0, Vec::new())
  }

  /// Battery percentage, charging state, and split left/right half levels.
  pub fn battery_info() -> Self {
    Self::new(0x12, 0x01, 0, Vec::new())
  }

  /// Which physical link the keyboard is using right now.
  pub fn connection_type() -> Self {
    Self::new(0x12, 0x03, 0, Vec::new())
  }

  /// Switch the active on-board profile. Profiles are numbered from 1; see `RogProfile`.
  pub fn change_profile(number: usize) -> Self {
    let clamped = number.min(u8::MAX as usize) as u8;
    Self::new(0x51, 0x00, 0, vec![clamped])
  }

  /// The four bytes the keyboard echoes back. A reply matches when these match.
  pub fn header(&self) -> [u8; 4] {
    [
      self.command,
      self.subcommand,
      (self.index & 0xFF) as u8,
      ((self.index >> 8) & 0xFF) as u8,
    ]
  }

  /// Raw protocol frame without Windows Report ID prefix.
  pub fn frame(&self, size: usize) -> Vec<u8> {
    let mut frame = vec![0u8; size];
    let header = self.header();
    for (i, byte) in header.iter().chain(self.payload.iter()).enumerate() {
      if i >= size {
        break;
      }
      frame[i] = *byte;
    }
    frame
  }

  /// Windows HID output report with leading `0x00` Report ID byte.
  pub fn windows_report(&self, output_report_byte_length: usize) -> Vec<u8> {
    let size = output_report_byte_length.max(2);
    let mut report = vec![0u8; size];
    let payload_frame = self.frame(size - 1);
    report[1..].copy_from_slice(&payload_frame);
    report
  }

  pub fn hex_description(&self) -> String {
    self
      .header()
      .iter()
      .chain(self.payload.iter())
      .map(|b| format!("{:02X}", b))
      .collect::<Vec<_>>()
      .join(" ")
  }
}

/// Profile numbering rules verified against a real ROG Falcata:
/// profiles are numbered from **1**, and sending **0 silently selects the LAST profile**
/// instead of failing. Every profile slot must be validated before sending.
pub struct RogProfile;

impl RogProfile {
  pub const FIRST: usize = 1;

  pub fn is_valid(number: usize, count: usize) -> bool {
    number >= Self::FIRST && number <= count.max(Self::FIRST)
  }

  #[allow(dead_code)]
  pub fn clamp(number: usize, count: usize) -> usize {
    number.clamp(Self::FIRST, count.max(Self::FIRST))
  }
}

/// Maps wire-level profile slot numbers to the names ASUS Gear Link displays:
/// "Default Profile" (last slot on the wire) followed by "Profile 1..N-1".
pub struct RogProfileNaming;

impl RogProfileNaming {
  pub fn default_slot(count: usize) -> usize {
    count.max(RogProfile::FIRST)
  }

  #[allow(dead_code)]
  pub fn ordered_slots(count: usize) -> Vec<usize> {
    let total = count.max(RogProfile::FIRST);
    if total <= 1 {
      return vec![total];
    }
    let mut slots = Vec::with_capacity(total);
    slots.push(Self::default_slot(total));
    for slot in RogProfile::FIRST..total {
      slots.push(slot);
    }
    slots
  }

  pub fn label(wire: usize, count: usize, is_zh: bool) -> String {
    if wire == Self::default_slot(count) {
      if is_zh {
        "默认 配置文件".to_string()
      } else {
        "Default Profile".to_string()
      }
    } else if is_zh {
      format!("配置文件 {}", wire)
    } else {
      format!("Profile {}", wire)
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RogLink {
  Wired = 0,
  #[serde(rename = "wireless24g")]
  Wireless24G = 1,
  Bluetooth = 2,
}

impl RogLink {
  pub fn from_raw(value: u8) -> Option<Self> {
    match value {
      0 => Some(Self::Wired),
      1 => Some(Self::Wireless24G),
      2 => Some(Self::Bluetooth),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RogDeviceInfo {
  pub firmware_version: String,
  pub profile_count: usize,
  pub current_profile: usize,
  pub effect_mode: usize,
}

impl RogDeviceInfo {
  /// Parses reply to `0x12 0x00` (payload without leading Report ID byte).
  /// Wired firmware version sits at bytes 4..8; over 2.4G dongle / BT it sits at bytes 11..15.
  pub fn from_reply(reply: &[u8], link: RogLink) -> Option<Self> {
    if reply.len() < 15 {
      return None;
    }
    let offset = if link == RogLink::Wired { 4 } else { 11 };
    let raw = u32::from(reply[offset])
      | (u32::from(reply[offset + 1]) << 8)
      | (u32::from(reply[offset + 2]) << 16)
      | (u32::from(reply[offset + 3]) << 24);
    let firmware_version = if raw == 0 {
      "—".to_string()
    } else {
      format!(
        "{:X}.{:02X}.{:02X}",
        (raw >> 16) & 0xFF,
        (raw >> 8) & 0xFF,
        raw & 0xFF
      )
    };
    Some(Self {
      firmware_version,
      profile_count: usize::from(reply[8]),
      effect_mode: usize::from(reply[9]),
      current_profile: usize::from(reply[10]),
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RogChargingState {
  Discharging,
  Charging,
  Full,
  Unknown,
}

impl RogChargingState {
  pub fn from_raw(value: u8) -> Self {
    match value {
      0 => Self::Discharging,
      1 => Self::Charging,
      2 => Self::Full,
      _ => Self::Unknown,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RogBatteryInfo {
  pub percent: u8,
  pub charging_state: RogChargingState,
  pub left_percent: Option<u8>,
  pub left_mv: Option<u16>,
  pub right_percent: Option<u8>,
  pub right_mv: Option<u16>,
}

impl RogBatteryInfo {
  /// Parses reply to `0x12 0x01` (payload without leading Report ID byte).
  /// Example captured from ROG Falcata:
  /// `12 01 00 00 00 64 02 01 01 14 64 FE 10 64 C3 10 ...`
  /// byte 5 = overall %, byte 6 = charging state (0=discharging, 1=charging, 2=full),
  /// byte 10 = left %, bytes 11..13 = left mV LE,
  /// byte 13 = right %, bytes 14..16 = right mV LE.
  pub fn from_reply(reply: &[u8]) -> Option<Self> {
    if reply.len() < 7 || reply[5] > 100 {
      return None;
    }
    let percent = reply[5];
    let charging_state = RogChargingState::from_raw(reply[6]);

    let (left_percent, left_mv) = if reply.len() >= 13 {
      let mv = u16::from_le_bytes([reply[11], reply[12]]);
      if reply[10] <= 100 && mv > 0 {
        (Some(reply[10]), Some(mv))
      } else {
        (None, None)
      }
    } else {
      (None, None)
    };

    let (right_percent, right_mv) = if reply.len() >= 16 {
      let mv = u16::from_le_bytes([reply[14], reply[15]]);
      if reply[13] <= 100 && mv > 0 {
        (Some(reply[13]), Some(mv))
      } else {
        (None, None)
      }
    } else {
      (None, None)
    };

    Some(Self {
      percent,
      charging_state,
      left_percent,
      left_mv,
      right_percent,
      right_mv,
    })
  }
}

pub struct RogModel;

impl RogModel {
  pub const DISPLAY_NAME: &'static str = "ROG Falcata";
  pub const VENDOR_ID: u16 = 0x0B05;
  pub const PID_WIRED: u16 = 0x1C2F;
  pub const PID_WIRELESS: u16 = 0x1C31;
  pub const CONTROL_USAGE_PAGE: u16 = 0xFF00;
  pub const CONTROL_USAGE: u16 = 0x0001;
  pub const ALL_PRODUCT_IDS: [u16; 2] = [Self::PID_WIRED, Self::PID_WIRELESS];
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn windows_report_prepends_zero_report_id_and_pads() {
    let cmd = RogCommand::change_profile(1);
    let wired = cmd.windows_report(65);
    assert_eq!(wired.len(), 65);
    assert_eq!(&wired[0..6], &[0x00, 0x51, 0x00, 0x00, 0x00, 0x01]);
    assert!(wired[6..].iter().all(|&b| b == 0));

    let dongle = RogCommand::device_info().windows_report(21);
    assert_eq!(dongle.len(), 21);
    assert_eq!(&dongle[0..5], &[0x00, 0x12, 0x00, 0x00, 0x00]);
  }

  #[test]
  fn profile_validation_rejects_zero_and_out_of_range() {
    assert!(!RogProfile::is_valid(0, 6));
    assert!(RogProfile::is_valid(1, 6));
    assert!(RogProfile::is_valid(6, 6));
    assert!(!RogProfile::is_valid(7, 6));
    assert_eq!(RogProfile::clamp(0, 6), 1);
    assert_eq!(RogProfile::clamp(9, 6), 6);
  }

  #[test]
  fn profile_naming_orders_default_first_and_formats_labels() {
    assert_eq!(RogProfileNaming::ordered_slots(6), vec![6, 1, 2, 3, 4, 5]);
    assert_eq!(RogProfileNaming::label(6, 6, true), "默认 配置文件");
    assert_eq!(RogProfileNaming::label(1, 6, true), "配置文件 1");
    assert_eq!(RogProfileNaming::label(6, 6, false), "Default Profile");
    assert_eq!(RogProfileNaming::label(1, 6, false), "Profile 1");
  }

  #[test]
  fn device_info_decodes_wired_and_wireless_firmware_versions() {
    // Real reply captured from ROG Falcata wired:
    // 12-00-00-00-23-00-08-00-06-02-01-00-00-00-00
    let wired_reply = [
      0x12, 0x00, 0x00, 0x00, 0x23, 0x00, 0x08, 0x00, 0x06, 0x02, 0x01, 0x00, 0x00, 0x00, 0x00,
    ];
    let info = RogDeviceInfo::from_reply(&wired_reply, RogLink::Wired).unwrap();
    assert_eq!(info.firmware_version, "8.00.23");
    assert_eq!(info.profile_count, 6);
    assert_eq!(info.effect_mode, 2);
    assert_eq!(info.current_profile, 1);

    let wireless_reply = [
      0x12, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x06, 0x03, 0x06, 0x23, 0x00, 0x08, 0x00,
    ];
    let info_w = RogDeviceInfo::from_reply(&wireless_reply, RogLink::Wireless24G).unwrap();
    assert_eq!(info_w.firmware_version, "8.00.23");
    assert_eq!(info_w.current_profile, 6);
  }

  #[test]
  fn battery_info_decodes_full_and_discharging_and_rejects_malformed() {
    // Live captured reply from ROG Falcata wired:
    // 12-01-00-00-00-64-02-01-01-14-64-FE-10-64-C3-10
    let full_reply = [
      0x12, 0x01, 0x00, 0x00, 0x00, 0x64, 0x02, 0x01, 0x01, 0x14, 0x64, 0xFE, 0x10, 0x64, 0xC3,
      0x10,
    ];
    let bat = RogBatteryInfo::from_reply(&full_reply).unwrap();
    assert_eq!(bat.percent, 100);
    assert_eq!(bat.charging_state, RogChargingState::Full);
    assert_eq!(bat.left_percent, Some(100));
    assert_eq!(bat.left_mv, Some(4350));
    assert_eq!(bat.right_percent, Some(100));
    assert_eq!(bat.right_mv, Some(4291));

    let discharging_reply = [
      0x12, 0x01, 0x00, 0x00, 0x00, 0x4E, 0x00, 0x00, 0x00, 0x00, 0x50, 0xA0, 0x0F, 0x4C, 0x8C,
      0x0F,
    ];
    let bat_d = RogBatteryInfo::from_reply(&discharging_reply).unwrap();
    assert_eq!(bat_d.percent, 78);
    assert_eq!(bat_d.charging_state, RogChargingState::Discharging);
    assert_eq!(bat_d.left_percent, Some(80));
    assert_eq!(bat_d.right_percent, Some(76));

    assert!(RogBatteryInfo::from_reply(&[0x12, 0x01, 0x00]).is_none());
    assert!(RogBatteryInfo::from_reply(&[0x12, 0x01, 0x00, 0x00, 0x00, 0xFF, 0x00]).is_none());
  }
}
