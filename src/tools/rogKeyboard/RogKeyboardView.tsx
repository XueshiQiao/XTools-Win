import {
  ArrowUpRight,
  Battery,
  BatteryCharging,
  BatteryFull,
  BatteryLow,
  Bell,
  Globe,
  Keyboard,
  Monitor,
  RotateCcw,
} from 'lucide-react';
import {
  ActionButton,
  Banner,
  FeatureLabel,
  FormRow,
  FormSection,
  IconLabel,
  SelectControl,
  StatusDot,
  ToggleSwitch,
} from '../../components/AppChrome';
import { xtoolsApi } from '../../core/api';
import { ToolRenderProps } from '../../core/toolRegistry';
import { RogBatteryInfo } from '../../core/types';

const GEAR_LINK_URL = 'https://gearlink.asus.com/zh-cn';

function renderBatteryIcon(battery: RogBatteryInfo) {
  if (battery.charging_state === 'charging') {
    return <BatteryCharging size={15} className="text-[#30D158]" />;
  }
  if (battery.charging_state === 'full') {
    return <BatteryFull size={15} className="text-[#30D158]" />;
  }
  if (battery.percent <= 20) {
    return <BatteryLow size={15} className="text-[#FF9F0A]" />;
  }
  return <Battery size={15} className="text-white/[0.8]" />;
}

export function RogKeyboardView({
  i18n,
  rogState,
  onUpdateRogState,
}: ToolRenderProps) {
  const slots = i18n.orderedProfileSlots(rogState.profile_count);
  const profileOptions = slots.map((slot) => ({
    value: slot,
    label: i18n.profileLabel(slot, rogState.profile_count),
  }));

  const updateSettings = async (
    patch: Parameters<typeof xtoolsApi.updateRogSettings>[0]
  ) => {
    const next = await xtoolsApi.updateRogSettings(patch);
    onUpdateRogState(next);
  };

  const battery = rogState.battery;
  const batteryStateLabel = (() => {
    if (!battery) return '';
    if (battery.charging_state === 'charging') return i18n.t('rog.battery.charging');
    if (battery.charging_state === 'full') return i18n.t('rog.battery.full');
    return '';
  })();

  const batterySplitLabel =
    battery && battery.left_percent !== null && battery.right_percent !== null
      ? i18n.t('rog.battery.split', {
          left: battery.left_percent,
          right: battery.right_percent,
        })
      : '';

  return (
    <div className="mx-auto max-w-[680px] space-y-5 px-6 py-5">
      {rogState.error_message ? (
        <Banner kind="warning" message={rogState.error_message} />
      ) : rogState.status_message ? (
        <Banner kind="success" message={rogState.status_message} />
      ) : null}

      {/* Status */}
      <FormSection header={i18n.t('rog.section.status')}>
        <FormRow
          leading={
            <IconLabel
              color="#5856D6"
              icon={<Keyboard size={14} strokeWidth={2.1} />}
              title={rogState.model_name}
            />
          }
          trailing={
            <div className="flex items-center gap-1.5">
              <StatusDot active={rogState.is_connected} />
              <span
                className="text-[11.5px] font-medium"
                style={{
                  color: rogState.is_connected ? '#30D158' : '#FF9F0A',
                }}
              >
                {i18n.t(
                  rogState.is_connected
                    ? 'rog.status.connected'
                    : 'rog.status.disconnected'
                )}
              </span>
            </div>
          }
        />

        {rogState.is_connected && rogState.current_profile !== null ? (
          <>
            <FormRow
              leading={
                <span className="text-[13px] text-white/[0.85]">
                  {i18n.t('rog.status.currentProfile')}
                </span>
              }
              trailing={
                <span className="text-[12px] font-semibold text-white/[0.95]">
                  {i18n.profileLabel(
                    rogState.current_profile,
                    rogState.profile_count
                  )}
                </span>
              }
            />
            {battery && (
              <FormRow
                leading={
                  <span className="text-[13px] text-white/[0.85]">
                    {i18n.t('rog.status.battery')}
                  </span>
                }
                trailing={
                  <div className="flex items-center gap-2">
                    {renderBatteryIcon(battery)}
                    <span className="text-[12px] font-semibold text-white/[0.95]">
                      {battery.percent}%
                    </span>
                    {batteryStateLabel && (
                      <span className="rounded bg-[#30D158]/15 px-1.5 py-0.5 text-[10.5px] font-medium text-[#30D158]">
                        {batteryStateLabel}
                      </span>
                    )}
                    {batterySplitLabel && (
                      <span className="text-[11.5px] text-white/[0.52]">
                        {batterySplitLabel}
                      </span>
                    )}
                  </div>
                }
              />
            )}
            <FormRow
              leading={
                <span className="text-[13px] text-white/[0.85]">
                  {i18n.t('rog.status.link')}
                </span>
              }
              trailing={
                <span className="text-[12px] text-white/[0.8]">
                  {i18n.linkLabel(rogState.link)}
                </span>
              }
            />
            <FormRow
              leading={
                <span className="text-[13px] text-white/[0.85]">
                  {i18n.t('rog.status.firmware')}
                </span>
              }
              trailing={
                <span className="font-mono text-[12px] text-white/[0.8]">
                  {rogState.firmware_version || '—'}
                </span>
              }
            />
          </>
        ) : (
          <div className="px-3.5 py-2.5 text-[12px] leading-relaxed text-white/[0.52]">
            {i18n.t('rog.status.disconnected.hint')}
          </div>
        )}
      </FormSection>

      {/* Profiles */}
      <FormSection
        header={i18n.t('rog.section.profiles')}
        footer={i18n.t('rog.section.profiles.footer')}
      >
        <FormRow
          leading={
            <IconLabel
              color="#30B0C7"
              icon={<Monitor size={14} strokeWidth={2.1} />}
              title={i18n.t('rog.profile.windows')}
            />
          }
          trailing={
            <SelectControl
              value={rogState.windows_profile}
              options={profileOptions}
              onChange={(slot) => void updateSettings({ windows_profile: slot })}
            />
          }
        />

        <div className="flex items-center justify-end gap-2 px-3.5 py-2.5">
          <ActionButton
            label={i18n.t('rog.action.switchToWindows')}
            disabled={rogState.is_busy || !rogState.is_connected}
            onClick={() => void xtoolsApi.switchRogToWindows()}
          />
        </div>
      </FormSection>

      {/* Gear Link Web Driver */}
      <FormSection>
        <button
          type="button"
          onClick={() => void xtoolsApi.openExternal(GEAR_LINK_URL)}
          className="block w-full cursor-pointer text-left transition-colors hover:bg-white/[0.04]"
        >
          <FormRow
            leading={
              <FeatureLabel
                color="#FF6B35"
                icon={<Globe size={14} strokeWidth={2.1} />}
                title={i18n.t('rog.gearlink.title')}
                subtitle={i18n.t('rog.gearlink.subtitle')}
              />
            }
            trailing={<ArrowUpRight size={15} className="text-white/[0.45]" />}
          />
        </button>
      </FormSection>

      {/* Automation */}
      <FormSection
        header={i18n.t('rog.section.automation')}
        footer={i18n.t('rog.section.automation.footer')}
      >
        <FormRow
          leading={
            <FeatureLabel
              color="#30D158"
              icon={<RotateCcw size={14} strokeWidth={2.2} />}
              title={i18n.t('rog.auto.title')}
              subtitle={i18n.t('rog.auto.subtitle')}
            />
          }
          trailing={
            <ToggleSwitch
              checked={rogState.auto_switch_enabled}
              onChange={(auto_switch_enabled) =>
                void updateSettings({ auto_switch_enabled })
              }
            />
          }
        />

        <FormRow
          leading={
            <IconLabel
              color="#FF2D55"
              icon={<Bell size={14} strokeWidth={2.1} />}
              title={i18n.t('rog.sound.title')}
            />
          }
          trailing={
            <ToggleSwitch
              checked={rogState.sound_enabled}
              onChange={(sound_enabled) =>
                void updateSettings({ sound_enabled })
              }
            />
          }
        />

        {!rogState.notifications_authorized && (
          <div className="px-3.5 py-2.5 text-[12px] leading-relaxed text-[#FF9F0A]">
            {i18n.t('rog.notify.denied')}
          </div>
        )}
      </FormSection>
    </div>
  );
}
