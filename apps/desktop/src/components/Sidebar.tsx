import { useEffect, useRef, useState } from "react";
import { api, type NoteComment, type Scope } from "../api";
import { report, useStore, type SidebarTab } from "../store";
import { useT, type Key } from "../i18n";
import { formatNumber, relativeTime } from "../lib/text";
import { Icon } from "./Icon";
import { NotebookTree } from "./FolderTree";
import { peekSidebarProps } from "../lib/peek";
import { dialogs } from "./Dialogs";
import { WindowControls } from "./Chrome";
import { contextMenu } from "./ContextMenu";
import { newNotebook } from "../lib/actions";
import { useInk } from "../lib/motion";

function sameScope(a: Scope, b: Scope) {
  return a.kind === b.kind && (a.kind !== "folder" || (b.kind === "folder" && a.id === b.id));
}

function NavItem({ icon, label, count, scope, onClick }: { icon: string; label: string; count?: number; scope?: Scope; onClick?: () => void }) {
  const current = useStore((s) => s.scope);
  const active = scope ? sameScope(scope, current) : false;
  return (
    <button className={`nav-item${active ? " active" : ""}`} onClick={onClick ?? (() => scope && void useStore.getState().setScope(scope))}>
      <Icon name={icon} />
      <span className="label">{label}</span>
      {count !== undefined && count > 0 && <span className="count">{count}</span>}
    </button>
  );
}

// -------------------------------------------------------------------------
// Notebooks

/** Pinned notes, at the top of the notebooks tab. */
function PinnedNotes() {
  const t = useT();
  const [pinned, setPinned] = useState<{ id: string; title: string }[]>([]);
  const notes = useStore((s) => s.notes);
  const selectedId = useStore((s) => s.selectedId);
  useEffect(() => {
    api.listNotes({ kind: "pinned" }).then((n) => setPinned(n.map((x) => ({ id: x.id, title: x.title }))), report);
  }, [notes]);
  if (!pinned.length) return null;
  return (
    <>
      <div className="section-head">
        <span className="grow">{t("nav.pinned")}</span>
      </div>
      {pinned.map((n) => (
        <button
          key={n.id}
          className={`nav-item pinned-item${n.id === selectedId ? " active" : ""}`}
          onClick={async () => {
            const s = useStore.getState();
            if (!s.notes.some((x) => x.id === n.id)) await s.setScope({ kind: "pinned" });
            useStore.getState().select(n.id);
          }}
          {...contextMenu(() => [
            { label: t("menu.unpin"), icon: "pin", run: async () => {
                await api.setNotePinned(n.id, false).catch(report);
                await useStore.getState().loadNotes();
              } },
          ])}
        >
          <Icon name="pin" />
          <span className="label">{n.title || t("list.untitled")}</span>
        </button>
      ))}
    </>
  );
}

/** Tags with their note counts. */
function TagList() {
  const t = useT();
  const tags = useStore((s) => s.tags);
  const scope = useStore((s) => s.scope);
  if (!tags.length) return null;
  return (
    <>
      <div className="section-head">
        <span className="grow">{t("nav.tags")}</span>
      </div>
      <div className="tag-cloud">
        {tags.map((tag) => (
          <button
            key={tag.id}
            className={`tag-chip big${scope.kind === "tag" && scope.id === tag.id ? " active" : ""}`}
            onClick={() => void useStore.getState().setScope({ kind: "tag", id: tag.id })}
            {...contextMenu(() => [
              { label: t("folder.rename"), icon: "pen", run: async () => {
                  const title = await dialogs.prompt(t("tags.rename"), { initial: tag.title });
                  if (!title) return;
                  await api.renameTag(tag.id, title).catch(report);
                  await useStore.getState().refresh();
                } },
              "sep",
              { label: t("comments.delete"), icon: "trash", danger: true, run: async () => {
                  if (!(await dialogs.confirm(t("tags.deleteConfirm", { name: tag.title }), t("tags.deleteDetail"), { okLabel: t("comments.delete"), danger: true }))) return;
                  await api.deleteTag(tag.id).catch(report);
                  if (scope.kind === "tag" && scope.id === tag.id) await useStore.getState().setScope({ kind: "all" });
                  await useStore.getState().refresh();
                } },
            ])}
          >
            #{tag.title}
            <small>{tag.noteCount}</small>
          </button>
        ))}
      </div>
    </>
  );
}

/** Shown only while there are conflict copies (made by syncs). */
function ConflictsItem() {
  const t = useT();
  const [count, setCount] = useState(0);
  const notes = useStore((s) => s.notes);
  const lastSync = useStore((s) => s.syncStatus?.lastSync);
  useEffect(() => {
    api.listNotes({ kind: "conflicts" }).then((n) => setCount(n.length), report);
  }, [notes, lastSync]);
  if (!count) return null;
  return <NavItem icon="conflict" label={t("nav.conflicts")} count={count} scope={{ kind: "conflicts" }} />;
}

function NotebooksTab() {
  const t = useT();
  const totalNotes = useStore((s) => s.allNotes);
  const openTasks = useStore((s) => s.openTasks);
  return (
    <>
      <NavItem icon="notes" label={t("nav.allNotes")} count={totalNotes} scope={{ kind: "all" }} />
      <PinnedNotes />
      <NavItem icon="todo" label={t("nav.todo")} count={openTasks} scope={{ kind: "todo" }} />
      <NavItem icon="diary" label={t("nav.diary")} scope={{ kind: "diary" }} />
      <NotebookTree />
      <TagList />
      <div className="section-sep" />
      <ConflictsItem />
      <NavItem icon="trash" label={t("nav.trash")} scope={{ kind: "trash" }} />
    </>
  );
}

// -------------------------------------------------------------------------
// Comments

/** Opens a note (switching to a list that has it) with a comment selected. */
export async function openComment(c: { noteId: string; id: string }) {
  const s = useStore.getState();
  if (!s.notes.some((n) => n.id === c.noteId)) {
    const note = await api.getNote(c.noteId).catch(report);
    if (!note) return;
    await s.setScope({ kind: "folder", id: note.parentId });
  }
  useStore.getState().set({ selectedId: c.noteId, panel: "comments", activeCommentId: c.id, focusMode: false });
}

function CommentsTab() {
  const t = useT();
  const version = useStore((s) => s.commentsVersion);
  const active = useStore((s) => s.activeCommentId);
  const [comments, setComments] = useState<NoteComment[]>([]);
  const [showResolved, setShowResolved] = useState(false);
  useEffect(() => {
    api.listComments(null).then(setComments).catch(report);
  }, [version]);
  const shown = comments.filter((c) => showResolved || !c.resolved);

  return (
    <div className="comment-list">
      <label className="check-row">
        <input type="checkbox" checked={showResolved} onChange={(e) => setShowResolved(e.target.checked)} />
        {t("comments.showResolved")}
      </label>
      {shown.length === 0 && <div className="empty small">{t("comments.emptyAll")}</div>}
      {shown.map((c) => (
        <button
          key={c.id}
          className={`comment-card${c.id === active ? " active" : ""}${c.resolved ? " resolved" : ""}`}
          onClick={() => void openComment(c)}
          {...contextMenu(() => [
            { label: t("menu.open"), icon: "note", run: () => openComment(c) },
            { label: c.resolved ? t("comments.reopen") : t("comments.resolve"), icon: "check", run: async () => {
                await api.updateComment(c.noteId, c.id, { resolved: !c.resolved }).catch(report);
                useStore.setState((s) => ({ commentsVersion: s.commentsVersion + 1 }));
              } },
            "sep",
            { label: t("comments.delete"), icon: "trash", danger: true, run: async () => {
                if (!(await dialogs.confirm(t("comments.deleteConfirm"), undefined, { okLabel: t("comments.delete"), danger: true }))) return;
                await api.deleteComment(c.noteId, c.id).catch(report);
                useStore.setState((s) => ({ commentsVersion: s.commentsVersion + 1 }));
              } },
          ])}
        >
          <div className="comment-note">{c.noteTitle || t("list.untitled")}</div>
          <div className="comment-quote">{c.quote}</div>
          <div className="comment-text">{c.text}</div>
          <div className="comment-time">
            {relativeTime(c.createdTime)}
            {c.resolved && ` ・ ${t("comments.resolved")}`}
          </div>
        </button>
      ))}
    </div>
  );
}

// -------------------------------------------------------------------------

function TodayCard() {
  const t = useT();
  const stats = useStore((s) => s.stats);
  const dailyGoal = useStore((s) => s.prefs.dailyGoal);
  if (!stats) return null;
  const max = Math.max(1, ...stats.days.map(([, v]) => v));
  return (
    <div className="today-card">
      <div className="row">
        <span>{t("today.label")}</span>
        <span>{stats.streak > 0 ? t("today.streak", { n: stats.streak }) : "　"}</span>
      </div>
      <div className="row" style={{ marginTop: 2 }}>
        <span className="big">
          {formatNumber(stats.today)}
          <small> {t("today.unit")}</small>
        </span>
        {dailyGoal > 0 && <span>/ {formatNumber(dailyGoal)}</span>}
      </div>
      {dailyGoal > 0 ? (
        <div className="progress">
          <div style={{ width: `${Math.min(100, (stats.today / dailyGoal) * 100)}%` }} />
        </div>
      ) : null}
      <div className="spark" title={t("today.chart")}>
        {stats.days.map(([d, v], i) => (
          <span key={d} className={i === stats.days.length - 1 ? "today" : undefined} style={{ height: `${Math.max(6, (v / max) * 100)}%`, opacity: v ? 1 : 0.35 }} title={`${d}: ${formatNumber(v)}`} />
        ))}
      </div>
    </div>
  );
}

function SyncButton() {
  const t = useT();
  const status = useStore((s) => s.syncStatus);
  useStore((s) => s.syncProgress); // the text and bar follow the sync
  if (!status || status.state === "off") {
    return (
      <button className="sync-btn" onClick={() => useStore.getState().set({ settingsTab: "sync" })}>
        <Icon name="cloudOff" />
        <span>{t("sync.setup")}</span>
      </button>
    );
  }
  const syncing = status.state === "syncing";
  const error = status.state === "error";
  const progress = syncing ? useStore.getState().syncProgress : null;
  const ratio = progress && progress.total > 0 ? Math.min(1, progress.done / progress.total) : null;
  return (
    <button
      className={`sync-btn${error ? " error" : ""}${syncing ? " syncing" : ""}`}
      style={ratio !== null ? { ["--sync-progress" as string]: ratio } : undefined}
      title={status.message ?? t("sync.now")}
      onClick={() => {
        if (status.needsPassword) useStore.getState().set({ settingsTab: "sync" });
        else void api.syncNow().catch(report);
      }}
    >
      <Icon name={error ? "alert" : "sync"} className={syncing ? "spin" : undefined} />
      <span>{syncing ? (progress?.text ?? t("sync.syncing")) : error ? (status.message ?? t("sync.error")) : status.lastSync ? t("sync.last", { time: relativeTime(status.lastSync) }) : t("sync.never")}</span>
    </button>
  );
}

const TABS: [SidebarTab, string, Key][] = [
  ["notebooks", "folder", "nav.notebooks"],
  ["comments", "comment", "nav.comments"],
];

/** Tabs only change what the sidebar shows; choosing something in one (a notebook, a day) opens it. */
function selectTab(tab: SidebarTab) {
  useStore.getState().set({ sidebarTab: tab });
}

export function Sidebar() {
  const t = useT();
  const open = useStore((s) => s.sidebarOpen);
  const peeking = useStore((s) => s.sidebarPeek && !s.sidebarOpen);
  const tab = useStore((s) => s.sidebarTab);
  const chips = useRef<HTMLDivElement>(null);
  useInk(chips, ".chip.active", tab);
  const layout = useStore((s) => s.layout);

  return (
    <nav
      className={`sidebar${open || peeking ? "" : " closed"}${peeking ? " peek" : ""}`}
      {...peekSidebarProps}
      {...contextMenu(() => [
        { label: t("list.new"), icon: "pen", hint: "Ctrl N", run: () => useStore.getState().newNote() },
        { label: t("nav.newNotebook"), icon: "folderPlus", run: () => newNotebook() },
        { label: t("cmd.diary"), icon: "sun", hint: "Ctrl D", run: () => useStore.getState().openDiary() },
        "sep",
        { label: t("cmd.settings"), icon: "settings", hint: "Ctrl ,", run: () => useStore.getState().set({ settingsTab: "general" }) },
      ])}
    >
      <div className="sidebar-head" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <span className="brand-mark" data-tauri-drag-region />
          {t("appName")}
        </div>
        {layout === "narrow" ? (
          <WindowControls />
        ) : (
          // Only on touch screens: with a mouse, the page beside it or Ctrl+\ closes it.
          <button className="icon-btn sidebar-close" title={`${t("nav.closeSidebar")} (Ctrl+\\)`} onClick={() => useStore.getState().set({ sidebarOpen: false })}>
            <Icon name="sidebarClose" />
          </button>
        )}
      </div>
      <button className="search-trigger" onClick={() => useStore.getState().set({ paletteOpen: true })}>
        <Icon name="search" />
        <span>{t("nav.search")}</span>
        <span className="kbd">Ctrl K</span>
      </button>
      <div className="chip-bar inked" role="tablist" ref={chips}>
        <span className="ink" aria-hidden="true" />
        {TABS.map(([id, icon, label]) => (
          <button key={id} role="tab" aria-selected={tab === id} className={`chip${tab === id ? " active" : ""}`} title={t(label)} onClick={() => selectTab(id)}>
            <Icon name={icon} />
          </button>
        ))}
      </div>
      <div className="sidebar-scroll">
        <div key={tab} className="tab-body" data-dir={tab === "notebooks" ? "back" : "forward"}>
          {tab === "notebooks" && <NotebooksTab />}
          {tab === "comments" && <CommentsTab />}
        </div>
      </div>
      <div className="sidebar-foot">
        <TodayCard />
        <div className="sync-row">
          <SyncButton />
          <button className="icon-btn" title={`${t("nav.settings")} (Ctrl+,)`} onClick={() => useStore.getState().set({ settingsTab: "general" })}>
            <Icon name="settings" />
          </button>
        </div>
      </div>
    </nav>
  );
}
