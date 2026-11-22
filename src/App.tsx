import { useEffect, useMemo, useState } from 'react';
import { Sidebar } from './components/Sidebar';
import { TitleBar } from './components/TitleBar';
import { xtoolsApi } from './core/api';
import { I18nManager } from './core/i18n';
import { ToolRegistry } from './core/toolRegistry';
import {
  GeneralSettings,
  RogKeyboardSnapshot,
  SidebarSelection,
} from './core/types';
import { AboutPage } from './pages/AboutPage';
import { GeneralPage } from './pages/GeneralPage';
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
  mac_profile: 6,
  auto_switch_enabled: true,
  sound_enabled: true,
};

const DEFAULT_GENERAL: GeneralSettings = {
  launch_at_login: false,
  language: null,
  effective_language: 'zh-Hans',
  version: '0.1.0',
  log_path: '',
};

export function App() {
  const registry = useMemo(() => new ToolRegistry([rogKeyboardTool]), []);
  const [selection, setSelection] = useState<SidebarSelection>({
    kind: 'tool',
    id: rogKeyboardTool.id,
  });
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [rogState, setRogState] = useState<RogKeyboardSnapshot>(DEFAULT_ROG_STATE);
  const [generalSettings, setGeneralSettings] =
    useState<GeneralSettings>(DEFAULT_GENERAL);

  const i18n = useMemo(
    () => new I18nManager(generalSettings.effective_language),
    [generalSettings.effective_language]
  );

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;

    const bootstrap = async () => {
      try {
        const [initialRog, initialGeneral] = await Promise.all([
          xtoolsApi.getRogState(),
          xtoolsApi.getGeneralSettings(),
        ]);
        if (!active) return;
        setRogState(initialRog);
        setGeneralSettings(initialGeneral);

        unlisten = await xtoolsApi.onRogStateChanged((next) => {
          if (active) setRogState(next);
        });
      } catch (err) {
        console.error('Failed to bootstrap XTools state:', err);
      }
    };

    void bootstrap();
    return () => {
      active = false;
      if (unlisten) unlisten();
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
          showRefresh={isRogPage}
          isBusy={rogState.is_busy}
          onRefresh={() => void xtoolsApi.refreshRog()}
          onToggleSidebar={() => setSidebarOpen((open) => !open)}
        />

        <div className="flex-1 overflow-y-auto">
          {selection.kind === 'tool' && currentTool
            ? currentTool.renderView({
                i18n,
                rogState,
                onUpdateRogState: setRogState,
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
