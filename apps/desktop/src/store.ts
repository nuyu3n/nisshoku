import { create } from "zustand";
import { api, isoDate, type Folder, type NoteKind, type NoteSummary, type Scope, type Stats, type SyncStatus, type TagInfo } from "./api";

export interface Preferences {
  language: "ja" | "en";
  theme: "system" | "light" | "dark";
  sidebarWidth: number;
  listWidth: number;
  editorFont: "serif" | "sans";
  fontSize: number;
  lineHeight: number;
  /** Width of the text column, in characters. */
  textWidth: number;
  indentParagraphs: boolean;
  typewriter: boolean;
  /** In focus mode, dim everything but the current paragraph. */
  dimOthers: boolean;
  /** "..." → "…", "--" → "――" and similar while typing. */
  smartPunctuation: boolean;
  /** In a work, characters' names in the text get their colour. */
  highlightCharacters: boolean;
  /** Numbers in the margin: paragraphs in the rich editor, lines in plain text. */
  lineNumbers: boolean;
  /** Typing 「 adds 」 (and other bracket pairs). */
  autoPair: boolean;
  /** Characters per day; 0 for none. */
  dailyGoal: number;
  diaryTemplate: string;
  /** Title of new diary entries: `iso` (2026-11-04), `isoWeekday` (2026-11-04 Wed), `ja` (2026年11月4日（水）). */
  diaryTitleFormat: "iso" | "isoWeekday" | "ja";
  /** Names of the notebooks the app creates by itself. */
  diaryNotebookName: string;
  defaultNotebookName: string;
  /** Characters per manuscript page (原稿用紙): 20 × 20. */
  manuscriptCols: number;
  manuscriptRows: number;
  /** Cell size of the vertical preview in pixels; 0 fits a page to the panel. */
  genkoCell: number;
  /** Colour of the grid of the vertical preview. */
  genkoInk: "accent" | "vermilion" | "green" | "indigo" | "sumi";
  /** The vertical preview shows the page where the cursor is. */
  genkoFollow: boolean;
  /** Notebooks folded in the sidebar (their sub-notebooks hidden). */
  collapsedFolders: string[];
  /** `custom`: the order set by dragging (Joplin's custom order). */
  sortNotes: "custom" | "updated" | "created" | "title";
  accent: "orange" | "vermilion" | "indigo" | "green" | "sumi";
  /** Interface zoom, percent. */
  zoom: number;
  /** What opens at start: the last note, today's diary entry, or nothing. */
  /** What opens at start; `list`: the note list (on phones; elsewhere the first note, as `last` without one). */
  startup: "last" | "diary" | "list" | "none";
  lastNoteId: string;
  /** The text column uses the whole width of the editor. */
  fullWidth: boolean;
  spellcheck: boolean;
  bubbleToolbar: boolean;
  slashMenu: boolean;
  showExcerpt: boolean;
  compactList: boolean;
  /** Characters a minute, for reading times. */
  readingSpeed: number;
  timerMinutes: number;
  statusManuscript: boolean;
  statusReading: boolean;
  statusToday: boolean;
  /** Count spaces as characters. */
  countSpaces: boolean;
  weekStart: "sun" | "mon";
  /** Minutes between background syncs; 0 for manual only. */
  syncInterval: number;
  syncOnStart: boolean;
  syncAfterEdit: boolean;
  /** Days after which the trash is emptied; 0 for never. */
  trashRetentionDays: number;
  /** Look for a new version at start (and every few hours). */
  autoUpdateCheck: boolean;
}

export const defaultPreferences: Preferences = {
  language: navigator.language.startsWith("ja") ? "ja" : "en",
  theme: "system",
  sidebarWidth: 248,
  listWidth: 308,
  editorFont: "serif",
  fontSize: 17,
  lineHeight: 1.95,
  textWidth: 38,
  indentParagraphs: false,
  typewriter: false,
  dimOthers: true,
  smartPunctuation: true,
  highlightCharacters: true,
  lineNumbers: false,
  autoPair: true,
  dailyGoal: 0,
  diaryTemplate: "",
  diaryTitleFormat: "iso",
  diaryNotebookName: "Diary",
  defaultNotebookName: "Notes",
  manuscriptCols: 20,
  manuscriptRows: 20,
  genkoCell: 0,
  genkoInk: "accent",
  genkoFollow: true,
  collapsedFolders: [],
  sortNotes: "updated",
  accent: "orange",
  zoom: 100,
  startup: "last",
  lastNoteId: "",
  fullWidth: false,
  spellcheck: false,
  bubbleToolbar: true,
  slashMenu: true,
  showExcerpt: true,
  compactList: false,
  readingSpeed: 500,
  timerMinutes: 25,
  statusManuscript: true,
  statusReading: true,
  statusToday: true,
  countSpaces: false,
  weekStart: "sun",
  syncInterval: 5,
  syncOnStart: true,
  syncAfterEdit: true,
  trashRetentionDays: 30,
  autoUpdateCheck: true,
};

/** medium: the note list beside the text, with the sidebar and panels floating over it (any window wider than a phone); narrow: one screen at a time (phones). */
export type Layout = "medium" | "narrow";
export type MobilePane = "nav" | "list" | "editor";

export function layoutFor(width: number): Layout {
  return width >= 720 ? "medium" : "narrow";
}

export type Panel = "none" | "outline" | "history" | "preview" | "comments" | "proof";
export type SidebarTab = "notebooks" | "comments";

export interface Toast {
  id: number;
  message: string;
  action?: { label: string; run: () => void };
}

interface AppStore {
  folders: Folder[];
  tags: TagInfo[];
  scope: Scope;
  notes: NoteSummary[];
  /** To-dos not done yet (for the sidebar). */
  openTasks: number;
  /** Notes under "All notes" (without diary entries and templates, as its list). */
  allNotes: number;
  selectedId: string | null;
  sidebarOpen: boolean;
  /** The sidebar shown for a moment over the page, while the pointer is on its button or on it (lib/peek.ts). */
  sidebarPeek: boolean;
  listOpen: boolean;
  focusMode: boolean;
  panel: Panel;
  paletteOpen: boolean;
  settingsTab: string | null;
  prefs: Preferences;
  syncStatus: SyncStatus | null;
  stats: Stats | null;
  toasts: Toast[];
  /** Bumped when the open note must be reloaded (e.g. restored from history). */
  noteVersion: number;
  /** Bumped when a sync changed notes on this device. */
  remoteVersion: number;
  sidebarTab: SidebarTab;
  /** The comment selected in the editor or a list. */
  activeCommentId: string | null;
  /** Bumped when comments change. */
  commentsVersion: number;
  /** Where the notebooks tab returns to. */
  notebookScope: Scope;
  layout: Layout;
  /** On narrow screens, the screen shown. */
  mobilePane: MobilePane;
  /** An attachment shown in the viewer (PDF). */
  preview: { title: string; url: string; id: string } | null;

  setScope: (scope: Scope) => Promise<void>;
  /** The scope `notes` were listed for (JSON); while it differs from `scope`, they are on their way. */
  notesScope: string | null;
  /** The kind of the note open in the editor: a work's plot, character or material page is not manuscript (no preview). */
  openKind: NoteKind | null;
  loadFolders: () => Promise<void>;
  loadNotes: () => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => void;
  newNote: (parentId?: string) => Promise<void>;
  openDiary: (date?: string) => Promise<void>;
  updateSummary: (summary: NoteSummary) => void;
  reloadNote: () => void;
  set: (patch: Partial<AppStore>) => void;
  setPrefs: (patch: Partial<Preferences>) => void;
  loadPrefs: () => Promise<void>;
  loadStats: () => Promise<void>;
  toast: (message: string, action?: Toast["action"]) => void;
}

let toastId = 0;

function sortNotes(notes: NoteSummary[], scope: Scope, sort: Preferences["sortNotes"]): NoteSummary[] {
  if (scope.kind === "diary" || scope.kind === "trash") return notes;
  // Pinned notes first.
  const sorted = [...notes];
  if (sort === "custom") sorted.sort((a, b) => b.order - a.order || b.createdTime - a.createdTime);
  else if (sort === "created") sorted.sort((a, b) => b.createdTime - a.createdTime);
  else if (sort === "title") sorted.sort((a, b) => a.title.localeCompare(b.title, "ja"));
  else sorted.sort((a, b) => b.updatedTime - a.updatedTime);
  return sorted.sort((a, b) => Number(b.pinned) - Number(a.pinned));
}

export const useStore = create<AppStore>((set, get) => ({
  folders: [],
  tags: [],
  scope: { kind: "all" },
  notes: [],
  notesScope: null,
  openKind: null,
  openTasks: 0,
  allNotes: 0,
  selectedId: null,
  sidebarOpen: false,
  sidebarPeek: false,
  listOpen: true,
  focusMode: false,
  panel: "none",
  paletteOpen: false,
  settingsTab: null,
  prefs: defaultPreferences,
  syncStatus: null,
  stats: null,
  toasts: [],
  noteVersion: 0,
  remoteVersion: 0,
  sidebarTab: "notebooks",
  activeCommentId: null,
  commentsVersion: 0,
  notebookScope: { kind: "all" },
  layout: layoutFor(window.innerWidth),
  mobilePane: "list",
  preview: null,

  setScope: async (scope) => {
    set({ scope, listOpen: true, mobilePane: "list" });
    // Choosing something closes the drawer, whichever tab it was in.
    if (get().layout === "medium") set({ sidebarOpen: false, sidebarPeek: false });
    if (scope.kind !== "search") set({ notebookScope: scope });
    await get().loadNotes();
    const notes = get().notes;
    if (!notes.some((n) => n.id === get().selectedId)) {
      set({ selectedId: notes[0]?.id ?? null });
    }
  },
  loadFolders: async () => {
    const [folders, tags, todos, all] = await Promise.all([api.listFolders(), api.listTags(), api.listNotes({ kind: "todo" }), api.listNotes({ kind: "all" })]);
    set({ folders, tags, openTasks: todos.filter((n) => !n.todoCompleted).length, allNotes: all.length });
  },
  loadNotes: async () => {
    const { scope, prefs } = get();
    const notes = await api.listNotes(scope);
    // A slower answer for a scope left since must not replace the new one's.
    if (get().scope !== scope && JSON.stringify(get().scope) !== JSON.stringify(scope)) return;
    set({ notes: sortNotes(notes, scope, prefs.sortNotes), notesScope: JSON.stringify(scope) });
  },
  refresh: async () => {
    await Promise.all([get().loadFolders(), get().loadNotes(), get().loadStats()]);
  },
  select: (id) => {
    set({ selectedId: id });
    if (id) {
      if (get().layout === "narrow") set({ mobilePane: "editor" });
      if (get().prefs.lastNoteId !== id) get().setPrefs({ lastNoteId: id });
    }
  },
  newNote: async (parentId) => {
    const { scope } = get();
    const target = parentId ?? (scope.kind === "folder" ? scope.id : undefined);
    const note = await api.createNote(target);
    if (scope.kind !== "all" && !(scope.kind === "folder" && scope.id === note.parentId)) {
      set({ scope: { kind: "folder", id: note.parentId } });
    }
    await get().refresh();
    set({ focusMode: false });
    get().select(note.id);
    requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>(".note-title")?.focus());
  },
  openDiary: async (date) => {
    const note = await api.openDiary(date ?? isoDate());
    set({ scope: { kind: "diary" }, listOpen: true });
    if (get().layout === "medium") set({ sidebarOpen: false, sidebarPeek: false });
    await get().refresh();
    get().select(note.id);
  },
  updateSummary: (summary) =>
    set((s) => ({
      notes: s.notes.map((n) => (n.id === summary.id ? { ...n, ...summary } : n)),
    })),
  reloadNote: () => set((s) => ({ noteVersion: s.noteVersion + 1 })),
  // Opening or closing the sidebar ends a peek at it.
  set: (patch) => set("sidebarOpen" in patch ? { sidebarPeek: false, ...patch } : patch),
  setPrefs: (patch) => {
    const prefs = { ...get().prefs, ...patch };
    set({ prefs });
    void api.setPreferences(prefs as unknown as Record<string, unknown>);
  },
  loadPrefs: async () => {
    const stored = await api.getPreferences();
    set({ prefs: { ...defaultPreferences, ...(stored as Partial<Preferences>) } });
  },
  loadStats: async () => set({ stats: await api.getStats() }),
  toast: (message, action) => {
    const id = ++toastId;
    set((s) => ({ toasts: [...s.toasts, { id, message, action }] }));
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), action ? 6000 : 3500);
  },
}));

/** Shows a failed command as a toast instead of an unhandled rejection. */
export function report(e: unknown) {
  useStore.getState().toast(typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
}
