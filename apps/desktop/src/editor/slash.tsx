// The "/" menu: insert blocks and snippets by typing a slash.

import { Extension, type Editor, type Range } from "@tiptap/core";
import Suggestion, { type SuggestionProps } from "@tiptap/suggestion";
import { PluginKey } from "@tiptap/pm/state";
import { useEffect, useRef } from "react";
import { create } from "zustand";
import { lang, useT } from "../i18n";
import { editMath, insertNoteLink } from "./menus";
import { chooseFile, chooseImage } from "./images";
import { openTablePicker } from "./table";

interface SlashItem {
  title: string;
  desc: string;
  en: [string, string];
  icon: string;
  keywords: string[];
  run: (editor: Editor, range: Range) => void;
}

const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];

const ITEMS: SlashItem[] = [
  { title: "見出し 1", desc: "章のタイトル", en: ["Heading 1", "Chapter title"], icon: "H1", keywords: ["h1", "midashi", "heading", "見出し", "章"],
    run: (e, r) => e.chain().focus().deleteRange(r).setNode("heading", { level: 1 }).run() },
  { title: "見出し 2", desc: "節のタイトル", en: ["Heading 2", "Section title"], icon: "H2", keywords: ["h2", "midashi", "heading", "見出し", "節"],
    run: (e, r) => e.chain().focus().deleteRange(r).setNode("heading", { level: 2 }).run() },
  { title: "見出し 3", desc: "小見出し", en: ["Heading 3", "Subheading"], icon: "H3", keywords: ["h3", "midashi", "heading", "見出し"],
    run: (e, r) => e.chain().focus().deleteRange(r).setNode("heading", { level: 3 }).run() },
  { title: "本文", desc: "ふつうの段落", en: ["Text", "A plain paragraph"], icon: "¶", keywords: ["p", "text", "honbun", "本文", "段落"],
    run: (e, r) => e.chain().focus().deleteRange(r).setParagraph().run() },
  { title: "箇条書き", desc: "・で始まるリスト", en: ["Bulleted list", "A simple list"], icon: "・", keywords: ["ul", "list", "bullet", "kajou", "箇条書き", "リスト"],
    run: (e, r) => e.chain().focus().deleteRange(r).toggleBulletList().run() },
  { title: "番号付きリスト", desc: "1. 2. 3.", en: ["Numbered list", "1. 2. 3."], icon: "1.", keywords: ["ol", "number", "bangou", "番号"],
    run: (e, r) => e.chain().focus().deleteRange(r).toggleOrderedList().run() },
  { title: "チェックリスト", desc: "やることの一覧", en: ["Checklist", "A to-do list"], icon: "☑", keywords: ["todo", "task", "check", "チェック", "タスク"],
    run: (e, r) => e.chain().focus().deleteRange(r).toggleTaskList().run() },
  { title: "引用", desc: "引用や抜き書き", en: ["Quote", "A quotation or excerpt"], icon: "❝", keywords: ["quote", "inyou", "引用"],
    run: (e, r) => e.chain().focus().deleteRange(r).toggleBlockquote().run() },
  { title: "場面転換", desc: "＊　＊　＊ の区切り", en: ["Scene break", "A * * * divider"], icon: "＊", keywords: ["hr", "divider", "kugiri", "区切り", "場面"],
    run: (e, r) => e.chain().focus().deleteRange(r).setHorizontalRule().run() },
  { title: "ルビ", desc: "|漢字《かんじ》 の形で入力", en: ["Ruby", "Type as |漢字《かんじ》"], icon: "ル", keywords: ["ruby", "furigana", "rubi", "ルビ", "ふりがな"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).insertContent("|《》").run();
      const pos = e.state.selection.from - 2;
      e.commands.setTextSelection(pos);
    } },
  { title: "傍点", desc: "《《強調》》 の形で入力", en: ["Emphasis dots", "Type as 《《text》》"], icon: "﹅", keywords: ["bouten", "emphasis", "傍点"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).insertContent("《《》》").run();
      e.commands.setTextSelection(e.state.selection.from - 2);
    } },
  { title: "マーカー", desc: "文字をハイライト", en: ["Highlight", "Highlight text"], icon: "▮", keywords: ["mark", "highlight", "marker", "マーカー"],
    run: (e, r) => e.chain().focus().deleteRange(r).toggleHighlight().run() },
  { title: "コード", desc: "コードブロック", en: ["Code", "A code block"], icon: "{}", keywords: ["code", "pre", "コード"],
    run: (e, r) => e.chain().focus().deleteRange(r).toggleCodeBlock().run() },
  { title: "画像", en: ["Image", "Insert a picture"], desc: "画像ファイルを挿入", icon: "▣", keywords: ["image", "picture", "gazou", "画像", "写真"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).run();
      void chooseImage(e);
    } },
  { title: "ノートへのリンク", en: ["Link to a note", "Link another note"], desc: "別のノートへリンク", icon: "↗", keywords: ["link", "note", "rinku", "リンク", "参照"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).run();
      void insertNoteLink(e);
    } },
  { title: "数式（文中）", en: ["Inline math", "A TeX formula in the text"], desc: "TeX の数式を文中に", icon: "∑", keywords: ["math", "tex", "latex", "suushiki", "数式", "katex"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).run();
      void editMath(e, "inline");
    } },
  { title: "数式（ブロック）", en: ["Math block", "A displayed TeX formula"], desc: "TeX の数式を独立した行に", icon: "∫", keywords: ["math", "tex", "latex", "block", "suushiki", "数式"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).run();
      void editMath(e, "block");
    } },
  { title: "表", desc: "大きさを選んで表を挿入", en: ["Table", "A 3 × 3 table"], icon: "▦", keywords: ["table", "hyou", "表"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).run();
      openTablePicker(e);
    } },
  { title: "ファイル", en: ["File", "Attach a file (PDF and others)"], desc: "PDF などのファイルを添付", icon: "📎", keywords: ["file", "attach", "pdf", "tenpu", "添付", "ファイル"],
    run: (e, r) => {
      e.chain().focus().deleteRange(r).run();
      void chooseFile(e);
    } },
  { title: "今日の日付", desc: "2026年1月1日（木） の形", en: ["Today’s date", "The current date"], icon: "日", keywords: ["date", "today", "hiduke", "日付", "今日"],
    run: (e, r) => {
      const d = new Date();
      const text = lang() === "en" ? d.toLocaleDateString("en-US", { weekday: "short", year: "numeric", month: "short", day: "numeric" }) : `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日（${WEEKDAYS[d.getDay()]}）`;
      e.chain().focus().deleteRange(r).insertContent(text).run();
    } },
  { title: "現在の時刻", desc: "14:05 の形", en: ["Current time", "The time, like 14:05"], icon: "時", keywords: ["time", "now", "jikoku", "時刻", "時間"],
    run: (e, r) => {
      const d = new Date();
      e.chain().focus().deleteRange(r).insertContent(`${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}`).run();
    } },
];

function filter(query: string): SlashItem[] {
  const q = query.trim().toLowerCase();
  if (!q) return ITEMS;
  return ITEMS.filter((i) => i.title.toLowerCase().includes(q) || i.en[0].toLowerCase().includes(q) || i.keywords.some((k) => k.includes(q)));
}

const label = (i: SlashItem) => (lang() === "en" ? i.en : [i.title, i.desc]);

// The menu's state lives outside React so that the editor plugin can drive it.
interface MenuState {
  open: boolean;
  items: SlashItem[];
  index: number;
  rect: DOMRect | null;
  props: SuggestionProps<SlashItem> | null;
}

const useSlash = create<MenuState>(() => ({ open: false, items: [], index: 0, rect: null, props: null }));

function choose(i: number) {
  const { items, props } = useSlash.getState();
  const item = items[i];
  if (item && props) props.command(item);
}

export const SlashCommand = Extension.create<{ enabled: () => boolean }>({
  name: "slashCommand",
  addOptions() {
    return { enabled: () => true };
  },
  addProseMirrorPlugins() {
    const enabled = this.options.enabled;
    return [
      Suggestion<SlashItem>({
        editor: this.editor,
        pluginKey: new PluginKey("slashCommand"),
        char: "/",
        allowSpaces: false,
        startOfLine: false,
        allow: ({ state, range }) => {
          if (!enabled()) return false;
          // Only at the start of a line or after a space, and not in code.
          const $from = state.doc.resolve(range.from);
          if ($from.parent.type.spec.code) return false;
          const before = $from.parent.textBetween(0, $from.parentOffset);
          return before === "" || /\s$/.test(before);
        },
        items: ({ query }) => filter(query),
        command: ({ editor, range, props }) => props.run(editor, range),
        render: () => ({
          onStart: (props) => useSlash.setState({ open: true, items: props.items, index: 0, rect: props.clientRect?.() ?? null, props }),
          onUpdate: (props) => useSlash.setState({ items: props.items, index: 0, rect: props.clientRect?.() ?? null, props }),
          onKeyDown: ({ event }) => {
            const s = useSlash.getState();
            if (!s.open) return false;
            if (event.key === "ArrowDown") {
              useSlash.setState({ index: (s.index + 1) % Math.max(1, s.items.length) });
              return true;
            }
            if (event.key === "ArrowUp") {
              useSlash.setState({ index: (s.index - 1 + s.items.length) % Math.max(1, s.items.length) });
              return true;
            }
            if (event.key === "Enter" || event.key === "Tab") {
              if (s.items.length === 0) return false;
              choose(s.index);
              return true;
            }
            if (event.key === "Escape") {
              useSlash.setState({ open: false });
              return true;
            }
            return false;
          },
          onExit: () => useSlash.setState({ open: false, props: null }),
        }),
      }),
    ];
  },
});

export function SlashMenu() {
  const t = useT();
  const { open, items, index, rect } = useSlash();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector(".menu-item.active")?.scrollIntoView({ block: "nearest" });
  }, [index]);
  if (!open || !rect || items.length === 0) return null;
  // On the side of the cursor with more room (phones: between the system
  // bars, above the keyboard), no taller than that room.
  const inset = (side: string) => parseFloat(getComputedStyle(document.documentElement).getPropertyValue(`--inset-${side}`)) || 0;
  const below = window.innerHeight - inset("bottom") - rect.bottom - 12;
  const above = rect.top - inset("top") - 12;
  const left = Math.max(6, Math.min(rect.left, window.innerWidth - 246));
  const style =
    below >= 240 || below >= above
      ? { left, top: rect.bottom + 6, maxHeight: Math.min(360, below) }
      : { left, bottom: window.innerHeight - rect.top + 6, maxHeight: Math.min(360, above) };
  return (
    <div className="menu slash-menu" style={style} ref={ref} onMouseDown={(e) => e.preventDefault()}>
      <div className="menu-label">{t("slash.title")}</div>
      {items.map((item, i) => (
        <button
          key={item.title}
          className={`menu-item${i === index ? " active" : ""}`}
          onMouseEnter={() => useSlash.setState({ index: i })}
          onClick={() => choose(i)}
        >
          <span className="ico">{item.icon}</span>
          <span>
            <div>{label(item)[0]}</div>
            <div className="desc">{label(item)[1]}</div>
          </span>
        </button>
      ))}
    </div>
  );
}
