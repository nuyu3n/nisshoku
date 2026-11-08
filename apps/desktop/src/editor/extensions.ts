// Editor extensions for writing: typographic replacements, ruby notation
// highlighting, and find & replace.

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey, TextSelection } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Node as PMNode } from "@tiptap/pm/model";

// -------------------------------------------------------------------------
// Typographic replacements

const REPLACEMENTS: [string, string][] = [
  ["...", "…"],
  ["。。。", "…"],
  ["--", "――"],
  ["ーー", "――"],
];

/**
 * Replaces typed sequences: "..." → "…", "--" → "――". Works after the fact
 * (on the text before the cursor), so it also applies to text committed by
 * an input method.
 */
export const SmartPunctuation = Extension.create<{ enabled: () => boolean }>({
  name: "smartPunctuation",
  addOptions() {
    return { enabled: () => true };
  },
  addProseMirrorPlugins() {
    const enabled = this.options.enabled;
    return [
      new Plugin({
        key: new PluginKey("smartPunctuation"),
        appendTransaction(transactions, _old, state) {
          if (!enabled() || !transactions.some((t) => t.docChanged)) return null;
          if (transactions.some((t) => t.getMeta("uiEvent") === "paste" || t.getMeta("paste"))) return null;
          const sel = state.selection;
          if (!sel.empty) return null;
          const $from = sel.$from;
          if ($from.parent.type.spec.code) return null;
          const before = $from.parent.textBetween(Math.max(0, $from.parentOffset - 4), $from.parentOffset, undefined, "￼");
          for (const [from, to] of REPLACEMENTS) {
            if (before.endsWith(from)) {
              // Don't turn "----" (a rule being typed) into dashes.
              if (from === "--" && before.endsWith("---")) return null;
              const start = sel.from - from.length;
              return state.tr.insertText(to, start, sel.from);
            }
          }
          return null;
        },
      }),
    ];
  },
});

// -------------------------------------------------------------------------
// Ruby notation: |漢字《かんじ》 and 漢字《かんじ》

const RUBY_RE = /([|｜])([^|｜《》\n]+)《([^《》\n]+)》|([\p{Script=Han}々〆ヵヶ]+)《([^《》\n]+)》/gu;

function rubyDecorations(doc: PMNode): DecorationSet {
  const decos: Decoration[] = [];
  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return;
    for (const m of node.text.matchAll(RUBY_RE)) {
      const start = pos + m.index!;
      if (m[1]) {
        const baseStart = start + 1;
        const baseEnd = baseStart + m[2].length;
        decos.push(Decoration.inline(start, baseStart, { class: "ruby-mark" }));
        decos.push(Decoration.inline(baseStart, baseEnd, { class: "ruby-base" }));
        decos.push(Decoration.inline(baseEnd, start + m[0].length, { class: "ruby-mark" }));
      } else {
        const baseEnd = start + m[4].length;
        decos.push(Decoration.inline(start, baseEnd, { class: "ruby-base" }));
        decos.push(Decoration.inline(baseEnd, start + m[0].length, { class: "ruby-mark" }));
      }
    }
  });
  return DecorationSet.create(doc, decos);
}

export const RubyHighlight = Extension.create({
  name: "rubyHighlight",
  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("rubyHighlight"),
        state: {
          init: (_, state) => rubyDecorations(state.doc),
          apply: (tr, old) => (tr.docChanged ? rubyDecorations(tr.doc) : old),
        },
        props: {
          decorations(state) {
            return this.getState(state);
          },
        },
      }),
    ];
  },
});

// -------------------------------------------------------------------------
// Find & replace

export interface SearchState {
  query: string;
  caseSensitive: boolean;
  matches: { from: number; to: number }[];
  current: number;
}

export const searchKey = new PluginKey<SearchState>("search");

function findMatches(doc: PMNode, query: string, caseSensitive: boolean) {
  const matches: { from: number; to: number }[] = [];
  if (!query) return matches;
  const q = caseSensitive ? query : query.toLowerCase();
  // Text of each textblock with a map from offsets to document positions.
  doc.descendants((node, pos) => {
    if (!node.isTextblock) return;
    let text = "";
    const positions: number[] = [];
    node.forEach((child, offset) => {
      if (child.isText && child.text) {
        for (let i = 0; i < child.text.length; i++) positions.push(pos + 1 + offset + i);
        text += child.text;
      } else {
        positions.push(-1);
        text += "￼";
      }
    });
    const hay = caseSensitive ? text : text.toLowerCase();
    let i = hay.indexOf(q);
    while (i !== -1) {
      const from = positions[i];
      const to = positions[i + q.length - 1] + 1;
      if (from >= 0 && to > from) matches.push({ from, to });
      i = hay.indexOf(q, i + Math.max(1, q.length));
    }
    return false;
  });
  return matches;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    search: {
      setSearch: (query: string, caseSensitive?: boolean) => ReturnType;
      findNext: () => ReturnType;
      findPrevious: () => ReturnType;
      replaceCurrent: (replacement: string) => ReturnType;
      replaceAll: (replacement: string) => ReturnType;
    };
  }
}

export const Search = Extension.create({
  name: "search",
  addProseMirrorPlugins() {
    return [
      new Plugin<SearchState>({
        key: searchKey,
        state: {
          init: () => ({ query: "", caseSensitive: false, matches: [], current: 0 }),
          apply(tr, prev) {
            const meta = tr.getMeta(searchKey) as Partial<SearchState> | undefined;
            const next = { ...prev, ...meta };
            if (meta?.query !== undefined || meta?.caseSensitive !== undefined || tr.docChanged) {
              next.matches = findMatches(tr.doc, next.query, next.caseSensitive);
              if (next.current >= next.matches.length) next.current = 0;
            }
            return next;
          },
        },
        props: {
          decorations(state) {
            const s = searchKey.getState(state);
            if (!s || !s.query) return DecorationSet.empty;
            return DecorationSet.create(
              state.doc,
              s.matches.map((m, i) =>
                Decoration.inline(m.from, m.to, { class: i === s.current ? "search-hit current" : "search-hit" }),
              ),
            );
          },
        },
      }),
    ];
  },
  addCommands() {
    const go =
      (delta: number) =>
      ({ state, dispatch, tr }: { state: any; dispatch: any; tr: any }) => {
        const s = searchKey.getState(state);
        if (!s || s.matches.length === 0) return false;
        const current = (s.current + delta + s.matches.length) % s.matches.length;
        const m = s.matches[current];
        if (dispatch) {
          tr.setMeta(searchKey, { current });
          tr.setSelection(TextSelection.create(tr.doc, m.from, m.to));
          tr.scrollIntoView();
        }
        return true;
      };
    return {
      setSearch:
        (query, caseSensitive = false) =>
        ({ tr, dispatch }) => {
          if (dispatch) tr.setMeta(searchKey, { query, caseSensitive, current: 0 });
          return true;
        },
      findNext: () => go(1),
      findPrevious: () => go(-1),
      replaceCurrent:
        (replacement) =>
        ({ state, tr, dispatch }) => {
          const s = searchKey.getState(state);
          if (!s || s.matches.length === 0) return false;
          const m = s.matches[s.current];
          if (dispatch) {
            if (replacement) tr.insertText(replacement, m.from, m.to);
            else tr.delete(m.from, m.to);
          }
          return true;
        },
      replaceAll:
        (replacement) =>
        ({ state, tr, dispatch }) => {
          const s = searchKey.getState(state);
          if (!s || s.matches.length === 0) return false;
          if (dispatch) {
            for (const m of [...s.matches].reverse()) {
              if (replacement) tr.insertText(replacement, m.from, m.to);
              else tr.delete(m.from, m.to);
            }
          }
          return true;
        },
    };
  },
});
