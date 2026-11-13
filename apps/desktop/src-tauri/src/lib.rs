//! 日色 — the desktop app: commands for the window, over the Joplin-compatible
//! data model and sync engine.

#[cfg(target_os = "android")]
mod android;
mod host;

use nisshoku_engine::{App, Call};
use serde_json::Value;
use tauri::Manager;
use tauri::ipc::{Invoke, InvokeBody};

pub use nisshoku_engine::Ctx;

/// How this copy of the app was installed, which decides how it updates:
/// `nsis`, `msi`, `appimage`, `deb`, `rpm`, `android`, or `dev` for a build
/// that is not an installed package.
#[tauri::command]
fn install_kind() -> &'static str {
    if cfg!(target_os = "android") {
        return "android";
    }
    if cfg!(debug_assertions) {
        return "dev";
    }
    match tauri::utils::platform::bundle_type() {
        Some(tauri::utils::config::BundleType::Nsis) => "nsis",
        Some(tauri::utils::config::BundleType::Msi) => "msi",
        Some(tauri::utils::config::BundleType::AppImage) => "appimage",
        Some(tauri::utils::config::BundleType::Deb) => "deb",
        Some(tauri::utils::config::BundleType::Rpm) => "rpm",
        _ => "dev",
    }
}

/// The commands callable from the window: the engine's (by name, see
/// `nisshoku_engine::dispatch`), and the few that need Tauri itself.
pub fn handlers<R: tauri::Runtime>()
-> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    let own: Box<dyn Fn(Invoke<R>) -> bool + Send + Sync> =
        Box::new(tauri::generate_handler![install_kind]);
    move |invoke: Invoke<R>| {
        let cmd = invoke.message.command().to_string();
        let raw = cmd == nisshoku_engine::dispatch::RAW_SAVE_RESOURCE;
        if !raw && !nisshoku_engine::COMMANDS.contains(&cmd.as_str()) {
            return own(invoke);
        }
        let Some(app) = invoke
            .message
            .webview()
            .try_state::<App>()
            .map(|s| s.inner().clone())
        else {
            invoke.resolver.reject("the profile is not open");
            return true;
        };
        let reply = if raw {
            let header = |name: &str| {
                invoke
                    .message
                    .headers()
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string()
            };
            match invoke.message.payload() {
                InvokeBody::Raw(data) => Call::Done(nisshoku_engine::dispatch::invoke_raw(
                    &app,
                    data.clone(),
                    &header("x-name"),
                    &header("x-mime"),
                )),
                InvokeBody::Json(_) => Call::Done(Err("expected the file's bytes".into())),
            }
        } else {
            let args = match invoke.message.payload() {
                InvokeBody::Json(v) => v.clone(),
                InvokeBody::Raw(_) => Value::Null,
            };
            nisshoku_engine::call(&app, &cmd, args)
        };
        let resolver = invoke.resolver;
        match reply {
            Call::Done(Ok(v)) => resolver.resolve(v),
            Call::Done(Err(e)) => resolver.reject(e),
            Call::Later(f) => {
                tauri::async_runtime::spawn(async move {
                    match f.await {
                        Ok(v) => resolver.resolve(v),
                        Err(e) => resolver.reject(e),
                    }
                });
            }
        }
        true
    }
}

/// The app over the profile in `dir`, for the window (or the tests) to manage.
pub fn make_state(dir: &std::path::Path) -> Result<App, Box<dyn std::error::Error>> {
    nisshoku_engine::Ctx::open(dir, "app.nisshoku.desktop")
}

/// Shows the main window (created hidden, `tauri.conf.json`) in the colour
/// of the theme: the window and WebView2 would otherwise be white until the
/// page paints, a flash in the dark theme.
fn show_main_window(app: &tauri::App) {
    use tauri::window::Color;
    let Some(window) = app.get_webview_window("main") else { return };
    let theme = nisshoku_engine::notes::preferences(&app.state::<App>().state.db())
        .get("theme")
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let dark = match theme.as_str() {
        "dark" => true,
        "light" => false,
        // The system's: what the page will pick with prefers-color-scheme.
        _ => window.theme().is_ok_and(|t| t == tauri::Theme::Dark),
    };
    // styles/tokens.css --bg (and the startup screen's background).
    let background = if dark { Color(0x26, 0x25, 0x22, 0xff) } else { Color(0xfa, 0xf9, 0xf5, 0xff) };
    if let Err(e) = window.set_background_color(Some(background)) {
        tracing::warn!("could not set the window background: {e}");
    }
    // (Desktop only: on phones the activity shows it.)
    #[cfg(desktop)]
    if let Err(e) = window.show() {
        tracing::warn!("could not show the window: {e}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // New versions are installed by the updater plugin on desktop; Android
    // checks the releases itself (`updates::latest_release`).
    // One window over the profile: a second launch brings the first to the
    // front (two would overwrite each other's edits and sync at once).
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    }));
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // `NISSHOKU_PROFILE` puts the notes elsewhere (to try things out, or
            // to carry the app on a drive).
            let profile = match std::env::var_os("NISSHOKU_PROFILE") {
                Some(dir) => std::path::PathBuf::from(dir),
                None => app.path().app_data_dir()?,
            };
            #[cfg(target_os = "android")]
            {
                if let Err(e) = android::init_tls() {
                    tracing::error!("could not set up certificate checks: {e}");
                }
            }
            let ctx = nisshoku_engine::Ctx::open(&profile, &app.config().identifier)?;
            ctx.set_host(std::sync::Arc::new(host::TauriHost(app.handle().clone())));
            app.manage(ctx.clone());
            // Attachments are shown from the profile's resources, wherever
            // the profile is (the configured scope covers the default one).
            if let Err(e) = app
                .asset_protocol_scope()
                .allow_directory(profile.join("resources"), true)
            {
                tracing::warn!("could not allow the resource folder: {e}");
            }
            show_main_window(app);
            // Old trash, the daily backup, then background syncs at the
            // interval of the preferences, the history, the reminders.
            tauri::async_runtime::block_on(async move { nisshoku_engine::start(&ctx) });
            Ok(())
        })
        .invoke_handler(handlers())
        .run(tauri::generate_context!())
        .expect("error while running 日色");
}
