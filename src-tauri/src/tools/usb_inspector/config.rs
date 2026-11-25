use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_LABEL_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UsbInspectorConfig {
  pub port_labels: BTreeMap<String, String>,
  pub show_empty_ports: bool,
}

impl Default for UsbInspectorConfig {
  fn default() -> Self {
    Self {
      port_labels: BTreeMap::new(),
      show_empty_ports: true,
    }
  }
}

impl UsbInspectorConfig {
  pub fn sanitize(&mut self) {
    let mut cleaned = BTreeMap::new();
    for (raw_key, raw_val) in &self.port_labels {
      let key = raw_key.trim().to_ascii_lowercase();
      let val: String = raw_val.trim().chars().take(MAX_LABEL_CHARS).collect();
      if !key.is_empty() && !val.is_empty() {
        cleaned.insert(key, val);
      }
    }
    self.port_labels = cleaned;
  }

  pub fn set_label(&mut self, port_key: &str, label: &str) {
    let key = port_key.trim().to_ascii_lowercase();
    if key.is_empty() {
      return;
    }
    let val: String = label.trim().chars().take(MAX_LABEL_CHARS).collect();
    if val.is_empty() {
      self.port_labels.remove(&key);
    } else {
      self.port_labels.insert(key, val);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn defaults_enable_empty_ports_and_have_no_labels() {
    let cfg = UsbInspectorConfig::default();
    assert!(cfg.show_empty_ports);
    assert!(cfg.port_labels.is_empty());
  }

  #[test]
  fn sanitize_trims_lowercases_keys_and_drops_empty_entries() {
    let mut cfg = UsbInspectorConfig {
      show_empty_ports: false,
      port_labels: BTreeMap::from([
        ("  Ctrl0/Root/P1 ".to_string(), "  背面蓝口 #1  ".to_string()),
        ("empty_val".to_string(), "   ".to_string()),
        ("   ".to_string(), "ignored".to_string()),
        ("long_key".to_string(), "a".repeat(100)),
      ]),
    };
    cfg.sanitize();
    assert_eq!(cfg.port_labels.len(), 2);
    assert_eq!(
      cfg.port_labels.get("ctrl0/root/p1").map(String::as_str),
      Some("背面蓝口 #1")
    );
    assert_eq!(
      cfg.port_labels.get("long_key").map(|s| s.chars().count()),
      Some(MAX_LABEL_CHARS)
    );
  }
}
