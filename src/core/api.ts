import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  GeneralSettings,
  LanguagePreference,
  RogKeyboardSnapshot,
  RogSettingsPatch,
} from './types';

export class XToolsApi {
  public async getRogState(): Promise<RogKeyboardSnapshot> {
    return invoke<RogKeyboardSnapshot>('rog_get_state');
  }

  public async refreshRog(): Promise<void> {
    await invoke('rog_refresh');
  }

  public async switchRogToWindows(): Promise<void> {
    await invoke('rog_switch_to_windows');
  }

  public async switchRogToMac(): Promise<void> {
    await invoke('rog_switch_to_mac');
  }

  public async updateRogSettings(patch: RogSettingsPatch): Promise<RogKeyboardSnapshot> {
    return invoke<RogKeyboardSnapshot>('rog_update_settings', { patch });
  }

  public async onRogStateChanged(
    handler: (snapshot: RogKeyboardSnapshot) => void
  ): Promise<UnlistenFn> {
    return listen<RogKeyboardSnapshot>('rog-keyboard://state-changed', (event) => {
      handler(event.payload);
    });
  }

  public async getGeneralSettings(): Promise<GeneralSettings> {
    return invoke<GeneralSettings>('app_get_general_settings');
  }

  public async setLaunchAtLogin(enabled: boolean): Promise<GeneralSettings> {
    return invoke<GeneralSettings>('app_set_launch_at_login', { enabled });
  }

  public async setLanguage(language: LanguagePreference): Promise<GeneralSettings> {
    return invoke<GeneralSettings>('app_set_language', { language });
  }

  public async revealLogFile(): Promise<void> {
    await invoke('app_reveal_log_file');
  }

  public async openExternal(url: string): Promise<void> {
    await openUrl(url);
  }

  public async minimizeWindow(): Promise<void> {
    await getCurrentWindow().minimize();
  }

  public async toggleMaximizeWindow(): Promise<void> {
    await getCurrentWindow().toggleMaximize();
  }

  public async hideToTray(): Promise<void> {
    await getCurrentWindow().hide();
  }
}

export const xtoolsApi = new XToolsApi();
