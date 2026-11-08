// Works (a novel, a collection): which work is open, its overview, and what
// can be done with it. The data model is in src-tauri/src/works.rs.

import { create } from "zustand";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api, isTauri, type ChapterMeta, type Folder, type NoteKind, type WorkMeta, type WorkOverview } from "../api";
import { report, useStore } from "../store";
import { lang } from "../i18n";
import { dialogs } from "../components/Dialogs";
import { flush } from "./actions";
import { markdownToPlain, rubyToHtml } from "./text";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

export type WorkTab = "manuscript" | "characters" | "plot" | "materials";

interface WorkState {
  /** The work of the notebook shown or the note open. */
  workId: string | null;
  overview: WorkOverview | null;
  tab: WorkTab;
  /** The work-details dialog is open (for this work). */
  editing: string | null;
  /** The work was made just now (so the dialog offers no way to unmake it). */
  fresh?: boolean;
  /** Bumped when an overview of another work than the one in view arrives. */
  cached?: number;
}

export const useWork = create<WorkState>(() => ({ workId: null, overview: null, tab: "manuscript", editing: null }));

export const emptyWorkMeta = (): WorkMeta => ({ logline: "", synopsis: "", genre: "", targetChars: null, deadline: null });

/** The work a notebook belongs to (its own, or the nearest above it). */
export function workIdOf(folderId: string | null | undefined): string | null {
  const folders = useStore.getState().folders;
  let f = folders.find((x) => x.id === folderId);
  for (let i = 0; f && i < 32; i++) {
    if (f.isWork) return f.id;
    const parent = f.parentId;
    f = folders.find((x) => x.id === parent);
  }
  return null;
}

/** The work in view: the notebook shown, else the open note's. */
function currentWorkId(): string | null {
  const s = useStore.getState();
  if (s.scope.kind === "folder") {
    const w = workIdOf(s.scope.id);
    if (w) return w;
  }
  const open = s.notes.find((n) => n.id === s.selectedId);
  return open ? workIdOf(open.parentId) : null;
}

let loading = 0;
/**
 * The last overview of each work seen: coming back to a work shows it at
 * once (then the fresh one replaces it), instead of the plain layout.
 */
const seen = new Map<string, WorkOverview>();

/** Reloads the overview of the work in view. */
export async function reloadWork(): Promise<void> {
  const workId = currentWorkId();
  const ticket = ++loading;
  if (!workId) {
    if (useWork.getState().workId) useWork.setState({ workId: null, overview: null });
    return;
  }
  if (useWork.getState().workId !== workId) useWork.setState({ workId, overview: seen.get(workId) ?? null });
  const overview = await api.workOverview(workId).catch(() => null);
  if (overview) seen.set(workId, overview);
  if (ticket === loading) useWork.setState({ workId, overview });
}

/**
 * The editor opened a note: when it is in another work than the one in view
 * (opened from a search or a link), that work's overview is loaded too.
 */
export function noteOpened(parentId: string): void {
  const id = workIdOf(parentId);
  if (!id || id === useWork.getState().workId) return;
  void api.workOverview(id).then((o) => {
    seen.set(id, o);
    useWork.setState((s) => ({ cached: (s.cached ?? 0) + 1 }));
  }, () => undefined);
}

/** Whether the overview shown is not the one of the work in view yet. */
export function workPending(): boolean {
  const id = currentWorkId();
  return !!id && useWork.getState().overview?.id !== id;
}

/** Keeps the overview current as notebooks, notes and the view change. */
export function startWorkSync(): void {
  let timer = 0;
  const later = () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void reloadWork(), 250);
  };
  useStore.subscribe((now, before) => {
    // Opening a character or material shows its tab in the list.
    if (now.selectedId !== before.selectedId && now.selectedId) {
      const kind = now.notes.find((n) => n.id === now.selectedId)?.kind;
      const tab: WorkTab | null = kind === "character" ? "characters" : kind === "material" ? "materials" : kind === "plot" ? "plot" : null;
      if (tab && useWork.getState().tab !== tab) useWork.setState({ tab });
    }
    if (now.scope !== before.scope || now.folders !== before.folders || now.notes !== before.notes || now.selectedId !== before.selectedId || now.noteVersion !== before.noteVersion) {
      // Another work: at once, so that its layout (the plot board, the
      // work panel) shows with the note instead of after it.
      if (currentWorkId() !== useWork.getState().workId) {
        window.clearTimeout(timer);
        void reloadWork();
      } else later();
    }
  });
  later();
}

/** The open note's work, by the notebook it is in. */
export function useNoteWork(parentId: string | undefined): WorkOverview | null {
  const work = useWork((s) => s.overview);
  useWork((s) => s.cached);
  useStore((s) => s.folders);
  const id = workIdOf(parentId);
  if (!id) return null;
  return work && id === work.id ? work : (seen.get(id) ?? null);
}

// -------------------------------------------------------------------------
// Actions

export async function makeWork(folder: Folder): Promise<void> {
  await api.setWork(folder.id, emptyWorkMeta()).catch(report);
  const s = useStore.getState();
  await s.loadFolders();
  await s.setScope({ kind: "folder", id: folder.id });
  useWork.setState({ tab: "manuscript", editing: folder.id, fresh: true });
  s.toast(L(`「${folder.title}」を作品にしました`, `“${folder.title}” is now a work`));
}

export async function unmakeWork(folder: Folder): Promise<void> {
  const ok = await dialogs.confirm(
    L(`「${folder.title}」を作品として扱うのをやめますか？`, `Stop treating “${folder.title}” as a work?`),
    L("ノートはそのまま残ります。人物やプロットのノートも、ふつうのノートとして残ります。", "The notes stay, including those of the characters and the plot, as ordinary notes."),
    { okLabel: L("やめる", "Stop") },
  );
  if (!ok) return;
  await api.setWork(folder.id, null).catch(report);
  await useStore.getState().loadFolders();
}

export async function saveWorkMeta(workId: string, meta: WorkMeta): Promise<void> {
  await api.setWork(workId, meta).catch(report);
  await reloadWork();
}

async function openNote(id: string, folderId: string) {
  const s = useStore.getState();
  if (!(s.scope.kind === "folder" && workIdOf(s.scope.id) === workIdOf(folderId))) await s.setScope({ kind: "folder", id: folderId });
  else await s.refresh();
  useStore.getState().select(id);
}

/** A new character, material or chapter in the work. */
export async function newWorkNote(workId: string, kind: NoteKind | "chapter", folderId?: string): Promise<void> {
  if (kind === "chapter") {
    await useStore.getState().newNote(folderId ?? workId);
    return;
  }
  const title =
    kind === "character"
      ? await dialogs.prompt(L("新しい登場人物", "New character"), { placeholder: L("名前", "Name") })
      : await dialogs.prompt(L("新しい資料", "New material"), { placeholder: L("題名（世界観、年表、地図…）", "Title (setting, timeline, map…)") });
  if (title === null) return;
  const note = await api.createWorkNote(workId, kind as NoteKind, title.trim()).catch(report);
  if (!note) return;
  await openNote(note.id, note.parentId);
  await reloadWork();
}

const TEMPLATES: [string, [string, string], [string, string]][] = [
  ["kishotenketsu", ["起承転結", "Four parts (kishōtenketsu)"], ["日本の物語の四部構成", "Beginning, development, twist, conclusion"]],
  ["threeAct", ["三幕構成", "Three acts"], ["発端・対立・解決", "Setup, confrontation, resolution"]],
  ["heroJourney", ["英雄の旅", "Hero’s journey"], ["日常から冒険へ、そして帰還", "From the ordinary world to the adventure and back"]],
  ["free", ["自由に作る", "Free"], ["列を自分で決める", "Make your own columns"]],
];

/** Opens the work's plot board (asking for its structure the first time). */
export async function openPlotBoard(workId: string): Promise<void> {
  let template: string | undefined;
  if (!useWork.getState().overview?.plotId) {
    const chosen = await dialogs.choose(
      L("プロットの型を選ぶ", "Choose a structure"),
      TEMPLATES.map(([value, label, hint]) => ({ value, label: `${L(...label)} — ${L(...hint)}` })),
      L("あとから列の追加・名前の変更ができます。", "Columns can be added and renamed later."),
    );
    if (!chosen) return;
    template = chosen;
  }
  const doc = await api.openPlot(workId, template).catch(report);
  if (!doc) return;
  await openNote(doc.noteId, workId);
  useWork.setState({ tab: "plot" });
  await reloadWork();
}

export async function setChapterMeta(noteId: string, meta: ChapterMeta): Promise<void> {
  await api.setChapterMeta(noteId, meta).catch(report);
  await reloadWork();
}

/** Writes the manuscript (chapters in order, parts as headings) as one file. */
export async function compileWork(workId: string, format: "txt" | "md" | "html"): Promise<void> {
  await flush();
  const parts = await api.workManuscript(workId).catch(report);
  const overview = useWork.getState().overview;
  if (!parts) return;
  const title = overview?.id === workId ? overview.title : (useStore.getState().folders.find((f) => f.id === workId)?.title ?? "manuscript");
  const name = (title || "manuscript").replace(/[\\/:*?"<>|]/g, "_");
  const path = isTauri
    ? await saveDialog({ defaultPath: `${name}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] })
    : `${name}.${format}`;
  if (!path) return;
  const chapters = parts.reduce((n, p) => n + p.notes.length, 0);
  let contents: string;
  if (format === "txt") {
    contents = [title, ...parts.flatMap((p) => [...(p.title ? [`\n【${p.title}】`] : []), ...p.notes.map((n) => `\n${n.title}\n\n${markdownToPlain(n.body)}`)])].join("\n\n");
  } else if (format === "md") {
    contents = [`# ${title}`, ...parts.flatMap((p) => [...(p.title ? [`## ${p.title}`] : []), ...p.notes.map((n) => `${p.title ? "###" : "##"} ${n.title}\n\n${n.body}`)])].join("\n\n");
  } else {
    const esc = (x: string) => x.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const para = (body: string) =>
      markdownToPlain(body)
        .split(/\n{2,}/)
        .map((p) => `<p>${rubyToHtml(p).replace(/\n/g, "<br>")}</p>`)
        .join("\n");
    const bodyHtml = parts
      .map((p) => `${p.title ? `<h2 class="part">${esc(p.title)}</h2>` : ""}${p.notes.map((n) => `<h3>${esc(n.title)}</h3>${para(n.body)}`).join("\n")}`)
      .join("\n");
    contents = `<!doctype html><html lang="${lang()}"><head><meta charset="utf-8"><title>${esc(title)}</title><style>body{max-width:40em;margin:3em auto;padding:0 1em;font-family:"Yu Mincho","Hiragino Mincho ProN","Noto Serif JP",serif;line-height:1.95;color:#2c2a26}h1{text-align:center;margin:3em 0}h2.part{text-align:center;page-break-before:always;margin:4em 0 2em}h3{page-break-before:always;margin-top:3em}p{text-indent:1em;margin:0 0 .4em}</style></head><body><h1>${esc(title)}</h1>${bodyHtml}</body></html>`;
  }
  await api.writeTextFile(path, contents + "\n").catch(report);
  useStore.getState().toast(L(`${chapters} 章をまとめて書き出しました`, `Compiled ${chapters} chapters`));
}

// -------------------------------------------------------------------------
// Labels

export const STATUSES = ["idea", "draft", "revise", "done"] as const;

export function statusLabel(status: string): string {
  switch (status) {
    case "idea":
      return L("構想", "Idea");
    case "draft":
      return L("執筆中", "Drafting");
    case "revise":
      return L("推敲中", "Revising");
    case "done":
      return L("完成", "Done");
    default:
      return L("未設定", "No status");
  }
}

export const ROLES = ["protagonist", "antagonist", "main", "supporting", "minor"] as const;

export function roleLabel(role: string): string {
  switch (role) {
    case "protagonist":
      return L("主人公", "Protagonist");
    case "antagonist":
      return L("敵対者", "Antagonist");
    case "main":
      return L("主要人物", "Main");
    case "supporting":
      return L("脇役", "Supporting");
    case "minor":
      return L("端役", "Minor");
    default:
      return "";
  }
}

/** Days until `deadline` (YYYY-MM-DD) from today, or null. */
export function daysLeft(deadline: string | null): number | null {
  if (!deadline) return null;
  const [y, m, d] = deadline.split("-").map(Number);
  if (!y || !m || !d) return null;
  const end = new Date(y, m - 1, d).getTime();
  const today = new Date();
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  return Math.round((end - start) / 86_400_000);
}

/** A colour for a character without one, from their name. */
export function characterColor(name: string, color: string): string {
  if (color) return color;
  const palette = ["#d9622b", "#6a8caf", "#5c8a5a", "#9a6fb0", "#b8873a", "#c25b78", "#4f8f94"];
  let h = 0;
  for (const ch of name) h = (h * 31 + ch.codePointAt(0)!) >>> 0;
  return palette[h % palette.length];
}
