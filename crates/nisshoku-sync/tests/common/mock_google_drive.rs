//! An in-process stand-in for the Google Drive API v3: files addressed by
//! ID, duplicate names, the query language subset the driver uses, paged
//! listings, generated IDs, multipart and resumable uploads, rate-limit
//! `403`s and OAuth tokens. Faults can be injected.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method};
use axum::response::Response;
use nisshoku_sync::drivers::GoogleDriveConfig;
use nisshoku_sync::drivers::google_drive::GoogleDriveEndpoints;
use percent_encoding::percent_decode_str;

use super::mock_oauth::{self, TokenIssuer, parse_form};

pub const CLIENT_ID: &str = "gdrive-test-client.apps.googleusercontent.com";
pub const CLIENT_SECRET: &str = "gdrive-test-secret";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
const CHUNK_UNIT: usize = 256 * 1024;

#[derive(Clone)]
struct File {
    name: String,
    parent: String,
    mime: String,
    data: Vec<u8>,
    created: i64,
    modified: i64,
    version: u64,
}

enum UploadTarget {
    Create(serde_json::Value),
    Update(String),
}

struct Session {
    target: UploadTarget,
    data: Vec<u8>,
}

#[derive(Default)]
struct ServerState {
    base_url: String,
    files: BTreeMap<String, File>,
    issued_ids: HashSet<String>,
    sessions: HashMap<String, Session>,
    next: u64,
    last_time: i64,
    page_size: usize,
    tokens: TokenIssuer,
    /// (method, path fragment, remaining count, after, rate limit).
    fail_rules: Vec<(String, String, usize, bool, bool)>,
    log: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct MockGoogleDrive {
    pub url: String,
    state: Arc<Mutex<ServerState>>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn rfc3339(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

fn respond(status: u16, value: serde_json::Value) -> Response {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap()
}

fn error(status: u16, reason: &str) -> Response {
    respond(
        status,
        serde_json::json!({ "error": { "code": status, "message": reason, "errors": [{ "reason": reason }] } }),
    )
}

fn empty(status: u16) -> Response {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}

// -------------------------------------------------------------------------
// Queries

#[derive(Debug, PartialEq)]
enum Filter {
    InParents(String),
    Name(String, bool),
    Mime(String, bool),
    Trashed(bool),
}

fn tokenize(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = q.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '\'' {
            chars.next();
            let mut lit = String::from("'");
            while let Some(c) = chars.next() {
                match c {
                    '\\' => lit.push(chars.next().unwrap()),
                    '\'' => break,
                    c => lit.push(c),
                }
            }
            out.push(lit);
        } else if c == '=' || c == '!' {
            let mut op = String::new();
            while let Some(&c) = chars.peek() {
                if c == '=' || c == '!' {
                    op.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(op);
        } else {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '=' || c == '!' || c == '\'' {
                    break;
                }
                word.push(c);
                chars.next();
            }
            out.push(word);
        }
    }
    out
}

fn parse_query(q: &str) -> Option<Vec<Filter>> {
    let tokens = tokenize(q);
    let mut filters = Vec::new();
    for clause in tokens.split(|t| t == "and") {
        let lit = |t: &String| t.strip_prefix('\'').map(str::to_string);
        filters.push(match clause {
            [a, op, b] if op == "in" && b == "parents" => Filter::InParents(lit(a)?),
            [f, op, v] if f == "name" => Filter::Name(lit(v)?, op == "="),
            [f, op, v] if f == "mimeType" => Filter::Mime(lit(v)?, op == "="),
            [f, op, v] if f == "trashed" && op == "=" => Filter::Trashed(v == "true"),
            _ => return None,
        });
    }
    Some(filters)
}

fn matches(f: &File, filters: &[Filter]) -> bool {
    filters.iter().all(|filter| match filter {
        Filter::InParents(p) => f.parent == *p,
        Filter::Name(n, eq) => (f.name == *n) == *eq,
        Filter::Mime(m, eq) => (f.mime == *m) == *eq,
        Filter::Trashed(t) => !*t,
    })
}

impl ServerState {
    fn time(&mut self) -> i64 {
        self.last_time = now_ms().max(self.last_time + 1);
        self.last_time
    }

    fn json_of(&self, id: &str) -> serde_json::Value {
        let f = &self.files[id];
        serde_json::json!({
            "kind": "drive#file",
            "id": id,
            "name": f.name,
            "mimeType": f.mime,
            "parents": [f.parent],
            "createdTime": rfc3339(f.created),
            "modifiedTime": rfc3339(f.modified),
            "version": f.version.to_string(),
            "size": f.data.len().to_string(),
            "trashed": false,
        })
    }

    fn parent_ok(&self, parent: &str) -> bool {
        parent == "root"
            || self
                .files
                .get(parent)
                .is_some_and(|f| f.mime == FOLDER_MIME)
    }

    /// Creates a file from metadata (with an ID from `generateIds`).
    fn create(&mut self, meta: &serde_json::Value, data: Vec<u8>) -> Response {
        let id = match meta["id"].as_str() {
            Some(id) if self.issued_ids.contains(id) => id.to_string(),
            Some(_) => return error(400, "invalidId"),
            None => {
                self.next += 1;
                format!("auto{:010}", self.next)
            }
        };
        if self.files.contains_key(&id) {
            return error(409, "fileIdInUse");
        }
        let parent = meta["parents"][0].as_str().unwrap_or("root").to_string();
        if !self.parent_ok(&parent) {
            return error(404, "notFound");
        }
        let t = self.time();
        let file = File {
            name: meta["name"].as_str().unwrap_or("Untitled").to_string(),
            parent,
            mime: meta["mimeType"]
                .as_str()
                .unwrap_or("application/octet-stream")
                .to_string(),
            data,
            created: t,
            modified: t,
            version: 1,
        };
        self.files.insert(id.clone(), file);
        respond(200, self.json_of(&id))
    }

    fn update(&mut self, id: &str, data: Vec<u8>) -> Response {
        let t = self.time();
        let Some(f) = self.files.get_mut(id) else {
            return error(404, "notFound");
        };
        f.data = data;
        f.modified = t;
        f.version += 1;
        respond(200, self.json_of(id))
    }

    fn delete_tree(&mut self, id: &str) {
        let children: Vec<String> = self
            .files
            .iter()
            .filter(|(_, f)| f.parent == id)
            .map(|(k, _)| k.clone())
            .collect();
        for c in children {
            self.delete_tree(&c);
        }
        self.files.remove(id);
    }

    /// The file at a path of names from My Drive (oldest first among duplicates).
    fn find(&self, path: &str) -> Option<String> {
        let mut current = "root".to_string();
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            current = self
                .files
                .iter()
                .filter(|(_, f)| f.parent == current && f.name == segment)
                .min_by_key(|(id, f)| (f.created, (*id).clone()))
                .map(|(id, _)| id.clone())?;
        }
        Some(current)
    }
}

impl MockGoogleDrive {
    pub async fn start() -> Self {
        let state = Arc::new(Mutex::new(ServerState {
            page_size: 50,
            ..Default::default()
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        state.lock().unwrap().base_url = url.clone();
        let app = axum::Router::new()
            .fallback(handle)
            .with_state(state.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, state }
    }

    pub fn endpoints(&self) -> GoogleDriveEndpoints {
        GoogleDriveEndpoints {
            api: format!("{}/drive/v3", self.url),
            upload: format!("{}/upload/drive/v3", self.url),
            auth: format!("{}{}", self.url, mock_oauth::AUTH_PATH),
            token: format!("{}{}", self.url, mock_oauth::TOKEN_PATH),
        }
    }

    pub fn config(&self, folder: &str) -> GoogleDriveConfig {
        GoogleDriveConfig {
            client_id: CLIENT_ID.into(),
            client_secret: CLIENT_SECRET.into(),
            folder: folder.into(),
            tokens: self.state.lock().unwrap().tokens.initial(),
            endpoints: self.endpoints(),
        }
    }

    pub fn expire_tokens(&self) {
        self.state.lock().unwrap().tokens.expire_all();
    }

    pub fn token_refreshes(&self) -> usize {
        self.state.lock().unwrap().tokens.refreshes
    }

    pub fn set_page_size(&self, n: usize) {
        self.state.lock().unwrap().page_size = n;
    }

    /// The next `n` matching requests fail with 503 without effect.
    pub fn fail_when(&self, method: &str, fragment: &str, n: usize) {
        self.state.lock().unwrap().fail_rules.push((
            method.into(),
            fragment.into(),
            n,
            false,
            false,
        ));
    }

    /// The next `n` matching requests take effect, then fail with 503.
    pub fn fail_after_when(&self, method: &str, fragment: &str, n: usize) {
        self.state.lock().unwrap().fail_rules.push((
            method.into(),
            fragment.into(),
            n,
            true,
            false,
        ));
    }

    /// The next `n` matching requests fail with a rate-limit 403.
    pub fn rate_limit_when(&self, method: &str, fragment: &str, n: usize) {
        self.state.lock().unwrap().fail_rules.push((
            method.into(),
            fragment.into(),
            n,
            false,
            true,
        ));
    }

    pub fn requests(&self) -> Vec<(String, String)> {
        self.state.lock().unwrap().log.clone()
    }

    /// Content of the file at a path of names from My Drive.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        let s = self.state.lock().unwrap();
        s.find(path).map(|id| s.files[&id].data.clone())
    }

    /// Number of files (and folders) named like the last segment of `path`
    /// in its folder.
    pub fn count_named(&self, path: &str) -> usize {
        let s = self.state.lock().unwrap();
        let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
        let Some(parent) = (if dir.is_empty() {
            Some("root".to_string())
        } else {
            s.find(dir)
        }) else {
            return 0;
        };
        s.files
            .values()
            .filter(|f| f.parent == parent && f.name == name)
            .count()
    }

    /// Adds a file named like an existing one, as another app (or a lost
    /// response) could have, created after it.
    pub fn add_duplicate(&self, path: &str, data: &[u8]) {
        let mut s = self.state.lock().unwrap();
        let (dir, name) = path.rsplit_once('/').unwrap();
        let parent = s.find(dir).unwrap();
        s.next += 1;
        let id = format!("dup{:010}", s.next);
        let t = s.time();
        s.files.insert(
            id,
            File {
                name: name.into(),
                parent,
                mime: "application/octet-stream".into(),
                data: data.to_vec(),
                created: t,
                modified: t,
                version: 1,
            },
        );
    }

    pub fn pending_sessions(&self) -> usize {
        self.state.lock().unwrap().sessions.len()
    }
}

async fn handle(State(state): State<Arc<Mutex<ServerState>>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = percent_decode_str(req.uri().path())
        .decode_utf8_lossy()
        .into_owned();
    let query = req.uri().query().unwrap_or("").to_string();
    let headers = req.headers().clone();
    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();

    let mut s = state.lock().unwrap();
    s.log.push((method.to_string(), format!("{path}?{query}")));
    let mut fail_after = false;
    let target = format!("{path}?{query}");
    if let Some(rule) = s
        .fail_rules
        .iter_mut()
        .find(|(m, f, n, _, _)| *n > 0 && m == method.as_str() && target.contains(f.as_str()))
    {
        rule.2 -= 1;
        if rule.4 {
            return error(403, "userRateLimitExceeded");
        }
        if !rule.3 {
            return error(503, "backendError");
        }
        fail_after = true;
    }
    let response = route(&mut s, &method, &path, &query, &headers, body);
    if fail_after {
        return error(503, "backendError");
    }
    response
}

fn route(
    s: &mut ServerState,
    method: &Method,
    path: &str,
    query: &str,
    headers: &HeaderMap,
    body: Vec<u8>,
) -> Response {
    if path == mock_oauth::TOKEN_PATH && *method == Method::POST {
        let form = parse_form(&body);
        if form.get("client_secret").map(String::as_str) != Some(CLIENT_SECRET) {
            return respond(401, serde_json::json!({ "error": "invalid_client" }));
        }
        return s.tokens.token_endpoint(&body, CLIENT_ID);
    }
    if path == mock_oauth::AUTH_PATH {
        return s.tokens.authorize(query);
    }
    if !s.tokens.authorized(headers) {
        return error(401, "authError");
    }
    let params = parse_form(query.as_bytes());
    let param = |k: &str| params.get(k).map(String::as_str);

    // Uploads.
    if let Some(rest) = path.strip_prefix("/upload/drive/v3/files") {
        let id = rest.trim_start_matches('/');
        if let Some(sid) = param("upload_id") {
            return upload_chunk(s, sid, headers, body);
        }
        return match (method.as_str(), param("uploadType"), id.is_empty()) {
            ("POST", Some("multipart"), true) => {
                let Some((meta, data)) = parse_multipart(headers, &body) else {
                    return error(400, "badMultipart");
                };
                s.create(&meta, data)
            }
            ("PATCH", Some("media"), false) => s.update(id, body),
            ("POST", Some("resumable"), true) | ("PATCH", Some("resumable"), false) => {
                let target = if id.is_empty() {
                    let meta: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
                    if !s.parent_ok(meta["parents"][0].as_str().unwrap_or("root")) {
                        return error(404, "notFound");
                    }
                    UploadTarget::Create(meta)
                } else {
                    if !s.files.contains_key(id) {
                        return error(404, "notFound");
                    }
                    UploadTarget::Update(id.to_string())
                };
                s.next += 1;
                let sid = format!("session{}", s.next);
                s.sessions.insert(
                    sid.clone(),
                    Session {
                        target,
                        data: vec![],
                    },
                );
                Response::builder()
                    .status(200)
                    .header(
                        "Location",
                        format!(
                            "{}/upload/drive/v3/files?uploadType=resumable&upload_id={sid}",
                            s.base_url
                        ),
                    )
                    .body(Body::empty())
                    .unwrap()
            }
            _ => error(400, "badRequest"),
        };
    }

    let Some(rest) = path.strip_prefix("/drive/v3/files") else {
        return error(404, "notFound");
    };
    let id = rest.trim_start_matches('/');
    match (method.as_str(), id) {
        ("GET", "") => {
            let Some(filters) = parse_query(param("q").unwrap_or("")) else {
                return error(400, "invalidQuery");
            };
            let page_size = param("pageSize")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(100)
                .min(s.page_size);
            let skip: usize = param("pageToken").and_then(|v| v.parse().ok()).unwrap_or(0);
            let ids: Vec<String> = s
                .files
                .iter()
                .filter(|(_, f)| matches(f, &filters))
                .map(|(id, _)| id.clone())
                .collect();
            let page: Vec<_> = ids
                .iter()
                .skip(skip)
                .take(page_size)
                .map(|id| s.json_of(id))
                .collect();
            let mut v = serde_json::json!({ "kind": "drive#fileList", "files": page });
            if skip + page_size < ids.len() {
                v["nextPageToken"] = (skip + page_size).to_string().into();
            }
            respond(200, v)
        }
        ("GET", "generateIds") => {
            let count: usize = param("count").and_then(|v| v.parse().ok()).unwrap_or(10);
            let ids: Vec<String> = (0..count)
                .map(|_| {
                    s.next += 1;
                    let id = format!("gen{:010}{}", s.next, &nisshoku_core::ids::new_id()[..6]);
                    s.issued_ids.insert(id.clone());
                    id
                })
                .collect();
            respond(
                200,
                serde_json::json!({ "kind": "drive#generatedIds", "ids": ids }),
            )
        }
        ("POST", "") => {
            let meta: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            s.create(&meta, vec![])
        }
        ("GET", id) => match s.files.get(id) {
            Some(f) if param("alt") == Some("media") => {
                if f.mime == FOLDER_MIME {
                    return error(403, "fileNotDownloadable");
                }
                Response::builder()
                    .status(200)
                    .body(Body::from(f.data.clone()))
                    .unwrap()
            }
            Some(_) => respond(200, s.json_of(id)),
            None => error(404, "notFound"),
        },
        ("DELETE", id) => {
            if !s.files.contains_key(id) {
                return error(404, "notFound");
            }
            s.delete_tree(id);
            empty(204)
        }
        _ => error(400, "badRequest"),
    }
}

fn parse_multipart(headers: &HeaderMap, body: &[u8]) -> Option<(serde_json::Value, Vec<u8>)> {
    let ct = headers.get("Content-Type")?.to_str().ok()?;
    let boundary = ct.split("boundary=").nth(1)?.trim_matches('"');
    let delimiter = format!("--{boundary}");
    let find = |hay: &[u8], needle: &[u8], from: usize| {
        hay[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|p| p + from)
    };
    let mut parts = Vec::new();
    let mut pos = find(body, delimiter.as_bytes(), 0)? + delimiter.len();
    loop {
        if body[pos..].starts_with(b"--") {
            break;
        }
        let head_end = find(body, b"\r\n\r\n", pos)? + 4;
        let next = find(body, format!("\r\n{delimiter}").as_bytes(), head_end)?;
        parts.push(body[head_end..next].to_vec());
        pos = next + 2 + delimiter.len();
    }
    let [meta, data] = <[Vec<u8>; 2]>::try_from(parts).ok()?;
    Some((serde_json::from_slice(&meta).ok()?, data))
}

fn upload_chunk(s: &mut ServerState, sid: &str, headers: &HeaderMap, body: Vec<u8>) -> Response {
    let Some(range) = headers
        .get("Content-Range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
    else {
        return error(400, "badRange");
    };
    let (span, total) = range.split_once('/').unwrap();
    let total: usize = total.parse().unwrap();
    let Some(session) = s.sessions.get_mut(sid) else {
        return error(404, "notFound");
    };
    if span != "*" {
        let (start, end) = span.split_once('-').unwrap();
        let (start, end): (usize, usize) = (start.parse().unwrap(), end.parse().unwrap());
        if start > session.data.len() || end + 1 - start != body.len() {
            return error(400, "badRange");
        }
        // A resent chunk replaces what was received from its start.
        session.data.truncate(start);
        if end + 1 != total && !body.len().is_multiple_of(CHUNK_UNIT) {
            return error(400, "badChunkSize");
        }
        session.data.extend_from_slice(&body);
    }
    if session.data.len() < total {
        return Response::builder()
            .status(308)
            .header("Range", format!("bytes=0-{}", session.data.len() - 1))
            .body(Body::empty())
            .unwrap();
    }
    let session = s.sessions.remove(sid).unwrap();
    match session.target {
        UploadTarget::Create(meta) => s.create(&meta, session.data),
        UploadTarget::Update(id) => s.update(&id, session.data),
    }
}

#[test]
fn parses_queries() {
    assert_eq!(
        parse_query(r"'abc' in parents and name = 'it\'s and x' and trashed = false"),
        Some(vec![
            Filter::InParents("abc".into()),
            Filter::Name("it's and x".into(), true),
            Filter::Trashed(false),
        ])
    );
}
