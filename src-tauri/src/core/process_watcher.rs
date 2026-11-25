use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use windows::{
  core::{BOOL, PCWSTR, PWSTR},
  Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM},
  Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
  },
  Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
  Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
  Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
  },
  Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
  },
  Win32::UI::Shell::{ExtractIconExW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
  Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumWindows, GetIconInfo, GetWindowThreadProcessId, IsWindowVisible, HICON,
    ICONINFO,
  },
};

/// A running process offered in the auto-pause picker.
#[derive(Debug, Clone, Serialize)]
pub struct RunningProcessInfo {
  pub name: String,
  pub icon_data_url: Option<String>,
  pub has_window: bool,
}

/// Minimal dependency-free PNG / base64 encoder used to ship executable icons to the UI.
pub struct IconEncoder;

impl IconEncoder {
  fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
      let mut c = i as u32;
      for _ in 0..8 {
        c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
      }
      *slot = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
      crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
  }

  fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
      a = (a + byte as u32) % 65521;
      b = (b + a) % 65521;
    }
    (b << 16) | a
  }

  fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = Vec::with_capacity(4 + data.len());
    body.extend_from_slice(kind);
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&Self::crc32(&body).to_be_bytes());
  }

  /// Encodes 8-bit RGBA pixels as an (uncompressed-deflate) PNG.
  pub fn rgba_to_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row_len = width as usize * 4;
    let mut raw = Vec::with_capacity((row_len + 1) * height as usize);
    for row in rgba.chunks(row_len).take(height as usize) {
      raw.push(0);
      raw.extend_from_slice(row);
    }

    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
      zlib.push(if i + 1 == blocks.len() { 1 } else { 0 });
      let len = block.len() as u16;
      zlib.extend_from_slice(&len.to_le_bytes());
      zlib.extend_from_slice(&(!len).to_le_bytes());
      zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&Self::adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    Self::push_chunk(&mut png, b"IHDR", &ihdr);
    Self::push_chunk(&mut png, b"IDAT", &zlib);
    Self::push_chunk(&mut png, b"IEND", &[]);
    png
  }

  pub fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
      b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
      let b0 = chunk[0] as u32;
      let b1 = *chunk.get(1).unwrap_or(&0) as u32;
      let b2 = *chunk.get(2).unwrap_or(&0) as u32;
      let n = (b0 << 16) | (b1 << 8) | b2;
      out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
      out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
      out.push(if chunk.len() > 1 {
        ALPHABET[((n >> 6) & 63) as usize] as char
      } else {
        '='
      });
      out.push(if chunk.len() > 2 {
        ALPHABET[(n & 63) as usize] as char
      } else {
        '='
      });
    }
    out
  }

  pub fn rgba_to_png_data_url(width: u32, height: u32, rgba: &[u8]) -> String {
    format!(
      "data:image/png;base64,{}",
      Self::base64_encode(&Self::rgba_to_png(width, height, rgba))
    )
  }
}

#[cfg(target_os = "windows")]
struct OwnedHandle(HANDLE);

#[cfg(target_os = "windows")]
impl Drop for OwnedHandle {
  fn drop(&mut self) {
    unsafe {
      let _ = CloseHandle(self.0);
    }
  }
}

#[cfg(target_os = "windows")]
struct IconGuard(HICON);

#[cfg(target_os = "windows")]
impl Drop for IconGuard {
  fn drop(&mut self) {
    unsafe {
      let _ = DestroyIcon(self.0);
    }
  }
}

#[cfg(target_os = "windows")]
struct BitmapGuard(HBITMAP);

#[cfg(target_os = "windows")]
impl Drop for BitmapGuard {
  fn drop(&mut self) {
    if !self.0.is_invalid() {
      unsafe {
        let _ = DeleteObject(HGDIOBJ(self.0 .0));
      }
    }
  }
}

#[cfg(target_os = "windows")]
struct DcGuard(HDC);

#[cfg(target_os = "windows")]
impl Drop for DcGuard {
  fn drop(&mut self) {
    unsafe {
      let _ = DeleteDC(self.0);
    }
  }
}

#[cfg(target_os = "windows")]
static ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

/// Enumerates running Windows processes through a ToolHelp snapshot.
///
/// `CreateToolhelp32Snapshot` reads executable names without `OpenProcess`, so it also
/// works for elevated or anti-cheat-protected processes (e.g. `cs2.exe`).
pub struct ProcessEnumerator;

impl ProcessEnumerator {
  /// Normalizes user input into a lowercase executable name:
  /// strips quotes / directories, lowercases, and appends `.exe` when no extension is given.
  pub fn normalize_process_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    let base = trimmed.rsplit(|c| c == '\\' || c == '/').next().unwrap_or("");
    let base = base.trim();
    if base.is_empty() || base.starts_with('.') {
      return None;
    }
    if base
      .chars()
      .any(|c| matches!(c, ':' | '*' | '?' | '<' | '>' | '|' | '"'))
    {
      return None;
    }
    let mut name = base.to_ascii_lowercase();
    if !name.contains('.') {
      name.push_str(".exe");
    }
    Some(name)
  }

  #[cfg(target_os = "windows")]
  fn collect_process_entries() -> Vec<(u32, String)> {
    let mut entries = Vec::new();
    unsafe {
      let Ok(handle) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
        return entries;
      };
      let guard = OwnedHandle(handle);
      let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
      };
      if Process32FirstW(guard.0, &mut entry).is_err() {
        return entries;
      }
      loop {
        let len = entry
          .szExeFile
          .iter()
          .position(|&c| c == 0)
          .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
        let name = name.trim();
        if !name.is_empty() {
          entries.push((entry.th32ProcessID, name.to_string()));
        }
        if Process32NextW(guard.0, &mut entry).is_err() {
          break;
        }
      }
    }
    entries
  }

  #[cfg(not(target_os = "windows"))]
  fn collect_process_entries() -> Vec<(u32, String)> {
    Vec::new()
  }

  fn collect_process_names() -> Vec<String> {
    Self::collect_process_entries()
      .into_iter()
      .map(|(_, name)| name)
      .collect()
  }

  /// Fast name-only snapshot used by the background watcher (never extracts icons).
  pub fn running_process_names_lowercase() -> HashSet<String> {
    Self::collect_process_names()
      .into_iter()
      .map(|n| n.to_ascii_lowercase())
      .collect()
  }

  #[cfg(target_os = "windows")]
  fn visible_window_pids() -> HashSet<u32> {
    unsafe extern "system" fn on_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
      if IsWindowVisible(hwnd).as_bool() {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 {
          let set = &mut *(lparam.0 as *mut HashSet<u32>);
          set.insert(pid);
        }
      }
      BOOL(1)
    }

    let mut pids: HashSet<u32> = HashSet::new();
    unsafe {
      let _ = EnumWindows(Some(on_window), LPARAM(&mut pids as *mut HashSet<u32> as isize));
    }
    pids
  }

  #[cfg(target_os = "windows")]
  fn query_process_image_path(pid: u32) -> Option<String> {
    unsafe {
      let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
      let guard = OwnedHandle(handle);
      let mut buf = vec![0u16; 1024];
      let mut size = buf.len() as u32;
      QueryFullProcessImageNameW(guard.0, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut size)
        .ok()?;
      Some(String::from_utf16_lossy(&buf[..size as usize]))
    }
  }

  #[cfg(target_os = "windows")]
  fn extract_exe_icon_data_url(exe_path: &str) -> Option<String> {
    let cache = ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = exe_path.to_ascii_lowercase();
    if let Some(hit) = cache.lock().unwrap().get(&key) {
      return hit.clone();
    }
    let result = Self::load_icon_data_url(exe_path);
    cache.lock().unwrap().insert(key, result.clone());
    result
  }

  #[cfg(target_os = "windows")]
  fn load_icon_data_url(exe_path: &str) -> Option<String> {
    let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
      let mut hicon = HICON::default();
      let extracted = ExtractIconExW(PCWSTR(wide.as_ptr()), 0, Some(&mut hicon), None, 1);
      if extracted == 0 || hicon.is_invalid() {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let mut info = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
          PCWSTR(wide.as_ptr()),
          FILE_ATTRIBUTE_NORMAL,
          Some(&mut info),
          std::mem::size_of::<SHFILEINFOW>() as u32,
          SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ok == 0 || info.hIcon.is_invalid() {
          return None;
        }
        hicon = info.hIcon;
      }
      let icon = IconGuard(hicon);
      Self::icon_to_data_url(icon.0)
    }
  }

  #[cfg(target_os = "windows")]
  unsafe fn icon_to_data_url(hicon: HICON) -> Option<String> {
    let mut info = ICONINFO::default();
    GetIconInfo(hicon, &mut info).ok()?;
    let color = BitmapGuard(info.hbmColor);
    let mask = BitmapGuard(info.hbmMask);
    if color.0.is_invalid() {
      return None;
    }

    let mut bmp = BITMAP::default();
    let got = GetObjectW(
      HGDIOBJ(color.0 .0),
      std::mem::size_of::<BITMAP>() as i32,
      Some(&mut bmp as *mut BITMAP as *mut std::ffi::c_void),
    );
    if got == 0 {
      return None;
    }
    let w = bmp.bmWidth as usize;
    let h = bmp.bmHeight.unsigned_abs() as usize;
    if !(1..=256).contains(&w) || !(1..=256).contains(&h) {
      return None;
    }

    let hdc = CreateCompatibleDC(None);
    if hdc.is_invalid() {
      return None;
    }
    let dc = DcGuard(hdc);

    let mut bmi = BITMAPINFO {
      bmiHeader: BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w as i32,
        biHeight: -(h as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0,
        ..Default::default()
      },
      ..Default::default()
    };

    let mut pixels = vec![0u8; w * h * 4];
    let lines = GetDIBits(
      dc.0,
      color.0,
      0,
      h as u32,
      Some(pixels.as_mut_ptr() as *mut std::ffi::c_void),
      &mut bmi,
      DIB_RGB_COLORS,
    );
    if lines == 0 {
      return None;
    }

    if pixels.chunks_exact(4).all(|px| px[3] == 0) && !mask.0.is_invalid() {
      let mut mask_px = vec![0u8; w * h * 4];
      let mask_lines = GetDIBits(
        dc.0,
        mask.0,
        0,
        h as u32,
        Some(mask_px.as_mut_ptr() as *mut std::ffi::c_void),
        &mut bmi,
        DIB_RGB_COLORS,
      );
      if mask_lines != 0 {
        for (px, m) in pixels.chunks_exact_mut(4).zip(mask_px.chunks_exact(4)) {
          px[3] = if m[0] == 0 { 255 } else { 0 };
        }
      }
    }

    for px in pixels.chunks_exact_mut(4) {
      px.swap(0, 2);
    }
    Some(IconEncoder::rgba_to_png_data_url(w as u32, h as u32, &pixels))
  }

  /// Distinct `.exe` processes with icons; windowed apps first, then the rest alphabetically.
  #[cfg(target_os = "windows")]
  pub fn list_selectable_processes() -> Vec<RunningProcessInfo> {
    struct Group {
      name: String,
      has_window: bool,
      pids: Vec<u32>,
    }

    let visible = Self::visible_window_pids();
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for (pid, name) in Self::collect_process_entries() {
      let lower = name.to_ascii_lowercase();
      if !lower.ends_with(".exe") || lower == "xtools.exe" {
        continue;
      }
      let group = groups.entry(lower).or_insert_with(|| Group {
        name: name.clone(),
        has_window: false,
        pids: Vec::new(),
      });
      if visible.contains(&pid) {
        group.has_window = true;
      }
      if group.pids.len() < 4 {
        group.pids.push(pid);
      }
    }

    let mut list: Vec<RunningProcessInfo> = groups
      .into_values()
      .map(|group| {
        let icon_data_url = group
          .pids
          .iter()
          .find_map(|&pid| Self::query_process_image_path(pid))
          .and_then(|path| Self::extract_exe_icon_data_url(&path));
        RunningProcessInfo {
          name: group.name,
          icon_data_url,
          has_window: group.has_window,
        }
      })
      .collect();

    list.sort_by_key(|p| {
      (
        !p.has_window,
        p.icon_data_url.is_none(),
        p.name.to_ascii_lowercase(),
      )
    });
    list
  }

  #[cfg(not(target_os = "windows"))]
  pub fn list_selectable_processes() -> Vec<RunningProcessInfo> {
    let _: BTreeMap<String, String> = BTreeMap::new();
    Vec::new()
  }

  /// Returns the watched names that are currently running (order preserved, deduped).
  pub fn find_matched_processes(
    watched: &[String],
    running_lower: &HashSet<String>,
  ) -> Vec<String> {
    let mut matched: Vec<String> = Vec::new();
    for name in watched {
      if running_lower.contains(&name.to_ascii_lowercase()) && !matched.contains(name) {
        matched.push(name.clone());
      }
    }
    matched
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalize_handles_bare_mixed_case_and_paths() {
    assert_eq!(
      ProcessEnumerator::normalize_process_name("cs2"),
      Some("cs2.exe".to_string())
    );
    assert_eq!(
      ProcessEnumerator::normalize_process_name("Steam.EXE"),
      Some("steam.exe".to_string())
    );
    assert_eq!(
      ProcessEnumerator::normalize_process_name("\"C:\\Games\\cs2.exe\""),
      Some("cs2.exe".to_string())
    );
    assert_eq!(
      ProcessEnumerator::normalize_process_name("  C:/Steam/steam.exe  "),
      Some("steam.exe".to_string())
    );
  }

  #[test]
  fn normalize_rejects_invalid_input() {
    assert_eq!(ProcessEnumerator::normalize_process_name(""), None);
    assert_eq!(ProcessEnumerator::normalize_process_name("   "), None);
    assert_eq!(ProcessEnumerator::normalize_process_name(".exe"), None);
    assert_eq!(ProcessEnumerator::normalize_process_name("a*b"), None);
  }

  #[test]
  fn find_matched_is_case_insensitive_and_ordered() {
    let watched = vec![
      "steam.exe".to_string(),
      "cs2.exe".to_string(),
      "other.exe".to_string(),
    ];
    let running: HashSet<String> = ["cs2.exe", "steam.exe", "explorer.exe"]
      .iter()
      .map(|s| s.to_string())
      .collect();
    assert_eq!(
      ProcessEnumerator::find_matched_processes(&watched, &running),
      vec!["steam.exe".to_string(), "cs2.exe".to_string()]
    );
  }

  #[test]
  fn png_encoder_produces_valid_decodable_rgba_png() {
    let rgba: Vec<u8> = vec![
      255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 255,
    ];
    let png = IconEncoder::rgba_to_png(2, 2, &rgba);
    let img = tauri::image::Image::from_bytes(&png).expect("png should decode");
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
    assert_eq!(img.rgba(), rgba.as_slice());
  }

  #[test]
  fn base64_encodes_standard_vectors() {
    assert_eq!(IconEncoder::base64_encode(b""), "");
    assert_eq!(IconEncoder::base64_encode(b"f"), "Zg==");
    assert_eq!(IconEncoder::base64_encode(b"fo"), "Zm8=");
    assert_eq!(IconEncoder::base64_encode(b"foo"), "Zm9v");
    assert_eq!(IconEncoder::base64_encode(b"foobar"), "Zm9vYmFy");
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn live_enumeration_sees_current_process() {
    let running = ProcessEnumerator::running_process_names_lowercase();
    assert!(!running.is_empty());
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn live_listing_returns_processes_with_some_icons() {
    let list = ProcessEnumerator::list_selectable_processes();
    assert!(!list.is_empty());
    assert!(list.iter().any(|p| p.icon_data_url.is_some()));
  }
}
