// The Android app and its home-screen widgets: links the widgets open the app
// with (`nisshoku://…`), tasks ticked off in a widget, and keeping the
// widgets' snapshot up to date. See MainActivity.kt and Widgets.kt.

import { api, isTauri } from "../api";
import { report, useStore } from "../store";
import { isMobile } from "./mobile";
import { t } from "../i18n";
import { FOCUS_TASK_INPUT } from "../components/Tasks";

declare global {
  interface Window {
    /** Set up by the Android activity. */
    NisshokuAndroid?: {
      takeLink: () => string | null;
      setDarkTheme: (dark: boolean) => void;
      /** Android 13+: the app's language in the system too. */
      setLanguage?: (language: string) => void;
      /** Android 13+: the language set for the app in the system settings, or "". */
      systemLanguage?: () => string;
      /** A sync begins: the sync service keeps it going off screen, with a notification. */
      syncStarted?: (dir: string) => void;
      /** One sync in the background as soon as the network allows. */
      syncInBackground?: () => void;
      /** Background syncs every `minutes` (0: none), the app closed too. */
      setBackgroundSync?: (minutes: number, dir: string) => void;
    };
  }
}

/** Opens what a widget link points to. */
async function openLink(link: string) {
  const s = useStore.getState();
  const url = new URL(link);
  // `nisshoku://note/<id>`: the host is the kind, the path the rest.
  const kind = url.host;
  const rest = url.pathname.replace(/^\//, "");
  if (kind === "note" && rest) {
    if (s.scope.kind !== "todo") await s.setScope({ kind: "todo" });
    s.select(rest);
  } else if (kind === "tasks" || kind === "new-task") {
    await s.setScope({ kind: "todo" });
    if (kind === "new-task") window.setTimeout(() => window.dispatchEvent(new Event(FOCUS_TASK_INPUT)), 300);
  } else if (kind === "diary") {
    await s.openDiary();
  } else if (kind === "new-note") {
    await s.newNote();
  } else if (kind === "share") {
    await newNoteFromShare(url.searchParams.get("title") ?? "", url.searchParams.get("text") ?? "");
  }
}

/** Text shared from another app (or picked in its text-selection menu) becomes a new note. */
async function newNoteFromShare(subject: string, text: string) {
  const s = useStore.getState();
  const body = text.trim();
  const firstLine = body.split("\n")[0].trim();
  // A short single line of text (a phrase picked in another app) is the title alone; links stay in the body.
  const titleOnly = !subject.trim() && !body.includes("\n") && body.length <= 60 && !/^[a-z]+:\/\//i.test(body);
  const title = (subject.trim() || firstLine).slice(0, 60);
  const note = await api.createNote(undefined, title, titleOnly ? "" : body);
  if (s.scope.kind !== "all") s.set({ scope: { kind: "all" } });
  await s.refresh();
  s.set({ focusMode: false });
  s.select(note.id);
  s.toast(t("android.shared"));
}

/** Applies tasks ticked off in the widgets. */
async function applyWidgetActions() {
  const changed = await api.applyWidgetActions().catch(() => 0);
  if (changed > 0) {
    const s = useStore.getState();
    await s.refresh().catch(report);
    s.reloadNote();
  }
}

/** A language chosen for the app in Android's settings becomes the app's. */
function adoptSystemLanguage() {
  const system = window.NisshokuAndroid?.systemLanguage?.();
  const s = useStore.getState();
  if ((system === "ja" || system === "en") && system !== s.prefs.language) s.setPrefs({ language: system });
}

/** The app's language and Android's per-app language follow each other. */
function syncLanguage() {
  const bridge = window.NisshokuAndroid;
  if (!bridge?.setLanguage) return;
  adoptSystemLanguage();
  bridge.setLanguage(useStore.getState().prefs.language);
  useStore.subscribe((now, before) => {
    if (now.prefs.language !== before.prefs.language) bridge.setLanguage?.(now.prefs.language);
  });
}

/**
 * Syncs: a running one is kept going off screen by the sync service (with
 * its progress in a notification), and background syncs follow the setting.
 */
async function startSyncBridge() {
  const bridge = window.NisshokuAndroid;
  if (!bridge?.syncStarted) return;
  const dir = await api.profileDir().catch(() => null);
  if (!dir) return;
  const schedule = () => bridge.setBackgroundSync?.(useStore.getState().syncStatus?.state === "off" ? 0 : useStore.getState().prefs.backgroundSyncMinutes, dir);
  schedule();
  useStore.subscribe((now, before) => {
    if (now.syncStatus?.state === "syncing" && before.syncStatus?.state !== "syncing") bridge.syncStarted?.(dir);
    if (now.prefs.backgroundSyncMinutes !== before.prefs.backgroundSyncMinutes || (now.syncStatus?.state === "off") !== (before.syncStatus?.state === "off")) schedule();
  });
}

let started = false;

/** Called once the app has opened what it shows at start. */
export function startAndroidBridge() {
  if (!isMobile || !isTauri || started) return;
  started = true;

  void applyWidgetActions();
  syncLanguage();
  void startSyncBridge();
  const link = window.NisshokuAndroid?.takeLink();
  if (link) void openLink(link).catch(report);

  window.addEventListener("nisshoku-intent", (e) => void openLink((e as CustomEvent<string>).detail).catch(report));
  window.addEventListener("nisshoku-widget-action", () => void applyWidgetActions());

  // The widgets follow the app's notes, tasks and figures, a moment after they change.
  let timer = 0;
  const refresh = () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void api.refreshWidgets().catch(() => {}), 1500);
  };
  useStore.subscribe((now, before) => {
    if (now.notes !== before.notes || now.stats !== before.stats || now.openTasks !== before.openTasks || now.prefs !== before.prefs) refresh();
  });
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") {
      void applyWidgetActions();
      adoptSystemLanguage();
    }
    else void api.refreshWidgets().catch(() => {});
  });
  refresh();
}
