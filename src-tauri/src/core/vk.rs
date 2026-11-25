use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
  GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
  KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CAPITAL, VK_END,
  VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RETURN, VK_RSHIFT, VK_SPACE,
};

#[cfg(target_pointer_width = "64")]
pub const SYNTHETIC_EXTRA_INFO: usize = 0x5854_4F4F_4C53;
#[cfg(not(target_pointer_width = "64"))]
pub const SYNTHETIC_EXTRA_INFO: usize = 0x5854_4F4F;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyShortcut {
  pub vk: u16,
  #[serde(default)]
  pub ctrl: bool,
  #[serde(default)]
  pub alt: bool,
  #[serde(default)]
  pub shift: bool,
  #[serde(default)]
  pub win: bool,
}

impl KeyShortcut {
  pub fn display(&self) -> String {
    let mut parts: Vec<String> = Vec::new();
    if self.ctrl {
      parts.push("Ctrl".to_string());
    }
    if self.alt {
      parts.push("Alt".to_string());
    }
    if self.shift {
      parts.push("Shift".to_string());
    }
    if self.win {
      parts.push("Win".to_string());
    }
    parts.push(VkHelper::vk_name(self.vk));
    parts.join("+")
  }
}

pub struct VkHelper;

impl VkHelper {
  pub fn is_extended_key(vk: u16) -> bool {
    matches!(
      vk,
      0x21..=0x28 | 0x2D | 0x2E | 0x5B | 0x5C | 0x5D | 0x6F | 0xA3 | 0xA5
    )
  }

  pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
  }

  pub fn is_sentinel_fn_vk(vk: u16) -> bool {
    matches!(vk, 0x7C..=0x87)
  }

  pub fn vk_name(vk: u16) -> String {
    match vk {
      48..=57 => ((b'0' + (vk as u8 - 48)) as char).to_string(),
      65..=90 => ((b'A' + (vk as u8 - 65)) as char).to_string(),
      0x60..=0x69 => format!("Num {}", vk - 0x60),
      0x70..=0x87 => format!("F{}", vk - 0x6F),
      8 => "Backspace".to_string(),
      9 => "Tab".to_string(),
      13 => "Enter".to_string(),
      27 => "Esc".to_string(),
      32 => "Space".to_string(),
      33 => "PgUp".to_string(),
      34 => "PgDn".to_string(),
      35 => "End".to_string(),
      36 => "Home".to_string(),
      37 => "Left".to_string(),
      38 => "Up".to_string(),
      39 => "Right".to_string(),
      40 => "Down".to_string(),
      45 => "Ins".to_string(),
      46 => "Del".to_string(),
      186 => ";".to_string(),
      187 => "=".to_string(),
      188 => ",".to_string(),
      189 => "-".to_string(),
      190 => ".".to_string(),
      191 => "/".to_string(),
      192 => "`".to_string(),
      219 => "[".to_string(),
      220 => "\\".to_string(),
      221 => "]".to_string(),
      222 => "'".to_string(),
      _ => format!("VK 0x{:02X}", vk),
    }
  }
}

pub struct KeySynthesizer;

impl KeySynthesizer {
  #[cfg(target_os = "windows")]
  pub fn make_vk_input(vk: u16, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
      flags |= KEYEVENTF_KEYUP;
    }
    if VkHelper::is_extended_key(vk) {
      flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
      r#type: INPUT_KEYBOARD,
      Anonymous: INPUT_0 {
        ki: KEYBDINPUT {
          wVk: VIRTUAL_KEY(vk),
          wScan: 0,
          dwFlags: flags,
          time: 0,
          dwExtraInfo: SYNTHETIC_EXTRA_INFO,
        },
      },
    }
  }

  #[cfg(target_os = "windows")]
  pub fn make_unicode_inputs(ch: u16) -> [INPUT; 2] {
    let down = INPUT {
      r#type: INPUT_KEYBOARD,
      Anonymous: INPUT_0 {
        ki: KEYBDINPUT {
          wVk: VIRTUAL_KEY(0),
          wScan: ch,
          dwFlags: KEYEVENTF_UNICODE,
          time: 0,
          dwExtraInfo: SYNTHETIC_EXTRA_INFO,
        },
      },
    };
    let up = INPUT {
      r#type: INPUT_KEYBOARD,
      Anonymous: INPUT_0 {
        ki: KEYBDINPUT {
          wVk: VIRTUAL_KEY(0),
          wScan: ch,
          dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
          time: 0,
          dwExtraInfo: SYNTHETIC_EXTRA_INFO,
        },
      },
    };
    [down, up]
  }

  #[cfg(target_os = "windows")]
  pub fn send_inputs(inputs: &[INPUT]) {
    if inputs.is_empty() {
      return;
    }
    unsafe {
      let _ = SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
  }

  pub fn send_key(vk: u16, up: bool) {
    #[cfg(target_os = "windows")]
    Self::send_inputs(&[Self::make_vk_input(vk, up)]);
    #[cfg(not(target_os = "windows"))]
    let _ = (vk, up);
  }

  pub fn send_tap(vk: u16) {
    #[cfg(target_os = "windows")]
    Self::send_inputs(&[Self::make_vk_input(vk, false), Self::make_vk_input(vk, true)]);
    #[cfg(not(target_os = "windows"))]
    let _ = vk;
  }

  pub fn send_repeated_taps(vk: u16, count: u8) {
    #[cfg(target_os = "windows")]
    {
      let total = usize::from(count);
      let mut inputs = Vec::with_capacity(total * 2);
      for _ in 0..total {
        inputs.push(Self::make_vk_input(vk, false));
        inputs.push(Self::make_vk_input(vk, true));
      }
      Self::send_inputs(&inputs);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (vk, count);
  }

  pub fn send_shortcut(shortcut: &KeyShortcut) {
    Self::send_shortcut_releasing_unwanted_shift(shortcut, false);
  }

  pub fn send_shortcut_releasing_unwanted_shift(shortcut: &KeyShortcut, release_shift: bool) {
    #[cfg(target_os = "windows")]
    {
      let mut inputs: Vec<INPUT> = Vec::with_capacity(14);
      let lshift_down = release_shift && !shortcut.shift && Self::is_key_down(VK_LSHIFT.0);
      let rshift_down = release_shift && !shortcut.shift && Self::is_key_down(VK_RSHIFT.0);

      if lshift_down {
        inputs.push(Self::make_vk_input(VK_LSHIFT.0, true));
      }
      if rshift_down {
        inputs.push(Self::make_vk_input(VK_RSHIFT.0, true));
      }

      if shortcut.ctrl {
        inputs.push(Self::make_vk_input(VK_LCONTROL.0, false));
      }
      if shortcut.alt {
        inputs.push(Self::make_vk_input(VK_LMENU.0, false));
      }
      if shortcut.shift {
        inputs.push(Self::make_vk_input(VK_LSHIFT.0, false));
      }
      if shortcut.win {
        inputs.push(Self::make_vk_input(VK_LWIN.0, false));
      }

      inputs.push(Self::make_vk_input(shortcut.vk, false));
      inputs.push(Self::make_vk_input(shortcut.vk, true));

      if shortcut.win {
        inputs.push(Self::make_vk_input(VK_LWIN.0, true));
      }
      if shortcut.shift {
        inputs.push(Self::make_vk_input(VK_LSHIFT.0, true));
      }
      if shortcut.alt {
        inputs.push(Self::make_vk_input(VK_LMENU.0, true));
      }
      if shortcut.ctrl {
        inputs.push(Self::make_vk_input(VK_LCONTROL.0, true));
      }

      if rshift_down {
        inputs.push(Self::make_vk_input(VK_RSHIFT.0, false));
      }
      if lshift_down {
        inputs.push(Self::make_vk_input(VK_LSHIFT.0, false));
      }

      Self::send_inputs(&inputs);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (shortcut, release_shift);
  }

  pub fn send_ctrl_arrow(arrow_vk: u16, up: bool) {
    #[cfg(target_os = "windows")]
    {
      if up {
        Self::send_inputs(&[
          Self::make_vk_input(arrow_vk, true),
          Self::make_vk_input(VK_LCONTROL.0, true),
        ]);
      } else {
        Self::send_inputs(&[
          Self::make_vk_input(VK_LCONTROL.0, false),
          Self::make_vk_input(arrow_vk, false),
        ]);
      }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (arrow_vk, up);
  }

  pub fn send_unicode_text(text: &str) {
    #[cfg(target_os = "windows")]
    {
      let mut inputs: Vec<INPUT> = Vec::new();
      for unit in text.encode_utf16() {
        let pair = Self::make_unicode_inputs(unit);
        inputs.push(pair[0]);
        inputs.push(pair[1]);
      }
      Self::send_inputs(&inputs);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = text;
  }

  pub fn send_insert_triple_quotes() {
    #[cfg(target_os = "windows")]
    {
      Self::send_unicode_text("\"\"\"\"\"\"");
      Self::send_repeated_taps(VK_LEFT.0, 3);
    }
  }

  pub fn send_next_line() {
    #[cfg(target_os = "windows")]
    {
      Self::send_inputs(&[
        Self::make_vk_input(VK_END.0, false),
        Self::make_vk_input(VK_END.0, true),
        Self::make_vk_input(VK_RETURN.0, false),
        Self::make_vk_input(VK_RETURN.0, true),
      ]);
    }
  }

  pub fn send_toggle_caps_lock() {
    #[cfg(target_os = "windows")]
    Self::send_tap(VK_CAPITAL.0);
  }

  pub fn send_switch_input_source() {
    #[cfg(target_os = "windows")]
    {
      Self::send_inputs(&[
        Self::make_vk_input(VK_LWIN.0, false),
        Self::make_vk_input(VK_SPACE.0, false),
        Self::make_vk_input(VK_SPACE.0, true),
        Self::make_vk_input(VK_LWIN.0, true),
      ]);
    }
  }

  pub fn is_key_down(vk: u16) -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
      (GetAsyncKeyState(i32::from(vk)) as u16 & 0x8000) != 0
    }
    #[cfg(not(target_os = "windows"))]
    {
      let _ = vk;
      false
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn vk_names_cover_common_and_sentinel_keys() {
    assert_eq!(VkHelper::vk_name(65), "A");
    assert_eq!(VkHelper::vk_name(49), "1");
    assert_eq!(VkHelper::vk_name(0x70), "F1");
    assert_eq!(VkHelper::vk_name(0x7C), "F13");
    assert_eq!(VkHelper::vk_name(0x87), "F24");
    assert_eq!(VkHelper::vk_name(37), "Left");
    assert_eq!(VkHelper::vk_name(32), "Space");
  }

  #[test]
  fn extended_and_sentinel_flags_match_windows_specs() {
    assert!(VkHelper::is_extended_key(0x25));
    assert!(VkHelper::is_extended_key(0x2E));
    assert!(VkHelper::is_extended_key(0x5B));
    assert!(!VkHelper::is_extended_key(b'A' as u16));

    assert!(VkHelper::is_sentinel_fn_vk(0x7C));
    assert!(VkHelper::is_sentinel_fn_vk(0x87));
    assert!(!VkHelper::is_sentinel_fn_vk(0x7B));

    assert!(VkHelper::is_modifier_vk(0xA0));
    assert!(VkHelper::is_modifier_vk(0x5B));
    assert!(!VkHelper::is_modifier_vk(0x14)); // CapsLock
  }

  #[test]
  fn shortcut_display_formats_modifiers_consistently() {
    let sc = KeyShortcut {
      vk: 87, // W
      ctrl: true,
      alt: true,
      shift: false,
      win: false,
    };
    assert_eq!(sc.display(), "Ctrl+Alt+W");

    let plain = KeyShortcut {
      vk: 0x74, // F5
      ctrl: false,
      alt: false,
      shift: false,
      win: false,
    };
    assert_eq!(plain.display(), "F5");
  }
}
