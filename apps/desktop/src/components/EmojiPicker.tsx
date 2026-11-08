// Choosing a notebook's icon: every emoji (that systems can show: Unicode
// 15.0 and before), found by name or keyword in the app's language, by
// category, or among the recently used. An emoji typed or pasted into the
// search can be used as it is. (Notebook icons are emoji, as in Joplin.)

import { useEffect, useMemo, useRef, useState } from "react";
import { lang } from "../i18n";
import { Icon } from "./Icon";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

interface Emoji {
  e: string;
  label: string;
  tags: string[];
  group: number;
}

const GROUPS: { id: number; icon: string; ja: string; en: string }[] = [
  { id: 0, icon: "😀", ja: "顔・感情", en: "Smileys" },
  { id: 1, icon: "🧑", ja: "人・体", en: "People" },
  { id: 3, icon: "🐈", ja: "動物・自然", en: "Animals & nature" },
  { id: 4, icon: "🍙", ja: "食べ物・飲み物", en: "Food & drink" },
  { id: 5, icon: "🚃", ja: "旅行・場所", en: "Travel & places" },
  { id: 6, icon: "🎨", ja: "活動", en: "Activities" },
  { id: 7, icon: "📘", ja: "物", en: "Objects" },
  { id: 8, icon: "💠", ja: "記号", en: "Symbols" },
  { id: 9, icon: "🏳️", ja: "旗", en: "Flags" },
];

/** For notebooks: shown first until there are recently used ones. */
const SUGGESTED = "📘📕📗📙📓📔📒📚✍️🖋️📝💡🌙☀️🌸🍁❄️🌊⛰️🏠🚃✈️🎵🎬🎨📷🧪🔬💼🗂️📅⭐❤️🔥🍀🐈🐦🌏🗝️⚓🕯️".match(/\p{Extended_Pictographic}️?/gu) ?? [];

const RECENT_KEY = "nisshoku.recentEmoji";
function recent(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]") as string[];
  } catch {
    return [];
  }
}
function remember(e: string) {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify([e, ...recent().filter((x) => x !== e)].slice(0, 24)));
  } catch {
    /* (a convenience only) */
  }
}

let cache: { language: string; list: Emoji[] } | null = null;
async function load(): Promise<Emoji[]> {
  const language = lang();
  if (cache?.language === language) return cache.list;
  type Raw = { emoji: string; label: string; tags?: string[]; group?: number; version: number };
  const raw: Raw[] = (language === "en" ? await import("emojibase-data/en/data.json") : await import("emojibase-data/ja/data.json")).default as Raw[];
  const list = raw
    // Not the skin-tone and hair parts on their own; nothing newer than systems show.
    .filter((x) => x.group !== undefined && x.group !== 2 && x.version <= 15)
    .map((x) => ({ e: x.emoji, label: x.label, tags: x.tags ?? [], group: x.group! }));
  cache = { language, list };
  return list;
}

const kana = (s: string) => s.toLowerCase().replace(/[ァ-ヶ]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0x60));

export function EmojiPicker({ current, onPick }: { current: string; onPick: (emoji: string) => void }) {
  const [list, setList] = useState<Emoji[] | null>(cache?.language === lang() ? cache.list : null);
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<number | "top">("top");
  const scroller = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const recents = useMemo(recent, []);

  useEffect(() => {
    if (!list) void load().then(setList);
    requestAnimationFrame(() => input.current?.focus());
  }, [list]);

  const pick = (e: string) => {
    remember(e);
    onPick(e);
  };

  // Search: by name or keyword (hiragana and katakana alike).
  const q = kana(query.trim());
  const typed = query.match(/\p{Extended_Pictographic}(️|‍\p{Extended_Pictographic}|[\u{1F3FB}-\u{1F3FF}])*/u)?.[0];
  const results = useMemo(() => {
    if (!list || !q || typed) return [];
    const words = q.split(/\s+/);
    const score = (x: Emoji) => {
      const label = kana(x.label);
      const tags = x.tags.map(kana);
      if (!words.every((w) => label.includes(w) || tags.some((t) => t.includes(w)))) return -1;
      return (label === q ? 4 : 0) + (label.startsWith(words[0]) ? 2 : 0) + (tags.includes(words[0]) ? 1 : 0);
    };
    return list
      .map((x) => [x, score(x)] as const)
      .filter(([, s]) => s >= 0)
      .sort((a, b) => b[1] - a[1])
      .map(([x]) => x)
      .slice(0, 160);
  }, [list, q, typed]);

  const byGroup = useMemo(() => GROUPS.map((g) => ({ ...g, items: (list ?? []).filter((x) => x.group === g.id) })), [list]);
  const labelOf = useMemo(() => new Map((list ?? []).map((x) => [x.e, x.label])), [list]);

  const jump = (id: number | "top") => {
    setTab(id);
    const el = scroller.current?.querySelector<HTMLElement>(`[data-group="${id}"]`);
    if (el && scroller.current) scroller.current.scrollTop = el.offsetTop - scroller.current.offsetTop;
  };
  // The tab follows the scrolling.
  const onScroll = () => {
    const box = scroller.current;
    if (!box) return;
    let at: number | "top" = "top";
    box.querySelectorAll<HTMLElement>("[data-group]").forEach((el) => {
      if (el.offsetTop - box.offsetTop <= box.scrollTop + 8) at = el.dataset.group === "top" ? "top" : Number(el.dataset.group);
    });
    setTab(at);
  };

  const tile = (e: string, key: string) => (
    <button key={key} type="button" className={`emoji-tile${e === current ? " on" : ""}`} title={labelOf.get(e) ?? e} onClick={() => pick(e)}>
      {e}
    </button>
  );
  const top = recents.length ? recents : SUGGESTED;

  return (
    <div className="emoji-picker">
      <div className="emoji-search">
        <Icon name="search" />
        <input
          ref={input}
          className="input"
          value={query}
          placeholder={L("名前で探す（例：本、ねこ、星）", "Search (book, cat, star…)")}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter" || e.nativeEvent.isComposing) return;
            e.preventDefault();
            const first = typed ?? results[0]?.e;
            if (first) pick(first);
          }}
        />
      </div>
      {!q && (
        <div className="emoji-tabs" role="tablist">
          <button type="button" role="tab" aria-selected={tab === "top"} className={tab === "top" ? "on" : ""} title={recents.length ? L("最近使ったもの", "Recent") : L("おすすめ", "Suggested")} onClick={() => jump("top")}>
            <Icon name={recents.length ? "clock" : "star"} />
          </button>
          {GROUPS.map((g) => (
            <button key={g.id} type="button" role="tab" aria-selected={tab === g.id} className={tab === g.id ? "on" : ""} title={L(g.ja, g.en)} onClick={() => jump(g.id)}>
              {g.icon}
            </button>
          ))}
        </div>
      )}
      <div ref={scroller} className="emoji-scroll" onScroll={onScroll}>
        {typed ? (
          <div className="emoji-section">
            <div className="emoji-head">{L("入力した絵文字を使う", "Use the emoji typed")}</div>
            <div className="emoji-grid">{tile(typed, "typed")}</div>
          </div>
        ) : q ? (
          <div className="emoji-section">
            <div className="emoji-grid">{results.map((x) => tile(x.e, x.e))}</div>
            {list && results.length === 0 && <div className="empty small">{L("見つかりません。絵文字をそのまま貼り付けても使えます。", "Nothing found. You can also paste an emoji.")}</div>}
          </div>
        ) : (
          <>
            <div className="emoji-section" data-group="top">
              <div className="emoji-head">{recents.length ? L("最近使ったもの", "Recent") : L("おすすめ", "Suggested")}</div>
              <div className="emoji-grid">{top.map((e) => tile(e, `top-${e}`))}</div>
            </div>
            {!list && <div className="empty small">{L("読み込み中…", "Loading…")}</div>}
            {byGroup.map((g) => (
              <div key={g.id} className="emoji-section" data-group={g.id}>
                <div className="emoji-head">{L(g.ja, g.en)}</div>
                <div className="emoji-grid">{g.items.map((x) => tile(x.e, x.e))}</div>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
