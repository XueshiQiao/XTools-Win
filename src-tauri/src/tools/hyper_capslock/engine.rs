use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::core::keyboard_hook::{HookDecision, LowLevelKeyEvent};
use crate::core::vk::{KeyShortcut, KeySynthesizer, VkHelper};

use super::config::{
  ActionConfig, ActionMappingEntry, DirectionalActionKind, IndependentActionKind, JumpDirection,
  ModifierKey, Trigger,
};

pub const CAPS_TAP_MAX_MS: u64 = 200;
pub const DOUBLE_TAP_WINDOW_MS: u64 = 280;
pub const MODIFIER_TAP_MAX_MS: u64 = 220;

const VK_CAPITAL_U16: u16 = 0x14;
const VK_SHIFT_U16: u16 = 0x10;
const VK_BACK_U16: u16 = 0x08;
const VK_END_U16: u16 = 0x23;
const VK_HOME_U16: u16 = 0x24;
const VK_LEFT_U16: u16 = 0x25;
const VK_UP_U16: u16 = 0x26;
const VK_RIGHT_U16: u16 = 0x27;
const VK_DOWN_U16: u16 = 0x28;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActiveRemap {
  HeldKey(u16),
  HeldCtrlArrow(u16),
  SwallowUpOnly,
}

#[derive(Debug, Default)]
struct ModifierTapTracker {
  active_modifier: Option<(ModifierKey, u64)>,
  last_tap: Option<(ModifierKey, u64)>,
}

impl ModifierTapTracker {
  fn reset_all(&mut self) {
    self.active_modifier = None;
    self.last_tap = None;
  }

  fn on_key_event(
    &mut self,
    vk: u16,
    is_down: bool,
    is_up: bool,
    ts_ms: u64,
    mappings: &[ActionMappingEntry],
  ) -> Option<ActionConfig> {
    if !VkHelper::is_modifier_vk(vk) {
      if is_down {
        self.reset_all();
      }
      return None;
    }

    let Some(mod_key) = ModifierKey::from_vk(vk) else {
      if is_down {
        self.reset_all();
      }
      return None;
    };

    if is_down {
      match self.active_modifier {
        Some((active, _)) if active == mod_key => {
          // Autorepeat while modifier is held: ignore without cancelling.
        }
        Some(_) => {
          // Another modifier pressed concurrently: abort tap tracking.
          self.reset_all();
        }
        None => {
          self.active_modifier = Some((mod_key, ts_ms));
        }
      }
      return None;
    }

    if is_up {
      if let Some((active, down_ts)) = self.active_modifier.take() {
        if active == mod_key && ts_ms.saturating_sub(down_ts) <= MODIFIER_TAP_MAX_MS {
          if let Some((prev_mod, prev_ts)) = self.last_tap.take() {
            if prev_mod == mod_key && ts_ms.saturating_sub(prev_ts) <= DOUBLE_TAP_WINDOW_MS {
              return mappings
                .iter()
                .find(|m| m.trigger == Trigger::DoubleTapModifier { modifier: mod_key })
                .map(|m| m.action.clone());
            }
          }
          self.last_tap = Some((mod_key, ts_ms));
          return None;
        }
      }
      self.reset_all();
    }

    None
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEffect {
  SendKey {
    vk: u16,
    up: bool,
  },
  SendCtrlArrow {
    arrow_vk: u16,
    up: bool,
  },
  ExecuteAction {
    action: ActionConfig,
    release_shift: bool,
  },
  ToggleNativeCapsLock,
  ScheduleSingleTapFallback {
    gen: u64,
    delay_ms: u64,
    action: Option<ActionConfig>,
  },
}

struct EngineState {
  paused: bool,
  mappings: Vec<ActionMappingEntry>,
  caps_down: bool,
  caps_pressed_at_ms: u64,
  did_remap: bool,
  last_caps_tap_ms: u64,
  pending_single_tap_gen: u64,
  active_remapped_keys: HashMap<u16, ActiveRemap>,
  modifier_tracker: ModifierTapTracker,
}

impl EngineState {
  fn find_hyper_plus_key(&self, vk: u16, shift_down: bool) -> Option<ActionConfig> {
    if shift_down {
      if let Some(entry) = self.mappings.iter().find(|m| {
        matches!(
          m.trigger,
          Trigger::HyperPlusKey {
            key,
            with_shift: true
          } if key == vk
        )
      }) {
        return Some(entry.action.clone());
      }
    }

    self
      .mappings
      .iter()
      .find(|m| {
        matches!(
          m.trigger,
          Trigger::HyperPlusKey {
            key,
            with_shift: false
          } if key == vk
        )
      })
      .map(|m| m.action.clone())
  }

  fn find_single_tap_hyper(&self) -> Option<ActionConfig> {
    self
      .mappings
      .iter()
      .find(|m| m.trigger == Trigger::SingleTapHyper)
      .map(|m| m.action.clone())
  }

  fn find_double_tap_hyper(&self) -> Option<ActionConfig> {
    self
      .mappings
      .iter()
      .find(|m| m.trigger == Trigger::DoubleTapHyper)
      .map(|m| m.action.clone())
  }
}

pub struct HyperCapslockEngine {
  state: Mutex<EngineState>,
}

impl HyperCapslockEngine {
  pub fn new(paused: bool, mappings: Vec<ActionMappingEntry>) -> Arc<Self> {
    Arc::new(Self {
      state: Mutex::new(EngineState {
        paused,
        mappings,
        caps_down: false,
        caps_pressed_at_ms: 0,
        did_remap: false,
        last_caps_tap_ms: 0,
        pending_single_tap_gen: 0,
        active_remapped_keys: HashMap::new(),
        modifier_tracker: ModifierTapTracker::default(),
      }),
    })
  }

  pub fn set_paused(&self, paused: bool) {
    let mut s = self.state.lock().unwrap();
    s.paused = paused;
    s.caps_down = false;
    s.caps_pressed_at_ms = 0;
    s.did_remap = false;
    s.last_caps_tap_ms = 0;
    s.pending_single_tap_gen = s.pending_single_tap_gen.wrapping_add(1);
    s.active_remapped_keys.clear();
    s.modifier_tracker.reset_all();
  }

  pub fn update_mappings(&self, mappings: Vec<ActionMappingEntry>) {
    let mut s = self.state.lock().unwrap();
    s.mappings = mappings;
  }

  pub fn process_key_event(
    &self,
    ev: &LowLevelKeyEvent,
    shift_down: bool,
  ) -> (HookDecision, Vec<EngineEffect>) {
    let mut effects: Vec<EngineEffect> = Vec::new();
    let mut s = self.state.lock().unwrap();

    if s.paused {
      return (HookDecision::Pass, effects);
    }

    // Release any synthetic key started while Caps was held, even if Caps was released first.
    if ev.is_up {
      if let Some(active) = s.active_remapped_keys.remove(&ev.vk) {
        match active {
          ActiveRemap::HeldKey(target_vk) => {
            effects.push(EngineEffect::SendKey {
              vk: target_vk,
              up: true,
            });
          }
          ActiveRemap::HeldCtrlArrow(arrow_vk) => {
            effects.push(EngineEffect::SendCtrlArrow { arrow_vk, up: true });
          }
          ActiveRemap::SwallowUpOnly => {}
        }
        return (HookDecision::Swallow, effects);
      }
    }

    if ev.vk == VK_CAPITAL_U16 {
      s.modifier_tracker.reset_all();
      if ev.is_down {
        if !s.caps_down {
          s.caps_down = true;
          s.caps_pressed_at_ms = ev.timestamp_ms;
          s.did_remap = false;
        }
        return (HookDecision::Swallow, effects);
      }
      if ev.is_up {
        let was_down = s.caps_down;
        s.caps_down = false;
        let pressed_at = s.caps_pressed_at_ms;
        s.caps_pressed_at_ms = 0;
        let held_ms = ev.timestamp_ms.saturating_sub(pressed_at);
        let did_remap = s.did_remap;

        if was_down && !did_remap && held_ms <= CAPS_TAP_MAX_MS {
          let single_action = s.find_single_tap_hyper();
          let double_action = s.find_double_tap_hyper();

          if let Some(d_act) = double_action {
            let within_double_window = s.last_caps_tap_ms > 0
              && ev.timestamp_ms.saturating_sub(s.last_caps_tap_ms) <= DOUBLE_TAP_WINDOW_MS;
            if within_double_window {
              s.last_caps_tap_ms = 0;
              s.pending_single_tap_gen = s.pending_single_tap_gen.wrapping_add(1);
              effects.push(EngineEffect::ExecuteAction {
                action: d_act,
                release_shift: false,
              });
            } else {
              s.last_caps_tap_ms = ev.timestamp_ms;
              s.pending_single_tap_gen = s.pending_single_tap_gen.wrapping_add(1);
              let gen = s.pending_single_tap_gen;
              effects.push(EngineEffect::ScheduleSingleTapFallback {
                gen,
                delay_ms: DOUBLE_TAP_WINDOW_MS,
                action: single_action,
              });
            }
          } else {
            s.last_caps_tap_ms = 0;
            if let Some(act) = single_action {
              effects.push(EngineEffect::ExecuteAction {
                action: act,
                release_shift: false,
              });
            } else {
              effects.push(EngineEffect::ToggleNativeCapsLock);
            }
          }
        } else {
          s.last_caps_tap_ms = 0;
        }
        return (HookDecision::Swallow, effects);
      }
      return (HookDecision::Swallow, effects);
    }

    if s.caps_down {
      s.modifier_tracker.reset_all();
      if VkHelper::is_modifier_vk(ev.vk) {
        return (HookDecision::Pass, effects);
      }

      if ev.is_down {
        if let Some(action) = s.find_hyper_plus_key(ev.vk, shift_down) {
          s.did_remap = true;
          match action {
            ActionConfig::Directional {
              action: dir @ (DirectionalActionKind::Left
              | DirectionalActionKind::Right
              | DirectionalActionKind::Up
              | DirectionalActionKind::Down
              | DirectionalActionKind::Home
              | DirectionalActionKind::End),
            } => {
              let target_vk = Self::directional_to_vk(dir);
              s.active_remapped_keys
                .insert(ev.vk, ActiveRemap::HeldKey(target_vk));
              effects.push(EngineEffect::SendKey {
                vk: target_vk,
                up: false,
              });
            }
            ActionConfig::Directional {
              action: dir @ (DirectionalActionKind::WordForward | DirectionalActionKind::WordBack),
            } => {
              let arrow_vk = Self::directional_to_vk(dir);
              let is_repeat = s.active_remapped_keys.contains_key(&ev.vk);
              s.active_remapped_keys
                .insert(ev.vk, ActiveRemap::HeldCtrlArrow(arrow_vk));
              if is_repeat {
                // While held, send repeated arrow presses with Ctrl already down or tap combo cleanly.
                effects.push(EngineEffect::SendCtrlArrow {
                  arrow_vk,
                  up: false,
                });
              } else {
                effects.push(EngineEffect::SendCtrlArrow {
                  arrow_vk,
                  up: false,
                });
              }
            }
            ActionConfig::Independent {
              action: IndependentActionKind::Backspace,
            } => {
              s.active_remapped_keys
                .insert(ev.vk, ActiveRemap::HeldKey(VK_BACK_U16));
              effects.push(EngineEffect::SendKey {
                vk: VK_BACK_U16,
                up: false,
              });
            }
            other => {
              // On autorepeat, don't re-fire one-shot commands/combos repeatedly unless held nav.
              let already_active = s.active_remapped_keys.contains_key(&ev.vk);
              s.active_remapped_keys
                .insert(ev.vk, ActiveRemap::SwallowUpOnly);
              if !already_active {
                effects.push(EngineEffect::ExecuteAction {
                  action: other,
                  release_shift: shift_down,
                });
              }
            }
          }
          return (HookDecision::Swallow, effects);
        }
      }

      return (HookDecision::Pass, effects);
    }

    // Outside Caps-held mode, track DoubleTapModifier.
    let mappings_snapshot = s.mappings.clone();
    if let Some(action) =
      s.modifier_tracker
        .on_key_event(ev.vk, ev.is_down, ev.is_up, ev.timestamp_ms, &mappings_snapshot)
    {
      effects.push(EngineEffect::ExecuteAction {
        action,
        release_shift: false,
      });
    }

    (HookDecision::Pass, effects)
  }

  fn directional_to_vk(dir: DirectionalActionKind) -> u16 {
    match dir {
      DirectionalActionKind::Left | DirectionalActionKind::WordBack => VK_LEFT_U16,
      DirectionalActionKind::Right | DirectionalActionKind::WordForward => VK_RIGHT_U16,
      DirectionalActionKind::Up => VK_UP_U16,
      DirectionalActionKind::Down => VK_DOWN_U16,
      DirectionalActionKind::Home => VK_HOME_U16,
      DirectionalActionKind::End => VK_END_U16,
    }
  }

  pub fn handle_event(self: &Arc<Self>, ev: &LowLevelKeyEvent) -> HookDecision {
    let shift_down = KeySynthesizer::is_key_down(VK_SHIFT_U16);
    let (decision, effects) = self.process_key_event(ev, shift_down);
    for effect in effects {
      self.run_effect(effect);
    }
    decision
  }

  fn run_effect(self: &Arc<Self>, effect: EngineEffect) {
    match effect {
      EngineEffect::SendKey { vk, up } => {
        KeySynthesizer::send_key(vk, up);
      }
      EngineEffect::SendCtrlArrow { arrow_vk, up } => {
        KeySynthesizer::send_ctrl_arrow(arrow_vk, up);
      }
      EngineEffect::ToggleNativeCapsLock => {
        KeySynthesizer::send_toggle_caps_lock();
      }
      EngineEffect::ExecuteAction {
        action,
        release_shift,
      } => {
        Self::execute_action(action, release_shift);
      }
      EngineEffect::ScheduleSingleTapFallback {
        gen,
        delay_ms,
        action,
      } => {
        let engine = Arc::clone(self);
        thread::spawn(move || {
          thread::sleep(Duration::from_millis(delay_ms));
          let still_pending = {
            let mut s = engine.state.lock().unwrap();
            if !s.paused && s.pending_single_tap_gen == gen {
              s.last_caps_tap_ms = 0;
              s.pending_single_tap_gen = s.pending_single_tap_gen.wrapping_add(1);
              true
            } else {
              false
            }
          };
          if still_pending {
            if let Some(act) = action {
              Self::execute_action(act, false);
            } else {
              KeySynthesizer::send_toggle_caps_lock();
            }
          }
        });
      }
    }
  }

  pub fn execute_action(action: ActionConfig, release_shift: bool) {
    match action {
      ActionConfig::Directional {
        action: dir @ (DirectionalActionKind::Left
        | DirectionalActionKind::Right
        | DirectionalActionKind::Up
        | DirectionalActionKind::Down
        | DirectionalActionKind::Home
        | DirectionalActionKind::End),
      } => {
        KeySynthesizer::send_tap(Self::directional_to_vk(dir));
      }
      ActionConfig::Directional {
        action: dir @ (DirectionalActionKind::WordForward | DirectionalActionKind::WordBack),
      } => {
        KeySynthesizer::send_shortcut(&KeyShortcut {
          vk: Self::directional_to_vk(dir),
          ctrl: true,
          alt: false,
          shift: false,
          win: false,
        });
      }
      ActionConfig::Jump { direction, count } => {
        let vk = match direction {
          JumpDirection::Up => VK_UP_U16,
          JumpDirection::Down => VK_DOWN_U16,
        };
        KeySynthesizer::send_repeated_taps(vk, count.max(1));
      }
      ActionConfig::Independent { action } => match action {
        IndependentActionKind::Backspace => KeySynthesizer::send_tap(VK_BACK_U16),
        IndependentActionKind::NextLine => KeySynthesizer::send_next_line(),
        IndependentActionKind::InsertQuotes => KeySynthesizer::send_insert_triple_quotes(),
        IndependentActionKind::ToggleCapsLock => KeySynthesizer::send_toggle_caps_lock(),
        IndependentActionKind::SwitchInputSource => KeySynthesizer::send_switch_input_source(),
      },
      ActionConfig::Command { command } => {
        thread::spawn(move || {
          #[cfg(target_os = "windows")]
          let _ = std::process::Command::new("cmd")
            .arg("/C")
            .arg(&command)
            .creation_flags(0x0800_0000)
            .spawn();
          #[cfg(not(target_os = "windows"))]
          let _ = command;
        });
      }
      ActionConfig::KeyCombo {
        target_key,
        with_ctrl,
        with_alt,
        with_win,
        with_shift,
      } => {
        KeySynthesizer::send_shortcut_releasing_unwanted_shift(
          &KeyShortcut {
            vk: target_key,
            ctrl: with_ctrl,
            alt: with_alt,
            shift: with_shift,
            win: with_win,
          },
          release_shift,
        );
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tools::hyper_capslock::config::{
    default_action_mappings, VK_KEY_H, VK_KEY_K,
  };

  fn ev(vk: u16, is_down: bool, ts: u64) -> LowLevelKeyEvent {
    LowLevelKeyEvent {
      vk,
      scan_code: 0,
      is_down,
      is_up: !is_down,
      is_extended: VkHelper::is_extended_key(vk),
      timestamp_ms: ts,
    }
  }

  #[test]
  fn short_caps_tap_toggles_native_capslock_and_long_hold_does_not() {
    let engine = HyperCapslockEngine::new(false, default_action_mappings());

    let (d1, e1) = engine.process_key_event(&ev(VK_CAPITAL_U16, true, 1000), false);
    assert_eq!(d1, HookDecision::Swallow);
    assert!(e1.is_empty());

    let (d2, e2) = engine.process_key_event(&ev(VK_CAPITAL_U16, false, 1120), false);
    assert_eq!(d2, HookDecision::Swallow);
    assert_eq!(e2, vec![EngineEffect::ToggleNativeCapsLock]);

    // Long hold (300ms > 200ms) does not toggle
    engine.process_key_event(&ev(VK_CAPITAL_U16, true, 2000), false);
    let (_, e3) = engine.process_key_event(&ev(VK_CAPITAL_U16, false, 2300), false);
    assert!(e3.is_empty());
  }

  #[test]
  fn caps_h_remaps_and_releases_cleanly_even_if_caps_released_first() {
    let engine = HyperCapslockEngine::new(false, default_action_mappings());

    engine.process_key_event(&ev(VK_CAPITAL_U16, true, 100), false);
    let (d_down, e_down) = engine.process_key_event(&ev(VK_KEY_H, true, 130), false);
    assert_eq!(d_down, HookDecision::Swallow);
    assert_eq!(
      e_down,
      vec![EngineEffect::SendKey {
        vk: VK_LEFT_U16,
        up: false
      }]
    );

    // Release CapsLock BEFORE releasing H
    let (_, e_caps_up) = engine.process_key_event(&ev(VK_CAPITAL_U16, false, 150), false);
    assert!(e_caps_up.is_empty());

    // Releasing H now must still release VK_LEFT and swallow H key-up
    let (d_h_up, e_h_up) = engine.process_key_event(&ev(VK_KEY_H, false, 180), false);
    assert_eq!(d_h_up, HookDecision::Swallow);
    assert_eq!(
      e_h_up,
      vec![EngineEffect::SendKey {
        vk: VK_LEFT_U16,
        up: true
      }]
    );
  }

  #[test]
  fn caps_shift_falls_back_to_plain_binding_or_prefers_explicit_shift_binding() {
    let mut mappings = default_action_mappings();
    mappings.push(ActionMappingEntry {
      trigger: Trigger::HyperPlusKey {
        key: VK_KEY_K,
        with_shift: true,
      },
      action: ActionConfig::Command {
        command: "calc.exe".to_string(),
      },
    });
    let engine = HyperCapslockEngine::new(false, mappings);

    engine.process_key_event(&ev(VK_CAPITAL_U16, true, 100), true);

    // Caps + Shift + H should fall back to Caps + H (Left) for Shift+Left selection
    let (d_h, e_h) = engine.process_key_event(&ev(VK_KEY_H, true, 110), true);
    assert_eq!(d_h, HookDecision::Swallow);
    assert_eq!(
      e_h,
      vec![EngineEffect::SendKey {
        vk: VK_LEFT_U16,
        up: false
      }]
    );

    // Caps + Shift + K should match the explicit with_shift: true command binding
    let (d_k, e_k) = engine.process_key_event(&ev(VK_KEY_K, true, 140), true);
    assert_eq!(d_k, HookDecision::Swallow);
    assert_eq!(
      e_k,
      vec![EngineEffect::ExecuteAction {
        action: ActionConfig::Command {
          command: "calc.exe".to_string()
        },
        release_shift: true,
      }]
    );
  }

  #[test]
  fn single_tap_and_double_tap_hyper_disambiguate_correctly() {
    let mut mappings = default_action_mappings();
    mappings.push(ActionMappingEntry {
      trigger: Trigger::SingleTapHyper,
      action: ActionConfig::Independent {
        action: IndependentActionKind::SwitchInputSource,
      },
    });
    mappings.push(ActionMappingEntry {
      trigger: Trigger::DoubleTapHyper,
      action: ActionConfig::Independent {
        action: IndependentActionKind::ToggleCapsLock,
      },
    });
    let engine = HyperCapslockEngine::new(false, mappings);

    // First tap schedules fallback for SingleTapHyper
    engine.process_key_event(&ev(VK_CAPITAL_U16, true, 1000), false);
    let (_, e1) = engine.process_key_event(&ev(VK_CAPITAL_U16, false, 1050), false);
    assert!(matches!(
      e1.as_slice(),
      [EngineEffect::ScheduleSingleTapFallback { .. }]
    ));

    // Second tap within 280ms executes DoubleTapHyper immediately
    engine.process_key_event(&ev(VK_CAPITAL_U16, true, 1180), false);
    let (_, e2) = engine.process_key_event(&ev(VK_CAPITAL_U16, false, 1210), false);
    assert_eq!(
      e2,
      vec![EngineEffect::ExecuteAction {
        action: ActionConfig::Independent {
          action: IndependentActionKind::ToggleCapsLock,
        },
        release_shift: false,
      }]
    );
  }

  #[test]
  fn double_tap_modifier_triggers_only_on_clean_double_tap() {
    let mut mappings = default_action_mappings();
    mappings.push(ActionMappingEntry {
      trigger: Trigger::DoubleTapModifier {
        modifier: ModifierKey::LeftControl,
      },
      action: ActionConfig::Independent {
        action: IndependentActionKind::SwitchInputSource,
      },
    });
    let engine = HyperCapslockEngine::new(false, mappings);

    // Dirty tap (interleaved with 'A') must not prime double-tap
    engine.process_key_event(&ev(0xA2, true, 100), false);
    engine.process_key_event(&ev(65, true, 120), false);
    engine.process_key_event(&ev(65, false, 140), false);
    let (_, e_dirty) = engine.process_key_event(&ev(0xA2, false, 160), false);
    assert!(e_dirty.is_empty());

    // Two clean taps within window trigger DoubleTapModifier
    engine.process_key_event(&ev(0xA2, true, 500), false);
    let (_, e_first) = engine.process_key_event(&ev(0xA2, false, 560), false);
    assert!(e_first.is_empty());

    engine.process_key_event(&ev(0xA2, true, 660), false);
    let (dec, e_second) = engine.process_key_event(&ev(0xA2, false, 700), false);
    assert_eq!(dec, HookDecision::Pass);
    assert_eq!(
      e_second,
      vec![EngineEffect::ExecuteAction {
        action: ActionConfig::Independent {
          action: IndependentActionKind::SwitchInputSource,
        },
        release_shift: false,
      }]
    );
  }
}
