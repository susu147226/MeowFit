import type { ReactNode } from "react";

/** 五个主分区共用的标题条（规范第七节）。 */
export function RegionTitle({ children, right }: { children: string; right?: ReactNode }) {
  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-border bg-surface-2 px-3 py-1.5">
      <h2 className="text-[12px] font-semibold tracking-wide text-muted">{children}</h2>
      <span className="ml-auto flex items-center gap-2">{right}</span>
    </div>
  );
}

export function EmptyHint({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center px-6 text-center text-[12px] text-faint">
      {children}
    </div>
  );
}

type ButtonVariant = "default" | "primary" | "ghost";

export function Button({
  children,
  onClick,
  disabled,
  variant = "default",
  title,
  type = "button",
}: {
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  variant?: ButtonVariant;
  title?: string;
  type?: "button" | "submit";
}) {
  const base =
    "rounded px-2.5 py-1 text-[12px] transition disabled:cursor-not-allowed disabled:opacity-40";
  const styles: Record<ButtonVariant, string> = {
    default:
      "border border-border bg-surface text-text hover:border-border-strong disabled:hover:border-border",
    primary:
      "border border-accent bg-accent text-accent-contrast hover:bg-accent-hover disabled:hover:bg-accent",
    ghost: "text-muted hover:text-text",
  };
  return (
    <button
      type={type}
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`${base} ${styles[variant]}`}
    >
      {children}
    </button>
  );
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11px] text-muted">{label}</span>
      {children}
      {hint && <span className="text-[11px] text-faint">{hint}</span>}
    </label>
  );
}

export function TextInput({
  value,
  onChange,
  placeholder,
  invalid,
  className = "",
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  invalid?: boolean;
  className?: string;
}) {
  return (
    <input
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      className={`w-full rounded border bg-surface px-2 py-1 font-mono text-[12px] outline-none transition placeholder:font-sans placeholder:text-faint focus:border-accent ${
        invalid ? "border-danger" : "border-border"
      } ${className}`}
    />
  );
}

export function Badge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "ok" | "warn" | "danger" | "info" | "accent";
}) {
  const tones: Record<string, string> = {
    neutral: "bg-surface-3 text-muted",
    ok: "bg-ok-soft text-ok",
    warn: "bg-warn-soft text-warn",
    danger: "bg-danger-soft text-danger",
    info: "bg-info-soft text-info",
    accent: "bg-accent-soft text-accent",
  };
  return (
    <span className={`rounded px-1.5 py-0.5 text-[11px] whitespace-nowrap ${tones[tone]}`}>
      {children}
    </span>
  );
}
