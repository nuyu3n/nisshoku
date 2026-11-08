//! OAuth 2.0 for the cloud drives.
//!
//! - [`BrowserLogin`]: sign-in through the system browser, with the
//!   authorization code returned to a loopback address (RFC 8252) and
//!   protected by PKCE (RFC 7636). No client secret is needed where the
//!   provider supports public clients.
//! - [`TokenSource`]: hands out access tokens, refreshing them before they
//!   expire or when the server rejects one, and reports new tokens so that
//!   the app can store rotated refresh tokens.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{Result, SyncError};
use nisshoku_core::time::now_ms;

/// An OAuth provider and the app's registration with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    /// Google requires one even for desktop apps (where it is not secret).
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    /// Host of the loopback redirect URI (`127.0.0.1` or `localhost`, as the
    /// provider's app registration expects).
    pub redirect_host: String,
    /// Extra parameters for the authorization request.
    pub extra_auth_params: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    /// When the access token expires, Unix milliseconds.
    pub expires_at: i64,
}

/// Called with the new tokens after every refresh.
pub type TokenListener = Arc<dyn Fn(&OAuthTokens) + Send + Sync>;

fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("system random number generator");
    URL_SAFE_NO_PAD.encode(buf)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

/// How long a token request keeps retrying network failures and server
/// errors.
#[derive(Clone, Copy)]
enum Patience {
    /// A few quick attempts (token refresh during a sync).
    Short,
    /// Up to this long, for the code exchange after a browser sign-in. On
    /// Android the app is in the background while the browser is in front,
    /// and the system may block its network access until the user switches
    /// back, so failures are retried until then.
    Until(std::time::Duration),
}

/// Posts a token request, retrying network failures and server errors.
async fn token_request(
    client: &reqwest::Client,
    provider: &OAuthProvider,
    mut form: Vec<(&str, String)>,
    patience: Patience,
) -> Result<TokenResponse> {
    let started = std::time::Instant::now();
    form.push(("client_id", provider.client_id.clone()));
    if let Some(secret) = &provider.client_secret {
        form.push(("client_secret", secret.clone()));
    }
    let body = form
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let mut attempt = 0;
    loop {
        attempt += 1;
        let result = client
            .post(&provider.token_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.clone())
            .send()
            .await;
        let retry = match result {
            Ok(resp) if resp.status().is_success() => {
                return resp
                    .json::<TokenResponse>()
                    .await
                    .map_err(|e| SyncError::Remote(format!("invalid token response: {e}")));
            }
            Ok(resp) if resp.status().is_server_error() || resp.status().as_u16() == 429 => {
                format!("HTTP {}", resp.status())
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(SyncError::Auth(format!(
                    "the sign-in is no longer valid; sign in again (HTTP {status}: {})",
                    text.chars().take(300).collect::<String>()
                )));
            }
            Err(e) => e.to_string(),
        };
        let give_up = match patience {
            Patience::Short => attempt >= 4,
            Patience::Until(limit) => started.elapsed() >= limit,
        };
        if give_up {
            return Err(SyncError::Remote(format!("token request failed: {retry}")));
        }
        let delay = (300 * attempt).min(3000);
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
}

fn tokens_from(resp: TokenResponse, previous_refresh: &str) -> OAuthTokens {
    OAuthTokens {
        access_token: resp.access_token,
        refresh_token: resp
            .refresh_token
            .unwrap_or_else(|| previous_refresh.to_string()),
        expires_at: now_ms() + resp.expires_in.unwrap_or(3600) * 1000,
    }
}

// -------------------------------------------------------------------------
// Access tokens

pub struct TokenSource {
    provider: OAuthProvider,
    client: reqwest::Client,
    tokens: tokio::sync::Mutex<OAuthTokens>,
    listener: Option<TokenListener>,
}

/// Tokens are refreshed when they have less than this left.
const EXPIRY_MARGIN_MS: i64 = 60_000;

impl TokenSource {
    pub fn new(
        provider: OAuthProvider,
        tokens: OAuthTokens,
        listener: Option<TokenListener>,
    ) -> Result<Self> {
        Ok(Self {
            provider,
            client: crate::drivers::http::client()?,
            tokens: tokio::sync::Mutex::new(tokens),
            listener,
        })
    }

    pub async fn tokens(&self) -> OAuthTokens {
        self.tokens.lock().await.clone()
    }

    /// A valid access token, refreshed first if it is about to expire.
    pub async fn access_token(&self) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        if tokens.access_token.is_empty() || tokens.expires_at - now_ms() < EXPIRY_MARGIN_MS {
            self.refresh(&mut tokens).await?;
        }
        Ok(tokens.access_token.clone())
    }

    /// Called when the server rejected `rejected`: refreshes, unless another
    /// request already did.
    pub async fn refresh_rejected(&self, rejected: &str) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        if tokens.access_token == rejected {
            self.refresh(&mut tokens).await?;
        }
        Ok(tokens.access_token.clone())
    }

    async fn refresh(&self, tokens: &mut OAuthTokens) -> Result<()> {
        if tokens.refresh_token.is_empty() {
            return Err(SyncError::Auth("not signed in".into()));
        }
        let resp = token_request(
            &self.client,
            &self.provider,
            vec![
                ("grant_type", "refresh_token".into()),
                ("refresh_token", tokens.refresh_token.clone()),
                ("scope", self.provider.scopes.join(" ")),
            ],
            Patience::Short,
        )
        .await?;
        *tokens = tokens_from(resp, &tokens.refresh_token);
        if let Some(listener) = &self.listener {
            listener(tokens);
        }
        Ok(())
    }
}

// -------------------------------------------------------------------------
// Browser sign-in

/// A sign-in in progress: open [`BrowserLogin::url`] in a browser, then await
/// [`BrowserLogin::finish`].
pub struct BrowserLogin {
    listener: tokio::net::TcpListener,
    provider: OAuthProvider,
    redirect_uri: String,
    verifier: String,
    state: String,
    url: String,
    return_link: Option<String>,
}

/// How long the code exchange waits for the network after the browser came
/// back (Google's authorization codes are valid for about 10 minutes).
const EXCHANGE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(9 * 60);

impl BrowserLogin {
    pub async fn start(provider: OAuthProvider) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let redirect_uri = format!("http://{}:{port}/", provider.redirect_host);
        let verifier = random_token(48);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_token(16);
        let mut params = vec![
            ("client_id".to_string(), provider.client_id.clone()),
            ("redirect_uri".into(), redirect_uri.clone()),
            ("response_type".into(), "code".into()),
            ("scope".into(), provider.scopes.join(" ")),
            ("state".into(), state.clone()),
            ("code_challenge".into(), challenge),
            ("code_challenge_method".into(), "S256".into()),
        ];
        params.extend(provider.extra_auth_params.iter().cloned());
        let query = params
            .iter()
            .map(|(k, v)| format!("{k}={}", encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let separator = if provider.auth_url.contains('?') {
            '&'
        } else {
            '?'
        };
        let url = format!("{}{separator}{query}", provider.auth_url);
        Ok(Self {
            listener,
            provider,
            redirect_uri,
            verifier,
            state,
            url,
            return_link: None,
        })
    }

    /// A link shown on the page the browser lands on, for going back to the
    /// app (on Android, an `intent:` URL that brings the app to the front).
    pub fn with_return_link(mut self, link: impl Into<String>) -> Self {
        self.return_link = Some(link.into());
        self
    }

    /// The page to open in the browser.
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// Waits for the browser to come back with the authorization code and
    /// exchanges it for tokens.
    ///
    /// The browser is answered as soon as the code arrives, before the
    /// exchange: on Android the app cannot reach the network while it is in
    /// the background, so the page asks the user to switch back, and the
    /// exchange is retried until they do.
    pub async fn finish(self) -> Result<OAuthTokens> {
        let back = self.return_link.as_deref();
        loop {
            let (mut stream, _) = self.listener.accept().await?;
            let Some(target) = read_request_target(&mut stream).await else {
                continue;
            };
            let Some(query) = target.strip_prefix("/?") else {
                let _ = respond(&mut stream, "404 Not Found", "").await;
                continue;
            };
            let params = parse_query(query);
            let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
            if get("state").as_deref() != Some(self.state.as_str()) {
                let _ = respond(&mut stream, "400 Bad Request", "Invalid sign-in response.").await;
                continue;
            }
            if let Some(error) = get("error") {
                let detail = get("error_description").unwrap_or_default();
                let message = format!("{error} {detail}");
                let _ = respond(&mut stream, "200 OK", &page(false, Some(&message), back)).await;
                return Err(SyncError::Auth(format!("sign-in failed: {}", message.trim())));
            }
            let Some(code) = get("code") else {
                let _ = respond(&mut stream, "400 Bad Request", "Missing code.").await;
                continue;
            };
            let _ = respond(&mut stream, "200 OK", &page(true, None, back)).await;
            drop(stream);
            let result = token_request(
                &crate::drivers::http::client()?,
                &self.provider,
                vec![
                    ("grant_type", "authorization_code".into()),
                    ("code", code),
                    ("redirect_uri", self.redirect_uri.clone()),
                    ("code_verifier", self.verifier.clone()),
                ],
                Patience::Until(EXCHANGE_PATIENCE),
            )
            .await;
            return result.map(|r| tokens_from(r, ""));
        }
    }
}

/// The page the browser lands on after the sign-in.
fn page(ok: bool, error: Option<&str>, back: Option<&str>) -> String {
    let message = if ok {
        "ログイン情報を受け取りました。日食に戻ってください（このタブは閉じてかまいません）。<br>\
         Signed in. Please return to Nisshoku (you can close this tab)."
    } else {
        "ログインできませんでした。日食に戻ってやり直してください。<br>\
         Could not sign in. Please return to Nisshoku and try again."
    };
    let detail = error
        .map(|e| format!("<p style=\"color:#888;font-size:.9em\">{}</p>", escape_html(e.trim())))
        .unwrap_or_default();
    let button = back
        .map(|link| {
            format!(
                "<p><a href=\"{}\" style=\"display:inline-block;padding:.7em 1.4em;border-radius:8px;\
                 background:#d97757;color:#fff;text-decoration:none\">日食に戻る / Back to Nisshoku</a></p>",
                escape_html(link)
            )
        })
        .unwrap_or_default();
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\">\
         <title>Nisshoku</title><body style=\"font-family:sans-serif;padding:1em;line-height:1.6\">\
         <p>{message}</p>{detail}{button}</body>"
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Reads an HTTP request head and returns its target (path and query).
async fn read_request_target(stream: &mut tokio::net::TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = tokio::time::timeout(std::time::Duration::from_secs(10), stream.read(&mut chunk))
            .await
            .ok()?
            .ok()?;
        if n == 0 || buf.len() > 16 * 1024 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf);
    let mut parts = head.lines().next()?.split(' ');
    (parts.next()? == "GET").then_some(())?;
    parts.next().map(str::to_string)
}

async fn respond(
    stream: &mut tokio::net::TcpStream,
    status: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let decode = |s: &str| {
                percent_decode_str(&s.replace('+', " "))
                    .decode_utf8_lossy()
                    .into_owned()
            };
            (!k.is_empty()).then(|| (decode(k), decode(v)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_redirect_queries() {
        assert_eq!(
            parse_query("code=4%2F0Ab+c&state=xyz&scope="),
            vec![
                ("code".into(), "4/0Ab c".into()),
                ("state".into(), "xyz".into()),
                ("scope".into(), "".into())
            ]
        );
    }

    #[tokio::test]
    async fn authorization_url_has_pkce() {
        let provider = OAuthProvider {
            auth_url: "https://login.example.com/authorize".into(),
            token_url: "https://login.example.com/token".into(),
            client_id: "app id".into(),
            client_secret: None,
            scopes: vec!["files".into(), "offline_access".into()],
            redirect_host: "localhost".into(),
            extra_auth_params: vec![("prompt".into(), "consent".into())],
        };
        let login = BrowserLogin::start(provider).await.unwrap();
        let url = login.url();
        assert!(url.starts_with("https://login.example.com/authorize?client_id=app%20id&"));
        assert!(url.contains("scope=files%20offline%5Faccess"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.ends_with("&prompt=consent"));
        assert!(login.redirect_uri().starts_with("http://localhost:"));
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(login.verifier.as_bytes()));
        assert!(url.contains(&format!("code_challenge={}", encode(&challenge))));
    }
}
