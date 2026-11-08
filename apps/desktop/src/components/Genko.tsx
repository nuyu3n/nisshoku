// Vertical preview on manuscript paper (縦書き・原稿用紙): pages drawn cell
// by cell like real 400字詰 paper — boxes in columns with a narrow band
// for ruby between them, the centre fold (柱) with its fish-tail marks,
// punctuation hanging below the grid, and the size printed in the margin.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { Editor } from "@tiptap/core";
import { lang, useT } from "../i18n";
import { useStore, type Preferences } from "../store";
import { layoutLines, pagesOf, type Cell, type Line } from "../lib/genko";
import { Icon } from "./Icon";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

const INKS: { id: Preferences["genkoInk"]; ja: string; en: string }[] = [
  { id: "accent", ja: "朱", en: "Orange" },
  { id: "vermilion", ja: "赤", en: "Red" },
  { id: "green", ja: "緑", en: "Green" },
  { id: "indigo", ja: "藍", en: "Indigo" },
  { id: "sumi", ja: "墨", en: "Ink" },
];

const MIN_CELL = 12;
const MAX_CELL = 48;
/** Page height in cells beyond the grid: top margin, hanging punctuation and the footer. */
const EXTRA_ROWS = 3.4;
const hasFold = (rows: number) => rows % 2 === 0 && rows >= 8;
/** Width of a sheet with its margins, in cells. */
const across = (rows: number) => rows * 1.32 + 0.32 + (hasFold(rows) ? 1.15 : 0) + 2.2;

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function cellHtml(c: Cell): string {
  if (c.t === "　") return `<span class="c"></span>`;
  return `<span class="c${c.k ? ` ${c.k}` : ""}">${esc(c.t)}</span>`;
}

function lineHtml(line: Line | undefined, index: number): string {
  if (!line) return `<div class="gk-line"><div class="gk-cells"></div></div>`;
  const ruby = line.ruby.map((r) => `<span class="rb" style="--at:${r.at};--len:${r.len}">${esc(r.text)}</span>`).join("");
  const hang = line.hang.length ? `<span class="hang">${line.hang.map(cellHtml).join("")}</span>` : "";
  return `<div class="gk-line" data-p="${line.para}" data-i="${index}"><div class="gk-cells">${line.cells.map(cellHtml).join("")}${hang}</div>${ruby}</div>`;
}

/** The inside of one sheet: its lines right to left, the fold in the middle. */
function pageHtml(lines: Line[], rows: number, cols: number, no: number, title: string): string {
  const first = (no - 1) * rows;
  const fold = hasFold(rows) ? rows / 2 : -1;
  let out = `<div class="gk-sheet">`;
  for (let i = 0; i < rows; i++) {
    if (i === fold) {
      out += `<div class="gk-fold"><span class="fish"></span><span class="gk-fold-title">${esc(title)}</span><span class="fish down"></span></div>`;
    }
    out += lineHtml(lines[i], first + i);
  }
  out += `</div><div class="gk-foot"><span class="gk-size">${cols}×${rows}</span><span class="gk-no">${no}</span></div>`;
  return out;
}

/** One sheet; its cells are drawn only near the screen. */
function Page({ html, index, root }: { html: string; index: number; root: HTMLElement | null }) {
  const ref = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el || !root) return;
    const io = new IntersectionObserver(([e]) => setNear(e.isIntersecting), { root, rootMargin: "100% 150% 100% 150%" });
    io.observe(el);
    return () => io.disconnect();
  }, [root]);
  return <div ref={ref} className="gk-page" data-page={index} dangerouslySetInnerHTML={{ __html: near ? html : "" }} />;
}

/**
 * Every page, for printing: in the main window (the app's own print path),
 * shown only by the print styles while `body.printing-genko` is set. One
 * landscape A4 sheet a page, the cell as large as fits.
 */
function PrintSheets({ htmls, cols, rows, ink, onDone }: { htmls: string[]; cols: number; rows: number; ink: string; onDone: () => void }) {
  // A4 landscape is asked for; the sheet still fits should the dialog print upright (190 mm wide).
  const cell = Math.min(186 / (cols + EXTRA_ROWS), 188 / across(rows));
  useEffect(() => {
    document.body.classList.add("printing-genko");
    // WebView2's print() does not wait for its dialog, which lays the page
    // out again as settings change there (and may say "afterprint" early):
    // the sheets stay until the app is used again.
    const done = () => onDone();
    const arm = () => {
      document.addEventListener("pointerdown", done, { capture: true, once: true });
      document.addEventListener("keydown", done, { capture: true, once: true });
    };
    // Printed once the sheets are laid out.
    const id = window.setTimeout(() => {
      window.print();
      arm();
    }, 150);
    return () => {
      window.clearTimeout(id);
      document.removeEventListener("pointerdown", done, { capture: true });
      document.removeEventListener("keydown", done, { capture: true });
      document.body.classList.remove("printing-genko");
    };
  }, [onDone]);
  return createPortal(
    <div className="genko gk-print" data-ink={ink} style={{ ["--cell" as string]: `${cell.toFixed(2)}mm`, ["--rows" as string]: rows, ["--cols" as string]: cols, ["--fold" as string]: hasFold(rows) ? 1 : 0 }}>
      <style>{"@page { size: A4 landscape; margin: 10mm; }"}</style>
      {htmls.map((h, i) => (
        <div key={i} className="gk-page" dangerouslySetInnerHTML={{ __html: h }} />
      ))}
    </div>,
    document.body,
  );
}

/** The paragraph (text block) the cursor is in, and the cursor's place in it. */
function cursorPlace(editor: Editor): { para: number; offset: number } {
  const at = editor.state.selection.head;
  let index = -1;
  let found = { para: -1, offset: 0 };
  editor.state.doc.descendants((node, pos) => {
    if (found.para >= 0) return false;
    if (node.isTextblock) {
      index++;
      if (at >= pos && at <= pos + node.nodeSize) found = { para: index, offset: Math.max(0, at - pos - 1) };
      return false;
    }
    return true;
  });
  return found;
}

function revealParagraph(editor: Editor, para: number) {
  let index = -1;
  let target = -1;
  editor.state.doc.descendants((node, pos) => {
    if (target >= 0) return false;
    if (node.isTextblock) {
      if (++index === para) target = pos + 1;
      return false;
    }
    return true;
  });
  if (target < 0) return;
  editor.chain().focus().setTextSelection(target).scrollIntoView().run();
}

export function PreviewPanel({ title, paragraphs, editor }: { title: string; paragraphs: string[]; editor: Editor | null }) {
  const t = useT();
  const prefs = useStore((s) => s.prefs);
  const { manuscriptCols: cols, manuscriptRows: rows, genkoCell, genkoInk, genkoFollow } = prefs;
  const setPrefs = useStore((s) => s.setPrefs);
  const [wrap, setWrap] = useState<HTMLDivElement | null>(null);
  const [fitCell, setFitCell] = useState(20);
  const [current, setCurrent] = useState(0);
  const [place, setPlace] = useState({ para: -1, offset: 0 });
  const active = place.para;
  const [printing, setPrinting] = useState(0);
  const donePrinting = useCallback(() => setPrinting(0), []);

  // Lines: the title three cells down, a blank line, then the text.
  const lines = useMemo(() => {
    const head: Line[] = [];
    if (title.trim()) {
      for (const l of layoutLines([title], Math.max(1, cols - 3), false, -1)) {
        const shift = <T extends { at: number }>(x: T): T => ({ ...x, at: x.at + 3 });
        head.push({ ...l, from: 0, cells: [{ t: "　" }, { t: "　" }, { t: "　" }, ...l.cells], ruby: l.ruby.map(shift) });
      }
      head.push({ cells: [], ruby: [], hang: [], para: -1, from: 0 });
    }
    return [...head, ...layoutLines(paragraphs, cols, true)];
  }, [title, paragraphs, cols]);
  const pages = useMemo(() => pagesOf(lines, rows), [lines, rows]);
  const htmls = useMemo(() => pages.map((p, i) => pageHtml(p, rows, cols, i + 1, title)), [pages, rows, cols, title]);
  /** The first line of each paragraph. */
  const firstLine = useMemo(() => {
    const m = new Map<number, number>();
    lines.forEach((l, i) => !m.has(l.para) && m.set(l.para, i));
    return m;
  }, [lines]);
  /** The line the cursor is on. */
  const cursorLine = useMemo(() => {
    const start = firstLine.get(place.para);
    if (start === undefined) return -1;
    let i = start;
    while (i + 1 < lines.length && lines[i + 1].para === place.para && lines[i + 1].from <= place.offset) i++;
    return i;
  }, [lines, firstLine, place]);

  // "Fit": a whole sheet in view, by its height and its width.
  useLayoutEffect(() => {
    if (!wrap) return;
    const ro = new ResizeObserver(() => {
      const h = wrap.clientHeight - 44;
      const w = wrap.clientWidth - 44;
      const fit = Math.floor(Math.min(h / (cols + EXTRA_ROWS), w / across(rows)));
      setFitCell(Math.max(MIN_CELL, Math.min(MAX_CELL, fit)));
    });
    ro.observe(wrap);
    return () => ro.disconnect();
  }, [wrap, cols, rows]);
  const cell = genkoCell || fitCell;

  // The page in the middle of the view.
  const onScroll = useCallback(() => {
    if (!wrap) return;
    const mid = wrap.getBoundingClientRect().left + wrap.clientWidth / 2;
    let best = 0;
    let dist = Infinity;
    wrap.querySelectorAll<HTMLElement>(".genko-live > .gk-page").forEach((el, i) => {
      const r = el.getBoundingClientRect();
      const d = Math.abs(r.left + r.width / 2 - mid);
      if (d < dist) {
        dist = d;
        best = i;
      }
    });
    setCurrent(best);
  }, [wrap]);
  useEffect(onScroll, [onScroll, pages.length, cell]);

  /**
   * Brings `el` to the middle of the preview (only when it is not in view
   * already, unless `always`). Scrolls the preview alone: scrollIntoView would
   * also scroll the app's other boxes, its overflow-hidden ones too.
   */
  const reveal = useCallback(
    (el: HTMLElement | null | undefined, always: boolean) => {
      if (!el || !wrap) return;
      const r = el.getBoundingClientRect();
      const w = wrap.getBoundingClientRect();
      const inView = r.left >= w.left - 1 && r.right <= w.right + 1 && r.top >= w.top - 1 && r.bottom <= w.bottom + 1;
      if (inView && !always) return;
      // Something larger than the view is shown from its start (right, top).
      const dx = r.width > w.width ? r.right - w.right : r.left + r.width / 2 - (w.left + w.width / 2);
      const dy = r.height > w.height ? r.top - w.top : r.top + r.height / 2 - (w.top + w.height / 2);
      wrap.scrollBy({ left: dx, top: dy, behavior: "smooth" });
    },
    [wrap],
  );
  const pageEl = useCallback((i: number) => wrap?.querySelector<HTMLElement>(`.genko-live > .gk-page[data-page="${i}"]`) ?? null, [wrap]);
  const goTo = (i: number) => reveal(pageEl(i), true);

  // The paragraph at the cursor is marked.
  useEffect(() => {
    if (!editor) return;
    let timer: number | undefined;
    const update = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        const next = cursorPlace(editor);
        setPlace((p) => (p.para === next.para && p.offset === next.offset ? p : next));
      }, 80);
    };
    update();
    editor.on("selectionUpdate", update);
    editor.on("update", update);
    return () => {
      window.clearTimeout(timer);
      editor.off("selectionUpdate", update);
      editor.off("update", update);
    };
  }, [editor]);

  // Following: when the cursor moves to another line, that line is brought
  // into view (if it is not in view already).
  useEffect(() => {
    if (!genkoFollow || cursorLine < 0 || !wrap) return;
    const page = pageEl(Math.floor(cursorLine / rows));
    if (!page) return;
    const lineOf = () => page.querySelector<HTMLElement>(`.gk-line[data-i="${cursorLine}"]`);
    if (lineOf()) {
      reveal(lineOf(), false);
      return;
    }
    // A page away from the screen has no lines yet: the page first, then the line.
    reveal(page, true);
    const id = window.setTimeout(() => reveal(lineOf(), false), 450);
    return () => window.clearTimeout(id);
  }, [cursorLine, rows, genkoFollow, wrap, pageEl, reveal]);

  const onClick = (e: React.MouseEvent) => {
    const line = (e.target as HTMLElement).closest<HTMLElement>(".gk-line[data-p]");
    const p = Number(line?.dataset.p ?? -1);
    if (editor && p >= 0) revealParagraph(editor, p);
  };

  const zoom = (d: number) => setPrefs({ genkoCell: Math.max(MIN_CELL, Math.min(MAX_CELL, cell + d)) });
  const filled = lines.filter((l) => l.cells.length > 0).length;
  const lastPage = lines.length % rows || (lines.length ? rows : 0);

  return (
    <aside className="side-panel wide genko-panel">
      <div className="side-panel-head">
        <span className="grow genko-head-title">{t("preview.title", { cols, rows, pages: pages.length })}</span>
        <button className="icon-btn" title={L("印刷・PDF に保存", "Print or save as PDF")} onClick={() => setPrinting((n) => n + 1)}>
          <Icon name="print" />
        </button>
        <button className="icon-btn" title={t("panel.close")} onClick={() => useStore.getState().set({ panel: "none" })}>
          <Icon name="x" />
        </button>
      </div>
      <div className="genko-tools">
        <div className="genko-nav">
          <button className="icon-btn" title={L("次の枚", "Next page")} disabled={current >= pages.length - 1} onClick={() => goTo(current + 1)}>
            <Icon name="chevronLeft" />
          </button>
          <span className="genko-count">
            <b>{pages.length ? current + 1 : 0}</b> / {pages.length}
            {L("枚", "")}
          </span>
          <button className="icon-btn" title={L("前の枚", "Previous page")} disabled={current <= 0} onClick={() => goTo(current - 1)}>
            <Icon name="chevronRight" />
          </button>
        </div>
        <div className="genko-zoom">
          <button className="icon-btn" title={L("縮小", "Smaller")} onClick={() => zoom(-2)}>
            <Icon name="minus" />
          </button>
          <button className={`genko-fit${genkoCell === 0 ? " on" : ""}`} title={L("1枚全体が見える大きさ", "Fit a whole page")} onClick={() => setPrefs({ genkoCell: 0 })}>
            {Math.round((cell / 20) * 100)}%
          </button>
          <button className="icon-btn" title={L("拡大", "Larger")} onClick={() => zoom(2)}>
            <Icon name="plus" />
          </button>
        </div>
        <div className="genko-inks" role="radiogroup" aria-label={L("罫の色", "Grid colour")}>
          {INKS.map((ink) => (
            <button
              key={ink.id}
              role="radio"
              aria-checked={genkoInk === ink.id}
              className={`genko-ink${genkoInk === ink.id ? " on" : ""}`}
              data-ink={ink.id}
              title={L(ink.ja, ink.en)}
              onClick={() => setPrefs({ genkoInk: ink.id })}
            />
          ))}
        </div>
        {editor && (
          <label className="genko-follow" title={L("カーソルのある行を表示します", "Keep the cursor's line in view")}>
            <input type="checkbox" checked={genkoFollow} onChange={(e) => setPrefs({ genkoFollow: e.target.checked })} />
            {L("追従", "Follow")}
          </label>
        )}
      </div>
      <div ref={setWrap} className="genko-wrap" onScroll={onScroll}>
        <div className="genko genko-live" data-ink={genkoInk} style={{ ["--cell" as string]: `${cell}px`, ["--rows" as string]: rows, ["--cols" as string]: cols, ["--fold" as string]: hasFold(rows) ? 1 : 0 }} onClick={onClick}>
          {active >= 0 && (
            <style>{`.genko-live .gk-line[data-p="${active}"] .gk-cells{background-color:var(--gk-active)}.genko-live .gk-line[data-i="${cursorLine}"] .gk-cells{background-color:var(--gk-cursor)}`}</style>
          )}
          {htmls.map((h, i) => (
            <Page key={i} html={h} index={i} root={wrap} />
          ))}
        </div>
      </div>
      <div className="genko-status">
        {L(`${filled}行・最終枚 ${lastPage}/${rows}行`, `${filled} lines · last page ${lastPage}/${rows}`)}
        {editor && <span className="genko-hint">{L("行をクリックで本文へ移動", "Click a line to go there")}</span>}
      </div>
      {printing > 0 && <PrintSheets key={printing} htmls={htmls} cols={cols} rows={rows} ink={genkoInk} onDone={donePrinting} />}
    </aside>
  );
}
