import { useMemo, useState } from 'react';
import {
  AlertCircle,
  AlertTriangle,
  Check,
  Cpu,
  Eye,
  HardDrive,
  Info,
  Pencil,
  Tag,
  Usb,
  X,
} from 'lucide-react';
import {
  Banner,
  FeatureLabel,
  FormRow,
  FormSection,
  IconLabel,
  IconTile,
  StatusDot,
  ToggleSwitch,
} from '../../components/AppChrome';
import { xtoolsApi } from '../../core/api';
import { I18nManager } from '../../core/i18n';
import { ToolRenderProps } from '../../core/toolRegistry';
import {
  UsbControllerNode,
  UsbControllerRole,
  UsbDiagnosticHint,
  UsbEventKind,
  UsbInspectorSnapshot,
  UsbPortNode,
  UsbSpeed,
} from '../../core/types';

class UsbViewHelper {
  public static roleColor(role: UsbControllerRole): string {
    switch (role) {
      case 'cpu_direct':
        return '#30B0C7';
      case 'chipset':
        return '#FF9F0A';
      case 'discrete':
        return '#AF52DE';
      default:
        return '#8E8E93';
    }
  }

  public static roleBadgeClasses(role: UsbControllerRole): string {
    switch (role) {
      case 'cpu_direct':
        return 'border-[#30B0C7]/45 bg-[#30B0C7]/15 text-[#64D2FF]';
      case 'chipset':
        return 'border-[#FF9F0A]/40 bg-[#FF9F0A]/15 text-[#FFD60A]';
      case 'discrete':
        return 'border-[#AF52DE]/45 bg-[#AF52DE]/15 text-[#DA8FFF]';
      default:
        return 'border-white/[0.15] bg-white/[0.08] text-white/[0.75]';
    }
  }

  public static capabilityBadgeClasses(speed: UsbSpeed): string {
    switch (speed) {
      case 'super_plus':
        return 'border-[#FF453A]/40 bg-[#FF453A]/15 text-[#FF8982]';
      case 'super':
        return 'border-[#0A84FF]/45 bg-[#0A84FF]/15 text-[#64B5FF]';
      default:
        return 'border-white/[0.14] bg-white/[0.06] text-white/[0.68]';
    }
  }

  public static countConnected(ports: UsbPortNode[]): number {
    let total = 0;
    for (const p of ports) {
      if (p.status === 'connected' && !p.is_hub) {
        total += 1;
      }
      total += this.countConnected(p.hub_children);
    }
    return total;
  }

  public static formatVidPid(vid: number | null, pid: number | null): string | null {
    if (vid === null || pid === null) return null;
    const v = vid.toString(16).toUpperCase().padStart(4, '0');
    const p = pid.toString(16).toUpperCase().padStart(4, '0');
    return `${v}:${p}`;
  }

  public static eventBadgeClasses(kind: UsbEventKind): string {
    switch (kind) {
      case 'arrived':
        return 'border-[#30D158]/40 bg-[#30D158]/15 text-[#30D158]';
      case 'removed':
        return 'border-white/[0.15] bg-white/[0.07] text-white/[0.68]';
      case 'surprise_removed':
        return 'border-[#FF453A]/45 bg-[#FF453A]/15 text-[#FF6961]';
    }
  }
}

interface UsbPortRowProps {
  port: UsbPortNode;
  depth: number;
  i18n: I18nManager;
  showEmpty: boolean;
  editingPortKey: string | null;
  draftLabel: string;
  focusedPortKey: string | null;
  onStartEdit: (port: UsbPortNode) => void;
  onDraftChange: (val: string) => void;
  onCommitEdit: (portKey: string) => void;
  onCancelEdit: () => void;
}

function UsbPortRow({
  port,
  depth,
  i18n,
  showEmpty,
  editingPortKey,
  draftLabel,
  focusedPortKey,
  onStartEdit,
  onDraftChange,
  onCommitEdit,
  onCancelEdit,
}: UsbPortRowProps) {
  const isEditing = editingPortKey === port.port_key;
  const isFocused = focusedPortKey === port.port_key;
  const isConnected = port.status === 'connected';
  const isError = port.status === 'error' || port.problem_code !== null;
  const vidPid = UsbViewHelper.formatVidPid(port.vendor_id, port.product_id);

  const visibleChildren = port.hub_children.filter(
    (c) => showEmpty || c.status !== 'empty' || c.recently_changed
  );

  const metaParts: string[] = [];
  if (vidPid) metaParts.push(vidPid);
  if (port.manufacturer) metaParts.push(port.manufacturer);
  if (port.current_speed) metaParts.push(i18n.t(`usb.speed.${port.current_speed}`));
  if (port.max_power_ma) metaParts.push(`${port.max_power_ma} mA`);

  const rowHighlight = isFocused
    ? 'bg-[#FF9F0A]/12 ring-1 ring-inset ring-[#FF9F0A]/60'
    : port.recently_changed
      ? 'bg-[#0A84FF]/12 ring-1 ring-inset ring-[#0A84FF]/60'
      : '';

  return (
    <>
      <div
        id={`usb-port-${port.port_key}`}
        className={`flex items-start justify-between gap-3 px-3.5 py-2.5 transition-colors ${rowHighlight}`}
        style={depth > 0 ? { paddingLeft: `${14 + depth * 22}px` } : undefined}
      >
        <div className="flex min-w-0 flex-1 items-start gap-2.5">
          {depth > 0 && (
            <span className="mt-1.5 h-2.5 w-2.5 shrink-0 rounded-bl-[4px] border-b border-l border-white/[0.25]" />
          )}

          {/* Port & Capability Pills */}
          <div className="mt-0.5 flex shrink-0 items-center gap-1">
            <span
              title={
                port.companion_port
                  ? i18n.t('usb.port.companionHint', { companion: port.companion_port })
                  : undefined
              }
              className="inline-flex h-[21px] min-w-[54px] items-center justify-center rounded-[5px] border border-white/[0.12] bg-white/[0.06] px-1.5 font-mono text-[11px] font-medium text-white/[0.86]"
            >
              {i18n.t('usb.port.name', { n: port.port_number })}
            </span>
            <span
              className={`inline-flex h-[21px] items-center justify-center rounded-[5px] border px-1.5 font-mono text-[10.5px] font-semibold ${UsbViewHelper.capabilityBadgeClasses(port.max_speed)}`}
            >
              {i18n.t(`usb.cap.${port.max_speed}`)}
            </span>
            {port.is_type_c && (
              <span className="inline-flex h-[21px] items-center rounded-[5px] border border-[#30B0C7]/40 bg-[#30B0C7]/12 px-1.5 font-mono text-[10px] font-semibold text-[#64D2FF]">
                Type-C
              </span>
            )}
          </div>

          {/* Device Info & Inline Custom Label */}
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-1.5">
              <span
                className={`truncate text-[13px] font-medium ${
                  isError
                    ? 'text-[#FF6961]'
                    : isConnected
                      ? 'text-white/[0.94]'
                      : 'text-white/[0.42]'
                }`}
              >
                {port.device_name || i18n.t('usb.port.empty')}
              </span>

              {port.is_hub && (
                <span className="rounded border border-white/[0.14] bg-white/[0.06] px-1.5 py-0.5 text-[10px] font-medium text-white/[0.68]">
                  {i18n.t('usb.port.hubTag')}
                </span>
              )}

              {!isEditing && port.custom_label && (
                <button
                  type="button"
                  onClick={() => onStartEdit(port)}
                  className="inline-flex cursor-pointer items-center gap-1 rounded-[5px] border border-[#30D158]/35 bg-[#30D158]/12 px-1.5 py-0.5 text-[10.5px] font-medium text-[#6EE78E] transition-colors hover:bg-[#30D158]/20"
                >
                  <Tag size={10} />
                  <span>{port.custom_label}</span>
                </button>
              )}

              {!isEditing && !port.custom_label && (
                <button
                  type="button"
                  onClick={() => onStartEdit(port)}
                  title={i18n.t('usb.port.labelAdd')}
                  className="inline-flex cursor-pointer items-center gap-1 rounded-[5px] px-1.5 py-0.5 text-[10.5px] text-white/[0.35] transition-colors hover:bg-white/[0.08] hover:text-white/[0.78]"
                >
                  <Pencil size={10} />
                </button>
              )}
            </div>

            {metaParts.length > 0 && (
              <div className="mt-0.5 truncate font-mono text-[11px] text-white/[0.48]">
                {metaParts.join(' · ')}
              </div>
            )}

            {isEditing && (
              <div className="mt-1.5 flex items-center gap-1.5">
                <input
                  type="text"
                  autoFocus
                  value={draftLabel}
                  placeholder={i18n.t('usb.port.labelPlaceholder')}
                  onChange={(e) => onDraftChange(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      onCommitEdit(port.port_key);
                    } else if (e.key === 'Escape') {
                      e.preventDefault();
                      onCancelEdit();
                    }
                  }}
                  className="h-[26px] min-w-[180px] flex-1 rounded-[6px] border border-[#0A84FF]/60 bg-[#1c1c23] px-2 text-[12px] text-white placeholder:text-white/[0.32] focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => onCommitEdit(port.port_key)}
                  title={i18n.t('usb.port.labelSave')}
                  className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] border border-[#30D158]/45 bg-[#30D158]/20 text-[#30D158] hover:bg-[#30D158]/30"
                >
                  <Check size={13} />
                </button>
                <button
                  type="button"
                  onClick={onCancelEdit}
                  title={i18n.t('usb.port.labelCancel')}
                  className="inline-flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[6px] border border-white/[0.12] bg-white/[0.06] text-white/[0.7] hover:bg-white/[0.12]"
                >
                  <X size={13} />
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Status & Alert Badges */}
        <div className="mt-0.5 flex shrink-0 flex-wrap items-center justify-end gap-1.5">
          {port.recently_changed && (
            <span className="rounded-full border border-[#0A84FF]/55 bg-[#0A84FF]/20 px-2 py-0.5 text-[10.5px] font-semibold text-[#7DC3FF]">
              {i18n.t('usb.port.changed')}
            </span>
          )}
          {port.problem_code !== null && (
            <span className="rounded-full border border-[#FF453A]/50 bg-[#FF453A]/20 px-2 py-0.5 font-mono text-[10.5px] font-semibold text-[#FF6961]">
              {i18n.t('usb.port.errorCode', { code: port.problem_code })}
            </span>
          )}
          {port.disconnect_count_24h > 0 && (
            <span
              className={`rounded-full border px-2 py-0.5 text-[10.5px] font-semibold ${
                port.disconnect_count_24h >= 10
                  ? 'border-[#FF453A]/50 bg-[#FF453A]/20 text-[#FF6961]'
                  : 'border-[#FF9F0A]/45 bg-[#FF9F0A]/15 text-[#FFD60A]'
              }`}
            >
              {i18n.t('usb.port.disconnects24h', { count: port.disconnect_count_24h })}
            </span>
          )}
        </div>
      </div>

      {visibleChildren.map((child) => (
        <UsbPortRow
          key={child.port_key}
          port={child}
          depth={depth + 1}
          i18n={i18n}
          showEmpty={showEmpty}
          editingPortKey={editingPortKey}
          draftLabel={draftLabel}
          focusedPortKey={focusedPortKey}
          onStartEdit={onStartEdit}
          onDraftChange={onDraftChange}
          onCommitEdit={onCommitEdit}
          onCancelEdit={onCancelEdit}
        />
      ))}
    </>
  );
}

interface ControllerCardProps {
  ctrl: UsbControllerNode;
  i18n: I18nManager;
  showEmpty: boolean;
  editingPortKey: string | null;
  draftLabel: string;
  focusedPortKey: string | null;
  onStartEdit: (port: UsbPortNode) => void;
  onDraftChange: (val: string) => void;
  onCommitEdit: (portKey: string) => void;
  onCancelEdit: () => void;
}

function ControllerCard({
  ctrl,
  i18n,
  showEmpty,
  editingPortKey,
  draftLabel,
  focusedPortKey,
  onStartEdit,
  onDraftChange,
  onCommitEdit,
  onCancelEdit,
}: ControllerCardProps) {
  const visiblePorts = ctrl.ports.filter(
    (p) => showEmpty || p.status !== 'empty' || p.recently_changed
  );
  const roleColor = UsbViewHelper.roleColor(ctrl.role);
  const vidPid = UsbViewHelper.formatVidPid(ctrl.vendor_id, ctrl.device_id);

  return (
    <FormSection>
      <FormRow
        className="bg-white/[0.025]"
        leading={
          <div className="flex items-center gap-2.5">
            <IconTile color={roleColor}>
              {ctrl.role === 'cpu_direct' ? (
                <Cpu size={14} strokeWidth={2.1} />
              ) : (
                <HardDrive size={14} strokeWidth={2.1} />
              )}
            </IconTile>
            <div className="min-w-0">
              <div className="truncate text-[13px] font-semibold text-white/[0.94]">
                {ctrl.name}
              </div>
              <div className="flex items-center gap-1.5 font-mono text-[11px] text-white/[0.45]">
                {vidPid && <span>PCI {vidPid}</span>}
                <span>·</span>
                <span>{ctrl.role_reason}</span>
              </div>
            </div>
          </div>
        }
        trailing={
          <span
            className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-[11px] font-semibold ${UsbViewHelper.roleBadgeClasses(ctrl.role)}`}
          >
            {i18n.t(`usb.role.${ctrl.role}`)}
          </span>
        }
      />

      {visiblePorts.length > 0 ? (
        visiblePorts.map((port) => (
          <UsbPortRow
            key={port.port_key}
            port={port}
            depth={0}
            i18n={i18n}
            showEmpty={showEmpty}
            editingPortKey={editingPortKey}
            draftLabel={draftLabel}
            focusedPortKey={focusedPortKey}
            onStartEdit={onStartEdit}
            onDraftChange={onDraftChange}
            onCommitEdit={onCommitEdit}
            onCancelEdit={onCancelEdit}
          />
        ))
      ) : (
        <div className="px-3.5 py-3 text-[12px] text-white/[0.45]">
          {i18n.t('usb.port.noVisiblePorts')}
        </div>
      )}
    </FormSection>
  );
}

export function UsbInspectorView({
  i18n,
  usbState,
  onUpdateUsbState,
}: ToolRenderProps) {
  const [editingPortKey, setEditingPortKey] = useState<string | null>(null);
  const [draftLabel, setDraftLabel] = useState('');
  const [focusedPortKey, setFocusedPortKey] = useState<string | null>(null);

  const connectedCount = useMemo(
    () =>
      usbState.controllers.reduce(
        (sum, ctrl) => sum + UsbViewHelper.countConnected(ctrl.ports),
        0
      ),
    [usbState.controllers]
  );

  const issueCount = usbState.diagnostics.length;

  const focusPort = (portKey: string | null) => {
    if (!portKey) return;
    const el = document.getElementById(`usb-port-${portKey}`);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
      setFocusedPortKey(portKey);
      window.setTimeout(() => {
        setFocusedPortKey((cur) => (cur === portKey ? null : cur));
      }, 2400);
    }
  };

  const handleToggleEmpty = async (show: boolean) => {
    const next: UsbInspectorSnapshot = await xtoolsApi.setUsbShowEmptyPorts(show);
    onUpdateUsbState(next);
  };

  const handleCommitLabel = async (portKey: string) => {
    const next = await xtoolsApi.setUsbPortLabel(portKey, draftLabel);
    onUpdateUsbState(next);
    setEditingPortKey(null);
    setDraftLabel('');
  };

  const renderHintIcon = (hint: UsbDiagnosticHint) => {
    if (hint.severity === 'error') {
      return <AlertCircle size={15} className="mt-0.5 shrink-0 text-[#FF6961]" />;
    }
    if (hint.severity === 'warning') {
      return <AlertTriangle size={15} className="mt-0.5 shrink-0 text-[#FF9F0A]" />;
    }
    return <Info size={15} className="mt-0.5 shrink-0 text-[#64D2FF]" />;
  };

  return (
    <div className="mx-auto max-w-[680px] space-y-5 px-6 py-5">
      {/* Overview & Controls */}
      <FormSection header={i18n.t('usb.section.overview')}>
        <FormRow
          leading={
            <IconLabel
              color="#30B0C7"
              icon={<Usb size={14} strokeWidth={2.1} />}
              title={i18n.t('usb.overview.summary', {
                devices: connectedCount,
                controllers: usbState.controllers.length,
              })}
            />
          }
          trailing={
            <div className="flex items-center gap-2">
              {usbState.is_scanning ? (
                <span className="text-[11.5px] text-white/[0.55]">
                  {i18n.t('usb.overview.scanning')}
                </span>
              ) : usbState.last_scan_time ? (
                <span className="font-mono text-[11px] text-white/[0.45]">
                  {i18n.t('usb.overview.lastScan', { time: usbState.last_scan_time })}
                </span>
              ) : null}
              <div className="flex items-center gap-1.5">
                <StatusDot active={issueCount === 0} />
                <span
                  className="text-[11.5px] font-medium"
                  style={{ color: issueCount === 0 ? '#30D158' : '#FF9F0A' }}
                >
                  {issueCount === 0
                    ? i18n.t('usb.overview.healthy')
                    : i18n.t('usb.overview.issues', { count: issueCount })}
                </span>
              </div>
            </div>
          }
        />

        <FormRow
          leading={
            <FeatureLabel
              color="#5856D6"
              icon={<Eye size={14} strokeWidth={2.1} />}
              title={i18n.t('usb.showEmpty.title')}
              subtitle={i18n.t('usb.showEmpty.subtitle')}
            />
          }
          trailing={
            <ToggleSwitch
              checked={usbState.show_empty_ports}
              onChange={(checked) => void handleToggleEmpty(checked)}
            />
          }
        />
      </FormSection>

      {/* Diagnostics */}
      {usbState.diagnostics.length > 0 ? (
        <FormSection header={i18n.t('usb.section.diagnostics')}>
          {usbState.diagnostics.map((hint, idx) => {
            const title = i18n.isZh ? hint.title_zh : hint.title_en;
            const detail = i18n.isZh ? hint.detail_zh : hint.detail_en;
            return (
              <button
                key={`${hint.port_key || 'diag'}-${idx}`}
                type="button"
                onClick={() => focusPort(hint.port_key)}
                className="flex w-full cursor-pointer items-start gap-2.5 px-3.5 py-2.5 text-left transition-colors hover:bg-white/[0.04]"
              >
                {renderHintIcon(hint)}
                <div className="min-w-0 flex-1 space-y-0.5">
                  <div className="text-[13px] font-medium text-white/[0.92]">{title}</div>
                  <div className="text-[11.5px] leading-snug text-white/[0.55]">
                    {detail}
                  </div>
                </div>
              </button>
            );
          })}
        </FormSection>
      ) : usbState.controllers.length > 0 ? (
        <Banner kind="success" message={i18n.t('usb.healthy.banner')} />
      ) : null}

      {/* Controllers & Ports */}
      {usbState.controllers.length > 0 ? (
        <div className="space-y-4">
          {usbState.controllers.map((ctrl) => (
            <ControllerCard
              key={ctrl.id}
              ctrl={ctrl}
              i18n={i18n}
              showEmpty={usbState.show_empty_ports}
              editingPortKey={editingPortKey}
              draftLabel={draftLabel}
              focusedPortKey={focusedPortKey}
              onStartEdit={(port) => {
                setEditingPortKey(port.port_key);
                setDraftLabel(port.custom_label ?? '');
              }}
              onDraftChange={setDraftLabel}
              onCommitEdit={(portKey) => void handleCommitLabel(portKey)}
              onCancelEdit={() => {
                setEditingPortKey(null);
                setDraftLabel('');
              }}
            />
          ))}
        </div>
      ) : (
        <FormSection>
          <div className="px-4 py-6 text-center text-[12.5px] text-white/[0.5]">
            {i18n.t('usb.empty.controllers')}
          </div>
        </FormSection>
      )}

      {/* Recent Hotplug & Surprise Disconnect Events */}
      <FormSection
        header={i18n.t('usb.section.events')}
        footer={i18n.t('usb.events.footer')}
      >
        {usbState.recent_events.length > 0 ? (
          <div className="max-h-[240px] divide-y divide-white/[0.07] overflow-y-auto">
            {usbState.recent_events.slice(0, 15).map((ev, idx) => (
              <div
                key={`${ev.timestamp}-${ev.instance_id}-${idx}`}
                onClick={() => focusPort(ev.port_key)}
                className={`flex items-center justify-between gap-3 px-3.5 py-2 text-[12px] ${
                  ev.port_key ? 'cursor-pointer hover:bg-white/[0.04]' : ''
                }`}
              >
                <div className="flex min-w-0 flex-1 items-center gap-2.5">
                  <span className="shrink-0 font-mono text-[11px] text-white/[0.48]">
                    {ev.timestamp}
                  </span>
                  <span
                    className={`shrink-0 rounded-full border px-2 py-0.5 text-[10.5px] font-semibold ${UsbViewHelper.eventBadgeClasses(ev.kind)}`}
                  >
                    {i18n.t(`usb.event.${ev.kind}`)}
                  </span>
                  <span className="truncate font-medium text-white/[0.9]">
                    {ev.device_name}
                  </span>
                </div>
                <span className="max-w-[200px] shrink-0 truncate font-mono text-[10.5px] text-white/[0.38]">
                  {ev.instance_id}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <div className="px-3.5 py-3 text-[12px] text-white/[0.48]">
            {i18n.t('usb.events.empty')}
          </div>
        )}
      </FormSection>
    </div>
  );
}
