import { useEffect, useState } from "react";
import { api, type NoteSummary } from "../api";
import { report, useStore } from "../store";
import { t, useT } from "../i18n";
import { formatNumber, shortDate } from "../lib/text";
import { Icon } from "./Icon";
import { dragChip } from "../lib/drag";
import { DiaryPanel } from "./Diary";
import { dialogs } from "./Dialogs";
import { Resizer, WindowControls } from "./Chrome";
import { contextMenu, openMenu } from "./ContextMenu";
import { clipWebPage, newNotebook, noteMenu, pickTemplate } from "../lib/actions";
import { FOCUS_TASK_INPUT, TaskList } from "./Tasks";
import { dueLabel, isOverdue } from "../lib/tasks";
import { useWork, workIdOf } from "../lib/works";
import { WorkPanel } from "./Work";
import { NoteListSkeleton, WorkPanelSkeleton } from "./Skeleton";
import { Fab, PullToSync, SwipeRow } from "./MobileUI";
import { isMobile } from "../lib/mobile";

/**
 * In a notebook with parents or sub-notebooks: the way up, and its
 * sub-notebooks, to move through deep nesting from the list too.
 */
function SubNotebooks({ folderId }: { folderId: string }) {
  const folders = useStore((s) => s.folders);
  const here = folders.find((f) => f.id === folderId);
  const parent = here && folders.find((f) => f.id === here.parentId);
  const kids = folders.filter((f) => f.parentId === folderId && !f.isDiary);
  if (!parent && kids.length === 0) return null;
  const go = (id: string) => void useStore.getState().setScope({ kind: "folder", id });
  return (
    <div className="sub-notebooks">
      {parent && (
        <button className="sub-nb up" title={t("list.upTo", { name: parent.title })} onClick={() => go(parent.id)}>
          <Icon name="up" />
          <span>{parent.title || t("folder.untitled")}</span>
        </button>
      )}
      {kids.map((f) => (
        <button key={f.id} className="sub-nb" onClick={() => go(f.id)}>
          {f.icon ? <span className="emoji">{f.icon}</span> : <Icon name={f.isWork ? "work" : "folder"} style={f.color ? { color: f.color } : undefined} />}
          <span>{f.title || t("folder.untitled")}</span>
          {f.noteCount > 0 && <small>{f.noteCount}</small>}
        </button>
      ))}
    </div>
  );
}

function scopeTitle(): string {
  const { scope, folders } = useStore.getState();
  switch (scope.kind) {
    case "all":
      return t("nav.allNotes");
    case "diary":
      return folders.find((f) => f.isDiary)?.title ?? t("nav.diary");
    case "trash":
      return t("nav.trash");
    case "conflicts":
      return t("nav.conflicts");
    case "search":
      return t("cmd.search");
    case "folder":
      return folders.find((f) => f.id === scope.id)?.title ?? t("nav.notebooks");
    case "tag":
      return `# ${useStore.getState().tags.find((x) => x.id === scope.id)?.title ?? ""}`;
    case "pinned":
      return t("nav.pinned");
    case "todo":
      return t("nav.todo");
  }
}

function NoteItem({ note, active, draggable: reorder, onReorder, index }: { note: NoteSummary; active: boolean; draggable: boolean; onReorder: (dragged: string, before: string) => void; index: number }) {
  const [over, setOver] = useState(false);
  // Once in, it stays: a later class change (selecting it) must not replay it.
  const [entered, setEntered] = useState(index >= 16);
  const scope = useStore((s) => s.scope);
  const folders = useStore((s) => s.folders);
  const showExcerpt = useStore((s) => s.prefs.showExcerpt);
  const folder = scope.kind === "all" || scope.kind === "search" ? folders.find((f) => f.id === note.parentId) : undefined;
  return (
    <div
      role="button"
      tabIndex={0}
      // The first rows come in one after the other (styles/motion.css); the
      // rest, off screen anyway, at once.
      className={`note-item${active ? " active" : ""}${over ? " drop-before" : ""}${note.todoCompleted ? " done" : ""}${entered ? "" : " enter"}`}
      style={entered ? undefined : { ["--i" as string]: index }}
      onAnimationEnd={(e) => e.animationName === "row-in" && setEntered(true)}
      onClick={() => useStore.getState().select(note.id)}
      onKeyDown={(e) => e.key === "Enter" && useStore.getState().select(note.id)}
      {...contextMenu(() => noteMenu(note))}
      // Not on phones: there a long press would start a drag that fights the
      // swipe and the long-press menu (moving is in the menu).
      draggable={!isMobile}
      onDragStart={(e) => {
        e.dataTransfer.setData("application/x-nisshoku-note", note.id);
        e.dataTransfer.effectAllowed = "move";
        dragChip(e, note.title || t("list.untitled"));
      }}
      onDragOver={(e) => {
        if (reorder && e.dataTransfer.types.includes("application/x-nisshoku-note")) {
          e.preventDefault();
          setOver(true);
        }
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        setOver(false);
        const id = e.dataTransfer.getData("application/x-nisshoku-note");
        if (reorder && id && id !== note.id) {
          e.preventDefault();
          e.stopPropagation();
          onReorder(id, note.id);
        }
      }}
    >
      <div className="t-row">
        {note.isTodo && (
          <input
            type="checkbox"
            className="todo-check"
            checked={note.todoCompleted}
            title={t("menu.todoDone")}
            onClick={(e) => e.stopPropagation()}
            onChange={async (e) => {
              await api.setTodoCompleted(note.id, e.target.checked).catch(report);
              await useStore.getState().loadNotes();
              useStore.getState().reloadNoteIf(note.id);
            }}
          />
        )}
        {note.pinned && <Icon name="pin" className="pin-mark" />}
        <div className={`t${note.title ? "" : " untitled"}`}>{note.title || t("list.untitled")}</div>
        <button
          className="item-more"
          title={t("editor.more")}
          onClick={(e) => {
            e.stopPropagation();
            openMenu(e.currentTarget, noteMenu(note));
          }}
        >
          <Icon name="more" />
        </button>
      </div>
      {showExcerpt && note.excerpt && <div className="x">{note.excerpt}</div>}
      <div className="m">
        {note.isTodo && !note.todoCompleted && note.todoDue > 0 ? (
          <span className={`due${isOverdue({ due: note.todoDue, allDay: note.dueAllDay }) ? " overdue" : ""}`}>{dueLabel({ due: note.todoDue, allDay: note.dueAllDay })}</span>
        ) : (
          <span>{shortDate(note.updatedTime)}</span>
        )}
        <span>
          {formatNumber(note.charCount)} {t("today.unit")}
        </span>
        {note.goal ? <span className="goal">{Math.min(100, Math.round((note.charCount / note.goal) * 100))}%</span> : null}
        {folder && <span className="folder-name">{folder.title}</span>}
        {note.isConflict && <span className="badge">{t("list.conflict")}</span>}
      </div>
      {note.tags.length > 0 && (
        <div className="tag-row">
          {note.tags.map((tag) => (
            <span key={tag} className="tag-chip">
              {tag}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

/** Phones: the list syncs when pulled down. */
function MaybePull({ children }: { children: React.ReactNode }) {
  return isMobile ? <PullToSync>{children}</PullToSync> : <>{children}</>;
}

export function NoteList() {
  const t = useT();
  const open = useStore((s) => s.listOpen);
  const notes = useStore((s) => s.notes);
  const scope = useStore((s) => s.scope);
  const selectedId = useStore((s) => s.selectedId);
  const sort = useStore((s) => s.prefs.sortNotes);
  const compact = useStore((s) => s.prefs.compactList);
  const layout = useStore((s) => s.layout);
  useStore((s) => s.folders);
  const workOverview = useWork((s) => s.overview);
  const workId = scope.kind === "folder" ? workIdOf(scope.id) : null;
  const work = workId && workOverview && workId === workOverview.id ? workOverview : null;
  // A work whose overview is on its way: the panel's shape, not the plain list.
  const workLoading = !!workId && !work;
  // Notes of the scope before (or none yet, at startup): placeholders instead.
  // (A search keeps its results while the next query runs: no flicker per key.)
  const notesLoading = useStore((s) => s.notesScope !== JSON.stringify(s.scope) && !(s.scope.kind === "search" && s.notesScope?.startsWith('{"kind":"search"')));
  const [query, setQuery] = useState(scope.kind === "search" ? scope.query : "");

  useEffect(() => {
    if (scope.kind !== "search") return;
    const timer = window.setTimeout(() => void useStore.getState().setScope({ kind: "search", query }), 200);
    return () => window.clearTimeout(timer);
  }, [query]);

  const total = notes.reduce((n, x) => n + x.charCount, 0);
  const empty =
    scope.kind === "trash" ? t("list.trashEmpty")
    : scope.kind === "search" ? (query ? t("list.notFound") : t("list.typeQuery"))
    : t("list.empty");

  return (
    <section
      className={`note-list${open ? "" : " closed"}${compact ? " compact" : ""}`}
      {...contextMenu(() => [
        { label: t("list.new"), icon: "pen", hint: "Ctrl N", run: () => useStore.getState().newNote() },
        { label: t("menu.fromTemplate"), icon: "template", run: () => pickTemplate() },
        { label: t("menu.webClip"), icon: "globe", run: () => clipWebPage() },
        { label: t("nav.newNotebook"), icon: "folderPlus", run: () => newNotebook() },
        "sep",
        {
          label: t("list.sort"),
          icon: "list",
          submenu: (["custom", "updated", "created", "title"] as const).map((k) => ({
            label: t(`sort.${k}`),
            checked: sort === k,
            run: () => {
              useStore.getState().setPrefs({ sortNotes: k });
              void useStore.getState().loadNotes();
            },
          })),
        },
        { label: t("settings.compactList"), checked: compact, run: () => useStore.getState().setPrefs({ compactList: !compact }) },
        { label: t("settings.showExcerpt"), checked: useStore.getState().prefs.showExcerpt, run: () => useStore.getState().setPrefs({ showExcerpt: !useStore.getState().prefs.showExcerpt }) },
      ])}
    >
      <div className="list-head" data-tauri-drag-region>
        {layout === "narrow" && (
          <button className="icon-btn back-btn" title={t("nav.back")} onClick={() => useStore.getState().set({ mobilePane: "nav" })}>
            <Icon name="back" />
          </button>
        )}
        <span className="list-title" data-tauri-drag-region>
          {work ? work.title : scopeTitle()}
        </span>
        {scope.kind !== "trash" && scope.kind !== "conflicts" && scope.kind !== "diary" && scope.kind !== "todo" && (
          <button
            className="icon-btn"
            title={t("list.sort")}
            aria-haspopup="menu"
            onClick={(e) =>
              openMenu(
                e.currentTarget,
                (["custom", "updated", "created", "title"] as const).map((k) => ({
                  label: t(`sort.${k}`),
                  checked: sort === k,
                  run: () => {
                    useStore.getState().setPrefs({ sortNotes: k });
                    void useStore.getState().loadNotes();
                  },
                })),
              )
            }
          >
            <Icon name="list" />
          </button>
        )}
        {scope.kind === "trash" ? (
          <button
            className="btn danger"
            style={{ height: 28 }}
            disabled={notes.length === 0}
            onClick={async () => {
              if (!(await dialogs.confirm(t("list.emptyTrashConfirm"), t("list.emptyTrashDetail"), { okLabel: t("list.emptyTrashOk"), danger: true }))) return;
              const n = await api.emptyTrash().catch(report);
              useStore.getState().select(null);
              await useStore.getState().refresh();
              useStore.getState().toast(t("list.deleted", { n: n ?? 0 }));
            }}
          >
            {t("list.emptyTrash")}
          </button>
        ) : (
          <button
            className="icon-btn"
            data-new
            title={scope.kind === "diary" ? t("diary.writeToday") : scope.kind === "todo" ? t("tasks.add") : `${t("list.new")} (Ctrl+N)`}
            onClick={() => {
              if (scope.kind === "todo") window.dispatchEvent(new Event(FOCUS_TASK_INPUT));
              else void (scope.kind === "diary" ? useStore.getState().openDiary() : useStore.getState().newNote());
            }}
          >
            <Icon name="plus" />
          </button>
        )}
        {layout !== "narrow" ? (
          <button className="icon-btn" title={`${t("list.hide")} (Ctrl+Shift+\\)`} onClick={() => useStore.getState().set({ listOpen: false })}>
            <Icon name="listHide" />
          </button>
        ) : (
          <WindowControls />
        )}
      </div>
      {scope.kind === "search" && (
        <div className="search-box">
          <Icon name="search" />
          <input className="input" autoFocus placeholder={t("list.searchPlaceholder")} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
      )}
      {work && <WorkPanel work={work} />}
      {workLoading && <WorkPanelSkeleton />}
      {!work && scope.kind === "diary" && (
        <div className="list-scroll">
          <DiaryPanel />
        </div>
      )}
      {!workId && scope.kind === "folder" && <SubNotebooks folderId={scope.id} />}
      {!workId && scope.kind !== "diary" && <div className={`list-sub${notesLoading ? " pending" : ""}`}>
        <span>{scope.kind === "todo" ? t("tasks.open", { n: notes.filter((n) => !n.todoCompleted).length }) : t("list.count", { n: notes.length })}</span>
        {scope.kind !== "trash" && scope.kind !== "todo" && (
          <span>
            {formatNumber(total)} {t("today.unit")}
          </span>
        )}
      </div>}
      {!workId && scope.kind !== "diary" && notesLoading && (
        <div className="list-scroll">
          <NoteListSkeleton />
        </div>
      )}
      {!workId && scope.kind !== "diary" && !notesLoading && <MaybePull><div className="list-scroll" key={scope.kind === "search" ? "search" : JSON.stringify(scope)}>
        {scope.kind === "todo" && <TaskList tasks={notes} />}
        {scope.kind !== "todo" && notes.length === 0 && (
          <div className="empty">{empty}</div>
        )}
        {scope.kind !== "todo" && notes.map((n, i) => isMobile ? (
          <SwipeRow key={n.id} note={n}>
            <NoteItem
              index={i}
              note={n}
              active={n.id === selectedId}
              draggable={false}
              onReorder={() => {}}
            />
          </SwipeRow>
        ) : (
          <NoteItem
            key={n.id}
            index={i}
            note={n}
            active={n.id === selectedId}
            draggable={sort === "custom" && scope.kind === "folder"}
            onReorder={async (dragged, before) => {
              const rest = notes.filter((x) => x.id !== dragged);
              const at = rest.findIndex((x) => x.id === before);
              // Between its new neighbours (higher order = higher up): only the
              // dragged note changes. When they leave no room (equal orders, as
              // notes from Joplin often have), the whole list is numbered again.
              const above = rest[at - 1]?.order;
              const below = rest[at].order;
              const order = above === undefined ? below + 1000 : (above + below) / 2;
              if (order > below && (above === undefined || order < above)) await api.setNoteOrder(dragged, order).catch(report);
              else {
                const ids = rest.map((x) => x.id);
                ids.splice(at, 0, dragged);
                await api.setNotesOrder(ids).catch(report);
              }
              await useStore.getState().loadNotes();
            }}
          />
        ))}
      </div></MaybePull>}
      {isMobile && layout === "narrow" && !workId && <Fab />}
      {layout !== "narrow" && <Resizer pref="listWidth" min={240} max={520} />}
    </section>
  );
}
