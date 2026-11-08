import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauri, type SyncStatus } from "./api";
import { report, useStore } from "./store";
import { Sidebar } from "./components/Sidebar";
import { NoteList } from "./components/NoteList";
import { EditorPane } from "./components/EditorPane";
import { CommandPalette } from "./components/CommandPalette";
import { SettingsDialog } from "./components/Settings";
import { WorkInfoDialog } from "./components/Work";
import { startWorkSync } from "./lib/works";
import { cancelDialog, DialogHost } from "./components/Dialogs";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { AttachmentViewer } from "./components/Viewer";
import { closeMenu, MenuHost, openMenu } from "./components/ContextMenu";
import { flush, inputMenu } from "./lib/actions";
import { loadResources, refreshImages } from "./editor/images";
import { layoutFor } from "./store";
import { isoDate } from "./api";
import { isMobile, useBackButton, useSystemInsets } from "./lib/mobile";
import { TooltipHost } from "./components/Tooltip";
import { startUpdateChecks } from "./lib/updates";
import { startAndroidBridge } from "./lib/android";
import { goBack, goForward } from "./lib/nav";
import { hideSplash, startExitAnimations } from "./lib/motion";

/** Zoom of the whole interface (the webview's zoom, so that layout stays exact). */
function useZoom() {
  const zoom = useStore((s) => s.prefs.zoom);
  useEffect(() => {
    if (!isTauri) return;
    void import("@tauri-apps/api/webview").then(({ getCurrentWebview }) => getCurrentWebview().setZoom(Math.min(2, Math.max(0.5, zoom / 100))).catch(() => {}));
  }, [zoom]);
}

/** Follows the window width; entering a narrower layout hides what would not fit. */
function useLayout() {
  useEffect(() => {
    const update = () => {
      const s = useStore.getState();
      const layout = layoutFor(window.innerWidth);
      if (layout === s.layout) return;
      if (layout === "medium") s.set({ layout, sidebarOpen: false });
      else s.set({ layout, mobilePane: s.selectedId ? "editor" : "list", panel: "none" });
    };
    // `resize` can come before the new width is settled (seen as a layout one
    // step behind); the breakpoints' media queries and the next frame catch up.
    let frame = 0;
    const later = () => {
      update();
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    };
    const queries = ["(min-width: 720px)"].map((q) => window.matchMedia(q));
    update();
    window.addEventListener("resize", later);
    queries.forEach((q) => q.addEventListener("change", later));
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", later);
      queries.forEach((q) => q.removeEventListener("change", later));
    };
  }, []);
}

/** No webview context menu anywhere; text fields get cut / copy / paste. */
function useContextMenus() {
  useEffect(() => {
    const onMenu = (e: MouseEvent) => {
      e.preventDefault();
      const el = e.target as HTMLElement;
      if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) openMenu(e, inputMenu(el));
    };
    window.addEventListener("contextmenu", onMenu);
    return () => window.removeEventListener("contextmenu", onMenu);
  }, []);
}

/** The app's name in the window title and the taskbar follows the language. */
function useAppTitle() {
  const language = useStore((s) => s.prefs.language);
  useEffect(() => {
    const name = language === "en" ? "Nisshoku" : "日食";
    document.title = name;
    if (!isTauri || isMobile) return;
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().setTitle(name).catch(() => {}));
  }, [language]);
}

function useTheme() {
  useAppTitle();
  const theme = useStore((s) => s.prefs.theme);
  const accent = useStore((s) => s.prefs.accent);
  useEffect(() => {
    if (accent === "orange") delete document.documentElement.dataset.accent;
    else document.documentElement.dataset.accent = accent;
  }, [accent]);
  useEffect(() => {
    const root = document.documentElement;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      if (theme === "system") delete root.dataset.theme;
      else root.dataset.theme = theme;
      root.dataset.resolvedTheme = theme === "system" ? (media.matches ? "dark" : "light") : theme;
      // For the startup screen of the next launch (index.html), before the
      // preferences are read.
      try {
        localStorage.setItem("nisshoku.theme", theme);
      } catch {
        // Storage off: the startup screen follows the system.
      }
      // Android: the icons of the status and navigation bars follow.
      window.NisshokuAndroid?.setDarkTheme(root.dataset.resolvedTheme === "dark");
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}

/**
 * Keys the webview would act on as a browser: reload (which loses what is
 * open), view source, open or save a page, downloads, history, its find bar,
 * caret browsing. The app's own uses of some (the editor's Ctrl+F, H, S, G)
 * run before this.
 */
function browserKey(e: KeyboardEvent): boolean {
  if (["F3", "F5", "F7"].includes(e.key)) return true;
  if (!(e.ctrlKey || e.metaKey) || e.altKey) return false;
  return ["r", "u", "o", "j", "s", "f", "g", "h", "e", "l"].includes(e.key.toLowerCase());
}

function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState();
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (e.key === "F11" || (mod && e.shiftKey && key === "enter")) {
        e.preventDefault();
        s.set({ focusMode: !s.focusMode });
      } else if (e.key === "Escape" && s.focusMode && !s.paletteOpen && !s.settingsTab) {
        s.set({ focusMode: false });
      } else if (mod && key === "k") {
        e.preventDefault();
        s.set({ paletteOpen: !s.paletteOpen });
      } else if (mod && key === "p" && !e.shiftKey) {
        e.preventDefault();
        s.set({ paletteOpen: true });
      } else if (mod && key === "n") {
        e.preventDefault();
        void s.newNote().catch(report);
      } else if (mod && key === "d") {
        e.preventDefault();
        void s.openDiary().catch(report);
      } else if (mod && e.shiftKey && key === "f") {
        e.preventDefault();
        void s.setScope({ kind: "search", query: "" });
      } else if (mod && (e.key === "\\" || e.code === "IntlRo" || e.code === "Backslash")) {
        e.preventDefault();
        if (e.shiftKey) s.set({ listOpen: !s.listOpen });
        else s.set({ sidebarOpen: !s.sidebarOpen });
      } else if (mod && e.key === ",") {
        e.preventDefault();
        s.set({ settingsTab: s.settingsTab ? null : "general" });
      } else if (e.altKey && !mod && !e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault();
        void (e.key === "ArrowLeft" ? goBack() : goForward());
      } else if (browserKey(e)) {
        // The webview's own: reload, view source, save or open a page, caret browsing…
        e.preventDefault();
      }
    };
    // The back and forward buttons of a mouse.
    const onMouse = (e: MouseEvent) => {
      if (e.button !== 3 && e.button !== 4) return;
      e.preventDefault();
      if (e.type === "mouseup") void (e.button === 3 ? goBack() : goForward());
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousedown", onMouse);
    window.addEventListener("mouseup", onMouse);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onMouse);
      window.removeEventListener("mouseup", onMouse);
    };
  }, []);
}

function Toasts() {
  const toasts = useStore((s) => s.toasts);
  return (
    <div className="toasts">
      {toasts.map((t) => (
        <div key={t.id} className="toast" ref={(el) => void (el && (el.dataset.h = String(el.offsetHeight)))}>
          <span>{t.message}</span>
          {t.action && (
            <button
              onClick={() => {
                t.action!.run();
                useStore.setState((s) => ({ toasts: s.toasts.filter((x) => x.id !== t.id) }));
              }}
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

/** In a browser preview, `?view=diary&panel=preview&theme=dark&settings=sync&palette=1&focus=1` sets up a screen. */
async function previewState() {
  const q = new URLSearchParams(window.location.search);
  const s = useStore.getState();
  if (q.get("theme")) s.setPrefs({ theme: q.get("theme") as "dark" });
  if (q.get("lang")) s.setPrefs({ language: q.get("lang") as "ja" });
  // `?pref.lineNumbers=1&pref.fontSize=19`: any preference.
  for (const [k, v] of q) {
    if (!k.startsWith("pref.")) continue;
    const value = v === "1" || v === "true" ? true : v === "0" || v === "false" ? false : Number.isFinite(Number(v)) ? Number(v) : v;
    s.setPrefs({ [k.slice(5)]: value } as Partial<typeof s.prefs>);
  }
  if (q.get("view") === "diary") {
    await s.setScope({ kind: "diary" });
    s.select(useStore.getState().notes[0]?.id ?? null);
  }
  if (q.get("view") === "today") await s.openDiary();
  if (q.get("view") === "todo" || q.get("view") === "trash") await s.setScope({ kind: q.get("view") as "todo" });
  if (q.get("panel")) s.set({ panel: q.get("panel") as "preview" });
  if (q.get("settings")) s.set({ settingsTab: q.get("settings") });
  if (q.get("palette")) s.set({ paletteOpen: true });
  if (q.get("focus")) s.set({ focusMode: true });
  if (q.get("sidebar") === "open") s.set({ sidebarOpen: true });
  // `?folder=第一部&worktab=characters`: a notebook (by title), and the tab of its work.
  const folderTitle = q.get("folder");
  if (folderTitle) {
    const f = useStore.getState().folders.find((x) => x.title.includes(folderTitle));
    if (f) await s.setScope({ kind: "folder", id: f.id });
  }
  if (q.get("worktab")) {
    const { useWork } = await import("./lib/works");
    useWork.setState({ tab: q.get("worktab") as "plot" });
  }
  const title = q.get("note");
  if (title) {
    const n = useStore.getState().notes.find((x) => x.title.includes(title));
    if (n) s.select(n.id);
  }
  if (q.get("pane")) s.set({ mobilePane: q.get("pane") as "nav" });
  if (q.get("drawer")) s.set({ sidebarOpen: true });
  await new Promise((r) => setTimeout(r, 400));
  const demo = q.get("demo");
  const { noteMenu, folderMenu } = await import("./lib/actions");
  const state = useStore.getState();
  if (demo === "noteMenu") openMenu({ clientX: 420, clientY: 160 }, noteMenu(state.notes[0]));
  if (demo === "folderMenu") {
    const items = folderMenu(state.folders[0]);
    openMenu({ clientX: 120, clientY: 240 }, items);
  }
  if (demo === "editorMenu" || demo === "table") {
    const el = document.querySelector<HTMLElement>(".ProseMirror");
    const editor = (el as unknown as { editor?: import("@tiptap/core").Editor })?.editor;
    if (editor) {
      if (demo === "table") {
        let pos = 0;
        editor.state.doc.descendants((n, p) => {
          if (!pos && n.type.name === "tableCell") pos = p + 2;
        });
        editor.commands.setTextSelection(pos);
        editor.view.dom.dispatchEvent(new Event("focus"));
      } else {
        editor.commands.setTextSelection({ from: 3, to: 8 });
        const { editorMenu } = await import("./editor/menus");
        openMenu({ clientX: 700, clientY: 220 }, editorMenu(editor, () => {}));
      }
    }
  }
}

const PANE_ORDER = { nav: 0, list: 1, editor: 2 } as const;

/**
 * Phones: whether the screen shown came forward (deeper: nav → list →
 * editor) or back. The new screen slides in from that side (styles/motion.css).
 */
function usePaneDirection(pane: keyof typeof PANE_ORDER): "forward" | "back" {
  const last = useRef(pane);
  const dir = useRef<"forward" | "back">("forward");
  if (last.current !== pane) {
    dir.current = PANE_ORDER[pane] >= PANE_ORDER[last.current] ? "forward" : "back";
    last.current = pane;
  }
  return dir.current;
}

export function App() {
  const focusMode = useStore((s) => s.focusMode);
  const sidebarWidth = useStore((s) => s.prefs.sidebarWidth);
  const listWidth = useStore((s) => s.prefs.listWidth);
  const language = useStore((s) => s.prefs.language);
  const layout = useStore((s) => s.layout);
  const mobilePane = useStore((s) => s.mobilePane);
  const sidebarOpen = useStore((s) => s.sidebarOpen);
  const sidebarTab = useStore((s) => s.sidebarTab);
  const scope = useStore((s) => s.scope);
  const selectedId = useStore((s) => s.selectedId);
  useTheme();
  useShortcuts();
  useZoom();
  useLayout();
  useContextMenus();
  useBackButton();
  useSystemInsets();
  const navDir = usePaneDirection(mobilePane);
  const appRef = useRef<HTMLDivElement>(null);
  useEffect(() => (appRef.current ? startExitAnimations(appRef.current) : undefined), []);

  useEffect(() => {
    (async () => {
      const s = useStore.getState();
      await s.loadPrefs().catch(report);
      await s.refresh().catch(report);
      // What opens first.
      const { prefs, notes } = useStore.getState();
      if (prefs.startup === "diary") await s.openDiary(isoDate()).catch(report);
      else if (prefs.startup === "last" && prefs.lastNoteId && (await api.getNote(prefs.lastNoteId).then(() => true, () => false))) {
        await s.openNote(prefs.lastNoteId);
      } else if (prefs.startup !== "none" && notes.length > 0) s.select(notes[0].id);
      // Phones show one screen: the note opened above (select() went to the editor), or else the chosen one.
      if (useStore.getState().layout === "narrow") {
        if (prefs.startup === "list") s.set({ mobilePane: "list" });
        else if (prefs.startup === "none") s.set({ mobilePane: "nav" });
        else if (!useStore.getState().selectedId) s.set({ mobilePane: "list" });
      }
      // The first screen is set: the startup screen gives way to it.
      requestAnimationFrame(() => hideSplash());
      const sync = await api.getSyncSettings().catch(report);
      if (sync) s.set({ syncStatus: sync.status });
      startAndroidBridge();
      startUpdateChecks();
      startWorkSync();
      if (!isTauri) await previewState();
    })();

    if (!isTauri) return;
    // Closing the window waits for the open note to be saved (the last
    // keystrokes are still in the autosave delay).
    const closing = isMobile
      ? Promise.resolve(() => {})
      : import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
          getCurrentWindow().onCloseRequested(async () => {
            await Promise.race([flush().catch(() => {}), new Promise((r) => setTimeout(r, 5000))]);
          }),
        );
    const unlisteners = [
      closing,
      listen<SyncStatus>("sync-status", (e) => {
        const prev = useStore.getState().syncStatus;
        useStore.getState().set({ syncStatus: e.payload });
        if (e.payload.state === "error" && prev?.state !== "error" && e.payload.message) {
          useStore.getState().toast(e.payload.message);
        }
      }),
      listen("data-changed", async () => {
        const s = useStore.getState();
        await s.refresh().catch(report);
        await loadResources();
        refreshImages();
        // The open note may have been changed by another device.
        s.set({ remoteVersion: s.remoteVersion + 1 });
      }),
    ];
    return () => {
      unlisteners.forEach((u) => void u.then((f) => f()));
    };
  }, []);

  return (
    <div
      ref={appRef}
      data-nav={navDir}
      className={`app layout-${layout} pane-${mobilePane}${isMobile ? " mobile" : ""}${focusMode ? " focus" : ""}${layout === "medium" && sidebarOpen ? " drawer-open" : ""}`}
      lang={language}
      style={{ ["--sidebar-w" as string]: `${sidebarWidth}px`, ["--list-w" as string]: `${listWidth}px` }}
    >
      {/* Each part fails on its own: the others stay usable. */}
      <ErrorBoundary kind="pane" className="sidebar" resetKey={sidebarTab}>
        <Sidebar />
      </ErrorBoundary>
      {layout === "medium" && sidebarOpen && <div className="drawer-backdrop" onClick={() => useStore.getState().set({ sidebarOpen: false })} />}
      <ErrorBoundary kind="pane" className="note-list" resetKey={JSON.stringify(scope)}>
        <NoteList />
      </ErrorBoundary>
      <ErrorBoundary kind="pane" className="main" resetKey={selectedId}>
        <EditorPane />
      </ErrorBoundary>
      <ErrorBoundary kind="overlay" onClose={() => useStore.getState().set({ paletteOpen: false })}>
        <CommandPalette />
      </ErrorBoundary>
      <ErrorBoundary kind="overlay" onClose={() => useStore.getState().set({ settingsTab: null })}>
        <SettingsDialog />
        <WorkInfoDialog />
      </ErrorBoundary>
      <ErrorBoundary kind="overlay" onClose={() => void cancelDialog()}>
        <DialogHost />
      </ErrorBoundary>
      <ErrorBoundary kind="overlay" onClose={closeMenu}>
        <MenuHost />
      </ErrorBoundary>
      <TooltipHost />
      <ErrorBoundary kind="overlay" onClose={() => useStore.getState().set({ preview: null })}>
        <AttachmentViewer />
      </ErrorBoundary>
      <ErrorBoundary kind="overlay" onClose={() => useStore.getState().set({ toasts: [] })}>
        <Toasts />
      </ErrorBoundary>
    </div>
  );
}
