// What changed: the release notes (CHANGELOG.md, bundled with the app),
// shown once after an update and on demand from Settings → About.

import { create } from "zustand";
import changelog from "../../CHANGELOG.md?raw";
import { useStore } from "../store";
import { appVersion, isNewer } from "./updates";

export interface ReleaseNotes {
  version: string;
  items: string[];
}

/** The versions of CHANGELOG.md, newest first. */
export function releases(text = changelog): ReleaseNotes[] {
  const out: ReleaseNotes[] = [];
  for (const line of text.replace(/\r\n/g, "\n").split("\n")) {
    const head = /^##\s+(\S+)/.exec(line);
    if (head) out.push({ version: head[1], items: [] });
    else if (out.length && /^\s*-\s+/.test(line)) out[out.length - 1].items.push(line.replace(/^\s*-\s+/, ""));
  }
  return out;
}

/** The dialog's content: the versions shown, or null when closed. */
export const useWhatsNew = create<{ shown: ReleaseNotes[] | null; title: "new" | "all" }>(() => ({ shown: null, title: "new" }));

export function showAllReleases() {
  useWhatsNew.setState({ shown: releases(), title: "all" });
}

/**
 * At start: if this version is newer than the one last opened, show what
 * changed since. A first start with no notes yet (a new install) shows
 * nothing; one with notes (an update from before this existed) shows this
 * version's notes.
 */
export async function showWhatsNewIfUpdated() {
  const current = await appVersion();
  const s = useStore.getState();
  const seen = s.prefs.lastSeenVersion;
  if (seen === current) return;
  s.setPrefs({ lastSeenVersion: current });
  const all = releases();
  let shown: ReleaseNotes[];
  if (seen) shown = all.filter((r) => isNewer(r.version, seen) && !isNewer(r.version, current));
  else shown = s.allNotes > 0 ? all.filter((r) => r.version === current) : [];
  if (shown.length && shown.some((r) => r.items.length)) useWhatsNew.setState({ shown, title: "new" });
}
