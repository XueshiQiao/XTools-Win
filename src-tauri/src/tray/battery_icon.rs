use tauri::image::Image;

use crate::tools::rog_keyboard::protocol::{RogBatteryInfo, RogChargingState};

const ICON_SIZE: usize = 32;

/// Pure-Rust 32x32 RGBA badge renderer for the dedicated ROG keyboard battery tray icon.
pub struct BatteryIconRenderer;

impl BatteryIconRenderer {
  pub fn render(info: &RogBatteryInfo) -> Image<'static> {
    let mut rgba = vec![0u8; ICON_SIZE * ICON_SIZE * 4];
    let clamped = info.percent.min(100);

    let is_charging_or_full = matches!(
      info.charging_state,
      RogChargingState::Charging | RogChargingState::Full
    );

    let accent = if is_charging_or_full {
      [0x30u8, 0xD1, 0x58, 0xFF]
    } else if clamped <= 10 {
      [0xFF, 0x45, 0x3A, 0xFF]
    } else if clamped <= 20 {
      [0xFF, 0x9F, 0x0A, 0xFF]
    } else {
      [0x30, 0xD1, 0x58, 0xFF]
    };

    let border_color = if is_charging_or_full || clamped <= 20 {
      accent
    } else {
      [0xFF, 0xFF, 0xFF, 0x58]
    };

    let text_color = if info.charging_state == RogChargingState::Charging {
      [0x4A, 0xDE, 0x80, 0xFF]
    } else if clamped <= 10 {
      [0xFF, 0x69, 0x61, 0xFF]
    } else if clamped <= 20 {
      [0xFF, 0xB3, 0x40, 0xFF]
    } else {
      [0xFF, 0xFF, 0xFF, 0xFF]
    };

    let bg_color = [0x18u8, 0x18, 0x1E, 0xF2];

    // Draw rounded badge x=1..31, y=2..30 (30x28), radius 5
    Self::fill_rounded_badge(&mut rgba, 1, 2, 30, 28, 5, bg_color, border_color);

    // Draw bottom 3px progress bar track & fill (y=25..28, x=4..28 -> width 24)
    let track_x = 4usize;
    let track_y = 25usize;
    let track_w = 24usize;
    let track_h = 3usize;
    Self::fill_rect(&mut rgba, track_x, track_y, track_w, track_h, [0xFF, 0xFF, 0xFF, 0x30]);

    let fill_w = if clamped == 0 {
      0
    } else {
      ((usize::from(clamped) * track_w + 99) / 100).max(2).min(track_w)
    };
    if fill_w > 0 {
      Self::fill_rect(&mut rgba, track_x, track_y, fill_w, track_h, accent);
    }

    // Draw centered battery number inside y=6..20
    let text = clamped.to_string();
    Self::draw_centered_digits(&mut rgba, &text, text_color);

    Image::new_owned(rgba, ICON_SIZE as u32, ICON_SIZE as u32)
  }

  fn set_pixel(rgba: &mut [u8], x: usize, y: usize, color: [u8; 4]) {
    if x >= ICON_SIZE || y >= ICON_SIZE {
      return;
    }
    let idx = (y * ICON_SIZE + x) * 4;
    rgba[idx] = color[0];
    rgba[idx + 1] = color[1];
    rgba[idx + 2] = color[2];
    rgba[idx + 3] = color[3];
  }

  fn fill_rect(rgba: &mut [u8], x0: usize, y0: usize, w: usize, h: usize, color: [u8; 4]) {
    for y in y0..(y0 + h) {
      for x in x0..(x0 + w) {
        Self::set_pixel(rgba, x, y, color);
      }
    }
  }

  fn fill_rounded_badge(
    rgba: &mut [u8],
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    r: usize,
    bg: [u8; 4],
    border: [u8; 4],
  ) {
    let x1 = x0 + w - 1;
    let y1 = y0 + h - 1;
    let rf = r as f32;

    for y in y0..=y1 {
      for x in x0..=x1 {
        let dx = if x < x0 + r {
          (x0 + r - x) as f32
        } else if x > x1 - r {
          (x - (x1 - r)) as f32
        } else {
          0.0
        };
        let dy = if y < y0 + r {
          (y0 + r - y) as f32
        } else if y > y1 - r {
          (y - (y1 - r)) as f32
        } else {
          0.0
        };
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > rf + 0.35 {
          continue;
        }
        let is_border = x == x0
          || x == x1
          || y == y0
          || y == y1
          || (dx > 0.0 && dy > 0.0 && dist >= rf - 0.85);
        Self::set_pixel(rgba, x, y, if is_border { border } else { bg });
      }
    }
  }

  fn draw_centered_digits(rgba: &mut [u8], text: &str, color: [u8; 4]) {
    let digits: Vec<char> = text.chars().collect();
    if digits.len() >= 3 {
      // Compact 2x glyphs for 3 digits ("100"): '1' = 4px wide (2 cols * 2), '0' = 8px wide (4 cols * 2), gap = 2px -> 24px total
      let total_w: usize = digits
        .iter()
        .enumerate()
        .map(|(i, &ch)| {
          let gw = if ch == '1' { 4 } else { 8 };
          if i > 0 { gw + 2 } else { gw }
        })
        .sum();
      let mut cx = (ICON_SIZE.saturating_sub(total_w)) / 2;
      let cy = 7usize; // 7 rows * 2 = 14px -> y=7..21
      for &ch in &digits {
        let rows = Self::compact_glyph(ch);
        let cols = if ch == '1' { 2 } else { 4 };
        for (ry, row_bits) in rows.iter().enumerate() {
          for rx in 0..cols {
            if (row_bits >> (cols - 1 - rx)) & 1 == 1 {
              Self::fill_rect(rgba, cx + rx * 2, cy + ry * 2, 2, 2, color);
            }
          }
        }
        cx += cols * 2 + 2;
      }
    } else {
      // Standard 5x7 glyphs scaled 2x -> 10x14px per digit, 2px gap
      let total_w = digits.len() * 10 + digits.len().saturating_sub(1) * 2;
      let mut cx = (ICON_SIZE.saturating_sub(total_w)) / 2;
      let cy = 7usize;
      for &ch in &digits {
        let rows = Self::std_glyph(ch);
        for (ry, row_bits) in rows.iter().enumerate() {
          for rx in 0..5 {
            if (row_bits >> (4 - rx)) & 1 == 1 {
              Self::fill_rect(rgba, cx + rx * 2, cy + ry * 2, 2, 2, color);
            }
          }
        }
        cx += 12;
      }
    }
  }

  fn std_glyph(ch: char) -> [u8; 7] {
    match ch {
      '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
      '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
      '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
      '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
      '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
      '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
      '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
      '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
      '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
      '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
      _ => [0; 7],
    }
  }

  fn compact_glyph(ch: char) -> [u8; 7] {
    match ch {
      '1' => [0b01, 0b11, 0b01, 0b01, 0b01, 0b01, 0b01],
      '0' => [0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110],
      _ => [0; 7],
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renders_32x32_rgba_for_various_battery_levels() {
    for (pct, state) in [
      (100, RogChargingState::Full),
      (85, RogChargingState::Discharging),
      (15, RogChargingState::Charging),
      (5, RogChargingState::Discharging),
    ] {
      let info = RogBatteryInfo {
        percent: pct,
        charging_state: state,
        left_percent: Some(pct),
        left_mv: Some(4000),
        right_percent: Some(pct),
        right_mv: Some(4000),
      };
      let img = BatteryIconRenderer::render(&info);
      assert_eq!(img.width(), 32);
      assert_eq!(img.height(), 32);
      assert_eq!(img.rgba().len(), 32 * 32 * 4);
      assert!(img.rgba().chunks_exact(4).any(|px| px[3] > 0));
    }
  }
}
