// Self-test of the editor in a browser (`pnpm dev`, then open
// `/?selftest`): Markdown goes in, is edited by nothing, and must come out
// the same (or at least stable, and with the same text). Results are written
// into #results, so that a headless browser can read them.

import { useEffect, useState } from "react";
import { Editor } from "@tiptap/core";
import { editorExtensions } from "../editor/setup";
import { editorMarkdown, normalizeMarkdown } from "../editor/cjk";
import { check } from "../lib/proof";
import { alignColumn, tsvToTable } from "../editor/table";
import { lockBody, unlockBody, isLocked } from "../lib/lock";
import { moveBlock, openBracket, unitCount } from "../editor/lines";

const CASES: [string, string][] = [
  ["paragraphs", "　朝の港には、まだ夜の名残があった。\n\n　「――まだ、帰ってこないのね」"],
  ["headings", "# 第一章\n\n## 一\n\n### 小見出し\n\n本文"],
  ["bold ascii", "This is **bold** and *italic* text."],
  ["bold japanese", "吾輩は**猫**である。"],
  ["bold japanese punctuation", "彼は**「待て」**と言った。"],
  ["bold japanese after punctuation", "そして、**ここ**が大事。"],
  ["italic japanese", "これは*強調*です。"],
  ["strike", "~~消した~~文字"],
  ["highlight", "ここは==大事==な所"],
  ["code", "インラインの `code` と\n\n```js\nconst a = 1;\n```"],
  ["bullet list", "- 一\n- 二\n  - 二の一\n- 三"],
  ["ordered list", "1. 一\n2. 二\n3. 三"],
  ["task list", "- [ ] やること\n- [x] 済んだこと"],
  ["quote", "> 引用の文章\n> 二行目"],
  ["hr", "前\n\n---\n\n後"],
  ["link", "[日食](https://example.com)のサイト"],
  ["image", "![写真](:/0123456789abcdef0123456789abcdef)"],
  ["table", "| 名前 | 年齢 |\n| --- | --- |\n| 澪 | 17 |\n| 母 | 45 |"],
  ["table with inline", "| 項目 | 説明 |\n| --- | --- |\n| **太字** | `code` |"],
  ["inline math", "質量とエネルギーは $E = mc^2$ の関係にある。"],
  ["block math", "$$\n\\int_0^1 x^2 \\, dx = \\frac{1}{3}\n$$"],
  ["ruby", "|灯台《とうだい》の光"],
  ["hard break", "一行目\\\n二行目"],
  ["escaped", "1\\. は番号ではない、\\*星\\*も"],
  ["dollar prices", "りんごは100円、オレンジは200円。"],
  ["html", "<span style=\"color:red\">赤い字</span>"],
  ["list-like paragraph", "\\- これは箇条書きではない"],
  ["heading-like paragraph", "\\# 見出しではない"],
  ["nested quote", "> 外\n>\n> > 内"],
  ["heading with bold", "## **強調**された見出し"],
  ["table pipes", "| a | b |\n| --- | --- |\n| x \\| y | z |"],
  ["bare url", "https://example.com 共有された本文"],
  ["bare url alone", "見て\n\nhttps://example.com/a_b?x=1&y=2"],
];

/** Cases the editor must keep (or change only in form): not "lossy". */
const LOSSLESS = new Set(CASES.map(([n]) => n).filter((n) => n !== "html"));

function normalizeText(s: string) {
  return s.replace(/\s+/g, "");
}

export function SelfTest() {
  const [results, setResults] = useState<string[]>([]);
  useEffect(() => {
    run().catch((e) => setResults([`FAILURES 1 (exception: ${e instanceof Error ? e.stack : e})`]));
  }, []);
  async function run() {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const editor = new Editor({ element: host, extensions: editorExtensions(), content: "" });
    const out: string[] = [];
    let failures = 0;
    for (const [name, md] of CASES) {
      editor.commands.setContent(md, { contentType: "markdown" });
      const text1 = editor.state.doc.textContent;
      const once = editorMarkdown(editor);
      editor.commands.setContent(once, { contentType: "markdown" });
      const twice = editorMarkdown(editor);
      const text2 = editor.state.doc.textContent;
      const same = once.trim() === md.trim();
      const stable = once === twice && normalizeText(text1) === normalizeText(text2);
      const status = same ? "SAME" : stable ? "CHANGED" : "UNSTABLE";
      if (status === "UNSTABLE") failures++;
      out.push(`${status}\t${name}\n  in:   ${JSON.stringify(md)}\n  out:  ${JSON.stringify(once)}${once !== twice ? `\n  out2: ${JSON.stringify(twice)}` : ""}`);
    }
    // Emphasis made in the editor survives a save and a reload.
    for (const [name, text, from, to] of [
      ["bold over brackets", "彼は「待て」と言った。", 2, 6],
      ["bold over a whole quote", "「待て」", 0, 4],
      ["bold ascii word", "say hello now", 4, 9],
    ] as [string, string, number, number][]) {
      editor.commands.setContent(`<p>${text}</p>`);
      editor.chain().setTextSelection({ from: from + 1, to: to + 1 }).toggleBold().run();
      const md = editorMarkdown(editor);
      editor.commands.setContent(md, { contentType: "markdown" });
      let bold = "";
      editor.state.doc.descendants((n) => {
        if (n.isText && n.marks.some((m) => m.type.name === "bold")) bold += n.text;
      });
      const ok = bold.length > 0 && text.slice(from, to).includes(bold);
      if (!ok) failures++;
      out.push(`${ok ? "OK" : "FAIL"}	${name}: ${JSON.stringify(md)} -> bold ${JSON.stringify(bold)}`);
    }
    // The lossy check: only real losses count.
    for (const [name, md] of CASES) {
      editor.commands.setContent(md, { contentType: "markdown" });
      const lossy = normalizeMarkdown(editorMarkdown(editor)) !== normalizeMarkdown(md);
      const expected = !LOSSLESS.has(name);
      if (lossy !== expected) {
        failures++;
        out.push(`FAIL	lossy check ${name}: ${lossy} (${JSON.stringify(normalizeMarkdown(editorMarkdown(editor)))} vs ${JSON.stringify(normalizeMarkdown(md))})`);
      }
    }
    // The writing check finds what it should.
    const proof = check([
      "雨が降った。風が吹いた。夜が来た。",
      "この料理は見れる。とてもとても楽しい。",
      "それは出来る。これもできる。",
    ]);
    const kinds = proof.issues.map((i) => i.kind).sort().join(",");
    const expected = "ending,ranuki,variant";
    const proofOk = ["ending", "ranuki", "variant"].every((k) => kinds.includes(k));
    if (!proofOk) failures++;
    out.push(`${proofOk ? "OK" : "FAIL"}	writing check: ${kinds} (expected ${expected})`);
    // Spreadsheet cells become a table; column alignment is saved.
    const table = tsvToTable("名前\t年齢\n澪\t17\n");
    editor.commands.setContent("");
    if (table) editor.commands.insertContent(table);
    let cell = 0;
    editor.state.doc.descendants((n, p) => {
      if (!cell && n.type.name === "tableCell") cell = p + 2;
    });
    editor.commands.setTextSelection(cell);
    alignColumn(editor, "center");
    const tableMd = editorMarkdown(editor);
    const tableOk = !!table && tableMd.includes("| 名前") && tableMd.includes(":-") && tableMd.includes("澪");
    if (!tableOk) failures++;
    out.push(`${tableOk ? "OK" : "FAIL"}\tpasted table, centered: ${JSON.stringify(tableMd)}`);

    // Brackets left open in committed input-method text.
    for (const [text, want] of [["「", "」"], ["「会話", "」"], ["「会話」", null], ["『題「中", "」"], ["(", null], ["です", null]] as [string, string | null][]) {
      const got = openBracket(text);
      const ok = got === want;
      if (!ok) failures++;
      out.push(`${ok ? "OK" : "FAIL"}\topen bracket ${JSON.stringify(text)} -> ${JSON.stringify(got)}`);
    }
    // Moving paragraphs and list items; numbering units.
    const moveCase = (name: string, md: string, cursorText: string, dir: -1 | 1, want: string) => {
      editor.commands.setContent(md, { contentType: "markdown" });
      let at = 0;
      editor.state.doc.descendants((n, p) => {
        if (!at && n.isText && n.text?.includes(cursorText)) at = p + 1;
      });
      editor.commands.setTextSelection(at);
      moveBlock(editor.view, dir);
      const got = editorMarkdown(editor).trim();
      const ok = got === want;
      if (!ok) failures++;
      out.push(`${ok ? "OK" : "FAIL"}\tmove ${name}: ${JSON.stringify(got)}`);
    };
    moveCase("paragraph up", "一\n\n二\n\n三", "三", -1, "一\n\n三\n\n二");
    moveCase("paragraph down", "一\n\n二\n\n三", "一", 1, "二\n\n一\n\n三");
    moveCase("list item up", "- あ\n- い\n- う", "う", -1, "- あ\n- う\n- い");
    moveCase("first stays", "一\n\n二", "一", -1, "一\n\n二");
    editor.commands.setContent("# 見出し\n\n段落\n\n- 一\n- 二\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n後", { contentType: "markdown" });
    const units = unitCount(editor.state.doc);
    const unitsOk = units === 6; // heading, paragraph, two list items, the table, the last paragraph
    if (!unitsOk) failures++;
    out.push(`${unitsOk ? "OK" : "FAIL"}\tnumbered units: ${units} (expected 6)`);

    setResults([...out, `FAILURES ${failures} (note lock pending)`]);
    // Locked notes: encrypted and opened again.
    const locked = await lockBody("秘密の本文", "pw", "🔒");
    const opened = await unlockBody(locked, "pw");
    const wrong = await unlockBody(locked, "nope").then(() => true, () => false);
    const lockOk = isLocked(locked) && !locked.includes("秘密") && opened === "秘密の本文" && !wrong;
    if (!lockOk) failures++;
    out.push(`${lockOk ? "OK" : "FAIL"}\tnote lock`);

    out.push(`FAILURES ${failures}`);
    editor.destroy();
    setResults(out);
  }
  return <pre id="results">{results.join("\n")}</pre>;
}
