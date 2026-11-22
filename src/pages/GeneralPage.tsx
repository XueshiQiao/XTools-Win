import { FileText, Globe, Power } from 'lucide-react';
import {
  ActionButton,
  FormRow,
  FormSection,
  IconLabel,
  SelectControl,
  ToggleSwitch,
} from '../components/AppChrome';
import { xtoolsApi } from '../core/api';
import { I18nManager } from '../core/i18n';
import { GeneralSettings, LanguagePreference } from '../core/types';

interface GeneralPageProps {
  i18n: I18nManager;
  settings: GeneralSettings;
  onUpdateSettings: (next: GeneralSettings) => void;
}

const SYSTEM_TAG = '__system__';

export function GeneralPage({
  i18n,
  settings,
  onUpdateSettings,
}: GeneralPageProps) {
  const handleAutostart = async (enabled: boolean) => {
    const updated = await xtoolsApi.setLaunchAtLogin(enabled);
    onUpdateSettings(updated);
  };

  const handleLanguage = async (tag: string) => {
    const lang: LanguagePreference =
      tag === SYSTEM_TAG ? null : (tag as 'zh-Hans' | 'en');
    const updated = await xtoolsApi.setLanguage(lang);
    onUpdateSettings(updated);
  };

  return (
    <div className="mx-auto max-w-[680px] space-y-5 px-6 py-5">
      <FormSection>
        <FormRow
          leading={
            <IconLabel
              color="#30D158"
              icon={<Power size={14} strokeWidth={2.2} />}
              title={i18n.t('general.launchAtLogin')}
            />
          }
          trailing={
            <ToggleSwitch
              checked={settings.launch_at_login}
              onChange={(next) => void handleAutostart(next)}
            />
          }
        />

        <FormRow
          leading={
            <IconLabel
              color="#32ADE6"
              icon={<Globe size={14} strokeWidth={2.1} />}
              title={i18n.t('general.language')}
            />
          }
          trailing={
            <SelectControl
              value={settings.language ?? SYSTEM_TAG}
              options={[
                { value: SYSTEM_TAG, label: i18n.t('general.language.followSystem') },
                { value: 'zh-Hans', label: i18n.t('general.language.zhHans') },
                { value: 'en', label: i18n.t('general.language.en') },
              ]}
              onChange={(val) => void handleLanguage(val)}
            />
          }
        />
      </FormSection>

      <FormSection
        header={i18n.t('general.diagnostics')}
        footer={i18n.t('general.logSubtitle')}
      >
        <FormRow
          leading={
            <IconLabel
              color="#8E8E93"
              icon={<FileText size={14} strokeWidth={2.1} />}
              title={i18n.t('general.logTitle')}
            />
          }
          trailing={
            <ActionButton
              label={i18n.t('general.revealLog')}
              onClick={() => void xtoolsApi.revealLogFile()}
            />
          }
        />
      </FormSection>
    </div>
  );
}
