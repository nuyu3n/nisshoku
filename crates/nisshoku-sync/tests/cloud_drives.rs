//! Tests specific to the OneDrive and Google Drive drivers: sign-in, token
//! refresh, throttling, paging, chunked uploads, and Google Drive's duplicate
//! names.

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use nisshoku_sync::drivers::google_drive::google_drive_oauth;
use nisshoku_sync::drivers::onedrive::onedrive_oauth;
use nisshoku_sync::drivers::{
    BrowserLogin, GoogleDriveStorage, OAuthProvider, OAuthTokens, OneDriveStorage,
};
use nisshoku_sync::{Precondition, RemoteStorage, SyncError};

async fn onedrive() -> (Backend, MockOneDrive) {
    let b = Backend::new(Kind::MockOneDrive).await.unwrap();
    let mock = b.onedrive_mock.clone().unwrap();
    (b, mock)
}

async fn gdrive() -> (Backend, MockGoogleDrive) {
    let b = Backend::new(Kind::MockGoogleDrive).await.unwrap();
    let mock = b.gdrive_mock.clone().unwrap();
    (b, mock)
}

/// Runs a browser sign-in against a mock: the "browser" follows the
/// authorization redirect back to the loopback listener.
async fn sign_in(provider: OAuthProvider) -> OAuthTokens {
    let login = BrowserLogin::start(provider).await.unwrap();
    let url = login.url().to_string();
    let redirect = login.redirect_uri().to_string();
    let browser = tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        // Favicon requests and other noise must not disturb the sign-in.
        let _ = client.get(format!("{redirect}favicon.ico")).send().await;
        let resp = client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), 302);
        let back = resp.headers()["Location"].to_str().unwrap().to_string();
        assert!(back.starts_with(&redirect), "{back}");
        let back = back.replace("localhost", "127.0.0.1");
        client.get(back).send().await.unwrap().text().await.unwrap()
    });
    let tokens = tokio::time::timeout(std::time::Duration::from_secs(10), login.finish())
        .await
        .unwrap()
        .unwrap();
    assert!(browser.await.unwrap().contains("ログイン情報を受け取りました"));
    tokens
}

/// On Android the app has no network while the browser is in front: the
/// browser must get its page at once, and the code exchange must wait until
/// the token endpoint becomes reachable instead of failing after a few tries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_waits_for_the_network() {
    let (_b, mock) = gdrive().await;
    let mut provider = google_drive_oauth(
        mock_google_drive::CLIENT_ID,
        mock_google_drive::CLIENT_SECRET,
        &mock.endpoints(),
    );
    // The token endpoint is unreachable at first: nothing listens on `port`.
    let real: reqwest::Url = provider.token_url.parse().unwrap();
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let mut blocked = real.clone();
    blocked.set_port(Some(port)).unwrap();
    provider.token_url = blocked.to_string();

    let login = BrowserLogin::start(provider).await.unwrap();
    let url = login.url().to_string();
    let redirect = login.redirect_uri().to_string();
    let finish = tokio::spawn(login.finish());
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client.get(&url).send().await.unwrap();
    let back = resp.headers()["Location"].to_str().unwrap().to_string();
    assert!(back.starts_with(&redirect), "{back}");
    let page = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        async { client.get(back).send().await.unwrap().text().await.unwrap() },
    )
    .await
    .expect("the browser is answered before the code exchange");
    assert!(page.contains("ログイン情報を受け取りました"), "{page}");

    // The user switches back a while later: the network comes back.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert!(!finish.is_finished(), "the exchange kept waiting");
    let proxy = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let target = format!("{}:{}", real.host_str().unwrap(), real.port().unwrap());
    tokio::spawn(async move {
        while let Ok((mut inbound, _)) = proxy.accept().await {
            let target = target.clone();
            tokio::spawn(async move {
                let mut outbound = tokio::net::TcpStream::connect(target).await.unwrap();
                let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
            });
        }
    });
    let tokens = tokio::time::timeout(std::time::Duration::from_secs(10), finish)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!tokens.refresh_token.is_empty());
}

// -------------------------------------------------------------------------
// OneDrive

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn onedrive_sign_in() {
    let (_b, mock) = onedrive().await;
    let provider = onedrive_oauth(mock_onedrive::CLIENT_ID, &mock.endpoints());
    let tokens = sign_in(provider).await;
    assert!(tokens.access_token.starts_with("at-"));
    assert!(tokens.refresh_token.starts_with("rt-"));

    let config = nisshoku_sync::drivers::OneDriveConfig {
        client_id: mock_onedrive::CLIENT_ID.into(),
        folder: "Nisshoku".into(),
        tokens,
        endpoints: mock.endpoints(),
    };
    let s = OneDriveStorage::connect(&config, None).await.unwrap();
    s.put("a.md", b"hello").await.unwrap();
    assert_eq!(mock.file("Nisshoku/a.md").unwrap(), b"hello");
    assert_eq!(mock.token_refreshes(), 0, "the signed-in token was used");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn onedrive_refreshes_expired_tokens() {
    let (_b, mock) = onedrive().await;
    let seen: Arc<Mutex<Vec<OAuthTokens>>> = Arc::default();
    let config = mock.config("Nisshoku");
    let listener = {
        let seen = seen.clone();
        Arc::new(move |t: &OAuthTokens| seen.lock().unwrap().push(t.clone()))
    };
    let s = OneDriveStorage::connect(&config, Some(listener))
        .await
        .unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1, "first access token");

    // The server rejects the token before its announced expiry.
    mock.expire_tokens();
    s.put("a.md", b"1").await.unwrap();
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(s.tokens().await, seen.lock().unwrap()[1]);

    // A revoked sign-in is an authentication error, not a retryable one.
    mock.expire_tokens();
    mock.revoke_refresh_tokens();
    assert!(matches!(s.get("a.md").await, Err(SyncError::Auth(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn onedrive_throttling_and_errors_are_retried() {
    let (b, mock) = onedrive().await;
    let s = b.onedrive_storage().await;
    mock.throttle_next(2);
    s.put("a.md", b"1").await.unwrap();
    mock.fail_when("PUT", "/b.md", 2);
    s.put("b.md", b"2").await.unwrap();
    assert_eq!(s.get("b.md").await.unwrap().unwrap(), b"2");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn onedrive_listings_are_paged() {
    let (b, mock) = onedrive().await;
    let s = b.onedrive_storage().await;
    mock.set_page_size(3);
    for i in 0..10 {
        s.put(&format!("{i:032x}.md"), b"x").await.unwrap();
    }
    s.mkdir("locks").await.unwrap();
    let listing = s.list("").await.unwrap();
    assert_eq!(listing.iter().filter(|e| !e.is_dir).count(), 10);
    assert!(listing.iter().any(|e| e.is_dir && e.path == "locks"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn onedrive_large_files_use_upload_sessions() {
    let (b, mock) = onedrive().await;
    let mut s = b.onedrive_storage().await;
    s.set_upload_limits(100_000, 1);
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("big.bin");
    let data: Vec<u8> = (0..1_000_000u32).map(|i| (i % 253) as u8).collect();
    std::fs::write(&src, &data).unwrap();

    s.put_file(".resource/big", &src).await.unwrap();
    let puts = mock
        .requests()
        .iter()
        .filter(|(m, p)| m == "PUT" && p.starts_with("/upload/"))
        .count();
    assert_eq!(puts, 4, "1 MB in 320 KiB chunks");
    let dest = dir.path().join("copy.bin");
    assert!(s.get_to_file(".resource/big", &dest).await.unwrap());
    assert_eq!(std::fs::read(&dest).unwrap(), data);

    // A chunk whose response is lost is resent; the service already has it.
    mock.fail_after_when("PUT", "/upload/", 1);
    s.put_file(".resource/big3", &src).await.unwrap();
    assert!(s.get_to_file(".resource/big3", &dest).await.unwrap());
    assert_eq!(std::fs::read(&dest).unwrap(), data);

    // A failing chunk cancels the session.
    mock.fail_when("PUT", "/upload/", 4); // every attempt at the first chunk
    assert!(s.put_file(".resource/big2", &src).await.is_err());
    assert_eq!(mock.pending_sessions(), 0);

    // In-memory writes over the limit too, with their precondition.
    assert!(
        s.put_if("big.md", &data, &Precondition::Absent)
            .await
            .unwrap()
    );
    assert!(
        !s.put_if("big.md", &data, &Precondition::Absent)
            .await
            .unwrap()
    );
    assert_eq!(s.get("big.md").await.unwrap().unwrap(), data);
}

// -------------------------------------------------------------------------
// Google Drive

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gdrive_sign_in() {
    let (_b, mock) = gdrive().await;
    let provider = google_drive_oauth(
        mock_google_drive::CLIENT_ID,
        mock_google_drive::CLIENT_SECRET,
        &mock.endpoints(),
    );
    let tokens = sign_in(provider).await;
    let mut config = mock.config("Nisshoku");
    config.tokens = tokens;
    let s = GoogleDriveStorage::connect(&config, None).await.unwrap();
    s.put("a.md", b"hello").await.unwrap();
    assert_eq!(mock.file("Nisshoku/a.md").unwrap(), b"hello");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gdrive_refreshes_expired_tokens_and_retries_rate_limits() {
    let (b, mock) = gdrive().await;
    let s = b.gdrive_storage().await;
    s.put("a.md", b"1").await.unwrap();
    let refreshes = mock.token_refreshes();
    mock.expire_tokens();
    assert_eq!(s.get("a.md").await.unwrap().unwrap(), b"1");
    assert_eq!(mock.token_refreshes(), refreshes + 1);

    mock.rate_limit_when("PATCH", "uploadType=media", 2);
    s.put("a.md", b"2").await.unwrap();
    assert_eq!(s.get("a.md").await.unwrap().unwrap(), b"2");
}

/// A create whose response is lost and which is retried must not leave two
/// files with the same name.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gdrive_retried_creates_do_not_duplicate() {
    let (_b, mock) = gdrive().await;
    let mut s = GoogleDriveStorage::new(&mock.config("Notes"), None).unwrap();
    s.set_retry(retry_for(Kind::MockGoogleDrive));
    s.mkdir("").await.unwrap();
    mock.fail_after_when("POST", "uploadType=multipart", 1);
    s.put("a.md", b"1").await.unwrap();
    mock.fail_after_when("POST", "/drive/v3/files?", 1);
    s.mkdir("locks").await.unwrap();
    assert_eq!(mock.count_named("Notes/a.md"), 1);
    assert_eq!(mock.count_named("Notes/locks"), 1);
    assert_eq!(s.get("a.md").await.unwrap().unwrap(), b"1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gdrive_duplicate_names_resolve_to_the_oldest_file() {
    let (_b, mock) = gdrive().await;
    let config = mock.config("Notes");
    let s = GoogleDriveStorage::connect(&config, None).await.unwrap();
    s.put("a.md", b"original").await.unwrap();
    mock.add_duplicate("Notes/a.md", b"duplicate");
    assert_eq!(mock.count_named("Notes/a.md"), 2);

    // A fresh client (no cached IDs) sees the original.
    let other = GoogleDriveStorage::connect(&mock.config("Notes"), None)
        .await
        .unwrap();
    assert_eq!(other.get("a.md").await.unwrap().unwrap(), b"original");
    let listing = other.list("").await.unwrap();
    assert_eq!(listing.iter().filter(|e| e.path == "a.md").count(), 1);
    other.put("a.md", b"updated").await.unwrap();
    assert_eq!(s.get("a.md").await.unwrap().unwrap(), b"updated");

    // Deleting removes every copy, so the duplicate does not come back.
    s.delete("a.md").await.unwrap();
    assert_eq!(mock.count_named("Notes/a.md"), 0);
    assert_eq!(other.get("a.md").await.unwrap(), None);
}

/// A folder deleted and recreated by another client is found again, not
/// taken for empty or written into while gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gdrive_recreated_folders_are_found_again() {
    let (_b, mock) = gdrive().await;
    let a = GoogleDriveStorage::connect(&mock.config("Notes"), None)
        .await
        .unwrap();
    let c = GoogleDriveStorage::connect(&mock.config("Notes"), None)
        .await
        .unwrap();
    a.put("x.md", b"1").await.unwrap();
    a.put(".resource/r", b"blob").await.unwrap();
    assert_eq!(c.list("").await.unwrap().len(), 2);

    c.delete_dir("").await.unwrap();
    c.mkdir("").await.unwrap();
    c.put("y.md", b"2").await.unwrap();

    let names: Vec<_> = a
        .list("")
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.path)
        .collect();
    assert_eq!(names, ["y.md"]);
    a.put(".resource/r2", b"blob").await.unwrap();
    assert_eq!(mock.file("Notes/.resource/r2").unwrap(), b"blob");
    assert_eq!(c.get("x.md").await.unwrap(), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gdrive_listings_are_paged() {
    let (b, mock) = gdrive().await;
    let s = b.gdrive_storage().await;
    mock.set_page_size(3);
    for i in 0..10 {
        s.put(&format!("{i:032x}.md"), b"x").await.unwrap();
    }
    assert_eq!(s.list("").await.unwrap().len(), 10);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gdrive_large_files_use_resumable_uploads() {
    let (b, mock) = gdrive().await;
    let mut s = b.gdrive_storage().await;
    s.set_upload_limits(100_000, 1);
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("big.bin");
    let data: Vec<u8> = (0..700_000u32).map(|i| (i % 249) as u8).collect();
    std::fs::write(&src, &data).unwrap();

    s.put_file(".resource/big", &src).await.unwrap();
    let dest = dir.path().join("copy.bin");
    assert!(s.get_to_file(".resource/big", &dest).await.unwrap());
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    let chunks = || {
        mock.requests()
            .iter()
            .filter(|(m, p)| m == "PUT" && p.contains("upload_id="))
            .count()
    };
    assert_eq!(chunks(), 3, "700 kB in 256 KiB chunks");

    // Updates of large files too.
    let changed: Vec<u8> = data.iter().map(|b| b.wrapping_add(1)).collect();
    std::fs::write(&src, &changed).unwrap();
    s.put_file(".resource/big", &src).await.unwrap();
    assert_eq!(chunks(), 6);
    assert!(s.get_to_file(".resource/big", &dest).await.unwrap());
    assert_eq!(std::fs::read(&dest).unwrap(), changed);
    assert_eq!(mock.pending_sessions(), 0);
}
