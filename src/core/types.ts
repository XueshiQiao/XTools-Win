export type RogLink = 'wired' | 'wireless24g' | 'bluetooth';
export type RogChargingState = 'discharging' | 'charging' | 'full' | 'unknown';

export interface RogBatteryInfo {
  percent: number;
  charging_state: RogChargingState;
  left_percent: number | null;
  left_mv: number | null;
  right_percent: number | null;
  right_mv: number | null;
}

export interface RogKeyboardSnapshot {
  is_connected: boolean;
  model_name: string;
  link: RogLink | null;
  firmware_version: string | null;
  profile_count: number;
  current_profile: number | null;
  battery: RogBatteryInfo | null;
  is_busy: boolean;
  status_message: string | null;
  error_message: string | null;
  notifications_authorized: boolean;
  windows_profile: number;
  auto_switch_enabled: boolean;
  sound_enabled: boolean;
}

export interface RogSettingsPatch {
  windows_profile?: number;
  auto_switch_enabled?: boolean;
  sound_enabled?: boolean;
}

export interface KeyShortcut {
  vk: number;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  win: boolean;
}

export type DirectionalActionKind =
  | 'left'
  | 'right'
  | 'up'
  | 'down'
  | 'word_forward'
  | 'word_back'
  | 'home'
  | 'end';

export type JumpDirection = 'up' | 'down';

export type IndependentActionKind =
  | 'backspace'
  | 'next_line'
  | 'insert_quotes'
  | 'toggle_caps_lock'
  | 'switch_input_source';

export type HyperActionConfig =
  | { kind: 'directional'; action: DirectionalActionKind }
  | { kind: 'jump'; direction: JumpDirection; count: number }
  | { kind: 'independent'; action: IndependentActionKind }
  | { kind: 'command'; command: string }
  | {
      kind: 'key_combo';
      target_key: number;
      with_ctrl: boolean;
      with_alt: boolean;
      with_win: boolean;
      with_shift: boolean;
    };

export type HyperModifierKey =
  | 'left_shift'
  | 'right_shift'
  | 'left_control'
  | 'right_control'
  | 'left_alt'
  | 'right_alt'
  | 'left_win'
  | 'right_win';

export type HyperTrigger =
  | { kind: 'hyper_plus_key'; key: number; with_shift: boolean }
  | { kind: 'single_tap_hyper' }
  | { kind: 'double_tap_hyper' }
  | { kind: 'double_tap_modifier'; modifier: HyperModifierKey };

export interface HyperActionMappingEntry {
  trigger: HyperTrigger;
  action: HyperActionConfig;
}

export interface RunningProcessInfo {
  name: string;
  icon_data_url: string | null;
  has_window: boolean;
}

export interface HyperCapslockSnapshot {
  paused: boolean;
  manual_paused: boolean;
  auto_paused: boolean;
  matched_processes: string[];
  auto_pause_enabled: boolean;
  auto_pause_processes: string[];
  auto_pause_notify_sound: boolean;
  mappings: HyperActionMappingEntry[];
}

export type NagaLink = 'wired' | 'wireless24g';
export type NagaChargingState = 'discharging' | 'charging' | 'full' | 'unknown';
export type NagaPlateLayout = 'two_button' | 'six_button' | 'twelve_button';
export type NagaMouseButton =
  | 'left'
  | 'right'
  | 'middle'
  | 'back'
  | 'forward'
  | 'scroll_up'
  | 'scroll_down';
export type NagaDpiOp = 'up' | 'down' | 'cycle_up' | 'cycle_down';

export interface NagaBatteryInfo {
  percent: number;
  charging_state: NagaChargingState;
}

export interface NagaDpi {
  x: number;
  y: number;
}

export interface NagaDpiStages {
  active_stage: number;
  stages: NagaDpi[];
}

export interface NagaProfileInfo {
  max_profiles: number;
  active_profile: number;
  available_profiles: number[];
}

export type NagaHardwareAction =
  | { kind: 'disabled' }
  | { kind: 'mouse'; button: NagaMouseButton }
  | {
      kind: 'keyboard';
      vk: number;
      hid_code: number;
      ctrl: boolean;
      shift: boolean;
      alt: boolean;
      win: boolean;
    }
  | { kind: 'dpi'; op: NagaDpiOp }
  | { kind: 'profile'; op: number }
  | { kind: 'media'; usage: number }
  | { kind: 'hypershift' }
  | { kind: 'raw'; action_type: number; data: number[] };

export interface NagaHardwareButton {
  index: number;
  button_id: number;
  action: NagaHardwareAction;
}

export interface NagaSnapshot {
  is_connected: boolean;
  model_name: string;
  link: NagaLink | null;
  firmware_version: string | null;
  battery: NagaBatteryInfo | null;
  dpi: NagaDpi | null;
  dpi_stages: NagaDpiStages | null;
  polling_rate_hz: number | null;
  profile_info: NagaProfileInfo | null;
  plate_layout: NagaPlateLayout;
  buttons: NagaHardwareButton[];
  is_busy: boolean;
  status_message: string | null;
  error_message: string | null;
}

export type EffectiveLanguage = 'zh-Hans' | 'en';
export type LanguagePreference = EffectiveLanguage | null;

export interface GeneralSettings {
  launch_at_login: boolean;
  language: LanguagePreference;
  effective_language: EffectiveLanguage;
  version: string;
  log_path: string;
}

export type SidebarSelection =
  | { kind: 'tool'; id: string }
  | { kind: 'general' }
  | { kind: 'about' };
