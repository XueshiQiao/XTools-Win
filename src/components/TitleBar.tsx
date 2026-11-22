import { Minus, PanelLeft, RotateCw, Square, X } from 'lucide-react';
import { xtoolsApi } from '../core/api';
import { I18nManager } from '../core/i18n';

interface TitleBarProps {
  i18n: I18nManager;
  title: string;
  showRefresh: boolean;
  isBusy: boolean;
  onRefresh: () => void;
  onToggleSidebar: () => void;
}

export function TitleBar({
  i18n,
  title,
  showRefresh,
  isBusy,
  onRefresh,
  onToggleSidebar,
}: TitleBarProps) {
  return (
    <header className="flex h-[44px] shrink-0 items-center justify-between border-b border-white/[0.06] pl-3 pr-1.5">
      <div className="flex min-w-0 items-center gap-2">
        <button
          type="button"
          onClick={onToggleSidebar}
          title={i18n.t('nav.toggleSidebar')}
          className="inline-flex h-[28px] w-[28px] cursor-pointer items-center justify-center rounded-[7px] text-white/[0.68] transition-colors hover:bg-white/[0.08] hover:text-white"
        >
          <PanelLeft size={15} />
        </button>
        <h1
          data-tauri-drag-region
          className="truncate text-[13.5px] font-semibold text-white/[0.92]"
        >
          {title}
        </h1>
      </div>

      {/* Draggable spacer */}
      <div data-tauri-drag-region className="h-full flex-1" />

      <div className="flex items-center gap-1">
        {showRefresh && (
          <button
            type="button"
            disabled={isBusy}
            onClick={onRefresh}
            title={i18n.t('rog.action.refresh')}
            className="mr-1 inline-flex h-[28px] w-[28px] cursor-pointer items-center justify-center rounded-[7px] text-white/[0.72] transition-colors hover:bg-white/[0.08] hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
          >
            <RotateCw size={14} className={isBusy ? 'animate-spin' : ''} />
          </button>
        )}

        <button
          type="button"
          onClick={() => void xtoolsApi.minimizeWindow()}
          className="inline-flex h-[30px] w-[34px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.65] transition-colors hover:bg-white/[0.08] hover:text-white"
          aria-label="Minimize"
        >
          <Minus size={14} />
        </button>
        <button
          type="button"
          onClick={() => void xtoolsApi.toggleMaximizeWindow()}
          className="inline-flex h-[30px] w-[34px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.65] transition-colors hover:bg-white/[0.08] hover:text-white"
          aria-label="Maximize"
        >
          <Square size={11} />
        </button>
        <button
          type="button"
          onClick={() => void xtoolsApi.hideToTray()}
          className="inline-flex h-[30px] w-[34px] cursor-pointer items-center justify-center rounded-[6px] text-white/[0.65] transition-colors hover:bg-[#e81123] hover:text-white"
          aria-label="Close to tray"
        >
          <X size={14} />
        </button>
      </div>
    </header>
  );
}
