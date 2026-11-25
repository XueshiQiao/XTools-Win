use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use super::file_log::FileLog;
use crate::tools::hyper_capslock::config::HyperCapslockConfig;
use crate::tools::razer_naga::config::NagaConfig;

const LOG: FileLog = FileLog::new("Preferences");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RogKeyboardConfig {
  /// Default Windows profile slot: 1 ("配置文件 1").
  pub windows_profile: usize,
  pub auto_switch_enabled: bool,
  pub sound_enabled: bool,
}

impl Default for RogKeyboardConfig {
  fn default() -> Self {
    Self {
      windows_profile: 1,
      auto_switch_enabled: true,
      sound_enabled: true,
    }
  }
}

impl RogKeyboardConfig {
  /// Never allow slot 0 to reach the keyboard firmware (slot 0 silently
  /// selects the last profile instead of failing).
  pub fn sanitize(&mut self) {
    if self.windows_profile == 0 {
      self.windows_profile = 1;
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct AppConfig {
  /// `None` follows system locale; `Some("zh-Hans")` or `Some("en")` overrides it.
  pub language: Option<String>,
  pub rog_keyboard: RogKeyboardConfig,
  pub hyper_capslock: HyperCapslockConfig,
  pub razer_naga: NagaConfig,
}

impl AppConfig {
  pub fn sanitize(&mut self) {
    self.rog_keyboard.sanitize();
    self.hyper_capslock.sanitize();
    self.razer_naga.sanitize();
    if let Some(lang) = &self.language {
      let trimmed = lang.trim();
      if trimmed.is_empty() {
        self.language = None;
      } else if trimmed != "zh-Hans" && trimmed != "en" {
        self.language = None;
      }
    }
  }
}

pub struct PreferencesStore {
  path: PathBuf,
  config: RwLock<AppConfig>,
}

impl PreferencesStore {
  pub fn default_config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    base.join("XTools").join("config.json")
  }

  pub fn load() -> Self {
    Self::with_path(Self::default_config_path())
  }

  pub fn with_path(path: PathBuf) -> Self {
    let mut cfg = AppConfig::default();
    if path.exists() {
      match fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<AppConfig>(&raw) {
          Ok(parsed) => cfg = parsed,
          Err(err) => LOG.warn(&format!("Failed to parse {}: {}", path.display(), err)),
        },
        Err(err) => LOG.warn(&format!("Failed to read {}: {}", path.display(), err)),
      }
    }
    cfg.sanitize();
    Self {
      path,
      config: RwLock::new(cfg),
    }
  }

  pub fn get(&self) -> AppConfig {
    self
      .config
      .read()
      .map(|guard| guard.clone())
      .unwrap_or_default()
  }

  pub fn update<F>(&self, mutator: F) -> Result<AppConfig, String>
  where
    F: FnOnce(&mut AppConfig),
  {
    let snapshot = {
      let mut guard = self
        .config
        .write()
        .map_err(|_| "Preferences lock poisoned".to_string())?;
      mutator(&mut guard);
      guard.sanitize();
      guard.clone()
    };
    self.persist(&snapshot)?;
    Ok(snapshot)
  }

  fn persist(&self, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = self.path.parent() {
      fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(config)
      .map_err(|e| format!("Failed to serialize config: {}", e))?;
    let tmp = self.path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes())
      .map_err(|e| format!("Failed to write temp config: {}", e))?;
    if self.path.exists() {
      let _ = fs::remove_file(&self.path);
    }
    fs::rename(&tmp, &self.path)
      .map_err(|e| format!("Failed to replace config file: {}", e))?;
    Ok(())
  }

  pub fn effective_is_zh(&self) -> bool {
    match self.get().language.as_deref() {
      Some("zh-Hans") => true,
      Some("en") => false,
      _ => system_is_chinese(),
    }
  }
}

pub fn system_is_chinese() -> bool {
  #[cfg(target_os = "windows")]
  {
    // Check user UI language via GetUserDefaultUILanguage (LANG_CHINESE = 0x04)
    #[link(name = "kernel32")]
    extern "system" {
      fn GetUserDefaultUILanguage() -> u16;
    }
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    (lang_id & 0x03FF) == 0x04
  }
  #[cfg(not(target_os = "windows"))]
  {
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn temp_config_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("xtools_tests");
    let _ = fs::create_dir_all(&dir);
    dir.join(format!("{}_{}.json", name, std::process::id()))
  }

  #[test]
  fn defaults_use_windows_profile_1() {
    let path = temp_config_path("defaults");
    let _ = fs::remove_file(&path);
    let store = PreferencesStore::with_path(path.clone());
    let cfg = store.get();
    assert_eq!(cfg.rog_keyboard.windows_profile, 1);
    assert!(cfg.rog_keyboard.auto_switch_enabled);
    assert!(cfg.rog_keyboard.sound_enabled);
    assert_eq!(cfg.language, None);
    let _ = fs::remove_file(&path);
  }

  #[test]
  fn zero_profile_slots_are_sanitized_on_load() {
    let path = temp_config_path("sanitize_zero");
    let raw = r#"{
      "language": "zh-Hans",
      "rog_keyboard": {
        "windows_profile": 0,
        "auto_switch_enabled": false,
        "sound_enabled": true
      }
    }"#;
    fs::write(&path, raw).unwrap();
    let store = PreferencesStore::with_path(path.clone());
    let cfg = store.get();
    assert_eq!(cfg.rog_keyboard.windows_profile, 1);
    assert!(!cfg.rog_keyboard.auto_switch_enabled);
    assert_eq!(cfg.language.as_deref(), Some("zh-Hans"));
    let _ = fs::remove_file(&path);
  }
}
