// Window chrome drawn by the app (the system title bar is hidden): window
// buttons, and handles to resize the side columns.

import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "../api";
import { useT } from "../i18n";
import { useStore } from "../store";
import { isMobile } from "../lib/mobile";

const isMac = navigator.userAgent.includes("Mac");

/** Minimize / maximize / close, at the right of the top bar (not on macOS). */
export function WindowControls() {
  const t = useT();
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    if (!isTauri || isMobile) return;
    const win = getCurrentWindow();
    void win.isMaximized().then(setMaximized);
    const unlisten = win.onResized(() => void win.isMaximized().then(setMaximized));
    return () => void unlisten.then((f) => f());
  }, []);
  if (isMac || isMobile) return null;
  const win = () => getCurrentWindow();
  return (
    <div className="window-controls">
      <button title={t("window.minimize")} onClick={() => isTauri && void win().minimize()}>
        <svg viewBox="0 0 10 10">
          <path d="M0 5h10" />
        </svg>
      </button>
      <button title={maximized ? t("window.restore") : t("window.maximize")} onClick={() => isTauri && void win().toggleMaximize()}>
        {maximized ? (
          <svg viewBox="0 0 10 10">
            <path d="M2.5 2.5V0.5h7v7h-2M0.5 2.5h7v7h-7z" />
          </svg>
        ) : (
          <svg viewBox="0 0 10 10">
            <path d="M0.5 0.5h9v9h-9z" />
          </svg>
        )}
      </button>
      <button className="close" title={t("window.close")} onClick={() => isTauri && void win().close()}>
        <svg viewBox="0 0 10 10">
          <path d="M0 0l10 10M10 0L0 10" />
        </svg>
      </button>
    </div>
  );
}

/** A handle on the right edge of a column; dragging it sets the width. */
export function Resizer({ pref, min, max }: { pref: "sidebarWidth" | "listWidth"; min: number; max: number }) {
  return (
    <div
      className="resizer"
      onMouseDown={(e) => {
        e.preventDefault();
        const startX = e.clientX;
        const start = useStore.getState().prefs[pref];
        let width = start;
        const app = document.querySelector<HTMLElement>(".app");
        const variable = pref === "sidebarWidth" ? "--sidebar-w" : "--list-w";
        document.body.classList.add("resizing");
        const move = (ev: MouseEvent) => {
          width = Math.round(Math.min(max, Math.max(min, start + ev.clientX - startX)));
          app?.style.setProperty(variable, `${width}px`);
        };
        const up = () => {
          document.body.classList.remove("resizing");
          window.removeEventListener("mousemove", move);
          window.removeEventListener("mouseup", up);
          useStore.getState().setPrefs({ [pref]: width });
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
      }}
      onDoubleClick={() => useStore.getState().setPrefs({ [pref]: pref === "sidebarWidth" ? 248 : 308 })}
    />
  );
}
