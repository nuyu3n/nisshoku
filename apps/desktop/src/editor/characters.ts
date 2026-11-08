// Characters' names in the text of a work, underlined in each character's
// colour, with their role on hover. The names come from the work's overview
// (src/lib/works.ts); `refreshCharacterNames` redraws when they change.

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Node as PMNode } from "@tiptap/pm/model";
import type { EditorView } from "@tiptap/pm/view";

export interface CharacterName {
  /** The character's note. */
  id: string;
  name: string;
  color: string;
  /** Shown on hover: name, reading and role. */
  label: string;
}

const key = new PluginKey<DecorationSet>("characterNames");

function decorations(doc: PMNode, names: CharacterName[]): DecorationSet {
  if (!names.length) return DecorationSet.empty;
  const sorted = [...names].filter((n) => n.name.trim()).sort((a, b) => b.name.length - a.name.length);
  const out: Decoration[] = [];
  doc.descendants((node, pos) => {
    if (!node.isTextblock) return;
    // Text of the block with the document position of each offset.
    let text = "";
    const positions: number[] = [];
    node.forEach((child, offset) => {
      if (child.isText && child.text) {
        for (let i = 0; i < child.text.length; i++) positions.push(pos + 1 + offset + i);
        text += child.text;
      } else {
        positions.push(-1);
        text += "\u0000";
      }
    });
    for (let i = 0; i < text.length; ) {
      const hit = sorted.find((n) => text.startsWith(n.name, i));
      if (!hit) {
        i++;
        continue;
      }
      const from = positions[i];
      const to = positions[i + hit.name.length - 1];
      if (from >= 0 && to >= 0) {
        out.push(
          Decoration.inline(from, to + 1, {
            class: "char-name",
            style: `--char-color: ${hit.color}`,
            // The app's tooltip (a `title` would show the system's).
            "data-tip": hit.label,
            "data-character": hit.id,
          }),
        );
      }
      i += hit.name.length;
    }
    return false;
  });
  return DecorationSet.create(doc, out);
}

export interface CharacterNamesOptions {
  names: () => CharacterName[];
  enabled: () => boolean;
  /** A name was clicked with Ctrl / ⌘ held: open the character. */
  onOpen: (id: string) => void;
}

export const CharacterNames = Extension.create<CharacterNamesOptions>({
  name: "characterNames",
  addOptions() {
    return { names: () => [], enabled: () => true, onOpen: () => {} };
  },
  addProseMirrorPlugins() {
    const opts = this.options;
    const compute = (doc: PMNode) => (opts.enabled() ? decorations(doc, opts.names()) : DecorationSet.empty);
    return [
      new Plugin({
        key,
        state: {
          init: (_, state) => compute(state.doc),
          apply: (tr, old) => (tr.docChanged || tr.getMeta(key) ? compute(tr.doc) : old.map(tr.mapping, tr.doc)),
        },
        props: {
          decorations(state) {
            return key.getState(state);
          },
          handleClick(_view, _pos, event) {
            if (!(event.ctrlKey || event.metaKey)) return false;
            const el = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-character]");
            if (!el?.dataset.character) return false;
            opts.onOpen(el.dataset.character);
            return true;
          },
        },
      }),
    ];
  },
});

/** Redraws the names (after the characters or the preference changed). */
export function refreshCharacterNames(editor: { view: EditorView } | null) {
  try {
    const view = editor?.view;
    if (view && !view.isDestroyed) view.dispatch(view.state.tr.setMeta(key, true));
  } catch {
    // The view is not mounted yet: it draws the names when it is.
  }
}
