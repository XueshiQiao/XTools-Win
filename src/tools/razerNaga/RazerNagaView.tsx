import React, { useEffect, useMemo, useState } from 'react';
import {
  Battery,
  BatteryCharging,
  BatteryFull,
  BatteryLow,
  ChevronDown,
  Gauge,
  Keyboard,
  Layers,
  Minus,
  Mouse,
  Plus,
  RotateCcw,
  Sliders,
  X,
} from 'lucide-react';
import {
  ActionButton,
  Banner,
  FormRow,
  FormSection,
  IconLabel,
  SelectControl,
  StatusDot,
} from '../../components/AppChrome';
import { xtoolsApi } from '../../core/api';
import { ToolRenderProps } from '../../core/toolRegistry';
import {
  NagaBatteryInfo,
  NagaDpi,
  NagaDpiOp,
  NagaHardwareAction,
  NagaMouseButton,
  NagaPlateLayout,
} from '../../core/types';

const PROFILE_SLOTS = [1, 2, 3, 4, 5] as const;
const PROFILE_COLORS: Record<number, string> = {
  1: '#F5F5F7',
  2: '#FF453A',
  3: '#30D158',
  4: '#0A84FF',
  5: '#32ADE6',
};
const DPI_MIN = 100;
const DPI_MAX = 35000;
const DPI_PRESETS = [400, 800, 1200, 1600, 2400, 3200];
const POLLING_RATES = [125, 500, 1000];

const MODIFIER_CODES = new Set([
  'ControlLeft',
  'ControlRight',
  'ShiftLeft',
  'ShiftRight',
  'AltLeft',
  'AltRight',
  'MetaLeft',
  'MetaRight',
]);

const CODE_TO_VK: Record<string, number> = {
  Enter: 0x0d,
  NumpadEnter: 0x0d,
  Escape: 0x1b,
  Backspace: 0x08,
  Tab: 0x09,
  Space: 0x20,
  Minus: 0xbd,
  Equal: 0xbb,
  BracketLeft: 0xdb,
  BracketRight: 0xdd,
  Backslash: 0xdc,
  Semicolon: 0xba,
  Quote: 0xde,
  Backquote: 0xc0,
  Comma: 0xbc,
  Period: 0xbe,
  Slash: 0xbf,
  CapsLock: 0x14,
  PrintScreen: 0x2c,
  ScrollLock: 0x91,
  Pause: 0x13,
  Insert: 0x2d,
  Home: 0x24,
  PageUp: 0x21,
  Delete: 0x2e,
  End: 0x23,
  PageDown: 0x22,
  ArrowRight: 0x27,
  ArrowLeft: 0x25,
  ArrowDown: 0x28,
  ArrowUp: 0x26,
  NumLock: 0x90,
  NumpadDivide: 0x6f,
  NumpadMultiply: 0x6a,
  NumpadSubtract: 0x6d,
  NumpadAdd: 0x6b,
  NumpadDecimal: 0x6e,
};

function eventToWin32Vk(e: KeyboardEvent): number | null {
  if (CODE_TO_VK[e.code] !== undefined) {
    return CODE_TO_VK[e.code];
  }
  if (/^Key[A-Z]$/.test(e.code)) {
    return e.code.charCodeAt(3);
  }
  if (/^Digit[0-9]$/.test(e.code)) {
    return e.code.charCodeAt(5);
  }
  if (/^Numpad[0-9]$/.test(e.code)) {
    return 0x60 + Number(e.code.slice(6));
  }
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.code)) {
    return 0x6f + Number(e.code.slice(1));
  }
  if (e.keyCode >= 0x70 && e.keyCode <= 0x87) {
    return e.keyCode;
  }
  return null;
}

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

function quickActionValue(action: NagaHardwareAction): string {
  switch (action.kind) {
    case 'keyboard':
      return 'keyboard';
    case 'mouse':
      return `mouse:${action.button}`;
    case 'dpi':
      return `dpi:${action.op}`;
    case 'disabled':
      return 'disabled';
    default:
      return 'other';
  }
}

export function RazerNagaView({
  i18n,
  nagaState,
  onUpdateNagaState,
}: ToolRenderProps) {
  const [recordingButtonId, setRecordingButtonId] = useState<number | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);
  const [draftStages, setDraftStages] = useState<NagaDpi[]>([{ x: 1200, y: 1200 }]);
  const [draftActiveStage, setDraftActiveStage] = useState<number>(1);
  const [dpiInputText, setDpiInputText] = useState<string>('1200');
  const [testText, setTestText] = useState('');
  const [lastTestEvent, setLastTestEvent] = useState<string | null>(null);

  const stagesKey = useMemo(
    () => JSON.stringify(nagaState.dpi_stages),
    [nagaState.dpi_stages]
  );

  useEffect(() => {
    if (nagaState.dpi_stages && nagaState.dpi_stages.stages.length > 0) {
      setDraftStages(nagaState.dpi_stages.stages.map((s) => ({ ...s })));
      setDraftActiveStage(nagaState.dpi_stages.active_stage);
      const activeIdx = Math.max(
        0,
        Math.min(nagaState.dpi_stages.active_stage - 1, nagaState.dpi_stages.stages.length - 1)
      );
      setDpiInputText(String(nagaState.dpi_stages.stages[activeIdx]?.x ?? 1200));
    } else if (nagaState.dpi) {
      setDraftStages([{ ...nagaState.dpi }]);
      setDraftActiveStage(1);
      setDpiInputText(String(nagaState.dpi.x));
    }
  }, [stagesKey, nagaState.dpi]);

  useEffect(() => {
    if (recordingButtonId === null) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (MODIFIER_CODES.has(e.code)) return;
      e.preventDefault();
      e.stopPropagation();

      if (e.code === 'Escape' && !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
        setRecordingButtonId(null);
        return;
      }

      const vk = eventToWin32Vk(e);
      if (vk === null) {
        setLocalError(i18n.t('naga.bind.unsupportedKey'));
        setRecordingButtonId(null);
        return;
      }

      const targetBtn = recordingButtonId;
      setRecordingButtonId(null);
      setLocalError(null);

      const action: NagaHardwareAction = {
        kind: 'keyboard',
        vk,
        hid_code: 0,
        ctrl: e.ctrlKey,
        shift: e.shiftKey,
        alt: e.altKey,
        win: e.metaKey,
      };
      void xtoolsApi
        .setNagaButtonAction(targetBtn, action)
        .then(onUpdateNagaState)
        .catch((err) => setLocalError(String(err)));
    };

    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [recordingButtonId, i18n, onUpdateNagaState]);

  const runAction = (promise: Promise<typeof nagaState>) => {
    setLocalError(null);
    void promise
      .then(onUpdateNagaState)
      .catch((err) => setLocalError(String(err)));
  };

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

  const activeStageIndex = Math.max(
    0,
    Math.min(draftActiveStage - 1, draftStages.length - 1)
  );
  const currentDraftDpi = draftStages[activeStageIndex]?.x ?? 1200;

  const isDpiDirty = useMemo(() => {
    const hw = nagaState.dpi_stages;
    if (!hw) return false;
    if (hw.active_stage !== draftActiveStage || hw.stages.length !== draftStages.length) {
      return true;
    }
    return hw.stages.some(
      (s, idx) => s.x !== draftStages[idx]?.x || s.y !== draftStages[idx]?.y
    );
  }, [nagaState.dpi_stages, draftActiveStage, draftStages]);

  const commitDpiStages = (nextActive: number, nextStages: NagaDpi[]) => {
    setDraftActiveStage(nextActive);
    setDraftStages(nextStages);
    const activeDpi = nextStages[Math.max(0, Math.min(nextActive - 1, nextStages.length - 1))]?.x;
    if (activeDpi !== undefined) {
      setDpiInputText(String(activeDpi));
    }
    runAction(xtoolsApi.setNagaDpiStages(nextActive, nextStages));
  };

  const clampDpi = (rawVal: number) =>
    Math.max(DPI_MIN, Math.min(DPI_MAX, Math.round(rawVal / 50) * 50));

  const updateActiveDraftDpi = (rawVal: number, syncText = true) => {
    const clamped = clampDpi(rawVal);
    setDraftStages((prev) =>
      prev.map((stage, idx) =>
        idx === activeStageIndex ? { x: clamped, y: clamped } : stage
      )
    );
    if (syncText) {
      setDpiInputText(String(clamped));
    }
  };

  const resolveStagesWithDpiText = (): NagaDpi[] => {
    const parsed = Number(dpiInputText);
    if (!Number.isFinite(parsed) || dpiInputText.trim() === '') {
      return draftStages;
    }
    const clamped = clampDpi(parsed);
    return draftStages.map((stage, idx) =>
      idx === activeStageIndex ? { x: clamped, y: clamped } : stage
    );
  };

  const handleQuickActionChange = (buttonId: number, selected: string) => {
    if (selected === 'keyboard') {
      setLocalError(null);
      setRecordingButtonId(buttonId);
      return;
    }
    setRecordingButtonId(null);
    if (selected === 'disabled') {
      runAction(xtoolsApi.setNagaButtonAction(buttonId, { kind: 'disabled' }));
      return;
    }
    if (selected.startsWith('mouse:')) {
      const button = selected.slice(6) as NagaMouseButton;
      runAction(xtoolsApi.setNagaButtonAction(buttonId, { kind: 'mouse', button }));
      return;
    }
    if (selected.startsWith('dpi:')) {
      const op = selected.slice(4) as NagaDpiOp;
      runAction(xtoolsApi.setNagaButtonAction(buttonId, { kind: 'dpi', op }));
    }
  };

  const activeProfile = nagaState.profile_info?.active_profile ?? 1;
  const availableProfiles = nagaState.profile_info?.available_profiles ?? [1];
  const placeholderCount =
    nagaState.plate_layout === 'two_button'
      ? 2
      : nagaState.plate_layout === 'twelve_button'
        ? 12
        : 6;

  const warningMsg = localError ?? nagaState.error_message;

  return (
    <div className="mx-auto max-w-[680px] space-y-5 px-6 py-5">
      {warningMsg ? (
        <Banner kind="warning" message={warningMsg} />
      ) : nagaState.status_message ? (
        <Banner kind="success" message={nagaState.status_message} />
      ) : null}

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
                  {i18n.t('naga.status.activeProfile')}
                </span>
              }
              trailing={
                <div className="flex items-center gap-2">
                  <span
                    className="inline-block h-2.5 w-2.5 rounded-full border border-white/25"
                    style={{ backgroundColor: PROFILE_COLORS[activeProfile] ?? '#F5F5F7' }}
                  />
                  <span className="text-[12px] font-semibold text-white/[0.95]">
                    {i18n.t('naga.profile.slot', { n: activeProfile })}
                  </span>
                </div>
              }
            />

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

            {nagaState.polling_rate_hz !== null && (
              <FormRow
                leading={
                  <span className="text-[13px] text-white/[0.85]">
                    {i18n.t('naga.status.pollingRate')}
                  </span>
                }
                trailing={
                  <span className="font-mono text-[12px] text-white/[0.85]">
                    {nagaState.polling_rate_hz} Hz
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

      {/* Onboard Profile & Sensitivity */}
      <FormSection
        header={i18n.t('naga.section.profileDpi')}
        footer={i18n.t('naga.profileDpi.footer')}
      >
        <FormRow
          leading={
            <IconLabel
              color="#0A84FF"
              icon={<Layers size={14} strokeWidth={2.1} />}
              title={i18n.t('naga.profile.title')}
            />
          }
          trailing={
            <div className="flex flex-wrap items-center gap-1.5">
              {PROFILE_SLOTS.map((slot) => {
                const isActive = activeProfile === slot;
                const exists = availableProfiles.includes(slot);
                return (
                  <div key={slot} className="inline-flex items-center">
                    <button
                      type="button"
                      disabled={!nagaState.is_connected || nagaState.is_busy}
                      onClick={() => runAction(xtoolsApi.setNagaActiveProfile(slot))}
                      className={`inline-flex h-[27px] cursor-pointer items-center gap-1.5 border px-2.5 text-[11.5px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${
                        exists && slot > 1 ? 'rounded-l-[7px]' : 'rounded-[7px]'
                      } ${
                        isActive
                          ? 'border-[#0A84FF] bg-[#0A84FF]/20 text-white'
                          : exists
                            ? 'border-white/[0.14] bg-white/[0.07] text-white/[0.85] hover:border-white/[0.28]'
                            : 'border-dashed border-white/[0.12] bg-white/[0.02] text-white/[0.45] hover:border-white/[0.26] hover:text-white/[0.75]'
                      }`}
                    >
                      <span
                        className="inline-block h-2 w-2 rounded-full border border-white/25"
                        style={{
                          backgroundColor: PROFILE_COLORS[slot],
                          opacity: exists || isActive ? 1 : 0.45,
                        }}
                      />
                      <span>{slot}</span>
                    </button>
                    {exists && slot > 1 && (
                      <button
                        type="button"
                        disabled={!nagaState.is_connected || nagaState.is_busy}
                        title={i18n.t('naga.profile.deleteHint', { n: slot })}
                        onClick={() => runAction(xtoolsApi.deleteNagaProfile(slot))}
                        className="inline-flex h-[27px] w-[20px] cursor-pointer items-center justify-center rounded-r-[7px] border border-l-0 border-white/[0.14] bg-white/[0.05] text-white/[0.5] transition-colors hover:bg-[#FF453A]/20 hover:text-[#FF6961] disabled:cursor-not-allowed disabled:opacity-45"
                      >
                        <X size={11} />
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          }
        />

        {/* DPI Stages */}
        <div className="space-y-3 px-3.5 py-3">
          <div className="flex items-center justify-between gap-3">
            <IconLabel
              color="#FF9F0A"
              icon={<Gauge size={14} strokeWidth={2.1} />}
              title={i18n.t('naga.dpi.title')}
            />
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                disabled={
                  !nagaState.is_connected ||
                  nagaState.is_busy ||
                  draftStages.length <= 1
                }
                title={i18n.t('naga.dpi.removeStage')}
                onClick={() => {
                  const nextStages = draftStages.filter((_, idx) => idx !== activeStageIndex);
                  const nextActive = Math.min(draftActiveStage, nextStages.length);
                  commitDpiStages(nextActive, nextStages);
                }}
                className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] border border-white/[0.12] bg-white/[0.06] text-white/[0.8] hover:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-35"
              >
                <Minus size={13} />
              </button>
              <button
                type="button"
                disabled={
                  !nagaState.is_connected ||
                  nagaState.is_busy ||
                  draftStages.length >= 5
                }
                title={i18n.t('naga.dpi.addStage')}
                onClick={() => {
                  const last = draftStages[draftStages.length - 1]?.x ?? 1200;
                  const nextVal = Math.min(DPI_MAX, last + 400);
                  const nextStages = [...draftStages, { x: nextVal, y: nextVal }];
                  commitDpiStages(nextStages.length, nextStages);
                }}
                className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] border border-white/[0.12] bg-white/[0.06] text-white/[0.8] hover:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-35"
              >
                <Plus size={13} />
              </button>
            </div>
          </div>

          {/* Stage Selector Pills */}
          <div className="flex flex-wrap gap-1.5">
            {draftStages.map((stage, idx) => {
              const stageNum = idx + 1;
              const isSelected = stageNum === draftActiveStage;
              return (
                <button
                  key={stageNum}
                  type="button"
                  disabled={!nagaState.is_connected || nagaState.is_busy}
                  onClick={() => commitDpiStages(stageNum, draftStages)}
                  className={`inline-flex h-[28px] cursor-pointer items-center gap-1.5 rounded-[7px] border px-2.5 text-[11.5px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${
                    isSelected
                      ? 'border-[#FF9F0A]/75 bg-[#FF9F0A]/20 text-[#FFD60A]'
                      : 'border-white/[0.12] bg-white/[0.05] text-white/[0.78] hover:border-white/[0.25]'
                  }`}
                >
                  <span>{i18n.t('naga.dpi.stage', { n: stageNum })}</span>
                  <span className="font-mono text-[11px] opacity-85">{stage.x}</span>
                </button>
              );
            })}
          </div>

          {/* Slider + Numeric Input + Apply */}
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={DPI_MIN}
              max={DPI_MAX}
              step={50}
              disabled={!nagaState.is_connected || nagaState.is_busy}
              value={currentDraftDpi}
              onChange={(e) => updateActiveDraftDpi(Number(e.target.value), true)}
              className="h-1.5 flex-1 cursor-pointer accent-[#FF9F0A] disabled:cursor-not-allowed disabled:opacity-45"
            />
            <div className="flex items-center gap-1.5">
              <input
                type="number"
                min={DPI_MIN}
                max={DPI_MAX}
                step={50}
                disabled={!nagaState.is_connected || nagaState.is_busy}
                value={dpiInputText}
                onChange={(e) => {
                  const text = e.target.value;
                  setDpiInputText(text);
                  const parsed = Number(text);
                  if (Number.isFinite(parsed) && parsed >= DPI_MIN && parsed <= DPI_MAX) {
                    updateActiveDraftDpi(parsed, false);
                  }
                }}
                onBlur={() => {
                  const parsed = Number(dpiInputText);
                  if (Number.isFinite(parsed) && dpiInputText.trim() !== '') {
                    updateActiveDraftDpi(parsed, true);
                  } else {
                    setDpiInputText(String(currentDraftDpi));
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    commitDpiStages(draftActiveStage, resolveStagesWithDpiText());
                  }
                }}
                className="h-[28px] w-[76px] rounded-[7px] border border-white/[0.14] bg-[#2c2c34] px-2 text-right font-mono text-[12px] text-white focus:border-[#0A84FF] focus:outline-none disabled:opacity-45"
              />
              <span className="text-[11px] text-white/[0.5]">DPI</span>
            </div>
            <ActionButton
              label={i18n.t('naga.dpi.apply')}
              variant={isDpiDirty ? 'primary' : 'secondary'}
              disabled={!nagaState.is_connected || nagaState.is_busy}
              onClick={() =>
                commitDpiStages(draftActiveStage, resolveStagesWithDpiText())
              }
            />
          </div>

          {/* Quick DPI Presets */}
          <div className="flex flex-wrap items-center gap-1.5">
            {DPI_PRESETS.map((preset) => (
              <button
                key={preset}
                type="button"
                disabled={!nagaState.is_connected || nagaState.is_busy}
                onClick={() => {
                  const nextStages = draftStages.map((s, idx) =>
                    idx === activeStageIndex ? { x: preset, y: preset } : s
                  );
                  commitDpiStages(draftActiveStage, nextStages);
                }}
                className={`h-[24px] cursor-pointer rounded-[6px] border px-2 font-mono text-[11px] transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
                  currentDraftDpi === preset
                    ? 'border-[#FF9F0A]/60 bg-[#FF9F0A]/15 text-[#FFD60A]'
                    : 'border-white/[0.1] bg-white/[0.04] text-white/[0.65] hover:border-white/[0.22] hover:text-white'
                }`}
              >
                {preset}
              </button>
            ))}
          </div>
        </div>

        {/* Polling Rate */}
        <FormRow
          leading={
            <span className="text-[13px] font-medium text-white/[0.88]">
              {i18n.t('naga.polling.title')}
            </span>
          }
          trailing={
            <div className="flex items-center gap-1.5">
              {POLLING_RATES.map((hz) => {
                const active = nagaState.polling_rate_hz === hz;
                return (
                  <button
                    key={hz}
                    type="button"
                    disabled={!nagaState.is_connected || nagaState.is_busy}
                    onClick={() => runAction(xtoolsApi.setNagaPollingRate(hz))}
                    className={`h-[26px] cursor-pointer rounded-[6px] border px-2.5 font-mono text-[11.5px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${
                      active
                        ? 'border-[#0A84FF] bg-[#0A84FF]/20 text-[#60A5FA]'
                        : 'border-white/[0.12] bg-white/[0.05] text-white/[0.75] hover:border-white/[0.24]'
                    }`}
                  >
                    {hz} Hz
                  </button>
                );
              })}
            </div>
          }
        />
      </FormSection>

      {/* Side-Plate Hardware Mappings */}
      <FormSection
        header={i18n.t('naga.section.mappings')}
        footer={i18n.t('naga.mappings.footer')}
      >
        <FormRow
          leading={
            <IconLabel
              color="#30B0C7"
              icon={<Sliders size={14} strokeWidth={2.1} />}
              title={i18n.t('naga.plate.title')}
            />
          }
          trailing={
            <div className="flex items-center gap-2">
              <SelectControl
                value={nagaState.plate_layout}
                options={layoutOptions}
                onChange={(layout) => runAction(xtoolsApi.setNagaPlateLayout(layout))}
              />
              <button
                type="button"
                disabled={!nagaState.is_connected || nagaState.is_busy}
                onClick={() => runAction(xtoolsApi.resetNagaPlateDefaults())}
                className="inline-flex h-[28px] cursor-pointer items-center gap-1.5 rounded-[7px] border border-white/[0.12] bg-white/[0.06] px-2.5 text-[11.5px] font-medium text-white/[0.85] transition-colors hover:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
              >
                <RotateCcw size={12} />
                <span>{i18n.t('naga.plate.reset')}</span>
              </button>
            </div>
          }
        />

        {nagaState.is_connected && nagaState.buttons.length > 0 ? (
          nagaState.buttons.map((btn) => {
            const isRecording = recordingButtonId === btn.button_id;
            const quickVal = quickActionValue(btn.action);
            return (
              <div
                key={btn.button_id}
                className="flex items-center justify-between gap-3 px-3.5 py-2.5"
              >
                <div className="flex min-w-0 flex-1 items-center gap-2.5">
                  <span className="inline-flex h-[24px] w-[24px] shrink-0 items-center justify-center rounded-full bg-[#44D62C]/15 text-[12px] font-bold text-[#4ADE80]">
                    {btn.index}
                  </span>

                  <button
                    type="button"
                    disabled={nagaState.is_busy}
                    onClick={() => {
                      setLocalError(null);
                      setRecordingButtonId(isRecording ? null : btn.button_id);
                    }}
                    className={`inline-flex h-[28px] min-w-[165px] max-w-[260px] flex-1 cursor-pointer items-center justify-center rounded-[7px] border px-3 text-[12px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${
                      isRecording
                        ? 'border-[#0A84FF] bg-[#0A84FF]/20 text-[#60A5FA]'
                        : btn.action.kind === 'disabled'
                          ? 'border-white/[0.1] bg-[#25252c] text-white/[0.42] hover:border-white/[0.22]'
                          : 'border-white/[0.16] bg-[#2c2c34] text-white/[0.95] hover:border-white/[0.3]'
                    }`}
                  >
                    <span className="truncate">
                      {isRecording
                        ? i18n.t('naga.bind.recording')
                        : i18n.formatNagaAction(btn.action)}
                    </span>
                  </button>
                </div>

                <div className="relative inline-flex shrink-0 items-center">
                  <select
                    value={quickVal}
                    disabled={nagaState.is_busy}
                    onChange={(e) => handleQuickActionChange(btn.button_id, e.target.value)}
                    className="h-[28px] cursor-pointer appearance-none rounded-[7px] border border-white/[0.12] bg-[#25252c] pl-2.5 pr-7 text-[11.5px] text-white/[0.85] transition-colors hover:border-white/[0.24] focus:border-[#0A84FF] focus:outline-none disabled:cursor-not-allowed disabled:opacity-45"
                  >
                    <option value="keyboard" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.type.keyboard')}
                    </option>
                    {quickVal === 'other' && (
                      <option value="other" disabled className="bg-[#23232a] text-white">
                        {i18n.t('naga.action.type.other')}
                      </option>
                    )}
                    <option value="mouse:back" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.mouse.back')}
                    </option>
                    <option value="mouse:forward" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.mouse.forward')}
                    </option>
                    <option value="mouse:left" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.mouse.left')}
                    </option>
                    <option value="mouse:right" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.mouse.right')}
                    </option>
                    <option value="mouse:middle" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.mouse.middle')}
                    </option>
                    <option value="mouse:scroll_up" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.mouse.scroll_up')}
                    </option>
                    <option value="mouse:scroll_down" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.mouse.scroll_down')}
                    </option>
                    <option value="dpi:up" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.dpi.up')}
                    </option>
                    <option value="dpi:down" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.dpi.down')}
                    </option>
                    <option value="dpi:cycle_up" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.dpi.cycle_up')}
                    </option>
                    <option value="dpi:cycle_down" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.dpi.cycle_down')}
                    </option>
                    <option value="disabled" className="bg-[#23232a] text-white">
                      {i18n.t('naga.action.type.disabled')}
                    </option>
                  </select>
                  <ChevronDown
                    size={12}
                    className="pointer-events-none absolute right-2 text-white/[0.5]"
                  />
                </div>
              </div>
            );
          })
        ) : (
          Array.from({ length: placeholderCount }, (_, idx) => (
            <div
              key={idx + 1}
              className="flex items-center justify-between gap-3 px-3.5 py-2.5 opacity-45"
            >
              <div className="flex items-center gap-2.5">
                <span className="inline-flex h-[24px] w-[24px] shrink-0 items-center justify-center rounded-full bg-[#44D62C]/15 text-[12px] font-bold text-[#4ADE80]">
                  {idx + 1}
                </span>
                <span className="inline-flex h-[28px] min-w-[165px] items-center justify-center rounded-[7px] border border-white/[0.1] bg-[#25252c] px-3 text-[12px] text-white/[0.5]">
                  {i18n.t('naga.bind.empty')}
                </span>
              </div>
            </div>
          ))
        )}
      </FormSection>

      {/* Local Key Test Box */}
      <FormSection header={i18n.t('naga.section.test')}>
        <FormRow
          leading={
            <IconLabel
              color="#5856D6"
              icon={<Keyboard size={14} strokeWidth={2.1} />}
              title={i18n.t('naga.section.test')}
            />
          }
          trailing={
            <div className="flex items-center gap-2">
              {lastTestEvent && (
                <span className="rounded bg-[#0A84FF]/20 px-2 py-0.5 font-mono text-[11.5px] font-medium text-[#60A5FA]">
                  {i18n.t('naga.test.lastKey')}: {lastTestEvent}
                </span>
              )}
              <button
                type="button"
                disabled={!testText && !lastTestEvent}
                onClick={() => {
                  setTestText('');
                  setLastTestEvent(null);
                }}
                className="inline-flex h-[26px] cursor-pointer items-center rounded-[6px] border border-white/[0.12] bg-white/[0.06] px-2.5 text-[11.5px] font-medium text-white/[0.78] transition-colors hover:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
              >
                {i18n.t('naga.test.clear')}
              </button>
            </div>
          }
        />
        <div className="px-3.5 py-2.5">
          <input
            type="text"
            value={testText}
            onChange={(e) => setTestText(e.target.value)}
            onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => {
              const mods: string[] = [];
              if (e.ctrlKey) mods.push('Ctrl');
              if (e.altKey) mods.push('Alt');
              if (e.shiftKey) mods.push('Shift');
              if (e.metaKey) mods.push('Win');
              const label = [...mods, e.code || e.key].join('+');
              setLastTestEvent(label);
            }}
            onMouseDown={(e: React.MouseEvent<HTMLInputElement>) => {
              if (e.button === 3 || e.button === 4 || e.button === 1) {
                setLastTestEvent(`Mouse ${e.button + 1}`);
              }
            }}
            placeholder={i18n.t('naga.test.placeholder')}
            className="h-[34px] w-full rounded-[8px] border border-white/[0.12] bg-[#222229] px-3 font-mono text-[12.5px] text-white placeholder:font-sans placeholder:text-white/[0.38] focus:border-[#0A84FF] focus:outline-none"
          />
        </div>
      </FormSection>
    </div>
  );
}
