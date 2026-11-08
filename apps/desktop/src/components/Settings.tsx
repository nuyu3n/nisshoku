import { useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, type InstallKind, type SyncSettings } from "../api";
import { appVersion, checkForUpdate, installUpdate, openReleasePage, useUpdate } from "../lib/updates";
import { report, useStore, type Preferences } from "../store";
import { lang } from "../i18n";
import { formatNumber, relativeTime } from "../lib/text";
import { Icon } from "./Icon";
import { NumberField, Select } from "./Controls";
import appIcon from "../assets/app-icon.png";
import { dialogs } from "./Dialogs";
import { isMobile } from "../lib/mobile";
import { useSlideDir } from "../lib/motion";

/** Settings texts live next to their controls: Japanese, English. */
function L(ja: string, en: string): string {
  return lang() === "en" ? en : ja;
}

const TABS: [string, string, [string, string]][] = [
  ["general", "settings", ["一般", "General"]],
  ["editor", "pen", ["エディタ", "Editor"]],
  ["writing", "target", ["執筆", "Writing"]],
  ["diary", "diary", ["日記", "Diary"]],
  ["list", "list", ["ノート一覧", "Note list"]],
  ["sync", "cloud", ["同期", "Sync"]],
  ["data", "data", ["データ", "Data"]],
  ["stats", "chart", ["統計", "Statistics"]],
  ["keys", "type", ["ショートカット", "Shortcuts"]],
  ["about", "download", ["アップデート", "Updates"]],
];

function Switch({ on, onChange }: { on: boolean; onChange: (v: boolean) => void }) {
  return <button className={`switch${on ? " on" : ""}`} role="switch" aria-checked={on} onClick={() => onChange(!on)} />;
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="field">
      <div className="lbl">
        {label}
        {hint && <small>{hint}</small>}
      </div>
      {children}
    </div>
  );
}

function Segmented<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="segmented">
      {options.map(([v, label]) => (
        <button key={v} className={value === v ? "on" : ""} onClick={() => onChange(v)}>
          {label}
        </button>
      ))}
    </div>
  );
}

function usePref<K extends keyof Preferences>(key: K): [Preferences[K], (v: Preferences[K]) => void] {
  const value = useStore((s) => s.prefs[key]);
  return [value, (v) => useStore.getState().setPrefs({ [key]: v } as Partial<Preferences>)];
}

function NumberInput({ value, onChange, min, max, step = 1, suffix }: { value: number; onChange: (v: number) => void; min: number; max: number; step?: number; suffix?: string }) {
  return (
    <span className="number-input">
      <NumberField value={value} min={min} max={max} step={step} onChange={(v) => v !== null && onChange(v)} />
      {suffix && <span>{suffix}</span>}
    </span>
  );
}

type Prefs = Preferences;

function SwitchPref<K extends { [P in keyof Prefs]: Prefs[P] extends boolean ? P : never }[keyof Prefs]>({ pref, label, hint }: { pref: K; label: string; hint?: string }) {
  const [value, set] = usePref(pref);
  return (
    <Field label={label} hint={hint}>
      <Switch on={value as boolean} onChange={(v) => set(v as Prefs[K])} />
    </Field>
  );
}

function NumberPref<K extends { [P in keyof Prefs]: Prefs[P] extends number ? P : never }[keyof Prefs]>({ pref, min, max, step, suffix }: { pref: K; min: number; max: number; step?: number; suffix?: string }) {
  const [value, set] = usePref(pref);
  return <NumberInput value={value as number} onChange={(v) => set(v as Prefs[K])} min={min} max={max} step={step} suffix={suffix} />;
}

function SelectPref<K extends keyof Prefs>({ pref, options, after }: { pref: K; options: [Prefs[K], string][]; after?: () => void }) {
  const [value, set] = usePref(pref);
  // Values are strings or numbers; the select shows them by their labels.
  const keyed = options.map(([v, label]) => [String(v), label] as const);
  return (
    <Select
      value={String(value)}
      options={keyed}
      onChange={(k) => {
        const option = options.find(([v]) => String(v) === k);
        if (option) set(option[0]);
        after?.();
      }}
    />
  );
}

const ACCENTS: [Prefs["accent"], string, [string, string]][] = [
  ["orange", "#d9622b", ["原稿用紙のオレンジ", "Manuscript orange"]],
  ["vermilion", "#c8392b", ["朱", "Vermilion"]],
  ["indigo", "#3f5ea8", ["藍", "Indigo"]],
  ["green", "#4b8a5e", ["若竹", "Bamboo green"]],
  ["sumi", "#4a4540", ["墨", "Ink"]],
];

function AccentPicker() {
  const [accent, setAccent] = usePref("accent");
  return (
    <div className="accent-swatches">
      {ACCENTS.map(([id, color, [ja, en]]) => (
        <button key={id} className={`swatch${accent === id ? " on" : ""}`} style={{ background: color }} title={L(ja, en)} aria-label={L(ja, en)} onClick={() => setAccent(id)} />
      ))}
    </div>
  );
}

/** A text preference saved when the field loses focus. */
function TextPref({ value, onSave, placeholder }: { value: string; onSave: (v: string) => void; placeholder?: string }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <input
      className="input"
      value={draft}
      placeholder={placeholder}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => draft.trim() && draft.trim() !== value && onSave(draft.trim())}
      onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && (e.target as HTMLInputElement).blur()}
    />
  );
}

// -------------------------------------------------------------------------

function GeneralTab() {
  const [language, setLanguage] = usePref("language");
  const [theme, setTheme] = usePref("theme");
  const [notebook, setNotebook] = usePref("defaultNotebookName");
  return (
    <>
      <h3>{L("一般", "General")}</h3>
      <Field label={L("表示言語", "Language")}>
        <Segmented value={language} options={[["ja", "日本語"], ["en", "English"]]} onChange={setLanguage} />
      </Field>
      <Field label={L("テーマ", "Theme")}>
        <Segmented
          value={theme}
          options={[
            ["system", L("システムに合わせる", "System")],
            ["light", L("ライト", "Light")],
            ["dark", L("ダーク", "Dark")],
          ]}
          onChange={setTheme}
        />
      </Field>
      <Field label={L("アクセントカラー", "Accent color")}>
        <AccentPicker />
      </Field>
      <Field label={L("画面の拡大率", "Zoom")} hint={L("アプリ全体の文字や部品の大きさです。", "The size of the whole interface.")}>
        <SelectPref
          pref="zoom"
          options={[80, 90, 100, 110, 125, 150].map((z) => [z, `${z}%`])}
        />
      </Field>
      <Field
        label={L("起動時に開くもの", "On start, open")}
        hint={L("スマホでは、ここで選んだ画面から始まります。", "On phones, the app starts on this screen.")}
      >
        <SelectPref
          pref="startup"
          options={[
            ["last", L("最後に開いていたノート", "The last note")],
            ["diary", L("今日の日記", "Today’s diary entry")],
            ["list", L("ノート一覧", "The note list")],
            ["none", L("何も開かない（ノートブックの一覧）", "Nothing (the notebooks)")],
          ]}
        />
      </Field>
      <Field
        label={L("標準のノートブック名", "Default notebook name")}
        hint={L("ノートブックがひとつもないときに、自動で作るノートブックの名前です。", "Used for the notebook created automatically when there is none.")}
      >
        <TextPref value={notebook} onSave={setNotebook} placeholder="Notes" />
      </Field>
    </>
  );
}

function EditorTab() {
  const [font, setFont] = usePref("editorFont");
  const [size, setSize] = usePref("fontSize");
  const [line, setLine] = usePref("lineHeight");
  const [width, setWidth] = usePref("textWidth");
  const [indent, setIndent] = usePref("indentParagraphs");
  const [typewriter, setTypewriter] = usePref("typewriter");
  const [smart, setSmart] = usePref("smartPunctuation");
  const [dim, setDim] = usePref("dimOthers");
  return (
    <>
      <h3>{L("エディタ", "Editor")}</h3>
      <Field label={L("書体", "Typeface")}>
        <Segmented value={font} options={[["serif", L("明朝", "Serif")], ["sans", L("ゴシック", "Sans-serif")]]} onChange={setFont} />
      </Field>
      <Field label={L("文字サイズ", "Font size")}>
        <NumberInput value={size} onChange={setSize} min={12} max={28} suffix="px" />
      </Field>
      <Field label={L("行の高さ", "Line height")}>
        <NumberInput value={line} onChange={setLine} min={1.3} max={2.6} step={0.05} suffix={L("倍", "×")} />
      </Field>
      <Field label={L("本文の幅", "Text width")} hint={L("1行に並ぶおおよその文字数です。", "About how many characters fit on a line.")}>
        <NumberInput value={width} onChange={setWidth} min={20} max={60} suffix={L("字", "chars")} />
      </Field>
      <Field label={L("段落の頭を1字下げる", "Indent paragraphs")} hint={L("表示だけの設定で、保存される文章は変わりません。", "Only changes the display, not the saved text.")}>
        <Switch on={indent} onChange={setIndent} />
      </Field>
      <Field label={L("タイプライタースクロール", "Typewriter scrolling")} hint={L("入力中の行を、常に画面の中ほどに保ちます。", "Keeps the line you are typing in the middle of the screen.")}>
        <Switch on={typewriter} onChange={setTypewriter} />
      </Field>
      <Field label={L("記号を自動で変換する", "Smart punctuation")} hint={L("「...」を「…」に、「--」を「――」に変えます。", "Turns “...” into “…” and “--” into “――”.")}>
        <Switch on={smart} onChange={setSmart} />
      </Field>
      <SwitchPref pref="lineNumbers" label={L("行番号を表示する", "Show line numbers")} hint={L("リッチ表示では段落ごと、Markdown 表示と日記では行ごとに番号を振ります。Ctrl+G で番号の場所へ移動できます。", "Numbers paragraphs in rich text and lines in Markdown and the diary. Ctrl+G jumps to a number.")} />
      <SwitchPref pref="autoPair" label={L("括弧を自動で閉じる", "Close brackets automatically")} hint={L("開き括弧（「『（ など）を打つと、閉じ括弧も入ります。文字を選んでから打つと、その文字を括弧で囲みます。", "Typing an opening bracket (「, 『, ( …) adds the closing one. With text selected, the brackets go around it.")} />
      <Field label={L("集中モードで、書いている段落だけを目立たせる", "In focus mode, dim all but the current paragraph")}>
        <Switch on={dim} onChange={setDim} />
      </Field>
      <SwitchPref pref="fullWidth" label={L("本文を横幅いっぱいに広げる", "Use the full width")} hint={L("「本文の幅」の設定より優先されます。", "Overrides the text width.")} />
      <SwitchPref pref="bubbleToolbar" label={L("文字を選んだときにツールバーを出す", "Show a toolbar on selection")} />
      <SwitchPref pref="slashMenu" label={L("「/」でブロック挿入メニューを出す", "Open the block menu with “/”")} />
      <SwitchPref pref="spellcheck" label={L("スペルチェック", "Spell checking")} hint={L("おもに英語の綴りの誤りに下線を引きます。", "Underlines misspelled (mostly English) words.")} />
    </>
  );
}

function WritingTab() {
  const [goal, setGoal] = usePref("dailyGoal");
  const [cols, setCols] = usePref("manuscriptCols");
  const [rows, setRows] = usePref("manuscriptRows");
  return (
    <>
      <h3>{L("執筆", "Writing")}</h3>
      <Field label={L("1日の目標", "Daily goal")} hint={L("0にすると目標なしになります。進み具合はサイドバーとステータスバーに出ます。", "0 for none. Progress shows in the sidebar and the status bar.")}>
        <NumberInput value={goal} onChange={setGoal} min={0} max={100000} step={100} suffix={L("字", "chars")} />
      </Field>
      <SwitchPref
        pref="highlightCharacters"
        label={L("登場人物の名前に色を付ける", "Colour characters’ names")}
        hint={L("作品のノートで、人物設定にある名前と別名に下線を引きます。Ctrl を押しながらクリックすると人物設定を開きます。", "In a work’s notes, names and other names from the character sheets are underlined. Ctrl+click opens the character.")}
      />
      <Field label={L("原稿用紙の字詰め", "Manuscript paper")} hint={L("原稿用紙換算と縦書きプレビューに使います。一般的なのは20字×20行です。", "For page counts and the vertical preview. The usual size is 20 × 20.")}>
        <span className="number-pair">
          <NumberInput value={cols} onChange={setCols} min={10} max={60} suffix={L("字 ×", "chars ×")} />
          <NumberInput value={rows} onChange={setRows} min={5} max={60} suffix={L("行", "lines")} />
        </span>
      </Field>
      <SwitchPref pref="countSpaces" label={L("空白も文字数に数える", "Count spaces as characters")} />
      <Field label={L("読む速さ", "Reading speed")} hint={L("読了時間の計算に使います。", "Used for reading times.")}>
        <NumberPref pref="readingSpeed" min={100} max={3000} step={50} suffix={L("字／分", "chars/min")} />
      </Field>
      <Field label={L("執筆タイマーの長さ", "Writing timer")}>
        <NumberPref pref="timerMinutes" min={1} max={240} suffix={L("分", "min")} />
      </Field>
      <h4>{L("ステータスバー", "Status bar")}</h4>
      <SwitchPref pref="statusManuscript" label={L("原稿用紙の枚数", "Manuscript pages")} />
      <SwitchPref pref="statusReading" label={L("読了時間", "Reading time")} />
      <SwitchPref pref="statusToday" label={L("今日書いた文字数", "Characters written today")} />
      <h4>{L("書き方のヒント", "Tips")}</h4>
      <ul className="tips">
        <li>{L("行頭で「/」を押すと、見出し・場面転換・ルビなどを挿入できます。", "Type “/” at the start of a line to insert headings, scene breaks, ruby and more.")}</li>
        <li>
          {L("ルビは", "Write ruby as")} <code>|漢字《かんじ》</code> {L("と書きます。縦書きプレビューと HTML の書き出しでルビとして表示されます。", "— it shows as ruby in the vertical preview and HTML exports.")}
        </li>
        <li>{L("「# 」で見出し、「- 」で箇条書き、「> 」で引用になります。", "“# ” starts a heading, “- ” a list, “> ” a quote.")}</li>
        <li>{L("文章を選ぶと、太字やコメントのボタンが出ます。", "Select text for bold, highlight and comment buttons.")}</li>
        <li>{L("書いた文章は自動で履歴に残り、いつでも前の版に戻せます。", "Earlier versions are kept automatically; you can go back to any of them.")}</li>
      </ul>
    </>
  );
}

function DiaryTab() {
  const [template, setTemplate] = usePref("diaryTemplate");
  const [format, setFormat] = usePref("diaryTitleFormat");
  const [name, setName] = usePref("diaryNotebookName");
  const folders = useStore((s) => s.folders);
  const diary = folders.find((f) => f.isDiary);
  const [draft, setDraft] = useState(template);
  const example = L("## 今日のできごと\n\n\n## 思ったこと\n\n\n## 明日やること\n\n- [ ] \n", "## What happened\n\n\n## Thoughts\n\n\n## Tomorrow\n\n- [ ] \n");
  return (
    <>
      <h3>{L("日記", "Diary")}</h3>
      <Field label={L("日記帳の名前", "Diary notebook name")} hint={L("日記を入れるノートブックの名前です。", "The notebook that holds the diary entries.")}>
        <TextPref
          value={diary?.title ?? name}
          placeholder="Diary"
          onSave={async (v) => {
            setName(v);
            if (diary) {
              await api.renameFolder(diary.id, v).catch(report);
              await useStore.getState().loadFolders();
            }
          }}
        />
      </Field>
      <Field label={L("タイトルの形式", "Entry title")} hint={L("新しく作る日記のタイトルです。", "The title of new entries.")}>
        <Select
          value={format}
          options={[["iso", "2026-11-04"], ["isoWeekday", "2026-11-04 Wed"], ["ja", "2026年11月4日（水）"]] as const}
          onChange={(v) => setFormat(v)}
        />
      </Field>
      <Field label={L("週の始まり", "Week starts on")}>
        <SelectPref pref="weekStart" options={[["sun", L("日曜日", "Sunday")], ["mon", L("月曜日", "Monday")]]} />
      </Field>
      <h4>{L("テンプレート", "Template")}</h4>
      <p className="muted">{L("新しい日記に最初から入れておく文章です（Markdown）。", "Text that new entries start with (Markdown).")}</p>
      <textarea className="input" rows={8} value={draft} placeholder={example} onChange={(e) => setDraft(e.target.value)} onBlur={() => setTemplate(draft)} />
      <div className="row-buttons">
        <button className="btn" onClick={() => setDraft(example)}>
          {L("例を入れる", "Use an example")}
        </button>
        <button className="btn primary" onClick={() => setTemplate(draft)}>
          {L("保存", "Save")}
        </button>
      </div>
    </>
  );
}

// -------------------------------------------------------------------------
// Sync

function SyncTab() {
  const [settings, setSettings] = useState<SyncSettings | null>(null);
  const status = useStore((s) => s.syncStatus);
  const [choice, setChoice] = useState<"local" | "nextcloud" | "googleDrive" | "oneDrive" | null>(null);
  const [server, setServer] = useState("");
  const [folder, setFolder] = useState("Nisshoku");
  const [busy, setBusy] = useState<string | null>(null);
  const [manual, setManual] = useState(false);
  const [user, setUser] = useState("");
  const [password, setPassword] = useState("");
  const [clientId, setClientId] = useState("");
  const [gClientId, setGClientId] = useState("");
  const [gSecret, setGSecret] = useState("");
  const [gOwn, setGOwn] = useState(false);

  const load = () => api.getSyncSettings().then(setSettings).catch(report);
  useEffect(() => {
    void load();
  }, [status?.state]);

  if (!settings) return null;
  const target = settings.target;
  const s = status ?? settings.status;

  const done = async (message: string) => {
    setBusy(null);
    setChoice(null);
    await load();
    useStore.getState().toast(message);
    void api.syncNow().catch(report);
  };
  const attempt = async (message: string, run: () => Promise<void>, success: string) => {
    try {
      setBusy(message);
      await run();
      await done(success);
    } catch (e) {
      setBusy(null);
      report(e);
    }
  };

  if (target.kind !== "none" && !choice) {
    const desc =
      target.kind === "local"
        ? `${L("ローカルフォルダ", "Local folder")}: ${target.path}`
        : target.kind === "nextcloud"
          ? `Nextcloud: ${target.user} @ ${target.server}（${target.folder}）`
          : target.kind === "oneDrive"
            ? `OneDrive: ${target.folder}`
            : `Google Drive: ${L("マイドライブ", "My Drive")} / ${target.folder}`;
    const r = s.lastReport;
    return (
      <>
        <h3>{L("同期", "Sync")}</h3>
        <div className="status-box">
          <b>{desc}</b>
          <div>{s.state === "syncing" ? L("同期中…", "Syncing…") : s.lastSync ? `${L("最終同期", "Last sync")}: ${relativeTime(s.lastSync)}` : L("まだ同期していません", "Not synced yet")}</div>
          {r && (
            <small>
              {L("送信", "Sent")} {r.created_remote + r.updated_remote + r.deleted_remote}・{L("受信", "Received")} {r.created_local + r.updated_local + r.deleted_local}
              {r.conflicts ? `・${L("競合", "Conflicts")} ${r.conflicts}` : ""}
            </small>
          )}
        </div>
        {s.state === "error" && <div className="status-box error">{s.message}</div>}
        {s.needsPassword && <PasswordPrompt />}
        <div className="row-buttons">
          <button className="btn primary" disabled={s.state === "syncing"} onClick={() => void api.syncNow().catch(report)}>
            <Icon name="sync" />
            {L("今すぐ同期", "Sync now")}
          </button>
          <button
            className="btn"
            onClick={() => {
              if (target.kind === "oneDrive") {
                setClientId(target.clientId);
                setFolder(target.folder);
              }
              if (target.kind === "googleDrive") {
                setGClientId(target.clientId ?? "");
                setGOwn(!!target.clientId);
                setFolder(target.folder);
              }
              setChoice(target.kind);
            }}
          >
            {L("同期先を変更", "Change target")}
          </button>
          <button
            className="btn danger"
            onClick={async () => {
              const ok = await dialogs.confirm(
                L("同期をやめますか？", "Stop syncing?"),
                L("この端末のノートはそのまま残り、同期先のデータも削除されません。", "The notes on this device stay, and nothing is deleted from the sync target."),
                { okLabel: L("同期をやめる", "Stop syncing"), danger: true },
              );
              if (!ok) return;
              await api.disconnectSync().catch(report);
              await load();
            }}
          >
            {L("同期をやめる", "Stop syncing")}
          </button>
        </div>
        <h4>{L("同期のタイミング", "When to sync")}</h4>
        <Field label={L("自動同期の間隔", "Sync every")}>
          <SelectPref
            pref="syncInterval"
            options={[
              [0, L("自動で同期しない", "Never (manual)")],
              [1, L("1分", "1 minute")],
              [5, L("5分", "5 minutes")],
              [15, L("15分", "15 minutes")],
              [30, L("30分", "30 minutes")],
              [60, L("1時間", "1 hour")],
            ]}
          />
        </Field>
        <SwitchPref pref="syncOnStart" label={L("起動したときに同期する", "Sync when the app starts")} />
        <SwitchPref pref="syncAfterEdit" label={L("書くのを止めて約20秒後に同期する", "Sync about 20 seconds after you stop typing")} />
        <EncryptionSection hasPassword={settings.hasMasterPassword} />
        <p className="muted small">
          {L(
            "同期は5分ごとと、入力が止まってから約20秒後に自動で行います。Joplin と同じ形式なので、Joplin アプリと同じ同期先を共有できます。",
            "Syncs every 5 minutes and about 20 seconds after you stop typing. The format is Joplin’s, so Joplin apps can share the same target.",
          )}
        </p>
      </>
    );
  }

  return (
    <>
      <h3>{L("同期先を選択", "Choose where to sync")}</h3>
      <div className="choice-grid">
        <button className={`choice${choice === "nextcloud" ? " active" : ""}`} onClick={() => setChoice("nextcloud")}>
          <b>Nextcloud</b>
          <small>{L("自分や組織のサーバー", "Your own or your organization’s server")}</small>
        </button>
        <button className={`choice${choice === "googleDrive" ? " active" : ""}`} onClick={() => setChoice("googleDrive")}>
          <b>Google Drive</b>
          <small>{settings.gdriveAvailable ? L("Google アカウントのドライブ", "The drive of your Google account") : L("自分で作った OAuth クライアントで接続", "With an OAuth client you create yourself")}</small>
        </button>
        <button className={`choice${choice === "oneDrive" ? " active" : ""}`} onClick={() => setChoice("oneDrive")}>
          <b>OneDrive</b>
          <small>{L("自分で登録したアプリで接続", "With an app you register yourself")}</small>
        </button>
        {!isMobile && (
          <button className={`choice${choice === "local" ? " active" : ""}`} onClick={() => setChoice("local")}>
            <b>{L("ローカルフォルダ", "Local folder")}</b>
            <small>{L("この PC や共有フォルダ", "A folder on this PC or a network share")}</small>
          </button>
        )}
      </div>

      {choice === "nextcloud" && (
        <div className="stack">
          <label>
            {L("サーバーの URL", "Server URL")}
            <input className="input" placeholder="https://cloud.example.com" value={server} onChange={(e) => setServer(e.target.value)} />
          </label>
          <label>
            {L("保存先のフォルダ", "Folder")}
            <input className="input" value={folder} onChange={(e) => setFolder(e.target.value)} />
          </label>
          {manual && (
            <>
              <label>
                {L("ユーザー名", "User name")}
                <input className="input" value={user} onChange={(e) => setUser(e.target.value)} />
              </label>
              <label>
                {L("アプリパスワード", "App password")}
                <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
              </label>
            </>
          )}
          <div className="row-buttons">
            {!manual ? (
              <button
                className="btn primary"
                disabled={!server.trim() || !!busy}
                onClick={() =>
                  attempt(
                    L("ブラウザでログインし、アクセスを許可してください…", "Sign in in the browser and allow access…"),
                    async () => {
                      await api.nextcloudLoginStart(server.trim());
                      await api.nextcloudLoginWait(folder);
                    },
                    L("Nextcloud に接続しました", "Connected to Nextcloud"),
                  )
                }
              >
                {L("ブラウザでログイン", "Sign in with the browser")}
              </button>
            ) : (
              <button
                className="btn primary"
                disabled={!server.trim() || !user || !password || !!busy}
                onClick={() =>
                  attempt(L("接続しています…", "Connecting…"), () => api.nextcloudConnect(server.trim(), user, password, folder), L("Nextcloud に接続しました", "Connected to Nextcloud"))
                }
              >
                {L("接続", "Connect")}
              </button>
            )}
            <button className="btn" onClick={() => setManual(!manual)}>
              {manual ? L("ブラウザでログインする", "Use the browser instead") : L("アプリパスワードで接続", "Use an app password")}
            </button>
          </div>
        </div>
      )}

      {choice === "googleDrive" && (
        <div className="stack">
          <label>
            {L("マイドライブ内のフォルダ名", "Folder in My Drive")}
            <input className="input" value={folder} onChange={(e) => setFolder(e.target.value)} />
          </label>
          <p className="muted">{L("日食が作成したファイルにだけアクセスします。ドライブのほかのファイルは見えません。", "Nisshoku can only see the files it created, nothing else in your drive.")}</p>
          {(gOwn || !settings.gdriveAvailable) && (
            <>
              <ol className="muted small">
                <li>{L("Google Cloud コンソール（console.cloud.google.com）でプロジェクトを作り、Google Drive API を有効にする", "In the Google Cloud console (console.cloud.google.com), create a project and enable the Google Drive API")}</li>
                <li>{L("OAuth 同意画面を設定し、スコープ …/auth/drive.file を追加。テスト中なら自分をテストユーザーに追加", "Set up the OAuth consent screen with the scope …/auth/drive.file; while testing, add yourself as a test user")}</li>
                <li>{L("「認証情報」で OAuth クライアント ID を作成（種類は「デスクトップ アプリ」）", "Under “Credentials”, create an OAuth client ID of type “Desktop app”")}</li>
              </ol>
              <label>
                {L("クライアント ID", "Client ID")}
                <input className="input" placeholder="….apps.googleusercontent.com" value={gClientId} onChange={(e) => setGClientId(e.target.value)} autoComplete="off" spellCheck={false} />
              </label>
              <label>
                {L("クライアント シークレット", "Client secret")}
                <input className="input" type="password" value={gSecret} onChange={(e) => setGSecret(e.target.value)} autoComplete="off" />
              </label>
            </>
          )}
          {settings.gdriveAvailable && (
            <Field label={L("自分の OAuth クライアントを使う", "Use my own OAuth client")}>
              <Switch on={gOwn} onChange={setGOwn} />
            </Field>
          )}
          <div>
            <button
              className="btn primary"
              disabled={!!busy || ((gOwn || !settings.gdriveAvailable) && (!gClientId.trim() || !gSecret.trim()))}
              onClick={() =>
                attempt(
                  L("ブラウザで Google にログインし、アクセスを許可してください…", "Sign in to Google in the browser and allow access…"),
                  () => (gOwn || !settings.gdriveAvailable ? api.gdriveLogin(folder, gClientId.trim(), gSecret.trim()) : api.gdriveLogin(folder)),
                  L("Google Drive に接続しました", "Connected to Google Drive"),
                )
              }
            >
              {L("Google でログイン", "Sign in with Google")}
            </button>
          </div>
        </div>
      )}

      {choice === "oneDrive" && (
        <div className="stack">
          <p className="muted">
            {L(
              "Microsoft Entra 管理センター（entra.microsoft.com）の「アプリの登録」で新しいアプリを登録し、そのクライアント ID を入力してください。",
              "Register a new app under “App registrations” in the Microsoft Entra admin center (entra.microsoft.com) and enter its client ID.",
            )}
          </p>
          <ol className="muted small">
            <li>{L("サポートされているアカウントの種類: 「任意の組織ディレクトリ内のアカウントと個人用 Microsoft アカウント」", "Supported account types: “Accounts in any organizational directory and personal Microsoft accounts”")}</li>
            <li>{L("リダイレクト URI: 「パブリック クライアント/ネイティブ (モバイルとデスクトップ)」で http://localhost", "Redirect URI: “Public client/native (mobile & desktop)” with http://localhost")}</li>
            <li>{L("API のアクセス許可: Microsoft Graph の委任されたアクセス許可 Files.ReadWrite", "API permissions: Microsoft Graph delegated permission Files.ReadWrite")}</li>
          </ol>
          <label>
            {L("アプリケーション (クライアント) ID", "Application (client) ID")}
            <input className="input" placeholder="00000000-0000-0000-0000-000000000000" value={clientId} onChange={(e) => setClientId(e.target.value)} />
          </label>
          <label>
            {L("OneDrive 内のフォルダ", "Folder in OneDrive")}
            <input className="input" value={folder} onChange={(e) => setFolder(e.target.value)} />
          </label>
          <p className="muted small">{L("Joplin の OneDrive 同期先と共有するには、フォルダを Apps/Joplin にします。", "To share with Joplin’s OneDrive target, use the folder Apps/Joplin.")}</p>
          <div>
            <button
              className="btn primary"
              disabled={!clientId.trim() || !!busy}
              onClick={() =>
                attempt(
                  L("ブラウザで Microsoft にログインし、アクセスを許可してください…", "Sign in to Microsoft in the browser and allow access…"),
                  () => api.onedriveLogin(clientId.trim(), folder),
                  L("OneDrive に接続しました", "Connected to OneDrive"),
                )
              }
            >
              {L("Microsoft でログイン", "Sign in with Microsoft")}
            </button>
          </div>
        </div>
      )}

      {choice === "local" && (
        <div className="stack">
          <p className="muted">{L("選んだフォルダにノートを保存します。USB メモリやネットワーク上の共有フォルダも使えます。", "Notes are saved in the folder you choose. USB drives and network shares work too.")}</p>
          <div>
            <button
              className="btn primary"
              onClick={async () => {
                const dir = await openDialog({ directory: true, title: L("同期に使うフォルダ", "Folder to sync with") });
                if (typeof dir !== "string") return;
                await attempt(L("設定しています…", "Setting up…"), () => api.setLocalTarget(dir), L("フォルダとの同期を設定しました", "Syncing with the folder"));
              }}
            >
              {L("フォルダを選択", "Choose a folder")}
            </button>
          </div>
        </div>
      )}

      {busy && <div className="status-box">{busy}</div>}
      {target.kind !== "none" && (
        <button className="btn" style={{ marginTop: 16 }} onClick={() => setChoice(null)}>
          {L("戻る", "Back")}
        </button>
      )}
    </>
  );
}

function PasswordPrompt() {
  const [pw, setPw] = useState("");
  return (
    <div className="status-box">
      <div>{L("同期先は暗号化されています。暗号化のパスワードを入力してください。", "The sync target is encrypted. Enter the encryption password.")}</div>
      <div className="row-buttons">
        <input className="input" type="password" value={pw} onChange={(e) => setPw(e.target.value)} />
        <button className="btn primary" disabled={!pw} onClick={() => void api.setMasterPassword(pw).catch(report)}>
          {L("ロックを解除", "Unlock")}
        </button>
      </div>
    </div>
  );
}

function EncryptionSection({ hasPassword }: { hasPassword: boolean }) {
  return (
    <>
      <h4>{L("エンドツーエンド暗号化", "End-to-end encryption")}</h4>
      <p className="muted">
        {L(
          "同期先に置くデータを、この端末で暗号化してから送ります。パスワードを忘れると、だれにも復元できません。",
          "Data is encrypted on this device before it is sent. If you forget the password, nobody can recover it.",
        )}
        {hasPassword ? L("（この端末にはパスワードが保存されています）", " (The password is stored on this device.)") : ""}
      </p>
      <div className="row-buttons">
        <button
          className="btn"
          onClick={async () => {
            const pw = await dialogs.prompt(L("暗号化のパスワード", "Encryption password"), {
              message: L("ほかの端末でも同じパスワードを入力します。忘れないものにしてください。", "You will enter the same password on your other devices. Choose one you will remember."),
              okLabel: L("暗号化を有効にする", "Turn on encryption"),
            });
            if (!pw) return;
            try {
              await api.setEncryption(true, pw);
              useStore.getState().toast(L("暗号化を有効にしました。次の同期から暗号化して送ります", "Encryption is on. The next sync sends encrypted data"));
              void api.syncNow();
            } catch (e) {
              report(e);
            }
          }}
        >
          <Icon name="lock" />
          {L("暗号化を有効にする", "Turn on encryption")}
        </button>
        <button
          className="btn"
          onClick={async () => {
            if (!(await dialogs.confirm(L("暗号化を無効にしますか？", "Turn off encryption?"), L("すべてのノートを暗号化せずに送り直します。", "All notes will be sent again without encryption.")))) return;
            try {
              await api.setEncryption(false);
              useStore.getState().toast(L("暗号化を無効にしました", "Encryption is off"));
              void api.syncNow();
            } catch (e) {
              report(e);
            }
          }}
        >
          {L("暗号化を無効にする", "Turn off encryption")}
        </button>
      </div>
    </>
  );
}

// -------------------------------------------------------------------------

function StatsTab() {
  const stats = useStore((s) => s.stats);
  useEffect(() => void useStore.getState().loadStats(), []);
  if (!stats) return null;
  const max = Math.max(1, ...stats.days.map(([, v]) => v));
  const month = stats.days.reduce((n, [, v]) => n + v, 0);
  const unit = L("字", " chars");
  return (
    <>
      <h3>{L("統計", "Statistics")}</h3>
      <div className="choice-grid">
        {[
          [L("今日", "Today"), `${formatNumber(stats.today)}${unit}`],
          [L("この30日", "Last 30 days"), `${formatNumber(month)}${unit}`],
          [L("連続執筆", "Streak"), L(`${stats.streak}日`, stats.streak === 1 ? "1 day" : `${stats.streak} days`)],
          [L("ノート", "Notes"), formatNumber(stats.totalNotes)],
          [L("総文字数", "Total"), `${formatNumber(stats.totalChars)}${unit}`],
          [L("原稿用紙換算", "Manuscript pages"), L(`約${formatNumber(Math.ceil(stats.totalChars / 400))}枚`, `~${formatNumber(Math.ceil(stats.totalChars / 400))}`)],
        ].map(([k, v]) => (
          <div key={k} className="choice static">
            <small>{k}</small>
            <b className="big">{v}</b>
          </div>
        ))}
      </div>
      <YearCalendar history={stats.history} goal={useStore.getState().prefs.dailyGoal} />
      <h4>{L("この30日間", "The last 30 days")}</h4>
      <div className="bar-chart">
        {stats.days.map(([d, v]) => (
          <div key={d} title={`${d}: ${formatNumber(v)}`} className={v ? "" : "zero"} style={{ height: `${Math.max(2, (v / max) * 100)}%` }} />
        ))}
      </div>
      <div className="bar-axis">
        <span>{stats.days[0]?.[0]}</span>
        <span>{L("今日", "Today")}</span>
      </div>
    </>
  );
}

function ListTab() {
  return (
    <>
      <h3>{L("ノート一覧", "Note list")}</h3>
      <Field label={L("並べ順", "Sort by")}>
        <SelectPref
          pref="sortNotes"
          options={[
            ["updated", L("更新日順", "Last edited")],
            ["created", L("作成日順", "Date created")],
            ["title", L("タイトル順", "Title")],
          ]}
          after={() => void useStore.getState().loadNotes()}
        />
      </Field>
      <SwitchPref pref="showExcerpt" label={L("本文の冒頭を表示する", "Show the start of the text")} />
      <SwitchPref pref="compactList" label={L("コンパクト表示", "Compact list")} hint={L("1件を短く表示して、多くのノートを一度に見られるようにします。", "Shorter rows, so that more notes fit.")} />
    </>
  );
}

function DataTab() {
  return (
    <>
      <h3>{L("データ", "Data")}</h3>
      {!isMobile && (
        <Field
          label={L("すべてのノートを書き出す", "Export all notes")}
          hint={L("ノートブックごとのフォルダに、Markdown ファイルとして保存します。", "Saves Markdown files in a folder per notebook.")}
        >
          <button
            className="btn"
            onClick={async () => {
              const dir = await openDialog({ directory: true, title: L("書き出し先のフォルダ", "Export to folder") });
              if (typeof dir !== "string") return;
              const n = await api.exportAll(dir).catch(report);
              if (n !== undefined) useStore.getState().toast(L(`${n}件のノートを書き出しました`, n === 1 ? "Exported 1 note" : `Exported ${n} notes`));
            }}
          >
            <Icon name="download" />
            {L("フォルダを選んで書き出す", "Choose a folder")}
          </button>
        </Field>
      )}
      <Field label={L("ゴミ箱を自動で空にする", "Empty the trash automatically")} hint={L("ゴミ箱に入れてから指定の日数が過ぎたノートを、起動時に完全に削除します。", "Notes in the trash longer than this are deleted at start.")}>
        <SelectPref
          pref="trashRetentionDays"
          options={[
            [0, L("しない", "Never")],
            [7, L("7日後", "After 7 days")],
            [30, L("30日後", "After 30 days")],
            [90, L("90日後", "After 90 days")],
          ]}
        />
      </Field>
    </>
  );
}

/** A year of writing, a square a day (darker = more), with the days the goal was met marked. */
function YearCalendar({ history, goal }: { history: [string, number][]; goal: number }) {
  const byDay = new Map(history);
  const today = new Date();
  const start = new Date(today);
  start.setDate(today.getDate() - 364 - today.getDay());
  const key = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const values = history.map(([, v]) => v).filter((v) => v > 0).sort((a, b) => a - b);
  const level = (v: number) => {
    if (v <= 0) return 0;
    const q = (p: number) => values[Math.floor((values.length - 1) * p)] ?? 0;
    return v >= q(0.85) ? 4 : v >= q(0.6) ? 3 : v >= q(0.3) ? 2 : 1;
  };
  const weeks: { date: string; value: number; future: boolean }[][] = [];
  let met = 0;
  let total = 0;
  for (let w = 0; w < 53; w++) {
    const week = [];
    for (let d = 0; d < 7; d++) {
      const day = new Date(start);
      day.setDate(start.getDate() + w * 7 + d);
      const k = key(day);
      const value = byDay.get(k) ?? 0;
      const future = day > today;
      if (!future) {
        total += value;
        if (goal > 0 && value >= goal) met++;
      }
      week.push({ date: k, value, future });
    }
    weeks.push(week);
  }
  return (
    <>
      <h4>
        {L("この1年の執筆", "The last year of writing")}
        <span className="muted" style={{ fontWeight: 400, marginLeft: 10 }}>
          {formatNumber(total)}
          {L("字", " chars")}
          {goal > 0 ? `・${L("目標達成", "goal met")} ${met}${L("日", " days")}` : ""}
        </span>
      </h4>
      <div className="year-calendar">
        {weeks.map((week, i) => (
          <div key={i} className="year-week">
            {week.map((d) => (
              <span
                key={d.date}
                className={`year-day l${d.future ? "x" : level(d.value)}${goal > 0 && d.value >= goal ? " goal" : ""}`}
                title={`${d.date}: ${formatNumber(d.value)}`}
              />
            ))}
          </div>
        ))}
      </div>
      <div className="year-legend">
        {L("少", "Less")}
        {[0, 1, 2, 3, 4].map((l) => (
          <span key={l} className={`year-day l${l}`} />
        ))}
        {L("多", "More")}
        {goal > 0 && (
          <>
            <span className="year-day l3 goal" style={{ marginLeft: 12 }} />
            {L("目標達成", "Goal met")}
          </>
        )}
      </div>
    </>
  );
}

function KeysTab() {
  const keys: [string, string, string][] = [
    ["Ctrl N", "新しいノート", "New note"],
    ["Ctrl D", "今日の日記", "Today’s diary entry"],
    ["Ctrl K", "検索・コマンド", "Search & commands"],
    ["Ctrl Shift F", "全文検索", "Search all notes"],
    ["Ctrl F / Ctrl H", "ノート内の検索／置換", "Find / replace in note"],
    ["Ctrl Alt M", "選択範囲にコメント", "Comment on selection"],
    ["Ctrl S", "すぐに保存", "Save now"],
    ["Ctrl G", "行（段落）へ移動", "Go to line (paragraph)"],
    ["Alt ↑ / ↓", "段落（行）を上下に移動", "Move paragraph (line) up / down"],
    ["Alt ← / →", "前のノートに戻る／次へ進む", "Previous / next note"],
    ["F11", "集中モード", "Focus mode"],
    ["Ctrl \\", "サイドバーの表示切り替え", "Toggle sidebar"],
    ["Ctrl Shift \\", "ノート一覧の表示切り替え", "Toggle note list"],
    ["Ctrl ,", "設定", "Settings"],
    ["/", "ブロックを挿入（行頭で）", "Insert a block (at line start)"],
    ["Ctrl B / I / U", "太字／斜体／下線", "Bold / italic / underline"],
    ["Ctrl Shift 7 / 8 / 9", "番号付き／箇条書き／チェックリスト", "Numbered / bulleted / checklist"],
    ["Ctrl Alt 1〜3", "見出し 1〜3", "Headings 1–3"],
  ];
  return (
    <>
      <h3>{L("ショートカット", "Shortcuts")}</h3>
      {keys.map(([k, ja, en]) => (
        <Field key={k} label={L(ja, en)}>
          <span className="kbd big">{k}</span>
        </Field>
      ))}
    </>
  );
}

const KIND_LABEL: Record<InstallKind, [string, string]> = {
  nsis: ["Windows インストーラー", "Windows installer"],
  msi: ["Windows インストーラー (MSI)", "Windows installer (MSI)"],
  appimage: ["Linux AppImage", "Linux AppImage"],
  deb: ["Linux .deb パッケージ", "Linux .deb package"],
  rpm: ["Linux .rpm パッケージ", "Linux .rpm package"],
  android: ["Android", "Android"],
  dev: ["開発版", "Development build"],
};

/** Release notes (Markdown): headings, list items and paragraphs. */
function ReleaseNotes({ text }: { text: string }) {
  const blocks: React.ReactNode[] = [];
  let items: string[] = [];
  const endList = () => {
    if (items.length) blocks.push(<ul key={blocks.length}>{items.map((i, n) => <li key={n}>{i}</li>)}</ul>);
    items = [];
  };
  const clean = (l: string) => l.replace(/\*\*(.+?)\*\*/g, "$1").replace(/`([^`]+)`/g, "$1").replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");
  for (const raw of text.replace(/\r/g, "").split("\n")) {
    const line = raw.trim();
    if (/^[-*] /.test(line)) items.push(clean(line.slice(2)));
    else {
      endList();
      if (/^#{1,6} /.test(line)) blocks.push(<h5 key={blocks.length}>{clean(line.replace(/^#+ /, ""))}</h5>);
      else if (line) blocks.push(<p key={blocks.length}>{clean(line)}</p>);
    }
  }
  endList();
  return <div className="release-notes">{blocks}</div>;
}

function formatBytes(n: number) {
  return n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.round(n / 1024)} KB`;
}

function AboutTab() {
  const u = useUpdate();
  useEffect(() => {
    if (useUpdate.getState().status === "idle") void checkForUpdate(true);
    else if (!useUpdate.getState().current) void appVersion().then((current) => useUpdate.setState({ current }));
    if (!useUpdate.getState().kind) void api.installKind().then((kind) => useUpdate.setState({ kind })).catch(() => {});
  }, []);
  const busy = u.status === "checking" || u.status === "downloading" || u.status === "installing";
  const kind = u.kind ?? "dev";
  const status = (() => {
    switch (u.status) {
      case "checking":
        return L("新しい版を確かめています…", "Checking for a new version…");
      case "none":
        return L("最新の版です", "You have the latest version");
      case "available":
        return L(`新しい版 ${u.version} があります`, `Version ${u.version} is available`);
      case "downloading":
        return u.total
          ? L(`ダウンロード中… ${formatBytes(u.downloaded)} / ${formatBytes(u.total)}`, `Downloading… ${formatBytes(u.downloaded)} of ${formatBytes(u.total)}`)
          : L(`ダウンロード中… ${formatBytes(u.downloaded)}`, `Downloading… ${formatBytes(u.downloaded)}`);
      case "installing":
        return L("インストールしています。まもなく再起動します…", "Installing. The app will restart shortly…");
      case "error":
        return L("新しい版を確かめられませんでした", "Could not check for updates");
      default:
        return "";
    }
  })();
  const installHint =
    kind === "android"
      ? L("ダウンロードした APK を開くと上書きでインストールされます（データはそのまま残ります）。", "Open the downloaded APK to install it over this one (your notes stay).")
      : kind === "deb" || kind === "rpm"
        ? L("インストールの途中で管理者のパスワードを求められます。", "You will be asked for an administrator password while it installs.")
        : kind === "dev"
          ? L("開発版のため、ここからはインストールできません。リリースのページから入手してください。", "This is a development build: get new versions from the releases page.")
          : L("ノートは保存してから更新し、自動で再起動します。", "Your notes are saved first; the app restarts by itself.");
  return (
    <>
      <h3>{L("アップデート", "Updates")}</h3>
      <div className="about-head">
        <img src={appIcon} alt="" width={56} height={56} />
        <div>
          <b>{L("日食", "Nisshoku")}</b>
          <small>
            {L("バージョン", "Version")} {u.current || "…"} · {L(...KIND_LABEL[kind])}
          </small>
        </div>
      </div>
      <div className={`update-card ${u.status}`}>
        <div className="update-status">
          <span>{status || L("まだ確かめていません", "Not checked yet")}</span>
          {u.status === "error" && u.error && <small>{u.error}</small>}
          {u.checkedAt && u.status !== "checking" && (
            <small>
              {L("最終確認", "Last checked")}: {new Date(u.checkedAt).toLocaleString(lang() === "en" ? "en" : "ja")}
            </small>
          )}
        </div>
        {(u.status === "downloading" || u.status === "installing") && (
          <div className="progress">
            <div style={{ width: u.total ? `${Math.min(100, (u.downloaded / u.total) * 100)}%` : u.status === "installing" ? "100%" : "30%" }} />
          </div>
        )}
        {u.status === "available" && (
          <>
            {u.notes.trim() && <ReleaseNotes text={u.notes} />}
            <small className="hint">{installHint}</small>
          </>
        )}
        <div className="update-actions">
          {u.status === "available" && kind !== "dev" && (
            <button className="btn primary" onClick={() => void installUpdate()}>
              {kind === "android" ? L("APK をダウンロード", "Download the APK") : L("今すぐ更新して再起動", "Update and restart")}
            </button>
          )}
          <button className="btn" disabled={busy} onClick={() => void checkForUpdate()}>
            {L("新しい版を確かめる", "Check for updates")}
          </button>
          <button className="btn" onClick={() => void openReleasePage()}>
            {u.status === "available" ? L("リリースノート", "Release notes") : L("すべての版", "All versions")}
          </button>
        </div>
      </div>
      <SwitchPref
        pref="autoUpdateCheck"
        label={L("新しい版を自動で確かめる", "Check for updates automatically")}
        hint={L("起動時と数時間ごとに確かめ、見つかったらお知らせします。勝手にはインストールしません。", "At start and every few hours; you are told when one is found. Nothing is installed without asking.")}
      />
    </>
  );
}

export function SettingsDialog() {
  const tab = useStore((s) => s.settingsTab);
  useStore((s) => s.prefs.language);
  const dir = useSlideDir(TABS.findIndex(([id]) => id === tab));
  if (!tab) return null;
  const close = () => useStore.getState().set({ settingsTab: null });
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && close()} onKeyDown={(e) => e.key === "Escape" && close()}>
      <div className="dialog large">
        <div className="settings">
          <div className="settings-nav">
            {TABS.filter(([id]) => !(isMobile && id === "keys")).map(([id, icon, [ja, en]]) => (
              <button key={id} className={`nav-item${tab === id ? " active" : ""}`} onClick={() => useStore.getState().set({ settingsTab: id })}>
                <Icon name={icon} />
                <span className="label">{L(ja, en)}</span>
              </button>
            ))}
          </div>
          <div className="settings-main tab-body" key={tab} data-dir={dir}>
            {tab === "general" && <GeneralTab />}
            {(tab === "editor" || tab === "appearance") && <EditorTab />}
            {tab === "writing" && <WritingTab />}
            {tab === "diary" && <DiaryTab />}
            {tab === "sync" && <SyncTab />}
            {tab === "stats" && <StatsTab />}
            {tab === "keys" && <KeysTab />}
            {tab === "list" && <ListTab />}
            {tab === "data" && <DataTab />}
            {tab === "about" && <AboutTab />}
          </div>
        </div>
        <div className="dialog-foot bordered">
          <button className="btn" onClick={close}>
            {L("閉じる", "Close")}
          </button>
        </div>
      </div>
    </div>
  );
}
