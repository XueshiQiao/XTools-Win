import { ArrowUpRight, Globe } from 'lucide-react';
import githubLogo from '../assets/github.svg';
import appLogo from '../assets/logo.png';
import xLogo from '../assets/x.svg';
import {
  AssetIconTile,
  FormRow,
  FormSection,
  IconLabel,
} from '../components/AppChrome';
import { xtoolsApi } from '../core/api';
import { I18nManager } from '../core/i18n';

interface AboutPageProps {
  i18n: I18nManager;
  version: string;
}

const BRAND_TINT = '#292b33';

export function AboutPage({ i18n, version }: AboutPageProps) {
  const links = [
    {
      key: 'github',
      icon: (
        <AssetIconTile
          src={githubLogo}
          alt="GitHub"
          color={BRAND_TINT}
        />
      ),
      title: i18n.t('about.github'),
      url: 'https://github.com/XueshiQiao/XTools',
    },
    {
      key: 'x',
      icon: <AssetIconTile src={xLogo} alt="X" color={BRAND_TINT} />,
      title: '@XueshiQiao',
      url: 'https://x.com/XueshiQiao',
    },
    {
      key: 'web',
      icon: (
        <IconLabel
          color="#0A84FF"
          icon={<Globe size={14} strokeWidth={2.1} />}
          title={`${i18n.t('about.websiteDesc')} xueshi.dev`}
        />
      ),
      title: '',
      url: 'https://xueshi.dev',
    },
  ];

  return (
    <div className="mx-auto max-w-[680px] space-y-5 px-6 py-5">
      <FormSection>
        <div className="flex flex-col items-center justify-center py-6 text-center">
          <img
            src={appLogo}
            alt="XTools"
            className="mb-3 h-[84px] w-[84px] rounded-[19px] object-cover shadow-lg"
          />
          <div className="text-[20px] font-bold tracking-tight text-white">
            XTools
          </div>
          <div className="mt-1 text-[13px] text-white/[0.55]">
            {i18n.t('about.versionFormat', { version: version || '0.1.0' })}
          </div>
        </div>
      </FormSection>

      <FormSection>
        {links.map((item) => (
          <button
            key={item.key}
            type="button"
            onClick={() => void xtoolsApi.openExternal(item.url)}
            className="block w-full cursor-pointer text-left transition-colors hover:bg-white/[0.04]"
          >
            <FormRow
              leading={
                item.title ? (
                  <div className="flex items-center gap-2.5">
                    {item.icon}
                    <span className="text-[13px] font-medium text-white/[0.92]">
                      {item.title}
                    </span>
                  </div>
                ) : (
                  item.icon
                )
              }
              trailing={
                <ArrowUpRight size={14} className="text-white/[0.4]" />
              }
            />
          </button>
        ))}
      </FormSection>

      <FormSection>
        <div className="px-4 py-3 text-center text-[11.5px] text-white/[0.48]">
          {i18n.t('about.copyright')}
        </div>
      </FormSection>
    </div>
  );
}
