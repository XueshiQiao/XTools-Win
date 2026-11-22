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
  mac_profile: number;
  auto_switch_enabled: boolean;
  sound_enabled: boolean;
}

export interface RogSettingsPatch {
  windows_profile?: number;
  mac_profile?: number;
  auto_switch_enabled?: boolean;
  sound_enabled?: boolean;
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
