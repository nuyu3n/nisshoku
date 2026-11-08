// Date and time pickers drawn by the app (the webview's own pickers look
// different on every platform and are clumsy on phones). Shown inside the
// dialogs: see `dialogs.date` and `dialogs.time`.

import { useState } from "react";
import { lang } from "../i18n";
import { useStore } from "../store";
import { isoDate } from "../api";
import { Icon } from "./Icon";
import { useSlideDir } from "../lib/motion";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);
const pad = (n: number) => String(n).padStart(2, "0");

/** A calendar for `value` (YYYY-MM-DD); days after `max` cannot be chosen. */
export function DatePicker({ value, max, onChange, onPick }: { value: string; max?: string; onChange: (v: string) => void; onPick: (v: string) => void }) {
  const weekStart = useStore((s) => (s.prefs.weekStart === "mon" ? 1 : 0));
  const [view, setView] = useState(() => ({ year: Number(value.slice(0, 4)), month: Number(value.slice(5, 7)) - 1 }));
  const viewDir = useSlideDir(view.year * 12 + view.month);
  const [mode, setMode] = useState<"days" | "months">("days");
  const today = isoDate();
  const en = lang() === "en";
  const shift = (months: number) => {
    const d = new Date(view.year, view.month + months, 1);
    setView({ year: d.getFullYear(), month: d.getMonth() });
  };
  const monthKey = (y: number, m: number) => `${y}-${pad(m + 1)}`;
  const afterMax = (key: string) => !!max && key > max.slice(0, key.length);

  if (mode === "months") {
    return (
      <div className="calendar picker">
        <div className="calendar-head">
          <button className="icon-btn" title={L("前の年", "Previous year")} onClick={() => setView({ ...view, year: view.year - 1 })}>
            <Icon name="chevronLeft" />
          </button>
          <button className="calendar-title" onClick={() => setMode("days")}>
            {en ? view.year : `${view.year}年`}
          </button>
          <button className="icon-btn" title={L("次の年", "Next year")} disabled={afterMax(String(view.year + 1))} onClick={() => setView({ ...view, year: view.year + 1 })}>
            <Icon name="chevronRight" />
          </button>
        </div>
        <div className="picker-months">
          {Array.from({ length: 12 }, (_, m) => (
            <button
              key={m}
              className={`picker-cell${monthKey(view.year, m) === value.slice(0, 7) ? " on" : ""}${monthKey(view.year, m) === today.slice(0, 7) ? " now" : ""}`}
              disabled={afterMax(monthKey(view.year, m))}
              onClick={() => {
                setView({ ...view, month: m });
                setMode("days");
              }}
            >
              {en ? new Date(2000, m, 1).toLocaleDateString("en-US", { month: "short" }) : `${m + 1}月`}
            </button>
          ))}
        </div>
      </div>
    );
  }

  const key = monthKey(view.year, view.month);
  const next = new Date(view.year, view.month + 1, 1);
  const first = (new Date(view.year, view.month, 1).getDay() - weekStart + 7) % 7;
  const count = new Date(view.year, view.month + 1, 0).getDate();
  const cells: (number | null)[] = [...Array(first).fill(null), ...Array.from({ length: count }, (_, i) => i + 1)];
  const names = en ? ["S", "M", "T", "W", "T", "F", "S"] : ["日", "月", "火", "水", "木", "金", "土"];
  const dow = [...names.slice(weekStart), ...names.slice(0, weekStart)];
  return (
    <div className="calendar picker">
      <div className="calendar-head">
        <button className="icon-btn" title={L("前の月", "Previous month")} onClick={() => shift(-1)}>
          <Icon name="chevronLeft" />
        </button>
        <button className="calendar-title" title={L("月を選ぶ", "Choose a month")} onClick={() => setMode("months")}>
          {en ? new Date(view.year, view.month, 1).toLocaleDateString("en-US", { year: "numeric", month: "long" }) : `${view.year}年${view.month + 1}月`}
        </button>
        <button className="icon-btn" title={L("次の月", "Next month")} disabled={afterMax(monthKey(next.getFullYear(), next.getMonth()))} onClick={() => shift(1)}>
          <Icon name="chevronRight" />
        </button>
      </div>
      <div className="calendar-grid tab-body" key={key} data-dir={viewDir}>
        {dow.map((d, i) => (
          <div key={i} className={`dow${(i + weekStart) % 7 === 0 ? " sun" : ""}`}>
            {d}
          </div>
        ))}
        {cells.map((day, i) => {
          if (day === null) return <span key={i} className="day out" />;
          const date = `${key}-${pad(day)}`;
          return (
            <button
              key={i}
              className={`day${date === today ? " today" : ""}${date === value ? " selected" : ""}`}
              disabled={afterMax(date)}
              onClick={() => onChange(date)}
              onDoubleClick={() => onPick(date)}
            >
              {day}
            </button>
          );
        })}
      </div>
      <div className="picker-foot">
        <button
          className="btn"
          onClick={() => {
            onChange(today);
            setView({ year: Number(today.slice(0, 4)), month: Number(today.slice(5, 7)) - 1 });
          }}
        >
          {L("今日", "Today")}
        </button>
      </div>
    </div>
  );
}

/** Hours, then minutes (in steps of 5, nudged by one) for `value` (HH:MM). */
export function TimePicker({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [part, setPart] = useState<"hour" | "minute">("hour");
  const [h, m] = value.split(":").map(Number);
  const set = (hour: number, minute: number) => onChange(`${pad((hour + 24) % 24)}:${pad((minute + 60) % 60)}`);
  return (
    <div className="picker time-picker">
      <div className="time-display">
        <button className={part === "hour" ? "on" : ""} onClick={() => setPart("hour")}>
          {pad(h)}
        </button>
        <span>:</span>
        <button className={part === "minute" ? "on" : ""} onClick={() => setPart("minute")}>
          {pad(m)}
        </button>
      </div>
      {part === "hour" ? (
        <div className="picker-hours">
          {Array.from({ length: 24 }, (_, hour) => (
            <button
              key={hour}
              className={`picker-cell${hour === h ? " on" : ""}`}
              onClick={() => {
                set(hour, m);
                setPart("minute");
              }}
            >
              {hour}
            </button>
          ))}
        </div>
      ) : (
        <div className="picker-minutes">
          {Array.from({ length: 12 }, (_, i) => i * 5).map((minute) => (
            <button key={minute} className={`picker-cell${minute === m ? " on" : ""}`} onClick={() => set(h, minute)}>
              {pad(minute)}
            </button>
          ))}
        </div>
      )}
      <div className="picker-foot">
        <button
          className="btn"
          onClick={() => {
            const d = new Date();
            set(d.getHours(), d.getMinutes());
          }}
        >
          {L("今", "Now")}
        </button>
        {part === "minute" && (
          <div className="nudge">
            <button className="btn" title={L("1分前", "1 minute earlier")} onClick={() => set(m === 0 ? h - 1 : h, m - 1)}>
              −1
            </button>
            <button className="btn" title={L("1分後", "1 minute later")} onClick={() => set(m === 59 ? h + 1 : h, m + 1)}>
              +1
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
