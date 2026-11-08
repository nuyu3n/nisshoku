// Comments on passages: the passages are found again by their text (the
// comments are stored apart from the note's text) and highlighted.

import { Extension, type Editor } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Node as PMNode } from "@tiptap/pm/model";

export interface CommentAnchor {
  id: string;
  quote: string;
  before: string;
  resolved: boolean;
}

interface Range {
  from: number;
  to: number;
}

interface CommentsState {
  comments: CommentAnchor[];
  active: string | null;
  ranges: Map<string, Range[]>;
}

export const commentsKey = new PluginKey<CommentsState>("comments");

/** The document's text (blocks separated by "\n") and the position of each character. */
function docText(doc: PMNode): { text: string; pos: number[] } {
  let text = "";
  const pos: number[] = [];
  doc.descendants((node, start) => {
    if (!node.isTextblock) return true;
    if (text) {
      text += "\n";
      pos.push(-1);
    }
    node.forEach((child, offset) => {
      if (child.isText && child.text) {
        for (let i = 0; i < child.text.length; i++) pos.push(start + 1 + offset + i);
        text += child.text;
      } else {
        text += "￼";
        pos.push(-1);
      }
    });
    return false;
  });
  return { text, pos };
}

/** Where a comment's passage is now: the occurrence whose preceding text matches best. */
function locate(text: string, pos: number[], c: CommentAnchor): Range[] | null {
  if (!c.quote) return null;
  let best = -1;
  let bestScore = -1;
  for (let i = text.indexOf(c.quote); i !== -1; i = text.indexOf(c.quote, i + 1)) {
    let score = 0;
    while (score < c.before.length && text[i - 1 - score] === c.before[c.before.length - 1 - score]) score++;
    if (score > bestScore) {
      best = i;
      bestScore = score;
      if (score === c.before.length) break;
    }
  }
  if (best === -1) return null;
  // Split into runs of consecutive positions (block boundaries break them).
  const ranges: Range[] = [];
  let cur: Range | null = null;
  for (let i = best; i < best + c.quote.length; i++) {
    const p = pos[i];
    if (p < 0) {
      cur = null;
      continue;
    }
    if (cur && cur.to === p) cur.to = p + 1;
    else {
      cur = { from: p, to: p + 1 };
      ranges.push(cur);
    }
  }
  return ranges;
}

function compute(doc: PMNode, comments: CommentAnchor[]): Map<string, Range[]> {
  const { text, pos } = docText(doc);
  const out = new Map<string, Range[]>();
  for (const c of comments) {
    const r = locate(text, pos, c);
    if (r) out.set(c.id, r);
  }
  return out;
}

/** The selected passage as a comment anchor (its text and a little of what precedes it). */
export function selectionAnchor(editor: Editor): { quote: string; before: string } | null {
  const { from, to, empty } = editor.state.selection;
  if (empty) return null;
  const { text, pos } = docText(editor.state.doc);
  const start = pos.findIndex((p) => p >= from);
  let end = start;
  while (end < pos.length && (pos[end] < 0 || pos[end] < to)) end++;
  if (start < 0) return null;
  const quote = text.slice(start, end).replace(/\n+$/, "");
  if (!quote.trim()) return null;
  return { quote, before: text.slice(Math.max(0, start - 16), start) };
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    comments: {
      setComments: (comments: CommentAnchor[], active: string | null) => ReturnType;
    };
  }
}

export const Comments = Extension.create<{ onActivate: (id: string) => void }>({
  name: "comments",
  addOptions() {
    return { onActivate: () => {} };
  },
  addProseMirrorPlugins() {
    const onActivate = this.options.onActivate;
    return [
      new Plugin<CommentsState>({
        key: commentsKey,
        state: {
          init: () => ({ comments: [], active: null, ranges: new Map() }),
          apply(tr, prev) {
            const meta = tr.getMeta(commentsKey) as { comments: CommentAnchor[]; active: string | null } | undefined;
            if (meta) return { ...meta, ranges: compute(tr.doc, meta.comments) };
            if (tr.docChanged) return { ...prev, ranges: compute(tr.doc, prev.comments) };
            return prev;
          },
        },
        props: {
          decorations(state) {
            const s = commentsKey.getState(state);
            if (!s || s.comments.length === 0) return DecorationSet.empty;
            const decos: Decoration[] = [];
            for (const c of s.comments) {
              if (c.resolved && c.id !== s.active) continue;
              for (const r of s.ranges.get(c.id) ?? []) {
                decos.push(
                  Decoration.inline(r.from, r.to, {
                    class: c.id === s.active ? "comment-mark active" : "comment-mark",
                    "data-comment": c.id,
                  }),
                );
              }
            }
            return DecorationSet.create(state.doc, decos);
          },
          handleClick(view, pos) {
            const s = commentsKey.getState(view.state);
            if (!s) return false;
            for (const c of s.comments) {
              if (c.resolved) continue;
              if ((s.ranges.get(c.id) ?? []).some((r) => pos >= r.from && pos <= r.to)) {
                onActivate(c.id);
                return false;
              }
            }
            return false;
          },
        },
      }),
    ];
  },
  addCommands() {
    return {
      setComments:
        (comments, active) =>
        ({ tr, dispatch }) => {
          if (dispatch) tr.setMeta(commentsKey, { comments, active });
          return true;
        },
    };
  },
});

/** The document range of a comment's passage, if it is still in the text. */
export function commentRange(editor: Editor, id: string): Range | null {
  const ranges = commentsKey.getState(editor.state)?.ranges.get(id);
  if (!ranges || ranges.length === 0) return null;
  return { from: ranges[0].from, to: ranges[ranges.length - 1].to };
}
