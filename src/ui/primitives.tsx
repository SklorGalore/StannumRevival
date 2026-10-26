import { useLayoutEffect, useRef, useState, type ReactNode, type ButtonHTMLAttributes } from "react";
import { OverflowMenu } from "./OverflowMenu";
import { useWidth } from "./useWidth";
import { AlertTriangle, Search } from "lucide-react";

export function Button({
  variant = "secondary",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "ghost" }) {
  const styles = {
    primary: "bg-teal-700 text-white hover:bg-teal-800 shadow-sm",
    secondary: "bg-white text-slate-700 border border-slate-300 hover:bg-slate-50",
    ghost: "text-slate-600 hover:bg-slate-100",
  }[variant];
  return (
    <button
      {...props}
      className={`inline-flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors disabled:opacity-40 disabled:cursor-not-allowed ${styles} ${className}`}
    />
  );
}

export function Panel({
  title,
  eyebrow,
  actions,
  children,
  className = "",
  bodyClassName = "",
}: {
  title?: ReactNode;
  eyebrow?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
}) {
  return (
    <section className={`bg-white border border-slate-200 rounded-lg flex flex-col min-h-0 ${className}`}>
      {(title || actions) && (
        <header className="flex items-center justify-between gap-2 px-3 py-2 border-b border-slate-200">
          <div className="min-w-0">
            {eyebrow && (
              <p className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase">{eyebrow}</p>
            )}
            {title && <h2 className="text-sm font-semibold text-slate-800 truncate">{title}</h2>}
          </div>
          {actions && <div className="flex items-center gap-1.5 shrink-0">{actions}</div>}
        </header>
      )}
      <div className={`flex-1 min-h-0 ${bodyClassName}`}>{children}</div>
    </section>
  );
}

/** Which tabs fit in `room` px, in order, always keeping the current one. */
export function fitTabs(widths: number[], current: number, room: number, more: number): boolean[] {
  const total = widths.reduce((a, b) => a + b, 0);
  if (total <= room) return widths.map(() => true);
  let left = room - more - (widths[current] ?? 0);
  let full = false;
  return widths.map((w, i) => {
    if (i === current) return true;
    if (full || w > left) {
      full = true;
      return false;
    }
    left -= w;
    return true;
  });
}

const tabClass = (on: boolean) =>
  `shrink-0 whitespace-nowrap px-3 py-2 text-xs font-medium border-b-2 -mb-px transition-colors disabled:opacity-40 ${
    on ? "border-teal-600 text-teal-800" : "border-transparent text-slate-500 hover:text-slate-800"
  }`;

function TabCount({ count }: { count?: number }) {
  if (count === undefined) return null;
  return <span className="ml-1.5 rounded-full bg-slate-100 px-1.5 py-px text-[10px] text-slate-600">{count.toLocaleString()}</span>;
}

/**
 * A row of tabs. Tabs that don't fit the pane (side by side, a narrow window)
 * move into a "More" menu at the end instead of spilling over their
 * neighbours; the open tab always stays in the row.
 */
export function Tabs<T extends string>({
  tabs,
  value,
  onChange,
}: {
  tabs: { id: T; label: ReactNode; count?: number; disabled?: boolean }[];
  value: T;
  onChange: (id: T) => void;
}) {
  const [rowRef, width] = useWidth<HTMLDivElement>(100000);
  const measure = useRef<HTMLDivElement>(null);
  const [widths, setWidths] = useState<number[]>([]);
  useLayoutEffect(() => {
    const el = measure.current;
    if (!el) return;
    const next = [...el.children].map((c) => (c as HTMLElement).offsetWidth + 2); // + the gap
    setWidths((old) => (old.length === next.length && old.every((w, i) => w === next[i]) ? old : next));
  });
  const current = Math.max(0, tabs.findIndex((t) => t.id === value));
  // The row's side padding, and room for the More button.
  const fits = widths.length === tabs.length ? fitTabs(widths, current, width - 16, 76) : tabs.map(() => true);
  const hidden = tabs.filter((_, i) => !fits[i]);
  return (
    <div ref={rowRef} role="tablist" className="relative flex items-center gap-0.5 border-b border-slate-200 px-2 bg-white overflow-hidden">
      {tabs.map(
        (t, i) =>
          fits[i] && (
            <button key={t.id} role="tab" aria-selected={t.id === value} disabled={t.disabled} onClick={() => onChange(t.id)} className={tabClass(t.id === value)}>
              {t.label}
              <TabCount count={t.count} />
            </button>
          )
      )}
      {hidden.length > 0 && (
        <OverflowMenu
          label="More"
          showLabel
          items={hidden.map((t) => ({
            id: t.id,
            label: t.label,
            badge: <TabCount count={t.count} />,
            disabled: t.disabled,
            onClick: () => onChange(t.id),
          }))}
        />
      )}
      {/* Every tab at its natural width, out of sight, to decide what fits. */}
      <div aria-hidden className="absolute inset-x-0 top-0 h-0 overflow-hidden invisible pointer-events-none">
        <div ref={measure} className="flex w-max gap-0.5">
          {tabs.map((t) => (
            <span key={t.id} className={`${tabClass(false)} inline-block`}>
              {t.label}
              <TabCount count={t.count} />
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

export function Metric({
  label,
  value,
  detail,
  tone = "neutral",
}: {
  label: string;
  value: ReactNode;
  detail?: ReactNode;
  tone?: "neutral" | "good" | "bad" | "warn";
}) {
  const toneClass = {
    neutral: "text-slate-800",
    good: "text-emerald-700",
    bad: "text-red-700",
    warn: "text-amber-700",
  }[tone];
  return (
    <article className="bg-white border border-slate-200 rounded-lg px-3 py-2.5 min-w-0">
      <p className="text-[10px] font-semibold tracking-wider text-slate-400 uppercase">{label}</p>
      <p className={`text-lg font-semibold tabular-nums truncate ${toneClass}`}>{value}</p>
      {detail && <p className="text-[11px] text-slate-500 truncate">{detail}</p>}
    </article>
  );
}

export function SearchBox({
  value,
  onChange,
  placeholder = "Search…",
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <label className="relative flex items-center">
      <Search size={13} className="absolute left-2 text-slate-400 pointer-events-none" />
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={placeholder}
        className="pl-7 pr-2 py-1.5 text-xs rounded-md border border-slate-300 bg-white w-56 focus:outline-none focus:ring-2 focus:ring-teal-500/40"
      />
    </label>
  );
}

export function ErrorBanner({ children }: { children: ReactNode }) {
  return (
    <div role="alert" className="flex items-start gap-2 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-800">
      <AlertTriangle size={14} className="mt-px shrink-0" />
      <div className="min-w-0 break-words">{children}</div>
    </div>
  );
}

export function Notice({ children }: { children: ReactNode }) {
  return (
    <div role="status" className="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900">
      <AlertTriangle size={14} className="mt-px shrink-0" />
      <div className="min-w-0 break-words">{children}</div>
    </div>
  );
}

export function EmptyState({
  icon,
  title,
  children,
  action,
}: {
  icon?: ReactNode;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="h-full flex items-center justify-center p-8">
      <div className="max-w-md text-center">
        {icon && <div className="mx-auto mb-3 w-12 h-12 rounded-xl bg-teal-50 text-teal-700 flex items-center justify-center">{icon}</div>}
        <h2 className="text-base font-semibold text-slate-800">{title}</h2>
        {children && <div className="mt-1.5 text-sm text-slate-500">{children}</div>}
        {action && <div className="mt-4 flex justify-center gap-2">{action}</div>}
      </div>
    </div>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-[11px] font-medium text-slate-600">
      {label}
      {children}
    </label>
  );
}

export const inputClass =
  "px-2 py-1.5 text-xs rounded-md border border-slate-300 bg-white focus:outline-none focus:ring-2 focus:ring-teal-500/40 disabled:bg-slate-50 disabled:text-slate-400";

export function StatusPill({ status }: { status: "added" | "removed" | "changed" | "unchanged" }) {
  const style = {
    added: "bg-emerald-100 text-emerald-800",
    removed: "bg-red-100 text-red-800",
    changed: "bg-amber-100 text-amber-900",
    unchanged: "bg-slate-100 text-slate-600",
  }[status];
  return (
    <span className={`inline-block rounded px-1.5 py-px text-[10px] font-semibold capitalize ${style}`}>
      {status}
    </span>
  );
}
