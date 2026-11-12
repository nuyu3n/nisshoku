// Menus of the app: context menus (right click, or long press on touch
// screens) and the menus of "…" buttons, with keyboard navigation and one
// level of submenus. The webview's own context menu is never shown.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { create } from "zustand";
import { Icon } from "./Icon";
import { isMobile } from "../lib/mobile";
import { useDrag } from "@use-gesture/react";
import { haptic } from "../lib/haptics";

export type MenuItem =
  | "sep"
  | { header: string }
  | {
      label: string;
      icon?: string;
      /** Shortcut shown at the right. */
      hint?: string;
      danger?: boolean;
      disabled?: boolean;
      checked?: boolean;
      /** A colour shown in place of the icon (a colour choice). */
      swatch?: string;
      run?: () => void | Promise<void>;
      submenu?: MenuItem[];
    };

interface MenuState {
  items: MenuItem[] | null;
  x: number;
  y: number;
  /** Align the menu's right edge with `x` (for "…" buttons at the right). */
  alignRight: boolean;
  /** At least this wide (a select's menu: as wide as the select). */
  minWidth: number;
  /** Where the menu's bottom goes when it has no room below `y`. */
  above: number;
  /** Where the menu's right edge goes when it has no room right of `x`. */
  before: number;
}

const useMenu = create<MenuState>(() => ({ items: null, x: 0, y: 0, alignRight: false, minWidth: 0, above: 0, before: 0 }));

/**
 * Opens a menu at the pointer, or below an element: its right edge under the
 * element's for "…" buttons, or (`asSelect`) its left edge, as wide as it.
 */
export function openMenu(at: { clientX: number; clientY: number } | HTMLElement, items: MenuItem[], opts: { asSelect?: boolean } = {}) {
  if (at instanceof HTMLElement) {
    const r = at.getBoundingClientRect();
    const near = { y: r.bottom + 4, above: r.top - 4 };
    if (opts.asSelect) useMenu.setState({ items, ...near, x: r.left, before: r.right, alignRight: false, minWidth: r.width });
    else useMenu.setState({ items, ...near, x: r.right, before: r.right, alignRight: true, minWidth: 0 });
  } else {
    // At the pointer; near an edge it opens the other way, so it never
    // covers the point it was opened from.
    const { clientX: x, clientY: y } = at;
    useMenu.setState({ items, x: x + 1, y: y + 1, above: y - 1, before: x - 1, alignRight: false, minWidth: 0 });
  }
}

export function closeMenu() {
  useMenu.setState({ items: null });
}

export const menuOpen = () => useMenu.getState().items !== null;
export const onMenuChange = (f: () => void) => useMenu.subscribe(f);

/** Props for an element with a context menu. */
/**
 * Whether a context menu event is a phone's long press on text being written:
 * that long press selects text (and Android shows its own copy / paste bar),
 * so no menu of the app's comes up from the bottom over it.
 */
export function isTextLongPress(e: { target: EventTarget | null }): boolean {
  if (!isMobile) return false;
  const el = e.target as HTMLElement | null;
  return !!el?.closest?.("input, textarea, [contenteditable='true'], .ProseMirror");
}

export function contextMenu(items: () => MenuItem[]) {
  return {
    onContextMenu: (e: React.MouseEvent) => {
      if (isTextLongPress(e)) return;
      e.preventDefault();
      e.stopPropagation();
      haptic("medium");
      openMenu(e, items());
    },
  };
}

type Action = Extract<MenuItem, { label: string }>;

const EDGE = 6;

/**
 * Where a menu of `w`×`h` goes: below/right of (x, y) when it fits, else on
 * the other side (bottom at `above`, right edge at `before`), else as far in
 * as the window allows.
 */
function place(w: number, h: number, p: { x: number; y: number; above: number; before: number; alignRight: boolean }) {
  const area = visibleArea();
  // Taller than the room there is: it scrolls, at the room's height.
  const maxHeight = area.bottom - area.top - 2 * EDGE;
  h = Math.min(h, maxHeight);
  let left = p.alignRight ? p.x - w : p.x;
  if (left + w > area.right - EDGE) left = p.before - w >= area.left + EDGE ? p.before - w : area.right - w - EDGE;
  let top = p.y;
  if (top + h > area.bottom - EDGE) top = p.above - h >= area.top + EDGE ? p.above - h : area.bottom - h - EDGE;
  return { left: Math.max(area.left + EDGE, Math.min(left, area.right - w - EDGE)), top: Math.max(area.top + EDGE, top), maxHeight };
}

/**
 * The part of the window that is on the screen, in CSS pixels. A window
 * partly off the monitor (or under the taskbar) would otherwise have its
 * menus open where they cannot be seen. On phones, or when the numbers look
 * wrong, the whole window.
 */
function visibleArea() {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const all = { left: 0, top: 0, right: vw, bottom: vh };
  if (/Android|iPhone|iPad/.test(navigator.userAgent)) return all;
  const scr = window.screen as Screen & { availLeft?: number; availTop?: number };
  // Screen coordinates are in device-independent pixels; the page may be zoomed.
  const zoom = window.outerWidth > 0 && vw > 0 ? window.outerWidth / vw : 1;
  if (!scr.availWidth || !scr.availHeight || !Number.isFinite(window.screenX) || zoom < 0.4 || zoom > 3.5) return all;
  // The frame above the page, if the window has one.
  const frameTop = Math.max(0, window.outerHeight - vh * zoom);
  const toCss = (dip: number) => dip / zoom;
  const area = {
    left: Math.max(0, toCss((scr.availLeft ?? 0) - window.screenX)),
    top: Math.max(0, toCss((scr.availTop ?? 0) - window.screenY - frameTop)),
    right: Math.min(vw, toCss((scr.availLeft ?? 0) + scr.availWidth - window.screenX)),
    bottom: Math.min(vh, toCss((scr.availTop ?? 0) + scr.availHeight - window.screenY - frameTop)),
  };
  return area.right - area.left >= 200 && area.bottom - area.top >= 160 ? area : all;
}

function MenuList({ items, x, y, above, before, alignRight, onClose, level, title, minWidth = 0 }: { items: MenuItem[]; x: number; y: number; above: number; before: number; alignRight: boolean; onClose: () => void; level: number; title?: string; minWidth?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  // Hidden until placed, so it never shows where it does not fit.
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight?: number; visibility?: "hidden" }>({ left: x, top: y, visibility: "hidden" });
  // The checked item (a select's value) is where the keys start.
  const [index, setIndex] = useState(() => items.findIndex((it) => typeof it === "object" && "label" in it && it.checked === true));
  const [sub, setSub] = useState<{ index: number; x: number; y: number; above: number; before: number } | null>(null);
  const actions = items.map((it, i) => (typeof it === "object" && "label" in it && !it.disabled ? i : -1)).filter((i) => i >= 0);

  // Phones: the sheet goes away when pulled down (from its top, or anywhere
  // when it is not scrolled), and springs back when let go early.
  useDrag(
    ({ movement: [, my], velocity: [, vy], direction: [, dy], last, first, memo }) => {
      const el = ref.current;
      if (!el) return memo;
      const fromTop = first ? el.scrollTop <= 0 : (memo as boolean | undefined);
      if (!fromTop) return fromTop;
      const y = Math.max(0, my);
      if (last) {
        el.style.transition = "transform 0.2s var(--ease), opacity 0.2s";
        if (y > 90 || (vy > 0.45 && dy > 0 && y > 20)) {
          el.style.transform = "translateY(110%)";
          el.style.opacity = "0";
          window.setTimeout(onClose, 180);
        } else el.style.transform = "";
      } else {
        el.style.transition = "none";
        el.style.transform = y ? `translateY(${y}px)` : "";
      }
      return fromTop;
    },
    { target: ref, axis: "y", enabled: isMobile && level === 0, filterTaps: true, pointer: { touch: true } },
  );

  // Keep the menu on screen.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setPos(place(el.offsetWidth, el.offsetHeight, { x, y, above, before, alignRight }));
  }, [x, y, above, before, alignRight, items]);

  const activate = (i: number, el?: HTMLElement) => {
    const item = items[i] as Action;
    if (!item || item.disabled) return;
    if (item.submenu) {
      const r = (el ?? ref.current?.querySelectorAll<HTMLElement>(".menu-item")[actions.indexOf(i)])?.getBoundingClientRect();
      // Beside the item: to its right, or its left when there is no room;
      // its first item level with this one, or its last when near the bottom.
      if (r) setSub({ index: i, x: r.right - 4, y: r.top - 5, above: r.bottom + 5, before: r.left + 4 });
      return;
    }
    onClose();
    void item.run?.();
  };

  useEffect(() => {
    if (level > 0) return;
    const onKey = (e: KeyboardEvent) => {
      if (sub) return;
      const k = actions.indexOf(index);
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setIndex(actions[(k + 1) % actions.length]);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setIndex(actions[(k - 1 + actions.length) % actions.length]);
      } else if (e.key === "Enter" || e.key === "ArrowRight") {
        if (index >= 0) {
          e.preventDefault();
          activate(index);
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });

  return (
    <>
      <div
        className="menu"
        ref={ref}
        // It grows from the corner it was opened at (styles/motion.css).
        style={{ left: pos.left, top: pos.top, visibility: pos.visibility, ["--menu-room" as string]: pos.maxHeight ? `${pos.maxHeight}px` : undefined, minWidth: minWidth || undefined, transformOrigin: `${pos.left < x - 1 ? "right" : "left"} ${pos.top < y - 1 ? "bottom" : "top"}` }}
        onMouseDown={(e) => e.stopPropagation()}
        onContextMenu={(e) => e.preventDefault()}
        role="menu"
      >
        {/* On phones a submenu covers its menu: it says what it is. */}
        {title && <div className="menu-label menu-sub-title">{title}</div>}
        {items.map((item, i) => {
          if (item === "sep") return <div key={i} className="menu-sep" />;
          if ("header" in item) return <div key={i} className="menu-label">{item.header}</div>;
          return (
            <button
              key={i}
              role="menuitem"
              disabled={item.disabled}
              className={`menu-item${item.danger ? " danger" : ""}${i === index || sub?.index === i ? " active" : ""}`}
              style={{ ["--i" as string]: Math.min(i, 12) }}
              onMouseEnter={(e) => {
                setIndex(i);
                if (item.submenu) activate(i, e.currentTarget);
                else setSub(null);
              }}
              onClick={(e) => activate(i, e.currentTarget)}
            >
              {/* A checkable item with an icon keeps the icon and shows the check at the end. */}
              {item.swatch ? <span className="menu-swatch" style={{ background: item.swatch }} /> : item.checked !== undefined && !item.icon ? <span className="menu-check">{item.checked ? <Icon name="check" /> : null}</span> : <Icon name={item.icon ?? "blank"} />}
              <span className="menu-text">{item.label}</span>
              {item.hint && <span className="menu-hint">{item.hint}</span>}
              {item.checked && (item.icon || item.swatch) && <Icon name="check" className="menu-arrow" />}
              {item.submenu && <Icon name="chevronRight" className="menu-arrow" />}
            </button>
          );
        })}
      </div>
      {/* Beside its menu, not inside: inside a menu that is animating (a
        transform), a fixed submenu would be placed and clipped by it. */}
      {sub && (items[sub.index] as Action).submenu && (
        <MenuList items={(items[sub.index] as Action).submenu!} x={sub.x} y={sub.y} above={sub.above} before={sub.before} alignRight={false} onClose={onClose} level={level + 1} title={(items[sub.index] as Action).label} />
      )}
    </>
  );
}

export function MenuHost() {
  const { items, x, y, above, before, alignRight, minWidth } = useMenu();
  useEffect(() => {
    if (!items) return;
    const close = () => closeMenu();
    // Scrolling inside a menu scrolls it; scrolling elsewhere closes it.
    const wheel = (e: WheelEvent) => {
      if (!(e.target as HTMLElement).closest?.(".menu")) closeMenu();
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    window.addEventListener("wheel", wheel, { passive: true });
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("wheel", wheel);
    };
  }, [items]);
  if (!items) return null;
  return <MenuList items={items} x={x} y={y} above={above} before={before} alignRight={alignRight} onClose={closeMenu} level={0} minWidth={minWidth} />;
}
