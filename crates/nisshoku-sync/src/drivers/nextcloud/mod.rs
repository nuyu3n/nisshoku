//! Nextcloud sync target, over Nextcloud's WebDAV API.
//!
//! Items live in a folder of the user's Nextcloud files, laid out like any
//! other target (and like Joplin's Nextcloud target), so the folder can be
//! shared with Joplin clients.
//!
//! Beyond plain file access this driver uses:
//! - conditional requests (`If-Match` / `If-None-Match`) so that concurrent
//!   clients never silently overwrite each other's changes,
//! - chunked uploads for large attachments,
//! - retries with backoff for transient errors (`423 Locked`, `429`, `5xx`,
//!   connection failures),
//! - explicit modification times (`X-OC-MTime`) on uploads: Nextcloud derives
//!   ETags from the modification time in whole seconds, so without this two
//!   writes within a second would be indistinguishable to other clients and
//!   to conditional requests. Each write gets a time at least one second after
//!   the previous version, in the server's clock.
//! - Login Flow v2 (see [`login`]) to obtain an app password.

mod login;
mod propfind;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures_util::StreamExt;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use reqwest::{Method, RequestBuilder, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub use login::{LoginCredentials, LoginFlow, fetch_user_id, poll_login_flow, start_login_flow};

use crate::error::{Result, SyncError};
use crate::storage::{Precondition, RemoteStat, RemoteStorage};
use propfind::{PROPFIND_BODY, parse_multistatus};

/// Characters left as is in a path segment.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

const DEFAULT_CHUNK_THRESHOLD: u64 = 20 * 1024 * 1024;
const DEFAULT_CHUNK_SIZE: u64 = 10 * 1024 * 1024;

fn encode_segment(s: &str) -> String {
    utf8_percent_encode(s, SEGMENT).to_string()
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn method(name: &str) -> Method {
    Method::from_bytes(name.as_bytes()).expect("valid method")
}

/// Normalizes a server URL as typed by a user: adds `https://` if missing and
/// strips trailing slashes and any pasted WebDAV or login path.
pub(crate) fn base_url(server_url: &str) -> Result<String> {
    let trimmed = server_url.trim();
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let url = Url::parse(&with_scheme)
        .map_err(|e| SyncError::Remote(format!("invalid server URL {server_url:?}: {e}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(SyncError::Remote(format!(
            "invalid server URL {server_url:?}"
        )));
    }
    let mut base = with_scheme;
    for marker in ["/remote.php", "/index.php", "/ocs/"] {
        if let Some(i) = base.find(marker) {
            base.truncate(i);
        }
    }
    Ok(base.trim_end_matches('/').to_string())
}

pub(crate) fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("Nisshoku/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| SyncError::Remote(e.to_string()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NextcloudConfig {
    /// e.g. `https://cloud.example.com` (a sub-path installation also works).
    pub server_url: String,
    /// Login name, as returned by the login flow.
    pub username: String,
    /// An app password (preferred) or the account password.
    pub password: String,
    /// Folder in the user's files that holds the items, e.g. `Nisshoku`.
    pub folder: String,
}

pub struct NextcloudStorage {
    client: reqwest::Client,
    base: String,
    username: String,
    password: String,
    user_id: String,
    folder: String,
    /// URL of the sync folder, ending with `/`.
    root_url: String,
    /// Decoded path of `root_url`, for turning hrefs into relative paths.
    root_path: String,
    chunk_threshold: u64,
    chunk_size: u64,
    max_attempts: u32,
    retry_delay: Duration,
    /// Server clock minus local clock, in seconds, from `Date` headers.
    clock_offset: AtomicI64,
    /// Last known modification time (seconds) of files, by relative path.
    mtimes: Mutex<HashMap<String, i64>>,
}

fn unix_secs(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn header_time(resp: &Response, name: &str) -> Option<i64> {
    let value = resp.headers().get(name)?.to_str().ok()?;
    httpdate::parse_http_date(value).ok().map(unix_secs)
}

impl NextcloudStorage {
    /// Checks the credentials, resolves the user ID and creates the folder.
    pub async fn connect(config: &NextcloudConfig) -> Result<Self> {
        let user_id = fetch_user_id(&config.server_url, &config.username, &config.password).await?;
        let storage = Self::with_user_id(config, &user_id)?;
        storage.ensure_root().await?;
        Ok(storage)
    }

    /// Creates the storage without contacting the server.
    pub fn with_user_id(config: &NextcloudConfig, user_id: &str) -> Result<Self> {
        let base = base_url(&config.server_url)?;
        let folder = encode_path(&config.folder);
        let mut root_url = format!("{base}/remote.php/dav/files/{}/", encode_segment(user_id));
        if !folder.is_empty() {
            root_url.push_str(&folder);
            root_url.push('/');
        }
        let parsed = Url::parse(&root_url)
            .map_err(|e| SyncError::Remote(format!("invalid URL {root_url}: {e}")))?;
        let root_path = percent_decode_str(parsed.path())
            .decode_utf8_lossy()
            .into_owned();
        Ok(Self {
            client: http_client()?,
            base,
            username: config.username.clone(),
            password: config.password.clone(),
            user_id: user_id.to_string(),
            folder: config.folder.clone(),
            root_url,
            root_path,
            chunk_threshold: DEFAULT_CHUNK_THRESHOLD,
            chunk_size: DEFAULT_CHUNK_SIZE,
            max_attempts: 4,
            retry_delay: Duration::from_millis(500),
            clock_offset: AtomicI64::new(0),
            mtimes: Mutex::new(HashMap::new()),
        })
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Files larger than `threshold` bytes are uploaded in chunks of `chunk_size`.
    pub fn set_chunking(&mut self, threshold: u64, chunk_size: u64) {
        self.chunk_threshold = threshold;
        self.chunk_size = chunk_size.max(1);
    }

    /// How often (and how quickly) transient failures are retried.
    pub fn set_retry(&mut self, max_attempts: u32, initial_delay: Duration) {
        self.max_attempts = max_attempts.max(1);
        self.retry_delay = initial_delay;
    }

    fn server_now_secs(&self) -> i64 {
        unix_secs(SystemTime::now()) + self.clock_offset.load(Ordering::Relaxed)
    }

    fn remember_mtime(&self, path: &str, secs: i64) {
        self.mtimes
            .lock()
            .unwrap()
            .insert(path.trim_matches('/').to_string(), secs);
    }

    /// Modification time for a new version of `path`: now in server time, but
    /// always later than the version it replaces.
    fn next_mtime(&self, path: &str, creating: bool) -> i64 {
        let server_now = self.server_now_secs();
        let mut mtimes = self.mtimes.lock().unwrap();
        let key = path.trim_matches('/').to_string();
        // A file created where one was just deleted must not reuse its time.
        let floor = if creating { server_now + 1 } else { server_now };
        let t = match mtimes.get(&key) {
            Some(prev) => floor.max(prev + 1),
            None => floor,
        };
        mtimes.insert(key, t);
        t
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.root_url, encode_path(path))
    }

    fn dir_url(&self, path: &str) -> String {
        let url = self.url(path);
        if url.ends_with('/') { url } else { url + "/" }
    }

    fn relative_path(&self, href: &str) -> Option<String> {
        let path = if href.starts_with("http://") || href.starts_with("https://") {
            Url::parse(href).ok()?.path().to_string()
        } else {
            href.to_string()
        };
        let decoded = percent_decode_str(&path).decode_utf8().ok()?;
        let rel = decoded.strip_prefix(self.root_path.as_str())?;
        Some(rel.trim_end_matches('/').to_string())
    }

    /// Sends a request, retrying transient failures. `build` is called for
    /// every attempt.
    async fn send(
        &self,
        method: &Method,
        url: &str,
        build: impl Fn(RequestBuilder) -> RequestBuilder,
    ) -> Result<Response> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let request = build(
                self.client
                    .request(method.clone(), url)
                    .basic_auth(&self.username, Some(&self.password)),
            );
            let failure = match request.send().await {
                Ok(resp) => {
                    if let Some(server) = header_time(&resp, "Date") {
                        self.clock_offset
                            .store(server - unix_secs(SystemTime::now()), Ordering::Relaxed);
                    }
                    let status = resp.status().as_u16();
                    if status == 401 {
                        return Err(SyncError::Auth(
                            "Nextcloud rejected the credentials (the app password may have been revoked)"
                                .into(),
                        ));
                    }
                    let transient = matches!(status, 423 | 429 | 500 | 502 | 503 | 504);
                    if !transient || attempt >= self.max_attempts {
                        return Ok(resp);
                    }
                    format!("HTTP {status}")
                }
                Err(e) => {
                    let transient = e.is_connect() || e.is_timeout() || e.is_request();
                    if !transient || attempt >= self.max_attempts {
                        return Err(SyncError::Remote(format!("{method} {url}: {e}")));
                    }
                    e.to_string()
                }
            };
            let delay = self.retry_delay * 2u32.pow(attempt - 1);
            tracing::debug!("{method} {url} failed ({failure}), retrying in {delay:?}");
            tokio::time::sleep(delay).await;
        }
    }

    fn unexpected(method: &Method, url: &str, status: StatusCode) -> SyncError {
        SyncError::Remote(format!(
            "Nextcloud: {method} {url} failed with HTTP {status}"
        ))
    }

    /// `None` if the path does not exist.
    async fn propfind(&self, url: &str, depth: u8) -> Result<Option<Vec<propfind::Entry>>> {
        let m = method("PROPFIND");
        let resp = self
            .send(&m, url, |r| {
                r.header("Depth", depth.to_string())
                    .header("Content-Type", "application/xml; charset=utf-8")
                    .body(PROPFIND_BODY)
            })
            .await?;
        match resp.status().as_u16() {
            207 => {
                let text = resp
                    .text()
                    .await
                    .map_err(|e| SyncError::Remote(e.to_string()))?;
                parse_multistatus(&text).map(Some).map_err(|e| {
                    SyncError::Remote(format!("invalid PROPFIND response from {url}: {e}"))
                })
            }
            404 => Ok(None),
            _ => Err(Self::unexpected(&m, url, resp.status())),
        }
    }

    fn to_stat(&self, entry: propfind::Entry) -> Option<RemoteStat> {
        let path = self.relative_path(&entry.href)?;
        if let (Some(t), false) = (entry.last_modified, entry.is_dir) {
            self.remember_mtime(&path, t / 1000);
        }
        Some(RemoteStat {
            path,
            updated_time: entry.last_modified.unwrap_or(0),
            is_dir: entry.is_dir,
            etag: entry.etag,
        })
    }

    /// Creates a collection. Returns `false` if its parent does not exist.
    async fn mkcol(&self, url: &str) -> Result<bool> {
        let m = method("MKCOL");
        let resp = self.send(&m, url, |r| r).await?;
        match resp.status().as_u16() {
            200 | 201 | 405 => Ok(true), // 405: already exists
            409 => Ok(false),
            _ => Err(Self::unexpected(&m, url, resp.status())),
        }
    }

    /// Creates the sync folder and its parents.
    async fn ensure_root(&self) -> Result<()> {
        let mut url = format!(
            "{}/remote.php/dav/files/{}",
            self.base,
            encode_segment(&self.user_id)
        );
        for segment in self.folder.split('/').filter(|s| !s.is_empty()) {
            url = format!("{url}/{}", encode_segment(segment));
            if !self.mkcol(&format!("{url}/")).await? {
                return Err(SyncError::Remote(format!("could not create folder {url}")));
            }
        }
        Ok(())
    }

    async fn mkdir_all(&self, path: &str) -> Result<()> {
        if self.mkcol(&self.dir_url(path)).await? {
            return Ok(());
        }
        // A parent is missing: create the hierarchy from the top.
        self.ensure_root().await?;
        let mut prefix = String::new();
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            if !self.mkcol(&self.dir_url(&prefix)).await? {
                return Err(SyncError::Remote(format!(
                    "could not create folder {prefix}"
                )));
            }
        }
        Ok(())
    }

    async fn ensure_parent(&self, path: &str) -> Result<()> {
        match path.rsplit_once('/') {
            Some((parent, _)) => self.mkdir_all(parent).await,
            None => self.ensure_root().await,
        }
    }

    /// Uploads through Nextcloud's chunked upload API. Returns `false` if the
    /// server does not support it.
    async fn chunked_upload(&self, path: &str, src: &Path, size: u64) -> Result<bool> {
        let upload_url = format!(
            "{}/remote.php/dav/uploads/{}/nisshoku-{}",
            self.base,
            encode_segment(&self.user_id),
            uuid::Uuid::new_v4().simple()
        );
        let dest = self.url(path);
        let mkcol = method("MKCOL");
        let resp = self
            .send(&mkcol, &upload_url, |r| r.header("Destination", &dest))
            .await?;
        if resp.status() != StatusCode::CREATED {
            tracing::debug!("chunked upload not available (HTTP {})", resp.status());
            return Ok(false);
        }

        let result = self
            .upload_chunks(&upload_url, &dest, path, src, size)
            .await;
        if result.is_err() {
            let _ = self.send(&Method::DELETE, &upload_url, |r| r).await;
        }
        result.map(|()| true)
    }

    async fn upload_chunks(
        &self,
        upload_url: &str,
        dest: &str,
        dest_path: &str,
        src: &Path,
        size: u64,
    ) -> Result<()> {
        let mut file = tokio::fs::File::open(src).await?;
        let mut buf = vec![0u8; self.chunk_size as usize];
        let total = size.to_string();
        for index in 1.. {
            let mut filled = 0;
            while filled < buf.len() {
                let n = file.read(&mut buf[filled..]).await?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
            if filled == 0 {
                break;
            }
            let chunk_url = format!("{upload_url}/{index:05}");
            let chunk = &buf[..filled];
            let resp = self
                .send(&Method::PUT, &chunk_url, |r| {
                    r.header("Destination", dest)
                        .header("OC-Total-Length", &total)
                        .body(chunk.to_vec())
                })
                .await?;
            if !resp.status().is_success() {
                return Err(Self::unexpected(&Method::PUT, &chunk_url, resp.status()));
            }
            if filled < buf.len() {
                break;
            }
        }
        let mv = method("MOVE");
        let file_url = format!("{upload_url}/.file");
        let mtime = self.next_mtime(dest_path, false).to_string();
        let resp = self
            .send(&mv, &file_url, |r| {
                r.header("Destination", dest)
                    .header("OC-Total-Length", &total)
                    .header("Overwrite", "T")
                    .header("X-OC-MTime", &mtime)
            })
            .await?;
        if !resp.status().is_success() {
            return Err(Self::unexpected(&mv, &file_url, resp.status()));
        }
        Ok(())
    }
}

fn with_precondition(r: RequestBuilder, precondition: &Precondition) -> RequestBuilder {
    match precondition {
        Precondition::None => r,
        Precondition::Absent => r.header("If-None-Match", "*"),
        Precondition::Version(v) => r.header("If-Match", v),
    }
}

#[async_trait]
impl RemoteStorage for NextcloudStorage {
    fn unstable_since(&self) -> Option<i64> {
        // Times have one-second resolution, and uploads may be stamped up to a
        // few seconds ahead (see `next_mtime`).
        Some((self.server_now_secs() - 1) * 1000)
    }

    async fn stat(&self, path: &str) -> Result<Option<RemoteStat>> {
        let entries = self.propfind(&self.url(path), 0).await?;
        Ok(entries
            .and_then(|e| e.into_iter().next())
            .and_then(|e| self.to_stat(e)))
    }

    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.get_versioned(path).await?.map(|(data, _)| data))
    }

    async fn get_versioned(&self, path: &str) -> Result<Option<(Vec<u8>, Option<String>)>> {
        let url = self.url(path);
        let resp = self.send(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => {
                if let Some(t) = header_time(&resp, "Last-Modified") {
                    self.remember_mtime(path, t);
                }
                let etag = ["ETag", "OC-ETag"]
                    .iter()
                    .find_map(|h| resp.headers().get(*h))
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                let data = resp
                    .bytes()
                    .await
                    .map_err(|e| SyncError::Remote(e.to_string()))?;
                Ok(Some((data.to_vec(), etag)))
            }
            404 => Ok(None),
            _ => Err(Self::unexpected(&Method::GET, &url, resp.status())),
        }
    }

    async fn put(&self, path: &str, data: &[u8]) -> Result<()> {
        self.put_if(path, data, &Precondition::None).await?;
        Ok(())
    }

    async fn put_if(&self, path: &str, data: &[u8], precondition: &Precondition) -> Result<bool> {
        let url = self.url(path);
        let mut created_parent = false;
        let mtime = self
            .next_mtime(path, *precondition == Precondition::Absent)
            .to_string();
        loop {
            let resp = self
                .send(&Method::PUT, &url, |r| {
                    with_precondition(r.body(data.to_vec()), precondition)
                        .header("X-OC-MTime", &mtime)
                })
                .await?;
            match resp.status().as_u16() {
                200 | 201 | 204 => return Ok(true),
                412 => return Ok(false),
                // The parent folder is missing.
                409 | 404 if !created_parent => {
                    self.ensure_parent(path).await?;
                    created_parent = true;
                }
                _ => return Err(Self::unexpected(&Method::PUT, &url, resp.status())),
            }
        }
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.delete_if(path, &Precondition::None).await?;
        Ok(())
    }

    async fn delete_if(&self, path: &str, precondition: &Precondition) -> Result<bool> {
        let url = self.url(path);
        let resp = self
            .send(&Method::DELETE, &url, |r| {
                with_precondition(r, precondition)
            })
            .await?;
        match resp.status().as_u16() {
            200 | 204 | 404 => Ok(true),
            412 => Ok(false),
            _ => Err(Self::unexpected(&Method::DELETE, &url, resp.status())),
        }
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        if path.trim_matches('/').is_empty() {
            self.ensure_root().await
        } else {
            self.mkdir_all(path).await
        }
    }

    async fn mkdir_exclusive(&self, path: &str) -> Result<bool> {
        let m = method("MKCOL");
        let url = self.dir_url(path);
        let mut created_parent = false;
        loop {
            let resp = self.send(&m, &url, |r| r).await?;
            match resp.status().as_u16() {
                201 => return Ok(true),
                405 => return Ok(false), // already exists
                // Nextcloud answers a concurrent creation with 403 (or 423).
                403 | 423 if self.stat(path).await?.is_some() => return Ok(false),
                409 if !created_parent => {
                    self.ensure_parent(path.trim_end_matches('/')).await?;
                    created_parent = true;
                }
                _ => return Err(Self::unexpected(&m, &url, resp.status())),
            }
        }
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let url = self.dir_url(path);
        let resp = self.send(&Method::DELETE, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 | 204 | 404 => Ok(()),
            _ => Err(Self::unexpected(&Method::DELETE, &url, resp.status())),
        }
    }

    async fn list(&self, dir: &str) -> Result<Vec<RemoteStat>> {
        let dir = dir.trim_matches('/');
        let Some(entries) = self.propfind(&self.dir_url(dir), 1).await? else {
            return Ok(Vec::new());
        };
        Ok(entries
            .into_iter()
            .filter_map(|e| self.to_stat(e))
            .filter(|s| !s.path.is_empty() && s.path != dir)
            .collect())
    }

    async fn get_to_file(&self, path: &str, dest: &Path) -> Result<bool> {
        let url = self.url(path);
        let resp = self.send(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => {}
            404 => return Ok(false),
            _ => return Err(Self::unexpected(&Method::GET, &url, resp.status())),
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut tmp = dest.as_os_str().to_owned();
        tmp.push(crate::storage::temp_suffix());
        let mut file = tokio::fs::File::create(&tmp).await?;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| SyncError::Remote(format!("downloading {url}: {e}")))?;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&tmp, dest).await?;
        Ok(true)
    }

    async fn put_file(&self, path: &str, src: &Path) -> Result<()> {
        let size = tokio::fs::metadata(src).await?.len();
        if size > self.chunk_threshold {
            self.ensure_parent(path).await?;
            if self.chunked_upload(path, src, size).await? {
                return Ok(());
            }
        }
        let data = tokio::fs::read(src).await?;
        self.put(path, &data).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage(server: &str, user_id: &str, folder: &str) -> NextcloudStorage {
        let config = NextcloudConfig {
            server_url: server.into(),
            username: "u".into(),
            password: "p".into(),
            folder: folder.into(),
        };
        NextcloudStorage::with_user_id(&config, user_id).unwrap()
    }

    #[test]
    fn normalizes_server_urls() {
        assert_eq!(
            base_url("cloud.example.com/").unwrap(),
            "https://cloud.example.com"
        );
        assert_eq!(
            base_url("https://example.com/nextcloud/remote.php/dav/files/alice/").unwrap(),
            "https://example.com/nextcloud"
        );
        assert_eq!(
            base_url("http://localhost:8080/index.php/apps/files").unwrap(),
            "http://localhost:8080"
        );
        assert!(base_url("ftp://example.com").is_err());
    }

    #[test]
    fn builds_urls_and_relative_paths() {
        let s = storage(
            "https://example.com/nc",
            "alice@example.com",
            "書類/My Notes",
        );
        assert_eq!(
            s.url("locks/1_desktop_x.json"),
            "https://example.com/nc/remote.php/dav/files/alice%40example.com/%E6%9B%B8%E9%A1%9E/My%20Notes/locks/1_desktop_x.json"
        );
        assert_eq!(
            s.relative_path("/nc/remote.php/dav/files/alice%40example.com/%E6%9B%B8%E9%A1%9E/My%20Notes/.resource/"),
            Some(".resource".into())
        );
        assert_eq!(
            s.relative_path(
                "https://example.com/nc/remote.php/dav/files/alice@example.com/書類/My%20Notes/a.md"
            ),
            Some("a.md".into())
        );
        assert_eq!(
            s.relative_path("/nc/remote.php/dav/files/alice%40example.com/other/a.md"),
            None
        );
    }
}
