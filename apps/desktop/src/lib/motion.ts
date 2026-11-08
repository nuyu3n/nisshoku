// Motion: what CSS alone cannot do. The look of each animation is in
// styles/motion.css; here are the exits, the entrance of an opened note,
// the tab indicators and the hand-off from the startup screen.

import { useLayoutEffect, useRef } from "react";

/** The springs of styles/motion.css, for the Web Animations API. */
export const SPRING =
  "linear(0, 0.0036 1%, 0.0151, 0.0347 3.1%, 0.0646 4.3%, 0.1361 6.6%, 0.3123 11.5%, 0.4919 16.3%, 0.6402 20.6%, 0.7656 24.8%, 0.8614 28.8%, 0.9416 33%, 0.9993 37.2%, 1.0379 41.4%, 1.0606 45.7%, 1.0702 50.3%, 1.0694 54.6%, 1.0566 61.2%, 1.0156 74.4%, 1.0023 83.4%, 0.9988 90.4%, 1)";
export const SPRING_SOFT =
  "linear(0, 0.0047 1%, 0.0196, 0.0446 3.2%, 0.1675 6.9%, 0.3587 11.5%, 0.5393 16.3%, 0.6866 21%, 0.8035 25.8%, 0.8889 30.6%, 0.9479 35.5%, 0.9867 40.6%, 1.0082 45.8%, 1.0181 51.6%, 1.0194 57.6%, 1.0121 68.4%, 1.0029 82.4%, 1)";

const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
export const reducedMotion = () => reduced.matches;

/**
 * An opened note's page comes in: up from a little below, fading in. Only
 * transform and opacity, so it costs nothing to the layout or the editor.
 */
export function enterPage(el: HTMLElement | null) {
  if (!el || reducedMotion() || !el.animate) return;
  el.animate(
    [
      { opacity: 0, transform: "translateY(14px)" },
      { opacity: 1, transform: "none" },
    ],
    { duration: 520, easing: SPRING_SOFT },
  );
}

/**
 * Parts that leave with an animation. React removes them at once; the
 * observer puts the removed element back where it was, lets it play its
 * `.leaving` animation (styles/motion.css) and then removes it for good.
 * React never sees it again, and it takes no clicks or focus meanwhile.
 */
const LEAVING = ".overlay, .menu, .toast, .drawer-backdrop, .panel-backdrop, .task-row.ticked";

export function startExitAnimations(root: HTMLElement) {
  const observer = new MutationObserver((records) => {
    if (reducedMotion()) return;
    for (const r of records) {
      r.removedNodes.forEach((node) => {
        if (!(node instanceof HTMLElement) || !node.matches(LEAVING) || node.classList.contains("leaving")) return;
        const parent = r.target as HTMLElement;
        if (!parent.isConnected) return;
        const before = r.nextSibling && r.nextSibling.parentNode === parent ? r.nextSibling : null;
        // A toast gives its room back while it goes: the others close up
        // (its height was noted by the Toasts component).
        if (node.matches(".toast, .task-row")) node.style.setProperty("--h", `${node.dataset.h ?? 40}px`);
        node.classList.add("leaving");
        node.setAttribute("inert", "");
        node.setAttribute("aria-hidden", "true");
        parent.insertBefore(node, before);
        let done = false;
        const remove = () => {
          if (done) return;
          done = true;
          node.remove();
        };
        node.addEventListener("animationend", (e) => e.target === node && remove());
        window.setTimeout(remove, 600);
      });
    }
  });
  // (The callback only looks at removed elements: typing costs it next to nothing.)
  observer.observe(root, { childList: true, subtree: true });
  return () => observer.disconnect();
}

/** The startup screen (index.html) fades out once the first screen is ready. */
export function hideSplash() {
  const splash = document.getElementById("splash");
  if (!splash || splash.classList.contains("gone")) return;
  splash.classList.add("gone");
  const remove = () => splash.remove();
  splash.addEventListener("transitionend", remove, { once: true });
  window.setTimeout(remove, 900);
}

/**
 * A sliding indicator under the selected tab of a tab bar: puts `--ink-x`
 * and `--ink-w` on the bar for its `.ink` child (styles/motion.css), from the
 * element matching `selected`, and follows resizes. The first placement
 * does not slide (`ink-ready` comes after it).
 */
export function useInk(bar: React.RefObject<HTMLElement | null>, selected: string, key: unknown) {
  useLayoutEffect(() => {
    const el = bar.current;
    if (!el) return;
    const place = () => {
      const tab = el.querySelector<HTMLElement>(selected);
      if (!tab) return;
      el.style.setProperty("--ink-x", `${tab.offsetLeft}px`);
      el.style.setProperty("--ink-w", `${tab.offsetWidth}px`);
    };
    place();
    const ready = requestAnimationFrame(() => el.classList.add("ink-ready"));
    const resize = new ResizeObserver(place);
    resize.observe(el);
    return () => {
      cancelAnimationFrame(ready);
      resize.disconnect();
    };
  }, [bar, selected, key]);
}

/**
 * Which way a value moved (a month, a tab's position): "forward" when it went
 * up, "back" when down. For content that slides in from that side.
 */
export function useSlideDir(position: number): "forward" | "back" {
  const last = useRef(position);
  const dir = useRef<"forward" | "back">("forward");
  if (last.current !== position) {
    dir.current = position > last.current ? "forward" : "back";
    last.current = position;
  }
  return dir.current;
}
