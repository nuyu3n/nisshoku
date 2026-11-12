// Phone-only parts that make the app feel like a phone app: the tab bar at
// the bottom, the floating "new" button, rows that swipe for their actions,
// and pull to sync.

import { useRef, useState, type ReactNode } from "react";
import { motion, useMotionValue, useTransform, animate } from "motion/react";
import { useDrag } from "@use-gesture/react";
import { api, type NoteSummary } from "../api";
import { report, useStore } from "../store";
import { useT, t } from "../i18n";
import { Icon } from "./Icon";
import { haptic } from "../lib/haptics";
import { togglePin, trashNote } from "../lib/actions";
import { FOCUS_TASK_INPUT } from "./Tasks";

type Tab = "notes" | "diary" | "todo" | "search" | "settings";

function useActiveTab(): Tab {
  const scope = useStore((s) => s.scope);
  const settings = useStore((s) => s.settingsTab);
  if (settings) return "settings";
  if (scope.kind === "diary") return "diary";
  if (scope.kind === "todo") return "todo";
  if (scope.kind === "search") return "search";
  return "notes";
}

const TABS: [Tab, string, "tab.notes" | "tab.diary" | "tab.todo" | "tab.search" | "tab.settings"][] = [
  ["notes", "notes", "tab.notes"],
  ["diary", "diary", "tab.diary"],
  ["todo", "todo", "tab.todo"],
  ["search", "search", "tab.search"],
  ["settings", "settings", "tab.settings"],
];

/** The tab bar of the phone's list and navigation screens. */
export function TabBar() {
  useT();
  const active = useActiveTab();
  const openTasks = useStore((s) => s.openTasks);
  const go = (tab: Tab) => {
    haptic();
    const s = useStore.getState();
    if (tab !== "settings" && s.settingsTab) s.set({ settingsTab: null });
    switch (tab) {
      case "notes":
        // Again on the notes tab: up to the notebooks.
        if (active === "notes") s.set({ mobilePane: s.mobilePane === "list" ? "nav" : "list" });
        else void s.setScope(s.notebookScope.kind === "diary" || s.notebookScope.kind === "todo" ? { kind: "all" } : s.notebookScope);
        break;
      case "diary":
        void s.setScope({ kind: "diary" });
        break;
      case "todo":
        void s.setScope({ kind: "todo" });
        break;
      case "search":
        void s.setScope({ kind: "search", query: "" });
        break;
      case "settings":
        s.set({ settingsTab: "general" });
        break;
    }
  };
  return (
    <nav className="tab-bar" aria-label={t("nav.tabs")}>
      {TABS.map(([id, icon, label]) => (
        <button key={id} className={`tab-item${active === id ? " on" : ""}`} aria-current={active === id ? "page" : undefined} onClick={() => go(id)}>
          {active === id && <motion.span layoutId="tab-ink" className="tab-ink" transition={{ type: "spring", damping: 30, stiffness: 420 }} />}
          <motion.span className="tab-icon" animate={{ scale: active === id ? 1.08 : 1, y: active === id ? -1 : 0 }} transition={{ type: "spring", damping: 18, stiffness: 400 }}>
            <Icon name={icon} />
            {id === "todo" && openTasks > 0 && <span className="tab-badge">{openTasks > 99 ? "99+" : openTasks}</span>}
          </motion.span>
          <span className="tab-label">{t(label)}</span>
        </button>
      ))}
    </nav>
  );
}

/** The floating button for something new in the list shown. */
export function Fab() {
  useT();
  const scope = useStore((s) => s.scope);
  // (The diary has its own big "write today" button.)
  if (scope.kind === "trash" || scope.kind === "conflicts" || scope.kind === "search" || scope.kind === "diary") return null;
  const label = scope.kind === "todo" ? t("tasks.add") : t("list.new");
  return (
    <motion.button
      className="fab"
      aria-label={label}
      initial={{ scale: 0.4, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      whileTap={{ scale: 0.88 }}
      transition={{ type: "spring", damping: 16, stiffness: 380 }}
      onClick={() => {
        haptic("medium");
        const s = useStore.getState();
        if (scope.kind === "todo") window.dispatchEvent(new Event(FOCUS_TASK_INPUT));
        else void s.newNote().catch(report);
      }}
    >
      <Icon name="plus" />
    </motion.button>
  );
}

const SWIPE = 88;

/** A list row that swipes: to the right to pin, to the left to the trash. */
export function SwipeRow({ note, children }: { note: NoteSummary; children: ReactNode }) {
  useT();
  const x = useMotionValue(0);
  const pinOpacity = useTransform(x, [12, SWIPE], [0, 1]);
  const trashOpacity = useTransform(x, [-SWIPE, -12], [1, 0]);
  const pinScale = useTransform(x, [SWIPE * 0.6, SWIPE], [0.7, 1]);
  const trashScale = useTransform(x, [-SWIPE, -SWIPE * 0.6], [1, 0.7]);
  const armed = useRef(0);
  return (
    <div className="swipe-row">
      <motion.div className="swipe-bg pin" style={{ opacity: pinOpacity }}>
        <motion.span style={{ scale: pinScale }} className="swipe-action">
          <Icon name="pin" />
          {note.pinned ? t("menu.unpin") : t("menu.pin")}
        </motion.span>
      </motion.div>
      <motion.div className="swipe-bg trash" style={{ opacity: trashOpacity }}>
        <motion.span style={{ scale: trashScale }} className="swipe-action">
          {t("menu.trash")}
          <Icon name="trash" />
        </motion.span>
      </motion.div>
      <motion.div
        className="swipe-front"
        drag="x"
        dragDirectionLock
        dragConstraints={{ left: 0, right: 0 }}
        dragElastic={0.55}
        style={{ x }}
        onDrag={(_, info) => {
          // A buzz when letting go would do the action.
          const side = info.offset.x > SWIPE ? 1 : info.offset.x < -SWIPE ? -1 : 0;
          if (side !== armed.current) {
            if (side !== 0) haptic("medium");
            armed.current = side;
          }
        }}
        onDragEnd={() => {
          const side = armed.current;
          armed.current = 0;
          if (side === 1) void togglePin({ id: note.id, pinned: note.pinned });
          else if (side === -1) {
            // Out to the left, then gone.
            void animate(x, -window.innerWidth, { duration: 0.18 }).then(() => void trashNote(note.id));
          }
        }}
      >
        {children}
      </motion.div>
    </div>
  );
}

/** Pulling the list down from its top syncs. */
export function PullToSync({ children }: { children: ReactNode }) {
  useT();
  const ref = useRef<HTMLDivElement>(null);
  const [pull, setPull] = useState(0);
  const [busy, setBusy] = useState(false);
  const READY = 72;
  useDrag(
    ({ first, movement: [, my], last, memo }) => {
      const scroller = ref.current?.querySelector<HTMLElement>(".list-scroll");
      const atTop = first ? (scroller?.scrollTop ?? 0) <= 0 : (memo as boolean | undefined);
      if (!atTop || busy) return atTop;
      const d = Math.max(0, my) * 0.5;
      if (last) {
        if (d >= READY) {
          haptic("medium");
          setBusy(true);
          setPull(READY);
          const status = useStore.getState().syncStatus;
          const run = status && status.state !== "off" ? api.syncNow() : Promise.resolve(null);
          void run
            .catch(report)
            .then(() => useStore.getState().refresh())
            .finally(() => {
              setBusy(false);
              setPull(0);
            });
        } else setPull(0);
      } else {
        if (d >= READY && pull < READY) haptic();
        setPull(Math.min(d, READY * 1.4));
      }
      return atTop;
    },
    { target: ref, axis: "y", filterTaps: true, pointer: { touch: true } },
  );
  const ready = pull >= READY;
  return (
    <div ref={ref} className="pull-sync" style={{ ["--pull" as string]: `${pull}px` }}>
      <div className={`pull-indicator${ready ? " ready" : ""}${busy ? " busy" : ""}`} style={{ opacity: Math.min(1, pull / 40) }}>
        <Icon name="sync" className={busy ? "spin" : undefined} />
        <span>{busy ? t("sync.syncing") : ready ? t("pull.release") : t("pull.pull")}</span>
      </div>
      <div className="pull-content" style={{ transform: pull ? `translateY(${pull}px)` : undefined, transition: pull === 0 ? "transform 0.3s var(--ease)" : "none" }}>
        {children}
      </div>
    </div>
  );
}
