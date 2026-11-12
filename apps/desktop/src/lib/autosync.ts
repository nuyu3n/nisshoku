// Syncs a little while after the last edit, so that other devices get
// changes soon without syncing on every keystroke.

import { api } from "../api";
import { useStore } from "../store";

const IDLE_MS = 20_000;
let timer: number | undefined;

/** Edits made since the last sync (sent by the next one). */
let unsent = false;

export function requestSync() {
  unsent = true;
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    // In the background (phones cut the network there): when back in front.
    if (document.visibilityState !== "visible") {
      pending = true;
      return;
    }
    const status = useStore.getState().syncStatus;
    if (status && status.state !== "off" && !status.needsPassword) void api.syncNow().catch(() => {});
  }, IDLE_MS);
}

let pending = false;
let watching = false;

/** Tells the app whether the window is in front (see sync.rs `set_foreground`). */
export function watchForeground() {
  if (watching) return;
  watching = true;
  // A finished sync sent everything written before it.
  useStore.subscribe((now, before) => {
    if (now.syncStatus?.lastSync !== before.syncStatus?.lastSync && now.syncStatus?.state === "idle") unsent = false;
  });
  const tell = () => {
    const visible = document.visibilityState === "visible";
    void api.setForeground(visible).catch(() => {});
    // Phones: leaving with edits not sent yet — a sync in the background
    // sends them (Android lets it run with the network, unlike the app's).
    if (!visible && unsent) {
      const status = useStore.getState().syncStatus;
      if (status && status.state !== "off" && !status.needsPassword) window.NisshokuAndroid?.syncInBackground?.();
    }
    if (visible && pending) {
      pending = false;
      requestSync();
    }
  };
  document.addEventListener("visibilitychange", tell);
  tell();
}
