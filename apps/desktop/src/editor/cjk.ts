// Making the Markdown the editor writes read back the same.
//
// - Emphasis next to punctuation: in Markdown (CommonMark, as Joplin reads
//   it) `は**「待て」**と` is not bold, because the `**` sits between a letter
//   and punctuation. Emphasis is therefore kept off punctuation at the edges
//   of an emphasized run when the text outside is a letter: `は「**待て**」と`.
// - Paragraphs whose text starts like a list, heading or quote (`1. `, `- `,
//   `# `, `> `) are written with the marker escaped, so that they stay
//   paragraphs.

import { Extension } from "@tiptap/core";
import { Paragraph } from "@tiptap/extension-paragraph";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { MarkType, Node as PMNode } from "@tiptap/pm/model";

const PUNCT = /[\p{P}\p{S}]/u;
const SPACE = /\s/u;

const isPunct = (c: string | undefined) => c !== undefined && PUNCT.test(c);
const isFlankFree = (c: string | undefined) => c === undefined || SPACE.test(c) || PUNCT.test(c);

/** Ranges where `type` must be removed in the textblock `node` at `pos`. */
function fixes(node: PMNode, pos: number, type: MarkType): [number, number][] {
  // One entry per character (text) or inline node.
  const chars: { ch: string | undefined; marked: boolean; pos: number }[] = [];
  node.forEach((child, offset) => {
    const marked = type.isInSet(child.marks) !== undefined;
    if (child.isText && child.text) {
      for (let i = 0; i < child.text.length; i++) chars.push({ ch: child.text[i], marked, pos: pos + 1 + offset + i });
    } else chars.push({ ch: "x", marked, pos: pos + 1 + offset });
  });
  const out: [number, number][] = [];
  let i = 0;
  while (i < chars.length) {
    if (!chars[i].marked) {
      i++;
      continue;
    }
    let j = i;
    while (j < chars.length && chars[j].marked) j++;
    // The run is chars[i..j).
    const before = chars[i - 1]?.ch;
    const after = chars[j]?.ch;
    let start = i;
    let end = j;
    if (!isFlankFree(before)) while (start < end && isPunct(chars[start].ch)) start++;
    if (!isFlankFree(after)) while (end > start && isPunct(chars[end - 1].ch)) end--;
    if (start > i) out.push([chars[i].pos, chars[start - 1].pos + 1]);
    if (end < j && end >= start) out.push([chars[end].pos, chars[j - 1].pos + 1]);
    i = j;
  }
  return out;
}

export const CjkEmphasis = Extension.create({
  name: "cjkEmphasis",
  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("cjkEmphasis"),
        appendTransaction(transactions, _old, state) {
          if (!transactions.some((t) => t.docChanged || t.storedMarksSet)) return null;
          const types = ["bold", "italic", "strike"].map((n) => state.schema.marks[n]).filter(Boolean);
          const tr = state.tr;
          state.doc.descendants((node, pos) => {
            if (!node.isTextblock) return true;
            if (node.type.spec.code) return false;
            for (const type of types) for (const [from, to] of fixes(node, pos, type)) tr.removeMark(from, to, type);
            return false;
          });
          return tr.docChanged ? tr : null;
        },
      }),
    ];
  },
});

/** Escapes what would make a paragraph line read as another block. */
export function escapeBlockStart(markdown: string): string {
  return markdown
    .split("\n")
    .map((line) =>
      line
        .replace(/^(\s*)(\d{1,9})([.)])(\s|$)/, "$1$2\\$3$4")
        .replace(/^(\s*)([-+*])(\s|$)/, "$1\\$2$3")
        .replace(/^(\s*)(#{1,6})(\s|$)/, "$1\\$2$3")
        .replace(/^(\s*)>/, "$1\\>")
        .replace(/^(\s*)(```|~~~)/, "$1\\$2")
        .replace(/^(\s*)(=+|-{2,})\s*$/, "$1\\$2"),
    )
    .join("\n");
}

export const SafeParagraph = Paragraph.extend({
  renderMarkdown(node, h, ctx) {
    const parent = this.parent as ((n: typeof node, helpers: typeof h, c: typeof ctx) => string) | undefined;
    const out = parent ? parent(node, h, ctx) : h.renderChildren(node.content ?? []);
    return escapeBlockStart(out);
  },
});

/**
 * Markdown reduced to what matters, to tell whether the editor would change
 * a note (spacing in tables, line break styles and list markers don't count).
 */
export function normalizeMarkdown(md: string): string {
  return md
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((line) => {
      if (/^\s*\|/.test(line)) {
        return line
          .trim()
          .replace(/\s*\|\s*/g, "|")
          .replace(/:?-{3,}:?/g, (m) => m.replace(/-+/, "---"));
      }
      return line.replace(/^(\s*)[*+] /, "$1- ").replace(/^(\s*)(\d+)\) /, "$1$2. ");
    })
    .join("\n")
    .replace(/\\\n/g, "\n")
    .replace(/ {2,}\n/g, "\n")
    .replace(/__(.+?)__/g, "**$1**")
    .replace(/(^|[^*])_([^_\n]+)_(?!_)/g, "$1*$2*")
    .replace(/[ \t]+$/gm, "")
    .replace(/\\(.)/g, "$1")
    .replace(/\n{2,}/g, "\n\n")
    .trim();
}

function unescapeMarkdownText(s: string): string {
  return s
    .replace(/\\([!-/:-@[-`{-~])/g, "$1")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&amp;/g, "&");
}

/**
 * Links whose text is their own address (bare URLs, as Joplin and other apps
 * write them, which the editor turns into links) are written back as the bare
 * URL rather than `[url](url)` with the text escaped. Only before a space or
 * the end of a line, where the bare URL reads back as the same link.
 */
export function simplifyBareLinks(md: string): string {
  return md.replace(/\[([^\]\n]+)\]\(((?:https?:\/\/|mailto:)[^)\s]+)\)(?=\s|$)/gi, (all, text: string, href: string) =>
    unescapeMarkdownText(text) === href ? href : all,
  );
}

/** The editor's content as Markdown, as it is saved. */
export function editorMarkdown(editor: { getMarkdown: () => string }): string {
  return simplifyBareLinks(editor.getMarkdown());
}
