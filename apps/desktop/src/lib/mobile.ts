// Phones and tablets (the Android build): the system back button / gesture.

import { useEffect } from "react";
import { isTauri } from "../api";
import { useStore } from "../store";
import { cancelDialog, dialogOpen, onDialogChange } from "../components/Dialogs";
import { closeMenu, menuOpen, onMenuChange } from "../components/ContextMenu";

export const isMobile = /Android|iPhone|iPad/.test(navigator.userAgent);

interface Insets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

declare global {
  interface Window {
    /** Set up by the Android activity: the system bars' insets as JSON. */
    NisshokuInsets?: { get: () => string };
  }
}

/**
 * Pads the app by the system bars (Android draws the app under them, and the
 * web view's `env(safe-area-inset-*)` only covers display cutouts), as
 * `--inset-*`; the activity reports changes with a `nisshoku-insets` event.
 */
export function useSystemInsets() {
  useEffect(() => {
    const apply = (i: Insets) => {
      const root = document.documentElement.style;
      for (const side of ["top", "right", "bottom", "left"] as const) root.setProperty(`--inset-${side}`, `${i[side]}px`);
    };
    try {
      const initial = window.NisshokuInsets?.get();
      if (initial) apply(JSON.parse(initial) as Insets);
    } catch {
      // Not on Android.
    }
    const onInsets = (e: Event) => apply((e as CustomEvent<Insets>).detail);
    window.addEventListener("nisshoku-insets", onInsets);
    return () => window.removeEventListener("nisshoku-insets", onInsets);
  }, []);
}

/** What the back button closes or leaves, innermost first; `null` at the root. */
function backAction(): (() => void) | null {
  const s = useStore.getState();
  if (menuOpen()) return closeMenu;
  if (dialogOpen()) return () => void cancelDialog();
  if (s.preview) return () => s.set({ preview: null });
  if (s.paletteOpen) return () => s.set({ paletteOpen: false });
  if (s.settingsTab) return () => s.set({ settingsTab: null });
  if (s.panel !== "none") return () => s.set({ panel: "none" });
  if (s.focusMode) return () => s.set({ focusMode: false });
  if (s.layout === "narrow") {
    if (s.mobilePane === "editor") return () => s.set({ mobilePane: "list" });
    if (s.mobilePane === "list") return () => s.set({ mobilePane: "nav" });
  } else if (s.layout === "medium" && s.sidebarOpen) return () => s.set({ sidebarOpen: false });
  return null;
}

/**
 * Handles the back button while there is something to go back from. At the
 * root the handler is removed, so that the system's default applies (leaving
 * the app).
 */
export function useBackButton() {
  useEffect(() => {
    if (!isMobile || !isTauri) return;
    let listener: Promise<{ unregister: () => Promise<void> }> | null = null;
    let disposed = false;
    const update = () => {
      const wanted = !disposed && backAction() !== null;
      if (wanted && !listener) {
        listener = import("@tauri-apps/api/app").then(({ onBackButtonPress }) =>
          onBackButtonPress(() => backAction()?.()),
        );
      } else if (!wanted && listener) {
        const l = listener;
        listener = null;
        void l.then((x) => x.unregister());
      }
    };
    update();
    const unsubscribe = [useStore.subscribe(update), onDialogChange(update), onMenuChange(update)];
    return () => {
      disposed = true;
      unsubscribe.forEach((u) => u());
      update();
    };
  }, []);
}
