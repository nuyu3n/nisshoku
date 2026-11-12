// The editor: title, body (rich text stored as Markdown), autosave, and the
// tools around them.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EditorContent, useEditor, type Editor } from "@tiptap/react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

import { api, type Note, type NoteComment } from "../api";
import { report, useStore } from "../store";
import { t, useT } from "../i18n";
import { editorExtensions } from "../editor/setup";
import { editorMarkdown, normalizeMarkdown } from "../editor/cjk";
import { goToNumber, numberAt, refreshLineNumbers, unitCount } from "../editor/lines";
import type { TextLinesHandle } from "./TextLines";
import { SlashMenu } from "../editor/slash";
import { selectionAnchor } from "../editor/comments";
import { editMath, editorMenu, setLink } from "../editor/menus";
import { countChars, countWithSpaces, formatNumber, longDate, manuscript, markdownToPlain, rubyToHtml } from "../lib/text";
import { requestSync } from "../lib/autosync";
import { crumbMenu, duplicateNote, editTags, folderTrail, inputMenu, moveNote, moveSubmenu, registerFlush, saveTemplate, togglePin, toggleTodo, trashNote } from "../lib/actions";
import { api as notesApi, type LinkedNote } from "../api";
import { writeText } from "../lib/clipboard";
import { Icon } from "./Icon";
import { peekButtonProps, pinSidebar } from "../lib/peek";
import { dialogs } from "./Dialogs";
import { CommentsPanel, HistoryPanel, OutlinePanel, PreviewPanel, ProofPanel, ReferencePanel } from "./Panels";
import { insertFiles, isAttachment, openAttachment, resourcesReady } from "../editor/images";
import { tsvToTable } from "../editor/table";
import { isLocked, lockBody, unlockBody } from "../lib/lock";
import { TablePicker } from "./TablePicker";
import { ConflictBar } from "./Conflict";
import { DiaryHeader, DiaryText } from "./Diary";
import { WindowControls } from "./Chrome";
import { DueChip } from "./Tasks";
import { contextMenu, openMenu, type MenuItem } from "./ContextMenu";
import { FindBar, MobileFormatBar, SelectionBubble, SourceEditor, StatusBar, TableTools } from "./EditorParts";
import { isMobile } from "../lib/mobile";
import { goBack, goForward, useNavState } from "../lib/nav";
import { ChapterBar, CharacterSheet, PlotBoard } from "./Work";
import { BoardSkeleton, ChapterBarSkeleton } from "./Skeleton";
import { enterPage } from "../lib/motion";
import { noteOpened, roleLabel, useNoteWork, workIdOf } from "../lib/works";
import { refreshCharacterNames, type CharacterName } from "../editor/characters";

type SaveState = "saved" | "dirty" | "saving" | "error";

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

export function EditorPane() {
  const t = useT();
  const selectedId = useStore((s) => s.selectedId);
  const noteVersion = useStore((s) => s.noteVersion);
  const prefs = useStore((s) => s.prefs);
  const panel = useStore((s) => s.panel);
  const layout = useStore((s) => s.layout);
  useStore((s) => s.folders); // re-render the breadcrumb when notebooks change

  const [note, setNote] = useState<Note | null>(null);
  const [title, setTitle] = useState("");
  const [source, setSource] = useState<string | null>(null);
  const [lossy, setLossy] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const [counts, setCounts] = useState({ chars: 0, withSpaces: 0, selection: 0, paragraphs: [] as string[] });
  const [findOpen, setFindOpen] = useState<false | "find" | "replace">(false);
  const [comments, setComments] = useState<NoteComment[]>([]);
  const activeComment = useStore((s) => s.activeCommentId);
  const commentsVersion = useStore((s) => s.commentsVersion);

  const prefsRef = useRef(prefs);
  prefsRef.current = prefs;
  const noteRef = useRef<Note | null>(null);
  const titleRef = useRef("");
  const sourceRef = useRef<string | null>(null);
  const lastSaved = useRef<{ title: string; body: string } | null>(null);
  const loading = useRef(false);
  const saveTimer = useRef<number | undefined>(undefined);
  const scrollRef = useRef<HTMLDivElement>(null);
  const paperRef = useRef<HTMLDivElement>(null);
  /** The note whose page came in last: reloading the same note does not replay it. */
  const enteredId = useRef<string | null>(null);
  const titleEl = useRef<HTMLTextAreaElement>(null);
  const editorRef = useRef<Editor | null>(null);
  const linesRef = useRef<TextLinesHandle | null>(null);

  const [diary, setDiary] = useState(false);
  const diaryRef = useRef(false);
  diaryRef.current = diary;
  const [locked, setLocked] = useState(false);
  const lockedRef = useRef(false);
  lockedRef.current = locked;
  const readOnly = !!note && (note.deletedTime > 0 || locked);

  // The work the note belongs to: its characters' names are highlighted.
  const work = useNoteWork(note?.parentId);
  useStore((s) => s.folders);
  const inWork = !!note && !!workIdOf(note.parentId);
  const namesRef = useRef<CharacterName[]>([]);
  namesRef.current = useMemo(
    () =>
      (work?.characters ?? []).flatMap((c) => {
        const label = [c.name, c.ruby && `（${c.ruby}）`, c.role && ` — ${roleLabel(c.role)}`].filter(Boolean).join("");
        return [c.name, ...c.aliases].filter((n) => n.trim()).map((name) => ({ id: c.id, name, color: c.color || "", label }));
      }),
    [work],
  );

  // ---------------------------------------------------------------------
  // Saving

  const currentBody = useCallback((): string => {
    if (sourceRef.current !== null) return sourceRef.current;
    return editorRef.current ? editorMarkdown(editorRef.current) : "";
  }, []);

  /** The title and body (as stored, encrypted if locked) the edits started from. */
  const baseRef = useRef<{ title: string; body: string } | null>(null);
  /** The save running, so that saves happen one after another. */
  const saving = useRef<Promise<void>>(Promise.resolve());

  const flushNow = useCallback(async () => {
    window.clearTimeout(saveTimer.current);
    const n = noteRef.current;
    if (!n || n.deletedTime > 0) return;
    if (lockedRef.current && !passwords.has(n.id)) return; // still locked: nothing was edited
    const plain = currentBody();
    const title = titleRef.current;
    if (lastSaved.current && lastSaved.current.title === title && lastSaved.current.body === plain) {
      if (noteRef.current?.id === n.id) setSaveState("saved");
      return;
    }
    setSaveState("saving");
    try {
      const password = passwords.get(n.id);
      const body = password ? await lockBody(plain, password, t("lock.notice")) : plain;
      const summary = await api.saveNote(n.id, title, body, baseRef.current ?? undefined);
      lastSaved.current = { title, body: plain };
      if (noteRef.current?.id === n.id) {
        baseRef.current = { title, body };
        noteRef.current = { ...noteRef.current, updatedTime: summary.updatedTime };
        // Typing during the save left it dirty, with its own save scheduled.
        const current = currentBody();
        setSaveState(titleRef.current === title && current === plain ? "saved" : "dirty");
      }
      useStore.getState().updateSummary(summary);
      if (summary.conflictId) {
        void useStore.getState().refresh();
        useStore.getState().toast(t("editor.keptConflict"), {
          label: t("editor.showConflict"),
          run: () => void useStore.getState().setScope({ kind: "conflicts" }),
        });
      }
      void useStore.getState().loadStats();
      if (prefsRef.current.syncAfterEdit) requestSync();
    } catch (e) {
      setSaveState("error");
      report(e);
    }
  }, [currentBody]);

  const flush = useCallback(() => {
    const next = saving.current.then(flushNow, flushNow);
    saving.current = next.catch(() => {});
    return next;
  }, [flushNow]);

  useEffect(() => registerFlush(flush), [flush]);

  const scheduleSave = useCallback(() => {
    if (loading.current) return;
    setSaveState("dirty");
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => void flush(), 700);
  }, [flush]);

  // ---------------------------------------------------------------------
  // Counting and typewriter scrolling

  const countTimer = useRef<number | undefined>(undefined);
  const updateCounts = useCallback(() => {
    window.clearTimeout(countTimer.current);
    countTimer.current = window.setTimeout(() => {
      const ed = editorRef.current;
      let paragraphs: string[];
      if (sourceRef.current !== null) {
        // Diary pages are plain text, other notes shown as Markdown.
        paragraphs = (diaryRef.current ? sourceRef.current : markdownToPlain(sourceRef.current)).split("\n");
      } else if (ed) {
        paragraphs = [];
        ed.state.doc.descendants((node) => {
          if (node.isTextblock) {
            paragraphs.push(node.textContent);
            return false;
          }
          return true;
        });
      } else paragraphs = [];
      const text = paragraphs.join("\n");
      let selection = 0;
      if (ed && sourceRef.current === null && !ed.state.selection.empty) {
        const { from, to } = ed.state.selection;
        const selected = ed.state.doc.textBetween(from, to, "\n");
        selection = prefsRef.current.countSpaces ? countWithSpaces(selected) : countChars(selected);
      }
      setCounts({ chars: countChars(text), withSpaces: countWithSpaces(text), selection, paragraphs });
    }, 120);
  }, []);

  const typewriter = useCallback(() => {
    const ed = editorRef.current;
    const scroller = scrollRef.current;
    if (!ed || !scroller || !prefsRef.current.typewriter || !ed.isFocused) return;
    const coords = ed.view.coordsAtPos(ed.state.selection.head);
    const box = scroller.getBoundingClientRect();
    scroller.scrollTop += coords.top - (box.top + box.height * 0.42);
  }, []);

  // ---------------------------------------------------------------------
  // The editor

  const editor = useEditor({
    extensions: editorExtensions({
      placeholder: (heading) => (heading ? t("editor.headingPlaceholder") : t("editor.placeholder")),
      smartPunctuation: () => prefsRef.current.smartPunctuation,
      slash: () => prefsRef.current.slashMenu,
      lineNumbers: () => prefsRef.current.lineNumbers,
      autoPair: () => prefsRef.current.autoPair,
      onActivateComment: (id) => useStore.getState().set({ activeCommentId: id, panel: "comments" }),
      onEditMath: (kind, latex, pos) => {
        if (editorRef.current?.isEditable) void editMath(editorRef.current, kind, latex, pos);
      },
      characterNames: () => namesRef.current,
      highlightCharacters: () => prefsRef.current.highlightCharacters,
      onOpenCharacter: (id) => useStore.getState().select(id),
    }),
    editorProps: {
      // (Phones draw the empty note's placeholder with CSS: see editor/setup.ts.)
      attributes: { class: "prose", spellcheck: "false", style: `--empty-placeholder: ${JSON.stringify(t("editor.placeholder"))}` },
      handlePaste: (_view, event) => {
        const ed = editorRef.current;
        if (!ed) return false;
        const files = event.clipboardData?.files;
        if (files && files.length) {
          void insertFiles(ed, files);
          return true;
        }
        // Cells copied from a spreadsheet (without a formatted table) become a table.
        const html = event.clipboardData?.getData("text/html") ?? "";
        const text = event.clipboardData?.getData("text/plain") ?? "";
        if (!html.includes("<table") && !ed.isActive("table")) {
          const table = tsvToTable(text);
          if (table) {
            ed.chain().focus().insertContent(table).run();
            return true;
          }
        }
        return false;
      },
      handleDrop: (view, event) => {
        const files = (event as DragEvent).dataTransfer?.files;
        if (!files || !files.length || !editorRef.current) return false;
        const at = view.posAtCoords({ left: (event as DragEvent).clientX, top: (event as DragEvent).clientY });
        void insertFiles(editorRef.current, files, at?.pos);
        return true;
      },
      handleClickOn: () => false,
      handleDOMEvents: {
        // Links to other notes open them.
        click: (_view, event) => {
          const a = (event.target as HTMLElement).closest?.("a");
          const href = a?.getAttribute("href") ?? "";
          const m = /^:\/([0-9a-f]{32})/.exec(href);
          if (!m) return false;
          event.preventDefault();
          if (isAttachment(m[1])) void openAttachment(m[1]);
          else void openLinkedNote(m[1]);
          return true;
        },
      },
    },
    onUpdate: () => {
      scheduleSave();
      updateCounts();
      requestAnimationFrame(typewriter);
    },
    onSelectionUpdate: () => {
      updateCounts();
      typewriter();
    },
  });
  editorRef.current = editor;

  useEffect(() => {
    editor?.setEditable(!readOnly);
  }, [editor, readOnly]);
  useEffect(() => {
    editor?.view.dom.setAttribute("spellcheck", String(prefs.spellcheck));
  }, [editor, prefs.spellcheck]);
  // The margin numbers follow the setting and anything that moves the text.
  useEffect(() => {
    refreshLineNumbers(editor);
  }, [editor, prefs.lineNumbers, prefs.fontSize, prefs.lineHeight, prefs.textWidth, prefs.fullWidth, prefs.editorFont, prefs.indentParagraphs, panel, source]);

  // Load the selected note (saving the previous one first).
  useEffect(() => {
    let cancelled = false;
    (async () => {
      await flush();
      if (!selectedId || !editor) {
        noteRef.current = null;
        setNote(null);
        return;
      }
      let n: Note;
      try {
        await resourcesReady();
        n = await api.getNote(selectedId);
      } catch (e) {
        report(e);
        return;
      }
      if (cancelled) return;
      noteOpened(n.parentId);
      useStore.getState().set({ openKind: n.kind });
      // A plot, character or material page is not manuscript: no preview of it.
      if (n.kind && useStore.getState().panel === "preview") useStore.getState().set({ panel: "none" });
      loading.current = true;
      noteRef.current = n;
      titleRef.current = n.title;
      setNote(n);
      setTitle(n.title);
      let body = n.body;
      const isLock = isLocked(body);
      const password = passwords.get(n.id);
      let stillLocked = isLock && !password;
      if (isLock && password) {
        try {
          body = await unlockBody(n.body, password);
        } catch {
          passwords.delete(n.id);
          stillLocked = true;
        }
      }
      setLocked(stillLocked);
      lockedRef.current = stillLocked;
      if (stillLocked) body = "";
      lastSaved.current = { title: n.title, body };
      baseRef.current = { title: n.title, body: n.body };
      editor.commands.setContent(body, { contentType: "markdown", emitUpdate: false });
      // Content the editor can't represent (raw HTML, unusual Markdown) is
      // edited as Markdown, so that it is not altered.
      // Diary entries are written as plain text, without Markdown.
      const isDiary = !!n.diaryDate && !!useStore.getState().folders.find((f) => f.id === n.parentId)?.isDiary;
      setDiary(isDiary);
      diaryRef.current = isDiary;
      const isLossy = !isDiary && body.trim() !== "" && normalizeMarkdown(editorMarkdown(editor)) !== normalizeMarkdown(body);
      setLossy(isLossy);
      sourceRef.current = isLossy || isDiary ? body : null;
      setSource(sourceRef.current);
      // Opened from a search: its first word is marked in the text.
      const scope = useStore.getState().scope;
      const word = scope.kind === "search" ? scope.query.split(/\s+/).find((w) => w && !/^(tag|in|タグ|場所):/i.test(w)) : undefined;
      editor.commands.setSearch(word ?? "");
      loading.current = false;
      setSaveState("saved");
      updateCounts();
      scrollRef.current?.scrollTo({ top: 0 });
      if (enteredId.current !== n.id) {
        enteredId.current = n.id;
        // (On phones the whole screen slides in already.)
        if (useStore.getState().layout !== "narrow") requestAnimationFrame(() => enterPage(paperRef.current));
      }
      if (!n.title && !n.body && n.deletedTime === 0) requestAnimationFrame(() => titleEl.current?.focus());
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedId, noteVersion, editor]);

  // A sync changed notes: take the new version of the open note, unless it
  // is being edited (then ask, so that typing is never lost).
  const remoteVersion = useStore((s) => s.remoteVersion);
  const saveStateRef = useRef<SaveState>("saved");
  saveStateRef.current = saveState;
  /**
   * Whether the page holds something not saved yet: not only by the save
   * state, which lags a keystroke behind, but by the text itself, and while
   * a phone keyboard is still composing a word (not in the document yet).
   * Replacing the page then would lose that text.
   */
  const hasUnsaved = () => {
    if (saveStateRef.current !== "saved") return true;
    if (editorRef.current?.view.composing) return true;
    const saved = lastSaved.current;
    if (!saved || !noteRef.current || lockedRef.current) return false;
    return titleRef.current !== saved.title || currentBody() !== saved.body;
  };
  useEffect(() => {
    const n = noteRef.current;
    if (remoteVersion === 0 || !n) return;
    api.getNote(n.id).then(
      async (fresh) => {
        if (fresh.updatedTime === n.updatedTime || noteRef.current?.id !== n.id) return;
        const password = passwords.get(n.id);
        const freshBody = password && isLocked(fresh.body) ? await unlockBody(fresh.body, password).catch(() => fresh.body) : fresh.body;
        if (fresh.title === lastSaved.current?.title && freshBody === lastSaved.current?.body) {
          noteRef.current = { ...n, updatedTime: fresh.updatedTime };
          baseRef.current = { title: fresh.title, body: fresh.body };
          return;
        }
        if (!hasUnsaved()) useStore.getState().reloadNote();
        else
          useStore.getState().toast(t("editor.updatedElsewhere"), {
            label: t("editor.loadLatest"),
            run: () => useStore.getState().reloadNote(),
          });
      },
      async () => {
        // Deleted elsewhere. What was being written here is kept as a new note.
        if (noteRef.current?.id !== n.id) return;
        if (hasUnsaved()) {
          window.clearTimeout(saveTimer.current);
          const kept = await api.createNote(undefined, titleRef.current, currentBody()).catch(report);
          noteRef.current = null;
          if (kept) {
            await useStore.getState().refresh();
            useStore.getState().select(kept.id);
            useStore.getState().toast(t("editor.keptDeleted"));
            return;
          }
        }
        useStore.getState().select(null);
      },
    );
  }, [remoteVersion]);

  // Notes linking to this one.
  const [backlinks, setBacklinks] = useState<LinkedNote[]>([]);
  useEffect(() => {
    if (!note) return;
    notesApi.listBacklinks(note.id).then(setBacklinks, () => setBacklinks([]));
  }, [note?.id, noteVersion]);
  const summary = useStore((s) => s.notes.find((x) => x.id === note?.id));

  // Comments of the open note, highlighted in the text.
  const loadComments = useCallback(() => {
    const id = noteRef.current?.id;
    if (!id) return;
    api.listComments(id).then((list) => {
      if (noteRef.current?.id === id) setComments(list);
    }, report);
  }, []);
  useEffect(() => {
    setComments([]);
    loadComments();
  }, [note?.id, commentsVersion, loadComments]);
  useEffect(() => {
    editor?.commands.setComments(
      comments.map((c) => ({ id: c.id, quote: c.quote, before: c.before, resolved: c.resolved })),
      activeComment,
    );
  }, [editor, comments, activeComment]);

  const addComment = async () => {
    if (!editor || !note) return;
    const anchor = selectionAnchor(editor);
    if (!anchor) {
      useStore.getState().toast(t("comments.selectFirst"));
      return;
    }
    const text = await dialogs.prompt(t("comments.prompt"), { message: `「${anchor.quote.slice(0, 60)}${anchor.quote.length > 60 ? "…" : ""}」`, multiline: true });
    if (!text) return;
    const c = await api.addComment(note.id, anchor.quote, anchor.before, text).catch(report);
    if (!c) return;
    useStore.setState((s) => ({ commentsVersion: s.commentsVersion + 1, activeCommentId: c.id, panel: "comments" }));
  };
  const addCommentRef = useRef(addComment);
  addCommentRef.current = addComment;

  /** Asks for a number and jumps there: a paragraph (rich text) or a line (plain text). */
  const goToLine = async () => {
    const ed = editorRef.current;
    const plain = sourceRef.current;
    if (!noteRef.current || lockedRef.current || (plain === null && !ed)) return;
    let total: number;
    let current: number;
    if (plain !== null) {
      total = plain.split(/\n/).length;
      const el = linesRef.current?.textarea;
      current = el ? plain.slice(0, el.selectionStart).split(/\n/).length : 1;
    } else {
      total = unitCount(ed!.state.doc);
      current = numberAt(ed!.state.doc, ed!.state.selection.head);
    }
    if (total === 0) return;
    const v = await dialogs.prompt(t(plain !== null ? "goto.line" : "goto.paragraph"), {
      initial: String(current),
      message: t("goto.range", { n: formatNumber(total) }),
    });
    if (v === null) return;
    const n = parseInt(v.normalize("NFKC").replace(/[,\s]/g, ""), 10);
    if (!Number.isFinite(n)) return;
    if (plain !== null) linesRef.current?.goToLine(n);
    else if (ed) goToNumber(ed, n);
    if (!prefsRef.current.lineNumbers) {
      useStore.getState().toast(t("goto.showNumbers"), { label: t("goto.turnOn"), run: () => useStore.getState().setPrefs({ lineNumbers: true }) });
    }
  };
  const goToLineRef = useRef(goToLine);
  goToLineRef.current = goToLine;
  useEffect(() => {
    const onGoto = () => void goToLineRef.current();
    window.addEventListener("nisshoku:goto-line", onGoto);
    return () => window.removeEventListener("nisshoku:goto-line", onGoto);
  }, []);

  // Redraw the characters' names when the cast or the preference changes.
  useEffect(() => {
    refreshCharacterNames(editor);
  }, [editor, work?.characters, prefs.highlightCharacters]);

  // Save when the window is hidden or closed.
  useEffect(() => {
    const onHide = () => void flush();
    const onVisibility = () => document.visibilityState === "hidden" && onHide();
    window.addEventListener("blur", onHide);
    window.addEventListener("beforeunload", onHide);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("blur", onHide);
      window.removeEventListener("beforeunload", onHide);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [flush]);

  // Title height follows its content (and the width it has).
  useEffect(() => {
    const el = titleEl.current;
    if (!el) return;
    const fit = () => {
      el.style.height = "auto";
      el.style.height = `${el.scrollHeight}px`;
    };
    fit();
    // Only a new width can change the height it needs; refitting on height
    // changes (which the fit itself makes) loops.
    const parent = el.parentElement ?? el;
    let width = parent.clientWidth;
    const observer = new ResizeObserver(() => {
      if (parent.clientWidth === width) return;
      width = parent.clientWidth;
      fit();
    });
    observer.observe(parent);
    return () => observer.disconnect();
  }, [title, note, prefs.fontSize]);

  // Shortcuts of the editor.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (mod && key === "f" && !e.shiftKey) {
        e.preventDefault();
        setFindOpen("find");
      } else if (mod && key === "h") {
        e.preventDefault();
        setFindOpen("replace");
      } else if (mod && key === "s") {
        e.preventDefault();
        void flush();
      } else if (mod && key === "g" && !e.shiftKey) {
        e.preventDefault();
        void goToLineRef.current();
      } else if (mod && e.altKey && key === "m") {
        e.preventDefault();
        void addCommentRef.current();
      } else if (mod && key === "k" && e.shiftKey && editorRef.current) {
        e.preventDefault();
        void setLink(editorRef.current);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [flush]);

  const toggleSource = () => {
    if (!editor) return;
    if (sourceRef.current === null) {
      sourceRef.current = editorMarkdown(editor);
    } else {
      loading.current = true;
      editor.commands.setContent(sourceRef.current, { contentType: "markdown", emitUpdate: false });
      loading.current = false;
      sourceRef.current = null;
      setLossy(false);
    }
    setSource(sourceRef.current);
    updateCounts();
  };

  // ---------------------------------------------------------------------
  // Actions

  const exportNote = async (format: "md" | "txt" | "html") => {
    if (!note) return;
    await flush();
    const body = currentBody();
    const name = (titleRef.current || t("list.untitled")).replace(/[\\/:*?"<>|]/g, "_");
    const path = await saveDialog({
      defaultPath: `${name}.${format}`,
      filters: [{ name: { md: "Markdown", txt: "Text", html: "HTML" }[format], extensions: [format] }],
    });
    if (!path) return;
    let contents: string;
    if (format === "md") contents = `# ${titleRef.current}\n\n${body}\n`;
    else if (format === "txt") contents = `${titleRef.current}\n\n${markdownToPlain(body)}\n`;
    else {
      const html = (editor?.getHTML() ?? "").replace(/>([^<]+)</g, (_, s: string) => `>${rubyToHtml(s.replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">"))}<`);
      contents = `<!doctype html><html lang="${prefs.language}"><head><meta charset="utf-8"><title>${escapeHtml(titleRef.current)}</title><link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.18.10/dist/katex.min.css"><style>body{max-width:40em;margin:3em auto;padding:0 1em;font-family:"Yu Mincho","Hiragino Mincho ProN",serif;line-height:1.9;color:#2c2a26;background:#faf9f5}h1,h2,h3{line-height:1.5}blockquote{border-left:3px solid #e9b89c;margin-left:0;padding-left:1em;color:#69645b}table{border-collapse:collapse}td,th{border:1px solid #d6d0c3;padding:.3em .6em}hr{border:none;text-align:center}hr::after{content:"＊　＊　＊"}</style></head><body><h1>${escapeHtml(titleRef.current)}</h1>${html}</body></html>`;
    }
    await api.writeTextFile(path, contents).catch(report);
    useStore.getState().toast(t("toast.exported"));
  };

  const setGoal = async () => {
    if (!note) return;
    const v = await dialogs.prompt(t("goal.title"), {
      initial: note.goal ? String(note.goal) : "",
      placeholder: t("goal.placeholder"),
      message: t("goal.message"),
    });
    if (v === null) return;
    const goal = parseInt(v.normalize("NFKC").replace(/[,\s]/g, ""), 10);
    await api.setNoteGoal(note.id, Number.isFinite(goal) && goal > 0 ? goal : null).catch(report);
    setNote({ ...note, goal: Number.isFinite(goal) && goal > 0 ? goal : null });
    void useStore.getState().loadNotes();
  };

  const moreMenu = (): MenuItem[] => {
    if (!note) return [];
    const s = useStore.getState();
    // Also when the editor is too narrow for the panel buttons (CSS hides them).
    const panelButtons = document.querySelector<HTMLElement>(".editor-top .panel-btn");
    const narrow = layout === "narrow" || (!!panelButtons && panelButtons.offsetParent === null);
    const panelItem = (id: "outline" | "preview" | "history" | "comments" | "proof" | "reference", icon: string, label: string): MenuItem => ({
      label,
      icon,
      checked: panel === id,
      run: () => s.set({ panel: panel === id ? "none" : id }),
    });
    return [
      ...(narrow
        ? ([
            panelItem("outline", "outline", t("editor.outline")),
            panelItem("comments", "comment", t("comments.title")),
            panelItem("history", "history", t("editor.history")),
            ...(note.kind ? [] : [panelItem("preview", "vertical", t("editor.preview"))]),
            panelItem("proof", "proof", t("editor.proof")),
            panelItem("reference", "memo", t("reference.title")),
            "sep",
          ] as MenuItem[])
        : []),
      { label: summary?.pinned ? t("menu.unpin") : t("menu.pin"), icon: "pin", run: () => togglePin({ id: note.id, pinned: !!summary?.pinned }) },
      { label: note.isTodo ? t("menu.untodo") : t("menu.todo"), icon: "todo", run: () => toggleTodo(note) },
      { label: t("menu.tags"), icon: "tag", run: () => editTags(note.id, summary?.tags ?? []) },
      "sep",
      { label: t("menu.goal"), icon: "target", run: setGoal },
      { label: t("menu.snapshot"), icon: "save", run: async () => {
          await flush();
          const saved = await api.saveSnapshot(note.id).catch(report);
          s.toast(saved ? t("toast.snapshotSaved") : t("toast.snapshotSame"));
        } },
      ...(diary ? [] : [{ label: source === null ? t("menu.markdown") : t("menu.rich"), icon: "type", run: toggleSource } as MenuItem]),
      { label: t("menu.findReplace"), icon: "replace", hint: "Ctrl H", run: () => setFindOpen("replace") },
      { label: source !== null ? t("goto.line") : t("goto.paragraph"), icon: "lines", hint: "Ctrl G", run: () => void goToLine() },
      { label: t("menu.lineNumbers"), icon: "lines", checked: prefs.lineNumbers, run: () => useStore.getState().setPrefs({ lineNumbers: !prefs.lineNumbers }) },
      "sep",
      { label: t("menu.move"), icon: "move", submenu: moveSubmenu((f) => f.id === note.parentId, (id) => {
          setNote({ ...note, parentId: id });
          void moveNote(note.id, id);
        }) },
      { label: t("menu.duplicate"), icon: "copy", run: () => duplicateNote(note.id) },
      { label: t("menu.saveTemplate"), icon: "template", run: () => saveTemplate(note.id) },
      { label: t("menu.copyText"), icon: "copy", run: async () => {
          await writeText(markdownToPlain(currentBody()));
          s.toast(t("toast.copied"));
        } },
      {
        label: t("menu.export"),
        icon: "download",
        submenu: [
          { label: "Markdown (.md)", icon: "download", run: () => exportNote("md") },
          { label: t("menu.exportTextShort"), icon: "download", run: () => exportNote("txt") },
          { label: "HTML (.html)", icon: "download", run: () => exportNote("html") },
        ],
      },
      passwords.has(note.id)
        ? { label: t("lock.unlockNow"), icon: "lock", run: async () => {
              passwords.delete(note.id);
              lastSaved.current = null;
              await flush();
              s.toast(t("lock.removed"));
            } }
        : { label: t("lock.lock"), icon: "lock", disabled: locked, run: async () => {
              const pw = await dialogs.prompt(t("lock.prompt"), { message: t("lock.promptDetail"), password: true });
              if (!pw) return;
              const again = await dialogs.prompt(t("lock.confirm"), { password: true });
              if (again !== pw) {
                s.toast(t("lock.mismatch"));
                return;
              }
              passwords.set(note.id, pw);
              lastSaved.current = null;
              await flush();
              // The history holds the text from before the lock: remove it
              // (here and, through the sync, on the target).
              await api.clearHistory(note.id).catch(report);
              s.toast(t("lock.done"));
            } },
      ...(passwords.has(note.id)
        ? [{ label: t("lock.relock"), icon: "lock", run: async () => {
              await flush();
              passwords.delete(note.id);
              s.reloadNote();
            } } as MenuItem]
        : []),
      { label: t("menu.print"), icon: "print", run: async () => {
          await flush();
          window.print();
        } },
      "sep",
      { label: t("menu.trash"), icon: "trash", danger: true, run: () => trashNote(note.id) },
    ];
  };

  // ---------------------------------------------------------------------
  // Rendering

  const crumbs = note ? folderTrail(note.parentId) : [];
  const ms = useMemo(
    () => manuscript(counts.paragraphs.filter((p) => p.trim() !== ""), prefs.manuscriptCols, prefs.manuscriptRows),
    [counts.paragraphs, prefs.manuscriptCols, prefs.manuscriptRows],
  );
  const style = {
    ["--editor-font" as string]: prefs.editorFont === "serif" ? "var(--font-serif)" : "var(--font-sans)",
    ["--editor-size" as string]: `${prefs.fontSize}px`,
    ["--editor-line" as string]: prefs.lineHeight,
    ["--text-width" as string]: prefs.fullWidth ? 1000 : prefs.textWidth,
  };

  if (!selectedId || !note) {
    return (
      <div className="main" style={style}>
        <div className="editor-top" data-tauri-drag-region>
          <PaneToggles />
          <div className="crumb" data-tauri-drag-region />
          <WindowControls />
        </div>
        <div className="empty editor-empty">
          <div className="empty-mark" />
          <div className="empty-title">{t("appName")}</div>
          {t("editor.empty")}
          <br />
          <button className="btn primary" onClick={() => void useStore.getState().newNote()}>
            <Icon name="plus" />
            {t("editor.newNote")}
          </button>
        </div>
      </div>
    );
  }

  const panelButton = (id: "outline" | "preview" | "history" | "comments" | "proof" | "reference", icon: string, label: string, badge = false) => (
    <button className={`icon-btn hide-narrow panel-btn${panel === id ? " active" : ""}`} title={label} onClick={() => useStore.getState().set({ panel: panel === id ? "none" : id })}>
      <Icon name={icon} />
      {badge && <span className="dot" />}
    </button>
  );

  return (
    <div className="main" style={style}>
      <div className="editor-top" data-tauri-drag-region>
        <PaneToggles />
        <NavButtons />
        <div className="crumb" data-tauri-drag-region>
          {crumbs.map((f, i) => (
            <span key={f.id} className="crumb-part">
              {i > 0 && <span className="crumb-sep">/</span>}
              {/* Each part is a menu of its notebook (lib/actions.ts crumbMenu). */}
              <button
                aria-haspopup="menu"
                title={f.title}
                onClick={async (e) => {
                  const at = e.currentTarget;
                  const items = await crumbMenu(f, note.id);
                  if (at.isConnected) openMenu(at, items, { asSelect: true });
                }}
              >
                {f.icon && <span className="crumb-emoji">{f.icon}</span>}
                <span className="crumb-text">{f.title}</span>
                <Icon name="chevronDown" className="crumb-caret" />
              </button>
            </span>
          ))}
        </div>
        {/* Nothing is saved from a read-only note (in the trash, a conflict copy). */}
        {!readOnly && (
          <span className={`save-state ${saveState}`}>
            {{ saved: t("editor.saved"), dirty: t("editor.editing"), saving: t("editor.saving"), error: t("editor.saveFailed") }[saveState]}
          </span>
        )}
        {!diary && panelButton("outline", "outline", t("editor.outline"))}
        {!diary && panelButton("comments", "comment", t("comments.title"), comments.some((c) => !c.resolved))}
        {panelButton("history", "history", t("editor.history"))}
        {!note.kind && panelButton("preview", "vertical", t("editor.preview"))}
        {!diary && panelButton("proof", "proof", t("editor.proof"))}
        {panelButton("reference", "memo", t("reference.title"))}
        <ViewButton />
        <button className="icon-btn" title={t("editor.more")} onClick={(e) => openMenu(e.currentTarget, moreMenu())}>
          <Icon name="more" />
        </button>
        <WindowControls />
      </div>

      <div className="editor-area">
        <div className="editor-scroll" ref={scrollRef}>
          <div ref={paperRef} className={`paper${prefs.typewriter ? " typewriter" : ""}${prefs.lineNumbers ? " with-lines" : ""}${note.kind === "plot" ? " board" : ""}`}>
            {note.deletedTime > 0 && (
              <div className="notice">
                <Icon name="trash" />
                <span className="grow">{t("editor.inTrash")}</span>
                <button
                  className="btn"
                  onClick={async () => {
                    await api.restoreNote(note.id).catch(report);
                    await useStore.getState().refresh();
                    useStore.getState().reloadNote();
                  }}
                >
                  {t("editor.restore")}
                </button>
              </div>
            )}
            {note.isConflict && <ConflictBar note={note} onDone={(id) => useStore.getState().select(id)} />}
            {lossy && source !== null && (
              <div className="notice">
                <Icon name="alert" />
                <span className="grow">{t("editor.lossyNotice")}</span>
                <button className="btn" onClick={toggleSource}>
                  {t("editor.showRich")}
                </button>
              </div>
            )}
            {diary && note.diaryDate && <DiaryHeader noteId={note.id} date={note.diaryDate} readOnly={readOnly} />}
            {!diary && note.isTodo && (
              <label className="todo-title">
                <input
                  type="checkbox"
                  checked={note.todoCompleted}
                  disabled={readOnly}
                  onChange={async (e) => {
                    await api.setTodoCompleted(note.id, e.target.checked).catch(report);
                    setNote({ ...note, todoCompleted: e.target.checked });
                    void useStore.getState().loadNotes();
                  }}
                />
                {t("menu.todoDone")}
                {!note.todoCompleted && !readOnly && (
                  <DueChip
                    task={{ due: note.todoDue, allDay: note.dueAllDay }}
                    placeholder
                    onChange={(due) => {
                      // Not setTaskDue: reloading the note would drop unsaved typing.
                      setNote({ ...note, todoDue: due.due, dueAllDay: due.allDay });
                      void api.setTodoDue(note.id, due.due, due.allDay).then(() => useStore.getState().loadNotes(), report);
                    }}
                  />
                )}
              </label>
            )}
            <textarea
              ref={titleEl}
              className={`note-title${note.todoCompleted ? " done" : ""}${diary ? " gone" : ""}`}
              rows={1}
              value={title}
              placeholder={t("editor.titlePlaceholder")}
              readOnly={readOnly}
              onChange={(e) => {
                setTitle(e.target.value);
                titleRef.current = e.target.value;
                scheduleSave();
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  if (source !== null) document.querySelector<HTMLTextAreaElement>(".source-editor")?.focus();
                  else editor?.chain().focus("start").run();
                }
              }}
              {...contextMenu(() => inputMenu(titleEl.current!))}
            />
            <div className={`note-meta${diary ? " gone" : ""}`}>
              <span>{longDate(note.createdTime)}</span>
              {note.goal ? (
                <button onClick={setGoal}>{t("editor.goalSet", { n: formatNumber(note.goal) })}</button>
              ) : (
                <button onClick={setGoal}>{t("editor.goalAdd")}</button>
              )}
              {source !== null && !lossy && <button onClick={toggleSource}>{t("editor.markdownMode")}</button>}
              {summary?.pinned && <span className="meta-pin"><Icon name="pin" /></span>}
            </div>
            <div className={`note-tags${diary ? " gone" : ""}`}>
              {(summary?.tags ?? []).map((tag) => (
                <button key={tag} className="tag-chip" onClick={() => {
                  const found = useStore.getState().tags.find((x) => x.title === tag);
                  if (found) void useStore.getState().setScope({ kind: "tag", id: found.id });
                }}>
                  #{tag}
                </button>
              ))}
              {!readOnly && (
                <button className="tag-add" onClick={() => editTags(note.id, summary?.tags ?? [])}>
                  {t("tags.add")}
                </button>
              )}
            </div>
            {!diary && !locked && note.kind === "character" && <CharacterSheet note={note} work={work} readOnly={readOnly} />}
            {!diary && !locked && !note.kind && work && <ChapterBar note={note} work={work} readOnly={readOnly} />}
            {/* In a work whose overview is still loading: its place, not the plain layout. */}
            {!diary && !locked && !note.kind && !work && inWork && <ChapterBarSkeleton />}
            {locked ? (
              <LockScreen
                onUnlock={async (pw) => {
                  try {
                    await unlockBody(note.body, pw);
                  } catch {
                    return false;
                  }
                  passwords.set(note.id, pw);
                  useStore.getState().reloadNote();
                  return true;
                }}
              />
            ) : note.kind === "plot" ? (
              work ? <PlotBoard note={note} work={work} readOnly={readOnly} /> : <BoardSkeleton />
            ) : diary && source !== null ? (
              <DiaryText
                lines={linesRef}
                value={source}
                readOnly={readOnly}
                date={note.diaryDate ?? ""}
                onChange={(v) => {
                  sourceRef.current = v;
                  setSource(v);
                  scheduleSave();
                  updateCounts();
                }}
              />
            ) : source !== null ? (
              <SourceEditor
                lines={linesRef}
                value={source}
                readOnly={readOnly}
                onChange={(v) => {
                  sourceRef.current = v;
                  setSource(v);
                  scheduleSave();
                  updateCounts();
                }}
              />
            ) : (
              <EditorContent
                editor={editor}
                className={`${prefs.indentParagraphs ? "indent-wrap" : ""}${prefs.dimOthers ? " dim-wrap" : ""}`}
                {...contextMenu(() => (editor ? editorMenu(editor, addComment) : []))}
              />
            )}
            {backlinks.length > 0 && (
              <div className="backlinks">
                <div className="backlinks-title">
                  <Icon name="linkNote" />
                  {t("links.title")}
                </div>
                {backlinks.map((b) => (
                  <button key={b.id} onClick={() => void openLinkedNote(b.id)}>
                    {b.title || t("list.untitled")}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        {panel !== "none" && layout === "narrow" && <div className="panel-backdrop" onClick={() => useStore.getState().set({ panel: "none" })} />}
        {panel === "outline" && source === null && !diary && <OutlinePanel editor={editor} />}
        {panel === "history" && <HistoryPanel noteId={note.id} currentBody={currentBody} onBeforeRestore={flush} />}
        {panel === "preview" && !note.kind && <PreviewPanel title={title} paragraphs={counts.paragraphs} editor={source === null ? editor : null} />}
        {panel === "proof" && source === null && <ProofPanel editor={editor} />}
        {panel === "reference" && <ReferencePanel currentId={note.id} />}
        {panel === "comments" && (
          <CommentsPanel
            editor={source === null ? editor : null}
            noteId={note.id}
            comments={comments}
            onChange={() => useStore.setState((s) => ({ commentsVersion: s.commentsVersion + 1 }))}
          />
        )}
      </div>

      <StatusBar editor={source === null && !readOnly ? editor : null} chars={counts.chars} withSpaces={counts.withSpaces} selection={counts.selection} pages={note.kind ? null : ms.pages} lines={ms.lines} goal={note.goal} />

      {isMobile && editor && source === null && !readOnly && <MobileFormatBar editor={editor} />}
      {findOpen && editor && source === null && <FindBar editor={editor} mode={findOpen} onClose={() => setFindOpen(false)} />}
      <SlashMenu />
      {editor && source === null && !readOnly && prefs.bubbleToolbar && <SelectionBubble editor={editor} onComment={addComment} onLink={() => void setLink(editor)} />}
      {editor && source === null && !readOnly && <TableTools editor={editor} scroller={scrollRef.current} />}
      <TablePicker />
    </div>
  );
}

/** Passwords of the notes unlocked in this session (never stored). */
const passwords = new Map<string, string>();

function LockScreen({ onUnlock }: { onUnlock: (password: string) => Promise<boolean> }) {
  const t = useT();
  const [pw, setPw] = useState("");
  const [wrong, setWrong] = useState(false);
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    if (!pw || busy) return;
    setBusy(true);
    const ok = await onUnlock(pw);
    setBusy(false);
    setWrong(!ok);
  };
  return (
    <div className="lock-screen">
      <div className="lock-icon">
        <Icon name="lock" />
      </div>
      <div className="lock-title">{t("lock.title")}</div>
      <p>{t("lock.enter")}</p>
      <div className="row-buttons" style={{ justifyContent: "center" }}>
        <input
          className="input"
          type="password"
          autoFocus
          value={pw}
          style={{ width: 240 }}
          onChange={(e) => setPw(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && void submit()}
        />
        <button className="btn primary" disabled={!pw || busy} onClick={() => void submit()}>
          {t("lock.open")}
        </button>
      </div>
      {wrong && <div className="lock-wrong">{t("lock.wrong")}</div>}
    </div>
  );
}

/** Opens a linked note, switching to a list that has it. */
async function openLinkedNote(id: string) {
  if (!(await useStore.getState().openNote(id))) useStore.getState().toast(t("links.missing"));
}

/** Back (on phones) and buttons to bring back the sidebar and the note list. */
function PaneToggles() {
  useT();
  const sidebarOpen = useStore((s) => s.sidebarOpen);
  const listOpen = useStore((s) => s.listOpen);
  const focusMode = useStore((s) => s.focusMode);
  const layout = useStore((s) => s.layout);
  if (layout === "narrow") {
    return (
      <button className="icon-btn back-btn" title={t("nav.back")} onClick={() => useStore.getState().set({ mobilePane: "list" })}>
        <Icon name="back" />
      </button>
    );
  }
  if (focusMode) return null;
  return (
    <>
      {(!sidebarOpen || layout === "medium") && (
        // Hovering peeks at the sidebar; a click opens it (lib/peek.ts).
        <button
          className="icon-btn"
          title={`${layout === "medium" && sidebarOpen ? t("nav.closeSidebar") : t("nav.openSidebar")} (Ctrl+\\)`}
          {...(layout === "medium" ? peekButtonProps : {})}
          onClick={() => (layout === "medium" && sidebarOpen ? useStore.getState().set({ sidebarOpen: false }) : pinSidebar())}
        >
          <Icon name="sidebarOpen" />
        </button>
      )}
      {!listOpen && (
        <button className="icon-btn" title={`${t("nav.openList")} (Ctrl+Shift+\\)`} onClick={() => useStore.getState().set({ listOpen: true })}>
          <Icon name="listShow" />
        </button>
      )}
    </>
  );
}

/** Back and forward between the notes opened (shown once there is somewhere to go). */
function NavButtons() {
  useT();
  const { canBack, canForward } = useNavState();
  const layout = useStore((s) => s.layout);
  if (layout === "narrow" || (!canBack && !canForward)) return null;
  return (
    <span className="nav-buttons">
      <button className="icon-btn" disabled={!canBack} title={`${t("nav.backNote")} (Alt+←)`} onClick={() => void goBack()}>
        <Icon name="chevronLeft" />
      </button>
      <button className="icon-btn" disabled={!canForward} title={`${t("nav.forwardNote")} (Alt+→)`} onClick={() => void goForward()}>
        <Icon name="chevronRight" />
      </button>
    </span>
  );
}

/** Which columns are shown. */
function ViewButton() {
  useT();
  const s = useStore();
  if (s.layout === "narrow") return null;
  const mode = s.focusMode ? "focus" : !s.listOpen ? "editorOnly" : "all";
  const set = (m: string) => {
    if (m === "focus") s.set({ focusMode: true });
    else if (m === "all") s.set({ focusMode: false, listOpen: true });
    else s.set({ focusMode: false, sidebarOpen: false, listOpen: false });
  };
  return (
    <button
      className={`icon-btn${mode !== "all" ? " active" : ""}`}
      title={t("menu.view")}
      onClick={(e) =>
        openMenu(e.currentTarget, [
          { header: t("menu.view") },
          { label: t("view.all"), icon: "columns", checked: mode === "all", hint: "", run: () => set("all") },
          { label: t("view.editorOnly"), icon: "note", checked: mode === "editorOnly", hint: "Ctrl Shift \\", run: () => set("editorOnly") },
          { label: t("view.focus"), icon: "focus", checked: mode === "focus", hint: "F11", run: () => set("focus") },
        ])
      }
    >
      <Icon name="columns" />
    </button>
  );
}
