use serde::{Deserialize, Serialize};

pub struct NagaModel;

impl NagaModel {
  pub const VENDOR_ID: u16 = 0x1532;
  pub const BT_VENDOR_ID: u16 = 0x068E;
  pub const PID_WIRED: u16 = 0x00E7;
  pub const PID_WIRELESS: u16 = 0x00E8;
  pub const ALL_PRODUCT_IDS: [u16; 2] = [Self::PID_WIRED, Self::PID_WIRELESS];

  pub const CONTROL_USAGE_PAGE: u16 = 0x0001;
  pub const CONTROL_USAGE: u16 = 0x0003;
  pub const FEATURE_REPORT_LEN: usize = 91;
  pub const REPORT_PAYLOAD_LEN: usize = 90;
  pub const TX_ID: u8 = 0x1F;
  pub const DISPLAY_NAME: &'static str = "Razer Naga V3 Pro";
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NagaDeviceState {
  pub link: NagaLink,
  pub firmware_version: Option<String>,
  pub battery: Option<NagaBatteryInfo>,
  pub dpi: Option<NagaDpi>,
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
}
