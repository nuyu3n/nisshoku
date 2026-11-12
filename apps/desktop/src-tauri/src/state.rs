use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard};

use nisshoku_core::Database;
use nisshoku_sync::SharedDb;
use nisshoku_sync::drivers::nextcloud::LoginFlow;

use crate::sync::SyncStatus;

pub struct AppState {
    pub db: SharedDb,
    /// The profile's directory (database, widget snapshot).
    pub data_dir: PathBuf,
    pub resource_dir: PathBuf,
    /// Held for the duration of a sync, so that only one runs at a time.
    pub sync_lock: tokio::sync::Mutex<()>,
    pub status: Mutex<SyncStatus>,
    /// A Nextcloud sign-in waiting for the browser.
    pub nextcloud_flow: Mutex<Option<LoginFlow>>,
    /// Whether the window is in front (phones: the system may cut the
    /// network of apps in the background).
    pub foreground: AtomicBool,
    /// Set when the app went to the background (since the last sync began).
    pub went_background: AtomicBool,
    /// A sync failed while in the background: run it again on return.
    pub retry_on_return: AtomicBool,
}

impl AppState {
    pub fn db(&self) -> MutexGuard<'_, Database> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}
