// Form controls drawn by the app instead of the system's: a select whose
// list is the app's menu, a number field with its own steppers, and a date
// field that opens the app's calendar. (Checkboxes and scrollbars are styled
// in controls.css; tooltips are in Tooltip.tsx.)

import { lang } from "../i18n";
import { Icon } from "./Icon";
import { openMenu } from "./ContextMenu";
import { dialogs } from "./Dialogs";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

export function Select<T extends string | number>({
  value,
  options,
  onChange,
  disabled,
  className,
  placeholder = "—",
  ariaLabel,
}: {
  value: T;
  options: readonly (readonly [T, string])[];
  onChange: (value: T) => void;
  disabled?: boolean;
  className?: string;
  placeholder?: string;
  ariaLabel?: string;
}) {
  const current = options.find(([v]) => v === value);
  const open = (el: HTMLElement) =>
    openMenu(
      el,
      options.map(([v, label]) => ({ label, checked: v === value, run: () => onChange(v) })),
      { asSelect: true },
    );
  return (
    <button
      type="button"
      className={`input select${className ? ` ${className}` : ""}`}
      disabled={disabled}
      aria-haspopup="menu"
      aria-label={ariaLabel}
      onClick={(e) => open(e.currentTarget)}
      onKeyDown={(e) => {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          open(e.currentTarget);
        }
      }}
    >
      <span className="select-label">{current ? current[1] : placeholder}</span>
      <Icon name="chevronDown" />
    </button>
  );
}

/** A number with − and + beside it (the keyboard's arrows step it too). */
export function NumberField({
  value,
  onChange,
  min = -Infinity,
  max = Infinity,
  step = 1,
  placeholder,
  allowEmpty,
  className,
}: {
  value: number | null;
  onChange: (value: number | null) => void;
  min?: number;
  max?: number;
  step?: number;
  placeholder?: string;
  /** An empty field means `null`. */
  allowEmpty?: boolean;
  className?: string;
}) {
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  const bump = (d: number) => onChange(clamp((value ?? 0) + d * step));
  return (
    <span className={`number-field${className ? ` ${className}` : ""}`}>
      <button type="button" tabIndex={-1} className="number-step" aria-label="−" disabled={value !== null && value <= min} onClick={() => bump(-1)}>
        <Icon name="minus" />
      </button>
      <input
        className="input"
        inputMode="numeric"
        value={value ?? ""}
        placeholder={placeholder}
        onChange={(e) => {
          const raw = e.target.value.replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/[^\d.-]/g, "");
          if (raw === "") {
            if (allowEmpty) onChange(null);
            return;
          }
          const v = Number(raw);
          if (Number.isFinite(v)) onChange(clamp(v));
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            bump(e.key === "ArrowUp" ? 1 : -1);
          }
        }}
      />
      <button type="button" tabIndex={-1} className="number-step" aria-label="+" disabled={value !== null && value >= max} onClick={() => bump(1)}>
        <Icon name="plus" />
      </button>
    </span>
  );
}

/** A day (YYYY-MM-DD), chosen on the app's calendar. */
export function DateField({ value, onChange, title, clearable = true }: { value: string | null; onChange: (value: string | null) => void; title: string; clearable?: boolean }) {
  const label = value
    ? lang() === "en"
      ? new Date(`${value}T12:00:00`).toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" })
      : `${Number(value.slice(0, 4))}年${Number(value.slice(5, 7))}月${Number(value.slice(8, 10))}日`
    : L("日付を選ぶ", "Choose a date");
  const today = new Date();
  const iso = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
  return (
    <span className="date-field">
      <button
        type="button"
        className={`input select${value ? "" : " unset"}`}
        onClick={async () => {
          const picked = await dialogs.date(title, value ?? iso, { max: "9999-12-31" });
          if (picked) onChange(picked);
        }}
      >
        <Icon name="calendar" />
        <span className="select-label">{label}</span>
      </button>
      {clearable && value && (
        <button type="button" className="icon-btn" title={L("日付を消す", "Clear the date")} onClick={() => onChange(null)}>
          <Icon name="x" />
        </button>
      )}
    </span>
  );
}
