use serde::{Deserialize, Deserializer, Serialize};

use crate::core::process_watcher::ProcessEnumerator;

pub const VK_KEY_H: u16 = 72;
pub const VK_KEY_J: u16 = 74;
pub const VK_KEY_K: u16 = 75;
pub const VK_KEY_L: u16 = 76;
pub const VK_KEY_P: u16 = 80;
pub const VK_KEY_Y: u16 = 89;
pub const VK_KEY_A: u16 = 65;
pub const VK_KEY_E: u16 = 69;
pub const VK_KEY_U: u16 = 85;
pub const VK_KEY_D: u16 = 68;
pub const VK_KEY_I: u16 = 73;
pub const VK_KEY_N: u16 = 78;
pub const VK_KEY_O: u16 = 79;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionalActionKind {
  Left,
  Right,
  Up,
  Down,
  WordForward,
  WordBack,
  Home,
  End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JumpDirection {
  Up,
  Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndependentActionKind {
  Backspace,
  NextLine,
  InsertQuotes,
  ToggleCapsLock,
  SwitchInputSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionConfig {
  Directional {
    action: DirectionalActionKind,
  },
  Jump {
    direction: JumpDirection,
    count: u8,
  },
  Independent {
    action: IndependentActionKind,
  },
  Command {
    command: String,
  },
  KeyCombo {
    target_key: u16,
    #[serde(default)]
    with_ctrl: bool,
    #[serde(default)]
    with_alt: bool,
    #[serde(default, alias = "with_cmd")]
    with_win: bool,
    #[serde(default, alias = "with_target_shift")]
    with_shift: bool,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierKey {
  LeftShift,
  RightShift,
  LeftControl,
  RightControl,
  #[serde(alias = "left_option")]
  LeftAlt,
  #[serde(alias = "right_option")]
  RightAlt,
  #[serde(alias = "left_command")]
  LeftWin,
  #[serde(alias = "right_command")]
  RightWin,
}

impl ModifierKey {
  pub fn from_vk(vk: u16) -> Option<Self> {
    match vk {
      0xA0 => Some(Self::LeftShift),
      0xA1 => Some(Self::RightShift),
      0xA2 => Some(Self::LeftControl),
      0xA3 => Some(Self::RightControl),
      0xA4 => Some(Self::LeftAlt),
      0xA5 => Some(Self::RightAlt),
      0x5B => Some(Self::LeftWin),
      0x5C => Some(Self::RightWin),
      _ => None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
  HyperPlusKey {
    key: u16,
    #[serde(default)]
    with_shift: bool,
  },
  SingleTapHyper,
  DoubleTapHyper,
  DoubleTapModifier {
    modifier: ModifierKey,
  },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionMappingEntry {
  pub trigger: Trigger,
  pub action: ActionConfig,
}

impl<'de> Deserialize<'de> for ActionMappingEntry {
  fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
    #[derive(Deserialize)]
    struct Raw {
      #[serde(default)]
      trigger: Option<Trigger>,
      #[serde(default)]
      key: Option<u16>,
      #[serde(default)]
      with_shift: Option<bool>,
      action: ActionConfig,
    }

    let raw = Raw::deserialize(de)?;
    let trigger = if let Some(t) = raw.trigger {
      t
    } else if let Some(key) = raw.key {
      Trigger::HyperPlusKey {
        key,
        with_shift: raw.with_shift.unwrap_or(false),
      }
    } else {
      return Err(serde::de::Error::custom(
        "action mapping entry missing both 'trigger' and legacy 'key' fields",
      ));
    };

    Ok(Self {
      trigger,
      action: raw.action,
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HyperCapslockConfig {
  pub paused: bool,
  pub auto_pause_enabled: bool,
  pub auto_pause_processes: Vec<String>,
  pub auto_pause_notify_sound: bool,
  pub mappings: Vec<ActionMappingEntry>,
}

impl Default for HyperCapslockConfig {
  fn default() -> Self {
    Self {
      paused: false,
      auto_pause_enabled: true,
      auto_pause_processes: Vec::new(),
      auto_pause_notify_sound: true,
      mappings: default_action_mappings(),
    }
  }
}

impl HyperCapslockConfig {
  pub fn sanitize(&mut self) {
    normalize_mappings(&mut self.mappings);
    normalize_auto_pause_processes(&mut self.auto_pause_processes);
  }
}

pub fn normalize_auto_pause_processes(list: &mut Vec<String>) {
  let mut out: Vec<String> = Vec::new();
  for raw in list.drain(..) {
    if let Some(name) = ProcessEnumerator::normalize_process_name(&raw) {
      if !out.contains(&name) {
        out.push(name);
      }
    }
  }
  *list = out;
}

pub fn add_auto_pause_process(list: &mut Vec<String>, raw: &str) -> Result<String, String> {
  let name = ProcessEnumerator::normalize_process_name(raw)
    .ok_or_else(|| "Invalid process name".to_string())?;
  if !list.contains(&name) {
    list.push(name.clone());
  }
  Ok(name)
}

pub fn remove_auto_pause_process(list: &mut Vec<String>, raw: &str) -> bool {
  let target = ProcessEnumerator::normalize_process_name(raw)
    .unwrap_or_else(|| raw.trim().to_ascii_lowercase());
  let before = list.len();
  list.retain(|p| p.to_ascii_lowercase() != target);
  list.len() != before
}

pub fn default_action_mappings() -> Vec<ActionMappingEntry> {
  vec![
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_H,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::Left,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_J,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::Down,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_K,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::Up,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_L,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::Right,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_P,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::WordForward,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_Y,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::WordBack,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_A,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::Home,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_E,
        with_shift: false,
      },
      action: ActionConfig::Directional {
        action: DirectionalActionKind::End,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_U,
        with_shift: false,
      },
      action: ActionConfig::Jump {
        direction: JumpDirection::Up,
        count: 10,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_D,
        with_shift: false,
      },
      action: ActionConfig::Jump {
        direction: JumpDirection::Down,
        count: 10,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_I,
        with_shift: false,
      },
      action: ActionConfig::Independent {
        action: IndependentActionKind::Backspace,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_N,
        with_shift: false,
      },
      action: ActionConfig::Independent {
        action: IndependentActionKind::InsertQuotes,
      },
    },
    ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_O,
        with_shift: false,
      },
      action: ActionConfig::Independent {
        action: IndependentActionKind::NextLine,
      },
    },
  ]
}

pub fn validate_action(action: &ActionConfig) -> Result<(), String> {
  match action {
    ActionConfig::Command { command } if command.trim().is_empty() => {
      Err("Command cannot be empty".to_string())
    }
    ActionConfig::Jump { count, .. } if *count == 0 => {
      Err("Jump count must be at least 1".to_string())
    }
    ActionConfig::KeyCombo { target_key, .. } if *target_key == 0 => {
      Err("Target key must be specified".to_string())
    }
    _ => Ok(()),
  }
}

pub fn upsert_mapping(mappings: &mut Vec<ActionMappingEntry>, entry: ActionMappingEntry) {
  if let Some(existing) = mappings.iter_mut().find(|m| m.trigger == entry.trigger) {
    *existing = entry;
  } else {
    mappings.push(entry);
  }
}

pub fn remove_mapping(mappings: &mut Vec<ActionMappingEntry>, trigger: &Trigger) -> bool {
  let before = mappings.len();
  mappings.retain(|m| m.trigger != *trigger);
  mappings.len() != before
}

pub fn normalize_mappings(mappings: &mut Vec<ActionMappingEntry>) {
  let mut deduped: Vec<ActionMappingEntry> = Vec::new();
  for entry in mappings.drain(..) {
    if let Some(existing) = deduped.iter_mut().find(|m| m.trigger == entry.trigger) {
      *existing = entry;
    } else {
      deduped.push(entry);
    }
  }
  *mappings = deduped;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn defaults_contain_all_classic_vim_and_editing_bindings() {
    let cfg = HyperCapslockConfig::default();
    assert!(!cfg.paused);
    assert_eq!(cfg.mappings.len(), 13);
    assert!(cfg.mappings.iter().any(|m| {
      m.trigger
        == Trigger::HyperPlusKey {
          key: VK_KEY_H,
          with_shift: false,
        }
        && matches!(
          m.action,
          ActionConfig::Directional {
            action: DirectionalActionKind::Left
          }
        )
    }));
  }

  #[test]
  fn legacy_aliases_and_fields_deserialize_cleanly() {
    let legacy = r#"
      {
        "key": 72,
        "with_shift": false,
        "action": {
          "kind": "key_combo",
          "target_key": 67,
          "with_ctrl": true,
          "with_cmd": true,
          "with_target_shift": false
        }
      }
    "#;
    let parsed: ActionMappingEntry = serde_json::from_str(legacy).unwrap();
    assert_eq!(
      parsed.trigger,
      Trigger::HyperPlusKey {
        key: 72,
        with_shift: false
      }
    );
    assert_eq!(
      parsed.action,
      ActionConfig::KeyCombo {
        target_key: 67,
        with_ctrl: true,
        with_alt: false,
        with_win: true,
        with_shift: false,
      }
    );

    let mod_json = r#"{"kind": "double_tap_modifier", "modifier": "left_command"}"#;
    let trig: Trigger = serde_json::from_str(mod_json).unwrap();
    assert_eq!(
      trig,
      Trigger::DoubleTapModifier {
        modifier: ModifierKey::LeftWin
      }
    );
  }

  #[test]
  fn auto_pause_defaults_and_legacy_config_deserialize() {
    let cfg = HyperCapslockConfig::default();
    assert!(cfg.auto_pause_enabled);
    assert!(cfg.auto_pause_notify_sound);
    assert!(cfg.auto_pause_processes.is_empty());

    let legacy: HyperCapslockConfig = serde_json::from_str(r#"{"paused": true}"#).unwrap();
    assert!(legacy.paused);
    assert!(legacy.auto_pause_enabled);
    assert!(legacy.auto_pause_processes.is_empty());
    assert_eq!(legacy.mappings.len(), 13);
  }

  #[test]
  fn auto_pause_sanitize_normalizes_and_dedups() {
    let mut cfg = HyperCapslockConfig::default();
    cfg.auto_pause_processes = vec![
      "Steam.exe".to_string(),
      "steam".to_string(),
      "  ".to_string(),
      "CS2".to_string(),
    ];
    cfg.sanitize();
    assert_eq!(
      cfg.auto_pause_processes,
      vec!["steam.exe".to_string(), "cs2.exe".to_string()]
    );
  }

  #[test]
  fn auto_pause_add_and_remove() {
    let mut list: Vec<String> = Vec::new();
    assert_eq!(
      add_auto_pause_process(&mut list, "\"C:\\Games\\CS2.exe\""),
      Ok("cs2.exe".to_string())
    );
    assert_eq!(
      add_auto_pause_process(&mut list, "cs2"),
      Ok("cs2.exe".to_string())
    );
    assert_eq!(list.len(), 1);
    assert!(add_auto_pause_process(&mut list, "  ").is_err());
    assert!(remove_auto_pause_process(&mut list, "CS2.EXE"));
    assert!(!remove_auto_pause_process(&mut list, "cs2.exe"));
    assert!(list.is_empty());
  }

  #[test]
  fn normalize_dedups_by_trigger_keeping_latest() {
    let mut list = vec![
      ActionMappingEntry {
        trigger: Trigger::SingleTapHyper,
        action: ActionConfig::Independent {
          action: IndependentActionKind::ToggleCapsLock,
        },
      },
      ActionMappingEntry {
        trigger: Trigger::SingleTapHyper,
        action: ActionConfig::Independent {
          action: IndependentActionKind::SwitchInputSource,
        },
      },
    ];
    normalize_mappings(&mut list);
    assert_eq!(list.len(), 1);
    assert_eq!(
      list[0].action,
      ActionConfig::Independent {
        action: IndependentActionKind::SwitchInputSource
      }
    );
  }
}
