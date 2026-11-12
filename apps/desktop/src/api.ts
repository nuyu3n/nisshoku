// Typed access to the commands of the Rust side (src-tauri).

import { invoke } from "@tauri-apps/api/core";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type MockInvoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
let mock: MockInvoke | null = null;

/** Calls a command; outside the app (a browser during development), a mock answers. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) return invoke<T>(cmd, args);
  mock ??= (await import("./dev/mock")).mockInvoke;
  return (await mock(cmd, args)) as T;
}

export interface Folder {
  id: string;
  title: string;
  parentId: string;
  icon: string;
  /** `#rrggbb`, or empty. */
  color: string;
  isDiary: boolean;
  /** Marked as a work (a novel…): its notes are a manuscript, characters, a plot and materials. */
  isWork: boolean;
  noteCount: number;
}

export interface NoteSummary {
  id: string;
  parentId: string;
  title: string;
  excerpt: string;
  charCount: number;
  createdTime: number;
  updatedTime: number;
  isConflict: boolean;
  deletedTime: number;
  diaryDate: string | null;
  goal: number | null;
  pinned: boolean;
  isTodo: boolean;
  todoCompleted: boolean;
  /** When the to-do is due (Unix ms), 0 if it has no due date. */
  todoDue: number;
  /** Due on a day, without a time. */
  dueAllDay: boolean;
  /** When the to-do was completed (Unix ms), 0 if open. */
  completedTime: number;
  order: number;
  tags: string[];
  kind: NoteKind | null;
}

export interface TagInfo {
  id: string;
  title: string;
  noteCount: number;
}

export interface LinkedNote {
  id: string;
  title: string;
}

export interface ResourceFile {
  id: string;
  path: string;
  mime: string;
  title: string;
  size: number;
}

export interface StorageInfo {
  databaseBytes: number;
  resourceBytes: number;
  resourceCount: number;
  unusedCount: number;
  unusedBytes: number;
}

export interface Note {
  id: string;
  parentId: string;
  title: string;
  body: string;
  createdTime: number;
  updatedTime: number;
  isConflict: boolean;
  /** For a conflict copy, the note it copies. */
  conflictOriginalId?: string;
  deletedTime: number;
  diaryDate: string | null;
  goal: number | null;
  pinned: boolean;
  isTodo: boolean;
  todoCompleted: boolean;
  todoDue: number;
  dueAllDay: boolean;
  kind: NoteKind | null;
}

/** What a note is in a work; none for the manuscript's chapters. */
export type NoteKind = "character" | "plot" | "material";

export interface WorkMeta {
  logline: string;
  synopsis: string;
  genre: string;
  targetChars: number | null;
  /** YYYY-MM-DD */
  deadline: string | null;
}

export type ChapterStatus = "" | "idea" | "draft" | "revise" | "done";

export interface ChapterMeta {
  status: ChapterStatus;
  synopsis: string;
  /** The point-of-view character's note. */
  pov: string | null;
}

export interface ChapterInfo extends NoteSummary, ChapterMeta {
  /** Characters mentioned: [id, count], most first. */
  characters: [string, number][];
}

export interface WorkSection {
  folderId: string;
  title: string;
  depth: number;
  chapters: ChapterInfo[];
}

export type CharacterRole = "" | "protagonist" | "antagonist" | "main" | "supporting" | "minor";

export interface CharacterInfo {
  id: string;
  name: string;
  ruby: string;
  role: CharacterRole;
  color: string;
  aliases: string[];
  excerpt: string;
  mentions: number;
  chapters: string[];
}

export interface WorkOverview {
  id: string;
  title: string;
  meta: WorkMeta;
  sections: WorkSection[];
  characters: CharacterInfo[];
  plotId: string | null;
  materials: NoteSummary[];
  totalChars: number;
  chapterCount: number;
  statusCounts: Record<string, number>;
}

export interface Relation {
  target: string;
  label: string;
}

export interface Character {
  ruby: string;
  role: CharacterRole;
  age: string;
  gender: string;
  appearance: string;
  personality: string;
  background: string;
  motivation: string;
  aliases: string[];
  color: string;
  relations: Relation[];
}

export interface PlotColumn {
  id: string;
  title: string;
}

export interface PlotCard {
  id: string;
  column: string;
  title: string;
  text: string;
  chapter: string | null;
  done: boolean;
}

export interface Plot {
  template: string;
  columns: PlotColumn[];
  cards: PlotCard[];
}

export interface ManuscriptPart {
  title: string;
  depth: number;
  notes: Note[];
}

export type Scope =
  | { kind: "all" }
  | { kind: "folder"; id: string }
  | { kind: "diary" }
  | { kind: "trash" }
  | { kind: "conflicts" }
  | { kind: "search"; query: string }
  | { kind: "tag"; id: string }
  | { kind: "pinned" }
  | { kind: "todo" };

export interface DiaryDay {
  date: string;
  noteId: string;
  charCount: number;
  mood: string | null;
}

export interface NoteComment {
  noteId: string;
  noteTitle: string;
  id: string;
  quote: string;
  before: string;
  text: string;
  createdTime: number;
  updatedTime: number;
  resolved: boolean;
}

export interface DiaryEntry {
  noteId: string;
  date: string;
  title: string;
  excerpt: string;
  charCount: number;
  mood: string | null;
  weather: string | null;
  events: DiaryEvent[];
}

export interface DiaryEvent {
  time: string;
  text: string;
}

export interface MemoItem {
  text: string;
  done: boolean;
}

export interface DiaryStats {
  streak: number;
  longestStreak: number;
  monthDays: number;
  totalDays: number;
  totalChars: number;
  moods: [string, number][];
}

export interface Revision {
  id: string;
  time: number;
}

export interface Stats {
  today: number;
  streak: number;
  days: [string, number][];
  history: [string, number][];
  totalNotes: number;
  totalChars: number;
}

export type Target =
  | { kind: "none" }
  | { kind: "local"; path: string }
  | { kind: "nextcloud"; server: string; user: string; folder: string }
  | { kind: "googleDrive"; folder: string; clientId?: string }
  | { kind: "oneDrive"; clientId: string; folder: string };

export interface SyncReport {
  created_remote: number;
  updated_remote: number;
  deleted_remote: number;
  created_local: number;
  updated_local: number;
  deleted_local: number;
  conflicts: number;
  errors: string[];
}

export interface SyncStatus {
  state: "off" | "idle" | "syncing" | "error";
  message: string | null;
  lastSync: number;
  needsPassword: boolean;
  lastReport: SyncReport | null;
}

export interface SyncSettings {
  target: Target;
  status: SyncStatus;
  gdriveAvailable: boolean;
  hasMasterPassword: boolean;
}

export const api = {
  listFolders: () => call<Folder[]>("list_folders"),
  createFolder: (title: string, parentId?: string) =>
    call<string>("create_folder", { title, parentId: parentId ?? null }),
  renameFolder: (id: string, title: string) => call<void>("rename_folder", { id, title }),
  moveFolder: (id: string, parentId: string) => call<void>("move_folder", { id, parentId }),
  setFolderIcon: (id: string, emoji: string | null) => call<void>("set_folder_icon", { id, emoji }),
  exportAll: (dir: string) => call<number>("export_all", { dir }),
  trashFolder: (id: string) => call<void>("trash_folder", { id }),

  listNotes: (scope: Scope) => call<NoteSummary[]>("list_notes", { scope }),
  getNote: (id: string) => call<Note>("get_note", { id }),
  createNote: (parentId?: string, title?: string, body?: string) =>
    call<Note>("create_note", { parentId: parentId ?? null, title: title ?? null, body: body ?? null }),
  /** `base`: the text (as stored) the edit started from; a different stored text is kept as a conflict copy. */
  saveNote: (id: string, title: string, body: string, base?: { title: string; body: string }) =>
    call<NoteSummary & { conflictId?: string }>("save_note", { id, title, body, baseTitle: base?.title ?? null, baseBody: base?.body ?? null }),
  moveNote: (id: string, parentId: string) => call<void>("move_note", { id, parentId }),
  setNoteGoal: (id: string, goal: number | null) => call<void>("set_note_goal", { id, goal }),
  duplicateNote: (id: string) => call<Note>("duplicate_note", { id }),
  trashNote: (id: string) => call<void>("trash_note", { id }),
  restoreNote: (id: string) => call<void>("restore_note", { id }),
  deleteNoteForever: (id: string) => call<void>("delete_note_forever", { id }),
  emptyTrash: () => call<number>("empty_trash"),

  openDiary: (date: string) => call<Note>("open_diary", { date }),
  diaryMonth: (month: string) => call<DiaryDay[]>("diary_month", { month }),
  diaryStreak: (today: string) => call<number>("diary_streak", { today }),
  diaryEntries: (query: string | null) => call<DiaryEntry[]>("diary_entries", { query }),
  onThisDay: (date: string) => call<DiaryEntry[]>("on_this_day", { date }),
  setDiaryMeta: (noteId: string, mood: string | null, weather: string | null) => call<void>("set_diary_meta", { noteId, mood, weather }),
  diaryStats: (today: string) => call<DiaryStats>("diary_stats", { today }),
  diaryExport: (period: string, path: string) => call<number>("diary_export", { period, path }),
  setDiaryEvents: (noteId: string, events: DiaryEvent[]) => call<void>("set_diary_events", { noteId, events }),
  getDiaryMemo: () => call<{ noteId: string; items: MemoItem[] }>("get_diary_memo"),
  setDiaryMemo: (items: MemoItem[]) => call<void>("set_diary_memo", { items }),

  listRevisions: (noteId: string) => call<Revision[]>("list_revisions", { noteId }),
  revisionContent: (noteId: string, revisionId: string) =>
    call<{ title: string; body: string }>("revision_content", { noteId, revisionId }),
  restoreRevision: (noteId: string, revisionId: string) =>
    call<Note>("restore_revision", { noteId, revisionId }),
  saveSnapshot: (noteId: string) => call<boolean>("save_snapshot", { noteId }),
  backupNow: (dir: string) => call<string>("backup_now", { dir }),
  openBackups: () => call<void>("open_backups"),
  storageInfo: () => call<StorageInfo>("storage_info"),
  deleteUnusedResources: () => call<number>("delete_unused_resources"),
  resolveConflict: (id: string, keep: "copy" | "original") => call<string>("resolve_conflict", { id, keep }),
  clearHistory: (noteId: string) => call<number>("clear_history", { noteId }),

  listTags: () => call<TagInfo[]>("list_tags"),
  setNoteTags: (noteId: string, titles: string[]) => call<void>("set_note_tags", { noteId, titles }),
  renameTag: (id: string, title: string) => call<void>("rename_tag", { id, title }),
  deleteTag: (id: string) => call<void>("delete_tag", { id }),
  setNotePinned: (id: string, pinned: boolean) => call<void>("set_note_pinned", { id, pinned }),
  setNoteTodo: (id: string, todo: boolean) => call<void>("set_note_todo", { id, todo }),
  setTodoCompleted: (id: string, completed: boolean) => call<void>("set_todo_completed", { id, completed }),
  createTask: (title: string, parentId: string | null, due: number, allDay: boolean) =>
    call<NoteSummary>("create_task", { title, parentId, due, allDay }),
  setTodoDue: (id: string, due: number, allDay: boolean) => call<void>("set_todo_due", { id, due, allDay }),
  refreshWidgets: () => call<void>("refresh_widgets"),
  applyWidgetActions: () => call<number>("apply_widget_actions"),
  setNotesOrder: (ids: string[]) => call<void>("set_notes_order", { ids }),
  setNoteOrder: (id: string, order: number) => call<void>("set_note_order", { id, order }),
  listBacklinks: (noteId: string) => call<LinkedNote[]>("list_backlinks", { noteId }),
  saveAsTemplate: (noteId: string) => call<string>("save_as_template", { noteId }),
  listTemplates: () => call<LinkedNote[]>("list_templates"),
  /** The data goes as the raw body of the call (not as a list of numbers). */
  saveResource: (name: string, mime: string, data: Uint8Array) =>
    isTauri
      ? invoke<ResourceFile>("save_resource_bytes", data, { headers: { "x-name": encodeURIComponent(name), "x-mime": mime } })
      : call<ResourceFile>("save_resource", { name, mime, data: Array.from(data) }),
  resourceFiles: () => call<ResourceFile[]>("resource_files"),
  importResource: (path: string) => call<ResourceFile>("import_resource", { path }),
  openResource: (id: string, confirmed = false) => call<void>("open_resource", { id, confirmed }),
  clipUrl: (url: string, parentId: string | null) => call<Note>("clip_url", { url, parentId }),

  listComments: (noteId: string | null) => call<NoteComment[]>("list_comments", { noteId }),
  addComment: (noteId: string, quote: string, before: string, text: string) =>
    call<NoteComment>("add_comment", { noteId, quote, before, text }),
  updateComment: (noteId: string, commentId: string, patch: { text?: string; resolved?: boolean }) =>
    call<void>("update_comment", { noteId, commentId, text: patch.text ?? null, resolved: patch.resolved ?? null }),
  deleteComment: (noteId: string, commentId: string) => call<void>("delete_comment", { noteId, commentId }),

  getStats: () => call<Stats>("get_stats"),
  getPreferences: () => call<Record<string, unknown>>("get_preferences"),
  setPreferences: (preferences: Record<string, unknown>) =>
    call<void>("set_preferences", { preferences }),
  writeTextFile: (path: string, contents: string) =>
    call<void>("write_text_file", { path, contents }),

  getSyncSettings: () => call<SyncSettings>("get_sync_settings"),
  setLocalTarget: (path: string) => call<void>("set_local_target", { path }),
  disconnectSync: () => call<void>("disconnect_sync"),
  nextcloudLoginStart: (server: string) => call<string>("nextcloud_login_start", { server }),
  nextcloudLoginWait: (folder: string) => call<void>("nextcloud_login_wait", { folder }),
  nextcloudConnect: (server: string, user: string, password: string, folder: string) =>
    call<void>("nextcloud_connect", { server, user, password, folder }),
  /** Without a client ID, the Google client built into the app is used. */
  gdriveLogin: (folder: string, clientId?: string, clientSecret?: string) =>
    call<void>("gdrive_login", { folder, clientId: clientId || null, clientSecret: clientSecret || null }),
  onedriveLogin: (clientId: string, folder: string) => call<void>("onedrive_login", { clientId, folder }),
  setMasterPassword: (password: string) => call<SyncStatus>("set_master_password", { password }),
  setEncryption: (enabled: boolean, password?: string) =>
    call<void>("set_encryption", { enabled, password: password ?? null }),
  setFolderColor: (id: string, color: string | null) => call<void>("set_folder_color", { id, color }),
  setForeground: (visible: boolean) => call<void>("set_foreground", { visible }),
  syncNow: () => call<SyncStatus>("sync_now"),
  latestRelease: () => call<Release>("latest_release"),
  // Works
  setWork: (folderId: string, meta: WorkMeta | null) => call<void>("set_work", { folderId, meta }),
  workOverview: (workId: string) => call<WorkOverview>("work_overview", { workId }),
  setNoteKind: (noteId: string, kind: NoteKind | null) => call<void>("set_note_kind", { noteId, kind }),
  setChapterMeta: (noteId: string, meta: ChapterMeta) => call<void>("set_chapter_meta", { noteId, meta }),
  createWorkNote: (workId: string, kind: NoteKind, title: string) => call<Note>("create_work_note", { workId, kind, title }),
  getCharacter: (noteId: string) => call<Character>("get_character", { noteId }),
  setCharacter: (noteId: string, character: Character) => call<void>("set_character", { noteId, character }),
  openPlot: (workId: string, template?: string) => call<{ noteId: string; plot: Plot }>("open_plot", { workId, template: template ?? null }),
  setPlot: (noteId: string, plot: Plot) => call<void>("set_plot", { noteId, plot }),
  exportEpub: (workId: string, path: string, vertical: boolean, author?: string) =>
    call<number>("export_epub", { workId, path, vertical, author: author ?? null }),
  workManuscript: (workId: string) => call<ManuscriptPart[]>("work_manuscript", { workId }),
  installKind: () => call<InstallKind>("install_kind"),
};

/** A published version (GitHub release). */
export interface Release {
  version: string;
  notes: string;
  url: string;
  apkUrl: string | null;
  publishedAt: string | null;
}

/** How the app was installed, which decides how it updates. */
export type InstallKind = "nsis" | "msi" | "appimage" | "deb" | "rpm" | "android" | "dev";

/** Local date as YYYY-MM-DD. */
export function isoDate(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
