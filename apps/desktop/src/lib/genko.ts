// Layout on manuscript paper (原稿用紙): text placed cell by cell, one
// character to a cell, following the usual rules of writing on it:
//
// - each paragraph starts on a new line, one cell down (not before 「『（);
// - half-width text keeps to the grid, two characters to a cell: up to two
//   stand side by side (縦中横: 10, OK, !?); longer numbers and short capitals
//   (NASA) stand one to a cell; words, phrases and URLs lie on their side
//   (横倒し), two characters in each cell, and break at any cell like
//   Japanese text (a short word is kept on one line);
// - 、。 and closing brackets never start a line: they hang below the last
//   cell of the line before (ぶら下げ);
// - ruby (|漢字《かんじ》) sits beside its characters, kept on one line.

export type CellKind = "tcy" | "up" | "rot";
export interface Cell {
  t: string;
  k?: CellKind;
}
export interface Span {
  /** First cell and number of cells. */
  at: number;
  len: number;
  text: string;
}
export interface Line {
  cells: Cell[];
  ruby: Span[];
  /** Punctuation hanging below the last cell. */
  hang: Cell[];
  /** Index of the paragraph the line belongs to (-1 for the title block). */
  para: number;
  /** Where the line starts in its paragraph (characters of the paragraph as given, about). */
  from: number;
}

const segmenter = new Intl.Segmenter("ja", { granularity: "grapheme" });
const RUBY_RE = /[|｜]([^|｜《》\n]+)《([^《》\n]+)》|([\p{Script=Han}々〆ヵヶ]+)《([^《》\n]+)》/gu;
/** May hang below the last cell. */
const HANG = new Set("、。，．」』）】〕〉》〙〗’”");
/** A paragraph starting with these is not indented. */
const NO_INDENT = new Set("「『（〈《【〔“‘");
/** Full-width forms with vertical shapes, for half-width symbols standing alone. */
const WIDE: Record<string, string> = { ",": "、", ".": "．", "!": "！", "?": "？", "(": "（", ")": "）", "[": "［", "]": "］", "-": "－", "~": "〜", "'": "’", '"': "”" };

interface Unit {
  cells: Cell[];
  ruby?: string;
  /** Several cells kept on one line when they fit on one (a ruby group, a short word). */
  keep?: boolean;
  /** Where the unit starts in the paragraph. */
  src: number;
}

function asciiUnits(raw: string, out: Unit[], src: number) {
  const run = raw.replace(/ {2,}/g, " ").trim();
  if (!run) {
    // A space between Japanese words: one empty cell.
    out.push({ cells: [{ t: "　" }], src });
    return;
  }
  const one = (c: string): Cell => (/[0-9]/.test(c) ? { t: c, k: "tcy" } : /[A-Za-z]/.test(c) ? { t: c, k: "up" } : { t: WIDE[c] ?? String.fromCharCode(c.charCodeAt(0) + 0xfee0) });
  if (run.length === 1) {
    out.push({ cells: [one(run)], src });
  } else if (run.length === 2 && !run.includes(" ")) {
    // 10, OK, !? side by side in one cell.
    out.push({ cells: [{ t: run, k: "tcy" }], src });
  } else if (/^[0-9]+$/.test(run) || /^[0-9]+([-/.:][0-9]+)+$/.test(run) || (/^[A-Z]+$/.test(run) && run.length <= 5)) {
    // 2026, NASA: one to a cell, upright. So are dates and times (2026-11-08,
    // 17:23), their separators full-width, as written by hand.
    [...run].forEach((c, i) => out.push({ cells: [one(c)], src: src + i }));
  } else {
    // Sideways, two characters a cell; a word of up to four cells is not broken.
    const cells = chunk(run, 2).map((t): Cell => ({ t, k: "rot" }));
    out.push({ cells, keep: cells.length <= 4, src });
  }
}

function plainUnits(s: string, out: Unit[], start: number) {
  let ascii = "";
  let asciiAt = 0;
  let at = start;
  const flush = () => {
    if (ascii) asciiUnits(ascii, out, asciiAt);
    ascii = "";
  };
  for (const { segment } of segmenter.segment(s)) {
    if (/^[\x20-\x7e]$/.test(segment)) {
      if (!ascii) asciiAt = at;
      ascii += segment;
    } else {
      flush();
      out.push({ cells: [{ t: segment === "\t" ? "　" : segment }], src: at });
    }
    at += segment.length;
  }
  flush();
}

function units(paragraph: string, start: number): Unit[] {
  const out: Unit[] = [];
  let last = 0;
  for (const m of paragraph.matchAll(RUBY_RE)) {
    plainUnits(paragraph.slice(last, m.index), out, start + last);
    const base: Unit[] = [];
    plainUnits(m[1] ?? m[3], base, 0);
    out.push({ cells: base.flatMap((u) => u.cells), ruby: m[2] ?? m[4], keep: true, src: start + m.index! });
    last = m.index! + m[0].length;
  }
  plainUnits(paragraph.slice(last), out, start + last);
  return out;
}

function chunk(s: string, n: number): string[] {
  const out: string[] = [];
  for (let i = 0; i < s.length; i += n) out.push(s.slice(i, i + n));
  return out;
}

/** Splits `text` into `parts` pieces of about the sizes given. */
function splitRuby(text: string, sizes: number[]): string[] {
  const chars = [...segmenter.segment(text)].map((s) => s.segment);
  const total = sizes.reduce((a, b) => a + b, 0);
  const out: string[] = [];
  let used = 0;
  let acc = 0;
  for (const n of sizes) {
    acc += n;
    const end = Math.round((chars.length * acc) / total);
    out.push(chars.slice(used, end).join(""));
    used = end;
  }
  return out;
}

/**
 * Lines of `cols` cells. `paragraphs` are plain text; an empty one is an
 * empty line. `para` of each line is the paragraph index plus `firstPara`.
 */
export function layoutLines(paragraphs: string[], cols: number, indent = true, firstPara = 0): Line[] {
  const lines: Line[] = [];
  paragraphs.forEach((p, pi) => {
    const trimmed = p.trim();
    const us = units(trimmed, p.length - p.trimStart().length);
    const blank = (from: number): Line => ({ cells: [], ruby: [], hang: [], para: pi + firstPara, from });
    let line = blank(0);
    /** Where the text placed next starts (the start of a line broken there). */
    let at = 0;
    const newLine = () => {
      lines.push(line);
      line = blank(at);
    };
    const first = us[0];
    if (indent && first && !(first.cells[0] && NO_INDENT.has(first.cells[0].t))) line.cells.push({ t: "　" });

    for (const u of us) {
      at = u.src;
      const n = u.cells.length;
      if (n === 0) continue;
      if (line.cells.length + n > cols) {
        // Punctuation that would start a line hangs below this one.
        if (n === 1 && !u.ruby && HANG.has(u.cells[0].t) && line.cells.length === cols && line.hang.length < 2) {
          line.hang.push(u.cells[0]);
          continue;
        }
        // A character, ruby group or short word goes to the next line whole
        // (longer sideways text fills this line first, below).
        if ((n === 1 || u.keep) && n <= cols) newLine();
      }
      if (n === 1 && !u.ruby) {
        line.cells.push(u.cells[0]);
        continue;
      }
      if (!u.ruby) {
        // Sideways text: cell by cell, onto the next line where this one ends.
        u.cells.forEach((c, i) => {
          if (line.cells.length >= cols) {
            at = u.src + i * 2;
            newLine();
          }
          line.cells.push(c);
        });
        continue;
      }
      const pieces: { line: Line; at: number; len: number }[] = [];
      let start = line.cells.length;
      for (const c of u.cells) {
        if (line.cells.length >= cols) {
          pieces.push({ line, at: start, len: line.cells.length - start });
          newLine();
          start = 0;
        }
        line.cells.push(c);
      }
      pieces.push({ line, at: start, len: line.cells.length - start });
      if (u.ruby) {
        const texts = splitRuby(u.ruby, pieces.map((x) => x.len));
        pieces.forEach((x, i) => texts[i] && x.line.ruby.push({ at: x.at, len: x.len, text: texts[i] }));
      }
    }
    lines.push(line);

  });
  return lines;
}

export function pagesOf<T>(lines: T[], rows: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < lines.length; i += rows) out.push(lines.slice(i, i + rows));
  return out;
}
