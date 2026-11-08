// The app's tooltips, in place of the system's: every `title` in the page is
// moved to `data-tip` as soon as it appears (so the system never shows one),
// and the element under the pointer shows its tip after a moment. Moving
// from one tip to the next shows it at once. Touch screens get none.

import { useEffect, useLayoutEffect, useRef, useState } from "react";

const SHOW_AFTER = 500;
/** After a tip was shown, the next one shows at once for this long. */
const WARM_FOR = 700;

/**
 * Moves `title` to `data-tip` (giving icon-only buttons the name the title
 * gave them). Never inside the editor: it watches its DOM and would draw it
 * again, and again (its own marks use `data-tip`).
 */
function adopt(el: Element) {
  const title = el.getAttribute("title");
  if (title === null || el.closest(".ProseMirror, [contenteditable]")) return;
  el.removeAttribute("title");
  if (!title) {
    el.removeAttribute("data-tip");
    return;
  }
  el.setAttribute("data-tip", title);
  if (!el.hasAttribute("aria-label") && !el.textContent?.trim()) el.setAttribute("aria-label", title);
}

function adoptAll(root: ParentNode) {
  if (root instanceof Element) adopt(root);
  root.querySelectorAll?.("[title]").forEach(adopt);
}

export function TooltipHost() {
  const [tip, setTip] = useState<{ text: string; rect: DOMRect } | null>(null);

  useEffect(() => {
    adoptAll(document.body);
    const observer = new MutationObserver((records) => {
      for (const r of records) {
        if (r.type === "attributes") adopt(r.target as Element);
        else r.addedNodes.forEach((n) => n.nodeType === 1 && adoptAll(n as Element));
      }
    });
    observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["title"] });

    let target: Element | null = null;
    let timer: number | undefined;
    /** A tip is on screen; `shownAt`: when the last one went. */
    let showing = false;
    let shownAt = 0;
    let frame = 0;
    const clear = () => {
      window.clearTimeout(timer);
      if (showing) shownAt = Date.now();
      showing = false;
      setTip(null);
    };
    const hide = () => {
      clear();
      target = null;
    };
    // The element under the pointer, found by position: disabled buttons get
    // no pointer events but still have tips.
    const check = (x: number, y: number) => {
      const el = document.elementFromPoint(x, y)?.closest("[data-tip]") ?? null;
      if (el === target) return;
      const warm = showing || Date.now() - shownAt < WARM_FOR;
      clear();
      target = el;
      if (!el) return;
      timer = window.setTimeout(() => {
        const text = el.getAttribute("data-tip");
        if (!el.isConnected || !text || target !== el) return;
        showing = true;
        setTip({ text, rect: el.getBoundingClientRect() });
      }, warm ? 0 : SHOW_AFTER);
    };
    const move = (e: PointerEvent) => {
      if (e.pointerType === "touch") return;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => check(e.clientX, e.clientY));
    };
    const reset = () => {
      hide();
      shownAt = 0;
    };
    window.addEventListener("pointermove", move, { passive: true });
    window.addEventListener("pointerdown", reset, true);
    window.addEventListener("keydown", reset, true);
    window.addEventListener("wheel", hide, { passive: true, capture: true });
    window.addEventListener("blur", reset);
    document.documentElement.addEventListener("pointerleave", hide);
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
      window.clearTimeout(timer);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerdown", reset, true);
      window.removeEventListener("keydown", reset, true);
      window.removeEventListener("wheel", hide, { capture: true });
      window.removeEventListener("blur", reset);
      document.documentElement.removeEventListener("pointerleave", hide);
    };
  }, []);

  return tip ? <Tip text={tip.text} rect={tip.rect} /> : null;
}

/** Below the element (above it near the bottom), kept on screen. */
function Tip({ text, rect }: { text: string; rect: DOMRect }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    let top = rect.bottom + 6;
    if (top + h > window.innerHeight - 6) top = rect.top - h - 6;
    const left = Math.min(window.innerWidth - w - 6, Math.max(6, rect.left + rect.width / 2 - w / 2));
    setPos({ left, top: Math.max(6, top) });
  }, [text, rect]);
  return (
    <div ref={ref} className="tooltip" role="tooltip" style={pos ?? { left: -9999, top: -9999 }}>
      {text}
    </div>
  );
}
