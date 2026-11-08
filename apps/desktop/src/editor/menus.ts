// The editor's context menu: clipboard, formatting, tables, inserting blocks.

import type { Editor } from "@tiptap/core";
import type { MenuItem } from "../components/ContextMenu";
import { dialogs } from "../components/Dialogs";
import { t } from "../i18n";
import { readText } from "../lib/clipboard";
import { useStore } from "../store";
import { api } from "../api";
import { chooseFile, chooseImage } from "./images";
import { alignColumn, openTablePicker } from "./table";

/** Asks for a note and inserts a link to it (Joplin's `:/id` form). */
export async function insertNoteLink(editor: Editor) {
  const notes = await api.listNotes({ kind: "all" });
  const id = await dialogs.choose(
    t("links.pick"),
    notes.map((n) => ({ value: n.id, label: n.title || t("list.untitled") })),
  );
  if (!id) return;
  const note = notes.find((n) => n.id === id)!;
  const { empty } = editor.state.selection;
  if (empty) {
    editor
      .chain()
      .focus()
      .insertContent({ type: "text", text: note.title || t("list.untitled"), marks: [{ type: "link", attrs: { href: `:/${id}` } }] })
      .run();
  } else editor.chain().focus().setLink({ href: `:/${id}` }).run();
}

export async function editMath(editor: Editor, kind: "inline" | "block", latex = "", pos?: number) {
  const result = await dialogs.math(t("math.title"), latex, kind === "block", pos !== undefined);
  if (result === null) return;
  const chain = editor.chain().focus();
  if (pos === undefined) {
    if (!result) return;
    if (kind === "inline") chain.insertInlineMath({ latex: result }).run();
    else chain.insertBlockMath({ latex: result }).run();
  } else if (!result) {
    if (kind === "inline") chain.deleteInlineMath({ pos }).run();
    else chain.deleteBlockMath({ pos }).run();
  } else if (kind === "inline") chain.updateInlineMath({ latex: result, pos }).run();
  else chain.updateBlockMath({ latex: result, pos }).run();
}

export async function setLink(editor: Editor) {
  const current = editor.getAttributes("link").href as string | undefined;
  const href = await dialogs.prompt(t("menu.linkPrompt"), { initial: current ?? "https://" });
  if (href === null) return;
  if (!href.trim()) editor.chain().focus().extendMarkRange("link").unsetLink().run();
  else editor.chain().focus().extendMarkRange("link").setLink({ href: href.trim() }).run();
}

export function tableItems(editor: Editor): MenuItem[] {
  const c = () => editor.chain().focus();
  return [
    { label: t("table.rowAbove"), icon: "rowAbove", hint: "Ctrl Alt ↑", run: () => void c().addRowBefore().run() },
    { label: t("table.rowBelow"), icon: "rowBelow", hint: "Ctrl Alt ↓", run: () => void c().addRowAfter().run() },
    { label: t("table.colLeft"), icon: "colLeft", hint: "Ctrl Alt ←", run: () => void c().addColumnBefore().run() },
    { label: t("table.colRight"), icon: "colRight", hint: "Ctrl Alt →", run: () => void c().addColumnAfter().run() },
    "sep",
    { label: t("table.deleteRow"), icon: "rowDelete", hint: "Ctrl Alt ⌫", run: () => void c().deleteRow().run() },
    { label: t("table.deleteCol"), icon: "colDelete", run: () => void c().deleteColumn().run() },
    "sep",
    {
      label: t("table.align"),
      icon: "type",
      submenu: [
        { label: t("table.alignLeft"), run: () => alignColumn(editor, "left") },
        { label: t("table.alignCenter"), run: () => alignColumn(editor, "center") },
        { label: t("table.alignRight"), run: () => alignColumn(editor, "right") },
        { label: t("table.alignNone"), run: () => alignColumn(editor, null) },
      ],
    },
    "sep",
    { label: t("table.headerRow"), icon: "heading", run: () => void c().toggleHeaderRow().run() },
    { label: t("table.headerCol"), icon: "heading", run: () => void c().toggleHeaderColumn().run() },
    "sep",
    { label: t("table.delete"), icon: "trash", danger: true, run: () => void c().deleteTable().run() },
  ];
}

export function insertItems(editor: Editor): MenuItem[] {
  const c = () => editor.chain().focus();
  return [
    { label: `${t("editor.headingPlaceholder")} 1`, icon: "heading", run: () => void c().setNode("heading", { level: 1 }).run() },
    { label: `${t("editor.headingPlaceholder")} 2`, icon: "heading", run: () => void c().setNode("heading", { level: 2 }).run() },
    { label: `${t("editor.headingPlaceholder")} 3`, icon: "heading", run: () => void c().setNode("heading", { level: 3 }).run() },
    "sep",
    { label: t("slash.bullet"), icon: "list", run: () => void c().toggleBulletList().run() },
    { label: t("slash.ordered"), icon: "outline", run: () => void c().toggleOrderedList().run() },
    { label: t("slash.task"), icon: "check", run: () => void c().toggleTaskList().run() },
    { label: t("slash.quote"), icon: "enter", run: () => void c().toggleBlockquote().run() },
    { label: t("slash.code"), icon: "type", run: () => void c().toggleCodeBlock().run() },
    { label: t("slash.scene"), icon: "more", run: () => void c().setHorizontalRule().run() },
    "sep",
    { label: t("menu.table"), icon: "table", run: () => openTablePicker(editor) },
    { label: t("menu.image"), icon: "image", run: () => chooseImage(editor) },
    { label: t("menu.file"), icon: "paste", run: () => chooseFile(editor) },
    { label: t("menu.linkNote"), icon: "linkNote", run: () => insertNoteLink(editor) },
    { label: t("math.inline"), icon: "math", run: () => editMath(editor, "inline") },
    { label: t("math.block"), icon: "math", run: () => editMath(editor, "block") },
  ];
}

export function editorMenu(editor: Editor, onComment: () => void): MenuItem[] {
  const { empty, from, to } = editor.state.selection;
  const selected = editor.state.doc.textBetween(from, to, " ");
  const editable = editor.isEditable;
  const c = () => editor.chain().focus();
  const items: MenuItem[] = [];
  if (editor.isActive("table")) {
    items.push({ label: t("menu.table"), icon: "table", submenu: tableItems(editor) }, "sep");
  }
  items.push(
    { label: t("edit.cut"), icon: "cut", hint: "Ctrl X", disabled: empty || !editable, run: () => {
        editor.commands.focus();
        document.execCommand("cut");
      } },
    { label: t("edit.copy"), icon: "copy", hint: "Ctrl C", disabled: empty, run: () => {
        editor.commands.focus();
        document.execCommand("copy");
      } },
    { label: t("edit.paste"), icon: "paste", hint: "Ctrl V", disabled: !editable, run: async () => {
        const text = await readText();
        if (text) c().insertContent(text.replace(/\r\n/g, "\n").split("\n").map((l) => ({ type: "paragraph", content: l ? [{ type: "text", text: l }] : [] }))).run();
      } },
    { label: t("menu.pasteMarkdown"), icon: "paste", disabled: !editable, run: async () => {
        const text = await readText();
        if (text) c().insertContent(text, { contentType: "markdown" }).run();
      } },
    { label: t("edit.selectAll"), icon: "selectAll", hint: "Ctrl A", run: () => void c().selectAll().run() },
    "sep",
    {
      label: t("menu.format"),
      icon: "type",
      disabled: !editable,
      submenu: [
        { label: t("fmt.bold"), icon: "bold", hint: "Ctrl B", checked: editor.isActive("bold"), run: () => void c().toggleBold().run() },
        { label: t("fmt.italic"), icon: "italic", hint: "Ctrl I", checked: editor.isActive("italic"), run: () => void c().toggleItalic().run() },
        { label: t("fmt.strike"), icon: "strike", checked: editor.isActive("strike"), run: () => void c().toggleStrike().run() },
        { label: t("fmt.mark"), icon: "marker", checked: editor.isActive("highlight"), run: () => void c().toggleHighlight().run() },
        { label: t("fmt.code"), icon: "type", checked: editor.isActive("code"), run: () => void c().toggleCode().run() },
        "sep",
        { label: t("menu.link"), icon: "link", run: () => setLink(editor) },
        { label: t("menu.unlink"), icon: "link", disabled: !editor.isActive("link"), run: () => void c().extendMarkRange("link").unsetLink().run() },
      ],
    },
    { label: t("menu.insert"), icon: "plus", disabled: !editable, submenu: insertItems(editor) },
    "sep",
    { label: t("menu.comment"), icon: "comment", hint: "Ctrl Alt M", disabled: empty || !editable, run: onComment },
    {
      label: t("menu.searchSelection"),
      icon: "search",
      disabled: empty || !selected.trim(),
      run: () => void useStore.getState().setScope({ kind: "search", query: selected.trim().slice(0, 80) }),
    },
  );
  return items;
}
