// Back and forward between the notes opened, like a browser.

import { useSyncExternalStore } from "react";
import { api } from "../api";
import { useStore } from "../store";

const back: string[] = [];
const forward: string[] = [];
let moving = false;
let version = 0;
const listeners = new Set<() => void>();
const LIMIT = 100;

function changed() {
  version++;
  for (const l of listeners) l();
}

useStore.subscribe((s, prev) => {
  if (s.selectedId === prev.selectedId) return;
  if (moving) return;
  if (prev.selectedId && s.selectedId) {
    if (back[back.length - 1] !== prev.selectedId) back.push(prev.selectedId);
    if (back.length > LIMIT) back.shift();
    forward.length = 0;
    changed();
  }
});

async function go(from: string[], to: string[]) {
  const s = useStore.getState();
  while (from.length) {
    const id = from.pop()!;
    if (id === s.selectedId) continue;
    // Skip notes that are gone (deleted for good).
    const note = await api.getNote(id).catch(() => null);
    if (!note) continue;
    if (s.selectedId) to.push(s.selectedId);
    moving = true;
    try {
      // Open it where it lives if the current list doesn't have it.
      await useStore.getState().openNote(id);
    } finally {
      moving = false;
    }
    changed();
    return true;
  }
  changed();
  return false;
}

export const goBack = () => go(back, forward);
export const goForward = () => go(forward, back);

/** Whether there is somewhere to go back or forward to. */
export function useNavState(): { canBack: boolean; canForward: boolean } {
  useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => version,
  );
  return { canBack: back.length > 0, canForward: forward.length > 0 };
}
