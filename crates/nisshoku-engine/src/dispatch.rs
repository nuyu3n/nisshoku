//! The commands by name, with their arguments as one JSON object (camelCase
//! names), as the window calls them: the Tauri window through its invoke
//! handler, the native Android app through its bridge.

use std::future::Future;
use std::pin::Pin;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::App;

/// What a command returns, as the JSON of its reply.
pub trait Reply {
    fn reply(self) -> Result<Value, String>;
}

impl<T: Serialize> Reply for Result<T, String> {
    fn reply(self) -> Result<Value, String> {
        self.and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
    }
}

impl Reply for () {
    fn reply(self) -> Result<Value, String> {
        Ok(Value::Null)
    }
}

impl Reply for String {
    fn reply(self) -> Result<Value, String> {
        Ok(Value::String(self))
    }
}

fn parse<T: DeserializeOwned>(cmd: &str, args: Value) -> Result<T, String> {
    let args = if args.is_null() { Value::Object(Default::default()) } else { args };
    serde_json::from_value(args).map_err(|e| format!("invalid arguments for {cmd}: {e}"))
}

/// A command's reply: right away, or a future to await.
pub enum Call {
    Done(Result<Value, String>),
    Later(Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>),
}

/// How a command is called: with the state (`state`), the app (`app`), both
/// (`app_state`), and the same awaited (`async_…`); `none` takes neither.
macro_rules! call {
    (state $app:ident $f:path; $($a:ident),*) => { Call::Done($f(&$app.state, $($a),*).reply()) };
    (app $app:ident $f:path; $($a:ident),*) => { Call::Done($f($app.clone(), $($a),*).reply()) };
    (app_state $app:ident $f:path; $($a:ident),*) => {
        Call::Done($f($app.clone(), &$app.state, $($a),*).reply())
    };
    (async_state $app:ident $f:path; $($a:ident),*) => {{
        let $app = $app.clone();
        Call::Later(Box::pin(async move { $f(&$app.state, $($a),*).await.reply() }))
    }};
    (async_app $app:ident $f:path; $($a:ident),*) => {{
        let $app = $app.clone();
        Call::Later(Box::pin(async move { $f($app, $($a),*).await.reply() }))
    }};
    (async_none $app:ident $f:path; $($a:ident),*) => {
        Call::Later(Box::pin(async move { $f($($a),*).await.reply() }))
    };
}

macro_rules! commands {
    ($( $kind:ident $m:ident :: $f:ident ( $($arg:ident : $ty:ty),* ) ),* $(,)?) => {
        /// The names of all commands.
        pub const COMMANDS: &[&str] = &[$(stringify!($f)),*];

        /// Starts the command `cmd` with its arguments: the commands that
        /// do not wait on the network run right here, in order.
        pub fn call(app: &App, cmd: &str, args: Value) -> Call {
            match cmd {
                $( stringify!($f) => {
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct Args { $($arg: $ty),* }
                    #[allow(unused_variables)]
                    let Args { $($arg),* } = match parse::<Args>(cmd, args) {
                        Ok(a) => a,
                        Err(e) => return Call::Done(Err(e)),
                    };
                    call!($kind app crate::$m::$f; $($arg),*)
                } )*
                _ => Call::Done(Err(format!("unknown command: {cmd}"))),
            }
        }
    };
}

/// Runs the command `cmd` with its arguments.
pub async fn invoke(app: &App, cmd: &str, args: Value) -> Result<Value, String> {
    match call(app, cmd, args) {
        Call::Done(r) => r,
        Call::Later(f) => f.await,
    }
}

commands! {
    state notes::list_folders(),
    state notes::create_folder(title: String, parent_id: Option<String>),
    state notes::rename_folder(id: String, title: String),
    state notes::move_folder(id: String, parent_id: String),
    state notes::set_folder_icon(id: String, emoji: Option<String>),
    state notes::set_folder_color(id: String, color: Option<String>),
    state notes::export_all(dir: String),
    state notes::trash_folder(id: String),
    state notes::list_notes(scope: crate::notes::NoteScope),
    state notes::get_note(id: String),
    state notes::create_note(parent_id: Option<String>, title: Option<String>, body: Option<String>),
    state notes::save_note(id: String, title: String, body: String, base_title: Option<String>, base_body: Option<String>),
    state notes::move_note(id: String, parent_id: String),
    state notes::set_note_goal(id: String, goal: Option<i64>),
    state notes::duplicate_note(id: String),
    state notes::trash_note(id: String),
    state notes::restore_note(id: String),
    state notes::delete_note_forever(id: String),
    state notes::empty_trash(),
    state notes::open_diary(date: String),
    state notes::diary_month(month: String),
    state notes::diary_streak(today: String),
    state notes::list_revisions(note_id: String),
    state notes::revision_content(note_id: String, revision_id: String),
    state notes::restore_revision(note_id: String, revision_id: String),
    state notes::save_snapshot(note_id: String),
    state notes::clear_history(note_id: String),
    state notes::resolve_conflict(id: String, keep: String),
    state notes::list_comments(note_id: Option<String>),
    state notes::add_comment(note_id: String, quote: String, before: String, text: String),
    state notes::update_comment(note_id: String, comment_id: String, text: Option<String>, resolved: Option<bool>),
    state notes::delete_comment(note_id: String, comment_id: String),
    state notes::get_stats(),
    state notes::get_preferences(),
    state notes::set_preferences(preferences: Value),
    app notes::write_text_file(path: String, contents: String),
    state storage::backup_now(dir: String),
    app_state storage::open_backups(),
    state storage::storage_info(),
    state storage::delete_unused_resources(),
    state features::list_tags(),
    state features::set_note_tags(note_id: String, titles: Vec<String>),
    state features::rename_tag(id: String, title: String),
    state features::delete_tag(id: String),
    state features::set_note_pinned(id: String, pinned: bool),
    state features::set_note_todo(id: String, todo: bool),
    state features::set_todo_completed(id: String, completed: bool),
    state features::set_notes_order(ids: Vec<String>),
    state features::set_note_order(id: String, order: f64),
    state features::list_backlinks(note_id: String),
    state features::save_as_template(note_id: String),
    state features::list_templates(),
    state features::save_resource(name: String, mime: String, data: Vec<u8>),
    state features::resource_files(),
    app_state features::import_resource(path: String),
    app_state features::open_resource(id: String, confirmed: Option<bool>),
    async_state features::clip_url(url: String, parent_id: Option<String>),
    state diary::diary_entries(query: Option<String>),
    state diary::on_this_day(date: String),
    state diary::set_diary_meta(note_id: String, mood: Option<String>, weather: Option<String>),
    state diary::diary_stats(today: String),
    app_state diary::diary_export(period: String, path: String),
    state diary::set_diary_events(note_id: String, events: Vec<crate::diary::DiaryEvent>),
    state diary::get_diary_memo(),
    state diary::set_diary_memo(items: Vec<crate::diary::MemoItem>),
    state sync::get_sync_settings(),
    app_state sync::set_local_target(path: String),
    app_state sync::disconnect_sync(),
    async_app sync::nextcloud_login_start(server: String),
    async_app sync::nextcloud_login_wait(folder: String),
    async_app sync::nextcloud_connect(server: String, user: String, password: String, folder: String),
    async_app sync::gdrive_login(folder: String, client_id: Option<String>, client_secret: Option<String>),
    async_app sync::onedrive_login(client_id: String, folder: String),
    async_app sync::set_master_password(password: String),
    async_app sync::set_encryption(enabled: bool, password: Option<String>),
    async_app sync::sync_now(),
    async_app sync::sync_unattended(),
    state sync::has_unsent_changes(),
    app_state sync::set_foreground(visible: bool),
    state sync::profile_dir(),
    state tasks::create_task(title: String, parent_id: Option<String>, due: Option<i64>, all_day: Option<bool>),
    state tasks::set_todo_due(id: String, due: i64, all_day: bool),
    state widgets::refresh_widgets(),
    state widgets::apply_widget_actions(),
    async_none updates::latest_release(),
    state works::set_work(folder_id: String, meta: Option<crate::works::WorkMeta>),
    state works::work_overview(work_id: String),
    state works::set_note_kind(note_id: String, kind: Option<String>),
    state works::set_chapter_meta(note_id: String, meta: crate::works::ChapterMeta),
    state works::create_work_note(work_id: String, kind: String, title: String),
    state works::get_character(note_id: String),
    state works::set_character(note_id: String, character: crate::works::Character),
    state works::open_plot(work_id: String, template: Option<String>),
    state works::set_plot(note_id: String, plot: crate::works::Plot),
    state works::work_manuscript(work_id: String),
    app_state epub::export_epub(work_id: String, path: String, vertical: bool, author: Option<String>),
}

/// `save_resource_bytes`: the file's bytes as the raw body of the call, its
/// name (URI-encoded) and type beside them (`x-name`, `x-mime` headers).
pub const RAW_SAVE_RESOURCE: &str = "save_resource_bytes";

pub fn invoke_raw(app: &App, data: Vec<u8>, name: &str, mime: &str) -> Result<Value, String> {
    crate::features::save_resource_bytes(&app.state, data, name, mime).reply()
}
