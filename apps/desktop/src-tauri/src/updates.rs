//! Updates. On desktop the updater plugin installs new versions (its feed is
//! the `latest.json` of the GitHub release, see `tauri.conf.json`); this
//! module serves what the plugin does not: the release notes page, and on
//! Android (which the plugin does not support) the newest release's APK.

use serde::{Deserialize, Serialize};

use crate::notes::{CmdResult, err};

/// Where the releases are published.
pub const REPOSITORY: &str = "nuyu3n/nisshoku";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// Without the leading `v`.
    pub version: String,
    pub notes: String,
    /// The release's page.
    pub url: String,
    /// The Android package, if the release has one.
    pub apk_url: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// The newest published release (drafts and pre-releases are not listed by
/// GitHub's `latest`).
#[tauri::command]
pub async fn latest_release() -> CmdResult<Release> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("Nisshoku/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(err)?;
    let resp = client
        .get(format!("https://api.github.com/repos/{REPOSITORY}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(err)?;
    if resp.status().as_u16() == 404 {
        return Err(crate::i18n::tr("公開されている版はまだありません", "No version has been published yet").into());
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub: HTTP {}", resp.status()));
    }
    let release: GithubRelease = resp.json().await.map_err(err)?;
    Ok(to_release(release))
}

fn to_release(r: GithubRelease) -> Release {
    let apk_url = r
        .assets
        .iter()
        .find(|a| a.name.ends_with(".apk"))
        .map(|a| a.browser_download_url.clone());
    Release {
        version: r.tag_name.trim_start_matches('v').to_string(),
        notes: r.body.unwrap_or_default(),
        url: r.html_url,
        apk_url,
        published_at: r.published_at,
    }
}

/// How this copy of the app was installed, which decides how it updates:
/// `nsis`, `msi`, `appimage`, `deb`, `rpm`, `android`, or `dev` for a build
/// that is not an installed package.
#[tauri::command]
pub fn install_kind() -> &'static str {
    if cfg!(target_os = "android") {
        return "android";
    }
    if cfg!(debug_assertions) {
        return "dev";
    }
    match tauri::utils::platform::bundle_type() {
        Some(tauri::utils::config::BundleType::Nsis) => "nsis",
        Some(tauri::utils::config::BundleType::Msi) => "msi",
        Some(tauri::utils::config::BundleType::AppImage) => "appimage",
        Some(tauri::utils::config::BundleType::Deb) => "deb",
        Some(tauri::utils::config::BundleType::Rpm) => "rpm",
        _ => "dev",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_apk_and_strips_the_v() {
        let r: GithubRelease = serde_json::from_str(
            r#"{"tag_name":"v0.2.0","body":"notes","html_url":"https://github.com/x/y/releases/tag/v0.2.0",
                "assets":[{"name":"latest.json","browser_download_url":"https://e/latest.json"},
                          {"name":"nisshoku_0.2.0.apk","browser_download_url":"https://e/a.apk"}]}"#,
        )
        .unwrap();
        let r = to_release(r);
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.apk_url.as_deref(), Some("https://e/a.apk"));
    }
}
