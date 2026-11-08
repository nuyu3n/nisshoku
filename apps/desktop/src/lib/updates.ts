// Updates: checking for a new version, and installing it.
//
// - Windows and Linux: the updater plugin downloads the signed package of
//   the newest GitHub release (its `latest.json`) and installs it the way
//   this copy was installed (NSIS / MSI, AppImage, .deb or .rpm), then the
//   app restarts.
// - Android: the release's APK is opened in the browser to download and
//   install (an app cannot replace itself outside a store).

import { create } from "zustand";
import { api, isTauri, type InstallKind } from "../api";
import { isMobile } from "./mobile";
import { flush } from "./actions";
import { useStore } from "../store";
import { lang } from "../i18n";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

export type UpdateStatus = "idle" | "checking" | "none" | "available" | "downloading" | "installing" | "error";

interface UpdateState {
  status: UpdateStatus;
  current: string;
  kind: InstallKind | null;
  version: string | null;
  notes: string;
  date: string | null;
  /** The release page. */
  url: string | null;
  apkUrl: string | null;
  /** Downloaded bytes and the total, while downloading. */
  downloaded: number;
  total: number | null;
  error: string | null;
  checkedAt: number | null;
}

export const useUpdate = create<UpdateState>(() => ({
  status: "idle",
  current: "",
  kind: null,
  version: null,
  notes: "",
  date: null,
  url: null,
  apkUrl: null,
  downloaded: 0,
  total: null,
  error: null,
  checkedAt: null,
}));

export const RELEASES_URL = "https://github.com/nuyu3n/nisshoku/releases";
const LAST_CHECK = "nisshoku.updateCheckedAt";
/** Automatic checks at most this often. */
const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

/** The plugin's pending update (desktop), kept to install it. */
let pending: import("@tauri-apps/plugin-updater").Update | null = null;

export async function appVersion(): Promise<string> {
  if (!isTauri) return "0.1.0";
  const { getVersion } = await import("@tauri-apps/api/app");
  return getVersion();
}

/** `a` is newer than `b` (dotted numbers; a pre-release such as `-beta.1` comes before its release). */
export function isNewer(a: string, b: string): boolean {
  const parse = (v: string) => {
    const [main, ...pre] = v.trim().replace(/^v/, "").split("-");
    return { nums: main.split(".").map((n) => parseInt(n, 10) || 0), pre: pre.length ? pre.join("-") : null };
  };
  const x = parse(a);
  const y = parse(b);
  for (let i = 0; i < Math.max(x.nums.length, y.nums.length); i++) {
    const d = (x.nums[i] ?? 0) - (y.nums[i] ?? 0);
    if (d !== 0) return d > 0;
  }
  if (x.pre === y.pre) return false;
  if (x.pre === null) return true;
  if (y.pre === null) return false;
  return x.pre.localeCompare(y.pre, "en", { numeric: true }) > 0;
}

/** Looks for a newer version. `silent`: failures are not shown (automatic checks). */
export async function checkForUpdate(silent = false): Promise<void> {
  const set = useUpdate.setState;
  const now = useUpdate.getState().status;
  if (now === "downloading" || now === "installing" || now === "checking") return;
  set({ status: "checking", error: null });
  try {
    const [current, kind] = await Promise.all([appVersion(), api.installKind().catch(() => "dev" as InstallKind)]);
    set({ current, kind });
    try {
      localStorage.setItem(LAST_CHECK, String(Date.now()));
    } catch {
      /* storage blocked */
    }
    if (isTauri && !isMobile) {
      // Desktop: the updater plugin reads the signed feed.
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check({ timeout: 30_000 });
      pending = update;
      if (!update) {
        set({ status: "none", checkedAt: Date.now() });
        return;
      }
      set({
        status: "available",
        version: update.version,
        notes: update.body ?? "",
        date: update.date ?? null,
        url: `${RELEASES_URL}/tag/v${update.version}`,
        apkUrl: null,
        checkedAt: Date.now(),
      });
    } else {
      // Android (and the browser preview): the newest release on GitHub.
      const release = await api.latestRelease();
      if (!isNewer(release.version, current)) {
        set({ status: "none", checkedAt: Date.now() });
        return;
      }
      set({
        status: "available",
        version: release.version,
        notes: release.notes,
        date: release.publishedAt,
        url: release.url,
        apkUrl: release.apkUrl,
        checkedAt: Date.now(),
      });
    }
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    set({ status: silent ? "idle" : "error", error: message });
  }
}

/** Downloads and installs the version found, then restarts the app. */
export async function installUpdate(): Promise<void> {
  const s = useUpdate.getState();
  const set = useUpdate.setState;
  if (s.status !== "available") return;
  if (isMobile || !isTauri) {
    // Android installs packages through the system installer.
    await openExternal(s.apkUrl ?? s.url ?? RELEASES_URL);
    return;
  }
  if (!pending) return;
  await flush().catch(() => {});
  set({ status: "downloading", downloaded: 0, total: null, error: null });
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === "Started") set({ total: event.data.contentLength ?? null });
      else if (event.event === "Progress") set((st) => ({ downloaded: st.downloaded + event.data.chunkLength }));
      else if (event.event === "Finished") set({ status: "installing" });
    });
    // Windows' installer closes the app by itself; elsewhere, restart into the new version.
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch (e) {
    set({ status: "error", error: e instanceof Error ? e.message : String(e) });
  }
}

async function openExternal(url: string) {
  if (!isTauri) {
    window.open(url, "_blank");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

/** Opens the page of the release (its notes in full, and every download). */
export function openReleasePage(): Promise<void> {
  return openExternal(useUpdate.getState().url ?? RELEASES_URL);
}

/** At start: an automatic check (unless turned off), at most every few hours. */
export function startUpdateChecks(): void {
  if (!isTauri) return;
  const run = async () => {
    if (!useStore.getState().prefs.autoUpdateCheck) return;
    let last = 0;
    try {
      last = Number(localStorage.getItem(LAST_CHECK) ?? 0);
    } catch {
      /* storage blocked */
    }
    if (Date.now() - last < CHECK_EVERY_MS) return;
    await checkForUpdate(true);
    const u = useUpdate.getState();
    if (u.status === "available" && u.version) {
      useStore.getState().toast(L(`新しい版 ${u.version} があります`, `Version ${u.version} is available`), {
        label: L("見る", "View"),
        run: () => useStore.getState().set({ settingsTab: "about" }),
      });
    }
  };
  // A moment after start (not to slow it down), then every few hours while open.
  window.setTimeout(() => void run(), 8000);
  window.setInterval(() => void run(), CHECK_EVERY_MS);
}
