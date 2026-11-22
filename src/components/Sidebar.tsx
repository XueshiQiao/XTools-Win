import { Info, Settings } from 'lucide-react';
import appLogo from '../assets/logo.png';
import { I18nManager } from '../core/i18n';
import { ToolRegistry } from '../core/toolRegistry';
import { SidebarSelection } from '../core/types';
import { SidebarIcon, StatusDot } from './AppChrome';

interface SidebarProps {
  i18n: I18nManager;
  version: string;
  registry: ToolRegistry;
  selection: SidebarSelection;
  onSelect: (next: SidebarSelection) => void;
}

export function Sidebar({
  i18n,
  version,
  registry,
  selection,
  onSelect,
}: SidebarProps) {
  const groups = registry.getGroups();

  const isSelected = (candidate: SidebarSelection): boolean => {
    if (candidate.kind !== selection.kind) return false;
    if (candidate.kind === 'tool' && selection.kind === 'tool') {
      return candidate.id === selection.id;
    }
    return true;
  };

  const rowClasses = (active: boolean): string =>
    `flex w-full cursor-pointer items-center gap-2.5 rounded-[8px] px-2 py-[5px] text-left text-[13px] transition-colors ${
      active
        ? 'bg-[#0A84FF] font-medium text-white shadow-sm'
        : 'text-white/[0.85] hover:bg-white/[0.06]'
    }`;

  return (
    <aside className="flex h-full w-[220px] shrink-0 flex-col border-r border-white/[0.07] bg-[#18181d]">
      {/* Brand header (draggable empty space) */}
      <div
        data-tauri-drag-region
        className="flex items-center gap-2.5 px-4 pb-3 pt-4"
      >
        <img
          src={appLogo}
          alt="XTools"
          className="pointer-events-none h-[34px] w-[34px] rounded-[8px] object-cover"
        />
        <div className="pointer-events-none min-w-0">
          <div className="text-[14px] font-bold leading-tight text-white">XTools</div>
          <div className="text-[11px] leading-tight text-white/[0.48]">
            v{version || '0.1.0'}
          </div>
        </div>
      </div>

      {/* Scrollable navigation list */}
      <nav className="flex-1 space-y-3 overflow-y-auto px-2 py-1">
        {groups.map((group) => (
          <div key={group.group} className="space-y-0.5">
            {group.items.map((tool) => {
              const target: SidebarSelection = { kind: 'tool', id: tool.id };
              return (
                <button
                  key={tool.id}
                  type="button"
                  onClick={() => onSelect(target)}
                  className={rowClasses(isSelected(target))}
                >
                  <SidebarIcon color={tool.color}>{tool.renderIcon(14)}</SidebarIcon>
                  <span className="truncate">{i18n.t(tool.titleKey)}</span>
                </button>
              );
            })}
          </div>
        ))}

        <div className="mx-2 border-t border-white/[0.07]" />

        <div className="space-y-0.5">
          <button
            type="button"
            onClick={() => onSelect({ kind: 'general' })}
            className={rowClasses(isSelected({ kind: 'general' }))}
          >
            <SidebarIcon color="#8E8E93">
              <Settings size={14} strokeWidth={2.2} />
            </SidebarIcon>
            <span className="truncate">{i18n.t('sidebar.general')}</span>
          </button>

          <button
            type="button"
            onClick={() => onSelect({ kind: 'about' })}
            className={rowClasses(isSelected({ kind: 'about' }))}
          >
            <SidebarIcon color="#FF2D55">
              <Info size={14} strokeWidth={2.2} />
            </SidebarIcon>
            <span className="truncate">{i18n.t('sidebar.about')}</span>
          </button>
        </div>
      </nav>

      {/* Bottom status footer */}
      <div className="flex items-center gap-2 border-t border-white/[0.05] px-4 py-2.5">
        <StatusDot active={true} />
        <span className="text-[11px] text-white/[0.52]">{i18n.t('status.ready')}</span>
      </div>
    </aside>
  );
}
