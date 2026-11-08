//! The commands called as the window calls them (JSON arguments with
//! camelCase names), on a mock app with a profile in a temporary directory.

use nisshoku_app_lib::{handlers, make_state};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

struct TestApp {
    _app: App<MockRuntime>,
    window: WebviewWindow<MockRuntime>,
    _dir: tempfile::TempDir,
}

impl TestApp {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = mock_builder()
            .manage(make_state(dir.path()).unwrap())
            .invoke_handler(handlers())
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        Self {
            _app: app,
            window,
            _dir: dir,
        }
    }

    fn try_call<T: DeserializeOwned>(&self, cmd: &str, args: Value) -> Result<T, Value> {
        get_ipc_response(
            &self.window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<T>().unwrap())
    }

    fn call<T: DeserializeOwned>(&self, cmd: &str, args: Value) -> T {
        self.try_call(cmd, args.clone())
            .unwrap_or_else(|e| panic!("{cmd} {args}: {e}"))
    }
}

fn ids(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn notes_notebooks_and_trash() {
    let t = TestApp::new();
    // A note without a notebook goes into a default one.
    let n: Value = t.call(
        "create_note",
        json!({ "parentId": null, "title": null, "body": null }),
    );
    let id = n["id"].as_str().unwrap().to_string();
    let folders: Value = t.call("list_folders", json!({}));
    assert_eq!(folders[0]["title"], "Notes");
    assert_eq!(folders[0]["noteCount"], 1);

    let summary: Value = t.call(
        "save_note",
        json!({ "id": id, "title": "第一章", "body": "# 霧\n\n朝の**港**には、まだ夜の名残があった。" }),
    );
    assert_eq!(summary["charCount"], 1 + 17);
    assert_eq!(summary["excerpt"], "霧 朝の港には、まだ夜の名残があった。");
    let stats: Value = t.call("get_stats", json!({}));
    assert_eq!(stats["today"], 18);
    assert_eq!(stats["streak"], 1);

    let book: String = t.call(
        "create_folder",
        json!({ "title": "長編", "parentId": null }),
    );
    t.call::<()>("move_note", json!({ "id": id, "parentId": book }));
    let in_book: Value = t.call(
        "list_notes",
        json!({ "scope": { "kind": "folder", "id": book } }),
    );
    assert_eq!(ids(&in_book), [id.as_str()]);
    let found: Value = t.call(
        "list_notes",
        json!({ "scope": { "kind": "search", "query": "港 名残" } }),
    );
    assert_eq!(ids(&found), [id.as_str()]);
    let none: Value = t.call(
        "list_notes",
        json!({ "scope": { "kind": "search", "query": "港 船" } }),
    );
    assert!(none.as_array().unwrap().is_empty());

    t.call::<()>("set_note_goal", json!({ "id": id, "goal": 4000 }));
    assert_eq!(
        t.call::<Value>("get_note", json!({ "id": id }))["goal"],
        4000
    );

    // The trash, with the notebook.
    t.call::<()>("trash_folder", json!({ "id": book }));
    let all: Value = t.call("list_notes", json!({ "scope": { "kind": "all" } }));
    assert!(all.as_array().unwrap().is_empty());
    let trash: Value = t.call("list_notes", json!({ "scope": { "kind": "trash" } }));
    assert_eq!(ids(&trash), [id.as_str()]);
    t.call::<()>("restore_note", json!({ "id": id }));
    let folders: Value = t.call("list_folders", json!({}));
    assert!(
        folders.as_array().unwrap().iter().any(|f| f["id"] == book),
        "notebook restored with the note"
    );

    t.call::<()>("trash_note", json!({ "id": id }));
    assert_eq!(t.call::<usize>("empty_trash", json!({})), 1);
    assert!(
        t.try_call::<Value>("get_note", json!({ "id": id }))
            .is_err()
    );
}

#[test]
fn diary_entries() {
    let t = TestApp::new();
    t.call::<()>(
        "set_preferences",
        json!({ "preferences": { "diaryTemplate": "## 今日のできごと\n" } }),
    );
    let entry: Value = t.call("open_diary", json!({ "date": "2026-11-04" }));
    assert_eq!(entry["title"], "2026-11-04");
    assert_eq!(entry["diaryDate"], "2026-11-04");
    assert_eq!(entry["body"], "## 今日のできごと\n");
    // The same day opens the same entry.
    let again: Value = t.call("open_diary", json!({ "date": "2026-11-04" }));
    assert_eq!(again["id"], entry["id"]);
    t.call::<Value>("open_diary", json!({ "date": "2026-11-03" }));

    let month: Value = t.call("diary_month", json!({ "month": "2026-11" }));
    assert_eq!(month.as_array().unwrap().len(), 2);
    assert_eq!(
        t.call::<usize>("diary_streak", json!({ "today": "2026-11-04" })),
        2
    );
    let list: Value = t.call("list_notes", json!({ "scope": { "kind": "diary" } }));
    assert_eq!(list[0]["diaryDate"], "2026-11-04", "newest first");
    let folders: Value = t.call("list_folders", json!({}));
    assert_eq!(folders[0]["isDiary"], true);
    assert_eq!(folders[0]["title"], "Diary");

    // An entry written elsewhere (Joplin) is dated by its title.
    let diary_id = folders[0]["id"].clone();
    t.call::<Value>(
        "create_note",
        json!({ "parentId": diary_id, "title": "2026/11/2 雨", "body": "" }),
    );
    let month: Value = t.call("diary_month", json!({ "month": "2026-11" }));
    let dates: Vec<&str> = month
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["date"].as_str().unwrap())
        .collect();
    assert_eq!(dates, ["2026-11-02", "2026-11-03", "2026-11-04"]);
    assert_eq!(
        t.call::<usize>("diary_streak", json!({ "today": "2026-11-04" })),
        3
    );

    // The title format is a preference.
    t.call::<()>(
        "set_preferences",
        json!({ "preferences": { "diaryTitleFormat": "ja" } }),
    );
    let entry: Value = t.call("open_diary", json!({ "date": "2026-11-05" }));
    assert_eq!(entry["title"], "2026年11月5日（木）");
}

#[test]
fn comments() {
    let t = TestApp::new();
    let n: Value = t.call(
        "create_note",
        json!({ "parentId": null, "title": "題", "body": "朝の港には霧があった。" }),
    );
    let id = n["id"].as_str().unwrap();
    let c: Value = t.call(
        "add_comment",
        json!({ "noteId": id, "quote": "霧", "before": "朝の港には", "text": "もっと具体的に" }),
    );
    let cid = c["id"].as_str().unwrap();
    t.call::<Value>(
        "add_comment",
        json!({ "noteId": id, "quote": "港", "before": "朝の", "text": "地名を入れる" }),
    );

    let all: Value = t.call("list_comments", json!({ "noteId": null }));
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert_eq!(all[1]["noteTitle"], "題");
    assert_eq!(all[1]["quote"], "霧");

    t.call::<()>(
        "update_comment",
        json!({ "noteId": id, "commentId": cid, "text": null, "resolved": true }),
    );
    t.call::<()>(
        "delete_comment",
        json!({ "noteId": id, "commentId": all[0]["id"] }),
    );
    let left: Value = t.call("list_comments", json!({ "noteId": id }));
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(left[0]["resolved"], true);
    // The text is untouched, and saving the text keeps the comments.
    t.call::<Value>(
        "save_note",
        json!({ "id": id, "title": "題", "body": "朝の港には濃い霧があった。" }),
    );
    assert_eq!(
        t.call::<Value>("list_comments", json!({ "noteId": id }))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        t.call::<Value>("get_note", json!({ "id": id }))["body"],
        "朝の港には濃い霧があった。"
    );
}

#[test]
fn history() {
    let t = TestApp::new();
    let n: Value = t.call(
        "create_note",
        json!({ "parentId": null, "title": "題", "body": "一稿" }),
    );
    let id = n["id"].as_str().unwrap();
    assert!(t.call::<bool>("save_snapshot", json!({ "noteId": id })));
    t.call::<Value>(
        "save_note",
        json!({ "id": id, "title": "題", "body": "二稿" }),
    );
    let revs: Value = t.call("list_revisions", json!({ "noteId": id }));
    let rev = revs[0]["id"].as_str().unwrap();
    let content: Value = t.call(
        "revision_content",
        json!({ "noteId": id, "revisionId": rev }),
    );
    assert_eq!(content["body"], "一稿");

    let restored: Value = t.call(
        "restore_revision",
        json!({ "noteId": id, "revisionId": rev }),
    );
    assert_eq!(restored["body"], "一稿");
    // The text replaced by the restore is in the history too.
    let revs: Value = t.call("list_revisions", json!({ "noteId": id }));
    let bodies: Vec<String> = revs
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            t.call::<Value>(
                "revision_content",
                json!({ "noteId": id, "revisionId": r["id"] }),
            )["body"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert!(bodies.contains(&"二稿".to_string()), "{bodies:?}");
}

#[test]
fn sync_settings_without_a_target() {
    let t = TestApp::new();
    let settings: Value = t.call("get_sync_settings", json!({}));
    assert_eq!(settings["target"]["kind"], "none");
    assert_eq!(settings["status"]["state"], "off");

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("sync");
    t.call::<()>(
        "set_local_target",
        json!({ "path": target.to_str().unwrap() }),
    );
    t.call::<Value>(
        "create_note",
        json!({ "parentId": null, "title": "同期", "body": "本文" }),
    );
    let status: Value = t.call("sync_now", json!({}));
    assert_eq!(status["state"], "idle", "{status}");
    assert_eq!(
        status["lastReport"]["created_remote"], 2,
        "a notebook and a note"
    );
    assert!(target.join("info.json").exists());
}

#[test]
fn onedrive_needs_a_client_id() {
    let t = TestApp::new();
    for client_id in ["", "my app", "0000000-0000-0000-0000-000000000000"] {
        let e = t
            .try_call::<()>(
                "onedrive_login",
                json!({ "clientId": client_id, "folder": "Nisshoku" }),
            )
            .unwrap_err();
        assert!(e.to_string().contains("ID"), "{e}");
    }
    let settings: Value = t.call("get_sync_settings", json!({}));
    assert_eq!(settings["target"]["kind"], "none");
}

#[test]
fn notebook_icons_and_export() {
    let t = TestApp::new();
    let book: String = t.call(
        "create_folder",
        json!({ "title": "長編/第一部", "parentId": null }),
    );
    t.call::<()>("set_folder_icon", json!({ "id": book, "emoji": "📘" }));
    let folders: Value = t.call("list_folders", json!({}));
    assert_eq!(folders[0]["icon"], "📘");
    t.call::<Value>(
        "create_note",
        json!({ "parentId": book, "title": "第一章", "body": "本文" }),
    );
    t.call::<Value>(
        "create_note",
        json!({ "parentId": book, "title": "第一章", "body": "同名" }),
    );

    let out = tempfile::tempdir().unwrap();
    let n: usize = t.call("export_all", json!({ "dir": out.path().to_str().unwrap() }));
    assert_eq!(n, 2);
    let dir = out.path().join("長編_第一部");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["第一章 (2).md", "第一章.md"]);
    let text = std::fs::read_to_string(dir.join("第一章.md")).unwrap()
        + &std::fs::read_to_string(dir.join("第一章 (2).md")).unwrap();
    assert!(text.contains("# 第一章\n\n本文") && text.contains("# 第一章\n\n同名"));

    t.call::<()>("set_folder_icon", json!({ "id": book, "emoji": null }));
    let folders: Value = t.call("list_folders", json!({}));
    assert_eq!(folders[0]["icon"], "");
}

#[test]
fn tags_pins_todos_order_links_templates_images() {
    let t = TestApp::new();
    let a: Value = t.call(
        "create_note",
        json!({ "parentId": null, "title": "一", "body": "" }),
    );
    let b: Value = t.call(
        "create_note",
        json!({ "parentId": a["parentId"], "title": "二", "body": "" }),
    );
    let (a_id, b_id) = (a["id"].as_str().unwrap(), b["id"].as_str().unwrap());

    t.call::<()>(
        "set_note_tags",
        json!({ "noteId": a_id, "titles": ["推敲中", "長編"] }),
    );
    t.call::<()>(
        "set_note_tags",
        json!({ "noteId": b_id, "titles": ["推敲中"] }),
    );
    let tags: Value = t.call("list_tags", json!({}));
    assert_eq!(tags.as_array().unwrap().len(), 2);
    let draft = tags
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["title"] == "推敲中")
        .unwrap();
    assert_eq!(draft["noteCount"], 2);
    let tagged: Value = t.call(
        "list_notes",
        json!({ "scope": { "kind": "tag", "id": draft["id"] } }),
    );
    assert_eq!(tagged.as_array().unwrap().len(), 2);
    let all: Value = t.call("list_notes", json!({ "scope": { "kind": "all" } }));
    let first = all
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == a_id)
        .unwrap();
    assert_eq!(first["tags"], json!(["推敲中", "長編"]));
    t.call::<()>(
        "set_note_tags",
        json!({ "noteId": a_id, "titles": ["長編"] }),
    );
    let tagged: Value = t.call(
        "list_notes",
        json!({ "scope": { "kind": "tag", "id": draft["id"] } }),
    );
    assert_eq!(ids(&tagged), [b_id]);

    t.call::<()>("set_note_pinned", json!({ "id": b_id, "pinned": true }));
    assert_eq!(
        ids(&t.call::<Value>("list_notes", json!({ "scope": { "kind": "pinned" } }))),
        [b_id]
    );
    t.call::<()>(
        "set_todo_completed",
        json!({ "id": a_id, "completed": true }),
    );
    let todo: Value = t.call("list_notes", json!({ "scope": { "kind": "todo" } }));
    assert_eq!(todo[0]["isTodo"], true);
    assert_eq!(todo[0]["todoCompleted"], true);

    t.call::<()>("set_notes_order", json!({ "ids": [b_id, a_id] }));
    let all: Value = t.call("list_notes", json!({ "scope": { "kind": "all" } }));
    let order = |id: &str| {
        all.as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == id)
            .unwrap()["order"]
            .as_f64()
            .unwrap()
    };
    assert!(
        order(b_id) > order(a_id),
        "first in the list has the larger order"
    );

    t.call::<Value>(
        "save_note",
        json!({ "id": b_id, "title": "二", "body": format!("[一へ](:/{a_id})") }),
    );
    let back: Value = t.call("list_backlinks", json!({ "noteId": a_id }));
    assert_eq!(ids(&back), [b_id]);

    let tpl: String = t.call("save_as_template", json!({ "noteId": b_id }));
    let templates: Value = t.call("list_templates", json!({}));
    assert_eq!(ids(&templates), [tpl.as_str()]);
    let all: Value = t.call("list_notes", json!({ "scope": { "kind": "all" } }));
    assert!(!ids(&all).contains(&tpl), "templates are not in all notes");

    let r: Value = t.call(
        "save_resource",
        json!({ "name": "写真.PNG", "mime": "image/png", "data": [137, 80, 78, 71] }),
    );
    assert!(r["path"].as_str().unwrap().ends_with(".png"));
    assert_eq!(
        std::fs::read(r["path"].as_str().unwrap()).unwrap(),
        [137, 80, 78, 71]
    );
    let files: Value = t.call("resource_files", json!({}));
    assert_eq!(files[0]["id"], r["id"]);
}

#[test]
fn diary_features() {
    let t = TestApp::new();
    let a: Value = t.call("open_diary", json!({ "date": "2025-11-05" }));
    let b: Value = t.call("open_diary", json!({ "date": "2026-11-04" }));
    t.call::<Value>("open_diary", json!({ "date": "2026-11-05" }));
    t.call::<Value>(
        "save_note",
        json!({ "id": b["id"], "title": b["title"], "body": "雨の日。本を読んだ。" }),
    );
    t.call::<()>(
        "set_diary_meta",
        json!({ "noteId": b["id"], "mood": "😊", "weather": "☔" }),
    );

    // Diary entries are not in "all notes".
    let all: Value = t.call("list_notes", json!({ "scope": { "kind": "all" } }));
    assert!(all.as_array().unwrap().is_empty(), "{all}");

    let entries: Value = t.call("diary_entries", json!({ "query": null }));
    assert_eq!(entries.as_array().unwrap().len(), 3);
    assert_eq!(entries[0]["date"], "2026-11-05", "newest first");
    let found: Value = t.call("diary_entries", json!({ "query": "本" }));
    assert_eq!(found[0]["mood"], "😊");
    assert_eq!(found[0]["weather"], "☔");

    let past: Value = t.call("on_this_day", json!({ "date": "2026-11-05" }));
    assert_eq!(past[0]["noteId"], a["id"]);

    let stats: Value = t.call("diary_stats", json!({ "today": "2026-11-05" }));
    assert_eq!(stats["streak"], 2);
    assert_eq!(stats["longestStreak"], 2);
    assert_eq!(stats["monthDays"], 2);
    assert_eq!(stats["totalDays"], 3);
    assert_eq!(stats["moods"][0][0], "😊");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("2026.txt");
    let n: usize = t.call(
        "diary_export",
        json!({ "period": "2026", "path": path.to_str().unwrap() }),
    );
    assert_eq!(n, 2);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with("■ 2026-11-04　😊 ☔\n\n雨の日。本を読んだ。"),
        "{text}"
    );
}

#[test]
fn diary_events_and_memo() {
    let t = TestApp::new();
    let e: Value = t.call("open_diary", json!({ "date": "2026-11-05" }));
    t.call::<()>("set_diary_events", json!({ "noteId": e["id"], "events": [
        { "time": "14:05", "text": "打ち合わせ" }, { "time": "09:30", "text": "散歩" }, { "time": "10:00", "text": " " }
    ] }));
    let list: Value = t.call("diary_entries", json!({ "query": "打ち合わせ" }));
    assert_eq!(
        list[0]["events"],
        json!([{ "time": "09:30", "text": "散歩" }, { "time": "14:05", "text": "打ち合わせ" }])
    );

    let memo: Value = t.call("get_diary_memo", json!({}));
    assert_eq!(memo["items"], json!([]));
    t.call::<()>("set_diary_memo", json!({ "items": [{ "text": "傘を返す", "done": false }, { "text": "本を買う", "done": true }] }));
    let memo: Value = t.call("get_diary_memo", json!({}));
    assert_eq!(
        memo["items"][1],
        json!({ "text": "本を買う", "done": true })
    );
    let note: Value = t.call("get_note", json!({ "id": memo["noteId"] }));
    assert_eq!(note["body"], "- [ ] 傘を返す\n- [x] 本を買う");
    // The memo is not an entry.
    assert_eq!(
        t.call::<Value>("diary_entries", json!({ "query": null }))
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
