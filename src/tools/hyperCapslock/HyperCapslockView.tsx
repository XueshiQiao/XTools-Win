import React, { useEffect, useRef, useState } from 'react';
import {
  AppWindow,
  Bell,
  Check,
  Command,
  Download,
  Pause,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  Trash2,
  Upload,
  X,
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
import { I18nManager } from '../../core/i18n';
import { ToolRenderProps } from '../../core/toolRegistry';
import {
  DirectionalActionKind,
  HyperActionConfig,
  HyperActionMappingEntry,
  HyperModifierKey,
  HyperTrigger,
  IndependentActionKind,
  JumpDirection,
  RunningProcessInfo,
} from '../../core/types';

type TriggerSelection =
  | 'plain'
  | 'with_shift'
  | 'single_tap'
  | 'double_tap'
  | `dtm:${HyperModifierKey}`;

const MODIFIER_KEYS: HyperModifierKey[] = [
  'left_control',
  'right_control',
  'left_alt',
  'right_alt',
  'left_shift',
  'right_shift',
  'left_win',
  'right_win',
];

const ACTION_GROUPS: Array<HyperActionConfig['kind']> = [
  'directional',
  'jump',
  'independent',
  'command',
  'key_combo',
];

const IGNORED_CAPTURE_KEYS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'CapsLock']);

function triggerSortKey(t: HyperTrigger): string {
  if (t.kind === 'single_tap_hyper') return '0:single';
  if (t.kind === 'double_tap_hyper') return '0:double';
  if (t.kind === 'double_tap_modifier') return `0:dtm:${t.modifier}`;
  return `1:${String(t.key).padStart(4, '0')}:${t.with_shift ? '1' : '0'}`;
}

function triggerUniqueKey(t: HyperTrigger): string {
  if (t.kind === 'single_tap_hyper') return 'single';
  if (t.kind === 'double_tap_hyper') return 'double';
  if (t.kind === 'double_tap_modifier') return `dtm:${t.modifier}`;
  return `hpk:${t.key}:${t.with_shift ? '1' : '0'}`;
}

function ProcessIcon({ src }: { src?: string | null }) {
  return (
    <span className="inline-flex h-[18px] w-[18px] shrink-0 items-center justify-center">
      {src ? (
        <img src={src} alt="" className="h-[18px] w-[18px] object-contain" draggable={false} />
      ) : (
        <AppWindow size={14} className="text-white/[0.42]" />
      )}
    </span>
  );
}

function KeyCap({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="inline-flex min-w-[26px] items-center justify-center rounded-[5px] border border-white/[0.15] border-b-2 border-b-black/60 bg-white/[0.08] px-1.5 py-0.5 font-mono text-[11.5px] font-medium text-white/[0.9]">
      {children}
    </kbd>
  );
}

function renderTriggerBadges(trigger: HyperTrigger, i18n: I18nManager) {
  if (trigger.kind === 'single_tap_hyper') {
    return <KeyCap>{i18n.t('hyper.trigger.single_tap')}</KeyCap>;
  }
  if (trigger.kind === 'double_tap_hyper') {
    return <KeyCap>{i18n.t('hyper.trigger.double_tap')}</KeyCap>;
  }
  if (trigger.kind === 'double_tap_modifier') {
    return <KeyCap>{i18n.t(`hyper.mod.${trigger.modifier}`)}</KeyCap>;
  }
  return (
    <div className="flex items-center gap-1">
      <KeyCap>Caps</KeyCap>
      <span className="text-[11px] text-white/[0.38]">+</span>
      {trigger.with_shift && (
        <>
          <KeyCap>Shift</KeyCap>
          <span className="text-[11px] text-white/[0.38]">+</span>
        </>
      )}
      <KeyCap>{i18n.vkName(trigger.key)}</KeyCap>
    </div>
  );
}

function describeAction(action: HyperActionConfig, i18n: I18nManager): string {
  switch (action.kind) {
    case 'directional':
      return i18n.t(`hyper.action.${action.action}`);
    case 'jump':
      return action.direction === 'up'
        ? i18n.t('hyper.action.jump_up', { count: action.count })
        : i18n.t('hyper.action.jump_down', { count: action.count });
    case 'independent':
      return i18n.t(`hyper.action.${action.action}`);
    case 'command':
      return action.command;
    case 'key_combo': {
      const parts: string[] = [];
      if (action.with_ctrl) parts.push('Ctrl');
      if (action.with_alt) parts.push('Alt');
      if (action.with_shift) parts.push('Shift');
      if (action.with_win) parts.push('Win');
      parts.push(i18n.vkName(action.target_key));
      return parts.join('+');
    }
  }
}

export function HyperCapslockView({
  i18n,
  hyperState,
  onUpdateHyperState,
}: ToolRenderProps) {
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [notice, setNotice] = useState<{ kind: 'success' | 'warning'; text: string } | null>(
    null
  );
  const [modalOpen, setModalOpen] = useState(false);
  const [editingTrigger, setEditingTrigger] = useState<HyperTrigger | null>(null);

  const [triggerSel, setTriggerSel] = useState<TriggerSelection>('plain');
  const [triggerKey, setTriggerKey] = useState<number | null>(null);

  const [actionKind, setActionKind] = useState<HyperActionConfig['kind']>('directional');
  const [dirAction, setDirAction] = useState<DirectionalActionKind>('left');
  const [jumpDir, setJumpDir] = useState<JumpDirection>('up');
  const [jumpCount, setJumpCount] = useState(10);
  const [indAction, setIndAction] = useState<IndependentActionKind>('backspace');
  const [commandText, setCommandText] = useState('');
  const [comboKey, setComboKey] = useState<number | null>(null);
  const [comboCtrl, setComboCtrl] = useState(false);
  const [comboAlt, setComboAlt] = useState(false);
  const [comboWin, setComboWin] = useState(false);
  const [comboShift, setComboShift] = useState(false);

  const [processInput, setProcessInput] = useState('');
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerQuery, setPickerQuery] = useState('');
  const [runningProcesses, setRunningProcesses] = useState<RunningProcessInfo[]>([]);
  const [pickerLoading, setPickerLoading] = useState(false);
  const [processIcons, setProcessIcons] = useState<Record<string, string>>({});
  const iconsRequestedRef = useRef(false);

  const mergeIcons = (list: RunningProcessInfo[]) => {
    setProcessIcons((prev) => {
      const next = { ...prev };
      for (const proc of list) {
        if (proc.icon_data_url) next[proc.name.toLowerCase()] = proc.icon_data_url;
      }
      return next;
    });
  };

  useEffect(() => {
    if (iconsRequestedRef.current || hyperState.auto_pause_processes.length === 0) return;
    iconsRequestedRef.current = true;
    void xtoolsApi
      .listRunningProcesses()
      .then(mergeIcons)
      .catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hyperState.auto_pause_processes.length]);

  const matchedLower = new Set(
    hyperState.matched_processes.map((p) => p.toLowerCase())
  );
  const watchedLower = new Set(
    hyperState.auto_pause_processes.map((p) => p.toLowerCase())
  );
  const filteredRunning = runningProcesses.filter((p) =>
    p.name.toLowerCase().includes(pickerQuery.trim().toLowerCase())
  );

  const reportError = (err: unknown) => {
    setNotice({ kind: 'warning', text: String(err) });
  };

  const handleSetAutoPauseEnabled = async (enabled: boolean) => {
    try {
      onUpdateHyperState(await xtoolsApi.setHyperAutoPauseEnabled(enabled));
    } catch (err) {
      reportError(err);
    }
  };

  const handleSetAutoPauseSound = async (enabled: boolean) => {
    try {
      onUpdateHyperState(await xtoolsApi.setHyperAutoPauseNotifySound(enabled));
    } catch (err) {
      reportError(err);
    }
  };

  const handleAddProcess = async (name: string): Promise<boolean> => {
    if (!name.trim()) return false;
    try {
      onUpdateHyperState(await xtoolsApi.addHyperAutoPauseProcess(name));
      setNotice(null);
      return true;
    } catch (err) {
      reportError(err);
      return false;
    }
  };

  const handleSubmitProcessInput = async () => {
    if (await handleAddProcess(processInput)) {
      setProcessInput('');
    }
  };

  const handleRemoveProcess = async (name: string) => {
    try {
      onUpdateHyperState(await xtoolsApi.removeHyperAutoPauseProcess(name));
    } catch (err) {
      reportError(err);
    }
  };

  const loadRunningProcesses = async () => {
    setPickerLoading(true);
    try {
      const list = await xtoolsApi.listRunningProcesses();
      setRunningProcesses(list);
      mergeIcons(list);
    } catch (err) {
      reportError(err);
    } finally {
      setPickerLoading(false);
    }
  };

  const openPicker = () => {
    setPickerQuery('');
    setPickerOpen(true);
    void loadRunningProcesses();
  };
  const sortedMappings = [...hyperState.mappings].sort((a, b) =>
    triggerSortKey(a.trigger).localeCompare(triggerSortKey(b.trigger))
  );

  const groupedMappings = ACTION_GROUPS.map((kind) => ({
    kind,
    label: i18n.t(`hyper.group.${kind}`),
    items: sortedMappings.filter((m) => m.action.kind === kind),
  })).filter((g) => g.items.length > 0);

  const triggerNeedsKey = triggerSel === 'plain' || triggerSel === 'with_shift';

  const resetDraft = () => {
    setTriggerSel('plain');
    setTriggerKey(null);
    setActionKind('directional');
    setDirAction('left');
    setJumpDir('up');
    setJumpCount(10);
    setIndAction('backspace');
    setCommandText('');
    setComboKey(null);
    setComboCtrl(false);
    setComboAlt(false);
    setComboWin(false);
    setComboShift(false);
  };

  const openAddModal = () => {
    resetDraft();
    setEditingTrigger(null);
    setModalOpen(true);
  };

  const openEditModal = (entry: HyperActionMappingEntry) => {
    resetDraft();
    const tr = entry.trigger;
    if (tr.kind === 'single_tap_hyper') {
      setTriggerSel('single_tap');
    } else if (tr.kind === 'double_tap_hyper') {
      setTriggerSel('double_tap');
    } else if (tr.kind === 'double_tap_modifier') {
      setTriggerSel(`dtm:${tr.modifier}`);
    } else {
      setTriggerSel(tr.with_shift ? 'with_shift' : 'plain');
      setTriggerKey(tr.key);
    }

    const act = entry.action;
    setActionKind(act.kind);
    switch (act.kind) {
      case 'directional':
        setDirAction(act.action);
        break;
      case 'jump':
        setJumpDir(act.direction);
        setJumpCount(act.count);
        break;
      case 'independent':
        setIndAction(act.action);
        break;
      case 'command':
        setCommandText(act.command);
        break;
      case 'key_combo':
        setComboKey(act.target_key);
        setComboCtrl(act.with_ctrl);
        setComboAlt(act.with_alt);
        setComboWin(act.with_win);
        setComboShift(act.with_shift);
        break;
    }

    setEditingTrigger(tr);
    setModalOpen(true);
  };

  const buildDraftTrigger = (): HyperTrigger | null => {
    if (triggerSel === 'single_tap') return { kind: 'single_tap_hyper' };
    if (triggerSel === 'double_tap') return { kind: 'double_tap_hyper' };
    if (triggerSel.startsWith('dtm:')) {
      return {
        kind: 'double_tap_modifier',
        modifier: triggerSel.slice(4) as HyperModifierKey,
      };
    }
    if (!triggerKey) return null;
    return {
      kind: 'hyper_plus_key',
      key: triggerKey,
      with_shift: triggerSel === 'with_shift',
    };
  };

  const buildDraftAction = (): HyperActionConfig | null => {
    switch (actionKind) {
      case 'directional':
        return { kind: 'directional', action: dirAction };
      case 'jump':
        return {
          kind: 'jump',
          direction: jumpDir,
          count: Math.max(1, Math.min(99, Number(jumpCount) || 1)),
        };
      case 'independent':
        return { kind: 'independent', action: indAction };
      case 'command':
        return commandText.trim()
          ? { kind: 'command', command: commandText.trim() }
          : null;
      case 'key_combo':
        return comboKey
          ? {
              kind: 'key_combo',
              target_key: comboKey,
              with_ctrl: comboCtrl,
              with_alt: comboAlt,
              with_win: comboWin,
              with_shift: comboShift,
            }
          : null;
    }
  };

  const draftTrigger = buildDraftTrigger();
  const draftAction = buildDraftAction();
  const canSave = Boolean(draftTrigger && draftAction);

  const saveDraft = async () => {
    if (!draftTrigger || !draftAction) return;
    try {
      const next = await xtoolsApi.upsertHyperMapping(draftTrigger, draftAction);
      onUpdateHyperState(next);
      setModalOpen(false);
      setEditingTrigger(null);
      setNotice(null);
    } catch (err) {
      setNotice({ kind: 'warning', text: String(err) });
    }
  };

  const handleTogglePaused = async () => {
    const next = await xtoolsApi.setHyperPaused(!hyperState.paused);
    onUpdateHyperState(next);
  };

  const handleExport = async () => {
    try {
      const json = await xtoolsApi.exportHyperMappings();
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = 'hypercapslock-mappings.json';
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      setNotice({ kind: 'warning', text: String(err) });
    }
  };

  const handleImportFile = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    try {
      const content = await file.text();
      const next = await xtoolsApi.importHyperMappings(content);
      onUpdateHyperState(next);
      setNotice({
        kind: 'success',
        text: i18n.t('hyper.importSuccess', { count: next.mappings.length }),
      });
    } catch (err) {
      setNotice({
        kind: 'warning',
        text: i18n.t('hyper.importFailed', { error: String(err) }),
      });
    }
  };

  const handleResetDefaults = async () => {
    const next = await xtoolsApi.resetHyperDefaults();
    onUpdateHyperState(next);
    setNotice(null);
  };

  return (
    <div className="mx-auto max-w-[680px] space-y-5 px-6 py-5">
      {notice && <Banner kind={notice.kind} message={notice.text} />}
      {hyperState.auto_paused && (
        <Banner
          kind="warning"
          message={i18n.t('hyper.status.autoPausedBy', {
            processes: hyperState.matched_processes.join(', '),
          })}
        />
      )}

      {/* Service control */}
      <FormSection header={i18n.t('hyper.section.service')}>
        <FormRow
          leading={
            <FeatureLabel
              color="#0A84FF"
              icon={<Command size={14} strokeWidth={2.1} />}
              title={
                <span className="inline-flex items-center gap-2">
                  <span>HyperCapslock</span>
                  <StatusDot active={!hyperState.paused} />
                  <span
                    className="text-[11.5px] font-medium"
                    style={{ color: hyperState.paused ? '#FF9F0A' : '#30D158' }}
                  >
                    {i18n.t(
                      hyperState.auto_paused
                        ? 'hyper.status.autoPaused'
                        : hyperState.paused
                          ? 'hyper.status.paused'
                          : 'hyper.status.running'
                    )}
                  </span>
                </span>
              }
              subtitle={i18n.t('hyper.status.subtitle')}
            />
          }
          trailing={
            <button
              type="button"
              onClick={() => void handleTogglePaused()}
              className={`inline-flex h-[30px] cursor-pointer items-center gap-1.5 rounded-[8px] border px-3 text-[12px] font-medium transition-colors ${
                hyperState.paused
                  ? 'border-[#0A84FF]/60 bg-[#0A84FF] text-white hover:bg-[#1a8eff]'
                  : 'border-white/[0.12] bg-white/[0.08] text-white/[0.9] hover:bg-white/[0.13]'
              }`}
            >
              {hyperState.paused ? <Play size={13} /> : <Pause size={13} />}
              <span>
                {i18n.t(hyperState.paused ? 'hyper.action.resume' : 'hyper.action.pause')}
              </span>
            </button>
          }
        />

        <div className="flex flex-wrap items-center justify-between gap-2 px-3.5 py-2.5">
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="inline-flex h-[28px] cursor-pointer items-center gap-1.5 rounded-[7px] border border-white/[0.12] bg-white/[0.06] px-2.5 text-[11.5px] font-medium text-white/[0.85] transition-colors hover:bg-white/[0.12]"
            >
              <Upload size={12} />
              <span>{i18n.t('hyper.action.import')}</span>
            </button>
            <input
              ref={fileInputRef}
              type="file"
              accept=".json,application/json"
              className="hidden"
              onChange={(e) => void handleImportFile(e)}
            />

            <button
              type="button"
              onClick={() => void handleExport()}
              className="inline-flex h-[28px] cursor-pointer items-center gap-1.5 rounded-[7px] border border-white/[0.12] bg-white/[0.06] px-2.5 text-[11.5px] font-medium text-white/[0.85] transition-colors hover:bg-white/[0.12]"
            >
              <Download size={12} />
              <span>{i18n.t('hyper.action.export')}</span>
            </button>

            <button
              type="button"
              onClick={() => void handleResetDefaults()}
              className="inline-flex h-[28px] cursor-pointer items-center gap-1.5 rounded-[7px] border border-white/[0.12] bg-white/[0.06] px-2.5 text-[11.5px] font-medium text-white/[0.85] transition-colors hover:bg-white/[0.12]"
            >
              <RotateCcw size={12} />
              <span>{i18n.t('hyper.action.reset')}</span>
            </button>
          </div>

          <button
            type="button"
            onClick={openAddModal}
            className="inline-flex h-[28px] cursor-pointer items-center gap-1.5 rounded-[7px] border border-[#0A84FF]/60 bg-[#0A84FF] px-3 text-[12px] font-medium text-white transition-colors hover:bg-[#1a8eff]"
          >
            <Plus size={13} />
            <span>{i18n.t('hyper.action.add')}</span>
          </button>
        </div>
      </FormSection>

      {/* Auto-pause on processes */}
      <FormSection
        header={i18n.t('hyper.autoPause.section')}
        footer={i18n.t('hyper.autoPause.footer')}
      >
        <FormRow
          leading={
            <FeatureLabel
              color="#FF9F0A"
              icon={<ShieldAlert size={14} strokeWidth={2.1} />}
              title={i18n.t('hyper.autoPause.enable.title')}
              subtitle={i18n.t('hyper.autoPause.enable.subtitle')}
            />
          }
          trailing={
            <ToggleSwitch
              checked={hyperState.auto_pause_enabled}
              onChange={(enabled) => void handleSetAutoPauseEnabled(enabled)}
            />
          }
        />

        <FormRow
          leading={
            <IconLabel
              color="#FF2D55"
              icon={<Bell size={14} strokeWidth={2.1} />}
              title={i18n.t('hyper.autoPause.sound.title')}
            />
          }
          trailing={
            <ToggleSwitch
              checked={hyperState.auto_pause_notify_sound}
              onChange={(enabled) => void handleSetAutoPauseSound(enabled)}
            />
          }
        />

        <div className="flex items-center gap-2 px-3.5 py-2.5">
          <input
            type="text"
            value={processInput}
            onChange={(e) => setProcessInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void handleSubmitProcessInput();
            }}
            placeholder={i18n.t('hyper.autoPause.input.placeholder')}
            className="h-[28px] min-w-0 flex-1 rounded-[7px] border border-white/[0.12] bg-[#2c2c34] px-2.5 text-[12px] text-white placeholder:text-white/[0.38] focus:border-[#0A84FF] focus:outline-none"
          />
          <ActionButton
            label={i18n.t('hyper.autoPause.add')}
            variant="primary"
            disabled={!processInput.trim()}
            onClick={() => void handleSubmitProcessInput()}
          />
          <ActionButton
            label={i18n.t('hyper.autoPause.pick')}
            onClick={openPicker}
          />
        </div>

        {hyperState.auto_pause_processes.length === 0 ? (
          <div className="px-4 py-5 text-center text-[12.5px] text-white/[0.48]">
            {i18n.t('hyper.autoPause.empty')}
          </div>
        ) : (
          hyperState.auto_pause_processes.map((name) => (
            <FormRow
              key={name}
              leading={
                <div className="flex items-center gap-2">
                  <ProcessIcon src={processIcons[name.toLowerCase()]} />
                  <span className="font-mono text-[12.5px] text-white/[0.92]">{name}</span>
                  {matchedLower.has(name.toLowerCase()) && (
                    <span className="rounded bg-[#FF9F0A]/15 px-1.5 py-0.5 text-[10.5px] font-medium text-[#FF9F0A]">
                      {i18n.t('hyper.autoPause.running')}
                    </span>
                  )}
                </div>
              }
              trailing={
                <button
                  type="button"
                  onClick={() => void handleRemoveProcess(name)}
                  title={i18n.t('hyper.autoPause.remove')}
                  className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.48] transition-colors hover:bg-[#e81123]/20 hover:text-[#FF6961]"
                >
                  <Trash2 size={13} />
                </button>
              }
            />
          ))
        )}
      </FormSection>

      {/* Running process picker */}
      {pickerOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 backdrop-blur-sm">
          <div className="flex max-h-[80vh] w-full max-w-[440px] flex-col space-y-3 rounded-[14px] border border-white/[0.12] bg-[#1e1e26] p-5 shadow-2xl">
            <div className="flex items-center justify-between">
              <h3 className="text-[14px] font-semibold text-white">
                {i18n.t('hyper.autoPause.picker.title')}
              </h3>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => void loadRunningProcesses()}
                  title={i18n.t('hyper.autoPause.picker.refresh')}
                  className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.55] hover:bg-white/[0.08] hover:text-white"
                >
                  <RefreshCw size={13} />
                </button>
                <button
                  type="button"
                  onClick={() => setPickerOpen(false)}
                  title={i18n.t('hyper.autoPause.picker.close')}
                  className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.55] hover:bg-white/[0.08] hover:text-white"
                >
                  <X size={15} />
                </button>
              </div>
            </div>

            <input
              type="text"
              value={pickerQuery}
              onChange={(e) => setPickerQuery(e.target.value)}
              placeholder={i18n.t('hyper.autoPause.picker.search')}
              className="h-[30px] w-full rounded-[7px] border border-white/[0.12] bg-[#2c2c34] px-3 text-[12px] text-white placeholder:text-white/[0.38] focus:border-[#0A84FF] focus:outline-none"
            />

            <div className="min-h-[120px] flex-1 overflow-y-auto rounded-[9px] border border-white/[0.08] bg-white/[0.03]">
              {pickerLoading && runningProcesses.length === 0 ? (
                <div className="px-4 py-6 text-center text-[12.5px] text-white/[0.48]">
                  {i18n.t('hyper.autoPause.picker.loading')}
                </div>
              ) : filteredRunning.length === 0 ? (
                <div className="px-4 py-6 text-center text-[12.5px] text-white/[0.48]">
                  {i18n.t('hyper.autoPause.picker.empty')}
                </div>
              ) : (
                filteredRunning.map((proc) => {
                  const checked = watchedLower.has(proc.name.toLowerCase());
                  return (
                    <button
                      key={proc.name}
                      type="button"
                      role="checkbox"
                      aria-checked={checked}
                      onClick={() =>
                        void (checked
                          ? handleRemoveProcess(proc.name)
                          : handleAddProcess(proc.name))
                      }
                      className="flex w-full cursor-pointer items-center gap-2.5 px-3 py-1.5 text-left text-[12.5px] text-white/[0.9] transition-colors hover:bg-white/[0.07]"
                    >
                      <span
                        className={`inline-flex h-[16px] w-[16px] shrink-0 items-center justify-center rounded-[4px] border transition-colors ${
                          checked
                            ? 'border-[#0A84FF] bg-[#0A84FF] text-white'
                            : 'border-white/[0.28] bg-white/[0.05]'
                        }`}
                      >
                        {checked && <Check size={11} strokeWidth={3} />}
                      </span>
                      <ProcessIcon src={proc.icon_data_url} />
                      <span className="min-w-0 flex-1 truncate font-mono">{proc.name}</span>
                    </button>
                  );
                })
              )}
            </div>

            <div className="flex items-center justify-between pt-1">
              <span className="text-[11.5px] text-white/[0.52]">
                {i18n.t('hyper.autoPause.picker.selectedCount', {
                  count: hyperState.auto_pause_processes.length,
                })}
              </span>
              <ActionButton
                label={i18n.t('hyper.autoPause.picker.done')}
                variant="primary"
                onClick={() => setPickerOpen(false)}
              />
            </div>
          </div>
        </div>
      )}
      {groupedMappings.length === 0 ? (
        <FormSection header={i18n.t('hyper.section.mappings')}>
          <div className="px-4 py-6 text-center text-[12.5px] text-white/[0.48]">
            {i18n.t('hyper.mappings.empty')}
          </div>
        </FormSection>
      ) : (
        groupedMappings.map((group) => (
          <FormSection key={group.kind} header={group.label}>
            {group.items.map((entry) => (
              <FormRow
                key={triggerUniqueKey(entry.trigger)}
                leading={renderTriggerBadges(entry.trigger, i18n)}
                trailing={
                  <div className="flex items-center gap-2.5">
                    <span className="max-w-[260px] truncate text-[12px] font-medium text-[#60A5FA]">
                      {describeAction(entry.action, i18n)}
                    </span>
                    <button
                      type="button"
                      onClick={() => openEditModal(entry)}
                      title={i18n.t('hyper.modal.editTitle')}
                      className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.48] transition-colors hover:bg-white/[0.08] hover:text-white"
                    >
                      <Pencil size={13} />
                    </button>
                    <button
                      type="button"
                      onClick={() =>
                        void xtoolsApi
                          .removeHyperMapping(entry.trigger)
                          .then(onUpdateHyperState)
                      }
                      title={i18n.t('naga.recorder.clear')}
                      className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.48] transition-colors hover:bg-[#e81123]/20 hover:text-[#FF6961]"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                }
              />
            ))}
          </FormSection>
        ))
      )}

      {/* Add / Edit Modal */}
      {modalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 backdrop-blur-sm">
          <div className="w-full max-w-[440px] space-y-4 rounded-[14px] border border-white/[0.12] bg-[#1e1e26] p-5 shadow-2xl">
            <div className="flex items-center justify-between">
              <h3 className="text-[14px] font-semibold text-white">
                {i18n.t(
                  editingTrigger ? 'hyper.modal.editTitle' : 'hyper.modal.addTitle'
                )}
              </h3>
              <button
                type="button"
                onClick={() => setModalOpen(false)}
                className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.55] hover:bg-white/[0.08] hover:text-white"
              >
                <X size={15} />
              </button>
            </div>

            <div className="space-y-3">
              <div className="space-y-1.5">
                <label className="text-[11.5px] font-medium text-white/[0.6]">
                  {i18n.t('hyper.modal.triggerType')}
                </label>
                <div className="grid grid-cols-[1fr_1fr] gap-2">
                  <SelectControl
                    value={triggerSel}
                    disabled={Boolean(editingTrigger)}
                    options={[
                      { value: 'plain', label: i18n.t('hyper.trigger.caps') },
                      { value: 'with_shift', label: i18n.t('hyper.trigger.caps_shift') },
                      {
                        value: 'single_tap',
                        label: i18n.t('hyper.trigger.single_tap'),
                      },
                      {
                        value: 'double_tap',
                        label: i18n.t('hyper.trigger.double_tap'),
                      },
                      ...MODIFIER_KEYS.map((mod) => ({
                        value: `dtm:${mod}` as TriggerSelection,
                        label: i18n.t(`hyper.mod.${mod}`),
                      })),
                    ]}
                    onChange={(val) => setTriggerSel(val)}
                  />
                  <input
                    type="text"
                    readOnly
                    disabled={!triggerNeedsKey || Boolean(editingTrigger)}
                    placeholder={
                      triggerNeedsKey ? i18n.t('hyper.modal.triggerKey') : '—'
                    }
                    value={triggerNeedsKey && triggerKey ? i18n.vkName(triggerKey) : ''}
                    onKeyDown={(e) => {
                      e.preventDefault();
                      if (IGNORED_CAPTURE_KEYS.has(e.key)) return;
                      if (e.keyCode) setTriggerKey(e.keyCode);
                    }}
                    className="h-[28px] cursor-pointer rounded-[7px] border border-white/[0.12] bg-[#2c2c34] px-2.5 text-center text-[12px] font-medium text-white placeholder:text-white/[0.4] focus:border-[#0A84FF] focus:outline-none disabled:cursor-not-allowed disabled:opacity-45"
                  />
                </div>
              </div>

              <div className="space-y-1.5">
                <label className="text-[11.5px] font-medium text-white/[0.6]">
                  {i18n.t('hyper.modal.actionType')}
                </label>
                <SelectControl
                  value={actionKind}
                  options={ACTION_GROUPS.map((kind) => ({
                    value: kind,
                    label: i18n.t(`hyper.group.${kind}`),
                  }))}
                  onChange={(val) => setActionKind(val)}
                />

                {actionKind === 'directional' && (
                  <SelectControl
                    value={dirAction}
                    options={[
                      { value: 'left', label: i18n.t('hyper.action.left') },
                      { value: 'right', label: i18n.t('hyper.action.right') },
                      { value: 'up', label: i18n.t('hyper.action.up') },
                      { value: 'down', label: i18n.t('hyper.action.down') },
                      {
                        value: 'word_forward',
                        label: i18n.t('hyper.action.word_forward'),
                      },
                      { value: 'word_back', label: i18n.t('hyper.action.word_back') },
                      { value: 'home', label: i18n.t('hyper.action.home') },
                      { value: 'end', label: i18n.t('hyper.action.end') },
                    ]}
                    onChange={(val) => setDirAction(val)}
                  />
                )}

                {actionKind === 'jump' && (
                  <div className="flex items-center gap-2">
                    <SelectControl
                      value={jumpDir}
                      options={[
                        { value: 'up', label: i18n.t('hyper.action.up') },
                        { value: 'down', label: i18n.t('hyper.action.down') },
                      ]}
                      onChange={(val) => setJumpDir(val)}
                    />
                    <div className="flex items-center gap-1.5">
                      <span className="text-[12px] text-white/[0.6]">
                        {i18n.t('hyper.modal.jumpCount')}
                      </span>
                      <input
                        type="number"
                        min={1}
                        max={99}
                        value={jumpCount}
                        onChange={(e) => setJumpCount(Number(e.target.value))}
                        className="h-[28px] w-20 rounded-[7px] border border-white/[0.12] bg-[#2c2c34] px-2 text-center text-[12px] text-white focus:border-[#0A84FF] focus:outline-none"
                      />
                    </div>
                  </div>
                )}

                {actionKind === 'independent' && (
                  <SelectControl
                    value={indAction}
                    options={[
                      { value: 'backspace', label: i18n.t('hyper.action.backspace') },
                      { value: 'next_line', label: i18n.t('hyper.action.next_line') },
                      {
                        value: 'insert_quotes',
                        label: i18n.t('hyper.action.insert_quotes'),
                      },
                      {
                        value: 'toggle_caps_lock',
                        label: i18n.t('hyper.action.toggle_caps_lock'),
                      },
                      {
                        value: 'switch_input_source',
                        label: i18n.t('hyper.action.switch_input_source'),
                      },
                    ]}
                    onChange={(val) => setIndAction(val)}
                  />
                )}

                {actionKind === 'command' && (
                  <input
                    type="text"
                    value={commandText}
                    onChange={(e) => setCommandText(e.target.value)}
                    placeholder={i18n.t('hyper.modal.commandPlaceholder')}
                    className="h-[32px] w-full rounded-[7px] border border-white/[0.12] bg-[#2c2c34] px-3 text-[12px] text-white placeholder:text-white/[0.38] focus:border-[#0A84FF] focus:outline-none"
                  />
                )}

                {actionKind === 'key_combo' && (
                  <div className="space-y-2">
                    <input
                      type="text"
                      readOnly
                      placeholder={i18n.t('hyper.modal.targetKey')}
                      value={comboKey ? i18n.vkName(comboKey) : ''}
                      onKeyDown={(e) => {
                        e.preventDefault();
                        if (IGNORED_CAPTURE_KEYS.has(e.key)) return;
                        if (e.keyCode) setComboKey(e.keyCode);
                      }}
                      className="h-[32px] w-full cursor-pointer rounded-[7px] border border-white/[0.12] bg-[#2c2c34] px-3 text-center text-[12px] font-medium text-white placeholder:text-white/[0.4] focus:border-[#0A84FF] focus:outline-none"
                    />
                    <div className="grid grid-cols-4 gap-2">
                      {(
                        [
                          { label: 'Ctrl', active: comboCtrl, toggle: setComboCtrl },
                          { label: 'Alt', active: comboAlt, toggle: setComboAlt },
                          { label: 'Shift', active: comboShift, toggle: setComboShift },
                          { label: 'Win', active: comboWin, toggle: setComboWin },
                        ] as const
                      ).map((mod) => (
                        <button
                          key={mod.label}
                          type="button"
                          onClick={() => mod.toggle(!mod.active)}
                          className={`h-[28px] cursor-pointer rounded-[7px] border text-[11.5px] font-medium transition-colors ${
                            mod.active
                              ? 'border-[#0A84FF] bg-[#0A84FF]/25 text-[#60A5FA]'
                              : 'border-white/[0.12] bg-white/[0.05] text-white/[0.6] hover:bg-white/[0.1]'
                          }`}
                        >
                          {mod.label}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-2">
              <ActionButton
                label={i18n.t('hyper.modal.cancel')}
                onClick={() => setModalOpen(false)}
              />
              <ActionButton
                label={i18n.t('hyper.modal.save')}
                variant="primary"
                disabled={!canSave}
                onClick={() => void saveDraft()}
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
