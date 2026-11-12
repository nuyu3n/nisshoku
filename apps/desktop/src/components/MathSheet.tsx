// Math on phones: a full-screen sheet, made for fingers. The formula as it
// will look at the top, the TeX under it, and a tray of keys (drawn as the
// symbols they insert) in place of the system keyboard while it is in use.

import { useEffect, useMemo, useRef, useState } from "react";
import { motion } from "motion/react";
import katex from "katex";
import { useT } from "../i18n";
import { Icon } from "./Icon";
import { haptic } from "../lib/haptics";

/** Snippets: `@` is where the selection (or the cursor) goes. */
export type MathGroupId = "basic" | "structure" | "ops" | "greek" | "accents";
export const MATH_GROUPS: [MathGroupId, string, string[]][] = [
  ["basic", "よく使う", ["\\frac{@}{}", "\\sqrt{@}", "^{@}", "_{@}", "= ", "\\neq ", "\\pm ", "\\times ", "\\cdot ", "\\pi ", "\\infty ", "\\sum_{i=1}^{n} @", "\\int_{a}^{b} @ \\, dx", "\\to "]],
  ["structure", "構造", ["\\frac{@}{}", "\\sqrt{@}", "\\sqrt[n]{@}", "^{@}", "_{@}", "\\left( @ \\right)", "\\left[ @ \\right]", "\\left\\{ @ \\right\\}", "\\left| @ \\right|", "\\sum_{i=1}^{n} @", "\\prod_{i=1}^{n} @", "\\int_{a}^{b} @ \\, dx", "\\lim_{x \\to 0} @", "\\begin{pmatrix} @ & \\\\ & \\end{pmatrix}", "\\begin{cases} @ & \\text{if } \\\\ & \\text{otherwise} \\end{cases}", "\\begin{aligned} @ &= \\\\ &= \\end{aligned}", "\\text{@}"]],
  ["ops", "演算・関係", ["\\times ", "\\div ", "\\pm ", "\\mp ", "\\cdot ", "\\neq ", "\\leq ", "\\geq ", "\\approx ", "\\equiv ", "\\propto ", "\\to ", "\\Rightarrow ", "\\Leftrightarrow ", "\\in ", "\\notin ", "\\subset ", "\\cup ", "\\cap ", "\\forall ", "\\exists ", "\\partial ", "\\nabla ", "^\\circ "]],
  ["greek", "ギリシャ文字", ["\\alpha ", "\\beta ", "\\gamma ", "\\delta ", "\\varepsilon ", "\\zeta ", "\\eta ", "\\theta ", "\\lambda ", "\\mu ", "\\pi ", "\\rho ", "\\sigma ", "\\tau ", "\\varphi ", "\\omega ", "\\Gamma ", "\\Delta ", "\\Theta ", "\\Lambda ", "\\Sigma ", "\\Phi ", "\\Omega "]],
  ["accents", "装飾", ["\\bar{@}", "\\hat{@}", "\\vec{@}", "\\dot{@}", "\\tilde{@}", "\\overline{@}", "\\mathbf{@}", "\\mathrm{@}", "\\mathbb{R}", "\\mathbb{N}", "\\mathbb{Z}", "\\mathcal{@}", "\\boxed{@}", "\\textcolor{red}{@}"]],
];

/** A key's face: the snippet rendered, empty places shown as boxes. */
function face(snippet: string): string {
  // A superscript or subscript shows on an x.
  const base = /^[_^]/.test(snippet) ? `x${snippet}` : snippet;
  const tex = base.replace("@", "\\square").replace(/\{\}/g, "{\\square}").replace(/\\text\{\\square\}/, "\\text{abc}").trim() || "\\square";
  try {
    return katex.renderToString(tex, { throwOnError: false, displayMode: false });
  } catch {
    return "";
  }
}

function Preview({ latex, display }: { latex: string; display: boolean }) {
  const t = useT();
  const { html, error } = useMemo(() => {
    try {
      return { html: katex.renderToString(latex || "\\;", { displayMode: display, throwOnError: true }), error: null as string | null };
    } catch (e) {
      return {
        html: katex.renderToString(latex || "\\;", { displayMode: display, throwOnError: false }),
        error: e instanceof Error ? e.message.replace(/^KaTeX parse error: /, "") : String(e),
      };
    }
  }, [latex, display]);
  return (
    <div className="ms-preview">
      <motion.div key={html.length > 2000 ? "big" : "f"} className="ms-formula" dangerouslySetInnerHTML={{ __html: html }} />
      {!latex.trim() && <div className="ms-empty">{t("math.placeholder")}</div>}
      {error && <div className="ms-error">{error}</div>}
    </div>
  );
}

export function MathSheet({ title, initial, display, canDelete, onClose }: { title: string; initial: string; display: boolean; canDelete: boolean; onClose: (result: string | null) => void }) {
  const t = useT();
  const [value, setValue] = useState(initial);
  const [group, setGroup] = useState(0);
  // The tray instead of the system keyboard (the text field takes no keyboard).
  const [tray, setTray] = useState(true);
  const [closing, setClosing] = useState<string | null | undefined>(undefined);
  const input = useRef<HTMLTextAreaElement>(null);
  const faces = useMemo(() => MATH_GROUPS.map(([, , keys]) => keys.map(face)), []);

  useEffect(() => {
    const el = input.current;
    if (!el) return;
    el.focus({ preventScroll: true });
    el.setSelectionRange(el.value.length, el.value.length);
  }, []);

  const close = (result: string | null) => {
    if (closing !== undefined) return;
    setClosing(result);
  };

  /** Replaces the selection with `text`, the cursor at `caret` within it. */
  const insert = (snippet: string) => {
    haptic();
    const el = input.current;
    const start = el?.selectionStart ?? value.length;
    const end = el?.selectionEnd ?? value.length;
    const selected = value.slice(start, end);
    const [before, after = ""] = snippet.split("@");
    const next = value.slice(0, start) + before + selected + after + value.slice(end);
    setValue(next);
    requestAnimationFrame(() => {
      if (!el) return;
      el.focus({ preventScroll: true });
      const caret = start + before.length + selected.length;
      el.setSelectionRange(caret, caret);
    });
  };

  const backspace = () => {
    haptic();
    const el = input.current;
    if (!el) return;
    const { selectionStart: s, selectionEnd: e } = el;
    if (s === e && s === 0) return;
    const from = s === e ? s - 1 : s;
    setValue(value.slice(0, from) + value.slice(e));
    requestAnimationFrame(() => {
      el.focus({ preventScroll: true });
      el.setSelectionRange(from, from);
    });
  };

  const move = (d: -1 | 1) => {
    haptic();
    const el = input.current;
    if (!el) return;
    const p = Math.max(0, Math.min(value.length, (d < 0 ? el.selectionStart : el.selectionEnd) + d));
    el.focus({ preventScroll: true });
    el.setSelectionRange(p, p);
  };

  return (
    <div className="ms-root">
      <motion.div
        className="ms-backdrop"
        initial={{ opacity: 0 }}
        animate={{ opacity: closing === undefined ? 1 : 0 }}
        transition={{ duration: 0.2 }}
        onClick={() => close(null)}
      />
      <motion.div
        className="ms-sheet"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        initial={{ y: "100%" }}
        animate={{ y: closing === undefined ? 0 : "100%" }}
        transition={{ type: "spring", damping: 32, stiffness: 340, mass: 0.9 }}
        onAnimationComplete={() => {
          if (closing !== undefined) onClose(closing);
        }}
      >
        <header className="ms-head">
          <button className="ms-head-btn" onClick={() => close(null)}>
            {t("dialog.cancel")}
          </button>
          <span className="ms-title">{title}</span>
          <button className="ms-head-btn primary" onClick={() => close(value.trim())}>
            {t("dialog.ok")}
          </button>
        </header>

        <Preview latex={value} display={display} />

        <div className="ms-input">
          <textarea
            ref={input}
            value={value}
            inputMode={tray ? "none" : "text"}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            placeholder="\frac{a}{b}"
            onChange={(e) => setValue(e.target.value)}
          />
          <div className="ms-tools">
            <button
              className={`ms-tool${tray ? "" : " on"}`}
              onClick={() => {
                haptic();
                setTray(!tray);
                requestAnimationFrame(() => input.current?.focus({ preventScroll: true }));
              }}
            >
              <Icon name={tray ? "keyboard" : "keyboardHide"} />
              <span>{tray ? t("math.keyboard") : t("math.symbols")}</span>
            </button>
            <span className="grow" />
            <button className="ms-tool icon" aria-label={t("math.left")} onClick={() => move(-1)}>
              <Icon name="chevronLeft" />
            </button>
            <button className="ms-tool icon" aria-label={t("math.right")} onClick={() => move(1)}>
              <Icon name="chevronRight" />
            </button>
            <button className="ms-tool icon" aria-label={t("math.backspace")} onClick={backspace}>
              <Icon name="backspace" />
            </button>
          </div>
        </div>

        {tray && (
          <motion.div className="ms-tray" initial={{ opacity: 0, y: 24 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.22 }}>
            <div className="ms-groups" role="tablist">
              {MATH_GROUPS.map(([id], i) => (
                <button
                  key={id}
                  role="tab"
                  aria-selected={group === i}
                  className={`ms-group${group === i ? " on" : ""}`}
                  onClick={() => {
                    haptic();
                    setGroup(i);
                  }}
                >
                  {group === i && <motion.span layoutId="ms-group-ink" className="ms-group-ink" transition={{ type: "spring", damping: 30, stiffness: 400 }} />}
                  <span className="ms-group-label">{t(`math.group.${id}`)}</span>
                </button>
              ))}
            </div>
            <motion.div key={group} className="ms-keys" initial={{ opacity: 0, x: 16 }} animate={{ opacity: 1, x: 0 }} transition={{ duration: 0.18 }}>
              {MATH_GROUPS[group][2].map((snippet, i) => (
                <motion.button
                  key={snippet + i}
                  className="ms-key"
                  whileTap={{ scale: 0.88 }}
                  transition={{ type: "spring", damping: 20, stiffness: 500 }}
                  onPointerDown={(e) => e.preventDefault()}
                  onClick={() => insert(snippet)}
                  title={snippet.replace("@", "…")}
                  dangerouslySetInnerHTML={{ __html: faces[group][i] }}
                />
              ))}
            </motion.div>
          </motion.div>
        )}

        {canDelete && (
          <button className="ms-delete" onClick={() => close("")}>
            <Icon name="trash" />
            {t("math.delete")}
          </button>
        )}
      </motion.div>
    </div>
  );
}
