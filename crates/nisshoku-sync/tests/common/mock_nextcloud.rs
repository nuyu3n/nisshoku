//! An in-process stand-in for a Nextcloud server, implementing the parts of
//! WebDAV, chunked upload, OCS and Login Flow v2 that the driver uses, with the
//! same status codes and quirks (second-precision modification times,
//! 409 for missing parents, 405 for existing collections, 412 for failed
//! preconditions). Faults can be injected.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use base64::Engine;
use nisshoku_sync::drivers::NextcloudConfig;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

pub const USER_ID: &str = "alice";
pub const LOGIN_NAME: &str = "alice@example.com";
pub const APP_PASSWORD: &str = "app-password-123";

const FILES_ROOT: &str = "/remote.php/dav/files/alice";
const UPLOADS_ROOT: &str = "/remote.php/dav/uploads/alice";
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

struct File {
    data: Vec<u8>,
    etag: String,
    mtime: SystemTime,
}

#[derive(Default)]
struct ServerState {
    base_url: String,
    files: BTreeMap<String, File>,
    dirs: BTreeSet<String>,
    fail_next: usize,
    /// (method, path fragment, remaining count): fail matching requests with 503.
    fail_rules: Vec<(String, String, usize)>,
    log: Vec<(String, String)>,
    login_polls: usize,
}

#[derive(Clone)]
pub struct MockNextcloud {
    pub url: String,
    state: Arc<Mutex<ServerState>>,
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|s| utf8_percent_encode(s, SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn now_secs() -> SystemTime {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    UNIX_EPOCH + std::time::Duration::from_secs(secs)
}

fn status(code: StatusCode) -> Response {
    Response::builder()
        .status(code)
        .body(Body::empty())
        .unwrap()
}

fn json(value: serde_json::Value) -> Response {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap()
}

impl MockNextcloud {
    pub async fn start() -> Self {
        let state = Arc::new(Mutex::new(ServerState::default()));
        {
            let mut s = state.lock().unwrap();
            for d in [
                "",
                "/remote.php",
                "/remote.php/dav",
                "/remote.php/dav/files",
                FILES_ROOT,
                "/remote.php/dav/uploads",
                UPLOADS_ROOT,
            ] {
                s.dirs.insert(d.to_string());
            }
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        state.lock().unwrap().base_url = url.clone();
        let app = axum::Router::new()
            .fallback(handle)
            .with_state(state.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, state }
    }

    pub fn config(&self, folder: &str) -> NextcloudConfig {
        NextcloudConfig {
            server_url: self.url.clone(),
            username: LOGIN_NAME.into(),
            password: APP_PASSWORD.into(),
            folder: folder.into(),
        }
    }

    /// The next `n` requests fail with 503.
    pub fn fail_next(&self, n: usize) {
        self.state.lock().unwrap().fail_next = n;
    }

    /// The next `n` requests with this method whose path contains `fragment` fail with 503.
    pub fn fail_when(&self, method: &str, fragment: &str, n: usize) {
        self.state
            .lock()
            .unwrap()
            .fail_rules
            .push((method.into(), fragment.into(), n));
    }

    /// (method, decoded path) of each request so far.
    pub fn requests(&self) -> Vec<(String, String)> {
        self.state.lock().unwrap().log.clone()
    }

    /// Content of a file, by path relative to the user's files.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        let s = self.state.lock().unwrap();
        s.files
            .get(&format!("{FILES_ROOT}/{path}"))
            .map(|f| f.data.clone())
    }

    pub fn has_dir(&self, path: &str) -> bool {
        self.state
            .lock()
            .unwrap()
            .dirs
            .contains(&format!("{FILES_ROOT}/{path}"))
    }

    /// Paths of pending (unfinished) chunked uploads.
    pub fn pending_uploads(&self) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.dirs
            .iter()
            .filter(|d| parent(d) == UPLOADS_ROOT)
            .cloned()
            .collect()
    }
}

async fn handle(State(state): State<Arc<Mutex<ServerState>>>, req: Request) -> Response {
    let method = req.method().clone();
    let raw_path = req.uri().path().to_string();
    let path = percent_decode_str(&raw_path)
        .decode_utf8_lossy()
        .into_owned();
    let path = if path.len() > 1 {
        path.trim_end_matches('/').to_string()
    } else {
        String::new()
    };
    let headers = req.headers().clone();
    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();

    // Like Nextcloud, preconditions are not checked atomically with the
    // write: a concurrent request may slip in between.
    if matches!(method.as_str(), "PUT" | "DELETE")
        && path.starts_with("/remote.php/dav/")
        && authorized(&headers)
    {
        let ok = {
            let s = state.lock().unwrap();
            precondition_ok(&s, &path, &headers)
        };
        if !ok {
            return status(StatusCode::PRECONDITION_FAILED);
        }
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    }

    let mut s = state.lock().unwrap();
    s.log.push((method.to_string(), path.clone()));
    if s.fail_next > 0 {
        s.fail_next -= 1;
        return status(StatusCode::SERVICE_UNAVAILABLE);
    }
    let rule = s
        .fail_rules
        .iter_mut()
        .find(|(m, frag, n)| *n > 0 && m == method.as_str() && path.contains(frag.as_str()));
    if let Some(rule) = rule {
        rule.2 -= 1;
        return status(StatusCode::SERVICE_UNAVAILABLE);
    }

    // Login flow v2 (unauthenticated).
    if method == Method::POST && path == "/index.php/login/v2" {
        let base = s.base_url.clone();
        return json(serde_json::json!({
            "poll": { "token": "poll-token", "endpoint": format!("{base}/login/v2/poll") },
            "login": format!("{base}/login/v2/flow/xyz"),
        }));
    }
    if method == Method::POST && path == "/login/v2/poll" {
        if String::from_utf8_lossy(&body) != "token=poll%2Dtoken"
            && String::from_utf8_lossy(&body) != "token=poll-token"
        {
            return status(StatusCode::NOT_FOUND);
        }
        s.login_polls += 1;
        if s.login_polls < 2 {
            return status(StatusCode::NOT_FOUND);
        }
        let base = s.base_url.clone();
        return json(serde_json::json!({
            "server": base, "loginName": LOGIN_NAME, "appPassword": APP_PASSWORD,
        }));
    }

    if !authorized(&headers) {
        return status(StatusCode::UNAUTHORIZED);
    }

    if path == "/ocs/v2.php/cloud/user" {
        if headers.get("OCS-APIRequest").is_none() {
            return status(StatusCode::BAD_REQUEST);
        }
        return json(
            serde_json::json!({ "ocs": { "meta": { "status": "ok", "statuscode": 200 }, "data": { "id": USER_ID } } }),
        );
    }
    if !path.starts_with("/remote.php/dav/") {
        return status(StatusCode::NOT_FOUND);
    }

    match method.as_str() {
        "PROPFIND" => propfind(&s, &path, &headers),
        "GET" => match s.files.get(&path) {
            Some(f) => Response::builder()
                .status(200)
                .header("ETag", &f.etag)
                .header("Last-Modified", httpdate::fmt_http_date(f.mtime))
                .header("Date", httpdate::fmt_http_date(SystemTime::now()))
                .body(Body::from(f.data.clone()))
                .unwrap(),
            None => status(StatusCode::NOT_FOUND),
        },
        "PUT" => {
            if !s.dirs.contains(parent(&path)) {
                return status(StatusCode::CONFLICT);
            }
            if s.dirs.contains(&path) {
                return status(StatusCode::METHOD_NOT_ALLOWED);
            }
            let existed = s.files.contains_key(&path);
            let mtime = requested_mtime(&headers);
            let etag = write_file(&mut s, &path, body, mtime);
            let mut resp = Response::builder()
                .status(if existed { 204 } else { 201 })
                .header("ETag", etag)
                .header("Date", httpdate::fmt_http_date(SystemTime::now()));
            if mtime.is_some() {
                resp = resp.header("X-OC-MTime", "accepted");
            }
            resp.body(Body::empty()).unwrap()
        }
        "DELETE" => {
            if s.files.remove(&path).is_some() {
                return status(StatusCode::NO_CONTENT);
            }
            if s.dirs.contains(&path) {
                let prefix = format!("{path}/");
                s.dirs.retain(|d| d != &path && !d.starts_with(&prefix));
                s.files.retain(|f, _| !f.starts_with(&prefix));
                return status(StatusCode::NO_CONTENT);
            }
            status(StatusCode::NOT_FOUND)
        }
        "MKCOL" => {
            if s.dirs.contains(&path) || s.files.contains_key(&path) {
                return status(StatusCode::METHOD_NOT_ALLOWED);
            }
            if !s.dirs.contains(parent(&path)) {
                return status(StatusCode::CONFLICT);
            }
            s.dirs.insert(path);
            status(StatusCode::CREATED)
        }
        "MOVE" => {
            let Some(dest) = headers
                .get("Destination")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| reqwest::Url::parse(v).ok())
                .map(|u| {
                    percent_decode_str(u.path())
                        .decode_utf8_lossy()
                        .into_owned()
                })
            else {
                return status(StatusCode::BAD_REQUEST);
            };
            if !s.dirs.contains(parent(&dest)) {
                return status(StatusCode::CONFLICT);
            }
            let existed = s.files.contains_key(&dest);
            if path.ends_with("/.file") && parent(parent(&path)) == UPLOADS_ROOT {
                let upload = parent(&path).to_string();
                let prefix = format!("{upload}/");
                let mut data = Vec::new();
                for (_, f) in s
                    .files
                    .range(prefix.clone()..)
                    .take_while(|(k, _)| k.starts_with(&prefix))
                {
                    data.extend_from_slice(&f.data);
                }
                let expected = headers
                    .get("OC-Total-Length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<usize>().ok());
                if expected.is_some_and(|n| n != data.len()) {
                    return status(StatusCode::BAD_REQUEST);
                }
                s.files.retain(|f, _| !f.starts_with(&prefix));
                s.dirs.remove(&upload);
                write_file(&mut s, &dest, data, requested_mtime(&headers));
            } else {
                let Some(f) = s.files.remove(&path) else {
                    return status(StatusCode::NOT_FOUND);
                };
                write_file(&mut s, &dest, f.data, requested_mtime(&headers));
            }
            status(if existed {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::CREATED
            })
        }
        _ => status(StatusCode::METHOD_NOT_ALLOWED),
    }
}

fn authorized(headers: &HeaderMap) -> bool {
    let expected = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{LOGIN_NAME}:{APP_PASSWORD}"))
    );
    headers.get("Authorization").and_then(|v| v.to_str().ok()) == Some(expected.as_str())
}

fn precondition_ok(s: &ServerState, path: &str, headers: &HeaderMap) -> bool {
    let current = s.files.get(path).map(|f| f.etag.as_str());
    if let Some(v) = headers.get("If-Match").and_then(|v| v.to_str().ok())
        && current != Some(v)
    {
        return false;
    }
    if headers.get("If-None-Match").and_then(|v| v.to_str().ok()) == Some("*") && current.is_some()
    {
        return false;
    }
    true
}

/// Like Nextcloud's local storage, the ETag only depends on the path and the
/// modification time in whole seconds: two writes within a second share one.
fn write_file(s: &mut ServerState, path: &str, data: Vec<u8>, mtime: Option<SystemTime>) -> String {
    use std::hash::{Hash, Hasher};
    let mtime = mtime.unwrap_or_else(now_secs);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    mtime
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .hash(&mut h);
    let etag = format!("\"{:016x}\"", h.finish());
    s.files.insert(
        path.to_string(),
        File {
            data,
            etag: etag.clone(),
            mtime,
        },
    );
    etag
}

fn requested_mtime(headers: &HeaderMap) -> Option<SystemTime> {
    let secs: u64 = headers.get("X-OC-MTime")?.to_str().ok()?.parse().ok()?;
    Some(UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

fn propfind(s: &ServerState, path: &str, headers: &HeaderMap) -> Response {
    let depth = headers
        .get("Depth")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("1");
    let mut entries = Vec::new();
    if let Some(f) = s.files.get(path) {
        entries.push(file_entry(path, f));
    } else if s.dirs.contains(path) {
        entries.push(dir_entry(path));
        if depth != "0" {
            for d in s.dirs.iter().filter(|d| parent(d) == path && !d.is_empty()) {
                entries.push(dir_entry(d));
            }
            for (p, f) in s.files.iter().filter(|(p, _)| parent(p) == path) {
                entries.push(file_entry(p, f));
            }
        }
    } else {
        return status(StatusCode::NOT_FOUND);
    }
    let xml = format!(
        r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">{}</d:multistatus>"#,
        entries.join("")
    );
    Response::builder()
        .status(207)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(Body::from(xml))
        .unwrap()
}

fn file_entry(path: &str, f: &File) -> String {
    format!(
        "<d:response><d:href>{}</d:href><d:propstat><d:prop><d:getlastmodified>{}</d:getlastmodified>\
         <d:getetag>{}</d:getetag><d:resourcetype/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>\
         <d:propstat><d:prop><oc:unknown/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>",
        encode_path(path),
        httpdate::fmt_http_date(f.mtime),
        f.etag.replace('"', "&quot;"),
    )
}

fn dir_entry(path: &str) -> String {
    format!(
        "<d:response><d:href>{}/</d:href><d:propstat><d:prop><d:getlastmodified>{}</d:getlastmodified>\
         <d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
        encode_path(path),
        httpdate::fmt_http_date(now_secs()),
    )
}
