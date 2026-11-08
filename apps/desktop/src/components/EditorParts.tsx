// Pieces of the editor: find bar, status bar, selection toolbar, table
// toolbar and the Markdown source editor.

import { useEffect, useRef, useState } from "react";
import type { Editor } from "@tiptap/react";
import { TextSelection } from "@tiptap/pm/state";
import { useStore } from "../store";
import { t, useT } from "../i18n";
import { searchKey } from "../editor/extensions";
import { tableItems } from "../editor/menus";
import { alignColumn } from "../editor/table";
import { formatNumber } from "../lib/text";
import { Icon } from "./Icon";
import { dialogs } from "./Dialogs";
import { contextMenu, openMenu } from "./ContextMenu";
import { inputMenu } from "../lib/actions";
import { TextLines, type TextLinesHandle } from "./TextLines";

export function SourceEditor({ value, readOnly, onChange, lines }: { value: string; readOnly: boolean; onChange: (v: string) => void; lines?: React.Ref<TextLinesHandle> }) {
  const local = useRef<TextLinesHandle>(null);
  return (
    <TextLines
      ref={(h) => {
        local.current = h;
        if (typeof lines === "function") lines(h);
        else if (lines) (lines as React.MutableRefObject<TextLinesHandle | null>).current = h;
      }}
      className="source-editor"
      value={value}
      readOnly={readOnly}
      spellCheck={false}
      onChange={onChange}
      {...contextMenu(() => (local.current?.textarea ? inputMenu(local.current.textarea) : []))}
    />
  );
}

// -------------------------------------------------------------------------

export function FindBar({ editor, mode, onClose }: { editor: Editor; mode: "find" | "replace"; onClose: () => void }) {
  useT();
  const [query, setQuery] = useState("");
  const [replacement, setReplacement] = useState("");
  const [showReplace, setShowReplace] = useState(mode === "replace");
  const [, force] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setShowReplace((s) => s || mode === "replace");
    input.current?.focus();
    input.current?.select();
  }, [mode]);
  useEffect(() => {
    editor.commands.setSearch(query);
    force((n) => n + 1);
  }, [query, editor]);
  useEffect(() => {
    const update = () => force((n) => n + 1);
    editor.on("transaction", update);
    return () => {
      editor.off("transaction", update);
      editor.commands.setSearch("");
    };
  }, [editor]);

  const s = searchKey.getState(editor.state);
  const total = s?.matches.length ?? 0;
  const close = () => {
    editor.commands.setSearch("");
    onClose();
    editor.commands.focus();
  };

  return (
    <div className="find-bar" onKeyDown={(e) => e.key === "Escape" && close()}>
      <div className="row">
        <input
          ref={input}
          className="input"
          placeholder={t("find.placeholder")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              e.preventDefault();
              if (e.shiftKey) editor.commands.findPrevious();
              else editor.commands.findNext();
            }
          }}
        />
        <span className="count">{query ? `${total ? (s?.current ?? 0) + 1 : 0} / ${total}` : ""}</span>
        <button className="icon-btn" title={t("find.prev")} onClick={() => editor.commands.findPrevious()}>
          <Icon name="chevronLeft" />
        </button>
        <button className="icon-btn" title={t("find.next")} onClick={() => editor.commands.findNext()}>
          <Icon name="chevronRight" />
        </button>
        <button className={`icon-btn${showReplace ? " active" : ""}`} title={t("find.replace")} onClick={() => setShowReplace(!showReplace)}>
          <Icon name="replace" />
        </button>
        <button className="icon-btn" title={t("find.close")} onClick={close}>
          <Icon name="x" />
        </button>
      </div>
      {showReplace && (
        <div className="row">
          <input className="input" placeholder={t("find.replacement")} value={replacement} onChange={(e) => setReplacement(e.target.value)} />
          <button className="btn" disabled={!total} onClick={() => editor.chain().replaceCurrent(replacement).findNext().run()}>
            {t("find.replaceOne")}
          </button>
          <button
            className="btn"
            disabled={!total}
            onClick={() => {
              const n = total;
              editor.commands.replaceAll(replacement);
              useStore.getState().toast(t("find.replaced", { n }));
            }}
          >
            {t("find.replaceAll")}
          </button>
        </div>
      )}
    </div>
  );
}

// -------------------------------------------------------------------------

type Recognition = { lang: string; continuous: boolean; interimResults: boolean; start: () => void; stop: () => void; onresult: ((e: { resultIndex: number; results: ArrayLike<{ isFinal: boolean; 0: { transcript: string } }> }) => void) | null; onend: (() => void) | null; onerror: ((e: { error: string }) => void) | null };

function speechRecognition(): (new () => Recognition) | null {
  const w = window as unknown as { SpeechRecognition?: new () => Recognition; webkitSpeechRecognition?: new () => Recognition };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
}

/** Dictation into the editor, where the webview offers speech recognition. */
function VoiceButton({ editor }: { editor: Editor | null }) {
  const [rec, setRec] = useState<Recognition | null>(null);
  const language = useStore((s) => s.prefs.language);
  useEffect(() => () => rec?.stop(), [rec]);
  return (
    <button
      className={`item${rec ? " recording" : ""}`}
      title={rec ? t("voice.stop") : t("voice.start")}
      onClick={() => {
        if (rec) {
          rec.stop();
          setRec(null);
          return;
        }
        const Ctor = speechRecognition();
        if (!Ctor || !editor) {
          useStore.getState().toast(t("voice.unavailable"));
          return;
        }
        const r = new Ctor();
        r.lang = language === "en" ? "en-US" : "ja-JP";
        r.continuous = true;
        r.interimResults = false;
        r.onresult = (e) => {
          for (let i = e.resultIndex; i < e.results.length; i++) {
            if (e.results[i].isFinal) editor.chain().focus().insertContent(e.results[i][0].transcript).run();
          }
        };
        r.onerror = (e) => {
          setRec(null);
          if (e.error === "not-allowed" || e.error === "service-not-allowed" || e.error === "network") useStore.getState().toast(t("voice.unavailable"));
        };
        r.onend = () => setRec(null);
        try {
          r.start();
          setRec(r);
          useStore.getState().toast(t("voice.listening"));
        } catch {
          useStore.getState().toast(t("voice.unavailable"));
        }
      }}
    >
      <Icon name="mic" />
    </button>
  );
}

export function StatusBar({ editor, chars, withSpaces, selection, pages, lines, goal }: { editor: Editor | null; chars: number; withSpaces: number; selection: number; pages: number | null; lines: number; goal: number | null }) {
  useT();
  const stats = useStore((s) => s.stats);
  const prefs = useStore((s) => s.prefs);
  const [timer, setTimer] = useState<{ end: number; startChars: number } | null>(null);
  const [now, setNow] = useState(Date.now());
  const count = prefs.countSpaces ? withSpaces : chars;

  useEffect(() => {
    if (!timer) return;
    const id = window.setInterval(() => {
      setNow(Date.now());
      if (Date.now() >= timer.end) {
        useStore.getState().toast(t("timer.done", { n: formatNumber(Math.max(0, chars - timer.startChars)) }));
        setTimer(null);
      }
    }, 1000);
    return () => window.clearInterval(id);
  }, [timer, chars]);

  const remaining = timer ? Math.max(0, timer.end - now) : 0;
  const mmss = `${Math.floor(remaining / 60000)}:${String(Math.floor((remaining % 60000) / 1000)).padStart(2, "0")}`;
  const goalRatio = goal ? Math.min(1, chars / goal) : 0;
  const dailyRatio = prefs.dailyGoal ? Math.min(1, (stats?.today ?? 0) / prefs.dailyGoal) : 0;
  const minutes = chars === 0 ? 0 : Math.max(1, Math.round(chars / Math.max(50, prefs.readingSpeed)));

  return (
    <div className="status-bar">
      <span className="item" title={t("status.withSpaces", { n: formatNumber(withSpaces) })}>
        <b>{formatNumber(selection || count)}</b> {t("status.chars")}
        {selection ? t("status.selected") : ""}
      </span>
      {/* (No sheet count for a page that is not manuscript: pages is null.) */}
      {prefs.statusManuscript && pages !== null && (
        <span className="item hide-narrow st-manuscript" title={t("status.manuscriptHint", { cols: prefs.manuscriptCols, rows: prefs.manuscriptRows, lines: formatNumber(lines) })}>
          {t("status.manuscript")} <b>{pages}</b>
          {t("status.pages")}
        </span>
      )}
      {prefs.statusReading && <span className="item hide-narrow st-reading">{t("status.reading", { n: minutes })}</span>}
      {goal ? (
        <span className="item" title={t("status.goal", { n: formatNumber(goal) })}>
          <div className={`goal-bar${goalRatio >= 1 ? " done" : ""}`}>
            <div style={{ width: `${goalRatio * 100}%` }} />
          </div>
          {Math.round(goalRatio * 100)}%
        </span>
      ) : null}
      <span className="grow" />
      {prefs.statusToday && (
        <span className="item hide-narrow st-today" title={t("status.todayHint")}>
          {t("status.today")} <b>{formatNumber(stats?.today ?? 0)}</b> {t("status.chars")}
          {prefs.dailyGoal ? (
            <div className={`goal-bar${dailyRatio >= 1 ? " done" : ""}`}>
              <div style={{ width: `${dailyRatio * 100}%` }} />
            </div>
          ) : null}
        </span>
      )}
      <VoiceButton editor={editor} />
      {timer ? (
        <button className="item timer-ring" title={t("status.timerStop")} onClick={() => setTimer(null)}>
          <Icon name="timer" />
          {mmss}・{formatNumber(Math.max(0, chars - timer.startChars))}
        </button>
      ) : (
        <button
          className="item"
          title={t("status.timerHint")}
          onClick={async () => {
            const v = await dialogs.prompt(t("timer.title"), { initial: String(prefs.timerMinutes), message: t("timer.message") });
            const min = v ? parseInt(v.normalize("NFKC"), 10) : NaN;
            if (Number.isFinite(min) && min > 0) setTimer({ end: Date.now() + min * 60000, startChars: chars });
          }}
        >
          <Icon name="timer" />
          <span className="hide-narrow st-timer-label">{t("status.timer")}</span>
        </button>
      )}
    </div>
  );
}

// -------------------------------------------------------------------------

/**
 * Phones: formatting above the keyboard while the text has the focus (there is
 * no menu bar, and selecting text to format it is fiddly).
 */
export function MobileFormatBar({ editor }: { editor: Editor }) {
  const t = useT();
  const [focused, setFocused] = useState(editor.isFocused);
  const [, refresh] = useState(0);
  useEffect(() => {
    const focus = () => setFocused(true);
    const blur = () => setFocused(false);
    const update = () => refresh((n) => n + 1);
    editor.on("focus", focus);
    editor.on("blur", blur);
    editor.on("transaction", update);
    return () => {
      editor.off("focus", focus);
      editor.off("blur", blur);
      editor.off("transaction", update);
    };
  }, [editor]);
  if (!focused) return null;
  const level = ([1, 2, 3] as const).find((l) => editor.isActive("heading", { level: l }));
  const btn = (icon: string, title: string, active: boolean, run: () => void, label?: string) => (
    <button className={active ? "on" : ""} title={title} aria-label={title} onPointerDown={(e) => e.preventDefault()} onMouseDown={(e) => e.preventDefault()} onClick={run}>
      {label ?? <Icon name={icon} />}
    </button>
  );
  const chain = () => editor.chain().focus();
  return (
    <div className="format-bar" role="toolbar">
      <div className="format-scroll">
        {btn("undo", t("fmt.undo"), false, () => chain().undo().run())}
        {btn("redo", t("fmt.redo"), false, () => chain().redo().run())}
        <span className="bubble-sep" />
        {btn("heading", t("fmt.heading"), level !== undefined, () =>
          level === 3 ? chain().setParagraph().run() : chain().setHeading({ level: level ? ((level + 1) as 2 | 3) : 1 }).run(),
          level ? `H${level}` : undefined,
        )}
        {btn("bold", t("fmt.bold"), editor.isActive("bold"), () => chain().toggleBold().run())}
        {btn("italic", t("fmt.italic"), editor.isActive("italic"), () => chain().toggleItalic().run())}
        {btn("list", t("fmt.bullet"), editor.isActive("bulletList"), () => chain().toggleBulletList().run())}
        {btn("listOrdered", t("fmt.ordered"), editor.isActive("orderedList"), () => chain().toggleOrderedList().run())}
        {btn("todo", t("fmt.checklist"), editor.isActive("taskList"), () => chain().toggleTaskList().run())}
        {btn("quote", t("fmt.quote"), editor.isActive("blockquote"), () => chain().toggleBlockquote().run())}
        {btn("plus", t("fmt.insert"), false, () => chain().insertContent("/").run())}
      </div>
      {/* Always in reach, outside the scrolling buttons. */}
      {btn("keyboardHide", t("fmt.done"), false, () => editor.commands.blur())}
    </div>
  );
}

/** Small toolbar over a text selection: emphasis, highlight, link and comments. */
export function SelectionBubble({ editor, onComment, onLink }: { editor: Editor; onComment: () => void; onLink: () => void }) {
  useT();
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  useEffect(() => {
    const update = () => {
      const { from, to, empty } = editor.state.selection;
      // Not for cell selections or nodes (math, images).
      if (empty || !editor.isFocused || !(editor.state.selection instanceof TextSelection)) {
        setPos(null);
        return;
      }
      const a = editor.view.coordsAtPos(from);
      const b = editor.view.coordsAtPos(to);
      const top = Math.min(a.top, b.top) - 46;
      // Within the editor's column (the bubble is centred, about 340px wide).
      const pane = editor.view.dom.closest(".main")?.getBoundingClientRect() ?? { left: 0, right: window.innerWidth };
      const half = 175;
      setPos({ left: Math.min(Math.max((a.left + b.right) / 2, pane.left + half), pane.right - half), top: top < 60 ? Math.max(a.bottom, b.bottom) + 10 : top });
    };
    editor.on("selectionUpdate", update);
    editor.on("blur", update);
    editor.on("focus", update);
    return () => {
      editor.off("selectionUpdate", update);
      editor.off("blur", update);
      editor.off("focus", update);
    };
  }, [editor]);
  if (!pos) return null;
  const btn = (icon: string, title: string, active: boolean, run: () => void) => (
    <button className={active ? "on" : ""} title={title} onMouseDown={(e) => e.preventDefault()} onClick={run}>
      <Icon name={icon} />
    </button>
  );
  return (
    <div className="bubble" style={{ left: pos.left, top: pos.top }} onMouseDown={(e) => e.preventDefault()}>
      {btn("bold", t("fmt.bold"), editor.isActive("bold"), () => editor.chain().focus().toggleBold().run())}
      {btn("italic", t("fmt.italic"), editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run())}
      {btn("strike", t("fmt.strike"), editor.isActive("strike"), () => editor.chain().focus().toggleStrike().run())}
      {btn("marker", t("fmt.mark"), editor.isActive("highlight"), () => editor.chain().focus().toggleHighlight().run())}
      {btn("link", t("menu.link"), editor.isActive("link"), onLink)}
      <span className="bubble-sep" />
      <button className="label" onClick={onComment} title="Ctrl+Alt+M">
        <Icon name="comment" />
        {t("comments.add")}
      </button>
    </div>
  );
}

// -------------------------------------------------------------------------

/** A toolbar above the table the cursor is in. */
export function TableTools({ editor, scroller }: { editor: Editor; scroller: HTMLElement | null }) {
  useT();
  const [rect, setRect] = useState<{ left: number; top: number } | null>(null);
  useEffect(() => {
    const update = () => {
      if (!editor.isActive("table") || !editor.isEditable) {
        setRect(null);
        return;
      }
      const { $from } = editor.state.selection;
      let depth = $from.depth;
      while (depth > 0 && $from.node(depth).type.name !== "table") depth--;
      const dom = depth > 0 ? (editor.view.nodeDOM($from.before(depth)) as HTMLElement | null) : null;
      const table = dom?.closest?.("table") ?? dom?.querySelector?.("table") ?? dom;
      if (!table) return setRect(null);
      const r = table.getBoundingClientRect();
      setRect({ left: r.left, top: r.top - 40 });
    };
    update();
    editor.on("selectionUpdate", update);
    editor.on("transaction", update);
    editor.on("blur", update);
    scroller?.addEventListener("scroll", update, { passive: true });
    return () => {
      editor.off("selectionUpdate", update);
      editor.off("transaction", update);
      editor.off("blur", update);
      scroller?.removeEventListener("scroll", update);
    };
  }, [editor, scroller]);
  if (!rect || rect.top < 48) return null;
  const c = () => editor.chain().focus();
  const btn = (icon: string, title: string, run: () => void, danger = false) => (
    <button className={danger ? "danger" : ""} title={title} onMouseDown={(e) => e.preventDefault()} onClick={run}>
      <Icon name={icon} />
    </button>
  );
  return (
    <div className="table-tools" style={{ left: rect.left, top: rect.top }} onMouseDown={(e) => e.preventDefault()}>
      {btn("rowAbove", t("table.rowAbove"), () => c().addRowBefore().run())}
      {btn("rowBelow", t("table.rowBelow"), () => c().addRowAfter().run())}
      {btn("rowDelete", t("table.deleteRow"), () => c().deleteRow().run())}
      <span className="bubble-sep" />
      {btn("colLeft", t("table.colLeft"), () => c().addColumnBefore().run())}
      {btn("colRight", t("table.colRight"), () => c().addColumnAfter().run())}
      {btn("colDelete", t("table.deleteCol"), () => c().deleteColumn().run())}
      <span className="bubble-sep" />
      {btn("heading", t("table.headerRow"), () => c().toggleHeaderRow().run())}
      {btn("alignLeft", t("table.alignLeft"), () => alignColumn(editor, "left"))}
      {btn("alignCenter", t("table.alignCenter"), () => alignColumn(editor, "center"))}
      {btn("alignRight", t("table.alignRight"), () => alignColumn(editor, "right"))}
      <button title={t("editor.more")} onMouseDown={(e) => e.preventDefault()} onClick={(e) => openMenu(e.currentTarget, tableItems(editor))}>
        <Icon name="more" />
      </button>
      {btn("trash", t("table.delete"), () => c().deleteTable().run(), true)}
    </div>
  );
}
