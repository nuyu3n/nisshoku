// A plain-text editing area (Markdown source, diary pages) with the comforts
// of the rich editor: line numbers that follow wrapped lines, bracket pairs,
// and moving lines with Alt+↑/↓.

import { forwardRef, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type TextareaHTMLAttributes } from "react";
import { PAIRS, openBracket } from "../editor/lines";
import { useStore } from "../store";

const CLOSERS = new Set(Object.values(PAIRS));

/** Inserts text as typing would (so that Ctrl+Z undoes it). */
function typeText(el: HTMLTextAreaElement, text: string, from: number, to: number) {
  el.setSelectionRange(from, to);
  if (!document.execCommand("insertText", false, text)) {
    el.setRangeText(text, from, to, "end");
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }
}

/** Line (0-based) holding offset `pos`. */
function lineOf(text: string, pos: number): number {
  let n = 0;
  for (let i = 0; i < pos && i < text.length; i++) if (text.charCodeAt(i) === 10) n++;
  return n;
}

export interface TextLinesHandle {
  textarea: HTMLTextAreaElement | null;
  /** Puts the cursor at the start of line `n` (1-based) and scrolls to it. */
  goToLine: (n: number) => void;
}

type Props = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "value" | "onChange"> & {
  value: string;
  onChange: (v: string) => void;
  /** Minimum height in pixels. */
  minHeight?: number;
};

export const TextLines = forwardRef<TextLinesHandle, Props>(function TextLines({ value, onChange, minHeight = 0, className, onKeyDown, ...rest }, handle) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const numbers = useStore((s) => s.prefs.lineNumbers);
  const autoPair = useStore((s) => s.prefs.autoPair);
  const [line, setLine] = useState(-1);
  const auto = useRef<number | null>(null); // offset of an automatic closer

  // Height follows the content. Measuring from "auto" lays out the whole
  // text again, which is slow for long notes: growing needs no reset, so
  // that is done only when text got shorter (or the width changed).
  const lastLength = useRef(-1);
  const fit = useCallback((reset: boolean) => {
    const el = ref.current;
    if (!el) return;
    if (reset) {
      const scroller = el.closest(".editor-scroll");
      const keep = scroller?.scrollTop ?? 0;
      el.style.height = "auto";
      el.style.height = `${Math.max(el.scrollHeight, minHeight)}px`;
      if (scroller) scroller.scrollTop = keep;
    } else if (el.scrollHeight > el.clientHeight) {
      el.style.height = `${Math.max(el.scrollHeight, minHeight)}px`;
    }
  }, [minHeight]);
  useLayoutEffect(() => {
    const shorter = value.length < lastLength.current || lastLength.current < 0;
    lastLength.current = value.length;
    fit(shorter);
  }, [value, fit]);
  useLayoutEffect(() => fit(true), [numbers, fit]);
  useEffect(() => {
    const el = ref.current?.parentElement;
    if (!el) return;
    let width = el.clientWidth;
    const observer = new ResizeObserver(() => {
      if (el.clientWidth === width) return;
      width = el.clientWidth;
      fit(true);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [fit]);

  const track = useCallback(() => {
    const el = ref.current;
    if (el) setLine(document.activeElement === el ? lineOf(el.value, el.selectionStart) : -1);
  }, []);
  useEffect(() => {
    document.addEventListener("selectionchange", track);
    return () => document.removeEventListener("selectionchange", track);
  }, [track]);

  useImperativeHandle(handle, () => ({
    get textarea() {
      return ref.current;
    },
    goToLine(n) {
      const el = ref.current;
      if (!el) return;
      const lines = el.value.split("\n");
      const target = Math.min(Math.max(n, 1), lines.length) - 1;
      let pos = 0;
      for (let i = 0; i < target; i++) pos += lines[i].length + 1;
      el.focus({ preventScroll: true });
      el.setSelectionRange(pos, pos);
      requestAnimationFrame(() => {
        const row = el.parentElement?.querySelector<HTMLElement>(`.line-mirror > div:nth-child(${target + 1})`);
        const scroller = el.closest(".editor-scroll");
        if (!scroller) return;
        const box = scroller.getBoundingClientRect();
        const top = row ? row.getBoundingClientRect().top : el.getBoundingClientRect().top;
        scroller.scrollTop += top - (box.top + box.height * 0.35);
      });
    },
  }));

  const moveLines = (el: HTMLTextAreaElement, dir: -1 | 1) => {
    const text = el.value;
    const { selectionStart: s, selectionEnd: e } = el;
    const start = text.lastIndexOf("\n", s - 1) + 1;
    let end = text.indexOf("\n", e > s && text[e - 1] === "\n" ? e - 1 : e);
    if (end === -1) end = text.length;
    const block = text.slice(start, end);
    if (dir === -1) {
      if (start === 0) return;
      const prevStart = text.lastIndexOf("\n", start - 2) + 1;
      const prev = text.slice(prevStart, start - 1);
      typeText(el, `${block}\n${prev}`, prevStart, end);
      el.setSelectionRange(s - prev.length - 1, e - prev.length - 1);
    } else {
      if (end >= text.length) return;
      let nextEnd = text.indexOf("\n", end + 1);
      if (nextEnd === -1) nextEnd = text.length;
      const next = text.slice(end + 1, nextEnd);
      typeText(el, `${next}\n${block}`, start, nextEnd);
      el.setSelectionRange(s + next.length + 1, e + next.length + 1);
    }
  };

  const keyDown = (ev: React.KeyboardEvent<HTMLTextAreaElement>) => {
    onKeyDown?.(ev);
    if (ev.defaultPrevented || rest.readOnly) return;
    const el = ev.currentTarget;
    const native = ev.nativeEvent;
    if (native.isComposing) return;
    if (ev.altKey && !ev.ctrlKey && !ev.metaKey && (ev.key === "ArrowUp" || ev.key === "ArrowDown")) {
      ev.preventDefault();
      moveLines(el, ev.key === "ArrowUp" ? -1 : 1);
      return;
    }
    if (!autoPair || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    const { selectionStart: s, selectionEnd: e, value: text } = el;
    if (ev.key === "Backspace" && s === e && s > 0) {
      const close = PAIRS[text[s - 1]];
      if (close && text[s] === close) {
        ev.preventDefault();
        typeText(el, "", s - 1, s + 1);
      }
      return;
    }
    if (ev.key.length !== 1) return;
    const close = PAIRS[ev.key];
    if (!close && CLOSERS.has(ev.key) && s === e && text[s] === ev.key && auto.current === s) {
      ev.preventDefault();
      el.setSelectionRange(s + 1, s + 1);
      auto.current = null;
      return;
    }
    if (!close) return;
    if (s !== e) {
      ev.preventDefault();
      const inner = text.slice(s, e);
      typeText(el, ev.key + inner + close, s, e);
      el.setSelectionRange(s + 1, e + 1);
      return;
    }
    if ((ev.key === "(" || ev.key === "[") && /[\p{L}\p{N}]/u.test(text[s - 1] ?? "")) return;
    if (!fits(text[s])) return;
    ev.preventDefault();
    typeText(el, ev.key + close, s, e);
    el.setSelectionRange(s + 1, s + 1);
    auto.current = s + 1;
  };

  const compositionEnd = (ev: React.CompositionEvent<HTMLTextAreaElement>) => {
    if (!autoPair || rest.readOnly) return;
    const el = ev.currentTarget;
    const data = ev.data ?? "";
    if (!data) return;
    setTimeout(() => {
      const { selectionStart: s, selectionEnd: e, value: text } = el;
      if (s !== e || text.slice(s - data.length, s) !== data) return;
      const last = data.slice(-1);
      if (CLOSERS.has(last) && text[s] === last && auto.current === s) {
        typeText(el, "", s, s + 1);
        auto.current = null;
        return;
      }
      const close = openBracket(data);
      if (!close || !fits(text[s])) return;
      typeText(el, close, s, s);
      el.setSelectionRange(s, s);
      auto.current = s;
    }, 0);
  };

  const lines = numbers ? value.split("\n") : [];
  return (
    <div className={`text-lines${numbers ? " numbered" : ""}`}>
      {numbers && (
        <div className={`line-mirror ${className ?? ""}`} aria-hidden="true">
          {lines.map((l, i) => (
            <div key={i} className={i === line ? "current" : undefined}>
              {l || "​"}
            </div>
          ))}
        </div>
      )}
      <textarea
        {...rest}
        ref={ref}
        className={className}
        value={value}
        onChange={(e) => {
          // Keep track of the automatic closer as text is typed before it.
          const a = auto.current;
          if (a !== null) {
            const next = e.target.value;
            const delta = next.length - value.length;
            const editAt = e.target.selectionStart - Math.max(delta, 0);
            const moved = editAt <= a ? a + delta : a;
            auto.current = CLOSERS.has(next[moved] ?? "") ? moved : null;
          }
          onChange(e.target.value);
        }}
        onKeyDown={keyDown}
        onCompositionEnd={compositionEnd}
        onFocus={track}
        onBlur={() => setLine(-1)}
      />
    </div>
  );
});

function fits(next: string | undefined): boolean {
  return next === undefined || next === "" || /[\s\p{P}\p{S}]/u.test(next) || CLOSERS.has(next);
}
