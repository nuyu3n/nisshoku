//! Sync settings, sign-in to the services, and running syncs (on demand and
//! every few minutes in the background).

use std::sync::Arc;
use std::time::Duration;

use nisshoku_core::Database;
use nisshoku_sync::drivers::google_drive::{GoogleDriveEndpoints, google_drive_oauth};
use nisshoku_sync::drivers::nextcloud::{poll_login_flow, start_login_flow};
use nisshoku_sync::drivers::onedrive::{OneDriveEndpoints, onedrive_oauth};
use nisshoku_sync::drivers::{
    BrowserLogin, FileSystemStorage, GoogleDriveConfig, GoogleDriveStorage, NextcloudConfig,
    NextcloudStorage, OAuthProvider, OAuthTokens, OneDriveConfig, OneDriveStorage,
};
use nisshoku_sync::{RemoteStorage, SyncError, SyncOptions, SyncReport, Synchronizer};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;

use crate::i18n::tr;
use crate::notes::{CmdResult, err};
use crate::secrets;
use crate::state::AppState;
use crate::trf;

const TARGET_SETTING: &str = "app.sync.target";
const TARGET_ID_SETTING: &str = "app.sync.targetId";
const LAST_SYNC_SETTING: &str = "app.sync.lastSync";
/// Syncs in the background: at start (unless turned off) and then at the
/// interval of the preferences (`syncInterval`, minutes; 0 for manual only).
pub async fn background<R: Runtime>(app: AppHandle<R>) {
    let prefs = |app: &AppHandle<R>| crate::notes::preferences(&app.state::<AppState>().db());
    tokio::time::sleep(Duration::from_secs(3)).await;
    if prefs(&app).get("syncOnStart").and_then(|v| v.as_bool()) != Some(false) {
        run_sync(&app).await;
    }
    let mut last = std::time::Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(20)).await;
        let minutes = prefs(&app)
            .get("syncInterval")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);
        if minutes > 0 && last.elapsed() >= Duration::from_secs(minutes * 60) {
            run_sync(&app).await;
            last = std::time::Instant::now();
        }
    }
}

/// OAuth client of this build for Google Drive (a "Desktop app" client),
/// given at build time.
fn gdrive_client() -> Option<(&'static str, &'static str)> {
    Some((
        option_env!("NISSHOKU_GDRIVE_CLIENT_ID").filter(|s| !s.is_empty())?,
        option_env!("NISSHOKU_GDRIVE_CLIENT_SECRET").filter(|s| !s.is_empty())?,
    ))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TargetConfig {
    #[default]
    None,
    Local {
        path: String,
    },
    Nextcloud {
        server: String,
        user: String,
        folder: String,
    },
    /// `client_id` is empty for the OAuth client built into the app, or the
    /// user's own "Desktop app" client (its secret is kept with the secrets).
    #[serde(rename_all = "camelCase")]
    GoogleDrive {
        folder: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        client_id: String,
    },
    /// With the user's own app registration in Microsoft Entra ID (the
    /// client ID is not a secret, so it lives here).
    #[serde(rename_all = "camelCase")]
    OneDrive {
        client_id: String,
        folder: String,
    },
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// `off`, `idle`, `syncing` or `error`.
    pub state: String,
    pub message: Option<String>,
    pub last_sync: i64,
    /// The target is encrypted and the password is missing or wrong.
    pub needs_password: bool,
    pub last_report: Option<SyncReport>,
}

fn target(db: &Database) -> TargetConfig {
    db.setting(TARGET_SETTING)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

const TARGET_IDS_SETTING: &str = "app.sync.targetIds";

/// Switching to another target starts its sync state afresh. Coming back to
/// a target used before (signing in again, after a disconnect) takes up its
/// state again: otherwise everything would be compared anew (conflict
/// copies for every difference) and deletions not yet sent would be lost.
fn set_target(db: &Database, config: &TargetConfig) -> CmdResult<()> {
    let key = serde_json::to_string(config).map_err(err)?;
    let mut known: std::collections::BTreeMap<String, i64> = db
        .setting(TARGET_IDS_SETTING)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let current = target(db);
    if current != TargetConfig::None {
        known.insert(serde_json::to_string(&current).map_err(err)?, target_id(db));
    }
    let id = match known.get(&key) {
        Some(id) if *config != TargetConfig::None => *id,
        _ => {
            let next = db
                .setting(TARGET_ID_SETTING)
                .ok()
                .flatten()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0)
                .max(known.values().copied().max().unwrap_or(0))
                + 1;
            if *config != TargetConfig::None {
                known.insert(key.clone(), next);
            }
            next
        }
    };
    let same = current == *config;
    db.set_setting(TARGET_IDS_SETTING, &serde_json::to_string(&known).map_err(err)?)
        .map_err(err)?;
    db.set_setting(TARGET_ID_SETTING, &id.to_string()).map_err(err)?;
    db.set_setting(TARGET_SETTING, &key).map_err(err)?;
    if !same {
        db.set_setting(LAST_SYNC_SETTING, "0").map_err(err)?;
    }
    Ok(())
}

/// Changing the target while a sync runs would mix the two up.
fn not_syncing(state: &AppState) -> CmdResult<tokio::sync::MutexGuard<'_, ()>> {
    state.sync_lock.try_lock().map_err(|_| {
        tr(
            "同期中です。終わってからもう一度お試しください",
            "A sync is running. Please try again when it is done",
        )
        .to_string()
    })
}

fn target_id(db: &Database) -> i64 {
    db.setting(TARGET_ID_SETTING)
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn initial_status(db: &Database) -> SyncStatus {
    let last_sync = db
        .setting(LAST_SYNC_SETTING)
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let state = if target(db) == TargetConfig::None {
        "off"
    } else {
        "idle"
    };
    SyncStatus {
        state: state.into(),
        last_sync,
        ..Default::default()
    }
}

pub fn load_status(db: &Database) -> SyncStatus {
    initial_status(db)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSettings {
    target: TargetConfig,
    status: SyncStatus,
    gdrive_available: bool,
    has_master_password: bool,
}

#[tauri::command]
pub fn get_sync_settings(state: State<'_, AppState>) -> CmdResult<SyncSettings> {
    let target = target(&state.db());
    Ok(SyncSettings {
        target,
        status: state.status.lock().unwrap().clone(),
        gdrive_available: gdrive_client().is_some(),
        has_master_password: secrets::get(secrets::MASTER_PASSWORD).is_some(),
    })
}

fn publish<R: Runtime>(app: &AppHandle<R>, status: SyncStatus) {
    *app.state::<AppState>().status.lock().unwrap() = status.clone();
    let _ = app.emit("sync-status", status);
}

fn reset_status<R: Runtime>(app: &AppHandle<R>) {
    let status = initial_status(&app.state::<AppState>().db());
    publish(app, status);
}

// -------------------------------------------------------------------------
// Choosing a target

#[tauri::command]
pub fn set_local_target<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<()> {
    std::fs::create_dir_all(&path).map_err(|e| {
        trf!(
            "フォルダを作成できません: {e}",
            "Could not create the folder: {e}"
        )
    })?;
    let _guard = not_syncing(&state)?;
    set_target(&state.db(), &TargetConfig::Local { path })?;
    reset_status(&app);
    Ok(())
}

#[tauri::command]
pub fn disconnect_sync<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> CmdResult<()> {
    let _guard = not_syncing(&state)?;
    set_target(&state.db(), &TargetConfig::None)?;
    secrets::remove(secrets::NEXTCLOUD_PASSWORD);
    secrets::remove(secrets::GDRIVE_REFRESH_TOKEN);
    secrets::remove(secrets::GDRIVE_CLIENT_SECRET);
    secrets::remove(secrets::ONEDRIVE_REFRESH_TOKEN);
    reset_status(&app);
    Ok(())
}

/// Starts the Nextcloud sign-in: opens the server's login page in the browser.
#[tauri::command]
pub async fn nextcloud_login_start<R: Runtime>(
    app: AppHandle<R>,
    server: String,
) -> CmdResult<String> {
    let flow = start_login_flow(&server).await.map_err(|e| {
        trf!(
            "Nextcloud に接続できません。URL を確認してください: {e}",
            "Could not reach Nextcloud. Check the URL: {e}"
        )
    })?;
    let url = flow.login_url.clone();
    *app.state::<AppState>().nextcloud_flow.lock().unwrap() = Some(flow);
    let _ = app.opener().open_url(&url, None::<&str>);
    Ok(url)
}

/// Waits until the sign-in started by [`nextcloud_login_start`] completes.
#[tauri::command]
pub async fn nextcloud_login_wait<R: Runtime>(app: AppHandle<R>, folder: String) -> CmdResult<()> {
    let flow = app
        .state::<AppState>()
        .nextcloud_flow
        .lock()
        .unwrap()
        .clone()
        .ok_or(tr("ログインが開始されていません", "No sign-in in progress"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(20 * 60);
    let creds = loop {
        if let Some(c) = poll_login_flow(&flow).await.map_err(err)? {
            break c;
        }
        if std::time::Instant::now() > deadline {
            return Err(tr(
                "ログインの待ち時間が過ぎました。もう一度お試しください",
                "The sign-in timed out. Please try again",
            )
            .into());
        }
        // Cancelled by starting another sign-in.
        let current = app
            .state::<AppState>()
            .nextcloud_flow
            .lock()
            .unwrap()
            .clone();
        if current.as_ref() != Some(&flow) {
            return Err(tr("ログインを取り消しました", "The sign-in was cancelled").into());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    };
    finish_nextcloud(
        &app,
        &creds.server,
        &creds.login_name,
        &creds.app_password,
        &folder,
    )
    .await
}

/// Connects with an app password typed by the user.
#[tauri::command]
pub async fn nextcloud_connect<R: Runtime>(
    app: AppHandle<R>,
    server: String,
    user: String,
    password: String,
    folder: String,
) -> CmdResult<()> {
    finish_nextcloud(&app, &server, &user, &password, &folder).await
}

async fn finish_nextcloud<R: Runtime>(
    app: &AppHandle<R>,
    server: &str,
    user: &str,
    password: &str,
    folder: &str,
) -> CmdResult<()> {
    let folder = if folder.trim().is_empty() {
        "Nisshoku".to_string()
    } else {
        folder.trim().to_string()
    };
    let config = NextcloudConfig {
        server_url: server.into(),
        username: user.into(),
        password: password.into(),
        folder: folder.clone(),
    };
    NextcloudStorage::connect(&config).await.map_err(|e| {
        trf!(
            "Nextcloud に接続できません: {e}",
            "Could not connect to Nextcloud: {e}"
        )
    })?;
    let state = app.state::<AppState>();
    let _guard = not_syncing(&state)?;
    secrets::set(secrets::NEXTCLOUD_PASSWORD, password)?;
    set_target(
        &state.db(),
        &TargetConfig::Nextcloud {
            server: server.into(),
            user: user.into(),
            folder,
        },
    )?;
    *state.nextcloud_flow.lock().unwrap() = None;
    reset_status(app);
    Ok(())
}

/// Signs in to Google in the browser and uses `folder` of My Drive. Without
/// `client_id`, the OAuth client built into the app is used; with it, the
/// user's own "Desktop app" client and its `client_secret`.
#[tauri::command]
pub async fn gdrive_login<R: Runtime>(
    app: AppHandle<R>,
    folder: String,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> CmdResult<()> {
    let own = client_id
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    let (client_id, secret) = match &own {
        Some(id) => {
            if !id.ends_with(".apps.googleusercontent.com") || id.contains(char::is_whitespace) {
                return Err(tr(
                    "クライアント ID を確認してください（….apps.googleusercontent.com の形式）",
                    "Check the client ID (it ends with .apps.googleusercontent.com)",
                )
                .into());
            }
            let secret = client_secret
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .ok_or(tr(
                    "クライアント シークレットを入力してください",
                    "Enter the client secret",
                ))?;
            (id.clone(), secret)
        }
        None => {
            let (id, secret) = gdrive_client().ok_or(tr(
                "このビルドには Google のクライアントが入っていません。クライアント ID とシークレットを入力してください",
                "This build has no Google client built in. Enter a client ID and secret",
            ))?;
            (id.to_string(), secret.to_string())
        }
    };
    let provider = google_drive_oauth(&client_id, &secret, &GoogleDriveEndpoints::default());
    let tokens = browser_login(&app, provider).await?;
    let folder = folder_or_default(&folder);
    let config = GoogleDriveConfig {
        client_id: client_id.clone(),
        client_secret: secret.clone(),
        folder: folder.clone(),
        tokens: tokens.clone(),
        endpoints: Default::default(),
    };
    GoogleDriveStorage::connect(&config, None)
        .await
        .map_err(|e| {
            trf!(
                "Google Drive に接続できません: {e}",
                "Could not connect to Google Drive: {e}"
            )
        })?;
    let state = app.state::<AppState>();
    let _guard = not_syncing(&state)?;
    secrets::set(secrets::GDRIVE_REFRESH_TOKEN, &tokens.refresh_token)?;
    if own.is_some() {
        secrets::set(secrets::GDRIVE_CLIENT_SECRET, &secret)?;
    } else {
        secrets::remove(secrets::GDRIVE_CLIENT_SECRET);
    }
    set_target(
        &app.state::<AppState>().db(),
        &TargetConfig::GoogleDrive {
            folder,
            client_id: own.unwrap_or_default(),
        },
    )?;
    reset_status(&app);
    Ok(())
}

/// Signs in to Microsoft in the browser with the user's own app
/// registration and uses `folder` of their OneDrive.
#[tauri::command]
pub async fn onedrive_login<R: Runtime>(
    app: AppHandle<R>,
    client_id: String,
    folder: String,
) -> CmdResult<()> {
    let client_id = client_id.trim().to_string();
    if !is_guid(&client_id) {
        return Err(tr(
            "アプリケーション (クライアント) ID を確認してください（xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx の形式）",
            "Check the application (client) ID (it looks like xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)",
        )
        .into());
    }
    let provider = onedrive_oauth(&client_id, &OneDriveEndpoints::default());
    let tokens = browser_login(&app, provider).await?;
    let folder = folder_or_default(&folder);
    let config = onedrive_config(&client_id, &folder, tokens.clone());
    OneDriveStorage::connect(&config, None).await.map_err(|e| {
        trf!(
            "OneDrive に接続できません: {e}",
            "Could not connect to OneDrive: {e}"
        )
    })?;
    let state = app.state::<AppState>();
    let _guard = not_syncing(&state)?;
    secrets::set(secrets::ONEDRIVE_REFRESH_TOKEN, &tokens.refresh_token)?;
    set_target(
        &app.state::<AppState>().db(),
        &TargetConfig::OneDrive { client_id, folder },
    )?;
    reset_status(&app);
    Ok(())
}

fn is_guid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.chars().all(|c| c.is_ascii_hexdigit()))
}

fn onedrive_config(client_id: &str, folder: &str, tokens: OAuthTokens) -> OneDriveConfig {
    OneDriveConfig {
        client_id: client_id.into(),
        folder: folder.into(),
        tokens,
        endpoints: Default::default(),
    }
}

/// Opens the provider's sign-in page in the browser and waits for it.
async fn browser_login<R: Runtime>(
    app: &AppHandle<R>,
    provider: OAuthProvider,
) -> CmdResult<OAuthTokens> {
    let login = BrowserLogin::start(provider).await.map_err(err)?;
    // On Android the browser page offers a button that brings the app back
    // to the front: the code exchange needs the network, which the system
    // may block while the app is in the background.
    #[cfg(target_os = "android")]
    let login = login.with_return_link(format!(
        "intent:#Intent;action=android.intent.action.MAIN;\
         category=android.intent.category.LAUNCHER;package={};end",
        app.config().identifier
    ));
    let _ = app.opener().open_url(login.url(), None::<&str>);
    tokio::time::timeout(Duration::from_secs(20 * 60), login.finish())
        .await
        .map_err(|_| {
            tr(
                "ログインの待ち時間が過ぎました。もう一度お試しください",
                "The sign-in timed out. Please try again",
            )
            .to_string()
        })?
        .map_err(err)
}

fn folder_or_default(folder: &str) -> String {
    if folder.trim().is_empty() {
        "Nisshoku".to_string()
    } else {
        folder.trim().to_string()
    }
}

/// The client of a saved Google Drive target: the user's own (`client_id`
/// non-empty, secret from the secrets) or the one built into the app.
fn gdrive_config(folder: &str, client_id: &str, tokens: OAuthTokens) -> Option<GoogleDriveConfig> {
    let (client_id, secret) = if client_id.is_empty() {
        let (id, secret) = gdrive_client()?;
        (id.to_string(), secret.to_string())
    } else {
        (client_id.to_string(), secrets::get(secrets::GDRIVE_CLIENT_SECRET)?)
    };
    Some(GoogleDriveConfig {
        client_id,
        client_secret: secret,
        folder: folder.into(),
        tokens,
        endpoints: Default::default(),
    })
}

// -------------------------------------------------------------------------
// Encryption

#[tauri::command]
pub async fn set_master_password<R: Runtime>(
    app: AppHandle<R>,
    password: String,
) -> CmdResult<SyncStatus> {
    secrets::set(secrets::MASTER_PASSWORD, &password)?;
    Ok(run_sync(&app).await)
}

#[tauri::command]
pub async fn set_encryption<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
    password: Option<String>,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let _guard = state.sync_lock.lock().await;
    let mut sync = synchronizer(&state)
        .await?
        .ok_or(tr("同期先が設定されていません", "No sync target is set up"))?;
    if enabled {
        let password = password.filter(|p| !p.is_empty()).ok_or(tr(
            "暗号化のパスワードを入力してください",
            "Enter the encryption password",
        ))?;
        sync.enable_encryption(&password).await.map_err(describe)?;
        secrets::set(secrets::MASTER_PASSWORD, &password)?;
    } else {
        sync.disable_encryption().await.map_err(describe)?;
    }
    Ok(())
}

// -------------------------------------------------------------------------
// Syncing

async fn storage(config: &TargetConfig) -> Result<Option<Arc<dyn RemoteStorage>>, String> {
    Ok(Some(match config {
        TargetConfig::None => return Ok(None),
        TargetConfig::Local { path } => Arc::new(FileSystemStorage::new(path)),
        TargetConfig::Nextcloud {
            server,
            user,
            folder,
        } => {
            let password = secrets::get(secrets::NEXTCLOUD_PASSWORD).ok_or(tr(
                "Nextcloud のパスワードが見つかりません。もう一度ログインしてください",
                "The Nextcloud password is missing. Please sign in again",
            ))?;
            let config = NextcloudConfig {
                server_url: server.clone(),
                username: user.clone(),
                password,
                folder: folder.clone(),
            };
            Arc::new(NextcloudStorage::connect(&config).await.map_err(describe)?)
        }
        TargetConfig::GoogleDrive { folder, client_id } => {
            let refresh_token = secrets::get(secrets::GDRIVE_REFRESH_TOKEN).ok_or(tr(
                "Google にもう一度ログインしてください",
                "Please sign in to Google again",
            ))?;
            let tokens = OAuthTokens {
                refresh_token,
                ..Default::default()
            };
            let config = gdrive_config(folder, client_id, tokens).ok_or(tr(
                "Google のクライアントが見つかりません。同期先を設定し直してください",
                "The Google client is missing. Please set up the sync target again",
            ))?;
            // Google may hand out a new refresh token; keep it.
            let listener: nisshoku_sync::drivers::TokenListener = Arc::new(|t: &OAuthTokens| {
                if !t.refresh_token.is_empty() {
                    if let Err(e) = secrets::set(secrets::GDRIVE_REFRESH_TOKEN, &t.refresh_token) {
                        tracing::error!("could not keep the new refresh token: {e}");
                    }
                }
            });
            Arc::new(
                GoogleDriveStorage::connect(&config, Some(listener))
                    .await
                    .map_err(describe)?,
            )
        }
        TargetConfig::OneDrive { client_id, folder } => {
            let refresh_token = secrets::get(secrets::ONEDRIVE_REFRESH_TOKEN).ok_or(tr(
                "Microsoft にもう一度ログインしてください",
                "Please sign in to Microsoft again",
            ))?;
            let tokens = OAuthTokens {
                refresh_token,
                ..Default::default()
            };
            // Microsoft rotates refresh tokens; keep the latest.
            let listener: nisshoku_sync::drivers::TokenListener = Arc::new(|t: &OAuthTokens| {
                if !t.refresh_token.is_empty() {
                    if let Err(e) = secrets::set(secrets::ONEDRIVE_REFRESH_TOKEN, &t.refresh_token) {
                        tracing::error!("could not keep the new refresh token: {e}");
                    }
                }
            });
            Arc::new(
                OneDriveStorage::connect(
                    &onedrive_config(client_id, folder, tokens),
                    Some(listener),
                )
                .await
                .map_err(describe)?,
            )
        }
    }))
}

async fn synchronizer(state: &AppState) -> Result<Option<Synchronizer>, String> {
    let (config, target_id) = {
        let db = state.db();
        (target(&db), target_id(&db))
    };
    let Some(storage) = storage(&config).await? else {
        return Ok(None);
    };
    Ok(Some(Synchronizer::new(
        state.db.clone(),
        storage,
        SyncOptions {
            target_id,
            resource_dir: state.resource_dir.clone(),
            client_type: if cfg!(target_os = "android") { "mobile" } else { "desktop" }.into(),
            master_password: secrets::get(secrets::MASTER_PASSWORD),
            mutex_timing: Default::default(),
        },
    )))
}

fn describe(e: SyncError) -> String {
    match e {
        SyncError::Auth(_) => tr(
            "ログインの有効期限が切れました。設定からもう一度ログインしてください",
            "The sign-in has expired. Please sign in again in Settings",
        )
        .into(),
        SyncError::Locked(_) => tr(
            "ほかの端末が同期中です。しばらくしてからもう一度お試しください",
            "Another device is syncing. Please try again shortly",
        )
        .into(),
        SyncError::EncryptionPasswordRequired => tr(
            "同期先は暗号化されています。暗号化のパスワードを入力してください",
            "The sync target is encrypted. Enter the encryption password",
        )
        .into(),
        SyncError::WrongEncryptionPassword => tr(
            "暗号化のパスワードが違います",
            "The encryption password is wrong",
        )
        .into(),
        SyncError::IncompatibleTarget(m) => trf!(
            "この同期先は使えません: {m}",
            "This sync target cannot be used: {m}"
        ),
        e => e.to_string(),
    }
}

#[tauri::command]
pub async fn sync_now<R: Runtime>(app: AppHandle<R>) -> CmdResult<SyncStatus> {
    Ok(run_sync(&app).await)
}

/// Phones cut the network of apps in the background (so a sync started
/// there, or still running when the app left, fails: the Google Drive or
/// OneDrive token cannot be refreshed, requests are cut). Syncs wait for the
/// front, and errors from the background are not shown but retried.
fn background_limited() -> bool {
    cfg!(target_os = "android")
}

/// The window came to the front or went to the background.
#[tauri::command]
pub fn set_foreground<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, visible: bool) {
    use std::sync::atomic::Ordering::SeqCst;
    state.foreground.store(visible, SeqCst);
    if !visible {
        state.went_background.store(true, SeqCst);
        return;
    }
    if !background_limited() {
        return;
    }
    // Back in front: run what failed meanwhile, or a sync if the last one is
    // a while ago (changes from other devices, and ours made just before
    // leaving, which could not be sent).
    let stale = state.status.lock().map(|s| nisshoku_core::time::now_ms() - s.last_sync > 60_000).unwrap_or(true);
    if state.retry_on_return.swap(false, SeqCst) || stale {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // (A moment for the network to come back.)
            tokio::time::sleep(Duration::from_millis(800)).await;
            run_sync(&app).await;
        });
    }
}

/// Runs a sync unless one is running, publishing the status as it goes.
pub async fn run_sync<R: Runtime>(app: &AppHandle<R>) -> SyncStatus {
    let state = app.state::<AppState>();
    let Ok(_guard) = state.sync_lock.try_lock() else {
        return state.status.lock().unwrap().clone();
    };
    use std::sync::atomic::Ordering::SeqCst;
    if background_limited() && !state.foreground.load(SeqCst) {
        // In the background: on return (see `set_foreground`).
        state.retry_on_return.store(true, SeqCst);
        return state.status.lock().unwrap().clone();
    }
    state.went_background.store(false, SeqCst);
    let mut status = state.status.lock().unwrap().clone();
    if target(&state.db()) == TargetConfig::None {
        status = initial_status(&state.db());
        publish(app, status.clone());
        return status;
    }
    status.state = "syncing".into();
    status.message = None;
    publish(app, status.clone());

    // Errors come with whether they are about the encryption password.
    let result: Result<SyncReport, (String, bool)> = async {
        let sync = synchronizer(&state)
            .await
            .map_err(|m| (m, false))?
            .ok_or_else(|| {
                (
                    tr("同期先が設定されていません", "No sync target is set up").to_string(),
                    false,
                )
            })?;
        let failed = |e: SyncError| {
            let needs_password = matches!(
                e,
                SyncError::EncryptionPasswordRequired | SyncError::WrongEncryptionPassword
            );
            (describe(e), needs_password)
        };
        let mut report = sync.sync().await.map_err(failed)?;
        // Entries of the same day written on two devices: merge them and
        // send the merge right away, before the other device makes its own.
        let merged = crate::diary::merge_duplicates(&state.db()).unwrap_or_else(|e| {
            tracing::warn!("could not merge diary duplicates: {e}");
            false
        });
        if merged {
            let again = sync.sync().await.map_err(failed)?;
            report.created_local += again.created_local;
            report.updated_local += again.updated_local + 1;
            report.deleted_local += again.deleted_local;
            report.errors.extend(again.errors);
        }
        Ok(report)
    }
    .await;

    match result {
        Ok(report) => {
            let now = nisshoku_core::time::now_ms();
            let _ = state.db().set_setting(LAST_SYNC_SETTING, &now.to_string());
            let changed_locally =
                report.created_local + report.updated_local + report.deleted_local > 0;
            status = SyncStatus {
                state: "idle".into(),
                message: (!report.errors.is_empty()).then(|| {
                    let n = report.errors.len();
                    trf!(
                        "{n} 件の項目を同期できませんでした",
                        "{n} items could not be synced"
                    )
                }),
                last_sync: now,
                needs_password: false,
                last_report: Some(report),
            };
            if changed_locally {
                let _ = app.emit("data-changed", ());
            }
        }
        // The app left during the sync: the error is most likely the network
        // the system cut. Not shown; tried again on return.
        Err((message, false))
            if background_limited()
                && (state.went_background.load(SeqCst) || !state.foreground.load(SeqCst)) =>
        {
            tracing::info!("sync failed in the background, retried on return: {message}");
            state.retry_on_return.store(true, SeqCst);
            status.state = "idle".into();
            status.message = None;
        }
        Err((message, needs_password)) => {
            status.state = "error".into();
            status.message = Some(message);
            status.needs_password = needs_password;
        }
    }
    publish(app, status.clone());
    // The widgets show synced tasks and diary entries too.
    if cfg!(target_os = "android")
        && let Err(e) = crate::widgets::write(&state.data_dir, &state.db())
    {
        tracing::warn!("could not update the widgets: {e}");
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onedrive_target_round_trips() {
        let config = TargetConfig::OneDrive {
            client_id: "1b2c3d4e-0000-4a5b-9c8d-0123456789ab".into(),
            folder: "Apps/Joplin".into(),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "oneDrive",
                "clientId": "1b2c3d4e-0000-4a5b-9c8d-0123456789ab",
                "folder": "Apps/Joplin"
            })
        );
        assert_eq!(
            serde_json::from_value::<TargetConfig>(json).unwrap(),
            config
        );
    }

    #[test]
    fn google_drive_targets_keep_the_client() {
        // Saved before own clients existed: the built-in client.
        let old: TargetConfig =
            serde_json::from_value(serde_json::json!({"kind": "googleDrive", "folder": "Nisshoku"}))
                .unwrap();
        assert_eq!(
            old,
            TargetConfig::GoogleDrive {
                folder: "Nisshoku".into(),
                client_id: String::new()
            }
        );
        assert_eq!(
            serde_json::to_value(&old).unwrap(),
            serde_json::json!({"kind": "googleDrive", "folder": "Nisshoku"})
        );
        let own = TargetConfig::GoogleDrive {
            folder: "Notes".into(),
            client_id: "123-abc.apps.googleusercontent.com".into(),
        };
        let json = serde_json::to_value(&own).unwrap();
        assert_eq!(json["clientId"], "123-abc.apps.googleusercontent.com");
        assert_eq!(serde_json::from_value::<TargetConfig>(json).unwrap(), own);
    }

    #[test]
    fn client_ids_are_guids() {
        assert!(is_guid("1b2c3d4e-0000-4a5b-9c8d-0123456789AB"));
        assert!(!is_guid("1b2c3d4e-0000-4a5b-9c8d-0123456789a"));
        assert!(!is_guid("1b2c3d4e00004a5b9c8d0123456789ab"));
        assert!(!is_guid("1b2c3d4e-0000-4a5b-9c8d-0123456789ag"));
    }
}
