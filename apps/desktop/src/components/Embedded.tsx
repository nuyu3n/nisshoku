// The editor alone, for the native Android app (apps/android): it shows its
// own screens (notebooks, lists, settings, sync) and opens a note here, in a
// web view, with `?embed=editor&id=<note>`. The commands reach the engine
// through the app's bridge, which stands in for Tauri's.

import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { report, useStore } from "../store";
import { EditorPane } from "./EditorPane";
import { cancelDialog, DialogHost } from "./Dialogs";
import { ErrorBoundary } from "./ErrorBoundary";
import { AttachmentViewer } from "./Viewer";
import { closeMenu, MenuHost } from "./ContextMenu";
import { TooltipHost } from "./Tooltip";
import { flush } from "../lib/actions";
import { loadResources, refreshImages } from "../editor/images";
import { useBackButton, useSystemInsets } from "../lib/mobile";
import { hideSplash } from "../lib/motion";
import { Toasts, useTheme } from "../App";

declare global {
  interface Window {
    /** The native app's side of the embedded editor. */
    NisshokuNative?: {
      /** The editor is done (back from the note): the app shows its own screens again. */
      close: () => void;
      /** The note open now (a link may lead to another). */
      opened: (id: string) => void;
      /** Opens the note in the app's own text editor instead. */
      plain?: () => void;
    };
    /** For the app: saves what is typed but not saved yet. */
    __nisshokuFlush?: () => Promise<void>;
  }
}

export function EmbeddedEditor({ noteId }: { noteId: string }) {
  const language = useStore((s) => s.prefs.language);
  const ready = useRef(false);
  useTheme();
  useBackButton();
  useSystemInsets();

  useEffect(() => {
    window.__nisshokuFlush = () => flush().catch(() => {});
    (async () => {
      const s = useStore.getState();
      s.set({ layout: "narrow" });
      await s.loadPrefs().catch(report);
      await s.refresh().catch(report);
      await loadResources().catch(() => {});
      const opened = await s.openNote(noteId).catch(() => false);
      if (!opened) window.NisshokuNative?.close();
      s.set({ mobilePane: "editor" });
      ready.current = true;
      requestAnimationFrame(() => hideSplash());
    })();

    // Back from the note (its back button, or the system's at the root):
    // saved first, then the app takes over.
    const unsubscribe = useStore.subscribe((now, before) => {
      if (!ready.current) return;
      if (now.mobilePane !== "editor" && before.mobilePane === "editor") {
        void flush()
          .catch(() => {})
          .then(() => window.NisshokuNative?.close());
      } else if (now.selectedId && now.selectedId !== before.selectedId) {
        window.NisshokuNative?.opened(now.selectedId);
      }
    });
    const unlisten = listen("data-changed", async () => {
      const s = useStore.getState();
      await s.refresh().catch(report);
      await loadResources().catch(() => {});
      refreshImages();
      // The open note may have been changed by another device.
      s.set({ remoteVersion: s.remoteVersion + 1 });
    });
    return () => {
      unsubscribe();
      void unlisten.then((f) => f());
    };
  }, [noteId]);

  return (
    <div className="app layout-narrow pane-editor mobile embedded" lang={language}>
      <ErrorBoundary kind="pane" className="main">
        <EditorPane />
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
