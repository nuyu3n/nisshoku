// The notebooks in the sidebar, made for many of them nested deep:
//
// - folding is remembered (preferences), and the open notebook's parents
//   unfold by themselves, scrolled into view;
// - guide lines show the levels; a folded notebook counts the notes inside
//   its sub-notebooks too;
// - with many notebooks, a filter shows the matching ones and their parents;
// - one button folds or unfolds them all;
// - notebooks are dragged onto others to nest them (onto the section's head
//   to bring them back to the top); a folded one unfolds when a drag waits
//   on it.

import { useEffect, useMemo, useRef, useState } from "react";
import { api, type Folder } from "../api";
import { report, useStore } from "../store";
import { lang, useT } from "../i18n";
import { Icon } from "./Icon";
import { contextMenu, openMenu } from "./ContextMenu";
import { folderMenu, newNotebook, renameFolder } from "../lib/actions";
import { dragChip } from "../lib/drag";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);
const FOLDER = "application/x-nisshoku-folder";
const NOTE = "application/x-nisshoku-note";
/** Above this many notebooks, the filter shows. */
const FILTER_FROM = 8;

/** Notebooks of the sidebar: no diary; those whose parent is gone at the top. */
function useTree() {
  const folders = useStore((s) => s.folders);
  return useMemo(() => {
    const list = folders.filter((f) => !f.isDiary);
    const ids = new Set(list.map((f) => f.id));
    const tree = list.map((f) => (ids.has(f.parentId) ? f : { ...f, parentId: "" }));
    const children = new Map<string, Folder[]>();
    for (const f of tree) children.set(f.parentId, [...(children.get(f.parentId) ?? []), f]);
    const byId = new Map(tree.map((f) => [f.id, f]));
    /** Notes in a notebook and all below it. */
    const total = new Map<string, number>();
    const count = (id: string): number => {
      if (total.has(id)) return total.get(id)!;
      total.set(id, 0); // (a cycle counts once)
      const n = (byId.get(id)?.noteCount ?? 0) + (children.get(id) ?? []).reduce((a, c) => a + count(c.id), 0);
      total.set(id, n);
      return n;
    };
    tree.forEach((f) => count(f.id));
    return { tree, children, byId, total };
  }, [folders]);
}

type Tree = ReturnType<typeof useTree>;

function ancestors(tree: Tree, id: string): string[] {
  const out: string[] = [];
  let f = tree.byId.get(id);
  for (let i = 0; f && f.parentId && i < 64; i++) {
    out.push(f.parentId);
    f = tree.byId.get(f.parentId);
  }
  return out;
}

function setFolded(id: string, folded: boolean) {
  const s = useStore.getState();
  const list = s.prefs.collapsedFolders.filter((x) => x !== id);
  s.setPrefs({ collapsedFolders: folded ? [...list, id] : list });
}

/** Moves notebook `id` into `parent` ("" for the top), unless that would put it inside itself. */
async function moveFolderTo(tree: Tree, id: string, parent: string) {
  const f = tree.byId.get(id);
  if (!f || id === parent || f.parentId === parent || (parent && ancestors(tree, parent).includes(id))) return;
  await api.moveFolder(id, parent).catch(report);
  if (parent) setFolded(parent, false);
  await useStore.getState().loadFolders();
  const to = parent ? tree.byId.get(parent)?.title : L("いちばん上", "the top");
  useStore.getState().toast(L(`「${f.title}」を${parent ? `「${to}」の中` : to}へ移しました`, `Moved “${f.title}” into ${parent ? `“${to}”` : to}`));
}

export function NotebookTree() {
  const t = useT();
  const tree = useTree();
  const scope = useStore((s) => s.scope);
  const collapsed = useStore((s) => s.prefs.collapsedFolders);
  const [filter, setFilter] = useState("");
  const [dropTop, setDropTop] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const activeId = scope.kind === "folder" ? scope.id : null;

  // The open notebook: its parents unfold, and it is scrolled to.
  useEffect(() => {
    if (!activeId) return;
    const folded = ancestors(tree, activeId).filter((id) => collapsed.includes(id));
    if (folded.length) useStore.getState().setPrefs({ collapsedFolders: collapsed.filter((id) => !folded.includes(id)) });
    requestAnimationFrame(() => {
      const row = ref.current?.querySelector<HTMLElement>(`[data-folder="${activeId}"]`);
      const scroller = row?.closest<HTMLElement>(".sidebar-scroll");
      if (!row || !scroller) return;
      const r = row.getBoundingClientRect();
      const s = scroller.getBoundingClientRect();
      if (r.top < s.top || r.bottom > s.bottom) scroller.scrollBy({ top: r.top - s.top - s.height / 3, behavior: "smooth" });
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId, tree]);

  // Filtering: the matches and their parents.
  const q = filter.trim().toLowerCase();
  const shown = useMemo(() => {
    if (!q) return null;
    const set = new Set<string>();
    for (const f of tree.tree) {
      if (!(f.title || "").toLowerCase().includes(q)) continue;
      set.add(f.id);
      ancestors(tree, f.id).forEach((a) => set.add(a));
    }
    return set;
  }, [q, tree]);

  const parents = tree.tree.filter((f) => tree.children.has(f.id));
  const allFolded = parents.length > 0 && parents.every((f) => collapsed.includes(f.id));

  return (
    <div ref={ref} className="notebook-tree">
      <div
        className={`section-head${dropTop ? " drop-target" : ""}`}
        onDragOver={(e) => {
          if (!e.dataTransfer.types.includes(FOLDER)) return;
          e.preventDefault();
          setDropTop(true);
        }}
        onDragLeave={() => setDropTop(false)}
        onDrop={(e) => {
          setDropTop(false);
          const id = e.dataTransfer.getData(FOLDER);
          if (id) void moveFolderTo(tree, id, "");
        }}
      >
        <span className="grow">{dropTop ? L("ここに置くといちばん上へ", "Drop here for the top level") : t("nav.notebooks")}</span>
        {parents.length > 0 && (
          <button
            className="icon-btn"
            title={allFolded ? L("すべて展開", "Unfold all") : L("すべて折りたたむ", "Fold all")}
            onClick={() => useStore.getState().setPrefs({ collapsedFolders: allFolded ? [] : parents.map((f) => f.id) })}
          >
            <Icon name={allFolded ? "unfold" : "fold"} />
          </button>
        )}
        <button className="icon-btn" title={t("nav.newNotebook")} onClick={() => newNotebook()}>
          <Icon name="plus" />
        </button>
      </div>
      {tree.tree.length > FILTER_FROM && (
        <div className="tree-filter">
          <Icon name="search" />
          <input
            className="input"
            value={filter}
            placeholder={L("ノートブックを絞り込む", "Filter notebooks")}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setFilter("")}
          />
          {filter && (
            <button className="icon-btn" title={L("消す", "Clear")} onClick={() => setFilter("")}>
              <Icon name="x" />
            </button>
          )}
        </div>
      )}
      <div className="tree-scroll">
        <Level tree={tree} parent="" depth={0} shown={shown} query={q} collapsed={collapsed} activeId={activeId} />
      </div>
      {shown && shown.size === 0 && <div className="empty small">{t("list.notFound")}</div>}
    </div>
  );
}

interface LevelProps {
  tree: Tree;
  parent: string;
  depth: number;
  shown: Set<string> | null;
  query: string;
  collapsed: string[];
  activeId: string | null;
}

function Level(props: LevelProps) {
  const kids = (props.tree.children.get(props.parent) ?? []).filter((f) => !props.shown || props.shown.has(f.id));
  if (kids.length === 0) return null;
  return (
    <div className={props.depth > 0 ? "folder-children" : undefined} role={props.depth > 0 ? "group" : "tree"}>
      {kids.map((f) => (
        <Row key={f.id} folder={f} {...props} />
      ))}
    </div>
  );
}

function Highlight({ text, query }: { text: string; query: string }) {
  const i = query ? text.toLowerCase().indexOf(query) : -1;
  if (i < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, i)}
      <mark>{text.slice(i, i + query.length)}</mark>
      {text.slice(i + query.length)}
    </>
  );
}

function Row({ folder, tree, depth, shown, query, collapsed, activeId }: LevelProps & { folder: Folder }) {
  const t = useT();
  const [drop, setDrop] = useState(false);
  const unfoldTimer = useRef<number | undefined>(undefined);
  const hasChildren = tree.children.has(folder.id);
  // A filter shows its matches unfolded.
  const open = shown ? true : !collapsed.includes(folder.id);
  const active = activeId === folder.id;
  const title = folder.title || t("folder.untitled");
  const total = tree.total.get(folder.id) ?? folder.noteCount;
  const choose = () => void useStore.getState().setScope({ kind: "folder", id: folder.id });

  return (
    <>
      <div
        role="treeitem"
        aria-expanded={hasChildren ? open : undefined}
        aria-selected={active}
        tabIndex={0}
        data-folder={folder.id}
        className={`nav-item has-actions folder-row${active ? " active" : ""}${drop ? " drop-target" : ""}`}
        onClick={choose}
        onKeyDown={(e) => {
          if (e.key === "Enter") choose();
          else if (e.key === "ArrowRight" && hasChildren && !open) setFolded(folder.id, false);
          else if (e.key === "ArrowLeft" && hasChildren && open) setFolded(folder.id, true);
        }}
        onDoubleClick={() => renameFolder(folder)}
        {...contextMenu(() => folderMenu(folder))}
        draggable
        onDragStart={(e) => {
          e.stopPropagation();
          e.dataTransfer.setData(FOLDER, folder.id);
          e.dataTransfer.effectAllowed = "move";
          dragChip(e, `${folder.icon ? `${folder.icon} ` : ""}${title}`);
        }}
        onDragOver={(e) => {
          const types = e.dataTransfer.types;
          if (!types.includes(NOTE) && !types.includes(FOLDER)) return;
          e.preventDefault();
          if (!drop) {
            setDrop(true);
            // Waiting on a folded notebook unfolds it.
            if (hasChildren && !open) unfoldTimer.current = window.setTimeout(() => setFolded(folder.id, false), 700);
          }
        }}
        onDragLeave={() => {
          setDrop(false);
          window.clearTimeout(unfoldTimer.current);
        }}
        onDrop={async (e) => {
          setDrop(false);
          window.clearTimeout(unfoldTimer.current);
          const folderId = e.dataTransfer.getData(FOLDER);
          if (folderId) {
            e.stopPropagation();
            await moveFolderTo(tree, folderId, folder.id);
            return;
          }
          const id = e.dataTransfer.getData(NOTE);
          if (!id) return;
          await api.moveNote(id, folder.id).catch(report);
          await useStore.getState().refresh();
          useStore.getState().reloadNoteIf(id);
          useStore.getState().toast(t("folder.moved", { name: folder.title }));
        }}
      >
        <span
          className={`twisty${open ? " open" : ""}${hasChildren ? "" : " none"}`}
          onClick={(e) => {
            e.stopPropagation();
            if (hasChildren && !shown) setFolded(folder.id, open);
          }}
        >
          {hasChildren && <Icon name="chevronRight" />}
        </span>
        {folder.icon ? <span className="emoji">{folder.icon}</span> : <Icon name={folder.isWork ? "work" : "folder"} style={folder.color ? { color: folder.color } : undefined} />}
        {folder.color && folder.icon && <span className="folder-dot" style={{ background: folder.color }} />}
        <span className="label" title={title.length > 12 ? title : undefined}>
          <Highlight text={title} query={query} />
        </span>
        {hasChildren && !open ? (
          <span className="count" title={L(`サブノートブックを含めて${total}件`, `${total} with sub-notebooks`)}>
            {total || ""}
          </span>
        ) : (
          <span className="count">{folder.noteCount || ""}</span>
        )}
        <span className="row-actions">
          <button
            title={t("folder.newNote")}
            onClick={(e) => {
              e.stopPropagation();
              void useStore.getState().newNote(folder.id);
            }}
          >
            <Icon name="plus" />
          </button>
          <button
            title={t("editor.more")}
            onClick={(e) => {
              e.stopPropagation();
              openMenu(e.currentTarget, folderMenu(folder));
            }}
          >
            <Icon name="more" />
          </button>
        </span>
      </div>
      {open && hasChildren && <Level tree={tree} parent={folder.id} depth={depth + 1} shown={shown} query={query} collapsed={collapsed} activeId={activeId} />}
    </>
  );
}
