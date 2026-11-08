// Note and notebook actions shared by menus, the editor and the palette.

import { api, type Folder, type NoteKind, type NoteSummary } from "../api";
import { report, useStore } from "../store";
import { lang, t } from "../i18n";
import { dialogs } from "../components/Dialogs";
import type { MenuItem } from "../components/ContextMenu";
import { readText, writeText } from "./clipboard";
import { markdownToPlain, rubyToHtml } from "./text";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

/** Saves the open note's pending edits (registered by the editor). */
let flushOpenNote: () => Promise<void> = async () => {};
export function registerFlush(fn: () => Promise<void>) {
  flushOpenNote = fn;
}
export const flush = () => flushOpenNote();

function store() {
  return useStore.getState();
}

/** Selects the note after `id` (or before it) in the list. */
function selectNeighbor(id: string) {
  const { notes, selectedId } = store();
  if (selectedId !== id) return;
  const i = notes.findIndex((n) => n.id === id);
  const next = notes[i + 1] ?? notes[i - 1];
  store().select(next && next.id !== id ? next.id : null);
}

export async function trashNote(id: string) {
  await flush();
  try {
    await api.trashNote(id);
  } catch (e) {
    report(e);
    return;
  }
  selectNeighbor(id);
  await store().refresh();
  store().toast(t("toast.trashed"), {
    label: t("toast.undo"),
    run: async () => {
      try {
        await api.restoreNote(id);
      } catch (e) {
        report(e);
        return;
      }
      await store().refresh();
      store().select(id);
    },
  });
}

export async function restoreNote(id: string) {
  await api.restoreNote(id).catch(report);
  await store().refresh();
  store().reloadNoteIf(id);
}

export async function deleteForever(id: string) {
  if (!(await dialogs.confirm(t("menu.deleteForeverConfirm"), t("list.emptyTrashDetail"), { okLabel: t("list.emptyTrashOk"), danger: true }))) return;
  selectNeighbor(id);
  await api.deleteNoteForever(id).catch(report);
  await store().refresh();
}

export async function duplicateNote(id: string) {
  await flush();
  const copy = await api.duplicateNote(id).catch(report);
  if (copy) {
    await store().refresh();
    store().select(copy.id);
  }
}

export async function moveNote(id: string, folderId: string) {
  await flush();
  await api.moveNote(id, folderId).catch(report);
  await store().refresh();
  store().reloadNoteIf(id);
  store().toast(t("folder.moved", { name: folderPath(folderId).join(" / ") }));
}

export function folderPath(id: string): string[] {
  const folders = store().folders;
  const out: string[] = [];
  let f = folders.find((x) => x.id === id);
  while (f && out.length < 32) {
    out.unshift(f.title);
    const parent = f.parentId;
    f = folders.find((x) => x.id === parent);
  }
  return out;
}

/** The notebooks from the top down to `id` (the breadcrumb of a note). */
export function folderTrail(id: string): Folder[] {
  const folders = store().folders;
  const out: Folder[] = [];
  let f = folders.find((x) => x.id === id);
  while (f && out.length < 32) {
    out.unshift(f);
    const parent = f.parentId;
    f = folders.find((x) => x.id === parent);
  }
  return out;
}

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);
const folderIcon = (f: Folder) => (f.isWork ? "work" : f.isDiary ? "diary" : "folder");
const byTitle = (a: Folder, b: Folder) => a.title.localeCompare(b.title, "ja");

/**
 * The menu of a part of the breadcrumb above a note: show that notebook, go
 * to one of its notes (the open one ticked), into a sub-notebook or across
 * to a notebook beside it, or do something with it.
 */
export async function crumbMenu(folder: Folder, openNoteId: string): Promise<MenuItem[]> {
  const s = store();
  const notes = await api.listNotes({ kind: "folder", id: folder.id }).catch(() => [] as NoteSummary[]);
  notes.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedTime - a.updatedTime);
  const children = s.folders.filter((f) => f.parentId === folder.id).sort(byTitle);
  const siblings = s.folders.filter((f) => f.parentId === folder.parentId && f.id !== folder.id).sort(byTitle);
  const show = (f: Folder) => () => void s.setScope({ kind: "folder", id: f.id });
  const SHOWN = 10;
  const items: MenuItem[] = [
    { header: folder.title || t("list.untitled") },
    { label: L("このノートブックを一覧に表示", "Show this notebook"), icon: "list", run: show(folder) },
  ];
  if (notes.length > 0) {
    items.push("sep", { header: L(`ノート（${notes.length}）`, `Notes (${notes.length})`) });
    for (const n of notes.slice(0, SHOWN)) {
      items.push({
        label: n.title || t("list.untitled"),
        icon: n.id === openNoteId ? "pen" : n.kind === "character" ? "person" : n.kind === "plot" ? "plot" : n.kind === "material" ? "memo" : n.pinned ? "pin" : n.isTodo ? "todo" : "note",
        checked: n.id === openNoteId,
        run: () => s.select(n.id),
      });
    }
    if (notes.length > SHOWN) items.push({ label: L(`ほか ${notes.length - SHOWN} 件をすべて見る`, `All ${notes.length} notes`), icon: "notes", run: show(folder) });
  }
  if (children.length > 0 || siblings.length > 0) items.push("sep");
  if (children.length > 0) {
    items.push({
      label: L("中のノートブック", "Notebooks inside"),
      icon: "folder",
      submenu: children.map((f) => ({ label: f.title || t("list.untitled"), icon: folderIcon(f), run: show(f) })),
    });
  }
  if (siblings.length > 0) {
    items.push({
      label: L("同じ階層のノートブック", "Notebooks beside it"),
      icon: "columns",
      submenu: siblings.map((f) => ({ label: f.title || t("list.untitled"), icon: folderIcon(f), run: show(f) })),
    });
  }
  items.push("sep", { label: L("ノートブックの操作", "Notebook actions"), icon: "settings", submenu: folderMenu(folder) });
  return items;
}

/** Notebooks in tree order, with their depth. */
export function folderTree(): { folder: Folder; depth: number }[] {
  const folders = store().folders;
  const out: { folder: Folder; depth: number }[] = [];
  const walk = (parent: string, depth: number) => {
    for (const f of folders.filter((x) => (parent === "" ? !folders.some((p) => p.id === x.parentId) : x.parentId === parent))) {
      out.push({ folder: f, depth });
      if (depth < 16) walk(f.id, depth + 1);
    }
  };
  walk("", 0);
  return out;
}

function folderLabel(f: Folder, depth: number) {
  return `${"　".repeat(depth)}${f.icon ? `${f.icon} ` : ""}${f.title || t("folder.untitled")}`;
}

/** A submenu of notebooks to move something into. */
export function moveSubmenu(exclude: (f: Folder) => boolean, run: (folderId: string) => void): MenuItem[] {
  const targets = folderTree().filter(({ folder }) => !exclude(folder));
  if (targets.length === 0) return [{ label: t("list.empty"), disabled: true }];
  // Many notebooks: a list to search instead of a menu taller than the window.
  if (targets.length > 12) {
    const byId = new Map(store().folders.map((f) => [f.id, f]));
    const path = (f: Folder) => {
      const names = [f.title || t("folder.untitled")];
      for (let p = byId.get(f.parentId), i = 0; p && i < 32; p = byId.get(p.parentId), i++) names.unshift(p.title);
      return names.join(" / ");
    };
    return [
      {
        label: t("menu.moveChoose"),
        icon: "search",
        run: async () => {
          const choices = targets.map(({ folder, depth }) => ({ value: folder.id, label: folderLabel(folder, 0), depth, path: path(folder) }));
          const id = await dialogs.choose(t("menu.moveTo"), choices);
          if (id) run(id);
        },
      },
    ];
  }
  return targets.map(({ folder, depth }): MenuItem => ({ label: folderLabel(folder, depth), icon: folder.isDiary ? "diary" : "folder", run: () => run(folder.id) }));
}

export async function editTags(noteId: string, current: string[]) {
  const value = await dialogs.prompt(t("tags.prompt"), { initial: current.join(", "), placeholder: t("tags.placeholder") });
  if (value === null) return;
  const titles = value.split(/[,、，]/).map((x) => x.trim()).filter(Boolean);
  await api.setNoteTags(noteId, titles).catch(report);
  await store().loadFolders();
  await store().loadNotes();
  store().reloadNoteIf(noteId);
}

export async function togglePin(note: { id: string; pinned: boolean }) {
  await api.setNotePinned(note.id, !note.pinned).catch(report);
  await store().loadNotes();
}

export async function toggleTodo(note: { id: string; isTodo: boolean }) {
  await flush();
  await api.setNoteTodo(note.id, !note.isTodo).catch(report);
  await store().loadNotes();
  store().reloadNoteIf(note.id);
}

export async function saveTemplate(noteId: string) {
  await flush();
  await api.saveAsTemplate(noteId).catch(report);
  await store().refresh();
  store().toast(t("toast.templateSaved"));
}

/** Fills `{{date}}`, `{{time}}` and `{{datetime}}` in a template. */
function fillTemplate(text: string): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  const date = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  const time = `${p(d.getHours())}:${p(d.getMinutes())}`;
  return text.replaceAll("{{date}}", date).replaceAll("{{time}}", time).replaceAll("{{datetime}}", `${date} ${time}`);
}

export async function newFromTemplate(templateId: string, parentId?: string) {
  const tpl = await api.getNote(templateId).catch(report);
  if (!tpl) return;
  const s = store();
  const target = parentId ?? (s.scope.kind === "folder" ? s.scope.id : undefined);
  const note = await api.createNote(target, fillTemplate(tpl.title), fillTemplate(tpl.body)).catch(report);
  if (!note) return;
  if (!(s.scope.kind === "folder" && s.scope.id === note.parentId)) await s.setScope({ kind: "folder", id: note.parentId });
  else await s.refresh();
  store().select(note.id);
}

/** Asks for a web page and saves it as a note. */
export async function clipWebPage(parentId?: string) {
  const url = await dialogs.prompt(t("menu.webClip"), { message: t("clip.prompt"), placeholder: "https://" });
  if (!url) return;
  const s = store();
  const target = parentId ?? (s.scope.kind === "folder" ? s.scope.id : null);
  s.toast(t("clip.working"));
  const note = await api.clipUrl(url, target).catch(report);
  if (!note) return;
  await s.setScope({ kind: "folder", id: note.parentId });
  store().select(note.id);
  store().toast(t("clip.done"));
}

/** Asks for a template and creates a note from it. */
export async function pickTemplate(parentId?: string) {
  const templates = await api.listTemplates().catch(() => []);
  if (!templates.length) {
    store().toast(t("menu.noTemplates"));
    return;
  }
  const id = await dialogs.choose(t("menu.fromTemplate"), templates.map((x) => ({ value: x.id, label: x.title || t("list.untitled") })));
  if (id) await newFromTemplate(id, parentId);
}

export async function templateSubmenu(parentId?: string): Promise<MenuItem[]> {
  const templates = await api.listTemplates().catch(() => []);
  if (!templates.length) return [{ label: t("menu.noTemplates"), disabled: true }];
  return templates.map((x) => ({ label: x.title || t("list.untitled"), icon: "template", run: () => newFromTemplate(x.id, parentId) }));
}

/** Writes the notes of a notebook, in their list order, as one manuscript. */
export async function compileFolder(folder: Folder, format: "txt" | "md" | "html") {
  await flush();
  const list = await api.listNotes({ kind: "folder", id: folder.id });
  const sort = store().prefs.sortNotes;
  const sorted = [...list].sort((a, b) =>
    sort === "custom" ? b.order - a.order || a.createdTime - b.createdTime : sort === "title" ? a.title.localeCompare(b.title, "ja") : a.createdTime - b.createdTime,
  );
  const notes = await Promise.all(sorted.map((n) => api.getNote(n.id)));
  const name = (folder.title || "manuscript").replace(/[\\/:*?"<>|]/g, "_");
  const path = await saveDialog({ defaultPath: `${name}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
  if (!path) return;
  let contents: string;
  if (format === "txt") contents = `${folder.title}\n\n` + notes.map((n) => `${n.title}\n\n${markdownToPlain(n.body)}`).join("\n\n\n");
  else if (format === "md") contents = `# ${folder.title}\n\n` + notes.map((n) => `## ${n.title}\n\n${n.body}`).join("\n\n");
  else {
    const esc = (x: string) => x.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const para = (body: string) =>
      markdownToPlain(body)
        .split(/\n{2,}/)
        .map((p) => `<p>${rubyToHtml(p).replace(/\n/g, "<br>")}</p>`)
        .join("\n");
    contents = `<!doctype html><html lang="${store().prefs.language}"><head><meta charset="utf-8"><title>${esc(folder.title)}</title><style>body{max-width:40em;margin:3em auto;padding:0 1em;font-family:"Yu Mincho","Hiragino Mincho ProN",serif;line-height:1.95;color:#2c2a26}h1{text-align:center;margin:3em 0}h2{page-break-before:always;margin-top:3em}p{text-indent:1em;margin:0 0 .4em}</style></head><body><h1>${esc(folder.title)}</h1>${notes.map((n) => `<h2>${esc(n.title)}</h2>${para(n.body)}`).join("\n")}</body></html>`;
  }
  await api.writeTextFile(path, contents + "\n").catch(report);
  store().toast(t("toast.compiled", { n: notes.length }));
}

/** In a work: what the note is (a chapter, a character, material). */
function workKindMenu(note: NoteSummary): MenuItem[] {
  if (!isInWork(note.parentId) || note.kind === "plot") return [];
  const set = async (kind: NoteKind | null) => {
    await api.setNoteKind(note.id, kind).catch(report);
    await store().loadNotes();
    store().reloadNoteIf(note.id);
    const w = await import("./works");
    await w.reloadWork();
    w.useWork.setState({ tab: kind === "character" ? "characters" : kind === "material" ? "materials" : "manuscript" });
  };
  return [
    {
      label: t("work.kind"),
      icon: "work",
      submenu: [
        { label: t("work.kindChapter"), icon: "book", checked: !note.kind, run: () => set(null) },
        { label: t("work.kindCharacter"), icon: "person", checked: note.kind === "character", run: () => set("character") },
        { label: t("work.kindMaterial"), icon: "memo", checked: note.kind === "material", run: () => set("material") },
      ],
    },
  ];
}

/** The notebook is a work or inside one. */
function isInWork(folderId: string): boolean {
  const folders = store().folders;
  let f = folders.find((x) => x.id === folderId);
  for (let i = 0; f && i < 32; i++) {
    if (f.isWork) return true;
    const parent = f.parentId;
    f = folders.find((x) => x.id === parent);
  }
  return false;
}

export function noteMenu(note: NoteSummary): MenuItem[] {
  if (note.deletedTime > 0) {
    return [
      { label: t("menu.open"), icon: "note", run: () => store().select(note.id) },
      "sep",
      { label: t("editor.restore"), icon: "restore", run: () => restoreNote(note.id) },
      { label: t("menu.deleteForever"), icon: "trash", danger: true, run: () => deleteForever(note.id) },
    ];
  }
  return [
    { label: t("menu.open"), icon: "note", run: () => store().select(note.id) },
    {
      label: t("menu.rename"),
      icon: "pen",
      run: () => {
        store().select(note.id);
        setTimeout(() => {
          const el = document.querySelector<HTMLTextAreaElement>(".note-title");
          el?.focus();
          el?.select();
        }, 120);
      },
    },
    "sep",
    { label: note.pinned ? t("menu.unpin") : t("menu.pin"), icon: "pin", run: () => togglePin(note) },
    { label: note.isTodo ? t("menu.untodo") : t("menu.todo"), icon: "todo", run: () => toggleTodo(note) },
    { label: t("menu.tags"), icon: "tag", run: () => editTags(note.id, note.tags) },
    "sep",
    ...workKindMenu(note),
    { label: t("menu.move"), icon: "move", submenu: moveSubmenu((f) => f.id === note.parentId, (id) => void moveNote(note.id, id)) },
    { label: t("menu.duplicate"), icon: "copy", run: () => duplicateNote(note.id) },
    { label: t("menu.saveTemplate"), icon: "template", run: () => saveTemplate(note.id) },
    "sep",
    { label: t("menu.copyTitle"), icon: "copy", run: async () => {
        await writeText(note.title);
        store().toast(t("toast.copied"));
      } },
    { label: t("menu.copyText"), icon: "copy", run: async () => {
        await flush();
        const n = await api.getNote(note.id);
        await writeText(markdownToPlain(n.body));
        store().toast(t("toast.copied"));
      } },
    "sep",
    { label: t("menu.trash"), icon: "trash", danger: true, run: () => trashNote(note.id) },
  ];
}

export async function chooseIcon(folder: Folder) {
  const picked = await dialogs.emoji(t("folder.icon"), folder.icon ?? "");
  if (picked === null) return;
  await api.setFolderIcon(folder.id, picked || null).catch(report);
  await store().loadFolders();
}

export async function renameFolder(folder: Folder) {
  const title = await dialogs.prompt(t("folder.namePrompt"), { initial: folder.title });
  if (title) {
    await api.renameFolder(folder.id, title).catch(report);
    await store().loadFolders();
  }
}

export async function newNotebook(parentId?: string) {
  const title = await dialogs.prompt(t("folder.newPrompt"), { placeholder: t("folder.newPlaceholder") });
  if (!title) return;
  const id = await api.createFolder(title, parentId).catch(report);
  await store().loadFolders();
  if (id) await store().setScope({ kind: "folder", id });
}

function isInside(folder: Folder, ancestor: string): boolean {
  const folders = store().folders;
  let f: Folder | undefined = folder;
  for (let i = 0; f && i < 32; i++) {
    if (f.id === ancestor) return true;
    const parent: string = f.parentId;
    f = folders.find((x) => x.id === parent);
  }
  return false;
}

function openWorkDetails(folderId: string) {
  void import("./works").then((w) => w.useWork.setState({ editing: folderId }));
}

export function folderMenu(folder: Folder): MenuItem[] {
  const active = store().scope.kind === "folder" && (store().scope as { id: string }).id === folder.id;
  return [
    { label: t("folder.newNote"), icon: "plus", run: () => store().newNote(folder.id) },
    { label: t("menu.fromTemplate"), icon: "template", run: () => pickTemplate(folder.id) },
    { label: t("menu.webClip"), icon: "globe", run: () => clipWebPage(folder.id) },
    { label: t("folder.newSub"), icon: "folderPlus", run: () => newNotebook(folder.id) },
    "sep",
    {
      label: t("menu.compile"),
      icon: "book",
      submenu: [
        { label: t("menu.exportTextShort"), icon: "download", run: () => compileFolder(folder, "txt") },
        { label: "Markdown (.md)", icon: "download", run: () => compileFolder(folder, "md") },
        { label: "HTML (.html)", icon: "download", run: () => compileFolder(folder, "html") },
      ],
    },
    folder.isWork
      ? { label: t("work.details"), icon: "work", run: () => openWorkDetails(folder.id) }
      : { label: t("work.make"), icon: "work", disabled: folder.isDiary, run: () => void import("./works").then((w) => w.makeWork(folder)) },
    { label: t("folder.rename"), icon: "pen", run: () => renameFolder(folder) },
    { label: t("folder.icon"), icon: "star", run: () => chooseIcon(folder) },
    {
      label: t("menu.move"),
      icon: "move",
      submenu: [
        { label: t("folder.toTop"), icon: "folder", disabled: !folder.parentId, run: async () => {
            await api.moveFolder(folder.id, "").catch(report);
            await store().loadFolders();
          } },
        "sep",
        ...moveSubmenu((f) => isInside(f, folder.id) || f.id === folder.parentId, async (id) => {
          await api.moveFolder(folder.id, id).catch(report);
          await store().loadFolders();
        }),
      ],
    },
    "sep",
    { label: t("folder.trash"), icon: "trash", danger: true, run: async () => {
        const ok = await dialogs.confirm(t("folder.trashConfirm", { name: folder.title }), t("folder.trashDetail"), { okLabel: t("folder.trash"), danger: true });
        if (!ok) return;
        await flush();
        // What goes with it: this notebook and every one inside.
        const inside = (id: string | undefined) => {
          const f = store().folders.find((x) => x.id === id);
          return !!f && isInside(f, folder.id);
        };
        const scope = store().scope;
        const listInside = active || (scope.kind === "folder" && inside(scope.id));
        // (The open note may not be in the list shown: ask for its notebook.)
        const selected = store().selectedId;
        const open = selected ? await api.getNote(selected).catch(() => null) : null;
        const openInside = !!open && inside(open.parentId);
        try {
          await api.trashFolder(folder.id);
        } catch (e) {
          report(e);
          return;
        }
        if (listInside) await store().setScope({ kind: "all" });
        await store().refresh();
        // The note being written went to the trash too: show it as it is now (read-only).
        if (openInside && open) store().reloadNoteIf(open.id);
      } },
  ];
}

/** Cut / copy / paste for text fields. */
export function inputMenu(el: HTMLInputElement | HTMLTextAreaElement): MenuItem[] {
  const hasSelection = (el.selectionStart ?? 0) !== (el.selectionEnd ?? 0);
  const readOnly = el.readOnly || el.disabled;
  return [
    { label: t("edit.cut"), icon: "cut", hint: "Ctrl X", disabled: !hasSelection || readOnly, run: () => {
        el.focus();
        document.execCommand("cut");
      } },
    { label: t("edit.copy"), icon: "copy", hint: "Ctrl C", disabled: !hasSelection, run: () => {
        el.focus();
        document.execCommand("copy");
      } },
    { label: t("edit.paste"), icon: "paste", hint: "Ctrl V", disabled: readOnly, run: async () => {
        const text = await readText();
        el.focus();
        document.execCommand("insertText", false, text);
      } },
    "sep",
    { label: t("edit.selectAll"), icon: "selectAll", hint: "Ctrl A", run: () => {
        el.focus();
        el.select();
      } },
  ];
}

