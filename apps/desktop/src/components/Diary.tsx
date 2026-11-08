// The diary: its panel in the note list (calendar, statistics, memo, "on
// this day", the entries by month) and the header, events and plain-text
// page of an entry.

import { useEffect, useMemo, useState } from "react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api, isoDate, type DiaryDay, type DiaryEntry, type DiaryEvent, type DiaryStats, type MemoItem } from "../api";
import { report, useStore } from "../store";
import { lang, useT } from "../i18n";
import { formatNumber } from "../lib/text";
import { Icon } from "./Icon";
import { AddInput } from "./AddInput";
import { isMobile } from "../lib/mobile";
import { useSlideDir } from "../lib/motion";
import { contextMenu, type MenuItem } from "./ContextMenu";
import { dialogs } from "./Dialogs";
import { TextLines, type TextLinesHandle } from "./TextLines";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

// -------------------------------------------------------------------------
// Moods and weather (stored as keys; emoji from earlier versions are read too)

export const MOODS: { key: string; icon: string; color: string; label: [string, string] }[] = [
  { key: "great", icon: "moodGreat", color: "#e0773b", label: ["とても良い", "Great"] },
  { key: "good", icon: "moodGood", color: "#e2a33c", label: ["良い", "Good"] },
  { key: "ok", icon: "moodOk", color: "#a39d92", label: ["ふつう", "Okay"] },
  { key: "low", icon: "moodLow", color: "#6f94b8", label: ["いまいち", "Low"] },
  { key: "bad", icon: "moodBad", color: "#6a6fa8", label: ["つらい", "Bad"] },
];

export const WEATHERS: { key: string; icon: string; label: [string, string] }[] = [
  { key: "sunny", icon: "wSunny", label: ["晴れ", "Sunny"] },
  { key: "partly", icon: "wPartly", label: ["晴れ時々くもり", "Partly cloudy"] },
  { key: "cloudy", icon: "wCloudy", label: ["くもり", "Cloudy"] },
  { key: "rain", icon: "wRain", label: ["雨", "Rain"] },
  { key: "snow", icon: "wSnow", label: ["雪", "Snow"] },
  { key: "storm", icon: "wStorm", label: ["雷雨", "Storm"] },
];

const LEGACY: Record<string, string> = {
  "😄": "great", "🥰": "great", "🙂": "good", "😐": "ok", "😴": "ok", "😔": "low", "😢": "bad", "😠": "bad",
  "☀️": "sunny", "⛅": "partly", "☁️": "cloudy", "🌫️": "cloudy", "☔": "rain", "⛈️": "storm", "❄️": "snow", "🌙": "sunny",
};

export const moodOf = (v: string | null | undefined) => MOODS.find((m) => m.key === (v && (LEGACY[v] ?? v)));
export const weatherOf = (v: string | null | undefined) => WEATHERS.find((w) => w.key === (v && (LEGACY[v] ?? v)));

// -------------------------------------------------------------------------
// Writing prompts

const PROMPTS_JA = [
  "今日いちばん心が動いたことは？",
  "今日、誰とどんな話をした？",
  "今日の自分に点数をつけるなら？ その理由は？",
  "最近気になっていることは？",
  "今日食べたもので印象に残ったものは？",
  "明日の自分に伝えたいことは？",
  "今日見つけた小さな良いことは？",
  "今日うまくいかなかったことと、次にどうするか",
  "今日の景色で覚えておきたいものは？",
  "いま読んでいる本や観ている作品について",
  "今日感謝したいことは？",
  "一年後の自分は今日をどう思い出すだろう？",
];
const PROMPTS_EN = [
  "What moved you most today?",
  "Who did you talk with, and about what?",
  "How would you score today, and why?",
  "What has been on your mind lately?",
  "What did you eat that you remember?",
  "What would you tell yourself tomorrow?",
  "What small good thing did you find today?",
  "What didn’t go well, and what will you try next?",
  "What did you see today that you want to remember?",
  "About the book or show you are into",
  "What are you grateful for today?",
  "How will you remember today a year from now?",
];

export function promptFor(date: string): string {
  const list = lang() === "en" ? PROMPTS_EN : PROMPTS_JA;
  let h = 0;
  for (const c of date) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return list[h % list.length];
}

function shiftDate(date: string, days: number): string {
  const d = new Date(`${date}T12:00:00`);
  d.setDate(d.getDate() + days);
  return isoDate(d);
}

const WEEKDAYS_JA = ["日", "月", "火", "水", "木", "金", "土"];

async function exportPeriod(period: string) {
  const path = await saveDialog({ defaultPath: `diary-${period}.txt`, filters: [{ name: "Text", extensions: ["txt"] }] });
  if (!path) return;
  const n = await api.diaryExport(period, path).catch(report);
  if (n !== undefined) useStore.getState().toast(L(`${n}日分を書き出しました`, n === 1 ? "Exported 1 entry" : `Exported ${n} entries`));
}

// -------------------------------------------------------------------------
// The panel in the note list

function MemoCard() {
  const [items, setItems] = useState<MemoItem[] | null>(null);
  const [draft, setDraft] = useState("");
  const [open, setOpen] = useState(true);
  useEffect(() => {
    api.getDiaryMemo().then((m) => setItems(m.items), report);
  }, []);
  const save = (next: MemoItem[]) => {
    setItems(next);
    void api.setDiaryMemo(next).catch(report);
  };
  if (!items) return null;
  const left = items.filter((i) => !i.done).length;
  return (
    <div className="side-card">
      <button className="side-card-head" onClick={() => setOpen(!open)}>
        <Icon name="memo" />
        <span className="grow">{L("備忘録", "Memo")}</span>
        {left > 0 && <span className="count">{left}</span>}
        <Icon name="chevronDown" className={`fold${open ? "" : " closed"}`} />
      </button>
      {open && (
        <>
          {items.map((item, i) => (
            <div key={i} className={`memo-item${item.done ? " done" : ""}`}>
              <input type="checkbox" checked={item.done} onChange={() => save(items.map((x, j) => (j === i ? { ...x, done: !x.done } : x)))} />
              <span className="memo-text">{item.text}</span>
              <button className="memo-delete" title={L("削除", "Delete")} onClick={() => save(items.filter((_, j) => j !== i))}>
                <Icon name="x" />
              </button>
            </div>
          ))}
          <AddInput
            className="memo-add"
            inputClassName="input memo-input"
            placeholder={L("覚えておくことを追加", "Add something to remember")}
            buttonLabel={L("追加", "Add")}
            value={draft}
            onChange={setDraft}
            onAdd={(text) => {
              save([...items, { text, done: false }]);
              setDraft("");
            }}
          />
          {items.some((i) => i.done) && (
            <button className="link-btn" onClick={() => save(items.filter((i) => !i.done))}>
              {L("済んだものを消す", "Clear done")}
            </button>
          )}
        </>
      )}
    </div>
  );
}

export function DiaryPanel() {
  const t = useT();
  const language = useStore((s) => s.prefs.language);
  const weekStart = useStore((s) => (s.prefs.weekStart === "mon" ? 1 : 0));
  const selectedId = useStore((s) => s.selectedId);
  const notesVersion = useStore((s) => s.notes);
  const [month, setMonth] = useState(() => {
    const d = new Date();
    return new Date(d.getFullYear(), d.getMonth(), 1);
  });
  const monthDir = useSlideDir(month.getFullYear() * 12 + month.getMonth());
  const [days, setDays] = useState<DiaryDay[]>([]);
  const [entries, setEntries] = useState<DiaryEntry[]>([]);
  const [stats, setStats] = useState<DiaryStats | null>(null);
  const [past, setPast] = useState<DiaryEntry[]>([]);
  const [query, setQuery] = useState("");
  const key = `${month.getFullYear()}-${String(month.getMonth() + 1).padStart(2, "0")}`;
  const today = isoDate();

  useEffect(() => {
    api.diaryMonth(key).then(setDays).catch(report);
  }, [key, notesVersion]);
  useEffect(() => {
    const timer = window.setTimeout(() => api.diaryEntries(query.trim() || null).then(setEntries).catch(report), query ? 200 : 0);
    return () => window.clearTimeout(timer);
  }, [query, notesVersion]);
  useEffect(() => {
    api.diaryStats(today).then(setStats).catch(report);
    api.onThisDay(today).then(setPast).catch(report);
  }, [notesVersion, today]);

  const byDate = useMemo(() => new Map(days.map((d) => [d.date, d])), [days]);
  const first = (month.getDay() - weekStart + 7) % 7;
  const count = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate();
  const cells: (number | null)[] = [...Array(first).fill(null), ...Array.from({ length: count }, (_, i) => i + 1)];
  const names = language === "en" ? ["S", "M", "T", "W", "T", "F", "S"] : WEEKDAYS_JA;
  const dow = [...names.slice(weekStart), ...names.slice(0, weekStart)];
  const monthLabel = (m: string) => {
    const [y, mo] = m.split("-").map(Number);
    return language === "en" ? new Date(y, mo - 1, 1).toLocaleDateString("en-US", { year: "numeric", month: "long" }) : `${y}年${mo}月`;
  };
  const isCurrentMonth = key === today.slice(0, 7);

  const open = (noteId: string) => useStore.getState().select(noteId);
  const groups = useMemo(() => {
    const out: { month: string; items: DiaryEntry[] }[] = [];
    for (const e of entries) {
      const m = e.date.slice(0, 7);
      if (out.at(-1)?.month !== m) out.push({ month: m, items: [] });
      out.at(-1)!.items.push(e);
    }
    return out;
  }, [entries]);

  return (
    <div className="diary-side">
      <div className="calendar">
        <div className="calendar-head">
          <button className="icon-btn" title={L("前の月", "Previous month")} onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() - 1, 1))}>
            <Icon name="chevronLeft" />
          </button>
          <button
            className="calendar-title"
            title={L("右クリックで書き出し", "Right-click to export")}
            onClick={() => {
              const d = new Date();
              setMonth(new Date(d.getFullYear(), d.getMonth(), 1));
            }}
            {...contextMenu(() => [
              { label: L(`${monthLabel(key)}を書き出す`, `Export ${monthLabel(key)}`), icon: "download", run: () => exportPeriod(key) },
              { label: L(`${month.getFullYear()}年を書き出す`, `Export ${month.getFullYear()}`), icon: "download", run: () => exportPeriod(String(month.getFullYear())) },
            ])}
          >
            {monthLabel(key)}
          </button>
          <button
            className="icon-btn"
            title={L("次の月", "Next month")}
            disabled={isCurrentMonth}
            onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() + 1, 1))}
          >
            <Icon name="chevronRight" />
          </button>
        </div>
        {/* Another month slides in from its side. */}
        <div className="calendar-grid tab-body" key={month.getTime()} data-dir={monthDir}>
          {dow.map((d, i) => (
            <div key={i} className={`dow${(i + weekStart) % 7 === 0 ? " sun" : ""}`}>
              {d}
            </div>
          ))}
          {cells.map((day, i) => {
            if (day === null) return <span key={i} className="day out" />;
            const date = `${key}-${String(day).padStart(2, "0")}`;
            const entry = byDate.get(date);
            const mood = moodOf(entry?.mood);
            const cls = ["day", entry ? "has" : "", date === today ? "today" : "", entry && entry.noteId === selectedId ? "selected" : ""].join(" ");
            return (
              <button
                key={i}
                className={cls}
                style={mood ? { ["--dot" as string]: mood.color, ["--level" as string]: 1 } : undefined}
                title={entry ? `${formatNumber(entry.charCount)} ${t("today.unit")}${mood ? `・${L(...mood.label)}` : ""}` : date > today ? undefined : t("diary.writeThisDay")}
                disabled={!entry && date > today}
                onClick={() => void useStore.getState().openDiary(date)}
              >
                {day}
              </button>
            );
          })}
        </div>
        <button className="btn primary calendar-today" onClick={() => void useStore.getState().openDiary()}>
          <Icon name="pen" />
          {t("diary.writeToday")}
        </button>
      </div>

      {stats && (
        <div className="diary-stats">
          <div>
            <b>{stats.streak}</b>
            <small>{L("連続", "Streak")}</small>
          </div>
          <div>
            <b>{stats.longestStreak}</b>
            <small>{L("最長", "Best")}</small>
          </div>
          <div>
            <b>{stats.monthDays}</b>
            <small>{L("今月", "Month")}</small>
          </div>
          <div>
            <b>{stats.totalDays}</b>
            <small>{L("合計", "Total")}</small>
          </div>
        </div>
      )}

      <MemoCard />

      {past.length > 0 && (
        <div className="side-card accent">
          <div className="side-card-head static">
            <Icon name="history" />
            <span className="grow">{L("過去の今日", "On this day")}</span>
          </div>
          {past.map((e) => (
            <button key={e.noteId} className="diary-entry" onClick={() => void open(e.noteId)}>
              <span className="entry-ago">{L(`${Number(today.slice(0, 4)) - Number(e.date.slice(0, 4))}年前`, `${Number(today.slice(0, 4)) - Number(e.date.slice(0, 4))}y ago`)}</span>
              <span className="entry-text">{e.excerpt || e.title}</span>
            </button>
          ))}
        </div>
      )}

      <div className="search-box diary-search">
        <Icon name="search" />
        <input className="input" placeholder={L("日記と出来事を検索", "Search entries and events")} value={query} onChange={(e) => setQuery(e.target.value)} />
      </div>

      {groups.length === 0 && <div className="empty small">{query ? t("list.notFound") : t("diary.empty")}</div>}
      {groups.map((g) => (
        <div key={g.month} className="diary-group">
          <div className="section-head">
            <span className="grow">{monthLabel(g.month)}</span>
            <span className="count">{g.items.length}</span>
          </div>
          {g.items.map((e) => {
            const d = new Date(`${e.date}T12:00:00`);
            const weekday = language === "en" ? d.toLocaleDateString("en-US", { weekday: "short" }) : WEEKDAYS_JA[d.getDay()];
            const mood = moodOf(e.mood);
            const weather = weatherOf(e.weather);
            return (
              <button
                key={e.noteId}
                className={`diary-entry${e.noteId === selectedId ? " active" : ""}`}
                onClick={() => void open(e.noteId)}
                {...contextMenu((): MenuItem[] => [
                  { label: t("menu.open"), icon: "note", run: () => void open(e.noteId) },
                  { label: t("menu.trash"), icon: "trash", danger: true, run: () => import("../lib/actions").then((a) => a.trashNote(e.noteId)) },
                ])}
              >
                <span className="entry-day" style={mood ? { ["--dot" as string]: mood.color } : undefined}>
                  <b>{d.getDate()}</b>
                  <small className={d.getDay() === 0 ? "sun" : d.getDay() === 6 ? "sat" : undefined}>{weekday}</small>
                </span>
                <span className="entry-body">
                  <span className="entry-text">{e.excerpt || (e.events[0] ? `${e.events[0].time} ${e.events[0].text}` : L("（白紙）", "(empty)"))}</span>
                  {(mood || weather || e.events.length > 0) && (
                    <span className="entry-marks">
                      {mood && (
                        <span className="mark" style={{ color: mood.color }}>
                          <Icon name={mood.icon} />
                        </span>
                      )}
                      {weather && (
                        <span className="mark">
                          <Icon name={weather.icon} />
                        </span>
                      )}
                      {e.events.length > 0 && (
                        <span className="mark text">
                          <Icon name="clock" />
                          {e.events.length}
                        </span>
                      )}
                    </span>
                  )}
                </span>
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}

// -------------------------------------------------------------------------
// The page of an entry

export function formatDiaryDate(date: string): { main: string; sub: string } {
  const d = new Date(`${date}T12:00:00`);
  if (lang() === "en") {
    return { main: d.toLocaleDateString("en-US", { month: "long", day: "numeric" }), sub: d.toLocaleDateString("en-US", { weekday: "long", year: "numeric" }) };
  }
  return { main: `${d.getMonth() + 1}月${d.getDate()}日`, sub: `${d.getFullYear()}年・${WEEKDAYS_JA[d.getDay()]}曜日` };
}

function nowTime() {
  const d = new Date();
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export function DiaryHeader({ noteId, date, readOnly }: { noteId: string; date: string; readOnly: boolean }) {
  const [meta, setMeta] = useState<{ mood: string | null; weather: string | null; events: DiaryEvent[] }>({ mood: null, weather: null, events: [] });
  const [eventText, setEventText] = useState("");
  const [eventTime, setEventTime] = useState(nowTime());
  useEffect(() => {
    api.diaryEntries(null).then((list) => {
      const e = list.find((x) => x.noteId === noteId);
      setMeta({ mood: moodOf(e?.mood)?.key ?? null, weather: weatherOf(e?.weather)?.key ?? null, events: e?.events ?? [] });
    }, report);
  }, [noteId]);

  const saveMeta = async (patch: { mood?: string | null; weather?: string | null }) => {
    const next = { ...meta, ...patch };
    setMeta(next);
    await api.setDiaryMeta(noteId, next.mood, next.weather).catch(report);
    void useStore.getState().loadNotes();
  };
  const saveEvents = async (events: DiaryEvent[]) => {
    const sorted = [...events].sort((a, b) => a.time.localeCompare(b.time));
    setMeta({ ...meta, events: sorted });
    await api.setDiaryEvents(noteId, sorted).catch(report);
    void useStore.getState().loadNotes();
  };
  const addEvent = () => {
    if (!eventText.trim()) return;
    void saveEvents([...meta.events, { time: eventTime || nowTime(), text: eventText.trim() }]);
    setEventText("");
    setEventTime(nowTime());
  };

  const today = isoDate();
  const go = (days: number) => {
    const target = shiftDate(date, days);
    if (target > today) return;
    void useStore.getState().openDiary(target);
  };
  const { main, sub } = formatDiaryDate(date);

  return (
    <div className="diary-head">
      <div className="diary-date-row">
        <button className="icon-btn" title={L("前の日", "Previous day")} onClick={() => go(-1)}>
          <Icon name="chevronLeft" />
        </button>
        <button
          className="diary-date"
          title={L("日付を選んで開く", "Open another day")}
          onClick={async () => {
            const target = await dialogs.date(L("日記を開く日", "Open the diary of"), date, { max: today, okLabel: L("開く", "Open") });
            if (target && target !== date) void useStore.getState().openDiary(target);
          }}
        >
          <span className="diary-date-main">{main}</span>
          <span className="diary-date-sub">
            {sub}
            {date === today && <span className="today-pill">{L("今日", "Today")}</span>}
          </span>
        </button>
        <button className="icon-btn" title={date >= today ? L("明日の日記はまだ書けません", "Tomorrow can’t be written yet") : L("次の日", "Next day")} disabled={date >= today} onClick={() => go(1)}>
          <Icon name="chevronRight" />
        </button>
      </div>

      <div className="diary-meta">
        <div className="meta-row">
          <span className="meta-label">{L("気分", "Mood")}</span>
          <div className="meta-choices">
            {MOODS.map((m) => (
              <button
                key={m.key}
                className={`meta-choice${meta.mood === m.key ? " on" : ""}`}
                style={{ ["--c" as string]: m.color }}
                title={L(...m.label)}
                aria-pressed={meta.mood === m.key}
                disabled={readOnly}
                onClick={() => void saveMeta({ mood: meta.mood === m.key ? null : m.key })}
              >
                <Icon name={m.icon} />
                <span>{L(...m.label)}</span>
              </button>
            ))}
          </div>
        </div>
        <div className="meta-row">
          <span className="meta-label">{L("天気", "Weather")}</span>
          <div className="meta-choices">
            {WEATHERS.map((w) => (
              <button
                key={w.key}
                className={`meta-choice${meta.weather === w.key ? " on" : ""}`}
                title={L(...w.label)}
                aria-pressed={meta.weather === w.key}
                disabled={readOnly}
                onClick={() => void saveMeta({ weather: meta.weather === w.key ? null : w.key })}
              >
                <Icon name={w.icon} />
                <span>{L(...w.label)}</span>
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="diary-events">
        <div className="events-title">
          <Icon name="clock" />
          {L("出来事", "Events")}
        </div>
        {meta.events.map((ev, i) => (
          <div key={i} className="event-row">
            <span className="event-time">{ev.time}</span>
            <span className="event-text">{ev.text}</span>
            {!readOnly && (
              <button className="memo-delete" title={L("削除", "Delete")} onClick={() => void saveEvents(meta.events.filter((_, j) => j !== i))}>
                <Icon name="x" />
              </button>
            )}
          </div>
        ))}
        {!readOnly && (
          <AddInput
            className="event-add"
            placeholder={isMobile ? L("出来事をひとこと", "What happened") : L("出来事をひとこと（Enter で追加）", "What happened (Enter to add)")}
            buttonLabel={L("追加", "Add")}
            value={eventText}
            onChange={setEventText}
            onAdd={() => addEvent()}
            before={
            <button
              type="button"
              className="input event-time-input"
              title={L("時刻を選ぶ", "Choose the time")}
              onClick={async () => {
                const time = await dialogs.time(L("出来事の時刻", "Time of the event"), eventTime || nowTime());
                if (time) setEventTime(time);
              }}
            >
              <Icon name="clock" />
              {eventTime}
            </button>
            }
          />
        )}
      </div>
    </div>
  );
}

/** The page of a diary entry: plain text, no Markdown. */
export function DiaryText({ value, readOnly, date, onChange, lines }: { value: string; readOnly: boolean; date: string; onChange: (v: string) => void; lines?: React.Ref<TextLinesHandle> }) {
  return (
    <TextLines
      ref={lines}
      className="diary-text"
      value={value}
      readOnly={readOnly}
      placeholder={promptFor(date)}
      spellCheck={false}
      minHeight={window.innerHeight * 0.4}
      onChange={onChange}
    />
  );
}
