use serde::{Deserialize, Serialize};

use crate::core::vk::KeyShortcut;

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
  pub fn button_count(self) -> u8 {
    match self {
      Self::TwoButton => 2,
      Self::SixButton => 6,
      Self::TwelveButton => 12,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NagaButtonMapping {
  pub index: u8,
  pub sentinel_vk: u16,
  #[serde(default)]
  pub target: Option<KeyShortcut>,
  #[serde(default = "default_true")]
  pub enabled: bool,
}

fn default_true() -> bool {
  true
}

impl NagaButtonMapping {
  pub fn default_sentinel_for_index(index: u8) -> u16 {
    // VK_F13 = 0x7C (124) .. VK_F24 = 0x87 (135)
    0x7C + u16::from(index.saturating_sub(1).min(11))
  }

  pub fn defaults_for_count(count: u8) -> Vec<Self> {
    (1..=count)
      .map(|idx| Self {
        index: idx,
        sentinel_vk: Self::default_sentinel_for_index(idx),
        target: None,
        enabled: true,
      })
      .collect()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NagaConfig {
  pub enabled: bool,
  pub plate_layout: NagaPlateLayout,
  pub mappings: Vec<NagaButtonMapping>,
}

impl Default for NagaConfig {
  fn default() -> Self {
    let plate_layout = NagaPlateLayout::SixButton;
    Self {
      enabled: true,
      plate_layout,
      mappings: NagaButtonMapping::defaults_for_count(plate_layout.button_count()),
    }
  }
}

impl NagaConfig {
  pub fn set_plate_layout(&mut self, layout: NagaPlateLayout) {
    self.plate_layout = layout;
    self.sync_mappings_to_layout();
  }

  pub fn sanitize(&mut self) {
    self.sync_mappings_to_layout();
  }

  fn sync_mappings_to_layout(&mut self) {
    let count = self.plate_layout.button_count();
    let existing = std::mem::take(&mut self.mappings);
    let mut next = Vec::with_capacity(usize::from(count));

    for idx in 1..=count {
      if let Some(prev) = existing.iter().find(|m| m.index == idx) {
        let sentinel = if prev.sentinel_vk == 0 {
          NagaButtonMapping::default_sentinel_for_index(idx)
        } else {
          prev.sentinel_vk
        };
        next.push(NagaButtonMapping {
          index: idx,
          sentinel_vk: sentinel,
          target: prev.target.clone(),
          enabled: prev.enabled,
        });
      } else {
        next.push(NagaButtonMapping {
          index: idx,
          sentinel_vk: NagaButtonMapping::default_sentinel_for_index(idx),
          target: None,
          enabled: true,
        });
      }
    }

    self.mappings = next;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn defaults_create_six_button_f13_to_f18_layout() {
    let cfg = NagaConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.plate_layout, NagaPlateLayout::SixButton);
    assert_eq!(cfg.mappings.len(), 6);
    assert_eq!(cfg.mappings[0].index, 1);
    assert_eq!(cfg.mappings[0].sentinel_vk, 0x7C); // F13
    assert_eq!(cfg.mappings[5].index, 6);
    assert_eq!(cfg.mappings[5].sentinel_vk, 0x81); // F18
  }

  #[test]
  fn switching_plate_layout_preserves_existing_mappings() {
    let mut cfg = NagaConfig::default();
    cfg.mappings[1].target = Some(KeyShortcut {
      vk: 67,
      ctrl: true,
      alt: false,
      shift: false,
      win: false,
    });
    cfg.mappings[1].enabled = false;

    cfg.set_plate_layout(NagaPlateLayout::TwelveButton);
    assert_eq!(cfg.mappings.len(), 12);
    assert_eq!(cfg.mappings[1].target.as_ref().map(|t| t.vk), Some(67));
    assert!(!cfg.mappings[1].enabled);
    assert_eq!(cfg.mappings[11].index, 12);
    assert_eq!(cfg.mappings[11].sentinel_vk, 0x87); // F24

    cfg.set_plate_layout(NagaPlateLayout::TwoButton);
    assert_eq!(cfg.mappings.len(), 2);
    assert_eq!(cfg.mappings[1].target.as_ref().map(|t| t.vk), Some(67));
  }
}
