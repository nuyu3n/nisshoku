//! An in-process stand-in for the Microsoft Graph drive API: path-addressed
//! items, eTags, `If-Match`, `conflictBehavior`, redirects to download URLs,
//! paged listings, upload sessions and OAuth tokens. Like the real service
//! (as far as we can tell), preconditions are not checked atomically with
//! writes. Faults can be injected.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method};
use axum::response::Response;
use nisshoku_sync::drivers::onedrive::OneDriveEndpoints;
use nisshoku_sync::drivers::{OAuthTokens, OneDriveConfig};
use percent_encoding::percent_decode_str;

use super::mock_oauth::{self, TokenIssuer};

pub const CLIENT_ID: &str = "onedrive-test-client";
const API: &str = "/v1.0/me/drive";

struct Item {
    id: String,
    dir: bool,
    data: Vec<u8>,
    version: u64,
    /// Whole seconds, like the service's timestamps.
    modified: u64,
}

impl Item {
    fn etag(&self) -> String {
        format!("\"{{{}}},{}\"", self.id, self.version)
    }
}

struct Session {
    path: String,
    total: u64,
    data: Vec<u8>,
}

#[derive(Default)]
struct ServerState {
    base_url: String,
    items: BTreeMap<String, Item>,
    sessions: HashMap<String, Session>,
    next_id: u64,
    page_size: usize,
    tokens: TokenIssuer,
    /// (method, path fragment, remaining count, after): fail matching
    /// requests with 503, before or after performing them.
    fail_rules: Vec<(String, String, usize, bool)>,
    /// Requests to answer with 429 and `Retry-After: 0`.
    throttle_next: usize,
    log: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct MockOneDrive {
    pub url: String,
    state: Arc<Mutex<ServerState>>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn rfc3339(secs: u64) -> String {
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}

fn name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, n)| n)
}

fn respond(status: u16, value: serde_json::Value) -> Response {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap()
}

fn error(status: u16, code: &str) -> Response {
    respond(
        status,
        serde_json::json!({ "error": { "code": code, "message": code } }),
    )
}

fn empty(status: u16) -> Response {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}

impl ServerState {
    fn item_json(&self, path: &str) -> serde_json::Value {
        let item = &self.items[path];
        let mut v = serde_json::json!({
            "id": item.id,
            "name": if path.is_empty() { "root" } else { name(path) },
            "eTag": item.etag(),
            "cTag": item.etag(),
            "lastModifiedDateTime": rfc3339(item.modified),
            "size": item.data.len(),
        });
        if item.dir {
            v["folder"] = serde_json::json!({ "childCount": self.children(path).len() });
        } else {
            v["file"] = serde_json::json!({ "mimeType": "application/octet-stream" });
            v["@microsoft.graph.downloadUrl"] =
                format!("{}/download/{}?v={}", self.base_url, item.id, item.version).into();
        }
        v
    }

    fn children(&self, dir: &str) -> Vec<String> {
        self.items
            .keys()
            .filter(|p| !p.is_empty() && parent(p) == dir)
            .cloned()
            .collect()
    }

    fn new_id(&mut self) -> String {
        self.next_id += 1;
        format!("ITEM{:08}", self.next_id)
    }

    fn write_file(&mut self, path: &str, data: Vec<u8>) -> u16 {
        let now = now_secs();
        let (status, id, version) = match self.items.get(path) {
            Some(item) => (200, item.id.clone(), item.version + 1),
            None => (201, self.new_id(), 1),
        };
        self.items.insert(
            path.to_string(),
            Item {
                id,
                dir: false,
                data,
                version,
                modified: now,
            },
        );
        status
    }

    fn remove_tree(&mut self, path: &str) {
        if path.is_empty() {
            self.items.clear();
            self.items.insert(
                String::new(),
                Item {
                    id: "ROOT".into(),
                    dir: true,
                    data: vec![],
                    version: 1,
                    modified: now_secs(),
                },
            );
            return;
        }
        let below = format!("{path}/");
        self.items
            .retain(|p, _| p != path && !p.starts_with(&below));
    }
}

impl MockOneDrive {
    pub async fn start() -> Self {
        let state = Arc::new(Mutex::new(ServerState {
            page_size: 50,
            ..Default::default()
        }));
        state.lock().unwrap().remove_tree("");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        state.lock().unwrap().base_url = url.clone();
        let app = axum::Router::new()
            .fallback(handle)
            .with_state(state.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, state }
    }

    pub fn endpoints(&self) -> OneDriveEndpoints {
        OneDriveEndpoints {
            api: format!("{}{API}", self.url),
            auth: format!("{}{}", self.url, mock_oauth::AUTH_PATH),
            token: format!("{}{}", self.url, mock_oauth::TOKEN_PATH),
        }
    }

    /// A configuration with fresh tokens for a new client.
    pub fn config(&self, folder: &str) -> OneDriveConfig {
        OneDriveConfig {
            client_id: CLIENT_ID.into(),
            folder: folder.into(),
            tokens: self.state.lock().unwrap().tokens.initial(),
            endpoints: self.endpoints(),
        }
    }

    pub fn initial_tokens(&self) -> OAuthTokens {
        self.state.lock().unwrap().tokens.initial()
    }

    pub fn expire_tokens(&self) {
        self.state.lock().unwrap().tokens.expire_all();
    }

    pub fn revoke_refresh_tokens(&self) {
        self.state.lock().unwrap().tokens.revoke_refresh_tokens();
    }

    pub fn token_refreshes(&self) -> usize {
        self.state.lock().unwrap().tokens.refreshes
    }

    pub fn set_page_size(&self, n: usize) {
        self.state.lock().unwrap().page_size = n;
    }

    /// The next `n` matching requests fail with 503 without effect.
    pub fn fail_when(&self, method: &str, fragment: &str, n: usize) {
        self.state
            .lock()
            .unwrap()
            .fail_rules
            .push((method.into(), fragment.into(), n, false));
    }

    /// The next `n` matching requests take effect, then fail with 503.
    pub fn fail_after_when(&self, method: &str, fragment: &str, n: usize) {
        self.state
            .lock()
            .unwrap()
            .fail_rules
            .push((method.into(), fragment.into(), n, true));
    }

    pub fn throttle_next(&self, n: usize) {
        self.state.lock().unwrap().throttle_next = n;
    }

    pub fn requests(&self) -> Vec<(String, String)> {
        self.state.lock().unwrap().log.clone()
    }

    /// Content of a file, by path in the drive.
    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        let s = self.state.lock().unwrap();
        s.items.get(path).filter(|i| !i.dir).map(|i| i.data.clone())
    }

    pub fn pending_sessions(&self) -> usize {
        self.state.lock().unwrap().sessions.len()
    }
}

/// `(item path, rest)` from a path like `/root:/a/b:/content`.
fn parse_item_path(rest: &str) -> Option<(String, String)> {
    if let Some(r) = rest.strip_prefix("/root:/") {
        let (p, tail) = r.split_once(':').unwrap_or((r, ""));
        return Some((p.trim_matches('/').to_string(), tail.to_string()));
    }
    let tail = rest.strip_prefix("/root")?;
    Some((String::new(), tail.to_string()))
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

    // Preconditions are checked before, not atomically with, the write.
    if matches!(method.as_str(), "PUT" | "DELETE") && path.starts_with(API) {
        let ok = {
            let s = state.lock().unwrap();
            precondition_ok(&s, &path, &headers)
        };
        if !ok {
            return error(412, "preconditionFailed");
        }
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    }

    let mut s = state.lock().unwrap();
    s.log.push((method.to_string(), path.clone()));
    if s.throttle_next > 0 {
        s.throttle_next -= 1;
        return Response::builder()
            .status(429)
            .header("Retry-After", "0")
            .body(Body::empty())
            .unwrap();
    }
    let mut fail_after = false;
    if let Some(rule) = s
        .fail_rules
        .iter_mut()
        .find(|(m, f, n, _)| *n > 0 && m == method.as_str() && path.contains(f.as_str()))
    {
        rule.2 -= 1;
        if !rule.3 {
            return error(503, "serviceNotAvailable");
        }
        fail_after = true;
    }
    let response = route(&mut s, &method, &path, &query, &headers, body);
    if fail_after {
        return error(503, "serviceNotAvailable");
    }
    response
}

fn precondition_ok(s: &ServerState, path: &str, headers: &HeaderMap) -> bool {
    let Some(expected) = headers.get("If-Match").and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let Some((item, tail)) = path.strip_prefix(API).and_then(parse_item_path) else {
        return true;
    };
    let _ = tail;
    s.items.get(&item).is_none_or(|i| i.etag() == expected)
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
        return s.tokens.token_endpoint(&body, CLIENT_ID);
    }
    if path == mock_oauth::AUTH_PATH {
        return s.tokens.authorize(query);
    }
    // Pre-authenticated URLs.
    if let Some(id) = path.strip_prefix("/download/") {
        return match s.items.values().find(|i| i.id == id && !i.dir) {
            Some(i) => Response::builder()
                .status(200)
                .body(Body::from(i.data.clone()))
                .unwrap(),
            None => error(404, "itemNotFound"),
        };
    }
    if let Some(sid) = path.strip_prefix("/upload/") {
        return upload_chunk(s, method, sid, headers, body);
    }

    let Some(rest) = path.strip_prefix(API) else {
        return error(404, "invalidRequest");
    };
    if !s.tokens.authorized(headers) {
        return error(401, "InvalidAuthenticationToken");
    }
    let Some((item, tail)) = parse_item_path(rest) else {
        return error(400, "invalidRequest");
    };
    let fail_on_conflict = query.contains("conflictBehavior=fail");
    match (method.as_str(), tail.as_str()) {
        ("GET", "") => match s.items.contains_key(&item) {
            true => respond(200, s.item_json(&item)),
            false => error(404, "itemNotFound"),
        },
        ("GET", "/content") => match s.items.get(&item) {
            Some(i) if !i.dir => Response::builder()
                .status(302)
                .header("Location", format!("{}/download/{}", s.base_url, i.id))
                .body(Body::empty())
                .unwrap(),
            _ => error(404, "itemNotFound"),
        },
        ("PUT", "/content") => {
            if !s.items.get(parent(&item)).is_some_and(|p| p.dir) {
                return error(404, "itemNotFound");
            }
            match s.items.get(&item) {
                Some(i) if i.dir => return error(409, "nameAlreadyExists"),
                Some(_) if fail_on_conflict => return error(409, "nameAlreadyExists"),
                None if headers.contains_key("If-Match") => return error(404, "itemNotFound"),
                _ => {}
            }
            let status = s.write_file(&item, body);
            respond(status, s.item_json(&item))
        }
        ("DELETE", "") => {
            if !s.items.contains_key(&item) {
                return error(404, "itemNotFound");
            }
            s.remove_tree(&item);
            empty(204)
        }
        ("GET", "/children") => {
            if !s.items.get(&item).is_some_and(|i| i.dir) {
                return error(404, "itemNotFound");
            }
            let skip: usize = query
                .split('&')
                .find_map(|p| p.strip_prefix("$skiptoken="))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let children = s.children(&item);
            let page: Vec<_> = children
                .iter()
                .skip(skip)
                .take(s.page_size)
                .map(|p| s.item_json(p))
                .collect();
            let mut v = serde_json::json!({ "value": page });
            if skip + s.page_size < children.len() {
                let rest_url = if item.is_empty() {
                    format!("{}{API}/root/children", s.base_url)
                } else {
                    format!("{}{API}/root:/{item}:/children", s.base_url)
                };
                v["@odata.nextLink"] = format!(
                    "{rest_url}?$top={}&$skiptoken={}",
                    s.page_size,
                    skip + s.page_size
                )
                .into();
            }
            respond(200, v)
        }
        ("POST", "/children") => {
            if !s.items.get(&item).is_some_and(|i| i.dir) {
                return error(404, "itemNotFound");
            }
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let Some(child_name) = req["name"].as_str() else {
                return error(400, "invalidRequest");
            };
            let child = if item.is_empty() {
                child_name.to_string()
            } else {
                format!("{item}/{child_name}")
            };
            if s.items.contains_key(&child) {
                return error(409, "nameAlreadyExists");
            }
            let id = s.new_id();
            s.items.insert(
                child.clone(),
                Item {
                    id,
                    dir: true,
                    data: vec![],
                    version: 1,
                    modified: now_secs(),
                },
            );
            respond(201, s.item_json(&child))
        }
        ("POST", "/createUploadSession") => {
            if !s.items.get(parent(&item)).is_some_and(|p| p.dir) {
                return error(404, "itemNotFound");
            }
            let sid = s.new_id();
            s.sessions.insert(
                sid.clone(),
                Session {
                    path: item,
                    total: 0,
                    data: vec![],
                },
            );
            respond(
                200,
                serde_json::json!({ "uploadUrl": format!("{}/upload/{sid}", s.base_url) }),
            )
        }
        _ => error(400, "invalidRequest"),
    }
}

const CHUNK_UNIT: usize = 320 * 1024;

fn upload_chunk(
    s: &mut ServerState,
    method: &Method,
    sid: &str,
    headers: &HeaderMap,
    body: Vec<u8>,
) -> Response {
    if *method == Method::DELETE {
        s.sessions.remove(sid);
        return empty(204);
    }
    if *method == Method::GET {
        return match s.sessions.get(sid) {
            Some(session) => respond(
                200,
                serde_json::json!({ "nextExpectedRanges": [format!("{}-", session.data.len())] }),
            ),
            None => error(404, "itemNotFound"),
        };
    }
    if headers.contains_key("Authorization") {
        // Upload URLs are pre-authenticated; the service rejects bearer tokens.
        return error(401, "unauthenticated");
    }
    let Some(range) = headers
        .get("Content-Range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
    else {
        return error(400, "invalidRange");
    };
    let (span, total) = range.split_once('/').unwrap();
    let (start, end) = span.split_once('-').unwrap();
    let (start, end, total): (u64, u64, u64) = (
        start.parse().unwrap(),
        end.parse().unwrap(),
        total.parse().unwrap(),
    );
    let Some(session) = s.sessions.get_mut(sid) else {
        return error(404, "itemNotFound");
    };
    if start != session.data.len() as u64 || end + 1 - start != body.len() as u64 {
        return error(416, "invalidRange");
    }
    let last = end + 1 == total;
    if !last && !body.len().is_multiple_of(CHUNK_UNIT) {
        return error(400, "invalidChunkSize");
    }
    session.total = total;
    session.data.extend_from_slice(&body);
    if !last {
        return respond(
            202,
            serde_json::json!({ "nextExpectedRanges": [format!("{}-", end + 1)] }),
        );
    }
    let session = s.sessions.remove(sid).unwrap();
    assert_eq!(session.data.len() as u64, session.total);
    let status = s.write_file(&session.path, session.data);
    respond(status, s.item_json(&session.path))
}
