// In-memory stand-in for the Rust commands, so that the interface can be
// opened in a plain browser during development (`pnpm dev`). Not used in the
// app itself.

import type { Character, ChapterMeta, ChapterInfo, Folder, Note, NoteKind, NoteSummary, Plot, Scope, WorkMeta, WorkOverview } from "../api";
import { countChars, markdownToPlain } from "../lib/text";

const now = Date.now();
const day = 86_400_000;
let seq = 0;
const id = () => (++seq).toString(16).padStart(32, "0");

interface F {
  id: string;
  title: string;
  parentId: string;
  icon: string;
  isDiary: boolean;
  deleted: number;
}

const extra = new Map<string, { pinned?: boolean; isTodo?: boolean; todoCompleted?: boolean; todoDue?: number; dueAllDay?: boolean; completedTime?: number; order?: number; tags?: string[] }>();

const folders: F[] = [];
const notes: (Note & { userUpdated: number })[] = [];

function folder(title: string, parentId = "", isDiary = false) {
  const f = { id: id(), title, parentId, isDiary, deleted: 0, icon: "" };
  folders.push(f);
  return f.id;
}

function note(parentId: string, title: string, body: string, age: number, extra: Partial<Note> = {}) {
  const t = now - age;
  notes.push({
    id: id(),
    parentId,
    title,
    body,
    createdTime: t - day,
    updatedTime: t,
    userUpdated: t,
    isConflict: false,
    deletedTime: 0,
    diaryDate: null,
    goal: null,
    pinned: false,
    isTodo: false,
    todoCompleted: false,
    todoDue: 0,
    dueAllDay: false,
    kind: null,
    ...extra,
  });
  return notes[notes.length - 1].id;
}

// Works: the novel is one, with characters, a plot and chapter details.
const workMeta = new Map<string, WorkMeta>();
const chapterMeta = new Map<string, ChapterMeta>();
const characters = new Map<string, Character>();
const plots = new Map<string, Plot>();
const blankCharacter = (): Character => ({ ruby: "", role: "", age: "", gender: "", appearance: "", personality: "", background: "", motivation: "", aliases: [], color: "", relations: [] });

const novel = folder("長編『夜明けの街』");
const part1 = folder("第一部　霧", novel);
const shorts = folder("短編");
const memo = folder("ネタ帳");
// Many nested notebooks, as a school year's notes would be.
{
  const school = folder("学校");
  for (const [term, subjects] of [["前期", ["数学", "物理", "英語", "情報", "国語"]], ["後期", ["数学", "化学", "英語", "プログラミング"]]] as const) {
    const t = folder(term, school);
    for (const name of subjects) {
      const sub = folder(name, t);
      if (name === "数学") ["課題", "小テスト", "ノート"].forEach((x) => folder(x, sub));
    }
  }
  const fest = folder("高専祭", school);
  ["シフト", "会計", "広報"].forEach((x) => folder(x, fest));
  // A very deep chain, for the tree's sideways scrolling.
  let deep = folder("学生会", school);
  for (const x of ["総務", "企画", "文化祭実行委員会", "ステージ班", "音響照明", "当日マニュアル"]) deep = folder(x, deep);
}
const diary = folder("日記", "", true);
let memoItems: { text: string; done: boolean }[] = [
  { text: "母の誕生日プレゼントを選ぶ（12/2）", done: false },
  { text: "図書館の本を返す", done: true },
];

note(
  part1,
  "第一章　港の灯",
  `　朝の港には、まだ夜の名残があった。霧は低く垂れこめ、|灯台《とうだい》の光だけが、ゆっくりと円を描いている。

　「――まだ、帰ってこないのね」

　母はそう言って、窓の外に目をやった。わたしは黙ってうなずき、湯気の立つ茶碗を両手で包んだ。父の船が出てから、もう三日が過ぎていた。

## 一

　港へ続く坂道は、石畳が濡れて黒く光っていた。……誰もいない。漁師たちはとうに沖へ出て、市場の女たちはまだ店を開けていない。その隙間のような時間が、わたしは昔から好きだった。

　坂の途中で、ふと足が止まった。霧の向こうに、見慣れない影がある。`,
  2 * 60_000,
  { goal: 8000 },
);
note(part1, "第二章　手紙", "　その手紙が届いたのは、雨の降る火曜日のことだった。\n\n　差出人の名前はなかった。", 3 * day);
extra.set(notes[0].id, { tags: ["推敲中", "長編"], pinned: true });
note(part1, "人物メモ", "- 澪（みお）：主人公。十七歳\n- 母：港町の食堂を営む\n- 父：漁師。三日前から行方不明", 6 * day);
note(memo, "資料メモ", "## 登場人物\n\n| 名前 | 年齢 | 役割 |\n| --- | --- | --- |\n| 澪 | 17 | 主人公 |\n| 母 | 45 | 食堂の店主 |\n\n## 物理の話\n\n灯台の光が届く距離は $d \\approx 3.57\\sqrt{h}$ で見積もれる。\n\n$$\n\\int_0^1 x^2 \\, dx = \\frac{1}{3}\n$$", 5 * day);
note(shorts, "Wi-Fiの届かない部屋", "　その部屋ではWi-Fiが届かなかった。iPhoneの画面には、2026年11月8日、午後3時の表示。\n\n　「OK、じゃあ10分だけ待つよ」と彼は言った。NASAのTシャツを着て、Hello worldと書かれたマグを持っている。\n\n　本当に？　嘘でしょ!?　ありえない!!\n\n　|東京《とうきょう》の|蒼穹《そうきゅう》は、いつもより低く見えた。電車はJR山手線、時速は約35km。彼女は、ゆっくりと、言葉を、選んだ。\n\n　資料はhttps://docs.google.com/spreadsheets/d/1zsYNc8oP1r7dRb7dqQex/editにまとめました。Discordのチャンネルhttps://discord.com/channels/1432285315317496にも貼っておきます。\n\n▼高専祭2日目シフト\n11:00-12:30\nhttps://nextcloud.kcct-gakuseikai.jp/index.php/f/985175\n\n" +"　霧の向こうに、見慣れない影がある。".repeat(40), 8 * day);
note(shorts, "雨の図書館","　閉館時間を過ぎても、その人は窓際の席から動かなかった。", 9 * day);
note(memo, "タイトル案", "- 夜明けの街\n- 霧の港\n- 灯台守の娘", 12 * day);
note(diary, "2026年11月3日（火）", "## 今日のできごと\n\n　午前中に第一章を書き直した。冒頭の霧の描写がようやく決まった気がする。\n\n## 明日やること\n\n- [ ] 第二章のプロットを見直す", day, { diaryDate: isoDay(-1) });
note(diary, "2026年11月1日（日）", "　散歩の途中で、古い灯台の写真を撮った。", 3 * day, { diaryDate: isoDay(-3) });
note(diary, "2026年10月31日（土）", "　雨。一日じゅう資料を読んでいた。", 4 * day, { diaryDate: isoDay(-4) });

// The novel as a work.
workMeta.set(novel, {
  logline: "行方不明の父を待つ港町の少女が、霧の夜に灯台で見た影の正体を追う。",
  synopsis: "",
  genre: "青春ミステリー",
  targetChars: 120000,
  deadline: isoDay(60),
});
const mio = note(novel, "澪", "　港町で母と暮らす。父の帰りを信じている。", 7 * day, { kind: "character" });
characters.set(mio, { ...blankCharacter(), ruby: "みお", role: "protagonist", age: "17", gender: "女", personality: "無口だが芯が強い", motivation: "父を見つけたい", color: "#d9622b", aliases: ["わたし"] });
const mother = note(novel, "母", "　港の食堂を営む。", 7 * day, { kind: "character" });
characters.set(mother, { ...blankCharacter(), role: "supporting", age: "45", color: "#6a8caf", relations: [{ target: mio, label: "娘" }] });
const shadow = note(novel, "影の男", "", 7 * day, { kind: "character" });
characters.set(shadow, { ...blankCharacter(), role: "antagonist", color: "#5c4a7a", aliases: ["見慣れない影"] });
chapterMeta.set(notes[0].id, { status: "revise", synopsis: "霧の朝、澪は坂道で見慣れない影を見る。", pov: mio });
chapterMeta.set(notes[1].id, { status: "draft", synopsis: "差出人のない手紙が届く。", pov: mio });
const plotId = note(novel, "プロット", "", 7 * day, { kind: "plot" });
plots.set(plotId, {
  template: "kishotenketsu",
  columns: ["起", "承", "転", "結"].map((title, i) => ({ id: `c${i + 1}`, title })),
  cards: [
    { id: "p1", column: "c1", title: "霧の朝の影", text: "父の不在。坂道で見慣れない影を見る。", chapter: notes[0].id, done: true },
    { id: "p2", column: "c1", title: "差出人のない手紙", text: "", chapter: notes[1].id, done: false },
    { id: "p3", column: "c2", title: "灯台守との出会い", text: "影の手がかりを持つ老人", chapter: null, done: false },
    { id: "p4", column: "c3", title: "父の船の真相", text: "", chapter: null, done: false },
  ],
});
note(novel, "港町の地図", "- 坂道：港と家を結ぶ\n- 灯台：岬の先", 8 * day, { kind: "material" });

// Sample mood, weather and events for the diary.
for (const [i, mood, weather] of [[0, "good", "partly"], [1, "great", "sunny"], [2, "low", "rain"]] as const) {
  const n = notes.filter((x) => x.diaryDate)[i];
  if (n) extra.set(n.id, { mood, weather, events: i === 0 ? [{ time: "09:30", text: "第一章の冒頭を書き直し" }, { time: "15:00", text: "編集のKさんと打ち合わせ" }] : [] } as never);
}

function isoDay(offset: number) {
  const d = new Date(now + offset * day);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function summary(n: Note): NoteSummary {
  const plain = markdownToPlain(n.body).split(/\s+/).join(" ");
  return {
    id: n.id,
    parentId: n.parentId,
    title: n.title,
    excerpt: plain.slice(0, 90),
    charCount: countChars(markdownToPlain(n.body)),
    createdTime: n.createdTime,
    updatedTime: n.updatedTime,
    isConflict: n.isConflict,
    deletedTime: n.deletedTime,
    diaryDate: n.diaryDate,
    goal: n.goal,
    pinned: false,
    isTodo: n.isTodo,
    todoCompleted: n.todoCompleted,
    todoDue: n.todoDue,
    dueAllDay: n.dueAllDay,
    completedTime: 0,
    order: 0,
    tags: [],
    kind: n.kind,
    ...extra.get(n.id),
  };
}

function workTree(rootId: string): { f: F; depth: number }[] {
  const out: { f: F; depth: number }[] = [];
  const walk = (f: F, depth: number) => {
    out.push({ f, depth });
    folders.filter((c) => c.parentId === f.id && !c.deleted).sort((a, b) => a.title.localeCompare(b.title)).forEach((c) => walk(c, depth + 1));
  };
  const root = folders.find((f) => f.id === rootId);
  if (root) walk(root, 0);
  return out;
}

function workOf(folderId: string): string | null {
  let f = folders.find((x) => x.id === folderId);
  while (f) {
    if (workMeta.has(f.id)) return f.id;
    f = folders.find((x) => x.id === f!.parentId);
  }
  return null;
}

function countMentions(text: string, names: string[]) {
  const sorted = names.filter(Boolean).sort((a, b) => b.length - a.length);
  let n = 0;
  for (let i = 0; i < text.length; ) {
    const hit = sorted.find((x) => text.startsWith(x, i));
    if (hit) {
      n++;
      i += hit.length;
    } else i++;
  }
  return n;
}

function overview(workId: string): WorkOverview {
  const root = folders.find((f) => f.id === workId)!;
  const tree = workTree(workId);
  const inWork = notes.filter((n) => !n.deletedTime && tree.some((t) => t.f.id === n.parentId));
  const chars = inWork.filter((n) => n.kind === "character");
  const roleOrder = ["protagonist", "antagonist", "main", "supporting", "minor", ""];
  chars.sort((a, b) => roleOrder.indexOf(characters.get(a.id)?.role ?? "") - roleOrder.indexOf(characters.get(b.id)?.role ?? ""));
  const names = chars.map((c) => [c.title, ...(characters.get(c.id)?.aliases ?? [])]);
  const mentions = chars.map(() => 0);
  const appears: string[][] = chars.map(() => []);
  const statusCounts: Record<string, number> = {};
  let total = 0;
  let count = 0;
  const sections = tree.map(({ f, depth }) => ({
    folderId: f.id,
    title: f.title,
    depth,
    chapters: inWork
      .filter((n) => n.parentId === f.id && !n.kind)
      .sort((a, b) => a.createdTime - b.createdTime)
      .map<ChapterInfo>((n) => {
        const meta = chapterMeta.get(n.id) ?? { status: "", synopsis: "", pov: null };
        const cast: [string, number][] = [];
        chars.forEach((c, i) => {
          const k = countMentions(`${n.title}\n${n.body}`, names[i]);
          if (k) {
            mentions[i] += k;
            appears[i].push(n.id);
            cast.push([c.id, k]);
          }
        });
        cast.sort((a, b) => b[1] - a[1]);
        statusCounts[meta.status] = (statusCounts[meta.status] ?? 0) + 1;
        const s = summary(n);
        total += s.charCount;
        count++;
        return { ...s, ...meta, characters: cast };
      }),
  }));
  return {
    id: workId,
    title: root.title,
    meta: workMeta.get(workId)!,
    sections,
    characters: chars.map((c, i) => {
      const ch = characters.get(c.id) ?? blankCharacter();
      return { id: c.id, name: c.title, ruby: ch.ruby, role: ch.role, color: ch.color, aliases: ch.aliases, excerpt: markdownToPlain(c.body).slice(0, 60), mentions: mentions[i], chapters: appears[i] };
    }),
    plotId: inWork.find((n) => n.kind === "plot")?.id ?? null,
    materials: inWork.filter((n) => n.kind === "material").map(summary),
    totalChars: total,
    chapterCount: count,
    statusCounts,
  };
}

let prefs: Record<string, unknown> = {};
const comments: { noteId: string; id: string; quote: string; before: string; text: string; createdTime: number; updatedTime: number; resolved: boolean }[] = [
  { noteId: "", id: "c1", quote: "見慣れない影", before: "霧の向こうに、", text: "ここで何が見えたのか、もう少しはっきり書く", createdTime: now - 3600_000, updatedTime: now - 3600_000, resolved: false },
  { noteId: "", id: "c2", quote: "湯気の立つ茶碗", before: "うなずき、", text: "季節感が出ていて良い", createdTime: now - 7200_000, updatedTime: now - 7200_000, resolved: false },
];
comments.forEach((c) => (c.noteId = notes[0].id));
const written: Record<string, number> = {};
for (let i = 0; i < 30; i++) if (i % 3 !== 1) written[isoDay(-i)] = Math.round(400 + 1600 * Math.abs(Math.sin(i * 1.7)));
written[isoDay(0)] = 1240;

/** `?latency=300`: every call answers that many ms late, like a slow device (for checking loading states). */
const latency = Number(new URLSearchParams(location.search).get("latency")) || 0;

export async function mockInvoke(cmd: string, args: Record<string, any> = {}): Promise<unknown> {
  if (latency) await new Promise((r) => setTimeout(r, latency));
  const live = notes.filter((n) => !n.deletedTime && !n.isConflict);
  switch (cmd) {
    case "move_folder": {
      const f = folders.find((x) => x.id === args.id);
      if (f) f.parentId = args.parentId;
      return null;
    }
    case "set_folder_icon": {
      const f = folders.find((x) => x.id === args.id);
      if (f) f.icon = args.emoji ?? "";
      return null;
    }
    case "list_folders":
      return folders
        .filter((f) => !f.deleted)
        .map<Folder>((f) => ({ id: f.id, title: f.title, parentId: f.parentId, icon: f.icon, isDiary: f.isDiary, isWork: workMeta.has(f.id), noteCount: live.filter((n) => n.parentId === f.id).length }));
    case "list_notes": {
      const scope = args.scope as Scope;
      const diaryIds = folders.filter((f) => f.isDiary).map((f) => f.id);
      const list =
        scope.kind === "pinned" ? live.filter((n) => extra.get(n.id)?.pinned)
        : scope.kind === "todo" ? live.filter((n) => extra.get(n.id)?.isTodo)
        : scope.kind === "tag" ? live.filter((n) => extra.get(n.id)?.tags?.includes(scope.id))
        : scope.kind === "all" ? live.filter((n) => !folders.find((f) => f.id === n.parentId)?.isDiary)
        : scope.kind === "folder" ? live.filter((n) => n.parentId === scope.id)
        : scope.kind === "diary" ? live.filter((n) => diaryIds.includes(n.parentId)).sort((a, b) => (b.diaryDate ?? "").localeCompare(a.diaryDate ?? ""))
        : scope.kind === "trash" ? notes.filter((n) => n.deletedTime)
        : scope.kind === "conflicts" ? notes.filter((n) => n.isConflict)
        : live.filter((n) => scope.query && `${n.title}\n${n.body}`.toLowerCase().includes(scope.query.toLowerCase()));
      return list.map(summary);
    }
    case "set_work":
      if (args.meta) workMeta.set(args.folderId, args.meta);
      else workMeta.delete(args.folderId);
      return null;
    case "work_overview":
      return overview(workOf(args.workId) ?? args.workId);
    case "set_note_kind": {
      const n = notes.find((x) => x.id === args.noteId)!;
      n.kind = args.kind;
      return null;
    }
    case "set_chapter_meta":
      chapterMeta.set(args.noteId, args.meta);
      return null;
    case "create_work_note": {
      const nid = note(args.workId, args.title, "", 0, { kind: args.kind as NoteKind });
      return { ...notes.find((n) => n.id === nid)! };
    }
    case "get_character":
      return characters.get(args.noteId) ?? blankCharacter();
    case "set_character": {
      characters.set(args.noteId, args.character);
      notes.find((x) => x.id === args.noteId)!.kind = "character";
      return null;
    }
    case "open_plot": {
      const ov = overview(args.workId);
      if (ov.plotId) return { noteId: ov.plotId, plot: plots.get(ov.plotId) };
      const plot: Plot = { template: "kishotenketsu", columns: ["起", "承", "転", "結"].map((title, i) => ({ id: `c${i + 1}`, title })), cards: [] };
      const nid = note(args.workId, "プロット", "", 0, { kind: "plot" });
      plots.set(nid, plot);
      return { noteId: nid, plot };
    }
    case "set_plot":
      plots.set(args.noteId, args.plot);
      return null;
    case "work_manuscript": {
      const ov = overview(args.workId);
      return ov.sections.map((s) => ({ title: s.depth === 0 ? "" : s.title, depth: s.depth, notes: s.chapters.map((c) => notes.find((n) => n.id === c.id)) }));
    }
    case "get_note": {
      const n = notes.find((x) => x.id === args.id);
      if (!n) throw "ノートが見つかりません";
      return { ...n, pinned: false, isTodo: false, todoCompleted: false, ...extra.get(n.id) };
    }
    case "list_tags": {
      const counts = new Map<string, number>();
      for (const e of extra.values()) for (const t of e.tags ?? []) counts.set(t, (counts.get(t) ?? 0) + 1);
      return [...counts].map(([title, noteCount]) => ({ id: title, title, noteCount }));
    }
    case "set_note_tags":
      extra.set(args.noteId, { ...extra.get(args.noteId), tags: args.titles });
      return null;
    case "set_note_pinned":
      extra.set(args.id, { ...extra.get(args.id), pinned: args.pinned });
      return null;
    case "set_note_todo":
      extra.set(args.id, { ...extra.get(args.id), isTodo: args.todo });
      return null;
    case "set_todo_completed":
      extra.set(args.id, { ...extra.get(args.id), isTodo: true, todoCompleted: args.completed, completedTime: args.completed ? Date.now() : 0 });
      return null;
    case "set_todo_due":
      extra.set(args.id, { ...extra.get(args.id), isTodo: true, todoDue: args.due, dueAllDay: args.allDay });
      return null;
    case "create_task": {
      note(args.parentId ?? folders[0].id, args.title, "", 0, { isTodo: true, todoDue: args.due, dueAllDay: args.allDay });
      return summary(notes[notes.length - 1]);
    }
    case "set_notes_order":
      (args.ids as string[]).forEach((id, i) => extra.set(id, { ...extra.get(id), order: 1000 - i }));
      return null;
    case "list_backlinks":
      return notes.filter((n) => n.body.includes(`:/${args.noteId}`)).map((n) => ({ id: n.id, title: n.title }));
    case "diary_entries": {
      const diaryIds = folders.filter((f) => f.isDiary).map((f) => f.id);
      return live
        .filter((n) => diaryIds.includes(n.parentId) && n.diaryDate && (!args.query || n.body.includes(args.query)))
        .sort((a, b) => (b.diaryDate ?? "").localeCompare(a.diaryDate ?? ""))
        .map((n) => ({ noteId: n.id, date: n.diaryDate, title: n.title, excerpt: markdownToPlain(n.body).replace(/\s+/g, " ").slice(0, 70), charCount: countChars(n.body), mood: (extra.get(n.id) as { mood?: string })?.mood ?? null, weather: (extra.get(n.id) as { weather?: string })?.weather ?? null, events: (extra.get(n.id) as { events?: unknown[] })?.events ?? [] }));
    }
    case "on_this_day":
      return [];
    case "diary_stats":
      return { streak: 2, longestStreak: 5, monthDays: 2, totalDays: 3, totalChars: 96, moods: [] };
    case "set_diary_meta":
      extra.set(args.noteId, { ...extra.get(args.noteId), mood: args.mood, weather: args.weather } as never);
      return null;
    case "set_diary_events":
      extra.set(args.noteId, { ...extra.get(args.noteId), events: args.events } as never);
      return null;
    case "get_diary_memo":
      return { noteId: "memo", items: memoItems };
    case "set_diary_memo":
      memoItems = args.items;
      return null;
    case "list_templates":
      return [];
    case "resource_files":
      return [];
    case "create_note": {
      note(args.parentId ?? shorts, args.title ?? "", args.body ?? "", 0);
      return { ...notes[notes.length - 1] };
    }
    case "save_note": {
      const n = notes.find((x) => x.id === args.id)!;
      const before = countChars(markdownToPlain(n.body));
      n.title = args.title;
      n.body = args.body;
      n.updatedTime = Date.now();
      const delta = countChars(markdownToPlain(n.body)) - before;
      if (delta > 0) written[isoDay(0)] = (written[isoDay(0)] ?? 0) + delta;
      return summary(n);
    }
    case "trash_note":
      notes.find((x) => x.id === args.id)!.deletedTime = Date.now();
      return null;
    case "restore_note":
      notes.find((x) => x.id === args.id)!.deletedTime = 0;
      return null;
    case "open_diary": {
      const existing = live.find((n) => n.diaryDate === args.date);
      if (existing) return { ...existing };
      note(diary, args.date, (prefs.diaryTemplate as string) ?? "", 0, { diaryDate: args.date });
      return { ...notes[notes.length - 1] };
    }
    case "diary_month":
      return live
        .filter((n) => n.diaryDate?.startsWith(args.month))
        .map((n) => ({ date: n.diaryDate, noteId: n.id, charCount: countChars(n.body) }));
    case "diary_streak":
      return 2;
    case "list_revisions":
      return [
        { id: "r2", time: now - 3 * 3600_000 },
        { id: "r1", time: now - 2 * day },
      ];
    case "revision_content":
      return { title: "第一章　港の灯", body: "　朝の港には、まだ夜の名残があった。\n\n　母はそう言って、窓の外を見た。" };
    case "list_comments":
      return comments
        .filter((c) => !args.noteId || c.noteId === args.noteId)
        .map((c) => ({ ...c, noteTitle: notes.find((n) => n.id === c.noteId)?.title ?? "" }));
    case "add_comment": {
      const c = { noteId: args.noteId, id: id(), quote: args.quote, before: args.before, text: args.text, createdTime: Date.now(), updatedTime: Date.now(), resolved: false };
      comments.unshift(c);
      return c;
    }
    case "update_comment": {
      const c = comments.find((x) => x.id === args.commentId)!;
      if (args.text != null) c.text = args.text;
      if (args.resolved != null) c.resolved = args.resolved;
      return null;
    }
    case "delete_comment":
      comments.splice(comments.findIndex((x) => x.id === args.commentId), 1);
      return null;
    case "get_stats": {
      const days: [string, number][] = [];
      for (let i = 29; i >= 0; i--) days.push([isoDay(-i), written[isoDay(-i)] ?? 0]);
      const history: [string, number][] = [];
      for (let i = 364; i >= 0; i--) history.push([isoDay(-i), i % 3 === 1 ? 0 : Math.round(300 + 1500 * Math.abs(Math.sin(i * 0.7)))]);
      return { history, today: written[isoDay(0)] ?? 0, streak: 2, days, totalNotes: live.length, totalChars: live.reduce((s, n) => s + countChars(n.body), 0) };
    }
    case "get_preferences":
      return prefs;
    case "set_preferences":
      prefs = args.preferences;
      return null;
    case "install_kind":
      return "dev";
    case "latest_release":
      return {
        version: "0.2.0",
        notes: "## 新機能\n- 作品ごとの登場人物・プロット・章の管理\n- 自動アップデート\n\n## 修正\n- URL を含むノートが Markdown 表示になる問題",
        url: "https://github.com/nuyu3n/nisshoku/releases/tag/v0.2.0",
        apkUrl: null,
        publishedAt: "2026-11-08T03:00:00Z",
      };
    case "get_sync_settings":
      return {
        target: { kind: "nextcloud", server: "https://cloud.example.com", user: "mio", folder: "Nisshoku" },
        status: { state: "idle", message: null, lastSync: now - 4 * 60_000, needsPassword: false, lastReport: null },
        gdriveAvailable: true,
        hasMasterPassword: false,
      };
    case "sync_now":
      return { state: "idle", message: null, lastSync: Date.now(), needsPassword: false, lastReport: null };
    default:
      return null;
  }
}
