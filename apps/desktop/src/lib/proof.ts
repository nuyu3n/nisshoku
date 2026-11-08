// A simple writing check for Japanese prose: long sentences, runs of the
// same sentence ending, ら抜き言葉, too many commas, doubled words, and
// inconsistent spellings. Plus a few measures of the text.

import { lang } from "../i18n";

export interface Issue {
  kind: "long" | "ending" | "ranuki" | "commas" | "double" | "variant";
  message: string;
  excerpt: string;
  /** Block index and character offsets within it. */
  block: number;
  from: number;
  to: number;
}

export interface Measures {
  sentences: number;
  averageLength: number;
  longest: number;
  kanjiRatio: number;
  paragraphs: number;
}

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

const VARIANTS: [string, string][] = [
  ["出来る", "できる"],
  ["下さい", "ください"],
  ["全て", "すべて"],
  ["沢山", "たくさん"],
  ["様々", "さまざま"],
  ["子供", "子ども"],
  ["私達", "私たち"],
  ["一つ", "ひとつ"],
  ["既に", "すでに"],
  ["更に", "さらに"],
  ["殆ど", "ほとんど"],
  ["何故", "なぜ"],
  ["頂く", "いただく"],
  ["事が出来", "ことができ"],
  ["如何", "いかが"],
  ["又は", "または"],
];

const RANUKI = /(見|来|寝|着|居|出|起き|食べ|考え|決め|忘れ|信じ|生き|借り|降り)れ(る|ない|ます|た|て|れば)/g;

/** Sentences of a block, with their offsets. */
function sentences(text: string): { text: string; from: number }[] {
  const out: { text: string; from: number }[] = [];
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if ("。！？!?".includes(c)) {
      // Keep a closing bracket with its sentence: 「……。」
      let end = i + 1;
      while (end < text.length && "」』）)".includes(text[end])) end++;
      out.push({ text: text.slice(start, end), from: start });
      start = end;
      i = end - 1;
    }
  }
  if (start < text.length && text.slice(start).trim()) out.push({ text: text.slice(start), from: start });
  return out;
}

export function check(blocks: string[], maxSentence = 80): { issues: Issue[]; measures: Measures } {
  const issues: Issue[] = [];
  let count = 0;
  let total = 0;
  let longest = 0;
  let kanji = 0;
  let letters = 0;
  let endings: { ending: string; block: number; from: number; to: number; text: string }[] = [];

  blocks.forEach((text, block) => {
    for (const ch of text) {
      if (/\p{Script=Han}/u.test(ch)) kanji++;
      if (/[\p{L}]/u.test(ch)) letters++;
    }
    for (const s of sentences(text)) {
      const body = s.text.trim();
      if (!body) continue;
      const len = [...body].length;
      count++;
      total += len;
      longest = Math.max(longest, len);
      if (len > maxSentence) {
        issues.push({ kind: "long", message: L(`文が長めです（${len}字）`, `Long sentence (${len} characters)`), excerpt: body, block, from: s.from, to: s.from + s.text.length });
      }
      const commas = (body.match(/[、，,]/g) ?? []).length;
      if (commas >= 5) {
        issues.push({ kind: "commas", message: L(`読点が多めです（${commas}個）`, `Many commas (${commas})`), excerpt: body, block, from: s.from, to: s.from + s.text.length });
      }
      const ending = body.replace(/[。！？!?」』）)\s]+$/u, "").slice(-1);
      if (ending) endings.push({ ending, block, from: s.from, to: s.from + s.text.length, text: body });
    }
    for (const m of text.matchAll(RANUKI)) {
      issues.push({ kind: "ranuki", message: L(`ら抜き言葉かもしれません（「${m[0]}」→「${m[1]}られ${m[2]}」）`, `Possible ら-dropping: ${m[0]}`), excerpt: m[0], block, from: m.index!, to: m.index! + m[0].length });
    }
    for (const m of text.matchAll(/([\p{Script=Han}\p{Script=Katakana}ー]{2,}|[ぁ-ん]{3,})\1/gu)) {
      issues.push({ kind: "double", message: L(`同じ言葉が続いています（「${m[1]}」）`, `Repeated word: ${m[1]}`), excerpt: m[0], block, from: m.index!, to: m.index! + m[0].length });
    }
    for (const m of text.matchAll(/([がをにはへでとのも])\1(?![ぁ-ん])/g)) {
      issues.push({ kind: "double", message: L(`助詞が重なっています（「${m[0]}」）`, `Doubled particle: ${m[0]}`), excerpt: m[0], block, from: m.index!, to: m.index! + m[0].length });
    }
  });

  // Three or more sentences in a row ending the same way.
  let run: typeof endings = [];
  const flush = () => {
    if (run.length >= 3) {
      const first = run[0];
      issues.push({
        kind: "ending",
        message: L(`文末が「${first.ending}」で${run.length}回続いています`, `${run.length} sentences in a row end with “${first.ending}”`),
        excerpt: run.map((r) => r.text).join(" "),
        block: first.block,
        from: first.from,
        to: first.to,
      });
    }
    run = [];
  };
  for (const e of endings) {
    if (run.length && run[run.length - 1].ending !== e.ending) flush();
    run.push(e);
  }
  flush();
  endings = [];

  // Spellings used both ways in the note.
  const all = blocks.join("\n");
  for (const [a, b] of VARIANTS) {
    if (all.includes(a) && all.includes(b)) {
      const block = blocks.findIndex((x) => x.includes(a));
      const from = blocks[block].indexOf(a);
      issues.push({ kind: "variant", message: L(`表記ゆれ：「${a}」と「${b}」が混在しています`, `Inconsistent spelling: ${a} / ${b}`), excerpt: a, block, from, to: from + a.length });
    }
  }

  issues.sort((x, y) => x.block - y.block || x.from - y.from);
  return {
    issues,
    measures: {
      sentences: count,
      averageLength: count ? Math.round(total / count) : 0,
      longest,
      kanjiRatio: letters ? Math.round((kanji / letters) * 100) : 0,
      paragraphs: blocks.filter((b) => b.trim()).length,
    },
  };
}
