import { lang } from "../i18n";
import { layoutLines } from "./genko";

// Measures for writing in Japanese.

const segmenter = new Intl.Segmenter("ja", { granularity: "grapheme" });

/** Characters as a reader counts them (graphemes), whitespace excluded. */
export function countChars(text: string): number {
  let n = 0;
  for (const { segment } of segmenter.segment(text)) {
    if (!/^\s+$/u.test(segment)) n++;
  }
  return n;
}

/** Characters including spaces (but not line breaks). */
export function countWithSpaces(text: string): number {
  let n = 0;
  for (const { segment } of segmenter.segment(text)) {
    if (segment !== "\n" && segment !== "\r\n") n++;
  }
  return n;
}

/**
 * Lines and pages on manuscript paper (原稿用紙), laid out as the vertical
 * preview does (see lib/genko.ts).
 */
export function manuscript(paragraphs: string[], cols = 20, rows = 20, indent = true) {
  const lines = layoutLines(paragraphs, cols, indent).length;
  return { lines, pages: lines === 0 ? 0 : Math.ceil(lines / rows) };
}

/** Reading time, at about 500 characters a minute. */
export function readingMinutes(chars: number): number {
  return chars === 0 ? 0 : Math.max(1, Math.round(chars / 500));
}

export function formatNumber(n: number): string {
  return n.toLocaleString("ja-JP");
}

const WEEKDAYS_JA = ["日", "月", "火", "水", "木", "金", "土"];

function hm(d: Date): string {
  return `${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** A short date for lists: the time today, yesterday, or the date. */
export function shortDate(ms: number): string {
  const d = new Date(ms);
  const now = new Date();
  const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const en = lang() === "en";
  if (sameDay(d, now)) return hm(d);
  if (sameDay(d, yesterday)) return en ? `Yesterday ${hm(d)}` : `昨日 ${hm(d)}`;
  if (en) {
    return d.toLocaleDateString("en-US", d.getFullYear() === now.getFullYear() ? { month: "short", day: "numeric" } : { year: "numeric", month: "short", day: "numeric" });
  }
  if (d.getFullYear() === now.getFullYear()) return `${d.getMonth() + 1}月${d.getDate()}日`;
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日`;
}

export function longDate(ms: number): string {
  const d = new Date(ms);
  if (lang() === "en") {
    return `${d.toLocaleDateString("en-US", { weekday: "short", year: "numeric", month: "short", day: "numeric" })} ${hm(d)}`;
  }
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日（${WEEKDAYS_JA[d.getDay()]}） ${hm(d)}`;
}

export function relativeTime(ms: number): string {
  const s = Math.round((Date.now() - ms) / 1000);
  const en = lang() === "en";
  if (s < 45) return en ? "just now" : "たった今";
  if (s < 3600) return en ? `${Math.round(s / 60)} min ago` : `${Math.round(s / 60)}分前`;
  if (s < 86400) return en ? `${Math.round(s / 3600)} h ago` : `${Math.round(s / 3600)}時間前`;
  return shortDate(ms);
}

/** Converts ruby notation (|漢字《かんじ》 or 漢字《かんじ》) to HTML, escaping the rest. */
export function rubyToHtml(text: string): string {
  const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const re = /[|｜]([^|｜《》\n]+)《([^《》\n]+)》|([\p{Script=Han}々〆ヵヶ]+)《([^《》\n]+)》/gu;
  let out = "";
  let last = 0;
  for (const m of text.matchAll(re)) {
    out += esc(text.slice(last, m.index));
    const base = m[1] ?? m[3];
    const ruby = m[2] ?? m[4];
    out += `<ruby>${esc(base)}<rt>${esc(ruby)}</rt></ruby>`;
    last = m.index! + m[0].length;
  }
  return out + esc(text.slice(last));
}

/** Plain text from Markdown, for exports and previews. */
export function markdownToPlain(md: string): string {
  return md
    .replace(/^```[\s\S]*?^```/gm, (b) => b.replace(/^```.*$/gm, ""))
    .replace(/^\s*\|[\s|:-]+\|\s*$/gm, "")
    .replace(/^\s*\|(.*)\|\s*$/gm, (_, row: string) => row.split("|").map((c) => c.trim()).join("　"))
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^>\s?/gm, "")
    .replace(/^\s*[-*+]\s+\[[ x]\]\s+/gm, "・")
    .replace(/^\s*[-*+]\s+/gm, "・")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/(\*\*|__|\*|_|~~|==|`)/g, "")
    .replace(/^\s*([-*_]\s*){3,}$/gm, "＊　＊　＊")
    .replace(/\\([\\`*_{}[\]()#+\-.!|])/g, "$1");
}
