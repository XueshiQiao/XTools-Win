use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::file_log::FileLog;

#[cfg(target_os = "windows")]
use windows::{
  core::w,
  Win32::Foundation::SYSTEMTIME,
  Win32::System::EventLog::{
    EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection, EvtRender,
    EvtRenderEventXml, EVT_HANDLE,
  },
};

const LOG: FileLog = FileLog::new("UsbEvents");
const MAX_QUERY_EVENTS: usize = 250;
const MAX_RECENT_EVENTS: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbEventKind {
  Arrived,
  Removed,
  SurpriseRemoved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbLiveEvent {
  pub timestamp: String,
  pub kind: UsbEventKind,
  pub device_name: String,
  pub instance_id: String,
  pub port_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PnpDisconnectSummary {
  pub counts_by_instance: HashMap<String, u32>,
  pub recent_events: Vec<UsbLiveEvent>,
}

#[cfg(target_os = "windows")]
struct EvtHandleGuard(EVT_HANDLE);

#[cfg(target_os = "windows")]
impl Drop for EvtHandleGuard {
  fn drop(&mut self) {
    if !self.0.is_invalid() {
      unsafe {
        let _ = EvtClose(self.0);
      }
    }
  }
}

pub struct UsbEventLogReader;

impl UsbEventLogReader {
  pub fn current_local_hh_mm_ss() -> String {
    #[cfg(target_os = "windows")]
    {
      #[link(name = "kernel32")]
      extern "system" {
        fn GetLocalTime(lp_system_time: *mut SYSTEMTIME);
      }
      let mut st = SYSTEMTIME::default();
      unsafe { GetLocalTime(&mut st) };
      format!("{:02}:{:02}:{:02}", st.wHour, st.wMinute, st.wSecond)
    }
    #[cfg(not(target_os = "windows"))]
    {
      "00:00:00".to_string()
    }
  }

  pub fn read_last_24h(device_names: &HashMap<String, String>) -> PnpDisconnectSummary {
    #[cfg(target_os = "windows")]
    {
      Self::read_last_24h_win32(device_names)
    }
    #[cfg(not(target_os = "windows"))]
    {
      let _ = device_names;
      PnpDisconnectSummary::default()
    }
  }

  #[cfg(target_os = "windows")]
  fn read_last_24h_win32(device_names: &HashMap<String, String>) -> PnpDisconnectSummary {
    let mut summary = PnpDisconnectSummary::default();
    let query = w!(
      "*[System[(EventID=1010 or EventID=1011) and TimeCreated[timediff(@SystemTime) <= 86400000]]]"
    );
    let channel = w!("Microsoft-Windows-Kernel-PnP/Device Management");

    let Ok(query_handle) = (unsafe {
      EvtQuery(
        None,
        channel,
        query,
        EvtQueryChannelPath.0 | EvtQueryReverseDirection.0,
      )
    }) else {
      LOG.warn("Unable to query Kernel-PnP/Device Management channel");
      return summary;
    };
    let _query_guard = EvtHandleGuard(query_handle);

    let mut total_read = 0usize;
    let mut raw_handles = [0isize; 32];

    while total_read < MAX_QUERY_EVENTS {
      let mut returned: u32 = 0;
      let res = unsafe {
        EvtNext(
          query_handle,
          &mut raw_handles,
          500,
          0,
          &mut returned,
        )
      };
      if res.is_err() || returned == 0 {
        break;
      }

      for &raw in raw_handles.iter().take(returned as usize) {
        let evt = EvtHandleGuard(EVT_HANDLE(raw));
        total_read += 1;
        if let Some(xml) = Self::render_event_xml(evt.0) {
          if let Some((timestamp, _event_id, instance_id)) = Self::parse_pnp_event_xml(&xml) {
            let key = instance_id.to_ascii_uppercase();
            *summary.counts_by_instance.entry(key.clone()).or_insert(0) += 1;
            if summary.recent_events.len() < MAX_RECENT_EVENTS {
              let device_name = device_names
                .get(&key)
                .cloned()
                .unwrap_or_else(|| Self::fallback_name_from_instance(&instance_id));
              summary.recent_events.push(UsbLiveEvent {
                timestamp,
                kind: UsbEventKind::SurpriseRemoved,
                device_name,
                instance_id,
                port_key: None,
              });
            }
          }
        }
      }
    }

    summary
  }

  #[cfg(target_os = "windows")]
  fn render_event_xml(event: EVT_HANDLE) -> Option<String> {
    let mut used: u32 = 0;
    let mut prop_count: u32 = 0;
    let _ = unsafe {
      EvtRender(
        None,
        event,
        EvtRenderEventXml.0,
        0,
        None,
        &mut used,
        &mut prop_count,
      )
    };
    if used < 2 {
      return None;
    }
    let word_len = (used as usize).div_ceil(2) + 1;
    let mut buf = vec![0u16; word_len];
    let ok = unsafe {
      EvtRender(
        None,
        event,
        EvtRenderEventXml.0,
        (buf.len() * 2) as u32,
        Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
        &mut used,
        &mut prop_count,
      )
    };
    if ok.is_err() {
      return None;
    }
    let nul = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..nul]))
  }

  /// Parses rendered Windows Event XML and returns `(local_hh_mm_ss, event_id, instance_id)`
  /// strictly for USB device instances (`USB\...`). Non-USB devices (e.g. `SWD\...`) return `None`.
  pub fn parse_pnp_event_xml(xml: &str) -> Option<(String, u32, String)> {
    let event_id = Self::extract_tag_text(xml, "EventID")
      .and_then(|s| s.trim().parse::<u32>().ok())?;
    let utc_iso = Self::extract_attr(xml, "SystemTime")?;
    let local_time = Self::format_utc_iso_to_local_hh_mm_ss(&utc_iso)?;
    let instance_id = Self::extract_usb_instance_id(xml)?;
    Some((local_time, event_id, instance_id))
  }

  fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open_prefix = format!("<{}", tag);
    let start = xml.find(&open_prefix)?;
    let gt = xml[start..].find('>')? + start + 1;
    let close = format!("</{}>", tag);
    let end = xml[gt..].find(&close)? + gt;
    Some(Self::unescape_xml(&xml[gt..end]))
  }

  fn extract_attr(xml: &str, attr: &str) -> Option<String> {
    for quote in ['\'', '"'] {
      let pat = format!("{}={}", attr, quote);
      if let Some(pos) = xml.find(&pat) {
        let val_start = pos + pat.len();
        if let Some(end) = xml[val_start..].find(quote) {
          return Some(xml[val_start..val_start + end].to_string());
        }
      }
    }
    None
  }

  fn extract_usb_instance_id(xml: &str) -> Option<String> {
    let mut rest = xml;
    while let Some(pos) = rest.find("<Data") {
      rest = &rest[pos..];
      let Some(gt) = rest.find('>') else {
        break;
      };
      let tag_head = &rest[..gt];
      let after_gt = &rest[gt + 1..];
      let Some(close_pos) = after_gt.find("</Data>") else {
        break;
      };
      let inner = Self::unescape_xml(after_gt[..close_pos].trim());
      if inner.to_ascii_uppercase().starts_with("USB\\") {
        return Some(inner);
      }
      if tag_head.contains("DeviceInstanceId") {
        return None;
      }
      rest = &after_gt[close_pos + 7..];
    }
    None
  }

  fn unescape_xml(raw: &str) -> String {
    raw
      .replace("&amp;", "&")
      .replace("&lt;", "<")
      .replace("&gt;", ">")
      .replace("&quot;", "\"")
      .replace("&apos;", "'")
  }

  fn format_utc_iso_to_local_hh_mm_ss(iso: &str) -> Option<String> {
    // Expected prefix: YYYY-MM-DDTHH:MM:SS...
    if iso.len() < 19 {
      return None;
    }
    let year = iso.get(0..4)?.parse::<u16>().ok()?;
    let month = iso.get(5..7)?.parse::<u16>().ok()?;
    let day = iso.get(8..10)?.parse::<u16>().ok()?;
    let hour = iso.get(11..13)?.parse::<u16>().ok()?;
    let minute = iso.get(14..16)?.parse::<u16>().ok()?;
    let second = iso.get(17..19)?.parse::<u16>().ok()?;

    #[cfg(target_os = "windows")]
    {
      #[link(name = "kernel32")]
      extern "system" {
        fn SystemTimeToTzSpecificLocalTime(
          lp_time_zone_information: *const core::ffi::c_void,
          lp_universal_time: *const SYSTEMTIME,
          lp_local_time: *mut SYSTEMTIME,
        ) -> i32;
      }
      let utc = SYSTEMTIME {
        wYear: year,
        wMonth: month,
        wDayOfWeek: 0,
        wDay: day,
        wHour: hour,
        wMinute: minute,
        wSecond: second,
        wMilliseconds: 0,
      };
      let mut local = SYSTEMTIME::default();
      let ok = unsafe { SystemTimeToTzSpecificLocalTime(core::ptr::null(), &utc, &mut local) };
      if ok != 0 {
        return Some(format!(
          "{:02}:{:02}:{:02}",
          local.wHour, local.wMinute, local.wSecond
        ));
      }
    }

    Some(format!("{:02}:{:02}:{:02}", hour, minute, second))
  }

  fn fallback_name_from_instance(instance_id: &str) -> String {
    let upper = instance_id.to_ascii_uppercase();
    if let Some(vid_pos) = upper.find("VID_") {
      if let Some(pid_pos) = upper[vid_pos..].find("PID_") {
        let vid = &upper[vid_pos + 4..].get(..4).unwrap_or("????");
        let pid = &upper[vid_pos + pid_pos + 4..].get(..4).unwrap_or("????");
        return format!("USB ({}:{})", vid, pid);
      }
    }
    instance_id.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_usb_surprise_removal_event_xml() {
    let xml = r#"<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'><System><Provider Name='Microsoft-Windows-Kernel-PnP'/><EventID Qualifiers='0'>1010</EventID><TimeCreated SystemTime='2026-11-25T11:33:47.1234567Z'/></System><EventData><Data Name='DeviceInstanceId'>USB\VID_27C6&amp;PID_5584\6&amp;2adbafcd&amp;0&amp;8</Data><Data Name='RemovedCount'>1</Data></EventData></Event>"#;
    let parsed = UsbEventLogReader::parse_pnp_event_xml(xml).unwrap();
    assert_eq!(parsed.1, 1010);
    assert_eq!(parsed.2, r"USB\VID_27C6&PID_5584\6&2adbafcd&0&8");
    assert_eq!(parsed.0.len(), 8);
  }

  #[test]
  fn rejects_non_usb_device_events() {
    let xml = r#"<Event><System><EventID>1010</EventID><TimeCreated SystemTime='2026-11-25T11:33:47.000Z'/></System><EventData><Data Name='DeviceInstanceId'>SWD\DAFUPnPProvider\uuid:1234</Data></EventData></Event>"#;
    assert!(UsbEventLogReader::parse_pnp_event_xml(xml).is_none());
  }
}
