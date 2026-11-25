use serde::{Deserialize, Serialize};

use crate::core::vk::KeyShortcut;

pub struct NagaModel;

impl NagaModel {
  pub const VENDOR_ID: u16 = 0x1532;
  pub const PID_WIRED: u16 = 0x00E7;
  pub const PID_WIRELESS: u16 = 0x00E8;
  pub const ALL_PRODUCT_IDS: [u16; 2] = [Self::PID_WIRED, Self::PID_WIRELESS];

  pub const CONTROL_USAGE_PAGE: u16 = 0x0001;
  pub const CONTROL_USAGE: u16 = 0x0003;
  pub const FEATURE_REPORT_LEN: usize = 91;
  pub const REPORT_PAYLOAD_LEN: usize = 90;
  pub const TX_ID: u8 = 0x1F;
  pub const DISPLAY_NAME: &'static str = "Razer Naga V3 Pro";

  pub const DPI_MIN: u16 = 100;
  pub const DPI_MAX: u16 = 35000;
  pub const MAX_PROFILES: u8 = 5;
  pub const MAX_DPI_STAGES: usize = 5;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NagaLink {
  Wired,
  Wireless24g,
}

impl NagaLink {
  pub fn from_pid(pid: u16) -> Option<Self> {
    match pid {
      NagaModel::PID_WIRED => Some(Self::Wired),
      NagaModel::PID_WIRELESS => Some(Self::Wireless24g),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NagaChargingState {
  Discharging,
  Charging,
  Full,
  Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagaBatteryInfo {
  pub percent: u8,
  pub charging_state: NagaChargingState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagaDpi {
  pub x: u16,
  pub y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagaDpiStages {
  pub active_stage: u8,
  pub stages: Vec<NagaDpi>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagaProfileInfo {
  pub max_profiles: u8,
  pub active_profile: u8,
  pub available_profiles: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NagaMouseButton {
  Left,
  Right,
  Middle,
  Back,
  Forward,
  ScrollUp,
  ScrollDown,
}

impl NagaMouseButton {
  pub fn to_wire(self) -> u8 {
    match self {
      Self::Left => 0x01,
      Self::Right => 0x02,
      Self::Middle => 0x03,
      Self::Back => 0x04,
      Self::Forward => 0x05,
      Self::ScrollUp => 0x09,
      Self::ScrollDown => 0x0A,
    }
  }

  pub fn from_wire(code: u8) -> Option<Self> {
    match code {
      0x01 => Some(Self::Left),
      0x02 => Some(Self::Right),
      0x03 => Some(Self::Middle),
      0x04 => Some(Self::Back),
      0x05 => Some(Self::Forward),
      0x09 => Some(Self::ScrollUp),
      0x0A => Some(Self::ScrollDown),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NagaDpiOp {
  Up,
  Down,
  CycleUp,
  CycleDown,
}

impl NagaDpiOp {
  pub fn to_wire(self) -> u8 {
    match self {
      Self::Up => 0x01,
      Self::Down => 0x02,
      Self::CycleUp => 0x06,
      Self::CycleDown => 0x07,
    }
  }

  pub fn from_wire(code: u8) -> Option<Self> {
    match code {
      0x01 => Some(Self::Up),
      0x02 => Some(Self::Down),
      0x06 => Some(Self::CycleUp),
      0x07 => Some(Self::CycleDown),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NagaHardwareAction {
  Disabled,
  Mouse {
    button: NagaMouseButton,
  },
  Keyboard {
    vk: u16,
    #[serde(default)]
    hid_code: u8,
    #[serde(default)]
    ctrl: bool,
    #[serde(default)]
    shift: bool,
    #[serde(default)]
    alt: bool,
    #[serde(default)]
    win: bool,
  },
  Dpi {
    op: NagaDpiOp,
  },
  Profile {
    op: u8,
  },
  Media {
    usage: u16,
  },
  Hypershift,
  Raw {
    action_type: u8,
    data: Vec<u8>,
  },
}

impl NagaHardwareAction {
  #[allow(dead_code)]
  pub fn from_shortcut(sc: &KeyShortcut) -> Result<Self, String> {
    let hid_code = HidKeyTable::vk_to_hid(sc.vk)
      .ok_or_else(|| format!("Unsupported virtual key for HID mapping: 0x{:02X}", sc.vk))?;
    Ok(Self::Keyboard {
      vk: sc.vk,
      hid_code,
      ctrl: sc.ctrl,
      shift: sc.shift,
      alt: sc.alt,
      win: sc.win,
    })
  }

  pub fn resolve(&self) -> Result<Self, String> {
    match self {
      Self::Keyboard {
        vk,
        hid_code,
        ctrl,
        shift,
        alt,
        win,
      } => {
        let resolved_hid = if *hid_code != 0 {
          *hid_code
        } else {
          HidKeyTable::vk_to_hid(*vk)
            .ok_or_else(|| format!("Unsupported virtual key for HID mapping: 0x{:02X}", vk))?
        };
        let resolved_vk = if *vk != 0 {
          *vk
        } else {
          HidKeyTable::hid_to_vk(resolved_hid).unwrap_or(0)
        };
        Ok(Self::Keyboard {
          vk: resolved_vk,
          hid_code: resolved_hid,
          ctrl: *ctrl,
          shift: *shift,
          alt: *alt,
          win: *win,
        })
      }
      other => Ok(other.clone()),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagaHardwareButton {
  pub index: u8,
  pub button_id: u8,
  pub action: NagaHardwareAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NagaDeviceState {
  pub link: NagaLink,
  pub firmware_version: Option<String>,
  pub battery: Option<NagaBatteryInfo>,
  pub dpi: Option<NagaDpi>,
  pub dpi_stages: Option<NagaDpiStages>,
  pub polling_rate_hz: Option<u16>,
  pub profile_info: Option<NagaProfileInfo>,
  pub buttons: Vec<NagaHardwareButton>,
}

pub struct HidKeyTable;

impl HidKeyTable {
  const PAIRS: &'static [(u16, u8)] = &[
    // Letters A-Z -> HID 0x04..=0x1D
    (0x41, 0x04),
    (0x42, 0x05),
    (0x43, 0x06),
    (0x44, 0x07),
    (0x45, 0x08),
    (0x46, 0x09),
    (0x47, 0x0A),
    (0x48, 0x0B),
    (0x49, 0x0C),
    (0x4A, 0x0D),
    (0x4B, 0x0E),
    (0x4C, 0x0F),
    (0x4D, 0x10),
    (0x4E, 0x11),
    (0x4F, 0x12),
    (0x50, 0x13),
    (0x51, 0x14),
    (0x52, 0x15),
    (0x53, 0x16),
    (0x54, 0x17),
    (0x55, 0x18),
    (0x56, 0x19),
    (0x57, 0x1A),
    (0x58, 0x1B),
    (0x59, 0x1C),
    (0x5A, 0x1D),
    // Digits 1-9, 0 -> HID 0x1E..=0x27
    (0x31, 0x1E),
    (0x32, 0x1F),
    (0x33, 0x20),
    (0x34, 0x21),
    (0x35, 0x22),
    (0x36, 0x23),
    (0x37, 0x24),
    (0x38, 0x25),
    (0x39, 0x26),
    (0x30, 0x27),
    // Control / whitespace
    (0x0D, 0x28), // Enter
    (0x1B, 0x29), // Esc
    (0x08, 0x2A), // Backspace
    (0x09, 0x2B), // Tab
    (0x20, 0x2C), // Space
    // Punctuation
    (0xBD, 0x2D), // -
    (0xBB, 0x2E), // =
    (0xDB, 0x2F), // [
    (0xDD, 0x30), // ]
    (0xDC, 0x31), // \
    (0xBA, 0x33), // ;
    (0xDE, 0x34), // '
    (0xC0, 0x35), // `
    (0xBC, 0x36), // ,
    (0xBE, 0x37), // .
    (0xBF, 0x38), // /
    (0x14, 0x39), // CapsLock
    // Function keys F1-F12 -> HID 0x3A..=0x45
    (0x70, 0x3A),
    (0x71, 0x3B),
    (0x72, 0x3C),
    (0x73, 0x3D),
    (0x74, 0x3E),
    (0x75, 0x3F),
    (0x76, 0x40),
    (0x77, 0x41),
    (0x78, 0x42),
    (0x79, 0x43),
    (0x7A, 0x44),
    (0x7B, 0x45),
    // System / navigation
    (0x2C, 0x46), // PrintScreen
    (0x91, 0x47), // ScrollLock
    (0x13, 0x48), // Pause
    (0x2D, 0x49), // Insert
    (0x24, 0x4A), // Home
    (0x21, 0x4B), // PageUp
    (0x2E, 0x4C), // Delete
    (0x23, 0x4D), // End
    (0x22, 0x4E), // PageDown
    (0x27, 0x4F), // Right
    (0x25, 0x50), // Left
    (0x28, 0x51), // Down
    (0x26, 0x52), // Up
    // Numpad
    (0x90, 0x53), // NumLock
    (0x6F, 0x54), // Num /
    (0x6A, 0x55), // Num *
    (0x6D, 0x56), // Num -
    (0x6B, 0x57), // Num +
    (0x61, 0x59), // Num 1
    (0x62, 0x5A), // Num 2
    (0x63, 0x5B), // Num 3
    (0x64, 0x5C), // Num 4
    (0x65, 0x5D), // Num 5
    (0x66, 0x5E), // Num 6
    (0x67, 0x5F), // Num 7
    (0x68, 0x60), // Num 8
    (0x69, 0x61), // Num 9
    (0x60, 0x62), // Num 0
    (0x6E, 0x63), // Num .
    // Extended function keys F13-F24 -> HID 0x68..=0x73
    (0x7C, 0x68),
    (0x7D, 0x69),
    (0x7E, 0x6A),
    (0x7F, 0x6B),
    (0x80, 0x6C),
    (0x81, 0x6D),
    (0x82, 0x6E),
    (0x83, 0x6F),
    (0x84, 0x70),
    (0x85, 0x71),
    (0x86, 0x72),
    (0x87, 0x73),
    // Standalone modifiers -> HID 0xE0..=0xE7
    (0xA2, 0xE0), // LCtrl
    (0xA0, 0xE1), // LShift
    (0xA4, 0xE2), // LAlt
    (0x5B, 0xE3), // LWin
    (0xA3, 0xE4), // RCtrl
    (0xA1, 0xE5), // RShift
    (0xA5, 0xE6), // RAlt
    (0x5C, 0xE7), // RWin
  ];

  pub fn vk_to_hid(vk: u16) -> Option<u8> {
    match vk {
      0x11 => return Some(0xE0), // VK_CONTROL -> LCtrl
      0x10 => return Some(0xE1), // VK_SHIFT -> LShift
      0x12 => return Some(0xE2), // VK_MENU -> LAlt
      _ => {}
    }
    Self::PAIRS
      .iter()
      .find(|&&(pair_vk, _)| pair_vk == vk)
      .map(|&(_, hid)| hid)
  }

  pub fn hid_to_vk(hid: u8) -> Option<u16> {
    Self::PAIRS
      .iter()
      .find(|&&(_, pair_hid)| pair_hid == hid)
      .map(|&(vk, _)| vk)
  }
}

pub fn is_allowed_writable_button(button_id: u8) -> bool {
  matches!(button_id, 0x04 | 0x05 | 0x40..=0x4B | 0x50..=0x55)
}

pub struct RazerReport;

impl RazerReport {
  pub const STATUS_REQUEST: u8 = 0x00;
  pub const STATUS_BUSY: u8 = 0x01;
  pub const STATUS_SUCCESS: u8 = 0x02;
  pub const STATUS_FAILURE: u8 = 0x03;
  pub const STATUS_TIMEOUT: u8 = 0x04;

  pub fn compute_crc(payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN]) -> u8 {
    let mut crc = 0u8;
    for byte in &payload[2..=87] {
      crc ^= byte;
    }
    crc
  }

  pub fn build_windows_feature_report(
    tx_id: u8,
    data_size: u8,
    cmd_class: u8,
    cmd_id: u8,
    args: &[u8],
  ) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    let mut buf = [0u8; NagaModel::FEATURE_REPORT_LEN];
    buf[0] = 0x00; // Windows HID Report ID

    let mut payload = [0u8; NagaModel::REPORT_PAYLOAD_LEN];
    payload[0] = Self::STATUS_REQUEST;
    payload[1] = tx_id;
    payload[2] = 0x00;
    payload[3] = 0x00;
    payload[4] = 0x00;
    payload[5] = data_size;
    payload[6] = cmd_class;
    payload[7] = cmd_id;

    let max_args = args.len().min(80);
    payload[8..(8 + max_args)].copy_from_slice(&args[..max_args]);
    payload[88] = Self::compute_crc(&payload);
    payload[89] = 0x00;

    buf[1..].copy_from_slice(&payload);
    buf
  }

  pub fn battery_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x02, 0x07, 0x80, &[])
  }

  pub fn charging_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x02, 0x07, 0x84, &[])
  }

  pub fn firmware_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x02, 0x00, 0x81, &[])
  }

  pub fn dpi_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x07, 0x04, 0x85, &[0x00])
  }

  pub fn set_dpi_request(varstore: u8, dpi: NagaDpi) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    let x = dpi.x.to_be_bytes();
    let y = dpi.y.to_be_bytes();
    Self::build_windows_feature_report(
      NagaModel::TX_ID,
      0x07,
      0x04,
      0x05,
      &[varstore, x[0], x[1], y[0], y[1], 0x00, 0x00],
    )
  }

  pub fn dpi_stages_get_request(profile: u8) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x26, 0x04, 0x86, &[profile])
  }

  pub fn dpi_stages_set_request(
    profile: u8,
    active_stage: u8,
    stages: &[NagaDpi],
  ) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    let count = stages.len().clamp(1, NagaModel::MAX_DPI_STAGES);
    let active = active_stage.clamp(1, count as u8);
    let mut args = vec![0u8; 0x26];
    args[0] = profile;
    args[1] = active;
    args[2] = count as u8;
    for (idx, stage) in stages.iter().take(count).enumerate() {
      let off = 3 + idx * 7;
      let x = stage.x.to_be_bytes();
      let y = stage.y.to_be_bytes();
      args[off] = (idx + 1) as u8;
      args[off + 1] = x[0];
      args[off + 2] = x[1];
      args[off + 3] = y[0];
      args[off + 4] = y[1];
      args[off + 5] = 0x00;
      args[off + 6] = 0x00;
    }
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x26, 0x04, 0x06, &args)
  }

  pub fn polling_rate_get_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x01, 0x00, 0x85, &[])
  }

  pub fn polling_rate_set_request(hz: u16) -> Option<[u8; NagaModel::FEATURE_REPORT_LEN]> {
    let code = match hz {
      1000 => 0x01,
      500 => 0x02,
      125 => 0x08,
      _ => return None,
    };
    Some(Self::build_windows_feature_report(
      NagaModel::TX_ID,
      0x01,
      0x00,
      0x05,
      &[code],
    ))
  }

  #[allow(dead_code)]
  pub fn profile_count_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x01, 0x05, 0x80, &[])
  }

  pub fn profile_list_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x10, 0x05, 0x81, &[])
  }

  pub fn active_profile_request() -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x01, 0x05, 0x84, &[])
  }

  pub fn create_profile_request(profile: u8) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x01, 0x05, 0x02, &[profile])
  }

  pub fn delete_profile_request(profile: u8) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x01, 0x05, 0x03, &[profile])
  }

  pub fn set_active_profile_request(profile: u8) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(NagaModel::TX_ID, 0x01, 0x05, 0x04, &[profile])
  }

  pub fn get_button_request(
    profile: u8,
    button_id: u8,
    layer: u8,
  ) -> [u8; NagaModel::FEATURE_REPORT_LEN] {
    Self::build_windows_feature_report(
      NagaModel::TX_ID,
      0x0A,
      0x02,
      0x8C,
      &[profile, button_id, layer],
    )
  }

  pub fn set_button_request(
    profile: u8,
    button_id: u8,
    layer: u8,
    action: &NagaHardwareAction,
  ) -> Result<[u8; NagaModel::FEATURE_REPORT_LEN], String> {
    let resolved = action.resolve()?;
    let (action_type, data): (u8, Vec<u8>) = match resolved {
      NagaHardwareAction::Disabled => (0x00, Vec::new()),
      NagaHardwareAction::Mouse { button } => (0x01, vec![button.to_wire()]),
      NagaHardwareAction::Keyboard {
        hid_code,
        ctrl,
        shift,
        alt,
        win,
        ..
      } => {
        let mut mod_mask = 0u8;
        if ctrl {
          mod_mask |= 0x01;
        }
        if shift {
          mod_mask |= 0x02;
        }
        if alt {
          mod_mask |= 0x04;
        }
        if win {
          mod_mask |= 0x08;
        }
        (0x02, vec![mod_mask, hid_code])
      }
      NagaHardwareAction::Dpi { op } => (0x06, vec![op.to_wire()]),
      NagaHardwareAction::Profile { op } => (0x07, vec![op]),
      NagaHardwareAction::Media { usage } => {
        let bytes = usage.to_be_bytes();
        (0x0A, vec![bytes[0], bytes[1]])
      }
      NagaHardwareAction::Hypershift => (0x0C, vec![0x01]),
      NagaHardwareAction::Raw { action_type, data } => {
        (action_type, data.into_iter().take(5).collect())
      }
    };

    let mut args = [0u8; 10];
    args[0] = profile;
    args[1] = button_id;
    args[2] = layer;
    args[3] = action_type;
    let data_len = data.len().min(5);
    args[4] = data_len as u8;
    args[5..(5 + data_len)].copy_from_slice(&data[..data_len]);

    Ok(Self::build_windows_feature_report(
      NagaModel::TX_ID,
      0x0A,
      0x02,
      0x0C,
      &args,
    ))
  }

  pub fn matches_request(
    req: &[u8; NagaModel::FEATURE_REPORT_LEN],
    payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN],
  ) -> bool {
    payload[1] == req[2] && payload[6] == req[7] && payload[7] == req[8]
  }

  pub fn parse_battery_percent(raw: u8) -> u8 {
    (((u16::from(raw) * 100) + 127) / 255).min(100) as u8
  }

  pub fn parse_firmware(payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN]) -> Option<String> {
    let major = payload[8];
    let minor = payload[9];
    if major == 0 && minor == 0 {
      return None;
    }
    Some(format!("v{}.{:02}", major, minor))
  }

  pub fn parse_dpi(payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN]) -> Option<NagaDpi> {
    let x = u16::from_be_bytes([payload[9], payload[10]]);
    let y = u16::from_be_bytes([payload[11], payload[12]]);
    if x == 0 || y == 0 {
      return None;
    }
    Some(NagaDpi { x, y })
  }

  pub fn parse_dpi_stages(payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN]) -> Option<NagaDpiStages> {
    let raw_count = usize::from(payload[10]);
    if raw_count == 0 {
      return None;
    }
    let count = raw_count.min(NagaModel::MAX_DPI_STAGES);
    let mut stages = Vec::with_capacity(count);
    for i in 0..count {
      let off = 11 + i * 7;
      let x = u16::from_be_bytes([payload[off + 1], payload[off + 2]]);
      let y = u16::from_be_bytes([payload[off + 3], payload[off + 4]]);
      if x == 0 || y == 0 {
        continue;
      }
      stages.push(NagaDpi { x, y });
    }
    if stages.is_empty() {
      return None;
    }
    let active_stage = payload[9].clamp(1, stages.len() as u8);
    Some(NagaDpiStages {
      active_stage,
      stages,
    })
  }

  pub fn parse_polling_rate(payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN]) -> Option<u16> {
    match payload[8] {
      0x01 => Some(1000),
      0x02 => Some(500),
      0x08 => Some(125),
      _ => None,
    }
  }

  pub fn parse_profile_list(
    payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN],
  ) -> Option<(u8, Vec<u8>)> {
    let max_profiles = payload[8];
    if max_profiles == 0 {
      return None;
    }
    let count = usize::from(payload[5])
      .saturating_sub(1)
      .min(usize::from(max_profiles))
      .min(15);
    let mut profiles = Vec::with_capacity(count);
    for &p in &payload[9..(9 + count)] {
      if (1..=NagaModel::MAX_PROFILES).contains(&p) && !profiles.contains(&p) {
        profiles.push(p);
      }
    }
    profiles.sort_unstable();
    Some((max_profiles, profiles))
  }

  pub fn parse_button_action(
    payload: &[u8; NagaModel::REPORT_PAYLOAD_LEN],
  ) -> NagaHardwareAction {
    let action_type = payload[11];
    let data_len = usize::from(payload[12]).min(5);
    let data = &payload[13..18];

    match action_type {
      0x00 => NagaHardwareAction::Disabled,
      0x01 => {
        if let Some(button) = NagaMouseButton::from_wire(data[0]) {
          NagaHardwareAction::Mouse { button }
        } else {
          NagaHardwareAction::Raw {
            action_type,
            data: data[..data_len].to_vec(),
          }
        }
      }
      0x02 => {
        // Note: Naga firmware returns data_len == 1 on factory defaults even though mod is at [0]
        // and HID usage code is at [1] (data[0..2] = payload[13..15]).
        let mod_mask = data[0];
        let hid_code = data[1];
        let vk = HidKeyTable::hid_to_vk(hid_code).unwrap_or(0);
        NagaHardwareAction::Keyboard {
          vk,
          hid_code,
          ctrl: (mod_mask & 0x11) != 0,
          shift: (mod_mask & 0x22) != 0,
          alt: (mod_mask & 0x44) != 0,
          win: (mod_mask & 0x88) != 0,
        }
      }
      0x06 if data_len == 1 => {
        if let Some(op) = NagaDpiOp::from_wire(data[0]) {
          NagaHardwareAction::Dpi { op }
        } else {
          NagaHardwareAction::Raw {
            action_type,
            data: data[..data_len].to_vec(),
          }
        }
      }
      0x07 if data_len == 1 => NagaHardwareAction::Profile { op: data[0] },
      0x0A if data_len >= 2 => NagaHardwareAction::Media {
        usage: u16::from_be_bytes([data[0], data[1]]),
      },
      0x0C => NagaHardwareAction::Hypershift,
      _ => NagaHardwareAction::Raw {
        action_type,
        data: data[..data_len].to_vec(),
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn builds_valid_razer_crc_and_matches_response() {
    let req = RazerReport::battery_request();
    assert_eq!(req[0], 0x00);
    let payload: [u8; 90] = req[1..].try_into().unwrap();
    assert_eq!(payload[1], NagaModel::TX_ID);
    assert_eq!(payload[5], 0x02);
    assert_eq!(payload[6], 0x07);
    assert_eq!(payload[7], 0x80);
    assert_eq!(payload[88], RazerReport::compute_crc(&payload));
    assert!(RazerReport::matches_request(&req, &payload));
  }

  #[test]
  fn parses_battery_firmware_and_dpi() {
    assert_eq!(RazerReport::parse_battery_percent(0), 0);
    assert_eq!(RazerReport::parse_battery_percent(128), 50);
    assert_eq!(RazerReport::parse_battery_percent(255), 100);

    let mut fw = [0u8; 90];
    fw[8] = 1;
    fw[9] = 1;
    assert_eq!(RazerReport::parse_firmware(&fw).as_deref(), Some("v1.01"));

    let mut dpi = [0u8; 90];
    dpi[8] = 0x00;
    dpi[9] = 0x04;
    dpi[10] = 0xB0;
    dpi[11] = 0x04;
    dpi[12] = 0xB0;
    assert_eq!(
      RazerReport::parse_dpi(&dpi),
      Some(NagaDpi { x: 1200, y: 1200 })
    );
  }

  #[test]
  fn hid_table_roundtrips_common_and_extended_keys() {
    let samples = [
      (b'W' as u16, 0x1A),
      (b'1' as u16, 0x1E),
      (b'0' as u16, 0x27),
      (0x70, 0x3A), // F1
      (0x7C, 0x68), // F13
      (0x87, 0x73), // F24
      (0x20, 0x2C), // Space
      (0x0D, 0x28), // Enter
      (0x25, 0x50), // Left
    ];
    for (vk, hid) in samples {
      assert_eq!(HidKeyTable::vk_to_hid(vk), Some(hid));
      assert_eq!(HidKeyTable::hid_to_vk(hid), Some(vk));
    }
  }

  #[test]
  fn parses_button_actions_including_factory_keyboard_length_one_quirk() {
    let mut payload = [0u8; 90];
    // Factory keyboard quirk: action=02, len=01, mod=00, hid=0x1E ('1')
    payload[11] = 0x02;
    payload[12] = 0x01;
    payload[13] = 0x00;
    payload[14] = 0x1E;
    assert_eq!(
      RazerReport::parse_button_action(&payload),
      NagaHardwareAction::Keyboard {
        vk: b'1' as u16,
        hid_code: 0x1E,
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
      }
    );

    // Synapse-written Ctrl+W (len=02, mod=01, hid=0x1A)
    payload[12] = 0x02;
    payload[13] = 0x01;
    payload[14] = 0x1A;
    assert_eq!(
      RazerReport::parse_button_action(&payload),
      NagaHardwareAction::Keyboard {
        vk: b'W' as u16,
        hid_code: 0x1A,
        ctrl: true,
        shift: false,
        alt: false,
        win: false,
      }
    );

    // Mouse back button
    payload[11] = 0x01;
    payload[12] = 0x01;
    payload[13] = 0x04;
    payload[14] = 0x00;
    assert_eq!(
      RazerReport::parse_button_action(&payload),
      NagaHardwareAction::Mouse {
        button: NagaMouseButton::Back
      }
    );
  }

  #[test]
  fn encodes_button_request_and_dpi_stages_correctly() {
    let req = RazerReport::set_button_request(
      1,
      0x55,
      0,
      &NagaHardwareAction::Keyboard {
        vk: b'W' as u16,
        hid_code: 0,
        ctrl: true,
        shift: false,
        alt: true,
        win: false,
      },
    )
    .unwrap();
    let payload: [u8; 90] = req[1..].try_into().unwrap();
    assert_eq!(payload[5], 0x0A);
    assert_eq!(payload[6], 0x02);
    assert_eq!(payload[7], 0x0C);
    assert_eq!(
      &payload[8..15],
      &[0x01, 0x55, 0x00, 0x02, 0x02, 0x05, 0x1A]
    );
    assert_eq!(payload[88], RazerReport::compute_crc(&payload));

    let stages = vec![
      NagaDpi { x: 800, y: 800 },
      NagaDpi { x: 1600, y: 1600 },
    ];
    let stage_req = RazerReport::dpi_stages_set_request(1, 2, &stages);
    let stage_payload: [u8; 90] = stage_req[1..].try_into().unwrap();
    let parsed = RazerReport::parse_dpi_stages(&stage_payload).unwrap();
    assert_eq!(parsed.active_stage, 2);
    assert_eq!(parsed.stages, stages);
  }

  #[test]
  fn parses_profile_list_and_polling_rate() {
    let mut prof = [0u8; 90];
    prof[5] = 0x03; // max + 2 profiles
    prof[8] = 0x05;
    prof[9] = 0x01;
    prof[10] = 0x03;
    assert_eq!(RazerReport::parse_profile_list(&prof), Some((5, vec![1, 3])));

    let mut poll = [0u8; 90];
    poll[8] = 0x01;
    assert_eq!(RazerReport::parse_polling_rate(&poll), Some(1000));
    poll[8] = 0x02;
    assert_eq!(RazerReport::parse_polling_rate(&poll), Some(500));
    poll[8] = 0x08;
    assert_eq!(RazerReport::parse_polling_rate(&poll), Some(125));
  }

  #[test]
  fn writable_button_whitelist_never_allows_left_click() {
    assert!(!is_allowed_writable_button(0x01));
    assert!(!is_allowed_writable_button(0x02));
    assert!(!is_allowed_writable_button(0x03));
    assert!(is_allowed_writable_button(0x04));
    assert!(is_allowed_writable_button(0x05));
    assert!(is_allowed_writable_button(0x40));
    assert!(is_allowed_writable_button(0x4B));
    assert!(is_allowed_writable_button(0x50));
    assert!(is_allowed_writable_button(0x55));
  }
}
