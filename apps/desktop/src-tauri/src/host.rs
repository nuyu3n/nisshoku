//! What the engine asks of the window (`nisshoku_engine::Host`), done with
//! Tauri and its plugins.

use serde_json::Value;
use tauri::{AppHandle, Emitter, Runtime};

pub struct TauriHost<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> nisshoku_engine::Host for TauriHost<R> {
    fn emit(&self, event: &str, payload: Value) {
        let _ = self.0.emit(event, payload);
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        self.0.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    }

    fn open_path(&self, path: &str) -> Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        self.0.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
    }

    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        use tauri_plugin_notification::NotificationExt;
        self.0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| e.to_string())
    }

    fn open_content(&self, uri: &str, write: bool) -> std::io::Result<std::fs::File> {
        use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
        let path: FilePath = uri.parse().map_err(std::io::Error::other)?;
        let mut opts = OpenOptions::new();
        opts.read(!write).write(write).create(write).truncate(write);
        self.0.fs().open(path, opts)
    }
}
