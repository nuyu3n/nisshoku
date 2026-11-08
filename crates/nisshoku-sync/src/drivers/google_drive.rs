//! Google Drive sync target, over the Drive API v3.
//!
//! Items live in a folder of the user's My Drive, laid out like any other
//! target. With the `drive.file` scope the app sees only the files it created
//! (on any device signed in to the same account), nothing else of the drive.
//!
//! Drive addresses files by ID and allows several files with the same name in
//! a folder, so this driver:
//! - resolves paths by name, folder by folder, caching the IDs. Among files
//!   sharing a name the oldest (by creation time, then ID) is the file; the
//!   others are ignored and removed with it;
//! - creates files with IDs reserved beforehand (`generateIds`), so a create
//!   that is retried after a lost response cannot produce a second file;
//! - emulates conditional writes by comparing the file's `version` first. As
//!   with Nextcloud this is not atomic: the sync mutex keeps this app's
//!   clients from writing at the same time;
//! - implements [`RemoteStorage::mkdir_exclusive`] as an election: every
//!   candidate creates its folder, waits a moment and keeps it only if it is
//!   the oldest.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{Method, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::drivers::http::{self, RetryPolicy, parse_time};
use crate::drivers::oauth::{OAuthProvider, OAuthTokens, TokenListener, TokenSource};
use crate::error::{Result, SyncError};
use crate::locks::MUTEX_DIR;
use crate::storage::{Precondition, RemoteStat, RemoteStorage};

const SERVICE: &str = "Google Drive";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
const FIELDS: &str = "id,name,mimeType,modifiedTime,createdTime,version";

pub const GOOGLE_DRIVE_SCOPES: &[&str] = &["https://www.googleapis.com/auth/drive.file"];

/// Larger files are uploaded with resumable uploads.
pub const DEFAULT_RESUMABLE_LIMIT: u64 = 5 * 1024 * 1024;
/// Resumable upload chunks must be multiples of 256 KiB.
pub const UPLOAD_CHUNK_UNIT: u64 = 256 * 1024;
const DEFAULT_UPLOAD_CHUNK: u64 = 32 * UPLOAD_CHUNK_UNIT;
const DEFAULT_ELECTION_SETTLE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoogleDriveEndpoints {
    pub api: String,
    pub upload: String,
    pub auth: String,
    pub token: String,
}

impl Default for GoogleDriveEndpoints {
    fn default() -> Self {
        Self {
            api: "https://www.googleapis.com/drive/v3".into(),
            upload: "https://www.googleapis.com/upload/drive/v3".into(),
            auth: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
        }
    }
}

/// The OAuth provider for a "Desktop app" OAuth client of a Google Cloud
/// project.
pub fn google_drive_oauth(
    client_id: &str,
    client_secret: &str,
    endpoints: &GoogleDriveEndpoints,
) -> OAuthProvider {
    OAuthProvider {
        auth_url: endpoints.auth.clone(),
        token_url: endpoints.token.clone(),
        client_id: client_id.into(),
        client_secret: Some(client_secret.into()),
        scopes: GOOGLE_DRIVE_SCOPES.iter().map(|s| s.to_string()).collect(),
        redirect_host: "127.0.0.1".into(),
        extra_auth_params: vec![
            ("access_type".into(), "offline".into()),
            ("prompt".into(), "consent".into()),
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoogleDriveConfig {
    pub client_id: String,
    pub client_secret: String,
    /// Folder of My Drive that holds the items, e.g. `Nisshoku`.
    pub folder: String,
    pub tokens: OAuthTokens,
    #[serde(default)]
    pub endpoints: GoogleDriveEndpoints,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriveFile {
    id: String,
    name: String,
    mime_type: String,
    modified_time: Option<String>,
    created_time: Option<String>,
    version: Option<String>,
}

impl DriveFile {
    fn is_dir(&self) -> bool {
        self.mime_type == FOLDER_MIME
    }

    /// Among files with the same name, the smallest key is the file.
    fn precedence(&self) -> (i64, String) {
        let created = self.created_time.as_deref().and_then(parse_time);
        (created.unwrap_or(i64::MAX), self.id.clone())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileList {
    files: Vec<DriveFile>,
    next_page_token: Option<String>,
}

pub struct GoogleDriveStorage {
    client: reqwest::Client,
    tokens: TokenSource,
    retry: RetryPolicy,
    api: String,
    upload: String,
    /// The sync folder's path in My Drive, without leading or trailing `/`.
    folder: String,
    /// Folder IDs by absolute path (`""` is My Drive).
    dirs: Mutex<HashMap<String, String>>,
    /// File IDs by absolute path, from listings and lookups.
    files: Mutex<HashMap<String, String>>,
    /// IDs reserved for files to create.
    reserved_ids: tokio::sync::Mutex<Vec<String>>,
    resumable_limit: u64,
    upload_chunk: u64,
    election_settle: Duration,
}

fn join(a: &str, b: &str) -> String {
    let (a, b) = (a.trim_matches('/'), b.trim_matches('/'));
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a}/{b}"),
    }
}

fn split_parent(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

/// Escapes a string for a literal in a Drive query.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn stat_of(path: String, f: &DriveFile) -> RemoteStat {
    RemoteStat {
        path,
        updated_time: f.modified_time.as_deref().and_then(parse_time).unwrap_or(0),
        is_dir: f.is_dir(),
        etag: f.version.clone(),
    }
}

enum UploadTarget<'a> {
    Create { parent: &'a str, name: &'a str },
    Update { id: &'a str },
}

impl GoogleDriveStorage {
    pub fn new(config: &GoogleDriveConfig, listener: Option<TokenListener>) -> Result<Self> {
        let provider =
            google_drive_oauth(&config.client_id, &config.client_secret, &config.endpoints);
        if config.folder.trim_matches('/').is_empty() {
            return Err(SyncError::Remote(format!(
                "{SERVICE}: a folder name is required"
            )));
        }
        Ok(Self {
            client: http::client()?,
            tokens: TokenSource::new(provider, config.tokens.clone(), listener)?,
            retry: RetryPolicy::default(),
            api: config.endpoints.api.trim_end_matches('/').to_string(),
            upload: config.endpoints.upload.trim_end_matches('/').to_string(),
            folder: config.folder.trim_matches('/').to_string(),
            dirs: Mutex::new(HashMap::from([(String::new(), "root".to_string())])),
            files: Mutex::new(HashMap::new()),
            reserved_ids: tokio::sync::Mutex::new(Vec::new()),
            resumable_limit: DEFAULT_RESUMABLE_LIMIT,
            upload_chunk: DEFAULT_UPLOAD_CHUNK,
            election_settle: DEFAULT_ELECTION_SETTLE,
        })
    }

    /// Creates the storage and its folder (checking the sign-in on the way).
    pub async fn connect(
        config: &GoogleDriveConfig,
        listener: Option<TokenListener>,
    ) -> Result<Self> {
        let storage = Self::new(config, listener)?;
        storage.mkdir("").await?;
        Ok(storage)
    }

    pub fn set_retry(&mut self, retry: RetryPolicy) {
        self.retry = retry;
    }

    /// Files over `limit` bytes are uploaded in chunks of `chunk` (rounded
    /// down to a multiple of [`UPLOAD_CHUNK_UNIT`]).
    pub fn set_upload_limits(&mut self, limit: u64, chunk: u64) {
        self.resumable_limit = limit;
        self.upload_chunk = (chunk / UPLOAD_CHUNK_UNIT).max(1) * UPLOAD_CHUNK_UNIT;
    }

    /// How long candidates for [`RemoteStorage::mkdir_exclusive`] wait before
    /// checking who won.
    pub fn set_election_settle(&mut self, settle: Duration) {
        self.election_settle = settle;
    }

    pub async fn tokens(&self) -> OAuthTokens {
        self.tokens.tokens().await
    }

    fn abs(&self, path: &str) -> String {
        join(&self.folder, path)
    }

    async fn send(
        &self,
        method: &Method,
        url: &str,
        build: impl Fn(RequestBuilder) -> RequestBuilder,
    ) -> Result<Response> {
        http::send(
            &self.client,
            Some(&self.tokens),
            &self.retry,
            SERVICE,
            method,
            url,
            build,
        )
        .await
    }

    async fn json<T: serde::de::DeserializeOwned>(resp: Response, what: &str) -> Result<T> {
        resp.json()
            .await
            .map_err(|e| SyncError::Remote(format!("{SERVICE}: invalid {what}: {e}")))
    }

    // ---------------------------------------------------------------------
    // Path resolution

    async fn query(&self, q: &str) -> Result<Vec<DriveFile>> {
        let mut out = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut url = format!(
                "{}/files?q={}&fields={}&pageSize=1000&spaces=drive",
                self.api,
                encode(q),
                encode(&format!("nextPageToken,files({FIELDS})"))
            );
            if let Some(t) = &page_token {
                url.push_str(&format!("&pageToken={}", encode(t)));
            }
            let resp = self.send(&Method::GET, &url, |r| r).await?;
            if !resp.status().is_success() {
                return Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await);
            }
            let page: FileList = Self::json(resp, "file list").await?;
            out.extend(page.files);
            match page.next_page_token {
                Some(t) => page_token = Some(t),
                None => return Ok(out),
            }
        }
    }

    /// Files named `name` in a folder, the one that counts first.
    async fn children_named(&self, parent_id: &str, name: &str) -> Result<Vec<DriveFile>> {
        let mut files = self
            .query(&format!(
                "{} in parents and name = {} and trashed = false",
                quote(parent_id),
                quote(name)
            ))
            .await?;
        files.sort_by_key(DriveFile::precedence);
        Ok(files)
    }

    /// Folders that can disappear while we hold their ID are not cached.
    fn cacheable(abs_path: &str, folder: &str) -> bool {
        let rel = abs_path
            .strip_prefix(folder)
            .unwrap_or(abs_path)
            .trim_start_matches('/');
        !rel.starts_with(MUTEX_DIR)
    }

    fn cache_dir(&self, abs: &str, id: &str) {
        if Self::cacheable(abs, &self.folder) {
            self.dirs
                .lock()
                .unwrap()
                .insert(abs.to_string(), id.to_string());
        }
    }

    fn cache_file(&self, abs: &str, id: &str) {
        if Self::cacheable(abs, &self.folder) {
            self.files
                .lock()
                .unwrap()
                .insert(abs.to_string(), id.to_string());
        }
    }

    /// Forgets cached IDs at and below `abs`.
    fn forget(&self, abs: &str) {
        let below = format!("{abs}/");
        let keep = |k: &String| k != abs && !k.starts_with(&below);
        self.dirs
            .lock()
            .unwrap()
            .retain(|k, _| k.is_empty() || keep(k));
        self.files.lock().unwrap().retain(|k, _| keep(k));
    }

    /// The ID of the folder at `abs`, creating it (and its parents) if
    /// `create` is set.
    async fn resolve_dir(&self, abs: &str, create: bool) -> Result<Option<String>> {
        if let Some(id) = self.dirs.lock().unwrap().get(abs) {
            return Ok(Some(id.clone()));
        }
        let mut current = "root".to_string();
        let mut prefix = String::new();
        for segment in abs.split('/').filter(|s| !s.is_empty()) {
            prefix = join(&prefix, segment);
            let cached = self.dirs.lock().unwrap().get(&prefix).cloned();
            current = match cached {
                Some(id) => id,
                None => {
                    let found = self
                        .children_named(&current, segment)
                        .await?
                        .into_iter()
                        .find(DriveFile::is_dir);
                    match found {
                        Some(f) => f.id,
                        None if create => self.create_folder(&current, segment).await?,
                        None => return Ok(None),
                    }
                }
            };
            self.cache_dir(&prefix, &current);
        }
        Ok(Some(current))
    }

    /// Creates a folder, returning the ID of the folder that counts (ours, or
    /// an older one created at the same time by someone else).
    async fn create_folder(&self, parent_id: &str, name: &str) -> Result<String> {
        let ours = self.create_folder_raw(parent_id, name).await?;
        let first = self
            .children_named(parent_id, name)
            .await?
            .into_iter()
            .find(DriveFile::is_dir);
        match first {
            Some(f) if f.id != ours => {
                self.delete_id(&ours).await?;
                Ok(f.id)
            }
            _ => Ok(ours),
        }
    }

    async fn create_folder_raw(&self, parent_id: &str, name: &str) -> Result<String> {
        let id = self.reserve_id().await?;
        let url = format!("{}/files?fields={}", self.api, encode(FIELDS));
        let body = serde_json::json!({
            "id": id,
            "name": name,
            "mimeType": FOLDER_MIME,
            "parents": [parent_id],
        });
        let resp = self.send(&Method::POST, &url, |r| r.json(&body)).await?;
        if resp.status().is_success() || self.exists(&id).await? {
            return Ok(id);
        }
        Err(http::unexpected(SERVICE, &Method::POST, &url, resp).await)
    }

    /// Whether a file with this ID exists (after a create whose response was
    /// an error, e.g. because a retry found the file already created).
    async fn exists(&self, id: &str) -> Result<bool> {
        let url = format!("{}/files/{id}?fields=id,trashed", self.api);
        let resp = self.send(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => {
                let v: serde_json::Value = Self::json(resp, "file").await?;
                Ok(v["trashed"] != true)
            }
            404 => Ok(false),
            _ => Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await),
        }
    }

    async fn reserve_id(&self) -> Result<String> {
        let mut ids = self.reserved_ids.lock().await;
        if ids.is_empty() {
            let url = format!(
                "{}/files/generateIds?count=100&space=drive&type=files",
                self.api
            );
            let resp = self.send(&Method::GET, &url, |r| r).await?;
            if !resp.status().is_success() {
                return Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await);
            }
            let v: serde_json::Value = Self::json(resp, "generated IDs").await?;
            ids.extend(
                v["ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|id| id.as_str().map(str::to_string)),
            );
            if ids.is_empty() {
                return Err(SyncError::Remote(format!("{SERVICE}: no IDs generated")));
            }
        }
        Ok(ids.pop().expect("not empty"))
    }

    /// The file (not folder) at `abs`, looked up now.
    async fn file_at(&self, abs: &str) -> Result<Option<DriveFile>> {
        let (parent, name) = split_parent(abs);
        let Some(parent_id) = self.resolve_dir(parent, false).await? else {
            return Ok(None);
        };
        let file = self
            .children_named(&parent_id, name)
            .await?
            .into_iter()
            .find(|f| !f.is_dir());
        match &file {
            Some(f) => self.cache_file(abs, &f.id),
            None => {
                self.files.lock().unwrap().remove(abs);
            }
        }
        Ok(file)
    }

    /// The ID of the file at `abs`, from the cache if possible.
    async fn file_id(&self, abs: &str) -> Result<Option<String>> {
        if let Some(id) = self.files.lock().unwrap().get(abs) {
            return Ok(Some(id.clone()));
        }
        Ok(self.file_at(abs).await?.map(|f| f.id))
    }

    // ---------------------------------------------------------------------
    // File operations

    /// Downloads a file by ID. `None` if it no longer exists.
    async fn download(&self, id: &str) -> Result<Option<Response>> {
        let url = format!("{}/files/{id}?alt=media", self.api);
        let resp = self.send(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => Ok(Some(resp)),
            404 => Ok(None),
            _ => Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await),
        }
    }

    /// Downloads the file at `abs`, looking it up again if the cached ID is stale.
    async fn download_path(&self, abs: &str) -> Result<Option<Response>> {
        for _ in 0..2 {
            let Some(id) = self.file_id(abs).await? else {
                return Ok(None);
            };
            if let Some(resp) = self.download(&id).await? {
                return Ok(Some(resp));
            }
            self.files.lock().unwrap().remove(abs);
        }
        Ok(None)
    }

    async fn delete_id(&self, id: &str) -> Result<()> {
        let url = format!("{}/files/{id}", self.api);
        let resp = self.send(&Method::DELETE, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 | 204 | 404 => Ok(()),
            _ => Err(http::unexpected(SERVICE, &Method::DELETE, &url, resp).await),
        }
    }

    /// Deletes every file (or folder, with `dirs`) named like the last
    /// segment of `abs`.
    async fn delete_all_named(&self, abs: &str, dirs: bool) -> Result<()> {
        let (parent, name) = split_parent(abs);
        if let Some(parent_id) = self.resolve_dir(parent, false).await? {
            for f in self.children_named(&parent_id, name).await? {
                if f.is_dir() == dirs {
                    if dirs {
                        self.delete_contents(&f.id).await?;
                    }
                    self.delete_id(&f.id).await?;
                }
            }
        }
        self.forget(abs);
        Ok(())
    }

    /// Deletes everything inside a folder. Drive deletes a folder's contents
    /// along with it, but not at once: files in a just-deleted folder can
    /// still be read for a while.
    async fn delete_contents(&self, folder_id: &str) -> Result<()> {
        let mut folders = vec![folder_id.to_string()];
        let mut doomed = Vec::new();
        while let Some(id) = folders.pop() {
            for f in self
                .query(&format!("{} in parents and trashed = false", quote(&id)))
                .await?
            {
                if f.is_dir() {
                    folders.push(f.id.clone());
                }
                doomed.push(f.id);
            }
        }
        // Deepest first, so that nothing is left in a folder already gone.
        for id in doomed.iter().rev() {
            self.delete_id(id).await?;
        }
        Ok(())
    }

    /// Writes small content in one request. `None` if the file to update no
    /// longer exists.
    async fn upload_simple(
        &self,
        target: &UploadTarget<'_>,
        data: &[u8],
    ) -> Result<Option<String>> {
        match target {
            UploadTarget::Update { id } => {
                let url = format!(
                    "{}/files/{id}?uploadType=media&fields={}",
                    self.upload,
                    encode(FIELDS)
                );
                let resp = self
                    .send(&Method::PATCH, &url, |r| {
                        r.header("Content-Type", "application/octet-stream")
                            .body(data.to_vec())
                    })
                    .await?;
                match resp.status().as_u16() {
                    200 => Ok(Some(id.to_string())),
                    404 => Ok(None),
                    _ => Err(http::unexpected(SERVICE, &Method::PATCH, &url, resp).await),
                }
            }
            UploadTarget::Create { parent, name } => {
                let id = self.reserve_id().await?;
                let url = format!(
                    "{}/files?uploadType=multipart&fields={}",
                    self.upload,
                    encode(FIELDS)
                );
                let boundary = format!("nisshoku-{}", nisshoku_core::ids::new_id());
                let metadata = serde_json::json!({ "id": id, "name": name, "parents": [parent] });
                let mut body = format!(
                    "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n"
                )
                .into_bytes();
                body.extend_from_slice(data);
                body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
                let resp = self
                    .send(&Method::POST, &url, |r| {
                        r.header(
                            "Content-Type",
                            format!("multipart/related; boundary={boundary}"),
                        )
                        .body(body.clone())
                    })
                    .await?;
                if resp.status().is_success() || self.exists(&id).await? {
                    return Ok(Some(id));
                }
                if resp.status().as_u16() == 404 {
                    return Ok(None); // the folder is gone
                }
                Err(http::unexpected(SERVICE, &Method::POST, &url, resp).await)
            }
        }
    }

    /// Uploads a file in chunks. `None` if the file to update no longer exists.
    async fn upload_resumable(
        &self,
        target: &UploadTarget<'_>,
        src: &Path,
        size: u64,
    ) -> Result<Option<String>> {
        let (method, url, metadata, id) = match target {
            UploadTarget::Update { id } => (
                Method::PATCH,
                format!(
                    "{}/files/{id}?uploadType=resumable&fields={}",
                    self.upload,
                    encode(FIELDS)
                ),
                serde_json::json!({}),
                id.to_string(),
            ),
            UploadTarget::Create { parent, name } => {
                let id = self.reserve_id().await?;
                (
                    Method::POST,
                    format!(
                        "{}/files?uploadType=resumable&fields={}",
                        self.upload,
                        encode(FIELDS)
                    ),
                    serde_json::json!({ "id": id, "name": name, "parents": [parent] }),
                    id,
                )
            }
        };
        let resp = self
            .send(&method, &url, |r| {
                r.header("X-Upload-Content-Length", size.to_string())
                    .json(&metadata)
            })
            .await?;
        match resp.status().as_u16() {
            200 | 201 => {}
            // The file to update, or the folder to create it in, is gone.
            404 => return Ok(None),
            _ => return Err(http::unexpected(SERVICE, &method, &url, resp).await),
        }
        let Some(session) = resp
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
        else {
            return Err(SyncError::Remote(format!(
                "{SERVICE}: upload session without URL"
            )));
        };

        let mut file = tokio::fs::File::open(src).await?;
        let mut buf = vec![0u8; self.upload_chunk as usize];
        let mut offset = 0u64;
        loop {
            let want = (size - offset).min(self.upload_chunk) as usize;
            file.read_exact(&mut buf[..want]).await?;
            let chunk = &buf[..want];
            let range = if want == 0 {
                format!("bytes */{size}")
            } else {
                format!("bytes {offset}-{}/{size}", offset + want as u64 - 1)
            };
            let resp = self
                .send(&Method::PUT, &session, |r| {
                    r.header("Content-Range", &range).body(chunk.to_vec())
                })
                .await?;
            offset += want as u64;
            match resp.status().as_u16() {
                308 if offset < size => {}
                200 | 201 if offset == size => return Ok(Some(id)),
                _ => {
                    return Err(
                        http::unexpected(SERVICE, &Method::PUT, "upload session", resp).await,
                    );
                }
            }
        }
    }

    async fn upload(
        &self,
        target: &UploadTarget<'_>,
        data: Option<&[u8]>,
        src: Option<&Path>,
    ) -> Result<Option<String>> {
        let size = match (data, src) {
            (Some(d), _) => d.len() as u64,
            (None, Some(p)) => tokio::fs::metadata(p).await?.len(),
            (None, None) => unreachable!(),
        };
        if size <= self.resumable_limit {
            let owned;
            let data = match data {
                Some(d) => d,
                None => {
                    owned = tokio::fs::read(src.expect("source")).await?;
                    &owned
                }
            };
            return self.upload_simple(target, data).await;
        }
        match src {
            Some(p) => self.upload_resumable(target, p, size).await,
            None => {
                let tmp = std::env::temp_dir()
                    .join(format!("nisshoku-upload{}", crate::storage::temp_suffix()));
                tokio::fs::write(&tmp, data.expect("data")).await?;
                let result = self.upload_resumable(target, &tmp, size).await;
                let _ = tokio::fs::remove_file(&tmp).await;
                result
            }
        }
    }

    /// Writes `abs`, updating the existing file or creating one.
    async fn write(&self, abs: &str, data: Option<&[u8]>, src: Option<&Path>) -> Result<()> {
        if let Some(id) = self.file_id(abs).await?
            && self
                .upload(&UploadTarget::Update { id: &id }, data, src)
                .await?
                .is_some()
        {
            return Ok(());
        }
        self.files.lock().unwrap().remove(abs);
        // The cached ID was stale: look again before creating.
        if let Some(f) = self.file_at(abs).await?
            && self
                .upload(&UploadTarget::Update { id: &f.id }, data, src)
                .await?
                .is_some()
        {
            return Ok(());
        }
        self.create(abs, data, src).await
    }

    async fn create(&self, abs: &str, data: Option<&[u8]>, src: Option<&Path>) -> Result<()> {
        let (parent, name) = split_parent(abs);
        for _ in 0..2 {
            let parent_id = self.resolve_dir(parent, true).await?.expect("created");
            let target = UploadTarget::Create {
                parent: &parent_id,
                name,
            };
            if let Some(id) = self.upload(&target, data, src).await? {
                self.cache_file(abs, &id);
                return Ok(());
            }
            // A cached folder was deleted (by another client): look again.
            self.forget_dirs();
        }
        Err(SyncError::Remote(format!(
            "{SERVICE}: could not create {abs}"
        )))
    }

    fn forget_dirs(&self) {
        self.dirs.lock().unwrap().retain(|k, _| k.is_empty());
        self.files.lock().unwrap().clear();
    }

    /// Resolves a folder without trusting the cache, so that a folder deleted
    /// and recreated by another client is not taken for empty.
    async fn resolve_dir_fresh(&self, abs: &str) -> Result<Option<String>> {
        let cached = self.dirs.lock().unwrap().get(abs).cloned();
        let mut current = "root".to_string();
        let mut prefix = String::new();
        for segment in abs.split('/').filter(|s| !s.is_empty()) {
            prefix = join(&prefix, segment);
            let found = self
                .children_named(&current, segment)
                .await?
                .into_iter()
                .find(DriveFile::is_dir);
            let Some(f) = found else {
                self.forget(&prefix);
                return Ok(None);
            };
            current = f.id;
        }
        if cached.as_deref().is_some_and(|c| c != current) {
            self.forget(abs);
            self.forget_dirs();
        }
        self.cache_dir(abs, &current);
        Ok(Some(current))
    }
}

#[async_trait]
impl RemoteStorage for GoogleDriveStorage {
    async fn stat(&self, path: &str) -> Result<Option<RemoteStat>> {
        let abs = self.abs(path);
        let (parent, name) = split_parent(&abs);
        let Some(parent_id) = self.resolve_dir(parent, false).await? else {
            return Ok(None);
        };
        Ok(self
            .children_named(&parent_id, name)
            .await?
            .first()
            .map(|f| stat_of(path.trim_matches('/').to_string(), f)))
    }

    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        match self.download_path(&self.abs(path)).await? {
            Some(resp) => Ok(Some(
                resp.bytes()
                    .await
                    .map_err(|e| SyncError::Remote(format!("{SERVICE}: {e}")))?
                    .to_vec(),
            )),
            None => Ok(None),
        }
    }

    async fn get_versioned(&self, path: &str) -> Result<Option<(Vec<u8>, Option<String>)>> {
        // Metadata first: if the file changes before the download, the
        // version is older than the data and a conditional write fails safely.
        let Some(f) = self.file_at(&self.abs(path)).await? else {
            return Ok(None);
        };
        let Some(resp) = self.download(&f.id).await? else {
            return Ok(None);
        };
        let data = resp
            .bytes()
            .await
            .map_err(|e| SyncError::Remote(format!("{SERVICE}: {e}")))?;
        Ok(Some((data.to_vec(), f.version)))
    }

    async fn put(&self, path: &str, data: &[u8]) -> Result<()> {
        self.write(&self.abs(path), Some(data), None).await
    }

    async fn put_if(&self, path: &str, data: &[u8], precondition: &Precondition) -> Result<bool> {
        let abs = self.abs(path);
        match precondition {
            Precondition::None => self.write(&abs, Some(data), None).await.map(|()| true),
            Precondition::Absent => {
                if self.file_at(&abs).await?.is_some() {
                    return Ok(false);
                }
                self.create(&abs, Some(data), None).await.map(|()| true)
            }
            Precondition::Version(v) => match self.file_at(&abs).await? {
                Some(f) if f.version.as_deref() == Some(v.as_str()) => Ok(self
                    .upload(&UploadTarget::Update { id: &f.id }, Some(data), None)
                    .await?
                    .is_some()),
                _ => Ok(false),
            },
        }
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.delete_all_named(&self.abs(path), false).await
    }

    async fn delete_if(&self, path: &str, precondition: &Precondition) -> Result<bool> {
        let abs = self.abs(path);
        match precondition {
            Precondition::None => {}
            Precondition::Absent => return Ok(self.file_at(&abs).await?.is_none()),
            Precondition::Version(v) => match self.file_at(&abs).await? {
                None => return Ok(true),
                Some(f) if f.version.as_deref() != Some(v.as_str()) => return Ok(false),
                Some(_) => {}
            },
        }
        self.delete_all_named(&abs, false).await?;
        Ok(true)
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        self.resolve_dir(&self.abs(path), true).await?;
        Ok(())
    }

    async fn mkdir_exclusive(&self, path: &str) -> Result<bool> {
        let abs = self.abs(path);
        let (parent, name) = split_parent(&abs);
        let parent_id = self.resolve_dir(parent, true).await?.expect("created");
        if self
            .children_named(&parent_id, name)
            .await?
            .iter()
            .any(DriveFile::is_dir)
        {
            return Ok(false);
        }
        let ours = self.create_folder_raw(&parent_id, name).await?;
        // Others may be creating theirs at the same moment: the oldest wins.
        tokio::time::sleep(self.election_settle).await;
        let first = self
            .children_named(&parent_id, name)
            .await?
            .into_iter()
            .find(DriveFile::is_dir);
        if first.is_some_and(|f| f.id == ours) {
            self.cache_dir(&abs, &ours);
            return Ok(true);
        }
        self.delete_id(&ours).await?;
        Ok(false)
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let abs = self.abs(path);
        self.delete_all_named(&abs, true).await
    }

    async fn list(&self, dir: &str) -> Result<Vec<RemoteStat>> {
        let dir = dir.trim_matches('/');
        let abs_dir = self.abs(dir);
        let Some(dir_id) = self.resolve_dir_fresh(&abs_dir).await? else {
            return Ok(Vec::new());
        };
        let mut files = self
            .query(&format!(
                "{} in parents and trashed = false",
                quote(&dir_id)
            ))
            .await?;
        files.sort_by_key(DriveFile::precedence);
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        let mut cache = self.files.lock().unwrap();
        for f in files {
            if !seen.insert(f.name.clone()) {
                continue; // a newer duplicate
            }
            let path = join(dir, &f.name);
            if !f.is_dir() && Self::cacheable(&abs_dir, &self.folder) {
                cache.insert(join(&abs_dir, &f.name), f.id.clone());
            }
            out.push(stat_of(path, &f));
        }
        Ok(out)
    }

    async fn get_to_file(&self, path: &str, dest: &Path) -> Result<bool> {
        match self.download_path(&self.abs(path)).await? {
            Some(resp) => {
                http::write_stream(resp, dest).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    async fn put_file(&self, path: &str, src: &Path) -> Result<()> {
        self.write(&self.abs(path), None, Some(src)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_query_literals() {
        assert_eq!(quote("it's"), r"'it\'s'");
        assert_eq!(quote(r"a\b"), r"'a\\b'");
    }

    #[test]
    fn mutex_folders_are_not_cached() {
        assert!(GoogleDriveStorage::cacheable(
            "Nisshoku/.resource",
            "Nisshoku"
        ));
        assert!(GoogleDriveStorage::cacheable(
            "Nisshoku/.nisshoku",
            "Nisshoku"
        ));
        assert!(!GoogleDriveStorage::cacheable(
            "Nisshoku/.nisshoku/sync",
            "Nisshoku"
        ));
    }
}
