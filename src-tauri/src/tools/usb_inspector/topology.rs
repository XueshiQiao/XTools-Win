use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

use super::config::UsbInspectorConfig;
use super::events::{PnpDisconnectSummary, UsbEventLogReader, UsbLiveEvent};

#[cfg(target_os = "windows")]
use windows::{
  core::w,
  Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceInterfaceDetailW,
    SetupDiGetDevicePropertyW, DIGCF_ALLCLASSES, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    HDEVINFO, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
  },
  Win32::Devices::Properties::{
    DEVPKEY_Device_BusReportedDeviceDesc, DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_Driver,
    DEVPKEY_Device_DriverProblemDesc, DEVPKEY_Device_FriendlyName, DEVPKEY_Device_LocationPaths,
    DEVPKEY_Device_Manufacturer, DEVPKEY_Device_ProblemCode, DEVPROPTYPE,
  },
  Win32::Devices::Usb::{
    DeviceConnected, GUID_DEVINTERFACE_USB_HOST_CONTROLLER, IOCTL_USB_GET_DESCRIPTOR_FROM_NODE_CONNECTION,
    IOCTL_USB_GET_NODE_CONNECTION_DRIVERKEY_NAME, IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX,
    IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX_V2, IOCTL_USB_GET_NODE_CONNECTION_NAME,
    IOCTL_USB_GET_NODE_INFORMATION, IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES,
    IOCTL_USB_GET_ROOT_HUB_NAME, USB_CONFIGURATION_DESCRIPTOR_TYPE, USB_DESCRIPTOR_REQUEST,
    USB_NODE_CONNECTION_DRIVERKEY_NAME, USB_NODE_CONNECTION_INFORMATION_EX,
    USB_NODE_CONNECTION_INFORMATION_EX_V2, USB_NODE_CONNECTION_NAME, USB_NODE_INFORMATION,
    USB_PIPE_INFO, USB_PORT_CONNECTOR_PROPERTIES, USB_ROOT_HUB_NAME,
  },
  Win32::Foundation::{CloseHandle, DEVPROPKEY, HANDLE, INVALID_HANDLE_VALUE},
  Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
  },
  Win32::System::IO::DeviceIoControl,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbControllerRole {
  CpuDirect,
  Discrete,
  Chipset,
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbSpeed {
  Low,
  Full,
  High,
  Super,
  SuperPlus,
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbPortStatus {
  Connected,
  Empty,
  Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbDiagnosticSeverity {
  Error,
  Warning,
  Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbPortNode {
  pub port_key: String,
  pub port_number: u32,
  pub companion_port: Option<u32>,
  pub is_type_c: bool,
  pub is_user_connectable: bool,
  pub max_speed: UsbSpeed,
  pub current_speed: Option<UsbSpeed>,
  pub status: UsbPortStatus,
  pub device_name: Option<String>,
  pub manufacturer: Option<String>,
  pub vendor_id: Option<u16>,
  pub product_id: Option<u16>,
  pub instance_id: Option<String>,
  pub problem_code: Option<u32>,
  pub problem_desc: Option<String>,
  pub max_power_ma: Option<u16>,
  pub is_hub: bool,
  pub hub_children: Vec<UsbPortNode>,
  pub disconnect_count_24h: u32,
  pub custom_label: Option<String>,
  pub recently_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbControllerNode {
  pub id: String,
  pub name: String,
  pub vendor_id: u16,
  pub device_id: u16,
  pub role: UsbControllerRole,
  pub role_reason: String,
  pub ports: Vec<UsbPortNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbDiagnosticHint {
  pub severity: UsbDiagnosticSeverity,
  pub title_zh: String,
  pub title_en: String,
  pub detail_zh: String,
  pub detail_en: String,
  pub port_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UsbScanOutcome {
  pub controllers: Vec<UsbControllerNode>,
  pub diagnostics: Vec<UsbDiagnosticHint>,
  pub recent_events: Vec<UsbLiveEvent>,
}

#[derive(Debug, Clone)]
struct PnpDeviceMeta {
  instance_id: String,
  bus_reported_desc: Option<String>,
  friendly_name: Option<String>,
  device_desc: Option<String>,
  manufacturer: Option<String>,
  problem_code: Option<u32>,
  problem_desc: Option<String>,
}

impl PnpDeviceMeta {
  fn best_name(&self) -> Option<String> {
    self
      .bus_reported_desc
      .clone()
      .or_else(|| self.friendly_name.clone())
      .or_else(|| self.device_desc.clone())
  }
}

#[cfg(target_os = "windows")]
struct OwnedHandle(HANDLE);

#[cfg(target_os = "windows")]
impl Drop for OwnedHandle {
  fn drop(&mut self) {
    if !self.0.is_invalid() && self.0 != INVALID_HANDLE_VALUE {
      unsafe {
        let _ = CloseHandle(self.0);
      }
    }
  }
}

#[cfg(target_os = "windows")]
struct DevInfoGuard(HDEVINFO);

#[cfg(target_os = "windows")]
impl Drop for DevInfoGuard {
  fn drop(&mut self) {
    if !self.0.is_invalid() {
      unsafe {
        let _ = SetupDiDestroyDeviceInfoList(self.0);
      }
    }
  }
}

#[derive(Debug, Clone, Default)]
struct PnpInventory {
  by_driver_key: HashMap<String, PnpDeviceMeta>,
  names_by_instance: HashMap<String, String>,
}

pub struct UsbTopologyScanner;

impl UsbTopologyScanner {
  pub fn scan(
    config: &UsbInspectorConfig,
    recent_changed_keys: &HashSet<String>,
  ) -> UsbScanOutcome {
    #[cfg(target_os = "windows")]
    {
      Self::scan_win32(config, recent_changed_keys)
    }
    #[cfg(not(target_os = "windows"))]
    {
      let _ = (config, recent_changed_keys);
      UsbScanOutcome {
        controllers: Vec::new(),
        diagnostics: Vec::new(),
        recent_events: Vec::new(),
      }
    }
  }

  #[cfg(target_os = "windows")]
  fn scan_win32(
    config: &UsbInspectorConfig,
    recent_changed_keys: &HashSet<String>,
  ) -> UsbScanOutcome {
    let inventory = Self::collect_pnp_inventory();
    let disconnect_summary = UsbEventLogReader::read_last_24h(&inventory.names_by_instance);
    let mut controllers = Self::enumerate_controllers(&inventory, &disconnect_summary, config, recent_changed_keys);
    controllers.sort_by(|a, b| a.role.cmp(&b.role).then_with(|| a.name.cmp(&b.name)));

    let diagnostics = Self::analyze_diagnostics(&controllers);
    let mut recent_events = disconnect_summary.recent_events;
    let instance_to_port = Self::build_instance_port_map(&controllers);
    for ev in &mut recent_events {
      if let Some(pk) = instance_to_port.get(&ev.instance_id.to_ascii_uppercase()) {
        ev.port_key = Some(pk.clone());
      }
    }

    UsbScanOutcome {
      controllers,
      diagnostics,
      recent_events,
    }
  }

  fn build_instance_port_map(controllers: &[UsbControllerNode]) -> HashMap<String, String> {
    fn visit(ports: &[UsbPortNode], out: &mut HashMap<String, String>) {
      for p in ports {
        if let Some(inst) = &p.instance_id {
          out.insert(inst.to_ascii_uppercase(), p.port_key.clone());
        }
        visit(&p.hub_children, out);
      }
    }
    let mut map = HashMap::new();
    for c in controllers {
      visit(&c.ports, &mut map);
    }
    map
  }

  #[cfg(target_os = "windows")]
  fn collect_pnp_inventory() -> PnpInventory {
    let mut inv = PnpInventory::default();
    let Ok(dev_info) = (unsafe {
      SetupDiGetClassDevsW(
        None,
        w!("USB"),
        None,
        DIGCF_ALLCLASSES | DIGCF_PRESENT,
      )
    }) else {
      return inv;
    };
    let _guard = DevInfoGuard(dev_info);

    let mut idx = 0u32;
    loop {
      let mut data = SP_DEVINFO_DATA {
        cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
      };
      if unsafe { SetupDiEnumDeviceInfo(dev_info, idx, &mut data) }.is_err() {
        break;
      }
      idx += 1;

      let Some(instance_id) = Self::get_device_instance_id(dev_info, &data) else {
        continue;
      };
      let driver_key = Self::get_devprop_string(dev_info, &data, &DEVPKEY_Device_Driver);
      let bus_reported_desc =
        Self::get_devprop_string(dev_info, &data, &DEVPKEY_Device_BusReportedDeviceDesc);
      let friendly_name = Self::get_devprop_string(dev_info, &data, &DEVPKEY_Device_FriendlyName);
      let device_desc = Self::get_devprop_string(dev_info, &data, &DEVPKEY_Device_DeviceDesc);
      let manufacturer = Self::get_devprop_string(dev_info, &data, &DEVPKEY_Device_Manufacturer);
      let problem_code =
        Self::get_devprop_u32(dev_info, &data, &DEVPKEY_Device_ProblemCode).filter(|&c| c != 0);
      let problem_desc = if problem_code.is_some() {
        Self::get_devprop_string(dev_info, &data, &DEVPKEY_Device_DriverProblemDesc)
      } else {
        None
      };

      let meta = PnpDeviceMeta {
        instance_id: instance_id.clone(),
        bus_reported_desc,
        friendly_name,
        device_desc,
        manufacturer,
        problem_code,
        problem_desc,
      };
      if let Some(best) = meta.best_name() {
        inv
          .names_by_instance
          .insert(instance_id.to_ascii_uppercase(), best);
      }
      if let Some(dk) = driver_key {
        inv.by_driver_key.insert(dk.to_ascii_uppercase(), meta);
      }
    }

    inv
  }

  #[cfg(target_os = "windows")]
  fn enumerate_controllers(
    inventory: &PnpInventory,
    disconnect_summary: &PnpDisconnectSummary,
    config: &UsbInspectorConfig,
    recent_changed_keys: &HashSet<String>,
  ) -> Vec<UsbControllerNode> {
    let mut controllers = Vec::new();
    let Ok(dev_info) = (unsafe {
      SetupDiGetClassDevsW(
        Some(&GUID_DEVINTERFACE_USB_HOST_CONTROLLER),
        None,
        None,
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
      )
    }) else {
      return controllers;
    };
    let _guard = DevInfoGuard(dev_info);

    let mut idx = 0u32;
    loop {
      let mut if_data = SP_DEVICE_INTERFACE_DATA {
        cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
      };
      if unsafe {
        SetupDiEnumDeviceInterfaces(
          dev_info,
          None,
          &GUID_DEVINTERFACE_USB_HOST_CONTROLLER,
          idx,
          &mut if_data,
        )
      }
      .is_err()
      {
        break;
      }
      idx += 1;

      let mut dev_data = SP_DEVINFO_DATA {
        cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
      };
      let Some(hcd_path) = Self::get_interface_path(dev_info, &if_data, &mut dev_data) else {
        continue;
      };
      let instance_id =
        Self::get_device_instance_id(dev_info, &dev_data).unwrap_or_else(|| hcd_path.clone());
      let friendly = Self::get_devprop_string(dev_info, &dev_data, &DEVPKEY_Device_FriendlyName);
      let desc = Self::get_devprop_string(dev_info, &dev_data, &DEVPKEY_Device_DeviceDesc);
      let name = friendly
        .or(desc)
        .unwrap_or_else(|| "USB Host Controller".to_string());
      let location_paths =
        Self::get_devprop_string_list(dev_info, &dev_data, &DEVPKEY_Device_LocationPaths);

      let (vendor_id, device_id, subsys_vid) = Self::parse_pci_ids(&instance_id);
      let (role, role_reason) = Self::classify_controller(
        &name,
        &instance_id,
        vendor_id,
        device_id,
        subsys_vid,
        &location_paths,
      );

      let ctrl_id = Self::make_controller_id(&instance_id, vendor_id, device_id, idx);
      let mut ports = Vec::new();
      if let Some(hcd_handle) = Self::open_device_handle(&hcd_path) {
        if let Some(root_hub_name) = Self::get_root_hub_name(hcd_handle.0) {
          let root_hub_path = Self::normalize_device_path(&root_hub_name);
          ports = Self::scan_hub(
            &root_hub_path,
            &ctrl_id,
            "root",
            0,
            inventory,
            disconnect_summary,
            config,
            recent_changed_keys,
          );
        }
      }

      controllers.push(UsbControllerNode {
        id: ctrl_id,
        name,
        vendor_id,
        device_id,
        role,
        role_reason,
        ports,
      });
    }

    controllers
  }

  #[cfg(target_os = "windows")]
  fn open_device_handle(path: &str) -> Option<OwnedHandle> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let handle = unsafe {
      CreateFileW(
        windows::core::PCWSTR(wide.as_ptr()),
        FILE_GENERIC_WRITE.0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None,
      )
    }
    .or_else(|_| unsafe {
      CreateFileW(
        windows::core::PCWSTR(wide.as_ptr()),
        0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None,
      )
    })
    .ok()?;
    if handle.is_invalid() || handle == INVALID_HANDLE_VALUE {
      return None;
    }
    Some(OwnedHandle(handle))
  }

  #[cfg(target_os = "windows")]
  fn normalize_device_path(name: &str) -> String {
    if name.starts_with(r"\\.\") || name.starts_with(r"\\?\") {
      name.to_string()
    } else {
      format!(r"\\.\{}", name)
    }
  }

  #[cfg(target_os = "windows")]
  fn get_root_hub_name(hcd: HANDLE) -> Option<String> {
    let mut buf = [0u8; 512];
    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hcd,
        IOCTL_USB_GET_ROOT_HUB_NAME,
        None,
        0,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
        buf.len() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() || returned < std::mem::size_of::<USB_ROOT_HUB_NAME>() as u32 {
      return None;
    }
    let name_offset = std::mem::offset_of!(USB_ROOT_HUB_NAME, RootHubName);
    Self::read_utf16_from_bytes(&buf[name_offset..returned as usize])
  }

  #[cfg(target_os = "windows")]
  #[allow(clippy::too_many_arguments)]
  fn scan_hub(
    hub_path: &str,
    ctrl_id: &str,
    hub_chain: &str,
    depth: usize,
    inventory: &PnpInventory,
    disconnect_summary: &PnpDisconnectSummary,
    config: &UsbInspectorConfig,
    recent_changed_keys: &HashSet<String>,
  ) -> Vec<UsbPortNode> {
    if depth > 6 {
      return Vec::new();
    }
    let Some(hub_handle) = Self::open_device_handle(hub_path) else {
      return Vec::new();
    };

    let mut node_info = USB_NODE_INFORMATION::default();
    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hub_handle.0,
        IOCTL_USB_GET_NODE_INFORMATION,
        Some(&node_info as *const _ as *const core::ffi::c_void),
        std::mem::size_of::<USB_NODE_INFORMATION>() as u32,
        Some(&mut node_info as *mut _ as *mut core::ffi::c_void),
        std::mem::size_of::<USB_NODE_INFORMATION>() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() {
      return Vec::new();
    }

    let num_ports = unsafe {
      std::ptr::addr_of!(node_info.u.HubInformation.HubDescriptor.bNumberOfPorts).read_unaligned()
    };
    let mut raw_ports = Vec::with_capacity(usize::from(num_ports));

    for port_idx in 1..=u32::from(num_ports) {
      let port_node = Self::scan_single_port(
        hub_handle.0,
        port_idx,
        ctrl_id,
        hub_chain,
        depth,
        inventory,
        disconnect_summary,
        config,
        recent_changed_keys,
      );
      if let Some(p) = port_node {
        raw_ports.push(p);
      }
    }

    Self::merge_companion_ports(raw_ports, config, recent_changed_keys)
  }

  #[cfg(target_os = "windows")]
  #[allow(clippy::too_many_arguments)]
  fn scan_single_port(
    hub: HANDLE,
    port_idx: u32,
    ctrl_id: &str,
    hub_chain: &str,
    depth: usize,
    inventory: &PnpInventory,
    disconnect_summary: &PnpDisconnectSummary,
    config: &UsbInspectorConfig,
    recent_changed_keys: &HashSet<String>,
  ) -> Option<UsbPortNode> {
    let buf_size =
      std::mem::size_of::<USB_NODE_CONNECTION_INFORMATION_EX>() + 30 * std::mem::size_of::<USB_PIPE_INFO>();
    let mut conn_buf = vec![0u8; buf_size];
    unsafe {
      let ptr = conn_buf.as_mut_ptr() as *mut USB_NODE_CONNECTION_INFORMATION_EX;
      std::ptr::addr_of_mut!((*ptr).ConnectionIndex).write_unaligned(port_idx);
    }

    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hub,
        IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX,
        Some(conn_buf.as_ptr() as *const core::ffi::c_void),
        conn_buf.len() as u32,
        Some(conn_buf.as_mut_ptr() as *mut core::ffi::c_void),
        conn_buf.len() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() {
      return None;
    }

    let (conn_status, speed_raw, is_hub, vid, pid) = unsafe {
      let ptr = conn_buf.as_ptr() as *const USB_NODE_CONNECTION_INFORMATION_EX;
      let status = std::ptr::addr_of!((*ptr).ConnectionStatus).read_unaligned();
      let speed = std::ptr::addr_of!((*ptr).Speed).read_unaligned();
      let hub_flag = std::ptr::addr_of!((*ptr).DeviceIsHub).read_unaligned();
      let desc = std::ptr::addr_of!((*ptr).DeviceDescriptor).read_unaligned();
      (status, speed, hub_flag, desc.idVendor, desc.idProduct)
    };

    let (max_speed, is_super_plus_active) = Self::query_port_speed_v2(hub, port_idx);
    let (is_user_connectable, is_type_c, companion_port) =
      Self::query_port_connector_props(hub, port_idx);

    let status = if conn_status.0 == 0 {
      UsbPortStatus::Empty
    } else if conn_status == DeviceConnected {
      UsbPortStatus::Connected
    } else {
      UsbPortStatus::Error
    };

    let port_key = format!("{}/{}/p{}", ctrl_id, hub_chain, port_idx);

    let mut device_name: Option<String> = None;
    let mut manufacturer: Option<String> = None;
    let mut vendor_id: Option<u16> = None;
    let mut product_id: Option<u16> = None;
    let mut instance_id: Option<String> = None;
    let mut problem_code: Option<u32> = None;
    let mut problem_desc: Option<String> = None;
    let mut current_speed: Option<UsbSpeed> = None;
    let mut max_power_ma: Option<u16> = None;
    let mut hub_children = Vec::new();
    let mut disconnect_count_24h = 0u32;

    if status != UsbPortStatus::Empty {
      if vid != 0 || pid != 0 {
        vendor_id = Some(vid);
        product_id = Some(pid);
      }

      if status == UsbPortStatus::Connected {
        let cur_spd = if is_super_plus_active {
          UsbSpeed::SuperPlus
        } else {
          match speed_raw {
            0 => UsbSpeed::Low,
            1 => UsbSpeed::Full,
            2 => UsbSpeed::High,
            3 => UsbSpeed::Super,
            _ => UsbSpeed::Unknown,
          }
        };
        current_speed = Some(cur_spd);
        max_power_ma = Self::query_max_power_ma(hub, port_idx, cur_spd);
      }

      if let Some(dk) = Self::query_driver_key(hub, port_idx) {
        if let Some(meta) = inventory.by_driver_key.get(&dk.to_ascii_uppercase()) {
          device_name = meta.best_name();
          manufacturer = meta.manufacturer.clone();
          instance_id = Some(meta.instance_id.clone());
          problem_code = meta.problem_code;
          problem_desc = meta.problem_desc.clone();
          if let Some(&cnt) = disconnect_summary
            .counts_by_instance
            .get(&meta.instance_id.to_ascii_uppercase())
          {
            disconnect_count_24h = cnt;
          }
        }
      }

      if let Some(v) = vendor_id {
        if manufacturer.as_deref().map_or(true, |m| {
          m.contains("Standard") || m.contains("Microsoft") || m.trim().is_empty()
        }) {
          if let Some(vn) = Self::vendor_name(v) {
            manufacturer = Some(vn.to_string());
          }
        }
      }

      if device_name.is_none() {
        device_name = if status == UsbPortStatus::Error {
          Some(Self::connection_error_label(conn_status.0))
        } else if let (Some(v), Some(p)) = (vendor_id, product_id) {
          Some(format!("USB Device ({:04X}:{:04X})", v, p))
        } else {
          Some("USB Device".to_string())
        };
      }

      if is_hub && status == UsbPortStatus::Connected {
        if let Some(ext_hub_name) = Self::query_ext_hub_name(hub, port_idx) {
          let ext_hub_path = Self::normalize_device_path(&ext_hub_name);
          let next_chain = format!("{}.p{}", hub_chain, port_idx);
          hub_children = Self::scan_hub(
            &ext_hub_path,
            ctrl_id,
            &next_chain,
            depth + 1,
            inventory,
            disconnect_summary,
            config,
            recent_changed_keys,
          );
        }
      }
    }

    let custom_label = config.port_labels.get(&port_key).cloned();
    let recently_changed = recent_changed_keys.contains(&port_key);

    Some(UsbPortNode {
      port_key,
      port_number: port_idx,
      companion_port,
      is_type_c,
      is_user_connectable,
      max_speed,
      current_speed,
      status,
      device_name,
      manufacturer,
      vendor_id,
      product_id,
      instance_id,
      problem_code,
      problem_desc,
      max_power_ma,
      is_hub,
      hub_children,
      disconnect_count_24h,
      custom_label,
      recently_changed,
    })
  }

  #[cfg(target_os = "windows")]
  fn query_port_speed_v2(hub: HANDLE, port_idx: u32) -> (UsbSpeed, bool) {
    let mut v2 = USB_NODE_CONNECTION_INFORMATION_EX_V2::default();
    v2.ConnectionIndex = port_idx;
    v2.Length = std::mem::size_of::<USB_NODE_CONNECTION_INFORMATION_EX_V2>() as u32;
    v2.SupportedUsbProtocols.ul = 0x07;

    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hub,
        IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX_V2,
        Some(&v2 as *const _ as *const core::ffi::c_void),
        std::mem::size_of::<USB_NODE_CONNECTION_INFORMATION_EX_V2>() as u32,
        Some(&mut v2 as *mut _ as *mut core::ffi::c_void),
        std::mem::size_of::<USB_NODE_CONNECTION_INFORMATION_EX_V2>() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() {
      return (UsbSpeed::High, false);
    }

    let proto_ul = unsafe { std::ptr::addr_of!(v2.SupportedUsbProtocols.ul).read_unaligned() };
    let flags_ul = unsafe { std::ptr::addr_of!(v2.Flags.ul).read_unaligned() };
    let max_speed = if (flags_ul & 0x08) != 0 {
      UsbSpeed::SuperPlus
    } else if (proto_ul & 0x04) != 0 {
      UsbSpeed::Super
    } else if (proto_ul & 0x02) != 0 {
      UsbSpeed::High
    } else {
      UsbSpeed::Full
    };
    let is_super_plus_active = (flags_ul & 0x04) != 0;
    (max_speed, is_super_plus_active)
  }

  #[cfg(target_os = "windows")]
  fn query_port_connector_props(hub: HANDLE, port_idx: u32) -> (bool, bool, Option<u32>) {
    let mut buf = [0u8; 512];
    unsafe {
      let ptr = buf.as_mut_ptr() as *mut USB_PORT_CONNECTOR_PROPERTIES;
      std::ptr::addr_of_mut!((*ptr).ConnectionIndex).write_unaligned(port_idx);
    }
    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hub,
        IOCTL_USB_GET_PORT_CONNECTOR_PROPERTIES,
        Some(buf.as_ptr() as *const core::ffi::c_void),
        buf.len() as u32,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
        buf.len() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() || (returned as usize) < std::mem::size_of::<USB_PORT_CONNECTOR_PROPERTIES>() {
      return (true, false, None);
    }

    let (props_ul, companion_num) = unsafe {
      let ptr = buf.as_ptr() as *const USB_PORT_CONNECTOR_PROPERTIES;
      (
        std::ptr::addr_of!((*ptr).UsbPortProperties.ul).read_unaligned(),
        std::ptr::addr_of!((*ptr).CompanionPortNumber).read_unaligned(),
      )
    };
    let is_user_connectable = (props_ul & 0x01) != 0;
    let is_type_c = (props_ul & 0x08) != 0;
    let companion = if companion_num != 0 && u32::from(companion_num) != port_idx {
      Some(u32::from(companion_num))
    } else {
      None
    };
    (is_user_connectable, is_type_c, companion)
  }

  #[cfg(target_os = "windows")]
  fn query_driver_key(hub: HANDLE, port_idx: u32) -> Option<String> {
    let mut buf = [0u8; 512];
    unsafe {
      let ptr = buf.as_mut_ptr() as *mut USB_NODE_CONNECTION_DRIVERKEY_NAME;
      std::ptr::addr_of_mut!((*ptr).ConnectionIndex).write_unaligned(port_idx);
    }
    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hub,
        IOCTL_USB_GET_NODE_CONNECTION_DRIVERKEY_NAME,
        Some(buf.as_ptr() as *const core::ffi::c_void),
        buf.len() as u32,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
        buf.len() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() {
      return None;
    }
    let offset = std::mem::offset_of!(USB_NODE_CONNECTION_DRIVERKEY_NAME, DriverKeyName);
    if (returned as usize) <= offset {
      return None;
    }
    Self::read_utf16_from_bytes(&buf[offset..returned as usize])
  }

  #[cfg(target_os = "windows")]
  fn query_ext_hub_name(hub: HANDLE, port_idx: u32) -> Option<String> {
    let mut buf = [0u8; 512];
    unsafe {
      let ptr = buf.as_mut_ptr() as *mut USB_NODE_CONNECTION_NAME;
      std::ptr::addr_of_mut!((*ptr).ConnectionIndex).write_unaligned(port_idx);
    }
    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hub,
        IOCTL_USB_GET_NODE_CONNECTION_NAME,
        Some(buf.as_ptr() as *const core::ffi::c_void),
        buf.len() as u32,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
        buf.len() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() {
      return None;
    }
    let offset = std::mem::offset_of!(USB_NODE_CONNECTION_NAME, NodeName);
    if (returned as usize) <= offset {
      return None;
    }
    Self::read_utf16_from_bytes(&buf[offset..returned as usize])
  }

  #[cfg(target_os = "windows")]
  fn query_max_power_ma(hub: HANDLE, port_idx: u32, speed: UsbSpeed) -> Option<u16> {
    const CONFIG_LEN: usize = 64;
    let data_offset = std::mem::offset_of!(USB_DESCRIPTOR_REQUEST, Data);
    let mut buf = vec![0u8; data_offset + CONFIG_LEN];
    unsafe {
      let req = buf.as_mut_ptr() as *mut USB_DESCRIPTOR_REQUEST;
      std::ptr::addr_of_mut!((*req).ConnectionIndex).write_unaligned(port_idx);
      std::ptr::addr_of_mut!((*req).SetupPacket.bmRequest).write_unaligned(0x80);
      std::ptr::addr_of_mut!((*req).SetupPacket.bRequest).write_unaligned(0x06);
      std::ptr::addr_of_mut!((*req).SetupPacket.wValue)
        .write_unaligned(((USB_CONFIGURATION_DESCRIPTOR_TYPE as u16) << 8) | 0);
      std::ptr::addr_of_mut!((*req).SetupPacket.wIndex).write_unaligned(0);
      std::ptr::addr_of_mut!((*req).SetupPacket.wLength).write_unaligned(CONFIG_LEN as u16);
    }

    let mut returned: u32 = 0;
    let ok = unsafe {
      DeviceIoControl(
        hub,
        IOCTL_USB_GET_DESCRIPTOR_FROM_NODE_CONNECTION,
        Some(buf.as_ptr() as *const core::ffi::c_void),
        buf.len() as u32,
        Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
        buf.len() as u32,
        Some(&mut returned),
        None,
      )
    };
    if ok.is_err() || (returned as usize) < data_offset + 9 {
      return None;
    }
    let desc = &buf[data_offset..returned as usize];
    if desc[0] < 9 || desc[1] != USB_CONFIGURATION_DESCRIPTOR_TYPE as u8 {
      return None;
    }
    let raw_power = u16::from(desc[8]);
    if raw_power == 0 {
      return None;
    }
    let mult = if matches!(speed, UsbSpeed::Super | UsbSpeed::SuperPlus) {
      8
    } else {
      2
    };
    Some(raw_power * mult)
  }

  #[cfg(target_os = "windows")]
  fn connection_error_label(status_code: i32) -> String {
    match status_code {
      2 => "Enumeration Failed (Error 43)".to_string(),
      3 => "USB General Failure".to_string(),
      5 => "Not Enough Power".to_string(),
      6 => "Not Enough Bandwidth".to_string(),
      7 => "Hub Nested Too Deeply".to_string(),
      _ => format!("USB Error ({})", status_code),
    }
  }

  /// Merges xHCI companion port pairs (where one physical socket exposes both a USB 2.0
  /// logical port and a USB 3.x logical port pointing to each other) into a single socket,
  /// keeping the canonical lower `port_number` key and retaining the active half's device info.
  pub fn merge_companion_ports(
    raw_ports: Vec<UsbPortNode>,
    config: &UsbInspectorConfig,
    recent_changed_keys: &HashSet<String>,
  ) -> Vec<UsbPortNode> {
    let mut by_num: BTreeMap<u32, UsbPortNode> =
      raw_ports.into_iter().map(|p| (p.port_number, p)).collect();
    let numbers: Vec<u32> = by_num.keys().copied().collect();
    let mut consumed = HashSet::new();
    let mut merged = Vec::new();

    for num in numbers {
      if consumed.contains(&num) {
        continue;
      }
      let Some(primary) = by_num.remove(&num) else {
        continue;
      };
      consumed.insert(num);

      let socket = if let Some(comp_num) = primary.companion_port {
        if !consumed.contains(&comp_num) {
          if let Some(partner) = by_num.remove(&comp_num) {
            consumed.insert(comp_num);
            Self::combine_companion_pair(primary, partner, config, recent_changed_keys)
          } else {
            primary
          }
        } else {
          primary
        }
      } else {
        primary
      };

      if socket.status != UsbPortStatus::Empty || socket.is_user_connectable {
        merged.push(socket);
      }
    }

    merged
  }

  fn combine_companion_pair(
    a: UsbPortNode,
    b: UsbPortNode,
    config: &UsbInspectorConfig,
    recent_changed_keys: &HashSet<String>,
  ) -> UsbPortNode {
    let (low, high) = if a.port_number <= b.port_number {
      (a, b)
    } else {
      (b, a)
    };
    let canonical_key = low.port_key.clone();
    let canonical_num = low.port_number;
    let partner_num = high.port_number;
    let max_speed = low.max_speed.max(high.max_speed);
    let is_type_c = low.is_type_c || high.is_type_c;
    let is_user_connectable = low.is_user_connectable || high.is_user_connectable;

    let active_half = match (low.status, high.status) {
      (UsbPortStatus::Connected, _) => low,
      (_, UsbPortStatus::Connected) => high,
      (UsbPortStatus::Error, _) => low,
      (_, UsbPortStatus::Error) => high,
      _ => low,
    };

    let custom_label = config
      .port_labels
      .get(&canonical_key)
      .cloned()
      .or(active_half.custom_label);
    let recently_changed = active_half.recently_changed || recent_changed_keys.contains(&canonical_key);

    UsbPortNode {
      port_key: canonical_key,
      port_number: canonical_num,
      companion_port: Some(partner_num),
      is_type_c,
      is_user_connectable,
      max_speed,
      custom_label,
      recently_changed,
      ..active_half
    }
  }

  pub fn parse_pci_ids(instance_id: &str) -> (u16, u16, Option<u16>) {
    let upper = instance_id.to_ascii_uppercase();
    let vid = Self::extract_hex_tag(&upper, "VID_")
      .or_else(|| Self::extract_hex_tag(&upper, "VEN_"))
      .unwrap_or(0);
    let did = Self::extract_hex_tag(&upper, "PID_")
      .or_else(|| Self::extract_hex_tag(&upper, "DEV_"))
      .unwrap_or(0);
    let subsys_vid = upper.find("SUBSYS_").and_then(|pos| {
      let s = upper.get(pos + 7..pos + 15)?;
      u16::from_str_radix(&s[4..8], 16).ok()
    });
    (vid, did, subsys_vid)
  }

  fn extract_hex_tag(upper: &str, prefix: &str) -> Option<u16> {
    let pos = upper.find(prefix)?;
    let hex = upper.get(pos + prefix.len()..pos + prefix.len() + 4)?;
    u16::from_str_radix(hex, 16).ok()
  }

  pub fn classify_controller(
    name: &str,
    _instance_id: &str,
    vendor_id: u16,
    device_id: u16,
    subsys_vid: Option<u16>,
    location_paths: &[String],
  ) -> (UsbControllerRole, String) {
    let lower_name = name.to_ascii_lowercase();
    let joined_loc = location_paths.join(" ").to_ascii_uppercase();

    if vendor_id == 0x8086
      && (lower_name.contains("usb4")
        || lower_name.contains("thunderbolt")
        || lower_name.contains("host router"))
    {
      return (UsbControllerRole::CpuDirect, "USB4 / Thunderbolt".to_string());
    }

    if let Some(vendor) = match vendor_id {
      0x1B21 => Some("ASMedia"),
      0x1912 => Some("Renesas"),
      0x1106 => Some("VIA"),
      0x1B73 => Some("Fresco Logic"),
      0x104C => Some("TI"),
      _ => None,
    } {
      return (UsbControllerRole::Discrete, vendor.to_string());
    }

    if vendor_id == 0x1022 {
      let is_known_chipset_did = matches!(
        device_id,
        0x43EE | 0x43EB | 0x43D5 | 0x43D0 | 0x43BB | 0x43F7 | 0x43FC
      );
      if is_known_chipset_did || subsys_vid == Some(0x1B21) || joined_loc.contains("ACPI(PTXH)") {
        return (UsbControllerRole::Chipset, "AMD Chipset".to_string());
      }
      return (UsbControllerRole::CpuDirect, "AMD CPU xHCI".to_string());
    }

    if vendor_id == 0x8086 {
      return (UsbControllerRole::Chipset, "Intel PCH".to_string());
    }

    (UsbControllerRole::Unknown, "PCI xHCI".to_string())
  }

  fn make_controller_id(instance_id: &str, vid: u16, did: u16, idx: u32) -> String {
    let tail = instance_id
      .rsplit('\\')
      .next()
      .unwrap_or("hcd")
      .chars()
      .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
      .collect::<String>();
    if vid != 0 || did != 0 {
      format!("{:04x}_{:04x}_{}", vid, did, tail)
    } else {
      format!("hcd_{}_{}", idx, tail)
    }
  }

  pub fn analyze_diagnostics(controllers: &[UsbControllerNode]) -> Vec<UsbDiagnosticHint> {
    let mut hints = Vec::new();
    let has_free_cpu_port = controllers.iter().any(|c| {
      c.role == UsbControllerRole::CpuDirect
        && c.ports
          .iter()
          .any(|p| p.status == UsbPortStatus::Empty && p.is_user_connectable)
    });

    for ctrl in controllers {
      Self::inspect_ports_recursive(ctrl, &ctrl.ports, 0, has_free_cpu_port, &mut hints);
    }
    hints
  }

  fn inspect_ports_recursive(
    ctrl: &UsbControllerNode,
    ports: &[UsbPortNode],
    hub_depth: usize,
    has_free_cpu_port: bool,
    hints: &mut Vec<UsbDiagnosticHint>,
  ) {
    for port in ports {
      let label = port
        .device_name
        .as_deref()
        .unwrap_or("USB Device");

      if port.status == UsbPortStatus::Error || port.problem_code.is_some() {
        let code_text = port
          .problem_code
          .map(|c| format!(" (Code {})", c))
          .unwrap_or_default();
        hints.push(UsbDiagnosticHint {
          severity: UsbDiagnosticSeverity::Error,
          title_zh: format!("Port {} 设备异常{}", port.port_number, code_text),
          title_en: format!("Port {} device error{}", port.port_number, code_text),
          detail_zh: format!(
            "{}（{}）当前未正常启动，可能拖累同总线设备。若不需要建议拔除，或换到其他端口测试。",
            label, ctrl.name
          ),
          detail_en: format!(
            "{} on {} failed to start properly and may destabilize sibling ports. Unplug it or test on another port.",
            label, ctrl.name
          ),
          port_key: Some(port.port_key.clone()),
        });
      }

      if port.disconnect_count_24h >= 3 {
        let severity = if port.disconnect_count_24h >= 10 {
          UsbDiagnosticSeverity::Error
        } else {
          UsbDiagnosticSeverity::Warning
        };
        hints.push(UsbDiagnosticHint {
          severity,
          title_zh: format!(
            "{} 近 24 小时异常断连 {} 次",
            label, port.disconnect_count_24h
          ),
          title_en: format!(
            "{} disconnected unexpectedly {}× in 24h",
            label, port.disconnect_count_24h
          ),
          detail_zh: format!(
            "位于「{} · Port {}」。高频意外断开通常由接口松动、供电不足、扩展坞休眠或芯片组总线不稳定引起。",
            ctrl.name, port.port_number
          ),
          detail_en: format!(
            "Attached to {} (Port {}). Frequent surprise removals indicate loose cabling, power droop, hub sleep, or bus instability.",
            ctrl.name, port.port_number
          ),
          port_key: Some(port.port_key.clone()),
        });
      }

      let is_latency_sensitive = Self::is_gaming_or_input_device(port);
      if is_latency_sensitive && hub_depth > 0 {
        hints.push(UsbDiagnosticHint {
          severity: UsbDiagnosticSeverity::Warning,
          title_zh: format!("{} 接在二级 USB Hub 上", label),
          title_en: format!("{} is connected through an external hub", label),
          detail_zh: "高回报率键鼠或无线接收器经显示器/扩展坞中转更容易受休眠断电或信号干扰影响，建议直插主板接口。".to_string(),
          detail_en: "High-polling peripherals or 2.4G dongles routed through a monitor/external hub can suffer sleep dropouts or latency; plug directly into the motherboard.".to_string(),
          port_key: Some(port.port_key.clone()),
        });
      } else if is_latency_sensitive && hub_depth == 0 && ctrl.role == UsbControllerRole::Chipset && has_free_cpu_port {
        hints.push(UsbDiagnosticHint {
          severity: UsbDiagnosticSeverity::Info,
          title_zh: format!("{} 可改插到 CPU 直连口", label),
          title_en: format!("{} could use a free CPU-direct port", label),
          detail_zh: "检测到主板还有空闲的 CPU 直连 USB 口；将主力游戏键鼠从芯片组口移至 CPU 直连口通常更稳定。".to_string(),
          detail_en: "A free CPU-direct USB port is available. Moving primary gaming peripherals off the chipset controller often improves stability.".to_string(),
          port_key: Some(port.port_key.clone()),
        });
      }

      if !port.hub_children.is_empty() {
        Self::inspect_ports_recursive(ctrl, &port.hub_children, hub_depth + 1, has_free_cpu_port, hints);
      }
    }
  }

  fn is_gaming_or_input_device(port: &UsbPortNode) -> bool {
    if port.is_hub || port.status != UsbPortStatus::Connected {
      return false;
    }
    if let Some(vid) = port.vendor_id {
      if matches!(
        vid,
        0x0B05 | 0x1532 | 0x046D | 0x31E3 | 0x1B1C | 0x1038 | 0x2516 | 0x3434 | 0x2DC8
      ) {
        return true;
      }
    }
    if let Some(name) = &port.device_name {
      let lower = name.to_ascii_lowercase();
      return lower.contains("keyboard")
        || lower.contains("mouse")
        || lower.contains("falcata")
        || lower.contains("naga")
        || lower.contains("viper")
        || lower.contains("wooting");
    }
    false
  }

  pub fn vendor_name(vid: u16) -> Option<&'static str> {
    match vid {
      0x0B05 => Some("ASUS"),
      0x1532 => Some("Razer"),
      0x054C => Some("Sony"),
      0x046D => Some("Logitech"),
      0x045E => Some("Microsoft"),
      0x05AC => Some("Apple"),
      0x8086 => Some("Intel"),
      0x1022 => Some("AMD"),
      0x0BDA => Some("Realtek"),
      0x043E => Some("LG"),
      0x27C6 => Some("Goodix"),
      0x1462 => Some("MSI"),
      0x1B1C => Some("Corsair"),
      0x1038 => Some("SteelSeries"),
      0x31E3 => Some("Wooting"),
      0x05E3 => Some("Genesys Logic"),
      0x2109 => Some("VIA Labs"),
      0x1B21 => Some("ASMedia"),
      0x3537 => Some("GameSir"),
      0x3434 => Some("Keychron"),
      0x0951 => Some("Kingston"),
      0x0781 => Some("SanDisk"),
      0x04E8 => Some("Samsung"),
      0x2516 => Some("Cooler Master"),
      _ => None,
    }
  }

  #[cfg(target_os = "windows")]
  fn get_interface_path(
    dev_info: HDEVINFO,
    if_data: &SP_DEVICE_INTERFACE_DATA,
    dev_data: &mut SP_DEVINFO_DATA,
  ) -> Option<String> {
    let mut required: u32 = 0;
    let _ = unsafe {
      SetupDiGetDeviceInterfaceDetailW(
        dev_info,
        if_data,
        None,
        0,
        Some(&mut required),
        Some(dev_data),
      )
    };
    let min_size = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    if required < min_size {
      return None;
    }
    let mut buf = vec![0u8; required as usize];
    let detail_ptr = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    unsafe {
      (*detail_ptr).cbSize = min_size;
      SetupDiGetDeviceInterfaceDetailW(
        dev_info,
        if_data,
        Some(detail_ptr),
        required,
        None,
        Some(dev_data),
      )
      .ok()?;
    }
    let path_offset = std::mem::offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath);
    Self::read_utf16_from_bytes(&buf[path_offset..])
  }

  #[cfg(target_os = "windows")]
  fn get_device_instance_id(dev_info: HDEVINFO, dev_data: &SP_DEVINFO_DATA) -> Option<String> {
    let mut buf = [0u16; 256];
    unsafe { SetupDiGetDeviceInstanceIdW(dev_info, dev_data, Some(&mut buf), None) }.ok()?;
    let nul = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..nul]))
  }

  #[cfg(target_os = "windows")]
  fn get_devprop_bytes(
    dev_info: HDEVINFO,
    dev_data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
  ) -> Option<(DEVPROPTYPE, Vec<u8>)> {
    let mut prop_type = DEVPROPTYPE::default();
    let mut required: u32 = 0;
    let _ = unsafe {
      SetupDiGetDevicePropertyW(
        dev_info,
        dev_data,
        key,
        &mut prop_type,
        None,
        Some(&mut required),
        0,
      )
    };
    if required == 0 {
      return None;
    }
    let mut buf = vec![0u8; required as usize];
    unsafe {
      SetupDiGetDevicePropertyW(
        dev_info,
        dev_data,
        key,
        &mut prop_type,
        Some(&mut buf),
        None,
        0,
      )
    }
    .ok()?;
    Some((prop_type, buf))
  }

  #[cfg(target_os = "windows")]
  fn get_devprop_string(
    dev_info: HDEVINFO,
    dev_data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
  ) -> Option<String> {
    let (_, bytes) = Self::get_devprop_bytes(dev_info, dev_data, key)?;
    let s = Self::read_utf16_from_bytes(&bytes)?;
    if s.trim().is_empty() {
      None
    } else {
      Some(s)
    }
  }

  #[cfg(target_os = "windows")]
  fn get_devprop_string_list(
    dev_info: HDEVINFO,
    dev_data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
  ) -> Vec<String> {
    let Some((_, bytes)) = Self::get_devprop_bytes(dev_info, dev_data, key) else {
      return Vec::new();
    };
    let words: Vec<u16> = bytes
      .chunks_exact(2)
      .map(|c| u16::from_le_bytes([c[0], c[1]]))
      .collect();
    words
      .split(|&w| w == 0)
      .filter(|slice| !slice.is_empty())
      .map(String::from_utf16_lossy)
      .collect()
  }

  #[cfg(target_os = "windows")]
  fn get_devprop_u32(
    dev_info: HDEVINFO,
    dev_data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
  ) -> Option<u32> {
    let (_, bytes) = Self::get_devprop_bytes(dev_info, dev_data, key)?;
    if bytes.len() < 4 {
      return None;
    }
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
  }

  #[cfg(target_os = "windows")]
  fn read_utf16_from_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 2 {
      return None;
    }
    let words: Vec<u16> = bytes
      .chunks_exact(2)
      .map(|c| u16::from_le_bytes([c[0], c[1]]))
      .take_while(|&w| w != 0)
      .collect();
    if words.is_empty() {
      None
    } else {
      Some(String::from_utf16_lossy(&words))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_port(port_number: u32, companion: Option<u32>, max_speed: UsbSpeed) -> UsbPortNode {
    UsbPortNode {
      port_key: format!("ctrl/root/p{}", port_number),
      port_number,
      companion_port: companion,
      is_type_c: false,
      is_user_connectable: true,
      max_speed,
      current_speed: None,
      status: UsbPortStatus::Empty,
      device_name: None,
      manufacturer: None,
      vendor_id: None,
      product_id: None,
      instance_id: None,
      problem_code: None,
      problem_desc: None,
      max_power_ma: None,
      is_hub: false,
      hub_children: Vec::new(),
      disconnect_count_24h: 0,
      custom_label: None,
      recently_changed: false,
    }
  }

  #[test]
  fn classifies_amd_intel_and_discrete_controllers() {
    let (vid, did, sub) =
      UsbTopologyScanner::parse_pci_ids(r"PCI\VEN_1022&DEV_149C&SUBSYS_7C941462&REV_00\4&A935EEF");
    assert_eq!((vid, did, sub), (0x1022, 0x149C, Some(0x1462)));
    assert_eq!(
      UsbTopologyScanner::classify_controller(
        "AMD USB 3.10 eXtensible Host Controller",
        "",
        vid,
        did,
        sub,
        &["ACPI(_SB_)#ACPI(PCI0)#ACPI(GP13)#ACPI(XHC0)".to_string()],
      )
      .0,
      UsbControllerRole::CpuDirect
    );

    let (vid2, did2, sub2) =
      UsbTopologyScanner::parse_pci_ids(r"PCI\VEN_1022&DEV_43EE&SUBSYS_11421B21&REV_00\4&33351444");
    assert_eq!(
      UsbTopologyScanner::classify_controller(
        "AMD USB 3.10 eXtensible Host Controller",
        "",
        vid2,
        did2,
        sub2,
        &[],
      )
      .0,
      UsbControllerRole::Chipset
    );

    assert_eq!(
      UsbTopologyScanner::classify_controller("Intel(R) USB 3.20 eXtensible Host Controller", "", 0x8086, 0x7A60, None, &[]).0,
      UsbControllerRole::Chipset
    );
    assert_eq!(
      UsbTopologyScanner::classify_controller("USB4(TM) Host Router (Microsoft)", "", 0x8086, 0x9A13, None, &[]).0,
      UsbControllerRole::CpuDirect
    );
    assert_eq!(
      UsbTopologyScanner::classify_controller("ASMedia USB 3.20 Host Controller", "", 0x1B21, 0x2142, None, &[]).0,
      UsbControllerRole::Discrete
    );
  }

  #[test]
  fn merges_companion_ports_preserving_occupied_device_and_max_speed() {
    let mut p1_usb2 = make_port(1, Some(5), UsbSpeed::High);
    p1_usb2.status = UsbPortStatus::Connected;
    p1_usb2.device_name = Some("ROG FALCATA".to_string());
    p1_usb2.vendor_id = Some(0x0B05);
    p1_usb2.current_speed = Some(UsbSpeed::High);

    let mut p5_usb3 = make_port(5, Some(1), UsbSpeed::Super);
    p5_usb3.is_type_c = true;

    let mut hidden_internal = make_port(6, None, UsbSpeed::High);
    hidden_internal.is_user_connectable = false;

    let cfg = UsbInspectorConfig::default();
    let merged = UsbTopologyScanner::merge_companion_ports(
      vec![p1_usb2, p5_usb3, hidden_internal],
      &cfg,
      &HashSet::new(),
    );

    assert_eq!(merged.len(), 1);
    let sock = &merged[0];
    assert_eq!(sock.port_key, "ctrl/root/p1");
    assert_eq!(sock.port_number, 1);
    assert_eq!(sock.companion_port, Some(5));
    assert_eq!(sock.max_speed, UsbSpeed::Super);
    assert!(sock.is_type_c);
    assert_eq!(sock.status, UsbPortStatus::Connected);
    assert_eq!(sock.device_name.as_deref(), Some("ROG FALCATA"));
  }

  #[test]
  fn analyze_diagnostics_flags_errors_disconnects_hubs_and_chipset_gaming_devices() {
    let mut err_port = make_port(1, None, UsbSpeed::High);
    err_port.status = UsbPortStatus::Error;
    err_port.problem_code = Some(43);
    err_port.device_name = Some("Unknown USB Device".to_string());

    let mut flaky_port = make_port(2, None, UsbSpeed::High);
    flaky_port.status = UsbPortStatus::Connected;
    flaky_port.device_name = Some("Goodix fingerprint".to_string());
    flaky_port.disconnect_count_24h = 14;

    let mut hub_child = make_port(1, None, UsbSpeed::High);
    hub_child.port_key = "c_chip/root.p3/p1".to_string();
    hub_child.status = UsbPortStatus::Connected;
    hub_child.vendor_id = Some(0x1532);
    hub_child.device_name = Some("Razer Naga V3 Pro".to_string());

    let mut hub_port = make_port(3, None, UsbSpeed::Super);
    hub_port.status = UsbPortStatus::Connected;
    hub_port.is_hub = true;
    hub_port.device_name = Some("Monitor Hub".to_string());
    hub_port.hub_children = vec![hub_child];

    let mut gaming_on_chipset = make_port(4, None, UsbSpeed::High);
    gaming_on_chipset.status = UsbPortStatus::Connected;
    gaming_on_chipset.vendor_id = Some(0x0B05);
    gaming_on_chipset.device_name = Some("ROG FALCATA".to_string());

    let controllers = vec![
      UsbControllerNode {
        id: "c_cpu".to_string(),
        name: "AMD CPU USB".to_string(),
        vendor_id: 0x1022,
        device_id: 0x149C,
        role: UsbControllerRole::CpuDirect,
        role_reason: "AMD CPU xHCI".to_string(),
        ports: vec![make_port(1, None, UsbSpeed::SuperPlus)],
      },
      UsbControllerNode {
        id: "c_chip".to_string(),
        name: "AMD Chipset USB".to_string(),
        vendor_id: 0x1022,
        device_id: 0x43EE,
        role: UsbControllerRole::Chipset,
        role_reason: "AMD Chipset".to_string(),
        ports: vec![err_port, flaky_port, hub_port, gaming_on_chipset],
      },
    ];

    let hints = UsbTopologyScanner::analyze_diagnostics(&controllers);
    assert_eq!(hints.len(), 4);
    assert_eq!(hints[0].severity, UsbDiagnosticSeverity::Error);
    assert_eq!(hints[1].severity, UsbDiagnosticSeverity::Error);
    assert_eq!(hints[2].severity, UsbDiagnosticSeverity::Warning);
    assert_eq!(hints[3].severity, UsbDiagnosticSeverity::Info);
  }

  #[test]
  #[ignore = "Requires live Windows USB stack"]
  fn live_scan_usb_topology() {
    let outcome = UsbTopologyScanner::scan(&UsbInspectorConfig::default(), &HashSet::new());
    println!("=== LIVE USB SCAN ({} controllers) ===", outcome.controllers.len());
    for c in &outcome.controllers {
      println!(
        "Controller [{} | {:?} | {}] {:04X}:{:04X}",
        c.id, c.role, c.role_reason, c.vendor_id, c.device_id
      );
      for p in &c.ports {
        println!(
          "  Port {} (max={:?}, cur={:?}, typeC={}, power={:?}mA, disc24h={}): {:?} | {:?}",
          p.port_number,
          p.max_speed,
          p.current_speed,
          p.is_type_c,
          p.max_power_ma,
          p.disconnect_count_24h,
          p.status,
          p.device_name
        );
        for ch in &p.hub_children {
          println!(
            "    -> Hub Port {}: {:?} | {:?}",
            ch.port_number, ch.status, ch.device_name
          );
        }
      }
    }
    println!("Diagnostics: {:?}", outcome.diagnostics);
    println!("Recent 24h surprise removals: {}", outcome.recent_events.len());
    assert!(!outcome.controllers.is_empty());
  }
}
