# Nisshoku

Tauri 製の執筆アプリ。同期エンジンは Joplin の同期形式（sync target version 3）と互換の Rust 実装。

## 方針

- **Joplin 互換**: 同期先のフォルダ構成・アイテムの直列化形式・ロックの仕組みは Joplin と同じ。
  同じ同期先を本家 Joplin と共有できる。Nextcloud 上で、Joplin 3.7（CLI・デスクトップ）と
  Joplin 2.14 との相互同期を確認済み（暗号化・ノート履歴を含む）。
- **独自実装**: Joplin のクライアントは AGPL-3.0。コードは移植せず、公開されている
  同期仕様・データ形式をもとに書き起こしている。

## 構成

| crate | 役割 |
|---|---|
| `crates/nisshoku-core` | データモデル（ノート・ノートブック・添付・タグ・リビジョン）、Joplin 互換の直列化、SQLite、ノート履歴 |
| `crates/nisshoku-sync` | ストレージ抽象 `RemoteStorage`、同期先ドライバ、ロック、`info.json`、エンドツーエンド暗号化、同期本体 |
| `apps/desktop` | アプリ「日食」（Tauri 2 + React + TipTap）。Windows・Linux・Android |

同期先ドライバ:

- **ローカルフォルダ** (`FileSystemStorage`) — 条件付き書き込みはロックファイルで排他（同じ PC・共有フォルダ上の同時同期でも安全）
- **Nextcloud** (`NextcloudStorage`) — WebDAV、Login Flow v2（ブラウザでログインしてアプリパスワードを取得）、
  ユーザー ID の自動解決、条件付き書き込み、大きな添付のチャンクアップロード、一時的エラーの再試行
- **OneDrive** (`OneDriveStorage`) — Microsoft Graph API。パス指定、`eTag` による条件付き書き込み、
  アップロードセッション（大きな添付）、スロットリング（`429`/`Retry-After`）の再試行。
  フォルダを `Apps/Joplin`（Joplin の OneDrive 同期先が使うアプリフォルダ）にすれば Joplin と共有できる
- **Google Drive** (`GoogleDriveStorage`) — Drive API v3、権限は `drive.file`（このアプリが作ったファイルだけ見える）。
  Drive は同名ファイルを許すため、同名があるときは最も古いものを正とし、削除時はすべて消す。
  ファイル ID を事前に確保して作成するので、応答が失われて再送しても重複しない。大きな添付は再開可能アップロード。
  Joplin には Google Drive 同期先が無いため、Joplin との共有はできない（形式は同じ）

OneDrive と Google Drive のログインは OAuth 2.0（ブラウザでログインし、ループバックアドレスで認可コードを
受け取る。PKCE 付き）。アクセストークンは期限前・拒否時に自動更新し、新しいリフレッシュトークンを通知する。

## デスクトップアプリ

```sh
cd apps/desktop
pnpm install
pnpm tauri dev        # 開発用に起動
pnpm build:app        # インストーラーを作る（更新用の署名付き。下記）
pnpm dev              # 画面だけをブラウザで確認（データは模擬）
```

`pnpm build:app` は `tauri build` を、`~/.tauri/nisshoku-updater.key`（とパスワードの `.password`）で
自動アップデート用に署名して実行する。鍵が無ければ署名なしで作る。

Linux（Ubuntu 22.04 以降・Debian 12 以降など）でのビルドには
`libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf` が要る。AppImage を作るには `file` コマンドも要る。
Linux のアプリ一覧での名前は、日本語環境なら「日食」（`installer/nisshoku.desktop`、`tauri.linux.conf.json`）。パスワードやトークンは
Secret Service（GNOME キーリング・KWallet）に保存し、それが無い環境ではプロファイルの `secrets.json`（本人だけが読める）に置く。

環境変数 `NISSHOKU_PROFILE` にフォルダを指定すると、ノートをそこに置く（試すとき・USB メモリで持ち歩くとき。既定はアプリのデータフォルダ）。

Android（Android Studio の SDK・NDK と `rustup target add x86_64-linux-android aarch64-linux-android` が要る）:

```sh
pnpm tauri android build --debug --target x86_64 --apk   # エミュレータ用
pnpm tauri android build --target aarch64 --apk          # 実機用（署名付き）
```

**まとめて一箇所に**: `pnpm collect`（`node scripts/collect-builds.mjs`）が、下の各出力先（Linux は WSL 内の
`~/nisshoku-build`）から今の版のインストーラーを `builds/<版>/` に集める（`Nisshoku_<版>_windows_x64-setup.exe`・`_windows_x64.msi`・
`_android.apk`・`_linux_amd64.AppImage`・`_linux_amd64.deb`・`_linux_x86_64.rpm`）。`pnpm build:app` は終わると自動でこれを行う。

インストーラーの出力先（ビルドが置く場所）:

- Windows: `target/release/bundle/nsis/Nisshoku_<版>_x64-setup.exe`（管理者権限なしで `%LOCALAPPDATA%\Nisshoku` に入る。日本語・英語、WebView2 が無ければ一緒に入れる）と `target/release/bundle/msi/Nisshoku_<版>_x64_ja-JP.msi`・`_en-US.msi`。インストーラーの画像とフックは `src-tauri/installer/`
- 名前: フォルダ・パッケージ・実行ファイルなどシステム側の名前は英字の `Nisshoku`（製品名）。人が見る名前は言語に合わせる:
  ウィンドウのタイトルはアプリの表示言語で「日食」／「Nisshoku」、Windows のスタートメニューと「アプリ」一覧は日本語の Windows なら「日食」
  （`installer/hooks.nsh`）、Linux のアプリ一覧は `Name[ja]=日食`、Android のアプリ名は端末（またはアプリごと）の言語で
- 0.1.0 は「日食」というフォルダ（`%LOCALAPPDATA%\日食`）に入っていた。新しいインストーラーはその古い版を（ノートと設定は残して）取り除き、ショートカットを張り直す
- Linux: `target/release/bundle/appimage/Nisshoku_<版>_amd64.AppImage`、`deb/Nisshoku_<版>_amd64.deb`、`rpm/Nisshoku-<版>-1.x86_64.rpm`
- Android: `apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`。スマホに送って開き、「提供元不明のアプリ」を許可して入れる

リリース版の署名鍵は `%USERPROFILE%\.android-keys\nisshoku-release.jks`（リポジトリの外）。パスワードは `gen/android/keystore.properties`（git 対象外）。**鍵をなくすと、入れたアプリを上書き更新できなくなる**ので、両方をバックアップしておくこと。デバッグ版とリリース版は鍵が違うので、同じ端末で切り替えるにはアンインストールが要る。

## リリースと自動アップデート

GitHub（`nuyu3n/nisshoku`）の Releases で配る。

1. `pnpm version:set 0.2.0`（`tauri.conf.json`・`package.json`・Cargo の版をそろえる）→ `cargo check` → コミット
2. `git tag v0.2.0 && git push origin v0.2.0`
3. `.github/workflows/release.yml` が Windows（NSIS・MSI）・Linux（AppImage・.deb・.rpm）・Android（APK）を作って
   下書きのリリースに載せ、全部そろったら公開する。`-beta.1` のような版はプレリリースになる

アプリは起動時と数時間ごとに新しい版を確かめ（設定 → アップデート で切り替え・手動確認）、見つかると知らせる。
インストールは確認してから:

- Windows・Linux: 更新プラグインがリリースの `latest.json` を読み、署名を検証してダウンロード・インストールし、再起動する。
  入れ方（NSIS／MSI、AppImage／.deb／.rpm）に合わせた形式を使う。.deb・.rpm は管理者のパスワードを求められる
- Android: GitHub API でいちばん新しいリリースを調べ、APK のダウンロードをブラウザで開く（上書きでインストール）

リポジトリに登録する Secrets（Settings → Secrets and variables → Actions）:

| 名前 | 中身 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `~/.tauri/nisshoku-updater.key` の中身 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | `~/.tauri/nisshoku-updater.password` の中身 |
| `ANDROID_KEYSTORE_BASE64` | 署名鍵 `nisshoku-release.jks` を base64 にしたもの |
| `ANDROID_KEYSTORE_PASSWORD`・`ANDROID_KEY_ALIAS`・`ANDROID_KEY_PASSWORD` | `keystore.properties` の値 |
| `NISSHOKU_GDRIVE_CLIENT_ID`・`NISSHOKU_GDRIVE_CLIENT_SECRET` | Google Drive の OAuth クライアント（任意） |

**更新用の署名鍵（`~/.tauri/`）をなくすと、配ったアプリに新しい版を届けられなくなる**。Android の鍵と一緒にバックアップしておくこと。
公開鍵は `tauri.conf.json` の `plugins.updater.pubkey`。

`.github/workflows/ci.yml` は push・プルリクエストごとに Rust のテスト（Ubuntu・Windows）と型チェックを行う。

## Android 版

Android 版の違い:

- 狭い画面ではナビ → 一覧 → エディタの 1 画面ずつ。戻るボタン／ジェスチャーで開いているメニュー・ダイアログ・パネルを閉じ、
  画面を 1 つ戻る（ナビ画面ではアプリを閉じる）
- メニューは画面下からのシート。書いている間はキーボードの上に書式バー（元に戻す・見出し・太字・リスト・チェックリスト・引用・ブロック挿入・キーボードを閉じる）
- ステータスバーとナビゲーションバーのアイコンの色はアプリのテーマ（ライト／ダーク）に合わせる。起動画面・文字選択のハンドルなども
  アプリの色（`res/values*/themes.xml`）
- ウィジェットはアプリのテーマ（ライト／ダーク／端末に合わせる）とアクセントカラーで描く
- 言語: Android 13 以降はアプリごとの言語設定（`res/xml/locales_config.xml`）とアプリの表示言語が連動し、
  アプリ名・ショートカット・ウィジェットもその言語になる
- 画面はシステムバーの下まで描き、バーとキーボードの大きさは `MainActivity.kt` がページに渡す（`--inset-*`）。
  キーボードが出るとその分だけ画面を縮める
- パスワードとトークンはアプリ専用領域の `no_backup/secrets.json` に保存（他のアプリから読めず、クラウドバックアップにも含まれない）
- HTTPS の証明書は Android の信頼ストアで検証（`rustls-platform-verifier`。ユーザーが追加した CA も使える）
- ホーム画面のウィジェット: タスク（タップで完了、＋で追加）、今日の執筆（文字数・目標・連続日数）、日記（今日の日記・新しいノート）。
  アプリが `widgets.json` に書いた内容を表示し、ウィジェットでの完了は `widget-actions/` に置いて次にアプリを開いたとき（開いていればすぐ）反映する
- アイコンは月食（地球の影で赤銅色になった月と、光の残る縁）。アダプティブアイコン（`res/mipmap-anydpi-v26`、Android 13 のテーマアイコン用モノクロ付き）。
  図案は `src-tauri/icons/source.svg`。`pnpm tauri icon` は `gen/android` のアイコンも上書きするので、デスクトップ用だけを作るときは `-o` で別フォルダに出してコピーする
- ファイルの書き出し・画像の挿入は Android のファイル選択（`content://`）に対応。ローカルフォルダ同期とフォルダへの一括書き出しは無い
- アイコンの長押しメニュー（ショートカット）: 今日の日記・新しいノート・新しいタスク（`res/xml/shortcuts.xml`）
- 他のアプリの「共有」で、テキストや URL を新しいノートにする。テキストを選んだときのメニューの「日食にメモ」でも同じ（`ACTION_PROCESS_TEXT`）
- クイック設定タイル「今日の日記」（`DiaryTileService.kt`。Android 14 以降の `PendingIntent` 版の起動に対応、ロック中は解除を求める）
- ネイティブライブラリは 16 KB ページサイズに対応（NDK r28 の既定。Android 15 以降の端末と Google Play の要件）
- Google Drive・OneDrive のブラウザログインは、ブラウザから戻った時点でページに「日食に戻る」ボタンを出し、
  トークンの取得はアプリが前面に戻って通信できるようになるまで待つ（バックグラウンドのアプリは通信を止められることがあるため）

Google Drive の OAuth クライアント（「デスクトップアプリ」型）は、ビルド時に環境変数
`NISSHOKU_GDRIVE_CLIENT_ID` と `NISSHOKU_GDRIVE_CLIENT_SECRET` で渡すとアプリに組み込まれる。
組み込まれていないビルドでも、設定画面で自分のクライアント ID とシークレットを入力して使える
（組み込み済みのビルドでも「自分の OAuth クライアントを使う」で切り替え可。シークレットは資格情報ストアに保存）。OneDrive は利用者が自分のアプリ登録
（Microsoft Entra ID、「任意の組織ディレクトリ内のアカウントと個人用 Microsoft アカウント」、
パブリッククライアントのリダイレクト URI `http://localhost`、委任されたアクセス許可 `Files.ReadWrite`）の
クライアント ID を設定画面で入力する方式（BYOK）で、ビルド時の設定は要らない。パスワードやトークンは
OS の資格情報ストア（Windows 資格情報マネージャーなど）に保存する。

主な機能:

- **画面**: ノート一覧とエディタの 2 列。サイドバー（検索と、ノートブック／日記／コメントを切り替えるタブ、今日の文字数、同期状態）は
  ボタン（Ctrl+\）で引き出す。一覧は幅を変えられ、畳める。タイトルバーはアプリが描く（上端をドラッグして移動）。表示言語は日本語と英語
- **エディタ**: Notion 風のリッチエディタ（保存は Markdown で Joplin と互換）。「/」でブロック挿入、`# `・`- `・`> ` などの記法、
  表・チェックリスト・マーカー。リッチ表示で崩れる書式のノートは自動で Markdown のまま開く
- **執筆**: 文字数（選択範囲も）、原稿用紙換算（禁則のぶら下げ込み）、読了時間、ノートごとの目標文字数、1日の目標と今日の文字数、
  30日間のグラフと連続執筆日数、執筆タイマー、集中モード（他の段落を薄く）、タイプライタースクロール、段落の字下げ表示、
  「...」→「…」「--」→「――」の自動変換、ルビ記法 `|漢字《かんじ》` の強調表示、縦書き原稿用紙プレビュー（ルビ付き）、
  アウトライン（章ごとの文字数）、ノート内の検索・置換、全文検索、コマンドパレット（Ctrl+K）、
  行番号（リッチ表示では段落ごと、Markdown 表示と日記では折り返しに合わせた行ごと。今いる行を強調）と番号への移動（Ctrl+G）、
  括弧の自動補完（「」『』（）など。日本語入力で確定した文字にも効く。選択範囲を囲む、閉じ括弧の読み飛ばし、空の括弧は Backspace でまとめて消える）、
  段落・箇条書きの項目・行の入れ替え（Alt+↑/↓）、開いたノートの履歴をたどる「戻る／進む」（Alt+←/→、マウスの戻る・進むボタン）
- **作品**: ノートブックを「作品」にすると、一覧が 原稿・人物・プロット・資料 のタブになる（右クリック → 作品にする）
  - 作品の情報: ログライン、あらすじ、ジャンル、目標の文字数、締め切り（残り日数と 1 日に必要な文字数を表示）
  - 原稿: 章（作品とその下のノートブック＝部 のノート）を順番に並べ、ドラッグで並べ替え。章ごとに状態（構想・執筆中・推敲中・完成）・あらすじ・視点人物。
    全体の文字数と進み具合、状態の内訳。原稿をまとめて書き出し（テキスト・Markdown・ルビ付き HTML）
  - 登場人物: 読み・役割・年齢・外見・性格・経歴・目的・別名・色・人間関係。本文中の名前と別名に人物の色で下線を引き（Ctrl+クリックで人物を開く）、
    どの章に何回出てくるかを数える。章の上には、その章に出てくる人物を並べる
  - プロット: 起承転結・三幕構成・英雄の旅・自由 から始めるカードのボード。列の追加・名前の変更・並べ替え、カードのドラッグ、章との結びつけ、書き終えた印
  - 資料: 世界観・年表・取材メモなど
  - すべてノートとノートブックの `user_data` に保存するので、Joplin と同期しても失われない（Joplin ではふつうのノートとして見える。プロットのノートは本文に読める形で書き出す）
- **履歴**: 自動で残る版の一覧、差分表示、復元（復元前の文章も履歴に残る）、「今の版を保存」
- **日記**: 今日の日記（Ctrl+D）、月カレンダー（書いた日と分量）、日付をタップして好きな日を開く、連続記録、テンプレート。日記帳は印の付いたふつうのノートブック
  （既定の名前は「Diary」、タイトルは「2026-11-04」形式。どちらも設定で変更可）。タイトルの日付からも日を読み取るので、Joplin で書いた日記もカレンダーに出る
- **コメント**: 文章を選んでコメントを付けられる（推敲・校正用）。本文には手を加えず、ノートのメタデータ（`user_data`）に保存して同期する
- **タスク**: ToDo の専用画面。その場で追加、期限（日付だけ／時刻つき。Joplin の `todo_due` と互換）、期限切れ・今日・これから・期限なし・完了に分けて表示、
  タップで完了（元に戻せる）。サイドバーに未完了の件数
- **整理**: ノートブックの階層、ドラッグでノートを移動、ゴミ箱（Joplin のゴミ箱と互換）、競合コピーの一覧
- **書き出し**: Markdown・テキスト・HTML（ルビ付き）、プレーンテキストのコピー
- **同期**: Nextcloud（ブラウザでログイン）・OneDrive（自分のクライアント ID でログイン）・Google Drive・ローカルフォルダ、5分ごとと編集が止まって20秒後に自動同期、暗号化

## 同期の仕組み

1 回の同期は 4 段階。どの段階で中断しても、次回の同期で正しく再開できる。

1. **アップロード**: 前回の同期以降にローカルで変わったものを送る
2. **リモート削除**: ローカルで削除したものを同期先から消す
3. **差分取り込み**: 同期先の変更をローカルに反映する
4. **添付データの整合**: 足りない添付データをダウンロード・再アップロードする

### 衝突時のルール

| 状況 | 結果 |
|---|---|
| 両方で同じノートを編集 | 同期先の版を採用し、ローカル版は**競合コピー**として残す（同期はされない） |
| 両方で同じ内容に編集 | 競合扱いしない |
| 相手が削除したノートを自分が編集していた | 自分の版を競合コピーとして残す |
| 自分が削除したノートを相手が編集していた | **編集が勝つ**（ノートは削除されずに戻る） |
| 削除されたノートブックに相手がノートを追加していた | ノートブックを残す（ノートが行き場を失わない） |
| 添付データを両方で変更 | 同期先の版を採用し、ローカル版は別の添付として残す |

### エンドツーエンド暗号化

Joplin と同じ方式（マスターキーは AES-256-GCM／PBKDF2-SHA512、本文・添付も AES-256-GCM）。
読み書きともに Joplin 3.x と相互運用でき、Joplin 2.x 以前の旧方式（SJCL / AES-CCM）で暗号化された
同期先も読める（書き込みは新方式）。暗号化のオン・オフ、マスターパスワードの変更に対応。
同期先に残るのは ID・親 ID・更新時刻・削除時刻だけで、題名・本文・添付データは暗号化される。

### ノート履歴（リビジョン）

Joplin と同じ形式のリビジョンを作成・同期・復元できる（Joplin デスクトップで作られた履歴も読める）。

- 前回のリビジョンから 10 分以上たって変更されたノートは、現在の版を記録（`collect_revisions`）
- 7 日以上リビジョンが無いノートを編集すると、編集前の版を記録（`save_note`）
- 削除したノートも履歴から復元できる。90 日を過ぎたリビジョンは、残る履歴が壊れないよう整理して削除

### 同時同期

Nextcloud の条件付き書き込み（`If-Match`）は原子的ではない（同じ版を前提にした書き込みが 2 つとも
通ることがあると実測で確認）。Google Drive には条件付き書き込み自体が無い。そのため日食の端末どうしは、
同期先の `.nisshoku/sync/` ディレクトリを排他ロックにして 1 台ずつ同期する。作成はローカルフォルダ・
Nextcloud・OneDrive では原子的。Google Drive では同名フォルダを作れてしまうため、各端末が自分のフォルダを
作って少し待ち、最も古いものを作った端末だけが勝つ「選挙」で決める。持ち主が更新しなくなったロックは一定時間で
放棄されたものとみなす（時刻の比較は待つ側の時計だけで行うので、端末間の時計のずれに影響されない）。
Joplin の同期ロックの仕組みはそのまま守るので、Joplin と同時に使える。

### Joplin 形式への追加

- 添付のメタデータに `nisshoku_blob_sha256`（データのハッシュ）を書く。ダウンロードしたデータを検証し、
  同時更新で壊れたデータを検出・修復するため。Joplin は未知のプロパティを無視するので互換性は保たれる。
- Nextcloud へのアップロードでは更新時刻（`X-OC-MTime`）を明示する。Nextcloud の ETag は秒単位の
  更新時刻から作られるため、1 秒以内の 2 回の書き込みを他の端末が見分けられなくなるのを防ぐ。

## テスト

```sh
cargo test
```

- `tests/sync.rs` — 同期シナリオ 27 種を、ローカルフォルダ／Nextcloud／OneDrive／Google Drive（それぞれ模擬と実物）で実行
- `tests/e2ee.rs` — 暗号化シナリオ 6 種（実 Joplin 3.7・2.14 が暗号化した同期先の読み取りを含む）
- `crates/nisshoku-core/tests/joplin_revisions.rs` — Joplin デスクトップが作った実際のリビジョンとの互換性
- `tests/storage.rs` — 各同期先が `RemoteStorage` の約束を守るか（排他ディレクトリの奪い合いを含む）、Nextcloud ドライバ固有の動作
- `tests/cloud_drives.rs` — OneDrive・Google Drive ドライバ固有の動作（ログイン、トークン更新、スロットリング、
  ページング、分割アップロード、Google Drive の同名ファイル・再作成されたフォルダ・再送された作成）
- `tests/fuzz.rs` — 3 台の端末がランダムに編集・移動・削除・添付・履歴記録・同期し（通信失敗・応答消失・
  同時同期・暗号化のオン／オフ切り替えあり）、最後に全端末が同じ状態に収束し、行き場のないノートが残らないことを確認

模擬 Nextcloud は実 Nextcloud の癖（秒単位の更新時刻と ETag、親フォルダが無いと 409 など）を再現している。
模擬 OneDrive・Google Drive は各 API の仕様（OAuth トークンの期限切れ、ページング、分割アップロードの
単位、Google Drive の同名ファイルと ID 指定の作成など）を再現している。

```sh
# ランダムテストの回数を増やす / 失敗したシードを再現する
NISSHOKU_FUZZ_COUNT=500 cargo test --test fuzz
NISSHOKU_FUZZ_SEED=100050 NISSHOKU_FUZZ_TRACE=1 cargo test --test fuzz -- --nocapture

# 実 Nextcloud でのテスト（Docker）
docker run -d --name nisshoku-nextcloud -p 18080:80 -e SQLITE_DATABASE=nextcloud \
  -e NEXTCLOUD_ADMIN_USER=admin -e NEXTCLOUD_ADMIN_PASSWORD=<pw> \
  -e "NEXTCLOUD_TRUSTED_DOMAINS=localhost 127.0.0.1" nextcloud:apache
NISSHOKU_TEST_NEXTCLOUD_URL=http://localhost:18080 NISSHOKU_TEST_NEXTCLOUD_USER=admin \
  NISSHOKU_TEST_NEXTCLOUD_PASSWORD=<app password> cargo test
```

実 OneDrive・Google Drive でのテストには、アプリ登録（クライアント ID）とリフレッシュトークンが要る。
トークンは `cargo run --example login -- onedrive <client-id>`（または `gdrive <client-id> <secret>`）で
ブラウザからログインすると表示される。環境変数は `tests/common/mod.rs` の冒頭を参照。

開発用 CLI（`crates/nisshoku-sync/examples/nisshoku.rs`）で、シェルから同期を操作できる:

```sh
NISSHOKU_NEXTCLOUD_URL=... NISSHOKU_NEXTCLOUD_USER=... NISSHOKU_NEXTCLOUD_PASSWORD=... \
  cargo run --example nisshoku -- ./profile sync
```

## ロードマップ

- [x] データモデル・直列化・SQLite
- [x] 同期エンジン（競合処理、ロック、添付、中断からの再開）
- [x] ローカルフォルダ同期先
- [x] Nextcloud 同期先
- [x] エンドツーエンド暗号化
- [x] ノート履歴（リビジョン）
- [x] Tauri アプリ（エディタ・執筆機能・日記・同期設定）
- [x] OneDrive / Google Drive 同期先（模擬サーバーで検証済み）
- [x] Android 版
- [x] Linux 版（AppImage・.deb・.rpm）
- [x] 自動アップデートと GitHub Releases への自動リリース
- [x] 作品の管理（原稿・登場人物・プロット・資料）
- [ ] Dropbox

## 既知の制約

- Dropbox や OneDrive のアプリが同期しているフォルダを「ローカルフォルダ」同期先にして、複数の PC から
  同時に同期した場合の競合は防げない（各サービス側の競合コピーになる）。同じ PC・ネットワーク共有では安全。
- 2 台で同じノートの履歴を同時に記録すると、履歴の復元結果が近似になることがある（Joplin も同様）。
- 共有ノートブック（Joplin Server / Cloud の機能）は未対応。
- Joplin 自身は条件付き書き込みも排他ロックも使わないため、Joplin と日食がまったく同時に同じノートを
  書き込んだ場合の上書きは防げない（Joplin どうしでも同じ）。
