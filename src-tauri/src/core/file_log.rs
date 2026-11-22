use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
static LOG_WRITER: OnceLock<Mutex<LogWriter>> = OnceLock::new();

#[cfg(target_os = "windows")]
#[repr(C)]
struct WinSystemTime {
  year: u16,
  month: u16,
  day_of_week: u16,
  day: u16,
  hour: u16,
  minute: u16,
  second: u16,
  milliseconds: u16,
}

struct LogWriter {
  path: PathBuf,
  file: Option<File>,
}

impl LogWriter {
  fn init() -> Self {
    let path = log_file_path();
    if let Some(parent) = path.parent() {
      let _ = fs::create_dir_all(parent);
    }
    Self::rotate_if_needed(&path);
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    Self { path, file }
  }

  fn rotate_if_needed(path: &PathBuf) {
    if let Ok(meta) = fs::metadata(path) {
      if meta.len() >= MAX_LOG_BYTES {
        let backup = path.with_file_name("xtools.log.old");
        let _ = fs::remove_file(&backup);
        let _ = fs::rename(path, backup);
      }
    }
  }

  fn write_line(&mut self, level: &str, tag: &str, msg: &str) {
    Self::rotate_if_needed(&self.path);
    if self.file.is_none() || !self.path.exists() {
      self.file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
    }
    let line = format!("[{}][{}][{}] {}\n", timestamp_string(), level, tag, msg);
    if let Some(file) = &mut self.file {
      let _ = file.write_all(line.as_bytes());
      let _ = file.flush();
    }
    #[cfg(debug_assertions)]
    eprint!("{}", line);
  }
}

fn timestamp_string() -> String {
  #[cfg(target_os = "windows")]
  {
    #[link(name = "kernel32")]
    extern "system" {
      fn GetLocalTime(lp_system_time: *mut WinSystemTime);
    }
    let mut st = WinSystemTime {
      year: 0,
      month: 0,
      day_of_week: 0,
      day: 0,
      hour: 0,
      minute: 0,
      second: 0,
      milliseconds: 0,
    };
    unsafe { GetLocalTime(&mut st) };
    format!(
      "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
      st.year, st.month, st.day, st.hour, st.minute, st.second, st.milliseconds
    )
  }
  #[cfg(not(target_os = "windows"))]
  {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|d| d.as_secs())
      .unwrap_or(0);
    format!("{}", secs)
  }
}

pub fn log_file_path() -> PathBuf {
  let base = std::env::var_os("LOCALAPPDATA")
    .map(PathBuf::from)
    .unwrap_or_else(std::env::temp_dir);
  base.join("XTools").join("xtools.log")
}

pub fn reveal_in_explorer() -> Result<(), String> {
  let path = log_file_path();
  if let Some(parent) = path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  if !path.exists() {
    let _ = File::create(&path);
  }
  Command::new("explorer.exe")
    .arg(format!("/select,{}", path.display()))
    .spawn()
    .map_err(|e| format!("Failed to open Explorer: {}", e))?;
  Ok(())
}

/// Structured file logger matching macOS XTools' `FileLog`.
#[derive(Clone, Copy)]
pub struct FileLog {
  tag: &'static str,
}

impl FileLog {
  pub const fn new(tag: &'static str) -> Self {
    Self { tag }
  }

  pub fn info(&self, msg: &str) {
    self.write("INFO", msg);
  }

  pub fn warn(&self, msg: &str) {
    self.write("WARN", msg);
  }

  pub fn error(&self, msg: &str) {
    self.write("ERROR", msg);
  }

  fn write(&self, level: &str, msg: &str) {
    let lock = LOG_WRITER.get_or_init(|| Mutex::new(LogWriter::init()));
    if let Ok(mut writer) = lock.lock() {
      writer.write_line(level, self.tag, msg);
    }
  }
}
