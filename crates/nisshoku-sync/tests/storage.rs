//! The `RemoteStorage` contract, checked for every backend, and tests specific
//! to the Nextcloud driver.

mod common;

use std::time::Duration;

use common::*;
use nisshoku_sync::drivers::NextcloudStorage;
use nisshoku_sync::drivers::nextcloud::{fetch_user_id, poll_login_flow, start_login_flow};
use nisshoku_sync::{Precondition, RemoteStorage, SyncError};

backend_tests!(
    basic_file_operations,
    listing,
    conditional_writes,
    large_files,
    exclusive_directories,
);

async fn basic_file_operations(b: &Backend) {
    let s = b.storage().await;
    s.mkdir("").await.unwrap();
    assert_eq!(s.get("missing.md").await.unwrap(), None);
    assert_eq!(s.stat("missing.md").await.unwrap(), None);
    s.delete("missing.md").await.unwrap();

    s.put("a.md", "日本語 content".as_bytes()).await.unwrap();
    assert_eq!(
        s.get("a.md").await.unwrap().unwrap(),
        "日本語 content".as_bytes()
    );
    let stat = s.stat("a.md").await.unwrap().unwrap();
    assert_eq!(stat.path, "a.md");
    assert!(!stat.is_dir);
    assert!(stat.updated_time > 0);

    s.put("a.md", b"replaced").await.unwrap();
    assert_eq!(s.get("a.md").await.unwrap().unwrap(), b"replaced");

    // Writing into a missing directory creates it.
    s.put("sub dir/deeper/b.json", b"{}").await.unwrap();
    assert_eq!(
        s.get("sub dir/deeper/b.json").await.unwrap().unwrap(),
        b"{}"
    );
    s.mkdir("sub dir").await.unwrap(); // already exists: fine
    assert!(s.stat("sub dir").await.unwrap().unwrap().is_dir);

    s.delete("a.md").await.unwrap();
    assert_eq!(s.get("a.md").await.unwrap(), None);
}

async fn listing(b: &Backend) {
    let s = b.storage().await;
    s.mkdir("").await.unwrap();
    assert!(s.list("nothing-here").await.unwrap().is_empty());
    s.mkdir("locks").await.unwrap();
    s.put("x.md", b"1").await.unwrap();
    s.put("y.md", b"2").await.unwrap();
    s.put("locks/1_desktop_abc.json", b"{}").await.unwrap();

    let mut root: Vec<(String, bool)> = s
        .list("")
        .await
        .unwrap()
        .into_iter()
        .map(|e| (e.path, e.is_dir))
        .collect();
    root.sort();
    assert_eq!(
        root,
        vec![
            ("locks".into(), true),
            ("x.md".into(), false),
            ("y.md".into(), false)
        ]
    );
    let locks: Vec<String> = s
        .list("locks")
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.path)
        .collect();
    assert_eq!(locks, vec!["locks/1_desktop_abc.json".to_string()]);

    // Rewriting a file changes its listing signature, even within a second.
    let before = s
        .list("")
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.path == "x.md")
        .unwrap();
    s.put("x.md", b"11").await.unwrap();
    let after = s
        .list("")
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.path == "x.md")
        .unwrap();
    assert_ne!(
        (before.updated_time, before.etag),
        (after.updated_time, after.etag)
    );
}

async fn conditional_writes(b: &Backend) {
    let s = b.storage().await;
    s.mkdir("").await.unwrap();
    assert!(
        s.put_if("c.md", b"v1", &Precondition::Absent)
            .await
            .unwrap()
    );
    assert!(
        !s.put_if("c.md", b"v1 again", &Precondition::Absent)
            .await
            .unwrap()
    );

    let (data, version) = s.get_versioned("c.md").await.unwrap().unwrap();
    assert_eq!(data, b"v1");
    let version = version.expect("this backend reports versions");
    let stale = Precondition::Version(version.clone());

    assert!(s.put_if("c.md", b"v2", &stale).await.unwrap());
    // The version is now outdated.
    assert!(!s.put_if("c.md", b"v3", &stale).await.unwrap());
    assert!(!s.delete_if("c.md", &stale).await.unwrap());
    assert_eq!(s.get("c.md").await.unwrap().unwrap(), b"v2");

    let (_, current) = s.get_versioned("c.md").await.unwrap().unwrap();
    assert!(
        s.delete_if("c.md", &Precondition::Version(current.unwrap()))
            .await
            .unwrap()
    );
    assert_eq!(s.get("c.md").await.unwrap(), None);
}

async fn large_files(b: &Backend) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("big.bin");
    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&src, &data).unwrap();

    let s = b.storage().await;
    s.mkdir(".resource").await.unwrap();
    s.put_file(".resource/big", &src).await.unwrap();
    let dest = dir.path().join("out/copy.bin");
    assert!(s.get_to_file(".resource/big", &dest).await.unwrap());
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert!(
        !s.get_to_file(".resource/none", &dir.path().join("none"))
            .await
            .unwrap()
    );
}

/// `mkdir_exclusive` creates a directory only if it does not exist, and of
/// clients creating it at the same moment exactly one succeeds.
async fn exclusive_directories(b: &Backend) {
    let (s1, s2) = (b.storage().await, b.storage().await);
    s1.mkdir("").await.unwrap();
    assert!(s1.mkdir_exclusive(".x/lock").await.unwrap());
    assert!(!s1.mkdir_exclusive(".x/lock").await.unwrap());
    assert!(!s2.mkdir_exclusive(".x/lock").await.unwrap());
    s1.put(".x/lock/owner.json", b"{}").await.unwrap();
    assert_eq!(s2.get(".x/lock/owner.json").await.unwrap().unwrap(), b"{}");
    s2.delete_dir(".x/lock").await.unwrap();
    assert_eq!(s1.get(".x/lock/owner.json").await.unwrap(), None);
    assert_eq!(s1.stat(".x/lock").await.unwrap(), None);
    s1.delete_dir(".x/lock").await.unwrap(); // already gone

    for round in 0..8 {
        let dir = format!(".x/race{round}");
        let (a, c) = tokio::join!(s1.mkdir_exclusive(&dir), s2.mkdir_exclusive(&dir));
        let winners = [a.unwrap(), c.unwrap()].iter().filter(|w| **w).count();
        assert_eq!(winners, 1, "round {round}");
        // The directory works for both.
        s2.put(&format!("{dir}/f"), b"x").await.unwrap();
        assert!(s1.get(&format!("{dir}/f")).await.unwrap().is_some());
        s1.delete_dir(&dir).await.unwrap();
    }
}

// -------------------------------------------------------------------------
// Nextcloud driver

async fn with_nextcloud(test: impl AsyncFnOnce(&Backend, &MockNextcloud)) {
    let b = Backend::new(Kind::MockNextcloud).await.unwrap();
    let mock = b.mock.clone().unwrap();
    test(&b, &mock).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_resolves_user_id_and_creates_folder() {
    with_nextcloud(async |_, mock| {
        let config = mock.config("書類/My Notes");
        assert_eq!(
            fetch_user_id(&config.server_url, &config.username, &config.password)
                .await
                .unwrap(),
            "alice"
        );
        let s = NextcloudStorage::connect(&config).await.unwrap();
        assert_eq!(s.user_id(), "alice");
        assert!(mock.has_dir("書類/My Notes"));
        s.put("a.md", b"x").await.unwrap();
        assert_eq!(mock.file("書類/My Notes/a.md").unwrap(), b"x");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_password_is_an_auth_error() {
    with_nextcloud(async |_, mock| {
        let mut config = mock.config("Nisshoku");
        config.password = "wrong".into();
        assert!(matches!(
            NextcloudStorage::connect(&config).await,
            Err(SyncError::Auth(_))
        ));

        // A password revoked after connecting.
        let s = NextcloudStorage::with_user_id(&config, "alice").unwrap();
        assert!(matches!(s.get("a.md").await, Err(SyncError::Auth(_))));
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transient_errors_are_retried() {
    with_nextcloud(async |b, mock| {
        let s = b.storage().await; // 4 attempts
        mock.fail_next(3);
        s.put("a.md", b"x").await.unwrap();
        assert_eq!(
            mock.file(&format!("{}/a.md", folder_of(mock))).unwrap(),
            b"x"
        );

        mock.fail_next(4);
        assert!(matches!(s.get("a.md").await, Err(SyncError::Remote(_))));
        assert_eq!(s.get("a.md").await.unwrap().unwrap(), b"x");
    })
    .await;
}

/// The test folder name chosen by `Backend::new`.
fn folder_of(mock: &MockNextcloud) -> String {
    let (_, path) = mock
        .requests()
        .into_iter()
        .find(|(m, p)| m == "MKCOL" && p.contains("/nisshoku-tests/"))
        .unwrap();
    path.trim_start_matches("/remote.php/dav/files/alice/")
        .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_files_use_chunked_upload() {
    with_nextcloud(async |_, mock| {
        let mut s = NextcloudStorage::connect(&mock.config("Nisshoku"))
            .await
            .unwrap();
        s.set_chunking(10, 4);
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("f.bin");
        std::fs::write(&src, b"0123456789abcdefghijklm").unwrap(); // 23 bytes: 6 chunks

        s.put_file(".resource/r1", &src).await.unwrap();
        assert_eq!(
            mock.file("Nisshoku/.resource/r1").unwrap(),
            b"0123456789abcdefghijklm"
        );
        let requests = mock.requests();
        let chunk_puts = requests
            .iter()
            .filter(|(m, p)| m == "PUT" && p.starts_with("/remote.php/dav/uploads/alice/"))
            .count();
        assert_eq!(chunk_puts, 6);
        assert!(
            requests
                .iter()
                .any(|(m, p)| m == "MOVE" && p.ends_with("/.file"))
        );
        assert!(mock.pending_uploads().is_empty());

        // An exact multiple of the chunk size, and an empty-ish small file (no chunking).
        std::fs::write(&src, b"abcdefghijkl").unwrap();
        s.put_file(".resource/r2", &src).await.unwrap();
        assert_eq!(mock.file("Nisshoku/.resource/r2").unwrap(), b"abcdefghijkl");
        std::fs::write(&src, b"").unwrap();
        s.put_file(".resource/r3", &src).await.unwrap();
        assert_eq!(mock.file("Nisshoku/.resource/r3").unwrap(), b"");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_chunked_upload_is_cleaned_up() {
    with_nextcloud(async |_, mock| {
        let mut s = NextcloudStorage::connect(&mock.config("Nisshoku"))
            .await
            .unwrap();
        s.set_chunking(10, 4);
        s.set_retry(1, Duration::from_millis(1));
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("f.bin");
        std::fs::write(&src, b"0123456789abcdefghijklm").unwrap();
        s.mkdir(".resource").await.unwrap();

        // The third chunk fails, after the upload was started.
        mock.fail_when("PUT", "/00003", 1);
        assert!(s.put_file(".resource/r1", &src).await.is_err());
        assert!(
            mock.pending_uploads().is_empty(),
            "upload directory removed"
        );
        assert!(mock.file("Nisshoku/.resource/r1").is_none());

        // The assembled file is never partial: a failing final MOVE leaves nothing.
        mock.fail_when("MOVE", "/.file", 1);
        assert!(s.put_file(".resource/r1", &src).await.is_err());
        assert!(mock.file("Nisshoku/.resource/r1").is_none());
        assert!(mock.pending_uploads().is_empty());

        // A retry produces the right file.
        s.set_retry(4, Duration::from_millis(1));
        s.put_file(".resource/r1", &src).await.unwrap();
        assert_eq!(
            mock.file("Nisshoku/.resource/r1").unwrap(),
            b"0123456789abcdefghijklm"
        );
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_flow() {
    with_nextcloud(async |_, mock| {
        let flow = start_login_flow(&format!("{}/", mock.url)).await.unwrap();
        assert!(flow.login_url.starts_with(&mock.url));
        assert_eq!(poll_login_flow(&flow).await.unwrap(), None);
        let creds = poll_login_flow(&flow).await.unwrap().unwrap();
        assert_eq!(creds.login_name, common::mock_nextcloud::LOGIN_NAME);
        assert_eq!(creds.app_password, common::mock_nextcloud::APP_PASSWORD);
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_nextcloud_chunked_upload() {
    let Some(config) =
        real_nextcloud_config(&format!("nisshoku-tests/{}", nisshoku_core::ids::new_id()))
    else {
        return;
    };
    let mut s = NextcloudStorage::connect(&config).await.unwrap();
    s.set_chunking(1024 * 1024, 1024 * 1024);
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("f.bin");
    let data: Vec<u8> = (0..3_500_000u32).map(|i| (i % 253) as u8).collect();
    std::fs::write(&src, &data).unwrap();
    s.put_file(".resource/big", &src).await.unwrap();
    assert_eq!(s.get(".resource/big").await.unwrap().unwrap(), data);
    s.delete("").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_nextcloud_login_flow_starts() {
    let Some(config) = real_nextcloud_config("") else {
        return;
    };
    let flow = start_login_flow(&config.server_url).await.unwrap();
    assert!(
        flow.login_url.contains("/login/v2/flow/"),
        "{}",
        flow.login_url
    );
    assert_eq!(poll_login_flow(&flow).await.unwrap(), None);
}

/// Clients collide on the sync mutex's paths by design, and Nextcloud answers
/// some collisions with `423 Locked`: taking and giving back the mutex waits
/// such failures out instead of failing the sync.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_mutex_survives_transient_errors() {
    use nisshoku_sync::locks::{MUTEX_DIR, MutexTiming, WriteMutex};
    with_nextcloud(async |_, mock| {
        let mut s = NextcloudStorage::connect(&mock.config("Nisshoku"))
            .await
            .unwrap();
        s.set_retry(1, Duration::from_millis(1));
        let timing = MutexTiming {
            stale_after: Duration::from_secs(5),
            refresh_every: Duration::from_millis(1),
            max_wait: Duration::from_secs(20),
        };

        mock.fail_when("MKCOL", "/.nisshoku/sync", 2);
        mock.fail_when("PUT", "/.nisshoku/sync/owner.json", 2);
        let mut mutex = WriteMutex::acquire(&s, "client-a", timing.clone())
            .await
            .unwrap();
        assert!(mock.file("Nisshoku/.nisshoku/sync/owner.json").is_some());

        mock.fail_when("PUT", "/.nisshoku/sync/owner.json", 1);
        mutex.refresh_if_due(&s).await.unwrap();

        mock.fail_when("DELETE", "/.nisshoku/sync", 2);
        mutex.release(&s).await.unwrap();
        assert_eq!(s.stat(MUTEX_DIR).await.unwrap(), None);
    })
    .await;
}
