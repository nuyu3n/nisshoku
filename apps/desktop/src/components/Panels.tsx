// Side panels of the editor: outline, history and the vertical manuscript
// preview.

import { useEffect, useMemo, useState } from "react";
import type { Editor } from "@tiptap/core";
import { api, type NoteComment, type Revision } from "../api";
import { useT } from "../i18n";
import { commentRange } from "../editor/comments";
import { check, type Issue } from "../lib/proof";
import { report, useStore } from "../store";
import { countChars, longDate, markdownToPlain, relativeTime, rubyToHtml } from "../lib/text";

export { PreviewPanel } from "./Genko";
import { Icon } from "./Icon";
import { dialogs } from "./Dialogs";

function PanelHead({ title, children }: { title: string; children?: React.ReactNode }) {
  const t = useT();
  return (
    <div className="side-panel-head">
      <span className="grow">{title}</span>
      {children}
      <button className="icon-btn" title={t("panel.close")} onClick={() => useStore.getState().set({ panel: "none" })}>
        <Icon name="x" />
      </button>
    </div>
  );
}

// -------------------------------------------------------------------------
// Outline

interface Heading {
  level: number;
  text: string;
  pos: number;
  chars: number;
}

function headings(editor: Editor): Heading[] {
  const out: Heading[] = [];
  let current: Heading | null = null;
  editor.state.doc.forEach((node, pos) => {
    if (node.type.name === "heading") {
      current = { level: node.attrs.level as number, text: node.textContent, pos, chars: 0 };
      out.push(current);
    } else if (current) {
      current.chars += countChars(node.textContent);
    }
  });
  return out;
}

export function OutlinePanel({ editor }: { editor: Editor | null }) {
  const t = useT();
  const [items, setItems] = useState<Heading[]>([]);
  useEffect(() => {
    if (!editor) return;
    const update = () => setItems(headings(editor));
    update();
    editor.on("update", update);
    return () => {
      editor.off("update", update);
    };
  }, [editor]);

  return (
    <aside className="side-panel">
      <PanelHead title={t("editor.outline")} />
      <div className="side-panel-body">
        {items.length === 0 && <div className="empty pre">{t("outline.empty")}</div>}
        {items.map((h, i) => (
          <button
            key={i}
            className={`outline-item l${h.level}`}
            title={`${h.text}（${h.chars.toLocaleString()}）`}
            onClick={() => {
              if (!editor) return;
              editor.chain().focus().setTextSelection(h.pos + 1).run();
              const dom = editor.view.nodeDOM(h.pos) as HTMLElement | null;
              dom?.scrollIntoView({ block: "start", behavior: "smooth" });
            }}
          >
            {h.text || t("outline.untitled")}
            <span style={{ float: "right", color: "var(--text-3)", fontSize: 11 }}>{h.chars.toLocaleString()}</span>
          </button>
        ))}
      </div>
    </aside>
  );
}

// -------------------------------------------------------------------------
// History

/** Line-based diff (LCS), for comparing a version with the current text. */
function diffLines(a: string[], b: string[]): { op: "same" | "add" | "del"; text: string }[] {
  const n = a.length;
  const m = b.length;
  if (n * m > 4_000_000) return b.map((text) => ({ op: "same" as const, text }));
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--) dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const out: { op: "same" | "add" | "del"; text: string }[] = [];
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && a[i] === b[j]) {
      out.push({ op: "same", text: a[i] });
      i++;
      j++;
    } else if (j < m && (i === n || dp[i][j + 1] >= dp[i + 1][j])) {
      out.push({ op: "add", text: b[j++] });
    } else {
      out.push({ op: "del", text: a[i++] });
    }
  }
  return out;
}

export function HistoryPanel({ noteId, currentBody, onBeforeRestore }: { noteId: string; currentBody: () => string; onBeforeRestore: () => Promise<void> }) {
  const t = useT();
  const [revs, setRevs] = useState<Revision[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState<{ title: string; body: string } | null>(null);
  const [showDiff, setShowDiff] = useState(true);

  const load = () => api.listRevisions(noteId).then(setRevs).catch(report);
  useEffect(() => {
    setSelected(null);
    setContent(null);
    void load();
  }, [noteId]);
  useEffect(() => {
    if (selected) api.revisionContent(noteId, selected).then(setContent).catch(report);
  }, [selected, noteId]);

  const diff = useMemo(() => {
    if (!content) return null;
    return diffLines(content.body.split("\n"), currentBody().split("\n"));
  }, [content]);

  return (
    <aside className="side-panel">
      <PanelHead title={t("editor.history")}>
        <button
          className="icon-btn"
          title={t("history.saveNow")}
          onClick={async () => {
            await onBeforeRestore();
            const saved = await api.saveSnapshot(noteId).catch(report);
            useStore.getState().toast(saved ? t("toast.snapshotSaved") : t("toast.snapshotSame"));
            void load();
          }}
        >
          <Icon name="save" />
        </button>
      </PanelHead>
      <div className="side-panel-body" style={{ flex: content ? "0 1 45%" : 1 }}>
        {revs.length === 0 && (
          <div className="empty pre">{t("history.empty")}</div>
        )}
        {revs.map((r) => (
          <button key={r.id} className={`rev-item${selected === r.id ? " active" : ""}`} onClick={() => setSelected(r.id)}>
            <span>{longDate(r.time)}</span>
            <small>{relativeTime(r.time)}</small>
          </button>
        ))}
      </div>
      {content && (
        <>
          <div className="side-panel-head" style={{ borderTop: "1px solid var(--border)" }}>
            <span className="grow" style={{ fontSize: 12.5 }}>
              {countChars(content.body).toLocaleString()} {t("today.unit")}
            </span>
            <div className="segmented">
              <button className={showDiff ? "on" : ""} onClick={() => setShowDiff(true)}>
                {t("history.diff")}
              </button>
              <button className={!showDiff ? "on" : ""} onClick={() => setShowDiff(false)}>
                {t("history.full")}
              </button>
            </div>
            <button
              className="btn primary"
              style={{ height: 28, marginLeft: 6 }}
              onClick={async () => {
                if (!(await dialogs.confirm(t("history.restoreConfirm"), t("history.restoreDetail"), { okLabel: t("history.restore") }))) return;
                await onBeforeRestore();
                await api.restoreRevision(noteId, selected!).catch(report);
                useStore.getState().reloadNote();
                useStore.getState().toast(t("history.restored"));
                void load();
              }}
            >
              <Icon name="restore" />
              {t("history.restore")}
            </button>
          </div>
          <div className="rev-preview">
            <strong>{content.title}</strong>
            {"\n\n"}
            {showDiff && diff
              ? diff.map((d, i) => (
                  <div key={i} className={d.op === "add" ? "diff-add" : d.op === "del" ? "diff-del" : undefined}>
                    {d.text || " "}
                  </div>
                ))
              : content.body}
          </div>
          {showDiff && (
            <div style={{ padding: "6px 16px 10px", fontSize: 11.5, color: "var(--text-3)" }}>
              {t("history.legend")}
            </div>
          )}
        </>
      )}
    </aside>
  );
}

// -------------------------------------------------------------------------
// Comments of the open note

export function CommentsPanel({ editor, noteId, comments, onChange }: { editor: Editor | null; noteId: string; comments: NoteComment[]; onChange: () => void }) {
  const t = useT();
  const active = useStore((s) => s.activeCommentId);
  const [showResolved, setShowResolved] = useState(false);
  const shown = comments.filter((c) => showResolved || !c.resolved || c.id === active);
  // Whether a comment's text is still there is the editor's state: follow it
  // (a new comment is marked a moment after the list gets it).
  const [, setVersion] = useState(0);
  useEffect(() => {
    if (!editor) return;
    const bump = () => setVersion((v) => v + 1);
    editor.on("transaction", bump);
    return () => void editor.off("transaction", bump);
  }, [editor]);

  useEffect(() => {
    if (active) document.querySelector(`.side-panel .comment-card[data-id="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const reveal = (c: NoteComment) => {
    useStore.getState().set({ activeCommentId: c.id });
    if (!editor) return;
    const r = commentRange(editor, c.id);
    if (!r) return;
    editor.chain().setTextSelection(r).run();
    const dom = editor.view.domAtPos(r.from).node as HTMLElement;
    (dom.nodeType === 1 ? dom : dom.parentElement)?.scrollIntoView({ block: "center", behavior: "smooth" });
  };

  return (
    <aside className="side-panel">
      <PanelHead title={t("comments.title")} />
      <div className="side-panel-body">
        <label className="check-row">
          <input type="checkbox" checked={showResolved} onChange={(e) => setShowResolved(e.target.checked)} />
          {t("comments.showResolved")}
        </label>
        {shown.length === 0 && <div className="empty pre small">{t("comments.empty")}</div>}
        {shown.map((c) => {
          const found = editor ? commentRange(editor, c.id) !== null : true;
          return (
            <div key={c.id} data-id={c.id} className={`comment-card${c.id === active ? " active" : ""}${c.resolved ? " resolved" : ""}`} onClick={() => reveal(c)}>
              <div className="comment-quote">{c.quote}</div>
              {!found && <div className="comment-missing">{t("comments.missing")}</div>}
              <div className="comment-text">{c.text}</div>
              <div className="comment-actions">
                <span className="comment-time">{relativeTime(c.createdTime)}</span>
                <span className="grow" />
                <button
                  onClick={async (e) => {
                    e.stopPropagation();
                    await api.updateComment(noteId, c.id, { resolved: !c.resolved }).catch(report);
                    onChange();
                  }}
                >
                  {c.resolved ? t("comments.reopen") : t("comments.resolve")}
                </button>
                <button
                  title={t("comments.edit")}
                  onClick={async (e) => {
                    e.stopPropagation();
                    const text = await dialogs.prompt(t("comments.edit"), { initial: c.text, multiline: true });
                    if (text === null) return;
                    await api.updateComment(noteId, c.id, { text }).catch(report);
                    onChange();
                  }}
                >
                  <Icon name="pen" />
                </button>
                <button
                  title={t("comments.delete")}
                  onClick={async (e) => {
                    e.stopPropagation();
                    if (!(await dialogs.confirm(t("comments.deleteConfirm"), undefined, { okLabel: t("comments.delete"), danger: true }))) return;
                    await api.deleteComment(noteId, c.id).catch(report);
                    onChange();
                  }}
                >
                  <Icon name="trash" />
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </aside>
  );
}

// -------------------------------------------------------------------------
// Writing check

/** Text blocks of the document with their positions. */
function blocksOf(editor: Editor): { text: string; pos: number }[] {
  const out: { text: string; pos: number }[] = [];
  editor.state.doc.descendants((node, pos) => {
    if (node.isTextblock) {
      out.push({ text: node.textContent, pos: pos + 1 });
      return false;
    }
    return true;
  });
  return out;
}

export function ProofPanel({ editor }: { editor: Editor | null }) {
  const t = useT();
  const [result, setResult] = useState<ReturnType<typeof check> | null>(null);
  const [blocks, setBlocks] = useState<{ text: string; pos: number }[]>([]);
  useEffect(() => {
    if (!editor) return;
    let timer: number | undefined;
    const run = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        const b = blocksOf(editor);
        setBlocks(b);
        setResult(check(b.map((x) => x.text)));
      }, 300);
    };
    run();
    editor.on("update", run);
    return () => {
      window.clearTimeout(timer);
      editor.off("update", run);
    };
  }, [editor]);

  const go = (issue: Issue) => {
    const b = blocks[issue.block];
    if (!editor || !b) return;
    // Offsets count characters of text nodes; positions map one-to-one within a paragraph of text.
    editor.chain().focus().setTextSelection({ from: b.pos + issue.from, to: b.pos + issue.to }).scrollIntoView().run();
  };
  const m = result?.measures;
  const labels: Record<Issue["kind"], string> = {
    long: "長", ending: "末", ranuki: "ら", commas: "、", double: "重", variant: "揺",
  };

  return (
    <aside className="side-panel">
      <PanelHead title={t("proof.title")} />
      <div className="side-panel-body">
        {m && (
          <div className="proof-stats">
            <div><b>{m.sentences}</b><small>{t("proof.sentences")}</small></div>
            <div><b>{m.averageLength}</b><small>{t("proof.average")}</small></div>
            <div><b>{m.longest}</b><small>{t("proof.longest")}</small></div>
            <div><b>{m.kanjiRatio}%</b><small>{t("proof.kanji")}</small></div>
          </div>
        )}
        {result && result.issues.length === 0 && <div className="empty small">{t("proof.none")}</div>}
        {result?.issues.map((issue, i) => (
          <button key={i} className={`proof-item ${issue.kind}`} onClick={() => go(issue)}>
            <span className="proof-kind">{labels[issue.kind]}</span>
            <span className="proof-body">
              <span className="proof-message">{issue.message}</span>
              <span className="proof-excerpt">{issue.excerpt.length > 60 ? `${issue.excerpt.slice(0, 60)}…` : issue.excerpt}</span>
            </span>
          </button>
        ))}
      </div>
    </aside>
  );
}

// -------------------------------------------------------------------------
// Reference: another note (a character, the plot, notes for the chapter)
// shown beside the text, to read while writing.

export function ReferencePanel({ currentId }: { currentId: string }) {
  const t = useT();
  const referenceId = useStore((s) => s.referenceId);
  const remoteVersion = useStore((s) => s.remoteVersion);
  const [note, setNote] = useState<{ title: string; body: string } | null>(null);
  useEffect(() => {
    if (!referenceId) return setNote(null);
    api.getNote(referenceId).then(
      (n) => setNote({ title: n.title, body: n.body }),
      () => useStore.getState().set({ referenceId: null }),
    );
  }, [referenceId, remoteVersion]);
  const choose = async () => {
    const notes = await api.listNotes({ kind: "all" }).catch(report);
    if (!notes) return;
    const id = await dialogs.choose(
      t("reference.choose"),
      notes.filter((n) => n.id !== currentId).map((n) => ({ value: n.id, label: n.title || t("list.untitled") })),
    );
    if (id) useStore.getState().set({ referenceId: id });
  };
  const paragraphs = useMemo(() => (note ? markdownToPlain(note.body).split(/\n+/).filter((p) => p.trim()) : []), [note]);
  return (
    <aside className="side-panel reference-panel">
      <PanelHead title={note?.title || t("reference.title")}>
        <button className="icon-btn" title={t("reference.choose")} onClick={() => void choose()}>
          <Icon name="search" />
        </button>
        {referenceId && (
          <button className="icon-btn" title={t("reference.open")} onClick={() => useStore.getState().select(referenceId)}>
            <Icon name="note" />
          </button>
        )}
      </PanelHead>
      <div className="side-panel-body reference-body">
        {!note ? (
          <div className="empty">
            {t("reference.empty")}
            <br />
            <button className="btn" onClick={() => void choose()}>
              {t("reference.choose")}
            </button>
          </div>
        ) : (
          paragraphs.map((p, i) => <p key={i} dangerouslySetInnerHTML={{ __html: rubyToHtml(p) }} />)
        )}
      </div>
    </aside>
  );
}
