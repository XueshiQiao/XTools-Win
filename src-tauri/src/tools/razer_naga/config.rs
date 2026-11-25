use serde::{Deserialize, Serialize};

use super::protocol::{HidKeyTable, NagaHardwareAction, NagaMouseButton};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NagaPlateLayout {
  TwoButton,
  SixButton,
  TwelveButton,
}

impl Default for NagaPlateLayout {
  fn default() -> Self {
    Self::SixButton
  }
}

impl NagaPlateLayout {
  const TWO_IDS: [u8; 2] = [0x04, 0x05];
  const SIX_IDS: [u8; 6] = [0x50, 0x51, 0x52, 0x53, 0x54, 0x55];
  const TWELVE_IDS: [u8; 12] = [
    0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B,
  ];

  pub fn button_ids(self) -> &'static [u8] {
    match self {
      Self::TwoButton => &Self::TWO_IDS,
      Self::SixButton => &Self::SIX_IDS,
      Self::TwelveButton => &Self::TWELVE_IDS,
    }
  }

  pub fn factory_default_action(self, button_id: u8) -> NagaHardwareAction {
    match self {
      Self::TwoButton => match button_id {
        0x04 => NagaHardwareAction::Mouse {
          button: NagaMouseButton::Back,
        },
        0x05 => NagaHardwareAction::Mouse {
          button: NagaMouseButton::Forward,
        },
        _ => NagaHardwareAction::Disabled,
      },
      Self::SixButton => {
        let idx = button_id.saturating_sub(0x50).min(5);
        let hid_code = 0x1E + idx; // Keys 1..=6
        NagaHardwareAction::Keyboard {
          vk: HidKeyTable::hid_to_vk(hid_code).unwrap_or(0),
          hid_code,
          ctrl: false,
          shift: false,
          alt: false,
          win: false,
        }
      }
      Self::TwelveButton => {
        let idx = button_id.saturating_sub(0x40).min(11);
        let hid_code = match idx {
          0..=9 => 0x1E + idx, // 1..=0
          10 => 0x2D,          // -
          _ => 0x2E,           // =
        };
        NagaHardwareAction::Keyboard {
          vk: HidKeyTable::hid_to_vk(hid_code).unwrap_or(0),
          hid_code,
          ctrl: false,
          shift: false,
          alt: false,
          win: false,
        }
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NagaConfig {
  pub plate_layout: NagaPlateLayout,
}

impl NagaConfig {
  pub fn sanitize(&mut self) {}
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn plate_layouts_return_expected_button_ids_and_defaults() {
    assert_eq!(NagaPlateLayout::TwoButton.button_ids(), &[0x04, 0x05]);
    assert_eq!(
      NagaPlateLayout::SixButton.button_ids(),
      &[0x50, 0x51, 0x52, 0x53, 0x54, 0x55]
    );
    assert_eq!(NagaPlateLayout::TwelveButton.button_ids().len(), 12);

    assert_eq!(
      NagaPlateLayout::TwoButton.factory_default_action(0x04),
      NagaHardwareAction::Mouse {
        button: NagaMouseButton::Back
      }
    );
    assert_eq!(
      NagaPlateLayout::SixButton.factory_default_action(0x50),
      NagaHardwareAction::Keyboard {
        vk: b'1' as u16,
        hid_code: 0x1E,
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
      }
    );
    assert_eq!(
      NagaPlateLayout::TwelveButton.factory_default_action(0x4B),
      NagaHardwareAction::Keyboard {
        vk: 0xBB,
        hid_code: 0x2E,
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
      }
    );
  }

  #[test]
  fn legacy_config_deserializes_cleanly() {
    let legacy = r#"{"enabled": true, "plate_layout": "twelve_button", "mappings": []}"#;
    let cfg: NagaConfig = serde_json::from_str(legacy).unwrap();
    assert_eq!(cfg.plate_layout, NagaPlateLayout::TwelveButton);
  }
}
