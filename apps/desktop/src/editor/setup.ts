// The editor's extensions, shared by the app and the self-test.

import { Extension, InputRule, type Extensions } from "@tiptap/core";
import { Plugin } from "@tiptap/pm/state";
import { isMobile } from "../lib/mobile";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { Focus, Placeholder } from "@tiptap/extensions";
import { TaskItem } from "@tiptap/extension-task-item";
import { TaskList } from "@tiptap/extension-task-list";
import { Highlight } from "@tiptap/extension-highlight";
import { TableKit } from "@tiptap/extension-table";
import { BlockMath, InlineMath } from "@tiptap/extension-mathematics";
import { RubyHighlight, Search, SmartPunctuation } from "./extensions";
import { SlashCommand } from "./slash";
import { Comments } from "./comments";
import { CjkEmphasis, SafeParagraph } from "./cjk";
import { ResourceImage } from "./images";
import { MathTyping, TableKeys } from "./table";
import { AutoPair, LineNumbers, MoveBlock } from "./lines";
import { CharacterNames, type CharacterName } from "./characters";

export interface SetupOptions {
  placeholder?: (heading: boolean) => string;
  smartPunctuation?: () => boolean;
  slash?: () => boolean;
  lineNumbers?: () => boolean;
  autoPair?: () => boolean;
  onActivateComment?: (id: string) => void;
  onEditMath?: (kind: "inline" | "block", latex: string, pos: number) => void;
  /** Characters of the work the note belongs to, for highlighting their names. */
  characterNames?: () => CharacterName[];
  highlightCharacters?: () => boolean;
  onOpenCharacter?: (id: string) => void;
}

/**
 * Ticking a checklist item only ticks it: the item's own handler focused the
 * editor, which put back the old cursor (and on phones brought up the
 * keyboard), and the page jumped there.
 */
const TaskCheck = Extension.create({
  name: "taskCheck",
  addProseMirrorPlugins() {
    const box = (e: Event) => {
      const el = e.target as HTMLElement | null;
      return el instanceof HTMLInputElement && el.type === "checkbox" && el.closest("li[data-checked]") ? el : null;
    };
    return [
      new Plugin({
        props: {
          handleDOMEvents: {
            // Keep the focus (and the cursor) where it is.
            mousedown: (_view, e) => {
              if (!box(e)) return false;
              e.preventDefault();
              return true;
            },
            click: (view, e) => {
              const el = box(e);
              if (!el) return false;
              e.preventDefault();
              if (!view.editable) return true;
              const li = el.closest("li[data-checked]")!;
              let pos: number;
              try {
                pos = view.posAtDOM(li, 0);
              } catch {
                return true;
              }
              // After the click: a prevented click puts the box back as it
              // was once the event is over, which would undo the tick shown.
              window.setTimeout(() => {
                const $pos = view.state.doc.resolve(Math.min(pos, view.state.doc.content.size));
                for (let d = $pos.depth; d > 0; d--) {
                  const node = $pos.node(d);
                  if (node.type.name === "taskItem") {
                    const at = $pos.before(d);
                    // No scrollIntoView, no selection change: the page stays.
                    view.dispatch(view.state.tr.setNodeMarkup(at, undefined, { ...node.attrs, checked: !node.attrs.checked }));
                    return;
                  }
                }
              }, 0);
              return true;
            },
          },
        },
      }),
    ];
  },
});

/** Typing `[[` asks for a note and links to it (as in many note apps). */
const NoteLinkTrigger = Extension.create({
  name: "noteLinkTrigger",
  addInputRules() {
    return [
      new InputRule({
        find: /\[\[$/,
        handler: ({ state, range }) => {
          if (state.selection.$from.parent.type.spec.code) return null;
          state.tr.delete(range.from, range.to);
          const editor = this.editor;
          setTimeout(() => void import("./menus").then((m) => m.insertNoteLink(editor)), 0);
        },
      }),
    ];
  },
});

export function editorExtensions(o: SetupOptions = {}): Extensions {
  return [
    NoteLinkTrigger,
    TaskCheck,
    StarterKit.configure({
      paragraph: false,
      heading: { levels: [1, 2, 3, 4] },
      link: { openOnClick: false, autolink: true },
    }),
    SafeParagraph,
    Markdown,
    CjkEmphasis,
    ...(isMobile
      ? []
      : [
          Placeholder.configure({
            placeholder: ({ node }) => o.placeholder?.(node.type.name === "heading") ?? "",
          }),
        ]),
    Focus.configure({ className: "has-focus", mode: "shallowest" }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Highlight,
    ResourceImage,
    TableKit.configure({ table: { resizable: false } }),
    // Configured one by one: Mathematics would hand both the same KaTeX options,
    // and block math needs displayMode (full-size fractions, sums with limits).
    InlineMath.configure({
      katexOptions: { throwOnError: false },
      onClick: (node, pos) => o.onEditMath?.("inline", node.attrs.latex as string, pos),
    }),
    BlockMath.configure({
      katexOptions: { throwOnError: false, displayMode: true },
      onClick: (node, pos) => o.onEditMath?.("block", node.attrs.latex as string, pos),
    }),
    TableKeys,
    MathTyping,
    SmartPunctuation.configure({ enabled: () => o.smartPunctuation?.() ?? true }),
    RubyHighlight,
    LineNumbers.configure({ enabled: () => o.lineNumbers?.() ?? false }),
    AutoPair.configure({ enabled: () => o.autoPair?.() ?? true }),
    MoveBlock,
    Search,
    SlashCommand.configure({ enabled: () => o.slash?.() ?? true }),
    Comments.configure({ onActivate: (id) => o.onActivateComment?.(id) }),
    CharacterNames.configure({
      names: () => o.characterNames?.() ?? [],
      enabled: () => o.highlightCharacters?.() ?? true,
      onOpen: (id) => o.onOpenCharacter?.(id),
    }),
  ];
}
