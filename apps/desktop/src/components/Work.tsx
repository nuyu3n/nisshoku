// A work's screens: the panel in the note list (manuscript, characters,
// plot, materials), its details dialog, and in the editor the chapter bar,
// the character sheet and the plot board.

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, type Character, type ChapterInfo, type ChapterStatus, type Note, type Plot, type PlotCard, type WorkMeta, type WorkOverview } from "../api";
import { report, useStore } from "../store";
import { lang } from "../i18n";
import { formatNumber } from "../lib/text";
import { Icon } from "./Icon";
import { dragChip } from "../lib/drag";
import { DateField, NumberField, Select } from "./Controls";
import { openMenu } from "./ContextMenu";
import { BoardSkeleton, SheetSkeleton } from "./Skeleton";
import { useInk } from "../lib/motion";
import { contextMenu } from "./ContextMenu";
import { noteMenu } from "../lib/actions";
import {
  ROLES,
  STATUSES,
  characterColor,
  compileWork,
  daysLeft,
  emptyWorkMeta,
  newWorkNote,
  openPlotBoard,
  reloadWork,
  roleLabel,
  saveWorkMeta,
  setChapterMeta,
  statusLabel,
  unmakeWork,
  useWork,
  type WorkTab,
} from "../lib/works";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

function newId() {
  return Math.random().toString(36).slice(2, 10);
}

/** Saves `value` a moment after the last change (and when unmounted). */
function useDebouncedSave<T>(value: T | null, save: (v: T) => void, delay = 600) {
  const saveRef = useRef(save);
  saveRef.current = save;
  const pending = useRef<T | null>(null);
  const first = useRef(true);
  useEffect(() => {
    if (value === null) return;
    if (first.current) {
      first.current = false;
      return;
    }
    pending.current = value;
    const timer = window.setTimeout(() => {
      pending.current = null;
      saveRef.current(value);
    }, delay);
    return () => window.clearTimeout(timer);
  }, [value, delay]);
  useEffect(
    () => () => {
      if (pending.current !== null) saveRef.current(pending.current);
    },
    [],
  );
}

function Avatar({ name, color, size = 28 }: { name: string; color: string; size?: number }) {
  return (
    <span className="avatar" style={{ background: characterColor(name, color), width: size, height: size, fontSize: size * 0.46 }}>
      {[...(name.trim() || "?")][0]}
    </span>
  );
}

function openNoteById(id: string) {
  useStore.getState().select(id);
}

// -------------------------------------------------------------------------
// The panel in the note list

function Progress({ work }: { work: WorkOverview }) {
  const target = work.meta.targetChars ?? 0;
  const left = daysLeft(work.meta.deadline);
  const remaining = Math.max(0, target - work.totalChars);
  const pct = target ? Math.min(100, (work.totalChars / target) * 100) : 0;
  const statuses = STATUSES.map((s) => [s, work.statusCounts[s] ?? 0] as const);
  const unset = work.statusCounts[""] ?? 0;
  return (
    <div className="work-progress">
      <div className="work-figures">
        <b>{formatNumber(work.totalChars)}</b>
        {target > 0 && (
          <span>
            {" "}
            / {formatNumber(target)} {L("字", "chars")} · {Math.floor(pct)}%
          </span>
        )}
        {target === 0 && <span> {L("字", "chars")}</span>}
      </div>
      {target > 0 && (
        <div className="progress">
          <div style={{ width: `${pct}%` }} />
        </div>
      )}
      {left !== null && (
        <div className={`work-deadline${left < 0 ? " over" : left <= 7 ? " soon" : ""}`}>
          <Icon name="calendar" />
          {left < 0
            ? L(`締め切りを ${-left} 日過ぎています`, `${-left} days past the deadline`)
            : left === 0
              ? L("今日が締め切り", "Due today")
              : L(`締め切りまで ${left} 日`, `${left} days to the deadline`)}
          {target > 0 && remaining > 0 && left > 0 && (
            <span>
              {" "}
              · {L("1日", "")} {formatNumber(Math.ceil(remaining / left))} {L("字", "chars a day")}
            </span>
          )}
        </div>
      )}
      {work.chapterCount > 0 && (
        <div className="status-bar-mini" title={statuses.map(([s, n]) => `${statusLabel(s)} ${n}`).join(" · ")}>
          {statuses.map(([s, n]) => (n > 0 ? <span key={s} className={`st-${s}`} style={{ flex: n }} /> : null))}
          {unset > 0 && <span className="st-" style={{ flex: unset }} />}
        </div>
      )}
    </div>
  );
}

function StatusChip({ chapter }: { chapter: ChapterInfo }) {
  return (
    <button
      className={`status-chip st-${chapter.status || "none"}`}
      title={L("状態を変える", "Change the status")}
      onClick={(e) => {
        e.stopPropagation();
        openMenu(
          e.currentTarget,
          (["", ...STATUSES] as ChapterStatus[]).map((s) => ({
            label: statusLabel(s),
            checked: chapter.status === s,
            run: () => void setChapterMeta(chapter.id, { status: s, synopsis: chapter.synopsis, pov: chapter.pov }),
          })),
        );
      }}
    >
      {statusLabel(chapter.status)}
    </button>
  );
}

function ChapterRow({ chapter, index, work, onDrop }: { chapter: ChapterInfo; index: number; work: WorkOverview; onDrop: (dragged: string) => void }) {
  const selected = useStore((s) => s.selectedId === chapter.id);
  const [over, setOver] = useState(false);
  const cast = chapter.characters.slice(0, 5).map(([id]) => work.characters.find((c) => c.id === id)).filter(Boolean);
  return (
    <div
      role="button"
      tabIndex={0}
      className={`chapter-item${selected ? " active" : ""}${over ? " drop-before" : ""}`}
      onClick={() => openNoteById(chapter.id)}
      onKeyDown={(e) => e.key === "Enter" && openNoteById(chapter.id)}
      {...contextMenu(() => noteMenu(chapter))}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("application/x-nisshoku-note", chapter.id);
        e.dataTransfer.effectAllowed = "move";
        dragChip(e, chapter.title || L("無題の章", "Untitled chapter"));
      }}
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes("application/x-nisshoku-note")) {
          e.preventDefault();
          setOver(true);
        }
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        setOver(false);
        const id = e.dataTransfer.getData("application/x-nisshoku-note");
        if (id && id !== chapter.id) {
          e.preventDefault();
          e.stopPropagation();
          onDrop(id);
        }
      }}
    >
      <div className="ch-row">
        <span className="ch-num">{index + 1}</span>
        <span className={`ch-title${chapter.title ? "" : " untitled"}`}>{chapter.title || L("無題の章", "Untitled chapter")}</span>
        <StatusChip chapter={chapter} />
      </div>
      {(chapter.synopsis || chapter.excerpt) && <div className={`ch-syn${chapter.synopsis ? "" : " excerpt"}`}>{chapter.synopsis || chapter.excerpt}</div>}
      <div className="ch-meta">
        <span>
          {formatNumber(chapter.charCount)} {L("字", "chars")}
        </span>
        {chapter.goal ? <span className="goal">{Math.min(100, Math.round((chapter.charCount / chapter.goal) * 100))}%</span> : null}
        <span className="cast">
          {cast.map((c) => (
            <Avatar key={c!.id} name={c!.name} color={c!.color} size={16} />
          ))}
        </span>
      </div>
    </div>
  );
}

function ManuscriptTab({ work }: { work: WorkOverview }) {
  let n = 0;
  const empty = work.chapterCount === 0;
  return (
    <>
      {work.sections.map((sec) => {
        if (sec.depth > 0 || sec.chapters.length > 0 || empty) {
          const chars = sec.chapters.reduce((k, c) => k + c.charCount, 0);
          return (
            <div key={sec.folderId} className="work-section">
              {sec.depth > 0 && (
                <div className="section-head" style={{ paddingLeft: 12 + (sec.depth - 1) * 12 }}>
                  <Icon name="folder" />
                  <span className="grow">{sec.title}</span>
                  <small>{formatNumber(chars)}</small>
                  <button className="icon-btn small" title={L("この部に章を追加", "Add a chapter to this part")} onClick={() => void newWorkNote(work.id, "chapter", sec.folderId)}>
                    <Icon name="plus" />
                  </button>
                </div>
              )}
              {sec.chapters.map((c) => (
                <ChapterRow
                  key={c.id}
                  chapter={c}
                  index={n++}
                  work={work}
                  onDrop={async (dragged) => {
                    const ids = sec.chapters.map((x) => x.id);
                    if (!ids.includes(dragged)) {
                      // From another part: move it here, before this chapter.
                      await api.moveNote(dragged, sec.folderId).catch(report);
                      ids.splice(ids.indexOf(c.id), 0, dragged);
                    } else {
                      ids.splice(ids.indexOf(dragged), 1);
                      ids.splice(ids.indexOf(c.id), 0, dragged);
                    }
                    await api.setNotesOrder(ids).catch(report);
                    await useStore.getState().refresh();
                    await reloadWork();
                  }}
                />
              ))}
            </div>
          );
        }
        return null;
      })}
      {empty && <div className="empty">{L("まだ章がありません。", "No chapters yet.")}</div>}
      <button className="add-row" onClick={() => void newWorkNote(work.id, "chapter")}>
        <Icon name="plus" />
        {L("章を追加", "Add a chapter")}
      </button>
    </>
  );
}

function CharactersTab({ work }: { work: WorkOverview }) {
  const selectedId = useStore((s) => s.selectedId);
  const chapterTitle = useMemo(() => {
    const m = new Map<string, string>();
    for (const s of work.sections) for (const c of s.chapters) m.set(c.id, c.title);
    return m;
  }, [work]);
  return (
    <>
      {work.characters.length === 0 && <div className="empty">{L("登場人物を追加すると、本文の中の名前に色が付き、どの章に出てくるかが分かります。", "Add characters: their names are highlighted in the text, and you see which chapters they appear in.")}</div>}
      <div className="char-grid">
        {work.characters.map((c) => (
          <div
            key={c.id}
            role="button"
            tabIndex={0}
            className={`char-card${selectedId === c.id ? " active" : ""}`}
            onClick={() => openNoteById(c.id)}
            onKeyDown={(e) => e.key === "Enter" && openNoteById(c.id)}
            {...contextMenu(() => [{ label: L("開く", "Open"), icon: "note", run: () => openNoteById(c.id) }])}
          >
            <Avatar name={c.name} color={c.color} size={34} />
            <div className="char-main">
              <div className="char-name-row">
                <b>{c.name || L("名前なし", "No name")}</b>
                {c.ruby && <small>{c.ruby}</small>}
              </div>
              {c.role && <span className={`role-chip role-${c.role}`}>{roleLabel(c.role)}</span>}
              <div className="char-appear" title={c.chapters.map((id) => chapterTitle.get(id)).join("\n")}>
                {c.chapters.length
                  ? L(`${c.chapters.length} 章に登場 · ${c.mentions} 回`, `In ${c.chapters.length} chapters · ${c.mentions}×`)
                  : L("まだ本文に出てきません", "Not in the text yet")}
              </div>
            </div>
          </div>
        ))}
      </div>
      <button className="add-row" onClick={() => void newWorkNote(work.id, "character")}>
        <Icon name="plus" />
        {L("登場人物を追加", "Add a character")}
      </button>
    </>
  );
}

function PlotTab({ work }: { work: WorkOverview }) {
  const [plot, setPlot] = useState<Plot | null>(null);
  useEffect(() => {
    if (!work.plotId) {
      setPlot(null);
      return;
    }
    void api.openPlot(work.id).then((d) => setPlot(d.plot), () => setPlot(null));
  }, [work.id, work.plotId]);
  return (
    <>
      {!work.plotId && <div className="empty">{L("物語の流れをカードで組み立てます。カードは章と結びつけられます。", "Build the story with cards, linked to chapters.")}</div>}
      {plot &&
        plot.columns.map((col) => {
          const cards = plot.cards.filter((c) => c.column === col.id);
          return (
            <div key={col.id} className="plot-summary">
              <div className="section-head">
                <span className="grow">{col.title}</span>
                <small>{cards.length}</small>
              </div>
              {cards.map((c) => (
                <div key={c.id} className={`plot-line${c.done ? " done" : ""}`}>
                  <span className="dot" />
                  {c.title || L("（無題）", "(untitled)")}
                </div>
              ))}
            </div>
          );
        })}
      <button className="add-row primary" onClick={() => void openPlotBoard(work.id)}>
        <Icon name="plot" />
        {work.plotId ? L("プロットボードを開く", "Open the plot board") : L("プロットボードを作る", "Make a plot board")}
      </button>
    </>
  );
}

function MaterialsTab({ work }: { work: WorkOverview }) {
  const selectedId = useStore((s) => s.selectedId);
  return (
    <>
      {work.materials.length === 0 && <div className="empty">{L("世界観、年表、地図、取材メモなど、本文以外の資料をまとめておく場所です。", "Setting, timeline, maps, research: everything that is not the text.")}</div>}
      {work.materials.map((m) => (
        <div
          key={m.id}
          role="button"
          tabIndex={0}
          className={`chapter-item${selectedId === m.id ? " active" : ""}`}
          onClick={() => openNoteById(m.id)}
          onKeyDown={(e) => e.key === "Enter" && openNoteById(m.id)}
          {...contextMenu(() => noteMenu(m))}
        >
          <div className="ch-row">
            <Icon name="memo" />
            <span className={`ch-title${m.title ? "" : " untitled"}`}>{m.title || L("無題", "Untitled")}</span>
          </div>
          {m.excerpt && <div className="ch-syn excerpt">{m.excerpt}</div>}
        </div>
      ))}
      <button className="add-row" onClick={() => void newWorkNote(work.id, "material")}>
        <Icon name="plus" />
        {L("資料を追加", "Add material")}
      </button>
    </>
  );
}

/** Shown in the note list in place of the notes, when a work's notebook is open. */
export function WorkPanel({ work }: { work: WorkOverview }) {
  const tab = useWork((s) => s.tab);
  const setTab = (t: WorkTab) => useWork.setState({ tab: t });
  const order: WorkTab[] = ["manuscript", "characters", "plot", "materials"];
  const lastTab = useRef(tab);
  const dirRef = useRef<"forward" | "back">("forward");
  if (lastTab.current !== tab) {
    dirRef.current = order.indexOf(tab) >= order.indexOf(lastTab.current) ? "forward" : "back";
    lastTab.current = tab;
  }
  const tabDir = dirRef.current;
  const tabs: [WorkTab, string, string, number | null][] = [
    ["manuscript", "book", L("原稿", "Text"), work.chapterCount],
    ["characters", "people", L("人物", "Cast"), work.characters.length],
    ["plot", "plot", L("プロット", "Plot"), null],
    ["materials", "memo", L("資料", "Notes"), work.materials.length],
  ];
  return (
    <div className="work-panel">
      <div className="work-head">
        {work.meta.logline && <div className="logline">{work.meta.logline}</div>}
        <Progress work={work} />
        <div className="work-actions">
          <button className="btn small" onClick={() => useWork.setState({ editing: work.id })}>
            <Icon name="pen" />
            {L("作品の情報", "Details")}
          </button>
          <button
            className="btn small"
            onClick={(e) =>
              openMenu(e.currentTarget, [
                { header: L("原稿をまとめて書き出す", "Compile the manuscript") },
                { label: L("テキスト (.txt)", "Text (.txt)"), icon: "download", run: () => compileWork(work.id, "txt") },
                { label: "Markdown (.md)", icon: "download", run: () => compileWork(work.id, "md") },
                { label: L("HTML（ルビ付き・印刷用）", "HTML (with ruby, for print)"), icon: "download", run: () => compileWork(work.id, "html") },
              ])
            }
          >
            <Icon name="download" />
            {L("書き出し", "Compile")}
          </button>
        </div>
      </div>
      <WorkTabs tab={tab}>
        {tabs.map(([id, icon, label, count]) => (
          <button key={id} role="tab" aria-selected={tab === id} className={tab === id ? "active" : ""} onClick={() => setTab(id)}>
            <Icon name={icon} />
            <span>{label}</span>
            {count !== null && count > 0 && <small>{count}</small>}
          </button>
        ))}
      </WorkTabs>
      <div className="list-scroll work-scroll">
        {/* The new tab's content comes in from its side. */}
        <div key={tab} className="tab-body" data-dir={tabDir}>
          {tab === "manuscript" && <ManuscriptTab work={work} />}
          {tab === "characters" && <CharactersTab work={work} />}
          {tab === "plot" && <PlotTab work={work} />}
          {tab === "materials" && <MaterialsTab work={work} />}
        </div>
      </div>
    </div>
  );
}

// -------------------------------------------------------------------------
// The work's details

export function WorkInfoDialog() {
  const editing = useWork((s) => s.editing);
  const fresh = useWork((s) => s.fresh);
  const folders = useStore((s) => s.folders);
  const work = useWork((s) => s.overview);
  const folder = folders.find((f) => f.id === editing);
  const [title, setTitle] = useState("");
  const [meta, setMeta] = useState<WorkMeta>(emptyWorkMeta());
  useEffect(() => {
    if (!editing) return;
    setTitle(folder?.title ?? "");
    if (work?.id === editing) setMeta(work.meta);
    else void api.workOverview(editing).then((o) => setMeta(o.meta), () => setMeta(emptyWorkMeta()));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editing]);
  if (!editing || !folder) return null;
  const close = () => useWork.setState({ editing: null, fresh: false });
  const save = async () => {
    if (title.trim() && title.trim() !== folder.title) {
      await api.renameFolder(folder.id, title.trim()).catch(report);
      await useStore.getState().loadFolders();
    }
    await saveWorkMeta(folder.id, meta);
    close();
  };
  const set = (patch: Partial<WorkMeta>) => setMeta((m) => ({ ...m, ...patch }));
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && close()} onKeyDown={(e) => e.key === "Escape" && close()}>
      <div className="dialog work-dialog">
        <h3>{L("作品の情報", "Work details")}</h3>
        <label className="form-row">
          <span>{L("作品名", "Title")}</span>
          <input className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
        </label>
        <label className="form-row">
          <span>{L("ジャンル", "Genre")}</span>
          <input className="input" value={meta.genre} placeholder={L("青春ミステリー、ファンタジー…", "Mystery, fantasy…")} onChange={(e) => set({ genre: e.target.value })} />
        </label>
        <label className="form-row">
          <span>{L("ひとことで（ログライン）", "In one sentence (logline)")}</span>
          <input className="input" value={meta.logline} placeholder={L("誰が、何を求め、何が立ちはだかるか", "Who wants what, and what stands in the way")} onChange={(e) => set({ logline: e.target.value })} />
        </label>
        <label className="form-row">
          <span>{L("あらすじ", "Synopsis")}</span>
          <textarea className="input" rows={5} value={meta.synopsis} onChange={(e) => set({ synopsis: e.target.value })} />
        </label>
        <div className="form-pair">
          <label className="form-row">
            <span>{L("目標の文字数", "Target length (chars)")}</span>
            <NumberField value={meta.targetChars ?? null} min={0} step={1000} placeholder="100000" allowEmpty onChange={(v) => set({ targetChars: v })} />
          </label>
          <label className="form-row">
            <span>{L("締め切り", "Deadline")}</span>
            <DateField value={meta.deadline ?? null} title={L("締め切り", "Deadline")} onChange={(v) => set({ deadline: v })} />
          </label>
        </div>
        <div className="dialog-foot">
          {folder.isWork && !fresh && (
            <button className="btn link danger-text" onClick={() => void unmakeWork(folder).then(close)}>
              {L("作品として扱うのをやめる", "Stop treating as a work")}
            </button>
          )}
          <span className="grow" />
          <button className="btn" onClick={close}>
            {L("キャンセル", "Cancel")}
          </button>
          <button className="btn primary" onClick={() => void save()}>
            {L("保存", "Save")}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * The tab bar of a work. When the tabs do not fit (a narrow list), it scrolls
 * sideways: by swiping, or with a mouse wheel (turned sideways), and the
 * selected tab is brought into view.
 */
function WorkTabs({ tab, children }: { tab: string; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useInk(ref, "[aria-selected=true]", tab);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>("[aria-selected=true]")?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
  }, [tab]);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      if (el.scrollWidth <= el.clientWidth || Math.abs(e.deltaX) >= Math.abs(e.deltaY)) return;
      e.preventDefault();
      el.scrollLeft += e.deltaY;
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  }, []);
  return (
    <div className="work-tabs inked" role="tablist" ref={ref}>
      <span className="ink" aria-hidden="true" />
      {children}
    </div>
  );
}

// -------------------------------------------------------------------------
// In the editor: a chapter's details

export function ChapterBar({ note, work, readOnly }: { note: Note; work: WorkOverview; readOnly: boolean }) {
  const chapter = useMemo(() => {
    for (const s of work.sections) for (const c of s.chapters) if (c.id === note.id) return c;
    return null;
  }, [work, note.id]);
  const [synopsis, setSynopsis] = useState(chapter?.synopsis ?? "");
  useEffect(() => setSynopsis(chapter?.synopsis ?? ""), [chapter?.id, chapter?.synopsis]);
  if (!chapter) return null;
  const save = (patch: Partial<{ status: ChapterInfo["status"]; synopsis: string; pov: string | null }>) =>
    void setChapterMeta(note.id, { status: chapter.status, synopsis: chapter.synopsis, pov: chapter.pov, ...patch });
  const pov = work.characters.find((c) => c.id === chapter.pov);
  return (
    <div className="chapter-bar">
      <div className="cb-row">
        <div className="seg small">
          {STATUSES.map((s) => (
            <button key={s} disabled={readOnly} className={`${chapter.status === s ? "active " : ""}st-${s}`} onClick={() => save({ status: chapter.status === s ? "" : s })}>
              {statusLabel(s)}
            </button>
          ))}
        </div>
        {work.characters.length > 0 && (
          <button
            className="pov-btn"
            disabled={readOnly}
            onClick={(e) =>
              openMenu(e.currentTarget, [
                { header: L("視点人物", "Point of view") },
                { label: L("なし", "None"), checked: !chapter.pov, run: () => save({ pov: null }) },
                ...work.characters.map((c) => ({ label: c.name, checked: chapter.pov === c.id, run: () => save({ pov: c.id }) })),
              ])
            }
          >
            <Icon name="person" />
            {pov ? L(`視点：${pov.name}`, `POV: ${pov.name}`) : L("視点人物", "Point of view")}
          </button>
        )}
      </div>
      <input
        className="cb-synopsis"
        value={synopsis}
        readOnly={readOnly}
        placeholder={L("この章のあらすじ（一覧に表示されます）", "This chapter in a sentence (shown in the list)")}
        onChange={(e) => setSynopsis(e.target.value)}
        onBlur={() => synopsis !== chapter.synopsis && save({ synopsis })}
        onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && (e.target as HTMLInputElement).blur()}
      />
      {chapter.characters.length > 0 && (
        <div className="cast-row">
          {chapter.characters.map(([id, n]) => {
            const c = work.characters.find((x) => x.id === id);
            if (!c) return null;
            return (
              <button key={id} className="cast-chip" title={L("人物を開く", "Open the character")} onClick={() => openNoteById(id)}>
                <Avatar name={c.name} color={c.color} size={18} />
                {c.name}
                <small>{n}</small>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

// -------------------------------------------------------------------------
// In the editor: a character's sheet

const COLORS = ["#d9622b", "#c25b78", "#9a6fb0", "#6a8caf", "#4f8f94", "#5c8a5a", "#b8873a", "#7a7368"];

function AutoText({ value, onChange, placeholder, readOnly }: { value: string; onChange: (v: string) => void; placeholder?: string; readOnly: boolean }) {
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [value]);
  return <textarea ref={ref} className="input auto" rows={1} value={value} placeholder={placeholder} readOnly={readOnly} onChange={(e) => onChange(e.target.value)} />;
}

export function CharacterSheet({ note, work, readOnly }: { note: Note; work: WorkOverview | null; readOnly: boolean }) {
  const [c, setC] = useState<Character | null>(null);
  const [aliases, setAliases] = useState("");
  const [open, setOpen] = useState(true);
  useEffect(() => {
    setC(null);
    void api.getCharacter(note.id).then((x) => {
      setC(x);
      setAliases(x.aliases.join("、"));
    }, report);
  }, [note.id]);
  useDebouncedSave(c, (value) => {
    void api.setCharacter(note.id, value).then(() => reloadWork(), report);
  });
  if (!c) return <SheetSkeleton />;
  const set = (patch: Partial<Character>) => setC((x) => (x ? { ...x, ...patch } : x));
  const info = work?.characters.find((x) => x.id === note.id);
  const others = (work?.characters ?? []).filter((x) => x.id !== note.id);
  const chapters = new Map<string, string>();
  for (const s of work?.sections ?? []) for (const ch of s.chapters) chapters.set(ch.id, ch.title);
  const field = (key: keyof Character, label: string, placeholder?: string) => (
    <label className="cs-field">
      <span>{label}</span>
      <AutoText value={c[key] as string} placeholder={placeholder} readOnly={readOnly} onChange={(v) => set({ [key]: v } as Partial<Character>)} />
    </label>
  );
  return (
    <div className={`character-sheet${open ? "" : " folded"}`} style={{ ["--char-color" as string]: characterColor(note.title, c.color) }}>
      <button className="cs-toggle" onClick={() => setOpen(!open)}>
        <Icon name={open ? "chevronDown" : "chevronRight"} />
        {L("人物設定", "Character sheet")}
        {c.role && <span className={`role-chip role-${c.role}`}>{roleLabel(c.role)}</span>}
      </button>
      {open && (
        <>
          <div className="cs-grid">
            <label className="cs-field">
              <span>{L("読み", "Reading")}</span>
              <input className="input" value={c.ruby} readOnly={readOnly} placeholder={L("ふりがな", "Pronunciation")} onChange={(e) => set({ ruby: e.target.value })} />
            </label>
            <label className="cs-field">
              <span>{L("役割", "Role")}</span>
              <Select
                value={c.role}
                disabled={readOnly}
                options={[["" as Character["role"], "—"], ...ROLES.map((r) => [r, roleLabel(r)] as const)]}
                onChange={(v) => set({ role: v })}
              />
            </label>
            <label className="cs-field">
              <span>{L("年齢", "Age")}</span>
              <input className="input" value={c.age} readOnly={readOnly} onChange={(e) => set({ age: e.target.value })} />
            </label>
            <label className="cs-field">
              <span>{L("性別", "Gender")}</span>
              <input className="input" value={c.gender} readOnly={readOnly} onChange={(e) => set({ gender: e.target.value })} />
            </label>
          </div>
          <label className="cs-field">
            <span>{L("別名・呼び名（読点で区切る）", "Other names (comma-separated)")}</span>
            <input
              className="input"
              value={aliases}
              readOnly={readOnly}
              placeholder={L("あだ名、名字、「わたし」など本文での呼ばれ方", "Nicknames, family name, how the text calls them")}
              onChange={(e) => {
                setAliases(e.target.value);
                set({ aliases: e.target.value.split(/[、,，]/).map((s) => s.trim()).filter(Boolean) });
              }}
            />
          </label>
          <div className="cs-field">
            <span>{L("色", "Colour")}</span>
            <div className="swatches">
              {COLORS.map((col) => (
                <button key={col} disabled={readOnly} className={`swatch${characterColor(note.title, c.color) === col ? " on" : ""}`} style={{ background: col }} onClick={() => set({ color: col })} aria-label={col} />
              ))}
            </div>
          </div>
          {field("appearance", L("外見", "Appearance"), L("背格好、服装、特徴", "Build, clothes, features"))}
          {field("personality", L("性格", "Personality"), L("長所と短所、口癖", "Strengths, flaws, ways of speaking"))}
          {field("background", L("経歴・背景", "Background"))}
          {field("motivation", L("目的・動機", "Goal and motivation"), L("何を求め、何を恐れているか", "What they want, and what they fear"))}
          <div className="cs-field">
            <span>{L("人間関係", "Relationships")}</span>
            {c.relations.map((r, i) => (
              <div key={i} className="relation-row">
                <Select
                  value={r.target}
                  disabled={readOnly}
                  options={[["", "—"], ...others.map((o) => [o.id, o.name] as const)]}
                  onChange={(v) => set({ relations: c.relations.map((x, j) => (j === i ? { ...x, target: v } : x)) })}
                />
                <input
                  className="input"
                  value={r.label}
                  readOnly={readOnly}
                  placeholder={L("関係（母、幼なじみ、宿敵…）", "Relation (mother, rival…)")}
                  onChange={(e) => set({ relations: c.relations.map((x, j) => (j === i ? { ...x, label: e.target.value } : x)) })}
                />
                {!readOnly && (
                  <button className="icon-btn small" title={L("削除", "Remove")} onClick={() => set({ relations: c.relations.filter((_, j) => j !== i) })}>
                    <Icon name="x" />
                  </button>
                )}
              </div>
            ))}
            {!readOnly && others.length > 0 && (
              <button className="btn link" onClick={() => set({ relations: [...c.relations, { target: others[0].id, label: "" }] })}>
                <Icon name="plus" />
                {L("関係を追加", "Add a relationship")}
              </button>
            )}
          </div>
          {info && (
            <div className="cs-field">
              <span>{L("登場する章", "Appears in")}</span>
              {info.chapters.length === 0 ? (
                <small className="muted">{L("名前（または別名）が本文に出てくると、ここに並びます。", "Chapters mentioning the name (or another name) are listed here.")}</small>
              ) : (
                <div className="cast-row">
                  {info.chapters.map((id) => (
                    <button key={id} className="cast-chip" onClick={() => openNoteById(id)}>
                      <Icon name="book" />
                      {chapters.get(id) || L("無題の章", "Untitled chapter")}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
          <div className="cs-note">{L("下の本文は自由なメモ欄です。", "The text below is for free notes.")}</div>
        </>
      )}
    </div>
  );
}

// -------------------------------------------------------------------------
// In the editor: the plot board

function CardEditor({ card, chapters, readOnly, onChange, onDelete }: { card: PlotCard; chapters: ChapterInfo[]; readOnly: boolean; onChange: (c: PlotCard) => void; onDelete: () => void }) {
  const [editing, setEditing] = useState(!card.title);
  const chapter = chapters.find((c) => c.id === card.chapter);
  if (editing && !readOnly) {
    return (
      <div className="plot-card editing">
        <input className="input" autoFocus value={card.title} placeholder={L("出来事", "What happens")} onChange={(e) => onChange({ ...card, title: e.target.value })} />
        <AutoText value={card.text} placeholder={L("詳しく（伏線、感情の動き…）", "Details (foreshadowing, feelings…)")} readOnly={false} onChange={(v) => onChange({ ...card, text: v })} />
        <Select
          value={card.chapter ?? ""}
          options={[["", L("章に結びつけない", "No chapter")], ...chapters.map((c, i) => [c.id, `${i + 1}. ${c.title || L("無題の章", "Untitled chapter")}`] as const)]}
          onChange={(v) => onChange({ ...card, chapter: v || null })}
        />
        <div className="card-actions">
          <button className="btn link danger-text" onClick={onDelete}>
            {L("削除", "Delete")}
          </button>
          <span className="grow" />
          <button className="btn small primary" onClick={() => setEditing(false)}>
            {L("完了", "Done")}
          </button>
        </div>
      </div>
    );
  }
  return (
    <div
      className={`plot-card${card.done ? " done" : ""}`}
      draggable={!readOnly}
      onDragStart={(e) => {
        e.dataTransfer.setData("application/x-nisshoku-card", card.id);
        e.dataTransfer.effectAllowed = "move";
        dragChip(e, card.title || L("出来事", "What happens"));
      }}
      onDoubleClick={() => setEditing(true)}
    >
      <div className="card-top">
        <input type="checkbox" checked={card.done} disabled={readOnly} title={L("書き終えた", "Written")} onChange={(e) => onChange({ ...card, done: e.target.checked })} />
        <b className="grow">{card.title || L("（無題）", "(untitled)")}</b>
        {!readOnly && (
          <button className="icon-btn small" title={L("編集", "Edit")} onClick={() => setEditing(true)}>
            <Icon name="pen" />
          </button>
        )}
      </div>
      {card.text && <div className="card-text">{card.text}</div>}
      {chapter && (
        <button className="cast-chip" onClick={() => openNoteById(chapter.id)}>
          <Icon name="book" />
          {chapter.title || L("無題の章", "Untitled chapter")}
        </button>
      )}
    </div>
  );
}

export function PlotBoard({ note, work, readOnly }: { note: Note; work: WorkOverview; readOnly: boolean }) {
  const [plot, setPlot] = useState<Plot | null>(null);
  const [overCol, setOverCol] = useState<string | null>(null);
  const [overCard, setOverCard] = useState<string | null>(null);
  useEffect(() => {
    setPlot(null);
    void api.openPlot(work.id).then((d) => setPlot(d.plot), report);
  }, [note.id, work.id]);
  useDebouncedSave(plot, (p) => void api.setPlot(note.id, p).then(() => reloadWork(), report));
  const chapters = useMemo(() => work.sections.flatMap((s) => s.chapters), [work]);
  if (!plot) return <BoardSkeleton />;
  const update = (p: Plot) => setPlot(p);
  const moveCard = (cardId: string, column: string, before: string | null) => {
    const card = plot.cards.find((c) => c.id === cardId);
    if (!card) return;
    const rest = plot.cards.filter((c) => c.id !== cardId);
    const moved = { ...card, column };
    const at = before ? rest.findIndex((c) => c.id === before) : -1;
    if (at >= 0) rest.splice(at, 0, moved);
    else rest.push(moved);
    update({ ...plot, cards: rest });
  };
  const done = plot.cards.filter((c) => c.done).length;
  return (
    <div className="plot-board">
      <div className="plot-head">
        <span>{L(`${plot.cards.length} 枚のカード · ${done} 枚書き終えた`, `${plot.cards.length} cards · ${done} written`)}</span>
        <span className="grow" />
        {!readOnly && (
          <button className="btn small" onClick={() => update({ ...plot, columns: [...plot.columns, { id: newId(), title: L("新しい列", "New column") }] })}>
            <Icon name="plus" />
            {L("列を追加", "Add a column")}
          </button>
        )}
      </div>
      <div className="plot-columns">
        {plot.columns.map((col) => {
          const cards = plot.cards.filter((c) => c.column === col.id);
          return (
            <div
              key={col.id}
              className={`plot-column${overCol === col.id ? " over" : ""}`}
              onDragOver={(e) => {
                if (e.dataTransfer.types.includes("application/x-nisshoku-card")) {
                  e.preventDefault();
                  setOverCol(col.id);
                }
              }}
              onDragLeave={() => setOverCol(null)}
              onDrop={(e) => {
                const id = e.dataTransfer.getData("application/x-nisshoku-card");
                setOverCol(null);
                setOverCard(null);
                if (id) moveCard(id, col.id, overCard);
              }}
            >
              <div className="col-head">
                <input
                  className="col-title"
                  value={col.title}
                  readOnly={readOnly}
                  onChange={(e) => update({ ...plot, columns: plot.columns.map((c) => (c.id === col.id ? { ...c, title: e.target.value } : c)) })}
                />
                <small>{cards.length}</small>
                {!readOnly && (
                  <button
                    className="icon-btn small"
                    title={L("列のメニュー", "Column menu")}
                    onClick={(e) =>
                      openMenu(e.currentTarget, [
                        {
                          label: L("左へ移動", "Move left"),
                          icon: "chevronLeft",
                          disabled: plot.columns[0].id === col.id,
                          run: () => {
                            const cols = [...plot.columns];
                            const i = cols.findIndex((c) => c.id === col.id);
                            [cols[i - 1], cols[i]] = [cols[i], cols[i - 1]];
                            update({ ...plot, columns: cols });
                          },
                        },
                        {
                          label: L("右へ移動", "Move right"),
                          icon: "chevronRight",
                          disabled: plot.columns[plot.columns.length - 1].id === col.id,
                          run: () => {
                            const cols = [...plot.columns];
                            const i = cols.findIndex((c) => c.id === col.id);
                            [cols[i + 1], cols[i]] = [cols[i], cols[i + 1]];
                            update({ ...plot, columns: cols });
                          },
                        },
                        "sep",
                        {
                          label: L("列を削除（カードも消えます）", "Delete the column and its cards"),
                          icon: "trash",
                          danger: true,
                          disabled: plot.columns.length <= 1,
                          run: () => update({ ...plot, columns: plot.columns.filter((c) => c.id !== col.id), cards: plot.cards.filter((c) => c.column !== col.id) }),
                        },
                      ])
                    }
                  >
                    <Icon name="more" />
                  </button>
                )}
              </div>
              {cards.map((card) => (
                <div
                  key={card.id}
                  className={overCard === card.id ? "drop-before" : ""}
                  onDragOver={(e) => {
                    if (e.dataTransfer.types.includes("application/x-nisshoku-card")) setOverCard(card.id);
                  }}
                >
                  <CardEditor
                    card={card}
                    chapters={chapters}
                    readOnly={readOnly}
                    onChange={(c) => update({ ...plot, cards: plot.cards.map((x) => (x.id === c.id ? c : x)) })}
                    onDelete={() => update({ ...plot, cards: plot.cards.filter((x) => x.id !== card.id) })}
                  />
                </div>
              ))}
              {!readOnly && (
                <button className="add-row" onClick={() => update({ ...plot, cards: [...plot.cards, { id: newId(), column: col.id, title: "", text: "", chapter: null, done: false }] })}>
                  <Icon name="plus" />
                  {L("カードを追加", "Add a card")}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
