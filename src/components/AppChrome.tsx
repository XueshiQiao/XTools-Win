import React from 'react';
import { AlertTriangle, CheckCircle2, ChevronDown } from 'lucide-react';

export function IconTile({
  color,
  children,
}: {
  color: string;
  children: React.ReactNode;
}) {
  return (
    <span
      className="inline-flex h-[26px] w-[26px] shrink-0 items-center justify-center rounded-[7px] border border-white/[0.18] text-white shadow-sm"
      style={{
        background: `linear-gradient(135deg, ${color}, ${color}b8)`,
      }}
    >
      {children}
    </span>
  );
}

export function AssetIconTile({
  src,
  alt,
  color,
}: {
  src: string;
  alt: string;
  color: string;
}) {
  return (
    <IconTile color={color}>
      <img src={src} alt={alt} className="h-[14px] w-[14px] brightness-0 invert" />
    </IconTile>
  );
}

export function SidebarIcon({
  color,
  children,
}: {
  color: string;
  children: React.ReactNode;
}) {
  return (
    <span
      className="inline-flex h-[26px] w-[26px] shrink-0 items-center justify-center rounded-[6px] text-white"
      style={{
        background: `linear-gradient(180deg, ${color}fa, ${color}ad)`,
      }}
    >
      {children}
    </span>
  );
}

export function StatusDot({ active }: { active: boolean }) {
  return (
    <span className="inline-flex h-3 w-3 items-center justify-center">
      <span
        className="h-[9px] w-[9px] rounded-full"
        style={{ backgroundColor: active ? '#30D158' : '#FF9F0A' }}
      />
    </span>
  );
}

export function ToggleSwitch({
  checked,
  onChange,
  disabled = false,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-[20px] w-[36px] shrink-0 cursor-pointer items-center rounded-full transition-colors duration-150 focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0A84FF]/60 disabled:cursor-not-allowed disabled:opacity-50 ${
        checked ? 'bg-[#0A84FF]' : 'bg-white/[0.18]'
      }`}
    >
      <span
        className={`inline-block h-[16px] w-[16px] transform rounded-full bg-white shadow transition-transform duration-150 ${
          checked ? 'translate-x-[18px]' : 'translate-x-[2px]'
        }`}
      />
    </button>
  );
}

export function FormSection({
  header,
  footer,
  children,
}: {
  header?: string;
  footer?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-1.5">
      {header && (
        <h2 className="px-1 text-[12.5px] font-semibold text-white/[0.88]">
          {header}
        </h2>
      )}
      <div className="divide-y divide-white/[0.07] overflow-hidden rounded-[11px] border border-white/[0.08] bg-white/[0.045]">
        {children}
      </div>
      {footer && (
        <p className="px-1 pt-0.5 text-[11.5px] leading-relaxed text-white/[0.46]">
          {footer}
        </p>
      )}
    </section>
  );
}

export function FormRow({
  leading,
  trailing,
  className = '',
}: {
  leading: React.ReactNode;
  trailing?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={`flex min-h-[44px] items-center justify-between gap-4 px-3.5 py-2.5 ${className}`}
    >
      <div className="min-w-0 flex-1">{leading}</div>
      {trailing !== undefined && <div className="shrink-0">{trailing}</div>}
    </div>
  );
}

export function IconLabel({
  color,
  icon,
  title,
}: {
  color: string;
  icon: React.ReactNode;
  title: string;
}) {
  return (
    <div className="flex items-center gap-2.5">
      <IconTile color={color}>{icon}</IconTile>
      <span className="text-[13px] font-medium text-white/[0.92]">{title}</span>
    </div>
  );
}

export function FeatureLabel({
  color,
  icon,
  title,
  subtitle,
}: {
  color: string;
  icon: React.ReactNode;
  title: React.ReactNode;
  subtitle: string;
}) {
  return (
    <div className="flex items-start gap-2.5">
      <div className="mt-0.5">
        <IconTile color={color}>{icon}</IconTile>
      </div>
      <div className="min-w-0 space-y-0.5">
        <div className="text-[13px] font-medium text-white/[0.92]">{title}</div>
        <div className="text-[11.5px] leading-snug text-white/[0.52]">{subtitle}</div>
      </div>
    </div>
  );
}

export function SelectControl<T extends string | number>({
  value,
  options,
  onChange,
  disabled = false,
}: {
  value: T;
  options: Array<{ value: T; label: string }>;
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  return (
    <div className="relative inline-flex items-center">
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => {
          const raw = e.target.value;
          const parsed = (typeof value === 'number' ? Number(raw) : raw) as T;
          onChange(parsed);
        }}
        className="h-[28px] cursor-pointer appearance-none rounded-[7px] border border-white/[0.12] bg-[#2c2c34] pl-2.5 pr-7 text-[12px] font-medium text-white/[0.9] transition-colors hover:border-white/[0.22] focus:border-[#0A84FF] focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
      >
        {options.map((opt) => (
          <option key={String(opt.value)} value={opt.value} className="bg-[#23232a] text-white">
            {opt.label}
          </option>
        ))}
      </select>
      <ChevronDown
        size={13}
        className="pointer-events-none absolute right-2 text-white/[0.55]"
      />
    </div>
  );
}

export function ActionButton({
  label,
  onClick,
  disabled = false,
  variant = 'secondary',
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  variant?: 'primary' | 'secondary';
}) {
  const styles =
    variant === 'primary'
      ? 'border-[#0A84FF]/60 bg-[#0A84FF] text-white hover:bg-[#1a8eff]'
      : 'border-white/[0.12] bg-white/[0.08] text-white/[0.9] hover:bg-white/[0.13]';

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`inline-flex h-[28px] cursor-pointer items-center justify-center rounded-[7px] border px-3 text-[12px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${styles}`}
    >
      {label}
    </button>
  );
}

export function Banner({
  message,
  kind,
}: {
  message: string;
  kind: 'success' | 'warning';
}) {
  const isWarning = kind === 'warning';
  return (
    <div className="flex items-start gap-2.5 rounded-[11px] border border-white/[0.09] bg-white/[0.05] px-3.5 py-2.5">
      {isWarning ? (
        <AlertTriangle size={16} className="mt-0.5 shrink-0 text-[#FF9F0A]" />
      ) : (
        <CheckCircle2 size={16} className="mt-0.5 shrink-0 text-[#30D158]" />
      )}
      <span className="text-[12.5px] leading-snug text-white/[0.9]">{message}</span>
    </div>
  );
}
