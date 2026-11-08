//! Token issuing for the mock cloud drives: a token endpoint (refresh and
//! authorization-code grants) and bearer token checks, with expiry that tests
//! can force.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::HeaderMap;
use axum::response::Response;
use nisshoku_sync::drivers::OAuthTokens;
use percent_encoding::percent_decode_str;

pub const TOKEN_PATH: &str = "/oauth/token";
pub const AUTH_PATH: &str = "/oauth/authorize";
pub const AUTH_CODE: &str = "auth-code-1";

#[derive(Default)]
pub struct TokenIssuer {
    next: u64,
    access: HashMap<String, Instant>,
    refresh: HashSet<String>,
    pub refreshes: usize,
    /// Lifetime of new access tokens.
    pub lifetime: Option<Duration>,
    /// PKCE challenge of the pending authorization (`code_challenge`).
    pub challenge: Option<String>,
}

pub fn parse_form(body: &[u8]) -> HashMap<String, String> {
    String::from_utf8_lossy(body)
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| {
            let d = |s: &str| {
                percent_decode_str(&s.replace('+', " "))
                    .decode_utf8_lossy()
                    .into_owned()
            };
            (d(k), d(v))
        })
        .collect()
}

fn json(status: u16, value: serde_json::Value) -> Response {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap()
}

impl TokenIssuer {
    /// Tokens for a new client: a valid refresh token and no access token.
    pub fn initial(&mut self) -> OAuthTokens {
        self.next += 1;
        let refresh = format!("rt-{}", self.next);
        self.refresh.insert(refresh.clone());
        OAuthTokens {
            access_token: String::new(),
            refresh_token: refresh,
            expires_at: 0,
        }
    }

    pub fn expire_all(&mut self) {
        self.access.clear();
    }

    pub fn revoke_refresh_tokens(&mut self) {
        self.refresh.clear();
    }

    pub fn authorized(&self, headers: &HeaderMap) -> bool {
        let Some(token) = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        else {
            return false;
        };
        self.access
            .get(token)
            .is_some_and(|exp| *exp > Instant::now())
    }

    fn issue(&mut self) -> serde_json::Value {
        self.next += 1;
        let access = format!("at-{}", self.next);
        let refresh = format!("rt-{}", self.next);
        let lifetime = self.lifetime.unwrap_or(Duration::from_secs(3600));
        self.access
            .insert(access.clone(), Instant::now() + lifetime);
        self.refresh.insert(refresh.clone());
        serde_json::json!({
            "access_token": access,
            "refresh_token": refresh,
            "expires_in": lifetime.as_secs().max(1),
            "token_type": "Bearer",
        })
    }

    /// Handles a token endpoint request.
    pub fn token_endpoint(&mut self, body: &[u8], client_id: &str) -> Response {
        let form = parse_form(body);
        if form.get("client_id").map(String::as_str) != Some(client_id) {
            return json(401, serde_json::json!({ "error": "invalid_client" }));
        }
        match form.get("grant_type").map(String::as_str) {
            Some("refresh_token") => {
                let token = form.get("refresh_token").cloned().unwrap_or_default();
                if !self.refresh.contains(&token) {
                    return json(400, serde_json::json!({ "error": "invalid_grant" }));
                }
                self.refreshes += 1;
                json(200, self.issue())
            }
            Some("authorization_code") => {
                use base64::Engine;
                use sha2::Digest;
                let verifier = form.get("code_verifier").cloned().unwrap_or_default();
                let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(sha2::Sha256::digest(verifier.as_bytes()));
                if form.get("code").map(String::as_str) != Some(AUTH_CODE)
                    || self.challenge.as_deref() != Some(challenge.as_str())
                {
                    return json(400, serde_json::json!({ "error": "invalid_grant" }));
                }
                self.challenge = None;
                json(200, self.issue())
            }
            _ => json(
                400,
                serde_json::json!({ "error": "unsupported_grant_type" }),
            ),
        }
    }

    /// The authorization page: remembers the PKCE challenge and redirects
    /// back with a code, as if the user had signed in.
    pub fn authorize(&mut self, query: &str) -> Response {
        let params = parse_form(query.as_bytes());
        self.challenge = params.get("code_challenge").cloned();
        let redirect = params.get("redirect_uri").cloned().unwrap_or_default();
        let state = params.get("state").cloned().unwrap_or_default();
        Response::builder()
            .status(302)
            .header(
                "Location",
                format!("{redirect}?code={AUTH_CODE}&state={state}"),
            )
            .body(Body::empty())
            .unwrap()
    }
}
