//! OneDrive sync target, over the Microsoft Graph API.
//!
//! Items live in a folder of the user's drive, addressed by path, laid out
//! like any other target. Joplin's OneDrive target keeps its items in its app
//! folder (`Apps/Joplin`); pointing [`OneDriveConfig::folder`] there shares
//! the items with Joplin clients.
//!
//! - Conditional writes use the items' `eTag` (`If-Match`) and
//!   `@microsoft.graph.conflictBehavior=fail` for create-only writes and
//!   folders (the latter also backs [`RemoteStorage::mkdir_exclusive`]).
//! - Files over [`DEFAULT_SIMPLE_UPLOAD_LIMIT`] go through upload sessions.
//! - Throttling (`429`, `503` with `Retry-After`) and server errors are
//!   retried; expired access tokens are refreshed.

use std::path::Path;

use async_trait::async_trait;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{Method, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::drivers::http::{self, RetryPolicy, parse_time};
use crate::drivers::oauth::{OAuthProvider, OAuthTokens, TokenListener, TokenSource};
use crate::error::{Result, SyncError};
use crate::storage::{Precondition, RemoteStat, RemoteStorage};

const SERVICE: &str = "OneDrive";

/// Characters left as is in a path segment.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Larger files are uploaded through an upload session.
pub const DEFAULT_SIMPLE_UPLOAD_LIMIT: u64 = 4 * 1024 * 1024;
/// Upload session chunks must be multiples of 320 KiB.
pub const UPLOAD_CHUNK_UNIT: u64 = 320 * 1024;
const DEFAULT_UPLOAD_CHUNK: u64 = 32 * UPLOAD_CHUNK_UNIT;

pub const ONEDRIVE_SCOPES: &[&str] = &["Files.ReadWrite", "offline_access"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OneDriveEndpoints {
    /// The drive, e.g. `https://graph.microsoft.com/v1.0/me/drive`.
    pub api: String,
    pub auth: String,
    pub token: String,
}

impl Default for OneDriveEndpoints {
    fn default() -> Self {
        Self {
            api: "https://graph.microsoft.com/v1.0/me/drive".into(),
            auth: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".into(),
            token: "https://login.microsoftonline.com/common/oauth2/v2.0/token".into(),
        }
    }
}

/// The OAuth provider for an app registered with Microsoft (a public client
/// with `http://localhost` as redirect URI).
pub fn onedrive_oauth(client_id: &str, endpoints: &OneDriveEndpoints) -> OAuthProvider {
    OAuthProvider {
        auth_url: endpoints.auth.clone(),
        token_url: endpoints.token.clone(),
        client_id: client_id.into(),
        client_secret: None,
        scopes: ONEDRIVE_SCOPES.iter().map(|s| s.to_string()).collect(),
        redirect_host: "localhost".into(),
        extra_auth_params: vec![("prompt".into(), "select_account".into())],
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OneDriveConfig {
    pub client_id: String,
    /// Folder of the drive that holds the items, e.g. `Nisshoku`.
    pub folder: String,
    pub tokens: OAuthTokens,
    #[serde(default)]
    pub endpoints: OneDriveEndpoints,
}

pub struct OneDriveStorage {
    client: reqwest::Client,
    tokens: TokenSource,
    retry: RetryPolicy,
    api: String,
    /// The sync folder's path in the drive, without leading or trailing `/`.
    folder: String,
    simple_upload_limit: u64,
    upload_chunk: u64,
}

#[derive(Deserialize)]
struct DriveItem {
    name: String,
    #[serde(rename = "eTag")]
    etag: Option<String>,
    #[serde(rename = "lastModifiedDateTime")]
    modified: Option<String>,
    folder: Option<serde_json::Value>,
    #[serde(rename = "@microsoft.graph.downloadUrl")]
    download_url: Option<String>,
}

#[derive(Deserialize)]
struct Children {
    value: Vec<DriveItem>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| utf8_percent_encode(s, SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
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

impl OneDriveStorage {
    pub fn new(config: &OneDriveConfig, listener: Option<TokenListener>) -> Result<Self> {
        let provider = onedrive_oauth(&config.client_id, &config.endpoints);
        Ok(Self {
            client: http::client()?,
            tokens: TokenSource::new(provider, config.tokens.clone(), listener)?,
            retry: RetryPolicy::default(),
            api: config.endpoints.api.trim_end_matches('/').to_string(),
            folder: config.folder.trim_matches('/').to_string(),
            simple_upload_limit: DEFAULT_SIMPLE_UPLOAD_LIMIT,
            upload_chunk: DEFAULT_UPLOAD_CHUNK,
        })
    }

    /// Creates the storage and its folder (checking the sign-in on the way).
    pub async fn connect(config: &OneDriveConfig, listener: Option<TokenListener>) -> Result<Self> {
        let storage = Self::new(config, listener)?;
        storage.mkdir("").await?;
        Ok(storage)
    }

    pub fn set_retry(&mut self, retry: RetryPolicy) {
        self.retry = retry;
    }

    /// Files over `simple_limit` bytes are uploaded in chunks of `chunk`
    /// (rounded down to a multiple of [`UPLOAD_CHUNK_UNIT`]).
    pub fn set_upload_limits(&mut self, simple_limit: u64, chunk: u64) {
        self.simple_upload_limit = simple_limit;
        self.upload_chunk = (chunk / UPLOAD_CHUNK_UNIT).max(1) * UPLOAD_CHUNK_UNIT;
    }

    /// The current tokens (refresh tokens rotate).
    pub async fn tokens(&self) -> OAuthTokens {
        self.tokens.tokens().await
    }

    fn abs(&self, path: &str) -> String {
        join(&self.folder, path)
    }

    /// URL of the item at an absolute drive path.
    fn item_url(&self, abs: &str) -> String {
        if abs.is_empty() {
            format!("{}/root", self.api)
        } else {
            format!("{}/root:/{}:", self.api, encode_path(abs))
        }
    }

    fn children_url(&self, abs: &str) -> String {
        if abs.is_empty() {
            format!("{}/root/children", self.api)
        } else {
            format!("{}/root:/{}:/children", self.api, encode_path(abs))
        }
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

    /// For pre-authenticated URLs (downloads, upload sessions).
    async fn send_anonymous(
        &self,
        method: &Method,
        url: &str,
        build: impl Fn(RequestBuilder) -> RequestBuilder,
    ) -> Result<Response> {
        http::send(&self.client, None, &self.retry, SERVICE, method, url, build).await
    }

    fn stat_of(path: String, item: &DriveItem) -> RemoteStat {
        RemoteStat {
            path,
            updated_time: item.modified.as_deref().and_then(parse_time).unwrap_or(0),
            is_dir: item.folder.is_some(),
            etag: item.etag.clone(),
        }
    }

    async fn item(&self, abs: &str) -> Result<Option<DriveItem>> {
        let url = self.item_url(abs);
        let resp = self.send(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => Ok(Some(resp.json().await.map_err(|e| {
                SyncError::Remote(format!("{SERVICE}: invalid item from {url}: {e}"))
            })?)),
            404 => Ok(None),
            _ => Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await),
        }
    }

    /// Creates a folder. `Ok(None)` if the parent does not exist, otherwise
    /// whether it was created (`false`: it already existed).
    async fn create_folder(&self, abs: &str) -> Result<Option<bool>> {
        let (parent, name) = split_parent(abs);
        let url = self.children_url(parent);
        let body = serde_json::json!({
            "name": name,
            "folder": {},
            "@microsoft.graph.conflictBehavior": "fail",
        });
        let resp = self.send(&Method::POST, &url, |r| r.json(&body)).await?;
        match resp.status().as_u16() {
            200 | 201 => Ok(Some(true)),
            409 => Ok(Some(false)),
            404 => Ok(None),
            _ => Err(http::unexpected(SERVICE, &Method::POST, &url, resp).await),
        }
    }

    /// Creates a folder and any missing parents.
    async fn mkdir_abs(&self, abs: &str) -> Result<()> {
        if abs.is_empty() || self.create_folder(abs).await?.is_some() {
            return Ok(());
        }
        let mut prefix = String::new();
        for segment in abs.split('/') {
            prefix = join(&prefix, segment);
            if self.create_folder(&prefix).await?.is_none() {
                return Err(SyncError::Remote(format!(
                    "{SERVICE}: could not create folder {prefix}"
                )));
            }
        }
        Ok(())
    }

    async fn mkdir_parent(&self, abs: &str) -> Result<()> {
        self.mkdir_abs(split_parent(abs).0).await
    }

    async fn upload_session(&self, path: &str, src: &Path, size: u64) -> Result<()> {
        let abs = self.abs(path);
        let url = format!("{}/createUploadSession", self.item_url(&abs));
        let body =
            serde_json::json!({ "item": { "@microsoft.graph.conflictBehavior": "replace" } });
        let mut created_parent = false;
        let upload_url = loop {
            let resp = self.send(&Method::POST, &url, |r| r.json(&body)).await?;
            match resp.status().as_u16() {
                200 | 201 => {
                    let session: serde_json::Value = resp
                        .json()
                        .await
                        .map_err(|e| SyncError::Remote(format!("{SERVICE}: {e}")))?;
                    match session["uploadUrl"].as_str() {
                        Some(u) => break u.to_string(),
                        None => {
                            return Err(SyncError::Remote(format!(
                                "{SERVICE}: upload session without URL"
                            )));
                        }
                    }
                }
                404 | 409 if !created_parent => {
                    self.mkdir_parent(&abs).await?;
                    created_parent = true;
                }
                _ => return Err(http::unexpected(SERVICE, &Method::POST, &url, resp).await),
            }
        };
        let result = self.upload_chunks(&upload_url, src, size).await;
        if result.is_err() {
            let _ = self
                .send_anonymous(&Method::DELETE, &upload_url, |r| r)
                .await;
        }
        result
    }

    async fn upload_chunks(&self, upload_url: &str, src: &Path, size: u64) -> Result<()> {
        let mut file = tokio::fs::File::open(src).await?;
        let mut buf = vec![0u8; self.upload_chunk as usize];
        let mut offset = 0u64;
        while offset < size {
            let want = (size - offset).min(self.upload_chunk) as usize;
            file.read_exact(&mut buf[..want]).await?;
            let chunk = &buf[..want];
            let range = format!("bytes {offset}-{}/{size}", offset + want as u64 - 1);
            let resp = self
                .send_anonymous(&Method::PUT, upload_url, |r| {
                    r.header("Content-Range", &range).body(chunk.to_vec())
                })
                .await?;
            let end = offset + want as u64;
            match resp.status().as_u16() {
                202 => {}
                200 | 201 if end == size => return Ok(()),
                // A resent chunk (its first response was lost) the service
                // already has: carry on if it expects what follows.
                416 if end < size && self.next_expected(upload_url).await? == Some(end) => {}
                _ => return Err(http::unexpected(SERVICE, &Method::PUT, upload_url, resp).await),
            }
            offset = end;
        }
        Err(SyncError::Remote(format!(
            "{SERVICE}: upload session did not complete"
        )))
    }

    /// The first byte the upload session is waiting for.
    async fn next_expected(&self, upload_url: &str) -> Result<Option<u64>> {
        let resp = self.send_anonymous(&Method::GET, upload_url, |r| r).await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let status: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyncError::Remote(format!("{SERVICE}: {e}")))?;
        Ok(status["nextExpectedRanges"][0]
            .as_str()
            .and_then(|r| r.split('-').next())
            .and_then(|start| start.parse().ok()))
    }
}

#[async_trait]
impl RemoteStorage for OneDriveStorage {
    async fn stat(&self, path: &str) -> Result<Option<RemoteStat>> {
        Ok(self
            .item(&self.abs(path))
            .await?
            .map(|item| Self::stat_of(path.trim_matches('/').to_string(), &item)))
    }

    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let url = format!("{}/content", self.item_url(&self.abs(path)));
        let resp = self.send(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => Ok(Some(
                resp.bytes()
                    .await
                    .map_err(|e| SyncError::Remote(format!("{SERVICE}: {e}")))?
                    .to_vec(),
            )),
            404 => Ok(None),
            _ => Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await),
        }
    }

    async fn get_versioned(&self, path: &str) -> Result<Option<(Vec<u8>, Option<String>)>> {
        // Metadata first: if the file changes before the download, the
        // version is older than the data and a conditional write fails safely.
        let Some(item) = self.item(&self.abs(path)).await? else {
            return Ok(None);
        };
        let Some(url) = item.download_url else {
            return Err(SyncError::Remote(format!(
                "{SERVICE}: {path} is not a file"
            )));
        };
        let resp = self.send_anonymous(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => {
                let data = resp
                    .bytes()
                    .await
                    .map_err(|e| SyncError::Remote(format!("{SERVICE}: {e}")))?;
                Ok(Some((data.to_vec(), item.etag)))
            }
            404 => Ok(None),
            _ => Err(http::unexpected(SERVICE, &Method::GET, "download URL", resp).await),
        }
    }

    async fn put(&self, path: &str, data: &[u8]) -> Result<()> {
        self.put_if(path, data, &Precondition::None).await?;
        Ok(())
    }

    async fn put_if(&self, path: &str, data: &[u8], precondition: &Precondition) -> Result<bool> {
        if data.len() as u64 > self.simple_upload_limit {
            // Upload sessions take no precondition: check first.
            if !self.check(path, precondition).await? {
                return Ok(false);
            }
            let tmp = std::env::temp_dir()
                .join(format!("nisshoku-upload{}", crate::storage::temp_suffix()));
            tokio::fs::write(&tmp, data).await?;
            let result = self.upload_session(path, &tmp, data.len() as u64).await;
            let _ = tokio::fs::remove_file(&tmp).await;
            return result.map(|()| true);
        }
        // The current documentation of simple uploads no longer mentions
        // `If-Match` or `conflictBehavior`: check first as well, so that the
        // precondition holds even if the service ignores them (the sync mutex
        // keeps this app's clients from racing between check and write).
        if !self.check(path, precondition).await? {
            return Ok(false);
        }
        let abs = self.abs(path);
        let behavior = if *precondition == Precondition::Absent {
            "fail"
        } else {
            "replace"
        };
        let url = format!(
            "{}/content?@microsoft.graph.conflictBehavior={behavior}",
            self.item_url(&abs)
        );
        let mut created_parent = false;
        loop {
            let resp = self
                .send(&Method::PUT, &url, |r| {
                    let r = r
                        .header("Content-Type", "application/octet-stream")
                        .body(data.to_vec());
                    match precondition {
                        Precondition::Version(v) => r.header("If-Match", v),
                        _ => r,
                    }
                })
                .await?;
            match resp.status().as_u16() {
                200 | 201 => return Ok(true),
                412 => return Ok(false),
                409 if *precondition != Precondition::None => return Ok(false),
                404 if matches!(precondition, Precondition::Version(_)) => return Ok(false),
                // The parent folder is missing.
                404 | 409 if !created_parent => {
                    self.mkdir_parent(&abs).await?;
                    created_parent = true;
                }
                _ => return Err(http::unexpected(SERVICE, &Method::PUT, &url, resp).await),
            }
        }
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.delete_if(path, &Precondition::None).await?;
        Ok(())
    }

    async fn delete_if(&self, path: &str, precondition: &Precondition) -> Result<bool> {
        if *precondition == Precondition::Absent {
            return Ok(self.stat(path).await?.is_none());
        }
        let url = self.item_url(&self.abs(path));
        let resp = self
            .send(&Method::DELETE, &url, |r| match precondition {
                Precondition::Version(v) => r.header("If-Match", v),
                _ => r,
            })
            .await?;
        match resp.status().as_u16() {
            200 | 204 | 404 => Ok(true),
            412 => Ok(false),
            _ => Err(http::unexpected(SERVICE, &Method::DELETE, &url, resp).await),
        }
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        self.mkdir_abs(&self.abs(path)).await
    }

    async fn mkdir_exclusive(&self, path: &str) -> Result<bool> {
        let abs = self.abs(path);
        if let Some(created) = self.create_folder(&abs).await? {
            return Ok(created);
        }
        self.mkdir_parent(&abs).await?;
        self.create_folder(&abs)
            .await?
            .ok_or_else(|| SyncError::Remote(format!("{SERVICE}: could not create folder {abs}")))
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        let url = self.item_url(&self.abs(path));
        let resp = self.send(&Method::DELETE, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 | 204 | 404 => Ok(()),
            _ => Err(http::unexpected(SERVICE, &Method::DELETE, &url, resp).await),
        }
    }

    async fn list(&self, dir: &str) -> Result<Vec<RemoteStat>> {
        let dir = dir.trim_matches('/');
        let mut url = format!(
            "{}?$top=1000&$select=name,eTag,lastModifiedDateTime,folder,file",
            self.children_url(&self.abs(dir))
        );
        let mut out = Vec::new();
        loop {
            let resp = self.send(&Method::GET, &url, |r| r).await?;
            match resp.status().as_u16() {
                200 => {}
                404 => return Ok(Vec::new()),
                _ => return Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await),
            }
            let page: Children = resp
                .json()
                .await
                .map_err(|e| SyncError::Remote(format!("{SERVICE}: invalid listing: {e}")))?;
            for item in &page.value {
                out.push(Self::stat_of(join(dir, &item.name), item));
            }
            match page.next_link {
                Some(next) => url = next,
                None => return Ok(out),
            }
        }
    }

    async fn get_to_file(&self, path: &str, dest: &Path) -> Result<bool> {
        let url = format!("{}/content", self.item_url(&self.abs(path)));
        let resp = self.send(&Method::GET, &url, |r| r).await?;
        match resp.status().as_u16() {
            200 => {}
            404 => return Ok(false),
            _ => return Err(http::unexpected(SERVICE, &Method::GET, &url, resp).await),
        }
        http::write_stream(resp, dest).await?;
        Ok(true)
    }

    async fn put_file(&self, path: &str, src: &Path) -> Result<()> {
        let size = tokio::fs::metadata(src).await?.len();
        if size > self.simple_upload_limit {
            return self.upload_session(path, src, size).await;
        }
        let data = tokio::fs::read(src).await?;
        self.put(path, &data).await
    }
}

impl OneDriveStorage {
    async fn check(&self, path: &str, precondition: &Precondition) -> Result<bool> {
        Ok(match precondition {
            Precondition::None => true,
            Precondition::Absent => self.stat(path).await?.is_none(),
            Precondition::Version(v) => {
                self.stat(path).await?.and_then(|s| s.etag).as_deref() == Some(v.as_str())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls() {
        let config = OneDriveConfig {
            client_id: "id".into(),
            folder: "/Apps/My Notes/".into(),
            tokens: OAuthTokens::default(),
            endpoints: OneDriveEndpoints::default(),
        };
        let s = OneDriveStorage::new(&config, None).unwrap();
        assert_eq!(
            s.item_url(&s.abs("locks/1_desktop_x.json")),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Apps/My%20Notes/locks/1_desktop_x.json:"
        );
        assert_eq!(
            s.children_url(&s.abs("")),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Apps/My%20Notes:/children"
        );
        assert_eq!(
            s.children_url(""),
            "https://graph.microsoft.com/v1.0/me/drive/root/children"
        );
        assert_eq!(split_parent("a/b/c"), ("a/b", "c"));
        assert_eq!(split_parent("c"), ("", "c"));
    }
}
