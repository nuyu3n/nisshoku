import { useEffect, useMemo, useRef, useState } from "react";
import { api, type NoteSummary } from "../api";
import { report, useStore } from "../store";
import { shortDate } from "../lib/text";
import { t, useT } from "../i18n";
import { Icon } from "./Icon";
import { goBack, goForward } from "../lib/nav";
import { compileWork, newWorkNote, openPlotBoard, useWork } from "../lib/works";

interface Command {
  label: string;
  icon: string;
  keys?: string;
  run: () => void;
}

/** In a work: its characters, plot and manuscript. */
function workCommands(): Command[] {
  const work = useWork.getState().overview;
  if (!work) return [];
  const L = (ja: string, en: string) => (useStore.getState().prefs.language === "en" ? en : ja);
  const name = work.title;
  return [
    { label: L(`${name}：章を追加`, `${name}: add a chapter`), icon: "book", run: () => void newWorkNote(work.id, "chapter") },
    { label: L(`${name}：登場人物を追加`, `${name}: add a character`), icon: "person", run: () => void newWorkNote(work.id, "character") },
    { label: L(`${name}：プロットボード`, `${name}: plot board`), icon: "plot", run: () => void openPlotBoard(work.id) },
    { label: L(`${name}：作品の情報`, `${name}: details`), icon: "work", run: () => useWork.setState({ editing: work.id }) },
    { label: L(`${name}：原稿を書き出す（HTML）`, `${name}: compile (HTML)`), icon: "download", run: () => void compileWork(work.id, "html") },
    ...work.characters.map((c) => ({ label: L(`人物：${c.name}`, `Character: ${c.name}`), icon: "person", run: () => useStore.getState().select(c.id) })),
  ];
}

function commands(): Command[] {
  const s = useStore.getState();
  const toggle = (k: "typewriter" | "indentParagraphs" | "smartPunctuation" | "lineNumbers" | "autoPair") => s.setPrefs({ [k]: !s.prefs[k] });
  return [
    { label: t("cmd.newNote"), icon: "pen", keys: "Ctrl N", run: () => void s.newNote() },
    { label: t("menu.fromTemplate"), icon: "template", run: () => void import("../lib/actions").then((a) => a.pickTemplate()) },
    { label: t("menu.webClip"), icon: "globe", run: () => void import("../lib/actions").then((a) => a.clipWebPage()) },
    { label: t("stats.year"), icon: "chart", run: () => s.set({ settingsTab: "stats" }) },
    { label: t("nav.todo"), icon: "todo", run: () => void s.setScope({ kind: "todo" }) },
    { label: t("nav.pinned"), icon: "pin", run: () => void s.setScope({ kind: "pinned" }) },
    { label: t("editor.proof"), icon: "proof", run: () => s.set({ panel: s.panel === "proof" ? "none" : "proof" }) },
    { label: t("cmd.diary"), icon: "sun", keys: "Ctrl D", run: () => void s.openDiary() },
    { label: t("cmd.search"), icon: "search", keys: "Ctrl Shift F", run: () => void s.setScope({ kind: "search", query: "" }) },
    { label: t("cmd.focus"), icon: "focus", keys: "F11", run: () => s.set({ focusMode: !s.focusMode }) },
    { label: t("cmd.sidebar"), icon: "sidebarOpen", keys: "Ctrl \\", run: () => s.set({ sidebarOpen: !s.sidebarOpen }) },
    { label: t("cmd.list"), icon: "listShow", keys: "Ctrl Shift \\", run: () => s.set({ listOpen: !s.listOpen }) },
    { label: t("editor.outline"), icon: "outline", run: () => s.set({ panel: s.panel === "outline" ? "none" : "outline" }) },
    ...(s.openKind ? [] : [{ label: t("editor.preview"), icon: "vertical", run: () => s.set({ panel: s.panel === "preview" ? "none" : "preview" }) }]),
    { label: t("editor.history"), icon: "history", run: () => s.set({ panel: s.panel === "history" ? "none" : "history" }) },
    { label: t("comments.title"), icon: "comment", run: () => s.set({ panel: s.panel === "comments" ? "none" : "comments" }) },
    { label: s.prefs.typewriter ? t("cmd.typewriterOff") : t("cmd.typewriterOn"), icon: "type", run: () => toggle("typewriter") },
    { label: s.prefs.indentParagraphs ? t("cmd.indentOff") : t("cmd.indentOn"), icon: "type", run: () => toggle("indentParagraphs") },
    { label: s.prefs.lineNumbers ? t("cmd.lineNumbersOff") : t("cmd.lineNumbersOn"), icon: "lines", run: () => toggle("lineNumbers") },
    { label: s.prefs.autoPair ? t("cmd.autoPairOff") : t("cmd.autoPairOn"), icon: "type", run: () => toggle("autoPair") },
    ...(s.selectedId ? [{ label: t("goto.line"), icon: "lines", keys: "Ctrl G", run: () => window.dispatchEvent(new Event("nisshoku:goto-line")) }] : []),
    ...workCommands(),
    { label: t("nav.backNote"), icon: "chevronLeft", keys: "Alt ←", run: () => void goBack() },
    { label: t("nav.forwardNote"), icon: "chevronRight", keys: "Alt →", run: () => void goForward() },
    { label: t("cmd.theme"), icon: "moon", run: () => s.setPrefs({ theme: document.documentElement.dataset.resolvedTheme === "dark" ? "light" : "dark" }) },
    { label: t("cmd.sync"), icon: "sync", run: () => void api.syncNow().catch(report) },
    { label: t("cmd.syncSettings"), icon: "cloud", run: () => s.set({ settingsTab: "sync" }) },
    { label: t("cmd.settings"), icon: "settings", keys: "Ctrl ,", run: () => s.set({ settingsTab: "general" }) },
    { label: t("nav.allNotes"), icon: "notes", run: () => void s.setScope({ kind: "all" }) },
    { label: t("nav.trash"), icon: "trash", run: () => void s.setScope({ kind: "trash" }) },
  ];
}

export function CommandPalette() {
  useT();
  const open = useStore((s) => s.paletteOpen);
  const [query, setQuery] = useState("");
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setIndex(0);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const t = window.setTimeout(() => {
      (query.trim() ? api.listNotes({ kind: "search", query }) : api.listNotes({ kind: "all" }))
        .then((n) => setNotes(n.slice(0, query.trim() ? 30 : 8)))
        .catch(report);
    }, 80);
    return () => window.clearTimeout(t);
  }, [query, open]);

  const cmds = useMemo(() => {
    const q = query.trim().toLowerCase();
    const all = commands();
    return q ? all.filter((c) => c.label.toLowerCase().includes(q)) : all.slice(0, 6);
  }, [query, open]);

  const items = [
    ...notes.map((n) => ({ kind: "note" as const, note: n })),
    ...cmds.map((c) => ({ kind: "cmd" as const, cmd: c })),
  ];

  useEffect(() => setIndex(0), [query]);
  useEffect(() => {
    listRef.current?.querySelector(".palette-item.active")?.scrollIntoView({ block: "nearest" });
  }, [index]);

  if (!open) return null;
  const close = () => useStore.getState().set({ paletteOpen: false });
  const run = (i: number) => {
    const item = items[i];
    if (!item) return;
    close();
    if (item.kind === "note") {
      const s = useStore.getState();
      if (!s.notes.some((n) => n.id === item.note.id)) {
        void s.setScope({ kind: "all" }).then(() => s.select(item.note.id));
      } else s.select(item.note.id);
    } else item.cmd.run();
  };

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="dialog palette">
        <div className="palette-input">
          <Icon name="search" />
          <input
            autoFocus
            placeholder={t("palette.placeholder")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.nativeEvent.isComposing) return;
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setIndex((i) => Math.min(items.length - 1, i + 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setIndex((i) => Math.max(0, i - 1));
              } else if (e.key === "Enter") {
                e.preventDefault();
                run(index);
              } else if (e.key === "Escape") close();
            }}
          />
          <span className="kbd">Esc</span>
        </div>
        <div className="palette-list" ref={listRef}>
          {notes.length > 0 && <div className="palette-group">{query.trim() ? t("palette.notes") : t("palette.recent")}</div>}
          {items.map((item, i) => {
            const first = item.kind === "cmd" && (i === 0 || items[i - 1].kind === "note");
            return (
              <div key={item.kind === "note" ? item.note.id : item.cmd.label}>
                {first && <div className="palette-group">{t("palette.commands")}</div>}
                <button className={`palette-item${i === index ? " active" : ""}`} onMouseEnter={() => setIndex(i)} onClick={() => run(i)}>
                  <Icon name={item.kind === "note" ? "note" : item.cmd.icon} />
                  {item.kind === "note" ? (
                    <>
                      <span className="main-t">
                        {item.note.title || t("list.untitled")}
                        {query.trim() && item.note.excerpt && (
                          <span style={{ color: "var(--text-3)", marginLeft: 10, fontSize: 12 }}>{item.note.excerpt.slice(0, 40)}</span>
                        )}
                      </span>
                      <span className="sub">{shortDate(item.note.updatedTime)}</span>
                    </>
                  ) : (
                    <>
                      <span className="main-t">{item.cmd.label}</span>
                      {item.cmd.keys && <span className="kbd">{item.cmd.keys}</span>}
                    </>
                  )}
                </button>
              </div>
            );
          })}
          {items.length === 0 && <div className="empty">{t("list.notFound")}</div>}
        </div>
      </div>
    </div>
  );
}
