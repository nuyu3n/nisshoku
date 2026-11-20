//! 日色 — the app's commands over the Joplin-compatible data model and sync
//! engine, for every front end: the Tauri window (desktop) and the native
//! Android app. A front end opens the profile ([`Ctx::open`]), hands in what
//! only it can do ([`Host`]: events to its screens, opening links, system
//! notifications, files the system's pickers hand out), and calls the
//! commands by name ([`dispatch::invoke`]).

pub mod clip;
pub mod diary;
pub mod dispatch;
pub mod epub;
pub mod features;
pub mod files;
pub mod i18n;
pub mod notes;
pub mod secrets;
pub mod state;
pub mod storage;
pub mod sync;
pub mod tasks;
pub mod text;
pub mod updates;
pub mod userdata;
pub mod widgets;
pub mod works;

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use nisshoku_core::Database;
use serde::Serialize;
use serde_json::Value;

pub use dispatch::{COMMANDS, Call, call, invoke};
pub use state::AppState;

/// What only the front end can do.
pub trait Host: Send + Sync {
    /// An event for the screens (`sync-status`, `sync-progress`, `data-changed`).
    fn emit(&self, event: &str, payload: Value);
    /// Opens a web page in the browser.
    fn open_url(&self, url: &str) -> Result<(), String>;
    /// Opens a file or folder with the system's default app.
    fn open_path(&self, path: &str) -> Result<(), String>;
    /// Shows a system notification.
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
    /// Opens a `content://` URI of Android's file pickers.
    fn open_content(&self, uri: &str, write: bool) -> std::io::Result<std::fs::File>;
}

/// The host of no front end (tests, the background sync): does nothing.
pub struct NoHost;

impl Host for NoHost {
    fn emit(&self, _event: &str, _payload: Value) {}
    fn open_url(&self, _url: &str) -> Result<(), String> {
        Err("no window to open links from".into())
    }
    fn open_path(&self, _path: &str) -> Result<(), String> {
        Err("no window to open files from".into())
    }
    fn notify(&self, _title: &str, _body: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_content(&self, _uri: &str, _write: bool) -> std::io::Result<std::fs::File> {
        Err(std::io::Error::other("content URIs need the Android app"))
    }
}

/// Facts about the app the commands need.
pub struct Config {
    /// The application ID (`app.nisshoku.desktop`).
    pub identifier: String,
}

/// The open profile and its front end.
pub struct Ctx {
    pub state: AppState,
    host: RwLock<Arc<dyn Host>>,
    config: Config,
}

/// What the commands get to reach the front end.
pub type App = Arc<Ctx>;

impl Ctx {
    /// Opens the profile in `dir` (created if needed), with no front end yet.
    pub fn open(dir: &Path, identifier: &str) -> Result<App, Box<dyn std::error::Error>> {
        secrets::init(dir);
        std::fs::create_dir_all(dir)?;
        Ok(Arc::new(Ctx {
            state: make_state(dir)?,
            host: RwLock::new(Arc::new(NoHost)),
            config: Config {
                identifier: identifier.to_string(),
            },
        }))
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn host(&self) -> Arc<dyn Host> {
        self.host.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_host(&self, host: Arc<dyn Host>) {
        *self.host.write().unwrap_or_else(|e| e.into_inner()) = host;
    }

    pub fn emit<S: Serialize>(&self, event: &str, payload: S) -> Result<(), String> {
        let payload = serde_json::to_value(payload).map_err(|e| e.to_string())?;
        self.host().emit(event, payload);
        Ok(())
    }

    pub fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.host().notify(title, body)
    }

    pub fn opener(&self) -> Opener<'_> {
        Opener(self)
    }
}

/// Opening links and files (as Tauri's opener plugin).
pub struct Opener<'a>(&'a Ctx);

impl Opener<'_> {
    pub fn open_url(&self, url: impl AsRef<str>, _with: Option<&str>) -> Result<(), String> {
        self.0.host().open_url(url.as_ref())
    }

    pub fn open_path(&self, path: impl AsRef<str>, _with: Option<&str>) -> Result<(), String> {
        self.0.host().open_path(path.as_ref())
    }
}

/// The app state over the profile in `dir`.
pub fn make_state(dir: &Path) -> Result<AppState, Box<dyn std::error::Error>> {
    let resource_dir = dir.join("resources");
    std::fs::create_dir_all(&resource_dir)?;
    let db = Database::open(dir.join("database.sqlite"))?;
    let status = sync::load_status(&db);
    notes::apply_preferences(&notes::preferences(&db));
    Ok(AppState {
        db: Arc::new(Mutex::new(db)),
        data_dir: dir.to_path_buf(),
        resource_dir,
        sync_lock: tokio::sync::Mutex::new(()),
        status: Mutex::new(status),
        nextcloud_flow: Mutex::new(None),
        // Phones: in the background until the app says it is in front (a
        // process started for WorkManager has no screen; see `sync::run`).
        foreground: std::sync::atomic::AtomicBool::new(!sync::background_limited()),
        went_background: std::sync::atomic::AtomicBool::new(false),
        retry_on_return: std::sync::atomic::AtomicBool::new(false),
    })
}

/// The work of a front end's start, once its host is set: old trash out,
/// the daily backup, then (on the current Tokio runtime) the background
/// syncs, the history and the reminders of due tasks.
pub fn start(app: &App) {
    {
        let state = app.state();
        let db = state.db();
        let days = notes::preferences(&db)
            .get("trashRetentionDays")
            .and_then(|v| v.as_i64())
            .unwrap_or(30);
        if let Err(e) = notes::purge_trash(&db, days) {
            tracing::warn!("could not empty old trash: {e}");
        }
        if let Err(e) = storage::daily_backup(&db, &state.data_dir) {
            tracing::warn!("could not back up the database: {e}");
        }
    }
    tokio::spawn(sync::background(app.clone()));
    tokio::spawn(notes::keep_history(app.clone()));
    tokio::spawn(tasks::remind_due(app.clone()));
}
