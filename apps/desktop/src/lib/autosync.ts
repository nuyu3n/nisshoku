// Syncs a little while after the last edit, so that other devices get
// changes soon without syncing on every keystroke.

import { api } from "../api";
import { useStore } from "../store";

const IDLE_MS = 20_000;
let timer: number | undefined;

export function requestSync() {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    const status = useStore.getState().syncStatus;
    if (status && status.state !== "off" && !status.needsPassword) void api.syncNow().catch(() => {});
  }, IDLE_MS);
}
