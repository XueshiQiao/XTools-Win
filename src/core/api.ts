import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  GeneralSettings,
  HyperActionConfig,
  HyperCapslockSnapshot,
  HyperTrigger,
  KeyShortcut,
  LanguagePreference,
  NagaPlateLayout,
  NagaRecordMode,
  NagaSnapshot,
  RogKeyboardSnapshot,
  RogSettingsPatch,
  RunningProcessInfo,
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

  public async getHyperState(): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_get_state');
  }

  public async setHyperPaused(paused: boolean): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_set_paused', { paused });
  }

  public async setHyperAutoPauseEnabled(enabled: boolean): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_set_auto_pause_enabled', { enabled });
  }

  public async setHyperAutoPauseNotifySound(
    enabled: boolean
  ): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_set_auto_pause_notify_sound', { enabled });
  }

  public async addHyperAutoPauseProcess(name: string): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_add_auto_pause_process', { name });
  }

  public async removeHyperAutoPauseProcess(name: string): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_remove_auto_pause_process', { name });
  }

  public async listRunningProcesses(): Promise<RunningProcessInfo[]> {
    return invoke<RunningProcessInfo[]>('hyper_list_running_processes');
  }

  public async upsertHyperMapping(
    trigger: HyperTrigger,
    action: HyperActionConfig
  ): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_upsert_mapping', { trigger, action });
  }

  public async removeHyperMapping(trigger: HyperTrigger): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_remove_mapping', { trigger });
  }

  public async resetHyperDefaults(): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_reset_defaults');
  }

  public async importHyperMappings(mappingsJson: string): Promise<HyperCapslockSnapshot> {
    return invoke<HyperCapslockSnapshot>('hyper_import_mappings', { mappingsJson });
  }

  public async exportHyperMappings(): Promise<string> {
    return invoke<string>('hyper_export_mappings');
  }

  public async onHyperStateChanged(
    handler: (snapshot: HyperCapslockSnapshot) => void
  ): Promise<UnlistenFn> {
    return listen<HyperCapslockSnapshot>('hyper-capslock://state-changed', (event) => {
      handler(event.payload);
    });
  }

  public async getNagaState(): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_get_state');
  }

  public async refreshNaga(): Promise<void> {
    await invoke('naga_refresh');
  }

  public async setNagaEnabled(enabled: boolean): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_set_enabled', { enabled });
  }

  public async setNagaPlateLayout(layout: NagaPlateLayout): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_set_plate_layout', { layout });
  }

  public async setNagaButtonShortcut(
    buttonIndex: number,
    shortcut: KeyShortcut | null
  ): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_set_button_shortcut', { buttonIndex, shortcut });
  }

  public async setNagaButtonSentinel(
    buttonIndex: number,
    sentinelVk: number
  ): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_set_button_sentinel', { buttonIndex, sentinelVk });
  }

  public async setNagaButtonEnabled(
    buttonIndex: number,
    enabled: boolean
  ): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_set_button_enabled', { buttonIndex, enabled });
  }

  public async beginNagaRecording(
    buttonIndex: number,
    mode: NagaRecordMode
  ): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_begin_recording', { buttonIndex, mode });
  }

  public async cancelNagaRecording(): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_cancel_recording');
  }

  public async clearNagaCaptures(): Promise<NagaSnapshot> {
    return invoke<NagaSnapshot>('naga_clear_captures');
  }

  public async onNagaStateChanged(
    handler: (snapshot: NagaSnapshot) => void
  ): Promise<UnlistenFn> {
    return listen<NagaSnapshot>('razer-naga://state-changed', (event) => {
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
