//! 日色 — the desktop app: commands for the window, over the Joplin-compatible
//! data model and sync engine.

#[cfg(target_os = "android")]
mod android;
mod clip;
mod diary;
mod epub;
mod features;
mod files;
mod i18n;
mod notes;
mod secrets;
pub mod state;
mod storage;
mod sync;
mod tasks;
mod text;
mod updates;
mod userdata;
mod widgets;
mod works;

use std::sync::{Arc, Mutex};

use nisshoku_core::Database;
use tauri::Manager;

use state::AppState;

/// The commands callable from the window.
pub fn handlers<R: tauri::Runtime>()
-> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        notes::list_folders,
        notes::create_folder,
        notes::rename_folder,
        notes::move_folder,
        notes::set_folder_icon,
        notes::set_folder_color,
        notes::export_all,
        notes::trash_folder,
        notes::list_notes,
        notes::get_note,
        notes::create_note,
        notes::save_note,
        notes::move_note,
        notes::set_note_goal,
        notes::duplicate_note,
        notes::trash_note,
        notes::restore_note,
        notes::delete_note_forever,
        notes::empty_trash,
        notes::open_diary,
        notes::diary_month,
        notes::diary_streak,
        notes::list_revisions,
        notes::revision_content,
        notes::restore_revision,
        notes::save_snapshot,
        notes::clear_history,
        notes::resolve_conflict,
        storage::backup_now,
        storage::open_backups,
        storage::storage_info,
        storage::delete_unused_resources,
        features::list_tags,
        features::set_note_tags,
        features::rename_tag,
        features::delete_tag,
        features::set_note_pinned,
        features::set_note_todo,
        features::set_todo_completed,
        features::set_notes_order,
        features::set_note_order,
        features::list_backlinks,
        features::save_as_template,
        features::list_templates,
        features::save_resource,
        features::save_resource_bytes,
        features::resource_files,
        features::import_resource,
        features::open_resource,
        features::clip_url,
        diary::diary_entries,
        diary::on_this_day,
        diary::set_diary_meta,
        diary::diary_stats,
        diary::diary_export,
        diary::set_diary_events,
        diary::get_diary_memo,
        diary::set_diary_memo,
        notes::list_comments,
        notes::add_comment,
        notes::update_comment,
        notes::delete_comment,
        notes::get_stats,
        notes::get_preferences,
        notes::set_preferences,
        notes::write_text_file,
        sync::get_sync_settings,
        sync::set_local_target,
        sync::disconnect_sync,
        sync::nextcloud_login_start,
        sync::nextcloud_login_wait,
        sync::nextcloud_connect,
        sync::gdrive_login,
        sync::onedrive_login,
        sync::set_master_password,
        sync::set_encryption,
        sync::sync_now,
        sync::set_foreground,
        sync::profile_dir,
        tasks::create_task,
        tasks::set_todo_due,
        widgets::refresh_widgets,
        widgets::apply_widget_actions,
        updates::latest_release,
        updates::install_kind,
        works::set_work,
        works::work_overview,
        works::set_note_kind,
        works::set_chapter_meta,
        works::create_work_note,
        works::get_character,
        works::set_character,
        works::open_plot,
        works::set_plot,
        works::work_manuscript,
        epub::export_epub,
    ]
}

/// The app state over the profile in `dir`.
pub fn make_state(dir: &std::path::Path) -> Result<AppState, Box<dyn std::error::Error>> {
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
        foreground: std::sync::atomic::AtomicBool::new(true),
        went_background: std::sync::atomic::AtomicBool::new(false),
        retry_on_return: std::sync::atomic::AtomicBool::new(false),
    })
}

/// Shows the main window (created hidden, `tauri.conf.json`) in the colour
/// of the theme: the window and WebView2 would otherwise be white until the
/// page paints, a flash in the dark theme.
fn show_main_window(app: &tauri::App) {
    use tauri::window::Color;
    let Some(window) = app.get_webview_window("main") else { return };
    let theme = notes::preferences(&app.state::<AppState>().db())
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
            secrets::init(&profile);
            std::fs::create_dir_all(&profile)?;
            app.manage(make_state(&profile)?);
            // Attachments are shown from the profile's resources, wherever
            // the profile is (the configured scope covers the default one).
            if let Err(e) = app
                .asset_protocol_scope()
                .allow_directory(profile.join("resources"), true)
            {
                tracing::warn!("could not allow the resource folder: {e}");
            }

            // Old trash, then background syncs at the interval of the preferences.
            {
                let state = app.state::<AppState>();
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
            show_main_window(app);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(sync::background(handle));
            tauri::async_runtime::spawn(notes::keep_history(app.handle().clone()));
            tauri::async_runtime::spawn(tasks::remind_due(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(handlers())
        .run(tauri::generate_context!())
        .expect("error while running 日色");
}
