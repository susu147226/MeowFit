import type { CSSProperties, ReactNode } from "react";

/** 五个主分区共用的面板外壳（规范第七节）。 */
export function Panel({
  title,
  right,
  children,
  className = "",
  width,
  height,
}: {
  title: string;
  right?: ReactNode;
  children: ReactNode;
  className?: string;
  /** 由分隔条拖拽出来的列宽（个性化布局） */
  width?: number;
  /** 由分隔条拖拽出来的高度（个性化布局） */
  height?: number;
}) {
  const style: CSSProperties = {};
  if (width !== undefined) style.width = width;
  if (height !== undefined) style.height = height;
  return (
    <section
      className={`flex min-h-0 flex-col overflow-hidden ${className}`}
      style={Object.keys(style).length > 0 ? style : undefined}
    >
      <div className="panel-head">
        <h2 className="panel-title">{title}</h2>
        <div className="ml-auto flex items-center gap-2">{right}</div>
      </div>
      {children}
    </section>
  );
}

export function EmptyHint({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1 px-8 text-center text-[12px] text-faint">
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
}: {
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  variant?: ButtonVariant;
  title?: string;
}) {
  const variantClass =
    variant === "primary" ? "btn-primary" : variant === "ghost" ? "btn-ghost" : "";
  return (
    <button type="button" title={title} disabled={disabled} onClick={onClick} className={`btn ${variantClass}`}>
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
      {hint && <span className="text-[11px] leading-relaxed text-faint">{hint}</span>}
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
      className={`input ${invalid ? "input-invalid" : ""} ${className}`}
    />
  );
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  className = "",
  title,
}: {
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string }[];
  className?: string;
  title?: string;
}) {
  return (
    <select
      title={title}
      value={value}
      onChange={(e) => onChange(e.target.value as T)}
      className={`select ${className}`}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export function Checkbox({
  checked,
  onChange,
  children,
  title,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children: ReactNode;
  title?: string;
}) {
  return (
    <label
      title={title}
      className="flex cursor-pointer items-center gap-1.5 text-[12px] text-muted transition select-none hover:text-text"
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="accent-accent"
      />
      {children}
    </label>
  );
}

export function Tag({
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
  return <span className={`tag ${tones[tone]}`}>{children}</span>;
}
