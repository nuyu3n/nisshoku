// The editor's extensions, shared by the app and the self-test.

import type { Extensions } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { Focus, Placeholder } from "@tiptap/extensions";
import { TaskItem } from "@tiptap/extension-task-item";
import { TaskList } from "@tiptap/extension-task-list";
import { Highlight } from "@tiptap/extension-highlight";
import { TableKit } from "@tiptap/extension-table";
import { Mathematics } from "@tiptap/extension-mathematics";
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

export function editorExtensions(o: SetupOptions = {}): Extensions {
  return [
    StarterKit.configure({
      paragraph: false,
      heading: { levels: [1, 2, 3, 4] },
      link: { openOnClick: false, autolink: true },
    }),
    SafeParagraph,
    Markdown,
    CjkEmphasis,
    Placeholder.configure({
      placeholder: ({ node }) => o.placeholder?.(node.type.name === "heading") ?? "",
    }),
    Focus.configure({ className: "has-focus", mode: "shallowest" }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Highlight,
    ResourceImage,
    TableKit.configure({ table: { resizable: false } }),
    Mathematics.configure({
      katexOptions: { throwOnError: false },
      inlineOptions: { onClick: (node, pos) => o.onEditMath?.("inline", node.attrs.latex as string, pos) },
      blockOptions: { onClick: (node, pos) => o.onEditMath?.("block", node.attrs.latex as string, pos) },
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
