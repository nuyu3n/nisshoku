//! 日色 — the engine for the native Android app (`apps/android`), through
//! UniFFI: the app opens the profile ([`Engine::open`]) with what only it
//! can do ([`NativeHost`]), and calls the commands by name with JSON, as the
//! desktop window does (`nisshoku_engine::dispatch`).

use std::sync::Arc;

use nisshoku_engine::{App, Call, Ctx, Host};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum EngineError {
    /// A command failed; the message is for the user.
    #[error("{reason}")]
    Failed { reason: String },
}

impl From<String> for EngineError {
    fn from(reason: String) -> Self {
        EngineError::Failed { reason }
    }
}

/// What only the app can do (see `nisshoku_engine::Host`).
#[uniffi::export(with_foreign)]
pub trait NativeHost: Send + Sync {
    /// An event for the screens, its payload as JSON.
    fn emit(&self, event: String, payload: String);
    /// Opens a web page; false when it could not.
    fn open_url(&self, url: String) -> bool;
    /// Opens a file with the system's app for it; false when it could not.
    fn open_path(&self, path: String) -> bool;
    fn notify(&self, title: String, body: String);
    /// A `content://` URI opened as a file descriptor the engine then owns
    /// (`ParcelFileDescriptor.detachFd`), or -1.
    fn open_content(&self, uri: String, write: bool) -> i32;
}

struct Bridge(Arc<dyn NativeHost>);

impl Host for Bridge {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.0.emit(event.to_string(), payload.to_string());
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        if self.0.open_url(url.to_string()) { Ok(()) } else { Err(format!("could not open {url}")) }
    }

    fn open_path(&self, path: &str) -> Result<(), String> {
        if self.0.open_path(path.to_string()) { Ok(()) } else { Err(format!("could not open {path}")) }
    }

    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.0.notify(title.to_string(), body.to_string());
        Ok(())
    }

    #[cfg(unix)]
    fn open_content(&self, uri: &str, write: bool) -> std::io::Result<std::fs::File> {
        use std::os::fd::FromRawFd;
        let fd = self.0.open_content(uri.to_string(), write);
        if fd < 0 {
            return Err(std::io::Error::other(format!("could not open {uri}")));
        }
        // SAFETY: the descriptor was detached for us, and no one else owns it.
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }

    #[cfg(not(unix))]
    fn open_content(&self, uri: &str, _write: bool) -> std::io::Result<std::fs::File> {
        Err(std::io::Error::other(format!("could not open {uri}")))
    }
}

/// The open profile, with the runtime its background work runs on.
#[derive(uniffi::Object)]
pub struct Engine {
    app: App,
    runtime: tokio::runtime::Runtime,
}

#[uniffi::export]
impl Engine {
    /// Opens the profile in `dir` and starts the background work (syncs at
    /// the interval of the preferences, history, reminders of due tasks).
    #[uniffi::constructor]
    pub fn open(dir: String, identifier: String, host: Arc<dyn NativeHost>) -> Result<Arc<Self>, EngineError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("nisshoku")
            .build()
            .map_err(|e| e.to_string())?;
        let app = Ctx::open(std::path::Path::new(&dir), &identifier).map_err(|e| e.to_string())?;
        app.set_host(Arc::new(Bridge(host)));
        runtime.block_on(async { nisshoku_engine::start(&app) });
        Ok(Arc::new(Engine { app, runtime }))
    }

    /// Runs the command `cmd` with its arguments (a JSON object); the reply
    /// as JSON. Commands that only touch the database run on the caller's
    /// thread, in order; those that wait on the network on the engine's.
    pub async fn invoke(&self, cmd: String, args: String) -> Result<String, EngineError> {
        let args: serde_json::Value = if args.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&args).map_err(|e| e.to_string())?
        };
        let reply = match nisshoku_engine::call(&self.app, &cmd, args) {
            Call::Done(r) => r,
            Call::Later(f) => self.runtime.spawn(f).await.map_err(|e| e.to_string())?,
        };
        Ok(reply?.to_string())
    }

    /// [`Engine::invoke`] for the commands that only touch the database, on
    /// the caller's thread. Fails for those that wait on the network.
    pub fn invoke_now(&self, cmd: String, args: String) -> Result<String, EngineError> {
        let args: serde_json::Value = if args.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&args).map_err(|e| e.to_string())?
        };
        match nisshoku_engine::call(&self.app, &cmd, args) {
            Call::Done(r) => Ok(r?.to_string()),
            Call::Later(_) => Err(format!("{cmd} has to be awaited").into()),
        }
    }

    /// Adds a file as an attachment (`save_resource_bytes`).
    pub fn save_resource(&self, data: Vec<u8>, name: String, mime: String) -> Result<String, EngineError> {
        Ok(nisshoku_engine::dispatch::invoke_raw(&self.app, data, &name, &mime)?.to_string())
    }

    /// The names of the commands.
    pub fn commands(&self) -> Vec<String> {
        nisshoku_engine::COMMANDS.iter().map(|c| c.to_string()).collect()
    }
}

/// The background sync of WorkManager (the app closed): syncs the profile
/// in `dir` on its own. 0 synced, 1 failed (retry later), 2 nothing to do.
#[uniffi::export]
pub fn background_sync(dir: String) -> i32 {
    nisshoku_engine::sync::background_sync(std::path::Path::new(&dir))
}

#[cfg(target_os = "android")]
mod android;
