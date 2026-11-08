// Table helpers: a size picker for new tables, keyboard shortcuts, pasting
// spreadsheet cells (tab-separated text) as a table, column alignment, and
// typing `$…$` to make inline math.

import { Extension, nodeInputRule, type Editor, type JSONContent } from "@tiptap/core";
import { create } from "zustand";

// -------------------------------------------------------------------------
// Size picker

interface PickerState {
  at: { x: number; y: number } | null;
  editor: Editor | null;
}
export const useTablePicker = create<PickerState>(() => ({ at: null, editor: null }));

export function openTablePicker(editor: Editor) {
  const { from } = editor.state.selection;
  const c = editor.view.coordsAtPos(from);
  useTablePicker.setState({ at: { x: c.left, y: c.bottom + 6 }, editor });
}

export function insertTable(editor: Editor, rows: number, cols: number) {
  editor.chain().focus().insertTable({ rows, cols, withHeaderRow: true }).run();
}

// -------------------------------------------------------------------------
// Pasting cells

/** Tab-separated text (copied from a spreadsheet) as a table, or null. */
export function tsvToTable(text: string): JSONContent | null {
  const lines = text.replace(/\r\n?/g, "\n").replace(/\n+$/, "").split("\n");
  if (lines.length < 1 || !lines.every((l) => l.includes("\t"))) return null;
  const rows = lines.map((l) => l.split("\t"));
  const cols = Math.max(...rows.map((r) => r.length));
  if (cols < 2) return null;
  const cell = (type: string, text: string): JSONContent => ({
    type,
    content: [{ type: "paragraph", content: text.trim() ? [{ type: "text", text: text.trim() }] : [] }],
  });
  return {
    type: "table",
    content: rows.map((r, i) => ({
      type: "tableRow",
      content: Array.from({ length: cols }, (_, j) => cell(i === 0 ? "tableHeader" : "tableCell", r[j] ?? "")),
    })),
  };
}

// -------------------------------------------------------------------------
// Alignment of the column the cursor is in

export function alignColumn(editor: Editor, align: "left" | "center" | "right" | null) {
  const { state } = editor;
  const $from = state.selection.$from;
  let depth = $from.depth;
  while (depth > 0 && !["tableCell", "tableHeader"].includes($from.node(depth).type.name)) depth--;
  if (depth < 2) return;
  const column = $from.index(depth - 1);
  const tableDepth = depth - 2;
  const table = $from.node(tableDepth);
  const tableStart = $from.start(tableDepth);
  const tr = state.tr;
  table.forEach((row, rowOffset) => {
    let cellOffset = 0;
    row.forEach((_cell, offset, index) => {
      if (index === column) cellOffset = offset;
    });
    const cell = row.maybeChild(column);
    if (!cell) return;
    const pos = tableStart + rowOffset + 1 + cellOffset;
    tr.setNodeMarkup(pos, undefined, { ...cell.attrs, align });
  });
  editor.view.dispatch(tr);
}

// -------------------------------------------------------------------------
// Keyboard shortcuts in tables, and inline math by typing $…$

export const TableKeys = Extension.create({
  name: "tableKeys",
  addKeyboardShortcuts() {
    const inTable = () => this.editor.isActive("table");
    const run = (f: () => boolean) => () => (inTable() ? f() : false);
    const c = () => this.editor.chain().focus();
    return {
      "Mod-Alt-ArrowDown": run(() => c().addRowAfter().run()),
      "Mod-Alt-ArrowUp": run(() => c().addRowBefore().run()),
      "Mod-Alt-ArrowRight": run(() => c().addColumnAfter().run()),
      "Mod-Alt-ArrowLeft": run(() => c().addColumnBefore().run()),
      "Mod-Alt-Backspace": run(() => c().deleteRow().run()),
      "Mod-Shift-Alt-Backspace": run(() => c().deleteColumn().run()),
      // In the last cell, Enter at the end of the text adds a row (like spreadsheets).
      "Mod-Enter": run(() => c().addRowAfter().goToNextCell().run()),
    };
  },
});

export const MathTyping = Extension.create({
  name: "mathTyping",
  addInputRules() {
    const type = this.editor.schema.nodes.inlineMath;
    if (!type) return [];
    return [
      nodeInputRule({
        // `$x^2$`: not after another `$` or a backslash, no spaces just inside.
        find: /(?<![$\\\d])\$([^$\s](?:[^$]*?[^$\s])?)\$$/,
        type,
        getAttributes: (m) => ({ latex: m[1] }),
      }),
    ];
  },
});
