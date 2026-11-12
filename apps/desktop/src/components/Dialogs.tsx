// Promise-based dialogs (the webview's prompt() and confirm() are not
// dependable), rendered once at the app root.

import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import katex from "katex";
import { useT } from "../i18n";
import type { MathGroupId } from "./MathSheet";
import { DatePicker, TimePicker } from "./Pickers";
import { EmojiPicker } from "./EmojiPicker";
import { MathSheet } from "./MathSheet";
import { isMobile } from "../lib/mobile";

interface DialogRequest {
  kind: "prompt" | "confirm" | "choose" | "math" | "date" | "time" | "emoji";
  /** For math: display (block) or inline. */
  display?: boolean;
  title: string;
  message?: string;
  initial?: string;
  placeholder?: string;
  multiline?: boolean;
  password?: boolean;
  okLabel?: string;
  danger?: boolean;
  /** `path`: where a nested choice is ("A / B / C"), shown and searched while filtering. */
  choices?: { value: string; label: string; depth?: number; path?: string }[];
  /** Choices as a grid of small tiles (emoji) instead of a list. */
  grid?: boolean;
  /** For date: the last day that can be chosen (YYYY-MM-DD). */
  max?: string;
  resolve: (value: string | null) => void;
}

const useDialog = create<{ current: DialogRequest | null }>(() => ({ current: null }));

function ask(req: Omit<DialogRequest, "resolve">): Promise<string | null> {
  return new Promise((resolve) => useDialog.setState({ current: { ...req, resolve } }));
}

export const dialogs = {
  prompt: (title: string, opts: { initial?: string; placeholder?: string; message?: string; okLabel?: string; multiline?: boolean; password?: boolean } = {}) =>
    ask({ kind: "prompt", title, ...opts }),
  confirm: async (title: string, message?: string, opts: { okLabel?: string; danger?: boolean } = {}) =>
    (await ask({ kind: "confirm", title, message, ...opts })) !== null,
  /** Edits a TeX formula with a preview. `""` means delete; `null`, cancel. */
  math: (title: string, latex: string, display: boolean, canDelete: boolean) =>
    ask({ kind: "math", title, initial: latex, display, danger: canDelete }),
  choose: (title: string, choices: DialogRequest["choices"], message?: string, layout?: "grid") =>
    ask({ kind: "choose", title, choices, message, grid: layout === "grid" }),
  /** A day (YYYY-MM-DD), or `null` if cancelled. */
  date: (title: string, initial: string, opts: { max?: string; okLabel?: string } = {}) => ask({ kind: "date", title, initial, ...opts }),
  /** A time of day (HH:MM), or `null` if cancelled. */
  time: (title: string, initial: string) => ask({ kind: "time", title, initial }),
  /** An emoji; `""` for none, `null` if cancelled. */
  emoji: (title: string, current: string) => ask({ kind: "emoji", title, initial: current }),
};

/** Cancels the open dialog, if any. */
export function cancelDialog(): boolean {
  const current = useDialog.getState().current;
  if (!current) return false;
  useDialog.setState({ current: null });
  current.resolve(null);
  return true;
}

export const dialogOpen = () => useDialog.getState().current !== null;
export const onDialogChange = (f: () => void) => useDialog.subscribe(f);

export function DialogHost() {
  const current = useDialog((s) => s.current);
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  const t = useT();

  useEffect(() => {
    if (current) {
      setValue(current.initial ?? "");
      requestAnimationFrame(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      });
    }
  }, [current]);

  if (!current) return null;
  const close = (result: string | null) => {
    useDialog.setState({ current: null });
    current.resolve(result);
  };
  // Phones: math in a sheet of its own (components/MathSheet.tsx).
  if (current.kind === "math" && isMobile) {
    return <MathSheet key={current.title + current.initial} title={current.title} initial={current.initial ?? ""} display={!!current.display} canDelete={!!current.danger} onClose={close} />;
  }
  const ok = () => {
    if (current.kind === "math") {
      close(value.trim());
      return;
    }
    if (current.kind === "prompt") {
      if (value.trim()) close(value.trim());
    } else if (current.kind === "date" || current.kind === "time") close(value || current.initial!);
    else close("ok");
  };

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && close(null)}>
      <div
        className={`dialog ${current.kind === "emoji" ? "emoji-dialog" : "small"}`}
        onKeyDown={(e) => {
          if (e.key === "Escape") close(null);
          if (e.nativeEvent.isComposing || e.key !== "Enter" || current.kind === "choose" || current.kind === "emoji") return;
          // In a multi-line prompt, Enter adds a line; Ctrl+Enter confirms.
          if ((current.multiline || current.kind === "math") && !(e.ctrlKey || e.metaKey)) return;
          e.preventDefault();
          ok();
        }}
      >
        <div className="dialog-head">{current.title}</div>
        <div className="dialog-body">
          {current.message && <p>{current.message}</p>}
          {current.kind === "emoji" && <EmojiPicker current={current.initial ?? ""} onPick={close} />}
          {current.kind === "math" && (
            <>
              <MathPalette onInsert={(snippet) => {
                const el = inputRef.current;
                const start = el?.selectionStart ?? value.length;
                const end = el?.selectionEnd ?? value.length;
                const selected = value.slice(start, end);
                // `@` marks where the selection (or the cursor) goes.
                const [before, after = ""] = snippet.split("@");
                const next = value.slice(0, start) + before + selected + after + value.slice(end);
                setValue(next);
                requestAnimationFrame(() => {
                  el?.focus();
                  const cursor = start + before.length + selected.length;
                  el?.setSelectionRange(cursor, cursor);
                });
              }} />
              <textarea
                ref={inputRef}
                className="input mono"
                rows={current.display ? 5 : 2}
                value={value}
                placeholder={t("math.placeholder")}
                spellCheck={false}
                onChange={(e) => setValue(e.target.value)}
              />
              <div className="math-preview" aria-label={t("math.preview")}>
                <MathPreview latex={value} display={!!current.display} />
              </div>
            </>
          )}
          {/* `value` still holds the previous dialog's on the first render. */}
          {current.kind === "date" && <DatePicker value={/^\d{4}-\d\d-\d\d$/.test(value) ? value : current.initial!} max={current.max} onChange={setValue} onPick={close} />}
          {current.kind === "time" && <TimePicker value={/^\d\d:\d\d$/.test(value) ? value : current.initial!} onChange={setValue} />}
          {current.kind === "prompt" && current.multiline && (
            <textarea
              ref={inputRef}
              className="input"
              rows={4}
              value={value}
              placeholder={current.placeholder}
              onChange={(e) => setValue(e.target.value)}
            />
          )}
          {current.kind === "prompt" && !current.multiline && (
            <input
              ref={inputRef}
              className="input"
              type={current.password ? "password" : "text"}
              value={value}
              placeholder={current.placeholder}
              onChange={(e) => setValue(e.target.value)}
            />
          )}
          {current.kind === "choose" && !current.grid && (current.choices?.length ?? 0) > 8 && (
            <input
              ref={inputRef}
              className="input"
              style={{ marginBottom: 8 }}
              placeholder={t("palette.placeholder")}
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          )}
          {current.kind === "choose" && (
            <div className={current.grid ? "choice-tiles" : "stack"} style={current.grid ? undefined : { gap: 2, maxHeight: 340, overflowY: "auto" }}>
              {current.choices?.filter((c) => current.grid || !value || (c.path ?? c.label).toLowerCase().includes(value.toLowerCase())).map((c) => (
                <button
                  key={c.value}
                  className={current.grid ? "tile" : "menu-item"}
                  // While filtering, a nested choice shows where it is instead of its indent.
                  style={current.grid ? undefined : { paddingLeft: 10 + (value ? 0 : (c.depth ?? 0) * 16) }}
                  onClick={() => close(c.value)}
                >
                  {value && c.path ? c.path : c.label}
                </button>
              ))}
            </div>
          )}
        </div>
        {current.kind !== "choose" && (
          <div className="dialog-foot">
            {current.kind === "math" && current.danger && (
              <button className="btn danger" style={{ marginRight: "auto" }} onClick={() => close("")}>
                {t("math.delete")}
              </button>
            )}
            {current.kind === "emoji" && current.initial && (
              <button className="btn" style={{ marginRight: "auto" }} onClick={() => close("")}>
                {t("folder.noIcon")}
              </button>
            )}
            <button className="btn" onClick={() => close(null)}>
              {t("dialog.cancel")}
            </button>
            {current.kind !== "emoji" && <button className={`btn ${current.danger && current.kind !== "math" ? "danger" : "primary"}`} onClick={ok} autoFocus={current.kind === "confirm"}>
              {current.okLabel ?? t("dialog.ok")}
            </button>}
          </div>
        )}
      </div>
    </div>
  );
}

function MathPreview({ latex, display }: { latex: string; display: boolean }) {
  const t = useT();
  let html = "";
  let error: string | null = null;
  try {
    html = katex.renderToString(latex || "\\;", { displayMode: display, throwOnError: true });
  } catch (e) {
    error = e instanceof Error ? e.message.replace(/^KaTeX parse error: /, "") : String(e);
    html = katex.renderToString(latex || "\\;", { displayMode: display, throwOnError: false });
  }
  return (
    <>
      <div dangerouslySetInnerHTML={{ __html: html }} />
      {error && (
        <div className="math-error">
          {t("math.error")}: {error}
        </div>
      )}
    </>
  );
}

const MATH_GROUPS: [MathGroupId, [string, string][]][] = [
  ["structure", [["a/b", "\\frac{@}{}"], ["√", "\\sqrt{@}"], ["xⁿ", "^{@}"], ["xₙ", "_{@}"], ["( )", "\\left( @ \\right)"], ["Σ", "\\sum_{i=1}^{n} @"], ["∫", "\\int_{a}^{b} @ \\, dx"], ["lim", "\\lim_{x \\to 0} @"], ["行列", "\\begin{pmatrix} @ & \\\\ & \\end{pmatrix}"], ["場合", "\\begin{cases} @ & \\text{if } \\\\ & \\text{otherwise} \\end{cases}"], ["揃え", "\\begin{aligned} @ &= \\\\ &= \\end{aligned}"], ["文字", "\\text{@}"]]],
  ["ops", [["×", "\\times "], ["÷", "\\div "], ["±", "\\pm "], ["·", "\\cdot "], ["≠", "\\neq "], ["≤", "\\leq "], ["≥", "\\geq "], ["≈", "\\approx "], ["∞", "\\infty "], ["→", "\\to "], ["⇒", "\\Rightarrow "], ["∈", "\\in "], ["⊂", "\\subset "], ["∪", "\\cup "], ["∩", "\\cap "], ["∂", "\\partial "], ["∇", "\\nabla "], ["°", "^\\circ "]]],
  ["greek", [["α", "\\alpha "], ["β", "\\beta "], ["γ", "\\gamma "], ["δ", "\\delta "], ["ε", "\\varepsilon "], ["θ", "\\theta "], ["λ", "\\lambda "], ["μ", "\\mu "], ["π", "\\pi "], ["ρ", "\\rho "], ["σ", "\\sigma "], ["φ", "\\varphi "], ["ω", "\\omega "], ["Δ", "\\Delta "], ["Σ", "\\Sigma "], ["Ω", "\\Omega "]]],
  ["accents", [["x̄", "\\bar{@}"], ["x̂", "\\hat{@}"], ["x⃗", "\\vec{@}"], ["ẋ", "\\dot{@}"], ["太字", "\\mathbf{@}"], ["ℝ", "\\mathbb{R}"], ["色", "\\textcolor{red}{@}"], ["枠", "\\boxed{@}"]]],
];

function MathPalette({ onInsert }: { onInsert: (snippet: string) => void }) {
  const t = useT();
  const [group, setGroup] = useState(0);
  return (
    <div className="math-palette">
      <div className="segmented small">
        {MATH_GROUPS.map(([name], i) => (
          <button key={name} className={group === i ? "on" : ""} onClick={() => setGroup(i)}>
            {t(`math.group.${name}`)}
          </button>
        ))}
      </div>
      <div className="math-keys">
        {MATH_GROUPS[group][1].map(([label, snippet]) => (
          <button key={label + snippet} title={snippet.replace("@", "…")} onMouseDown={(e) => e.preventDefault()} onClick={() => onInsert(snippet)}>
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
