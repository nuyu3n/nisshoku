//! HTTP plumbing shared by the cloud drive drivers: a client, and requests
//! with OAuth bearer tokens and retries.

use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use reqwest::{Method, RequestBuilder, Response};

use crate::drivers::oauth::TokenSource;
use crate::error::{Result, SyncError};

pub(crate) fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("Nisshoku/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| SyncError::Remote(e.to_string()))
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    /// Upper bound for a server-requested delay (`Retry-After`).
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 6,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(60),
        }
    }
}

/// Rate limiting reported as `403` (Google) is recognized from the body.
fn is_rate_limit_body(body: &str) -> bool {
    body.contains("rateLimitExceeded") || body.contains("userRateLimitExceeded")
}

fn retry_after(resp: &Response) -> Option<Duration> {
    let value = resp.headers().get("Retry-After")?.to_str().ok()?;
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Sends a request built by `build` (called once per attempt), with a bearer
/// token when `auth` is given.
///
/// - `401` refreshes the token once, then fails with [`SyncError::Auth`];
/// - `429`, `5xx` and rate-limit `403`s are retried with backoff (or after
///   `Retry-After`), as are connection failures;
/// - any other response is returned as is. After the last attempt the
///   transient response is returned too, for the caller to report.
pub(crate) async fn send(
    client: &reqwest::Client,
    auth: Option<&TokenSource>,
    retry: &RetryPolicy,
    service: &str,
    method: &Method,
    url: &str,
    build: impl Fn(RequestBuilder) -> RequestBuilder,
) -> Result<Response> {
    let mut attempt = 0;
    let mut refreshed = false;
    let mut token = match auth {
        Some(a) => Some(a.access_token().await?),
        None => None,
    };
    loop {
        attempt += 1;
        let mut request = client.request(method.clone(), url);
        if let Some(t) = &token {
            request = request.bearer_auth(t);
        }
        let (failure, wait) = match build(request).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if status == 401
                    && let (Some(a), Some(t)) = (auth, &token)
                {
                    if refreshed {
                        return Err(SyncError::Auth(format!(
                            "{service} rejected the sign-in; sign in again"
                        )));
                    }
                    token = Some(a.refresh_rejected(t).await?);
                    refreshed = true;
                    attempt -= 1;
                    continue;
                }
                let wait = retry_after(&resp);
                if status == 403 {
                    let body = resp.text().await.unwrap_or_default();
                    if !is_rate_limit_body(&body) {
                        return Err(SyncError::Remote(format!(
                            "{service}: {method} {url} failed with HTTP 403: {}",
                            snippet(&body)
                        )));
                    }
                    if attempt >= retry.max_attempts {
                        return Err(SyncError::Remote(format!(
                            "{service}: rate limit exceeded ({method} {url})"
                        )));
                    }
                    ("HTTP 403 rate limit".to_string(), wait)
                } else if matches!(status, 429 | 500 | 502 | 503 | 504 | 509)
                    && attempt < retry.max_attempts
                {
                    (format!("HTTP {status}"), wait)
                } else {
                    return Ok(resp);
                }
            }
            Err(e) => {
                let transient = e.is_connect() || e.is_timeout() || e.is_request() || e.is_body();
                if !transient || attempt >= retry.max_attempts {
                    return Err(SyncError::Remote(format!("{service}: {method} {url}: {e}")));
                }
                (e.to_string(), None)
            }
        };
        let backoff = retry.initial_delay * 2u32.pow((attempt - 1).min(10));
        let delay = wait.unwrap_or(backoff).min(retry.max_delay);
        tracing::debug!("{method} {url} failed ({failure}), retrying in {delay:?}");
        tokio::time::sleep(delay).await;
    }
}

fn snippet(body: &str) -> String {
    body.chars().take(400).collect()
}

/// An error for an unexpected response, with the start of its body.
pub(crate) async fn unexpected(
    service: &str,
    method: &Method,
    url: &str,
    resp: Response,
) -> SyncError {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    SyncError::Remote(format!(
        "{service}: {method} {url} failed with HTTP {status}: {}",
        snippet(&body)
    ))
}

/// Parses an RFC 3339 timestamp into Unix milliseconds.
pub(crate) fn parse_time(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.timestamp_millis())
}

/// Streams a response body into `dest` through a temporary file.
pub(crate) async fn write_stream(resp: Response, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(crate::storage::temp_suffix());
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();
    let result = async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| SyncError::Remote(format!("download failed: {e}")))?;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok::<_, SyncError>(())
    }
    .await;
    drop(file);
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    crate::storage::retry_sharing_violation(|| tokio::fs::rename(&tmp, dest)).await?;
    Ok(())
}
