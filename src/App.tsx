import { useEffect, useMemo, useState } from 'react';
import { Sidebar } from './components/Sidebar';
import { TitleBar } from './components/TitleBar';
import { xtoolsApi } from './core/api';
import { I18nManager } from './core/i18n';
import { ToolRegistry } from './core/toolRegistry';
import {
  GeneralSettings,
  HyperCapslockSnapshot,
  NagaSnapshot,
  RogKeyboardSnapshot,
  SidebarSelection,
} from './core/types';
import { AboutPage } from './pages/AboutPage';
import { GeneralPage } from './pages/GeneralPage';
import { hyperCapslockTool } from './tools/hyperCapslock/hyperCapslockTool';
import { razerNagaTool } from './tools/razerNaga/razerNagaTool';
import { rogKeyboardTool } from './tools/rogKeyboard/rogKeyboardTool';

const DEFAULT_ROG_STATE: RogKeyboardSnapshot = {
  is_connected: false,
  model_name: 'ROG Falcata',
  link: null,
  firmware_version: null,
  profile_count: 6,
  current_profile: null,
  battery: null,
  is_busy: false,
  status_message: null,
  error_message: null,
  notifications_authorized: true,
  windows_profile: 1,
  auto_switch_enabled: true,
  sound_enabled: true,
};

const DEFAULT_NAGA_STATE: NagaSnapshot = {
  is_connected: false,
  model_name: 'Razer Naga V3 Pro',
  link: null,
  firmware_version: null,
  battery: null,
  dpi: null,
  dpi_stages: null,
  polling_rate_hz: null,
  profile_info: null,
  plate_layout: 'six_button',
  buttons: [],
  is_busy: false,
  status_message: null,
  error_message: null,
};

const DEFAULT_HYPER_STATE: HyperCapslockSnapshot = {
  paused: false,
  manual_paused: false,
  auto_paused: false,
  matched_processes: [],
  auto_pause_enabled: true,
  auto_pause_processes: [],
  auto_pause_notify_sound: true,
  mappings: [],
};

const DEFAULT_GENERAL: GeneralSettings = {
  launch_at_login: false,
  language: null,
  effective_language: 'zh-Hans',
  version: '0.1.0',
  log_path: '',
};

export function App() {
  const registry = useMemo(
    () => new ToolRegistry([rogKeyboardTool, razerNagaTool, hyperCapslockTool]),
    []
  );
  const [selection, setSelection] = useState<SidebarSelection>({
    kind: 'tool',
    id: rogKeyboardTool.id,
  });
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [rogState, setRogState] = useState<RogKeyboardSnapshot>(DEFAULT_ROG_STATE);
  const [nagaState, setNagaState] = useState<NagaSnapshot>(DEFAULT_NAGA_STATE);
  const [hyperState, setHyperState] = useState<HyperCapslockSnapshot>(DEFAULT_HYPER_STATE);
  const [generalSettings, setGeneralSettings] =
    useState<GeneralSettings>(DEFAULT_GENERAL);

  const i18n = useMemo(
    () => new I18nManager(generalSettings.effective_language),
    [generalSettings.effective_language]
  );

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];

    const bootstrap = async () => {
      try {
        const [initialRog, initialNaga, initialHyper, initialGeneral] = await Promise.all([
          xtoolsApi.getRogState(),
          xtoolsApi.getNagaState(),
          xtoolsApi.getHyperState(),
          xtoolsApi.getGeneralSettings(),
        ]);
        if (!active) return;
        setRogState(initialRog);
        setNagaState(initialNaga);
        setHyperState(initialHyper);
        setGeneralSettings(initialGeneral);

        const [offRog, offNaga, offHyper] = await Promise.all([
          xtoolsApi.onRogStateChanged((next) => {
            if (active) setRogState(next);
          }),
          xtoolsApi.onNagaStateChanged((next) => {
            if (active) setNagaState(next);
          }),
          xtoolsApi.onHyperStateChanged((next) => {
            if (active) setHyperState(next);
          }),
        ]);
        if (active) {
          unlisteners.push(offRog, offNaga, offHyper);
        } else {
          offRog();
          offNaga();
          offHyper();
        }
      } catch (err) {
        console.error('Failed to bootstrap XTools state:', err);
      }
    };

    void bootstrap();
    return () => {
      active = false;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, []);

  const currentTool =
    selection.kind === 'tool' ? registry.getById(selection.id) : undefined;

  const pageTitle = (() => {
    if (selection.kind === 'general') return i18n.t('general.title');
    if (selection.kind === 'about') return i18n.t('about.title');
    return currentTool ? i18n.t(currentTool.titleKey) : 'XTools';
  })();

  const isRogPage = selection.kind === 'tool' && selection.id === 'rog-keyboard';
  const isNagaPage = selection.kind === 'tool' && selection.id === 'razer-naga';
  const showRefresh = isRogPage || isNagaPage;
  const isBusy = isRogPage ? rogState.is_busy : isNagaPage ? nagaState.is_busy : false;

  const handleRefresh = () => {
    if (isRogPage) {
      void xtoolsApi.refreshRog();
    } else if (isNagaPage) {
      void xtoolsApi.refreshNaga();
    }
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#18181d] text-white">
      {sidebarOpen && (
        <Sidebar
          i18n={i18n}
          version={generalSettings.version}
          registry={registry}
          selection={selection}
          onSelect={setSelection}
        />
      )}

      <main className="aurora-bg flex min-w-0 flex-1 flex-col overflow-hidden">
        <TitleBar
          i18n={i18n}
          title={pageTitle}
          showRefresh={showRefresh}
          isBusy={isBusy}
          onRefresh={handleRefresh}
          onToggleSidebar={() => setSidebarOpen((open) => !open)}
        />

        <div className="flex-1 overflow-y-auto">
          {selection.kind === 'tool' && currentTool
            ? currentTool.renderView({
                i18n,
                rogState,
                onUpdateRogState: setRogState,
                nagaState,
                onUpdateNagaState: setNagaState,
                hyperState,
                onUpdateHyperState: setHyperState,
              })
            : null}

          {selection.kind === 'general' && (
            <GeneralPage
              i18n={i18n}
              settings={generalSettings}
              onUpdateSettings={setGeneralSettings}
            />
          )}

          {selection.kind === 'about' && (
            <AboutPage i18n={i18n} version={generalSettings.version} />
          )}
        </div>
      </main>
    </div>
  );
}

export default App;
