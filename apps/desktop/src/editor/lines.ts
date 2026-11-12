// Editing comforts of the rich editor: paragraph numbers in the margin,
// bracket pairs (「」 and the like), moving paragraphs with Alt+↑/↓, and
// jumping to a numbered paragraph.

import { Extension } from "@tiptap/core";
import type { Editor } from "@tiptap/core";
import { Plugin, PluginKey, TextSelection, type EditorState, type Transaction } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import type { Node as PMNode } from "@tiptap/pm/model";

// -------------------------------------------------------------------------
// Numbered units

/**
 * Start positions of the numbered units: every textblock outside tables, and
 * each table as one. The same order as the numbers in the margin.
 */
export function numberedStarts(doc: PMNode): number[] {
  const out: number[] = [];
  doc.descendants((node, pos) => {
    if (node.type.name === "table") {
      out.push(pos);
      return false;
    }
    if (node.isTextblock) {
      out.push(pos);
      return false;
    }
    return true;
  });
  return out;
}

const startsCache = new WeakMap<PMNode, number[]>();
/** `numberedStarts`, computed once per version of the document. */
function startsOf(doc: PMNode): number[] {
  let s = startsCache.get(doc);
  if (!s) {
    s = numberedStarts(doc);
    startsCache.set(doc, s);
  }
  return s;
}

/** Number (1-based) of the unit holding `pos`. */
export function numberAt(doc: PMNode, pos: number): number {
  const starts = startsOf(doc);
  // The last start at or before pos (binary search).
  let lo = 0;
  let hi = starts.length - 1;
  let n = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (starts[mid] <= pos) {
      n = mid + 1;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return Math.max(1, n);
}

export function unitCount(doc: PMNode): number {
  return startsOf(doc).length;
}

/** Puts the cursor at the start of unit `n` and scrolls to it. */
export function goToNumber(editor: Editor, n: number): boolean {
  const starts = numberedStarts(editor.state.doc);
  if (starts.length === 0) return false;
  const start = starts[Math.min(Math.max(n, 1), starts.length) - 1];
  const node = editor.state.doc.nodeAt(start);
  // Inside the block: past its opening (and, for a table, its first cell's).
  let inside = start + 1;
  if (node?.type.name === "table") {
    const $p = editor.state.doc.resolve(start + 1);
    inside = TextSelection.findFrom($p, 1)?.from ?? start + 1;
  }
  editor.chain().focus().setTextSelection(inside).scrollIntoView().run();
  // Bring it towards the middle of the screen rather than the very edge.
  requestAnimationFrame(() => {
    const coords = editor.view.coordsAtPos(editor.state.selection.head);
    const scroller = editor.view.dom.closest(".editor-scroll");
    if (scroller) {
      const box = scroller.getBoundingClientRect();
      scroller.scrollTop += coords.top - (box.top + box.height * 0.35);
    }
  });
  return true;
}

// -------------------------------------------------------------------------
// Line numbers in the margin

const lineNumbersKey = new PluginKey("lineNumbers");

/**
 * How far below the top of a line of `font` (its content box, as the caret
 * rectangle has it) the baseline is. Measured once per font: the numbers sit
 * on the text's baseline, as a number written in the text would (centring
 * the small digits made them look off by a pixel or two).
 */
const baselines = new Map<string, number>();
/**
 * `glyph`: from the top of the character's own box (what a caret rectangle
 * covers) to the baseline. Otherwise from the top of a block of that font
 * with normal line height (how a number is laid out in the gutter).
 */
function baselineOffset(el: Element, sample: string, glyph: boolean): number {
  // (Per character too: a font stack draws Latin and kana with different
  // fonts, whose heights differ. And per language: the fallback font for a
  // kanji depends on `lang`, and the app's can differ from the page's.)
  const font = fontOf(el);
  const lang = el.closest("[lang]")?.getAttribute("lang") ?? "";
  const key = `${font}|${lang}|${sample}|${glyph}`;
  const known = baselines.get(key);
  if (known !== undefined) return known;
  const probe = document.createElement("div");
  probe.style.cssText = "position:absolute;left:-9999px;top:0;visibility:hidden;white-space:nowrap;";
  probe.style.font = font;
  if (lang) probe.lang = lang;
  probe.style.lineHeight = "normal";
  const text = document.createElement("span");
  text.textContent = sample;
  const mark = document.createElement("span");
  mark.style.cssText = "display:inline-block;width:1px;height:0;vertical-align:baseline;";
  probe.append(text, mark);
  document.body.appendChild(probe);
  const value = glyph ? mark.getBoundingClientRect().top - text.getBoundingClientRect().top : mark.offsetTop;
  probe.remove();
  baselines.set(key, value);
  return value;
}

function fontOf(el: Element): string {
  const c = getComputedStyle(el);
  return `${c.fontStyle} ${c.fontWeight} ${c.fontSize} ${c.fontFamily}`;
}

class Gutter {
  private el: HTMLDivElement;
  private spans: HTMLSpanElement[] = [];
  private frame = 0;
  private observer: ResizeObserver;

  constructor(
    private view: EditorView,
    private enabled: () => boolean,
  ) {
    this.el = document.createElement("div");
    this.el.className = "line-gutter";
    this.el.setAttribute("aria-hidden", "true");
    this.el.contentEditable = "false";
    this.mount();
    this.observer = new ResizeObserver(() => this.schedule());
    this.observer.observe(view.dom);
    this.schedule();
  }

  private scroller: Element | null = null;
  private onScroll = () => this.schedule();

  private mount() {
    const parent = this.view.dom.parentElement;
    if (parent && this.el.parentElement !== parent) parent.insertBefore(this.el, this.view.dom);
    // Numbers come into view as the text scrolls.
    const scroller = this.view.dom.closest(".editor-scroll");
    if (scroller !== this.scroller) {
      this.scroller?.removeEventListener("scroll", this.onScroll);
      scroller?.addEventListener("scroll", this.onScroll, { passive: true });
      this.scroller = scroller;
    }
  }

  schedule() {
    cancelAnimationFrame(this.frame);
    this.frame = requestAnimationFrame(() => this.draw());
  }

  private draw() {
    this.mount();
    const on = this.enabled() && this.view.dom.isConnected;
    this.el.hidden = !on;
    if (!on) return;
    const { doc, selection } = this.view.state;
    const starts = startsOf(doc);
    const parent = this.el.parentElement;
    if (!parent) return;
    const origin = parent.getBoundingClientRect().top;
    const current = numberAt(doc, selection.head) - 1;

    // Only the units on screen (and a screen's worth around them) are measured.
    const scroller = this.view.dom.closest(".editor-scroll");
    const box = scroller?.getBoundingClientRect() ?? { top: 0, bottom: window.innerHeight, left: 0, width: window.innerWidth };
    const margin = box.bottom - box.top;
    const x = this.view.dom.getBoundingClientRect().left + 4;
    const posAt = (y: number, fallback: number) => this.view.posAtCoords({ left: x, top: y })?.pos ?? fallback;
    const fromPos = posAt(Math.max(box.top - margin, this.view.dom.getBoundingClientRect().top + 1), 0);
    const toPos = posAt(Math.min(box.bottom + margin, this.view.dom.getBoundingClientRect().bottom - 1), doc.content.size);
    let first = 0;
    while (first < starts.length - 1 && starts[first + 1] <= fromPos) first++;
    let last = first;
    while (last < starts.length - 1 && starts[last + 1] <= toPos) last++;
    const shown = starts.slice(first, last + 1);

    while (this.spans.length < shown.length) {
      const s = document.createElement("span");
      this.el.appendChild(s);
      this.spans.push(s);
    }
    while (this.spans.length > shown.length) this.spans.pop()!.remove();

    shown.forEach((start, k) => {
      const i = first + k;
      const s = this.spans[k];
      const node = doc.nodeAt(start);
      let top: number;
      try {
        const dom = this.view.nodeDOM(start) as HTMLElement | null;
        if (node?.isTextblock && dom) {
          // The first line of the block: the number's baseline on its baseline.
          const c = this.view.coordsAtPos(start + 1);
          const first = node.textContent.slice(0, 1) || "あ";
          top = c.top + baselineOffset(dom, first, true) - baselineOffset(s, "1", false);
        } else {
          const r = dom?.getBoundingClientRect();
          top = r?.top ?? 0;
        }
      } catch {
        s.hidden = true;
        return;
      }
      s.hidden = false;
      const text = String(i + 1);
      if (s.textContent !== text) s.textContent = text;
      s.style.transform = `translateY(${(top - origin).toFixed(1)}px)`;
      s.style.height = "";
      s.style.lineHeight = "normal";
      s.classList.toggle("current", i === current && this.view.hasFocus());
    });
  }

  destroy() {
    cancelAnimationFrame(this.frame);
    this.scroller?.removeEventListener("scroll", this.onScroll);
    this.observer.disconnect();
    this.el.remove();
  }
}

export const LineNumbers = Extension.create<{ enabled: () => boolean }>({
  name: "lineNumbers",
  addOptions() {
    return { enabled: () => false };
  },
  addProseMirrorPlugins() {
    const enabled = this.options.enabled;
    return [
      new Plugin({
        key: lineNumbersKey,
        view(view) {
          const gutter = new Gutter(view, enabled);
          const redraw = () => gutter.schedule();
          view.dom.addEventListener("focus", redraw);
          view.dom.addEventListener("blur", redraw);
          window.addEventListener("resize", redraw);
          // Images load and fonts swap after the text is laid out.
          view.dom.addEventListener("load", redraw, true);
          return {
            update: redraw,
            destroy() {
              view.dom.removeEventListener("focus", redraw);
              view.dom.removeEventListener("blur", redraw);
              window.removeEventListener("resize", redraw);
              view.dom.removeEventListener("load", redraw, true);
              gutter.destroy();
            },
          };
        },
      }),
    ];
  },
});

/** Redraws the numbers after a setting changed. */
export function refreshLineNumbers(editor: Editor | null) {
  if (editor && !editor.isDestroyed) editor.view.dispatch(editor.state.tr.setMeta(lineNumbersKey, true));
}

// -------------------------------------------------------------------------
// Bracket pairs

/** Opening bracket → closing bracket. */
export const PAIRS: Record<string, string> = {
  "「": "」",
  "『": "』",
  "（": "）",
  "【": "】",
  "〈": "〉",
  "《": "》",
  "〔": "〕",
  "“": "”",
  "(": ")",
  "[": "]",
};
const CLOSERS = new Set(Object.values(PAIRS));

interface PairState {
  /** Positions of closing brackets that were inserted automatically (and can be typed over). */
  auto: number[];
}

const pairKey = new PluginKey<PairState>("autoPair");

function charAt(state: EditorState, pos: number): string {
  const $pos = state.doc.resolve(pos);
  if (!$pos.parent.isTextblock) return "";
  return $pos.parent.textBetween($pos.parentOffset, Math.min($pos.parentOffset + 1, $pos.parent.content.size), undefined, "￼");
}

function charBefore(state: EditorState, pos: number): string {
  const $pos = state.doc.resolve(pos);
  if (!$pos.parent.isTextblock || $pos.parentOffset === 0) return "";
  return $pos.parent.textBetween($pos.parentOffset - 1, $pos.parentOffset, undefined, "￼");
}

/** A closing bracket may follow when the next character doesn't continue a word. */
function closerFits(next: string): boolean {
  return next === "" || /[\s\p{P}\p{S}]/u.test(next) || CLOSERS.has(next);
}

function inCode(state: EditorState): boolean {
  const $from = state.selection.$from;
  return !!$from.parent.type.spec.code || state.schema.marks.code?.isInSet($from.marks()) !== undefined;
}

/**
 * The closer for the last bracket in `text` that is still open at its end
 * ("「会話" → "」"; "「会話」" → none). ASCII brackets aren't paired here.
 */
export function openBracket(text: string): string | null {
  const stack: string[] = [];
  for (const ch of text) {
    const close = PAIRS[ch];
    if (close && ch !== "(" && ch !== "[") stack.push(close);
    else if (stack.length && ch === stack[stack.length - 1]) stack.pop();
  }
  return stack.length ? stack[stack.length - 1] : null;
}

function remember(tr: Transaction, pos: number) {
  tr.setMeta(pairKey, { add: pos });
}

export const AutoPair = Extension.create<{ enabled: () => boolean }>({
  name: "autoPair",
  addOptions() {
    return { enabled: () => true };
  },
  addProseMirrorPlugins() {
    const enabled = this.options.enabled;
    return [
      new Plugin<PairState>({
        key: pairKey,
        state: {
          init: () => ({ auto: [] }),
          apply(tr, prev, _old, state) {
            const meta = tr.getMeta(pairKey) as { add?: number; drop?: number } | undefined;
            // Text typed right before a closer pushes it along (assoc 1).
            let auto = tr.docChanged ? prev.auto.map((p) => tr.mapping.map(p, 1)) : prev.auto;
            if (meta?.drop !== undefined) auto = auto.filter((p) => p !== meta.drop);
            if (meta?.add !== undefined) auto = [...auto, meta.add];
            // Only closers still in place, near the cursor's paragraph.
            const $head = state.selection.$head;
            const start = $head.start();
            const end = $head.end();
            auto = auto.filter((p) => p >= start && p < end && CLOSERS.has(charAt(state, p)));
            return auto.length === prev.auto.length && auto.every((p, i) => p === prev.auto[i]) ? prev : { auto };
          },
        },
        props: {
          handleTextInput(view, from, to, text) {
            if (!enabled() || text.length !== 1 || view.composing) return false;
            const state = view.state;
            if (inCode(state)) return false;
            const close = PAIRS[text];
            // Typing over an automatic closer.
            if (!close && CLOSERS.has(text) && from === to) {
              const auto = pairKey.getState(state)?.auto ?? [];
              if (charAt(state, from) === text && auto.includes(from)) {
                const tr = state.tr.setSelection(TextSelection.create(state.doc, from + 1));
                tr.setMeta(pairKey, { drop: from });
                view.dispatch(tr);
                return true;
              }
              return false;
            }
            if (!close) return false;
            // Wrapping the selection.
            if (from !== to) {
              const tr = state.tr.insertText(close, to).insertText(text, from);
              tr.setSelection(TextSelection.create(tr.doc, from + 1, to + 1));
              view.dispatch(tr);
              return true;
            }
            // ASCII quotes and brackets only where they open something.
            if ((text === "(" || text === "[") && /[\p{L}\p{N}]/u.test(charBefore(state, from))) return false;
            if (!closerFits(charAt(state, from))) return false;
            const tr = state.tr.insertText(text + close, from, to);
            tr.setSelection(TextSelection.create(tr.doc, from + 1));
            remember(tr, from + 1);
            view.dispatch(tr);
            return true;
          },
          handleKeyDown(view, event) {
            if (!enabled() || event.key !== "Backspace" || event.isComposing) return false;
            const state = view.state;
            const { empty, from } = state.selection;
            if (!empty) return false;
            const before = charBefore(state, from);
            const close = PAIRS[before];
            if (!close || charAt(state, from) !== close) return false;
            // An empty pair goes as a whole.
            view.dispatch(state.tr.delete(from - 1, from + 1));
            return true;
          },
          handleDOMEvents: {
            // Input methods: the bracket arrives when the composition ends.
            compositionend(view, event) {
              if (!enabled()) return false;
              const data = (event as CompositionEvent).data ?? "";
              if (!data) return false;
              setTimeout(() => {
                const state = view.state;
                if (view.composing || !state.selection.empty || inCode(state)) return;
                const pos = state.selection.from;
                if (charBefore(state, pos) !== data.slice(-1)) return;
                const last = data.slice(-1);
                const auto = pairKey.getState(state)?.auto ?? [];
                // A closer typed just before the automatic one: keep one.
                if (CLOSERS.has(last) && charAt(state, pos) === last && auto.includes(pos)) {
                  const tr = state.tr.delete(pos, pos + 1);
                  tr.setMeta(pairKey, { drop: pos });
                  view.dispatch(tr);
                  return;
                }
                // The last bracket left open in what was committed ("「", or "「会話" at once).
                const close = openBracket(data);
                if (!close || !closerFits(charAt(state, pos))) return;
                const tr = state.tr.insertText(close, pos);
                tr.setSelection(TextSelection.create(tr.doc, pos));
                remember(tr, pos);
                view.dispatch(tr);
              }, 0);
              return false;
            },
          },
        },
      }),
    ];
  },
});

// -------------------------------------------------------------------------
// Moving paragraphs

/** The block to move: a list item, or a block of the document (or a quote). */
function movableAt(state: EditorState): { depth: number } | null {
  const $from = state.selection.$from;
  for (let d = $from.depth; d > 0; d--) {
    const node = $from.node(d);
    const parent = $from.node(d - 1);
    if (node.type.name === "tableCell" || node.type.name === "tableHeader" || node.type.name === "tableRow") return null;
    if (node.type.name === "listItem" || node.type.name === "taskItem") return { depth: d };
    if (parent.type.name === "doc" || parent.type.name === "blockquote") return { depth: d };
  }
  return null;
}

export function moveBlock(view: EditorView, dir: -1 | 1): boolean {
  const state = view.state;
  const found = movableAt(state);
  if (!found) return false;
  const { depth } = found;
  const $from = state.selection.$from;
  const parent = $from.node(depth - 1);
  const index = $from.index(depth - 1);
  const target = index + dir;
  if (target < 0 || target >= parent.childCount) return true; // at the edge: nothing to do, but keep the key
  const node = parent.child(index);
  const other = parent.child(target);
  const start = $from.before(depth);
  const end = start + node.nodeSize;
  const tr = state.tr;
  const offset = state.selection.from - start;
  const span = state.selection.to - state.selection.from;
  const newStart = dir === -1 ? start - other.nodeSize : start + other.nodeSize;
  tr.delete(start, end).insert(newStart, node);
  const at = newStart + offset;
  try {
    tr.setSelection(state.selection instanceof TextSelection ? TextSelection.create(tr.doc, at, at + span) : TextSelection.near(tr.doc.resolve(at)));
  } catch {
    tr.setSelection(TextSelection.near(tr.doc.resolve(Math.min(at, tr.doc.content.size))));
  }
  view.dispatch(tr.scrollIntoView());
  return true;
}

export const MoveBlock = Extension.create({
  name: "moveBlock",
  addKeyboardShortcuts() {
    return {
      "Alt-ArrowUp": () => moveBlock(this.editor.view, -1),
      "Alt-ArrowDown": () => moveBlock(this.editor.view, 1),
    };
  },
});
