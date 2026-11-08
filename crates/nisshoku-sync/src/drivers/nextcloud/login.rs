//! Nextcloud Login Flow v2: the user signs in in their browser and the app
//! receives an app password, so the account password is never entered here.
//!
//! 1. [`start_login_flow`] returns a URL to open in the browser.
//! 2. [`poll_login_flow`] is called every few seconds until it returns the
//!    credentials (the user approved) or the flow expires (after 20 minutes).

use serde::{Deserialize, Serialize};

use super::{base_url, http_client};
use crate::error::{Result, SyncError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginFlow {
    /// Page to open in the user's browser.
    pub login_url: String,
    pub poll_endpoint: String,
    pub poll_token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginCredentials {
    pub server: String,
    pub login_name: String,
    pub app_password: String,
}

#[derive(Deserialize)]
struct StartResponse {
    poll: Poll,
    login: String,
}

#[derive(Deserialize)]
struct Poll {
    token: String,
    endpoint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PollResponse {
    server: String,
    login_name: String,
    app_password: String,
}

fn remote_err(e: reqwest::Error) -> SyncError {
    SyncError::Remote(e.to_string())
}

pub async fn start_login_flow(server_url: &str) -> Result<LoginFlow> {
    let url = format!("{}/index.php/login/v2", base_url(server_url)?);
    let resp = http_client()?.post(&url).send().await.map_err(remote_err)?;
    if !resp.status().is_success() {
        return Err(SyncError::Remote(format!(
            "could not start Nextcloud login (HTTP {}); is {server_url} a Nextcloud server?",
            resp.status()
        )));
    }
    let body: StartResponse = resp.json().await.map_err(remote_err)?;
    Ok(LoginFlow {
        login_url: body.login,
        poll_endpoint: body.poll.endpoint,
        poll_token: body.poll.token,
    })
}

/// Returns `None` while the user has not approved the login yet.
pub async fn poll_login_flow(flow: &LoginFlow) -> Result<Option<LoginCredentials>> {
    let body = format!(
        "token={}",
        percent_encoding::utf8_percent_encode(&flow.poll_token, percent_encoding::NON_ALPHANUMERIC)
    );
    let resp = http_client()?
        .post(&flow.poll_endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(remote_err)?;
    match resp.status().as_u16() {
        404 => Ok(None),
        200 => {
            let r: PollResponse = resp.json().await.map_err(remote_err)?;
            Ok(Some(LoginCredentials {
                server: r.server,
                login_name: r.login_name,
                app_password: r.app_password,
            }))
        }
        s => Err(SyncError::Remote(format!("login polling failed: HTTP {s}"))),
    }
}

#[derive(Deserialize)]
struct OcsUser {
    ocs: OcsUserInner,
}

#[derive(Deserialize)]
struct OcsUserInner {
    data: OcsUserData,
}

#[derive(Deserialize)]
struct OcsUserData {
    id: String,
}

/// The user ID used in WebDAV paths. It can differ from the login name, for
/// example when users sign in with their email address.
pub async fn fetch_user_id(server_url: &str, login_name: &str, password: &str) -> Result<String> {
    let url = format!(
        "{}/ocs/v2.php/cloud/user?format=json",
        base_url(server_url)?
    );
    let resp = http_client()?
        .get(&url)
        .basic_auth(login_name, Some(password))
        .header("OCS-APIRequest", "true")
        .send()
        .await
        .map_err(remote_err)?;
    match resp.status().as_u16() {
        200 => {
            let user: OcsUser = resp.json().await.map_err(remote_err)?;
            Ok(user.ocs.data.id)
        }
        401 | 403 => Err(SyncError::Auth(
            "Nextcloud rejected the user name or password".into(),
        )),
        s => Err(SyncError::Remote(format!(
            "could not read the Nextcloud user (HTTP {s}); is {server_url} a Nextcloud server?"
        ))),
    }
}
