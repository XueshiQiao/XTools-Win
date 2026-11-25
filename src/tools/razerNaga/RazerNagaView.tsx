import {
  Activity,
  ArrowRight,
  Battery,
  BatteryCharging,
  BatteryFull,
  BatteryLow,
  Mouse,
  Sliders,
  X,
  Zap,
} from 'lucide-react';
import {
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
import { NagaBatteryInfo, NagaPlateLayout } from '../../core/types';

function renderBatteryIcon(battery: NagaBatteryInfo) {
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

function formatCaptureTime(tsMs: number): string {
  if (!tsMs) return '';
  const d = new Date(tsMs);
  return d.toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

export function RazerNagaView({
  i18n,
  nagaState,
  onUpdateNagaState,
}: ToolRenderProps) {
  const battery = nagaState.battery;
  const batteryStateLabel = (() => {
    if (!battery) return '';
    if (battery.charging_state === 'charging') return i18n.t('naga.battery.charging');
    if (battery.charging_state === 'full') return i18n.t('naga.battery.full');
    return '';
  })();

  const layoutOptions: Array<{ value: NagaPlateLayout; label: string }> = [
    { value: 'two_button', label: i18n.t('naga.plate.two_button') },
    { value: 'six_button', label: i18n.t('naga.plate.six_button') },
    { value: 'twelve_button', label: i18n.t('naga.plate.twelve_button') },
  ];

  return (
    <div className="mx-auto max-w-[680px] space-y-5 px-6 py-5">
      {nagaState.error_message && (
        <Banner kind="warning" message={nagaState.error_message} />
      )}

      {/* Mouse Status */}
      <FormSection header={i18n.t('naga.section.status')}>
        <FormRow
          leading={
            <IconLabel
              color="#44D62C"
              icon={<Mouse size={14} strokeWidth={2.1} />}
              title={nagaState.model_name}
            />
          }
          trailing={
            <div className="flex items-center gap-1.5">
              <StatusDot active={nagaState.is_connected} />
              <span
                className="text-[11.5px] font-medium"
                style={{
                  color: nagaState.is_connected ? '#30D158' : '#FF9F0A',
                }}
              >
                {i18n.t(
                  nagaState.is_connected
                    ? 'naga.status.connected'
                    : 'naga.status.disconnected'
                )}
              </span>
            </div>
          }
        />

        {nagaState.is_connected ? (
          <>
            {battery && (
              <FormRow
                leading={
                  <span className="text-[13px] text-white/[0.85]">
                    {i18n.t('naga.status.battery')}
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
                  </div>
                }
              />
            )}

            <FormRow
              leading={
                <span className="text-[13px] text-white/[0.85]">
                  {i18n.t('naga.status.link')}
                </span>
              }
              trailing={
                <span className="text-[12px] text-white/[0.8]">
                  {i18n.nagaLinkLabel(nagaState.link)}
                </span>
              }
            />

            {nagaState.dpi && (
              <FormRow
                leading={
                  <span className="text-[13px] text-white/[0.85]">
                    {i18n.t('naga.status.dpi')}
                  </span>
                }
                trailing={
                  <span className="font-mono text-[12px] text-white/[0.9]">
                    {nagaState.dpi.x === nagaState.dpi.y
                      ? `${nagaState.dpi.x}`
                      : `${nagaState.dpi.x} × ${nagaState.dpi.y}`}{' '}
                    DPI
                  </span>
                }
              />
            )}

            <FormRow
              leading={
                <span className="text-[13px] text-white/[0.85]">
                  {i18n.t('naga.status.firmware')}
                </span>
              }
              trailing={
                <span className="font-mono text-[12px] text-white/[0.8]">
                  {nagaState.firmware_version || '—'}
                </span>
              }
            />
          </>
        ) : (
          <div className="px-3.5 py-2.5 text-[12px] leading-relaxed text-white/[0.52]">
            {i18n.t('naga.status.disconnected.hint')}
          </div>
        )}
      </FormSection>

      {/* Side-Button Mappings */}
      <FormSection
        header={i18n.t('naga.section.mappings')}
        footer={i18n.t('naga.mappings.footer')}
      >
        <FormRow
          leading={
            <FeatureLabel
              color="#30D158"
              icon={<Zap size={14} strokeWidth={2.1} />}
              title={i18n.t('naga.enabled.title')}
              subtitle={i18n.t('naga.enabled.subtitle')}
            />
          }
          trailing={
            <ToggleSwitch
              checked={nagaState.enabled}
              onChange={(enabled) =>
                void xtoolsApi.setNagaEnabled(enabled).then(onUpdateNagaState)
              }
            />
          }
        />

        <FormRow
          leading={
            <IconLabel
              color="#30B0C7"
              icon={<Sliders size={14} strokeWidth={2.1} />}
              title={i18n.t('naga.plate.title')}
            />
          }
          trailing={
            <SelectControl
              value={nagaState.plate_layout}
              options={layoutOptions}
              onChange={(layout) =>
                void xtoolsApi.setNagaPlateLayout(layout).then(onUpdateNagaState)
              }
            />
          }
        />

        {nagaState.mappings.map((m) => {
          const recordingSentinel =
            nagaState.recording?.button_index === m.index &&
            nagaState.recording?.mode === 'sentinel';
          const recordingTarget =
            nagaState.recording?.button_index === m.index &&
            nagaState.recording?.mode === 'target';

          return (
            <div
              key={m.index}
              className="flex items-center justify-between gap-3 px-3.5 py-2.5"
            >
              <div className="flex min-w-0 flex-1 items-center gap-2.5">
                <span className="inline-flex h-[24px] w-[24px] shrink-0 items-center justify-center rounded-full bg-[#44D62C]/15 text-[12px] font-bold text-[#4ADE80]">
                  {m.index}
                </span>

                <button
                  type="button"
                  onClick={() =>
                    void xtoolsApi
                      .beginNagaRecording(m.index, 'sentinel')
                      .then(onUpdateNagaState)
                  }
                  title={i18n.t('naga.sentinel.hint')}
                  className={`inline-flex h-[26px] min-w-[68px] cursor-pointer items-center justify-center rounded-[6px] border px-2 font-mono text-[11.5px] font-medium transition-colors ${
                    recordingSentinel
                      ? 'border-[#0A84FF] bg-[#0A84FF]/20 text-[#60A5FA]'
                      : 'border-white/[0.12] bg-white/[0.06] text-white/[0.78] hover:border-white/[0.25]'
                  }`}
                >
                  {recordingSentinel
                    ? i18n.t('naga.sentinel.recording')
                    : i18n.vkName(m.sentinel_vk)}
                </button>

                <ArrowRight size={13} className="shrink-0 text-white/[0.32]" />

                <button
                  type="button"
                  onClick={() =>
                    void xtoolsApi
                      .beginNagaRecording(m.index, 'target')
                      .then(onUpdateNagaState)
                  }
                  className={`inline-flex h-[28px] min-w-[165px] max-w-[220px] flex-1 cursor-pointer items-center justify-center rounded-[7px] border px-2.5 text-[12px] font-medium transition-colors ${
                    recordingTarget
                      ? 'border-[#0A84FF] bg-[#0A84FF]/20 text-[#60A5FA]'
                      : m.target
                        ? 'border-white/[0.16] bg-[#2c2c34] text-white/[0.95] hover:border-white/[0.28]'
                        : 'border-white/[0.1] bg-[#25252c] text-white/[0.42] hover:border-white/[0.22]'
                  }`}
                >
                  <span className="truncate">
                    {recordingTarget
                      ? i18n.t('naga.recorder.recording')
                      : m.target
                        ? i18n.formatShortcut(m.target)
                        : i18n.t('naga.recorder.empty')}
                  </span>
                </button>

                <button
                  type="button"
                  disabled={!m.target}
                  onClick={() =>
                    void xtoolsApi
                      .setNagaButtonShortcut(m.index, null)
                      .then(onUpdateNagaState)
                  }
                  title={i18n.t('naga.recorder.clear')}
                  className={`inline-flex h-[24px] w-[24px] shrink-0 items-center justify-center rounded-full text-white/[0.45] transition-opacity hover:bg-white/[0.08] hover:text-white ${
                    m.target ? 'cursor-pointer opacity-100' : 'pointer-events-none opacity-0'
                  }`}
                >
                  <X size={13} />
                </button>
              </div>

              <ToggleSwitch
                checked={m.enabled}
                onChange={(enabled) =>
                  void xtoolsApi
                    .setNagaButtonEnabled(m.index, enabled)
                    .then(onUpdateNagaState)
                }
              />
            </div>
          );
        })}
      </FormSection>

      {/* Live Input Monitor */}
      <FormSection header={i18n.t('naga.section.monitor')}>
        <FormRow
          leading={
            <IconLabel
              color="#5856D6"
              icon={<Activity size={14} strokeWidth={2.1} />}
              title={i18n.t('naga.section.monitor')}
            />
          }
          trailing={
            <button
              type="button"
              disabled={nagaState.captures.length === 0}
              onClick={() => void xtoolsApi.clearNagaCaptures().then(onUpdateNagaState)}
              className="inline-flex h-[26px] cursor-pointer items-center rounded-[6px] border border-white/[0.12] bg-white/[0.06] px-2.5 text-[11.5px] font-medium text-white/[0.78] transition-colors hover:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
            >
              {i18n.t('naga.monitor.clear')}
            </button>
          }
        />

        {nagaState.captures.length === 0 ? (
          <div className="px-3.5 py-3 text-[12px] text-white/[0.48]">
            {i18n.t('naga.monitor.empty')}
          </div>
        ) : (
          <div className="max-h-[200px] divide-y divide-white/[0.06] overflow-y-auto">
            {nagaState.captures.map((cap) => (
              <div
                key={cap.id}
                className="flex items-center justify-between gap-3 px-3.5 py-2 text-[12px]"
              >
                <div className="flex min-w-0 items-center gap-2.5">
                  <span className="inline-flex min-w-[48px] items-center justify-center rounded-[5px] border border-white/[0.14] bg-white/[0.07] px-2 py-0.5 font-mono text-[11.5px] font-semibold text-white">
                    {cap.display}
                  </span>
                  <span className="truncate font-mono text-[11px] text-white/[0.48]">
                    {cap.detail}
                  </span>
                  {cap.remapped_to && (
                    <span className="rounded bg-[#44D62C]/15 px-1.5 py-0.5 text-[11px] font-medium text-[#4ADE80]">
                      {i18n.t('naga.monitor.remapped', { target: cap.remapped_to })}
                    </span>
                  )}
                </div>
                <span className="shrink-0 font-mono text-[11px] text-white/[0.4]">
                  {formatCaptureTime(cap.timestamp_ms)}
                </span>
              </div>
            ))}
          </div>
        )}
      </FormSection>
    </div>
  );
}
