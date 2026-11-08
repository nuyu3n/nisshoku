// A conflict copy: what differs from the original note, and settling it
// (take this version, or keep the original and drop this copy).

import { useEffect, useState } from "react";
import { api, type Note } from "../api";
import { report, useStore } from "../store";
import { useT } from "../i18n";
import { Icon } from "./Icon";
import { dialogs } from "./Dialogs";

type Line = { kind: "same" | "add" | "del"; text: string };

/** Line diff from `a` to `b` (longest common subsequence; long texts by a cheaper pass). */
export function lineDiff(a: string, b: string): Line[] {
  const x = a.split("\n");
  const y = b.split("\n");
  // Common start and end first: conflicts usually differ in a few places.
  let start = 0;
  while (start < x.length && start < y.length && x[start] === y[start]) start++;
  let end = 0;
  while (end < x.length - start && end < y.length - start && x[x.length - 1 - end] === y[y.length - 1 - end]) end++;
  const xs = x.slice(start, x.length - end);
  const ys = y.slice(start, y.length - end);
  const middle: Line[] = [];
  if (xs.length * ys.length > 4_000_000) {
    xs.forEach((text) => middle.push({ kind: "del", text }));
    ys.forEach((text) => middle.push({ kind: "add", text }));
  } else {
    const n = xs.length;
    const m = ys.length;
    const lcs: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
    for (let i = n - 1; i >= 0; i--)
      for (let j = m - 1; j >= 0; j--) lcs[i][j] = xs[i] === ys[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    let i = 0;
    let j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && xs[i] === ys[j]) {
        middle.push({ kind: "same", text: xs[i] });
        i++;
        j++;
      } else if (j < m && (i >= n || lcs[i][j + 1] >= lcs[i + 1][j])) middle.push({ kind: "add", text: ys[j++] });
      else middle.push({ kind: "del", text: xs[i++] });
    }
  }
  return [
    ...x.slice(0, start).map((text): Line => ({ kind: "same", text })),
    ...middle,
    ...x.slice(x.length - end).map((text): Line => ({ kind: "same", text })),
  ];
}

export function ConflictBar({ note, onDone }: { note: Note; onDone: (id: string) => void }) {
  const t = useT();
  const [original, setOriginal] = useState<Note | null | undefined>(undefined);
  const [open, setOpen] = useState(false);
  const originalId = note.conflictOriginalId;
  useEffect(() => {
    if (!originalId) return setOriginal(null);
    api.getNote(originalId).then(
      (n) => setOriginal(n.deletedTime ? null : n),
      () => setOriginal(null),
    );
  }, [originalId, note.id]);

  const settle = async (keep: "copy" | "original") => {
    if (keep === "original" && !(await dialogs.confirm(t("conflict.dropTitle"), t("conflict.dropDetail"), { okLabel: t("conflict.drop"), danger: true }))) return;
    try {
      const id = await api.resolveConflict(note.id, keep);
      await useStore.getState().refresh();
      onDone(id);
      useStore.getState().toast(t(keep === "copy" ? "conflict.tookCopy" : "conflict.dropped"));
    } catch (e) {
      report(e);
    }
  };

  const diff = open && original ? lineDiff(`${original.title}\n\n${original.body}`, `${note.title}\n\n${note.body}`) : [];
  return (
    <div className="notice conflict-notice">
      <div className="conflict-row">
        <Icon name="conflict" />
        <span className="grow">{original === null ? t("conflict.orphan") : t("editor.conflictNotice")}</span>
        {original && (
          <button className="btn" onClick={() => setOpen(!open)}>
            {open ? t("conflict.hideDiff") : t("conflict.showDiff")}
          </button>
        )}
        <button className="btn primary" onClick={() => void settle("copy")}>
          {original === null ? t("conflict.makeNote") : t("conflict.take")}
        </button>
        {original && (
          <button className="btn" onClick={() => void settle("original")}>
            {t("conflict.keepOriginal")}
          </button>
        )}
      </div>
      {open && original && (
        <div className="conflict-diff" aria-label={t("conflict.showDiff")}>
          <div className="conflict-legend">
            <span className="del">− {t("conflict.legendOriginal")}</span>
            <span className="add">＋ {t("conflict.legendCopy")}</span>
          </div>
          {diff.map((l, i) => (
            <div key={i} className={`dl ${l.kind}`}>
              <span className="mark">{l.kind === "add" ? "＋" : l.kind === "del" ? "−" : " "}</span>
              {l.text || " "}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
