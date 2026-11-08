//! Shared test harness: sync clients, interchangeable backends and fault
//! injection.
//!
//! Tests against real services run when these are set, and are skipped
//! otherwise:
//! - Nextcloud: `NISSHOKU_TEST_NEXTCLOUD_URL`, `NISSHOKU_TEST_NEXTCLOUD_USER`,
//!   `NISSHOKU_TEST_NEXTCLOUD_PASSWORD`;
//! - OneDrive: `NISSHOKU_TEST_ONEDRIVE_CLIENT_ID`,
//!   `NISSHOKU_TEST_ONEDRIVE_REFRESH_TOKEN`;
//! - Google Drive: `NISSHOKU_TEST_GDRIVE_CLIENT_ID`,
//!   `NISSHOKU_TEST_GDRIVE_CLIENT_SECRET`, `NISSHOKU_TEST_GDRIVE_REFRESH_TOKEN`.
//!
//! (`cargo run --example login -- onedrive|gdrive` prints a refresh token.)

#![allow(dead_code)]

pub mod mock_google_drive;
pub mod mock_nextcloud;
pub mod mock_oauth;
pub mod mock_onedrive;

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use nisshoku_core::{Database, Folder, ItemType, Model, Note, NoteTag, Resource, Revision, Tag};
use nisshoku_sync::drivers::http::RetryPolicy;
use nisshoku_sync::drivers::{
    FileSystemStorage, GoogleDriveConfig, GoogleDriveStorage, NextcloudConfig, NextcloudStorage,
    OAuthTokens, OneDriveConfig, OneDriveStorage,
};
use nisshoku_sync::{
    DeltaPage, Precondition, RemoteStat, RemoteStorage, SharedDb, SyncError, SyncOptions,
    SyncReport, Synchronizer,
};
use tempfile::TempDir;

pub use mock_google_drive::MockGoogleDrive;
pub use mock_nextcloud::MockNextcloud;
pub use mock_onedrive::MockOneDrive;

pub const TARGET_ID: i64 = 5;

// -------------------------------------------------------------------------
// Backends

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    FileSystem,
    MockNextcloud,
    Nextcloud,
    MockOneDrive,
    OneDrive,
    MockGoogleDrive,
    GoogleDrive,
}

impl Kind {
    pub fn is_real_service(self) -> bool {
        matches!(self, Kind::Nextcloud | Kind::OneDrive | Kind::GoogleDrive)
    }
}

pub struct Backend {
    pub kind: Kind,
    dir: Option<TempDir>,
    pub mock: Option<MockNextcloud>,
    pub onedrive_mock: Option<MockOneDrive>,
    pub gdrive_mock: Option<MockGoogleDrive>,
    config: Option<NextcloudConfig>,
    onedrive: Option<OneDriveConfig>,
    gdrive: Option<GoogleDriveConfig>,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

pub fn real_onedrive_config(folder: &str) -> Option<OneDriveConfig> {
    Some(OneDriveConfig {
        client_id: env("NISSHOKU_TEST_ONEDRIVE_CLIENT_ID")?,
        folder: folder.into(),
        tokens: OAuthTokens {
            refresh_token: env("NISSHOKU_TEST_ONEDRIVE_REFRESH_TOKEN")?,
            ..Default::default()
        },
        endpoints: Default::default(),
    })
}

pub fn real_gdrive_config(folder: &str) -> Option<GoogleDriveConfig> {
    Some(GoogleDriveConfig {
        client_id: env("NISSHOKU_TEST_GDRIVE_CLIENT_ID")?,
        client_secret: env("NISSHOKU_TEST_GDRIVE_CLIENT_SECRET")?,
        folder: folder.into(),
        tokens: OAuthTokens {
            refresh_token: env("NISSHOKU_TEST_GDRIVE_REFRESH_TOKEN")?,
            ..Default::default()
        },
        endpoints: Default::default(),
    })
}

/// Fast retries for mocks; patient ones for real services.
pub fn retry_for(kind: Kind) -> RetryPolicy {
    if kind.is_real_service() {
        RetryPolicy {
            max_attempts: 8,
            ..Default::default()
        }
    } else {
        RetryPolicy {
            max_attempts: 4,
            initial_delay: std::time::Duration::from_millis(20),
            max_delay: std::time::Duration::from_secs(1),
        }
    }
}

pub fn real_nextcloud_config(folder: &str) -> Option<NextcloudConfig> {
    Some(NextcloudConfig {
        server_url: std::env::var("NISSHOKU_TEST_NEXTCLOUD_URL").ok()?,
        username: std::env::var("NISSHOKU_TEST_NEXTCLOUD_USER").ok()?,
        password: std::env::var("NISSHOKU_TEST_NEXTCLOUD_PASSWORD").ok()?,
        folder: folder.into(),
    })
}

impl Backend {
    /// `None` if the backend is not available (real Nextcloud not configured).
    pub async fn new(kind: Kind) -> Option<Self> {
        let folder = format!("nisshoku-tests/{}", nisshoku_core::ids::new_id());
        let empty = Self {
            kind,
            dir: None,
            mock: None,
            onedrive_mock: None,
            gdrive_mock: None,
            config: None,
            onedrive: None,
            gdrive: None,
        };
        Some(match kind {
            Kind::FileSystem => Self {
                dir: Some(tempfile::tempdir().unwrap()),
                ..empty
            },
            Kind::MockNextcloud => {
                let mock = MockNextcloud::start().await;
                let config = mock.config(&folder);
                Self {
                    mock: Some(mock),
                    config: Some(config),
                    ..empty
                }
            }
            Kind::Nextcloud => {
                let Some(config) = real_nextcloud_config(&folder) else {
                    eprintln!("real Nextcloud not configured; skipping");
                    return None;
                };
                Self {
                    config: Some(config),
                    ..empty
                }
            }
            Kind::MockOneDrive => {
                let mock = MockOneDrive::start().await;
                let config = mock.config(&folder);
                Self {
                    onedrive_mock: Some(mock),
                    onedrive: Some(config),
                    ..empty
                }
            }
            Kind::OneDrive => {
                let Some(config) = real_onedrive_config(&folder) else {
                    eprintln!("real OneDrive not configured; skipping");
                    return None;
                };
                Self {
                    onedrive: Some(config),
                    ..empty
                }
            }
            Kind::MockGoogleDrive => {
                let mock = MockGoogleDrive::start().await;
                let config = mock.config(&folder);
                Self {
                    gdrive_mock: Some(mock),
                    gdrive: Some(config),
                    ..empty
                }
            }
            Kind::GoogleDrive => {
                let Some(config) = real_gdrive_config(&folder) else {
                    eprintln!("real Google Drive not configured; skipping");
                    return None;
                };
                Self {
                    gdrive: Some(config),
                    ..empty
                }
            }
        })
    }

    /// A new OneDrive connection (each with its own tokens on the mock).
    pub async fn onedrive_storage(&self) -> OneDriveStorage {
        let mut config = self.onedrive.clone().unwrap();
        if let Some(mock) = &self.onedrive_mock {
            config.tokens = mock.initial_tokens();
        }
        let mut s = OneDriveStorage::new(&config, None).unwrap();
        s.set_retry(retry_for(self.kind));
        s.mkdir("").await.unwrap();
        s
    }

    pub async fn gdrive_storage(&self) -> GoogleDriveStorage {
        let config = self.gdrive.clone().unwrap();
        let mut s = GoogleDriveStorage::new(&config, None).unwrap();
        s.set_retry(retry_for(self.kind));
        if self.kind == Kind::MockGoogleDrive {
            s.set_election_settle(std::time::Duration::from_millis(20));
        }
        s.mkdir("").await.unwrap();
        s
    }

    /// A new connection to the target (each client gets its own).
    pub async fn storage(&self) -> Arc<dyn RemoteStorage> {
        match self.kind {
            Kind::FileSystem => Arc::new(FileSystemStorage::new(self.dir.as_ref().unwrap().path())),
            Kind::MockNextcloud | Kind::Nextcloud => {
                let mut s = NextcloudStorage::connect(self.config.as_ref().unwrap())
                    .await
                    .unwrap();
                s.set_retry(4, std::time::Duration::from_millis(20));
                Arc::new(s)
            }
            Kind::MockOneDrive | Kind::OneDrive => Arc::new(self.onedrive_storage().await),
            Kind::MockGoogleDrive | Kind::GoogleDrive => Arc::new(self.gdrive_storage().await),
        }
    }

    pub async fn cleanup(&self) {
        match self.kind {
            Kind::Nextcloud => {
                let s = NextcloudStorage::connect(self.config.as_ref().unwrap())
                    .await
                    .unwrap();
                s.delete("").await.unwrap();
            }
            Kind::OneDrive => self.onedrive_storage().await.delete_dir("").await.unwrap(),
            Kind::GoogleDrive => self.gdrive_storage().await.delete_dir("").await.unwrap(),
            _ => {}
        }
    }
}

/// Generates one test per backend for each scenario function.
#[macro_export]
macro_rules! backend_tests {
    ($($name:ident),* $(,)?) => {
        $crate::backend_tests!(@kind file_system, FileSystem, $($name),*);
        $crate::backend_tests!(@kind mock_nextcloud, MockNextcloud, $($name),*);
        $crate::backend_tests!(@kind nextcloud, Nextcloud, $($name),*);
        $crate::backend_tests!(@kind mock_onedrive, MockOneDrive, $($name),*);
        $crate::backend_tests!(@kind onedrive, OneDrive, $($name),*);
        $crate::backend_tests!(@kind mock_google_drive, MockGoogleDrive, $($name),*);
        $crate::backend_tests!(@kind google_drive, GoogleDrive, $($name),*);
    };
    (@kind $module:ident, $kind:ident, $($name:ident),*) => {
        mod $module {
            $(
                #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
                async fn $name() {
                    let Some(b) = $crate::common::Backend::new($crate::common::Kind::$kind).await else { return };
                    super::$name(&b).await;
                    b.cleanup().await;
                }
            )*
        }
    };
}

// -------------------------------------------------------------------------
// Clients

pub struct Client {
    pub db: SharedDb,
    pub synchronizer: Synchronizer,
    pub resource_dir: TempDir,
    pub storage: Arc<dyn RemoteStorage>,
}

impl Client {
    pub fn new(storage: Arc<dyn RemoteStorage>) -> Self {
        Self::with_target(storage, TARGET_ID)
    }

    pub fn with_target(storage: Arc<dyn RemoteStorage>, target_id: i64) -> Self {
        let db: SharedDb = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let resource_dir = tempfile::tempdir().unwrap();
        Self::with_db(db, storage, target_id, resource_dir)
    }

    pub fn with_db(
        db: SharedDb,
        storage: Arc<dyn RemoteStorage>,
        target_id: i64,
        resource_dir: TempDir,
    ) -> Self {
        let synchronizer = Synchronizer::new(
            db.clone(),
            storage.clone(),
            SyncOptions {
                target_id,
                resource_dir: resource_dir.path().to_path_buf(),
                client_type: "desktop".into(),
                master_password: None,
                mutex_timing: nisshoku_sync::locks::MutexTiming {
                    stale_after: std::time::Duration::from_millis(1500),
                    refresh_every: std::time::Duration::from_millis(300),
                    max_wait: std::time::Duration::from_secs(60),
                },
            },
        );
        Self {
            db,
            synchronizer,
            resource_dir,
            storage,
        }
    }

    pub async fn of(backend: &Backend) -> Self {
        Self::new(backend.storage().await)
    }

    /// Syncs, expecting success without item errors.
    pub async fn sync(&self) -> SyncReport {
        let report = self.synchronizer.sync().await.unwrap();
        assert!(report.errors.is_empty(), "sync errors: {:?}", report.errors);
        report
    }

    pub fn db(&self) -> MutexGuard<'_, Database> {
        self.db.lock().unwrap()
    }

    /// Notes, the original(s) first and conflict copies last.
    pub fn notes(&self) -> Vec<Note> {
        let mut notes = self.db().all::<Note>().unwrap();
        notes.sort_by_key(|n| (n.is_conflict, n.created_time, n.id.clone()));
        notes
    }

    pub fn note(&self, id: &str) -> Option<Note> {
        self.db().load::<Note>(id).unwrap()
    }

    pub fn edit_note(&self, id: &str, body: &str) {
        let mut n = self.note(id).unwrap();
        n.body = body.into();
        self.db().save(&mut n).unwrap();
    }

    pub fn create_folder(&self, parent: &str, title: &str) -> Folder {
        let mut f = Folder::new(parent, title);
        self.db().save(&mut f).unwrap();
        f
    }

    pub fn create_note(&self, parent: &str, title: &str, body: &str) -> Note {
        let mut n = Note::new(parent, title, body);
        self.db().save(&mut n).unwrap();
        n
    }

    pub fn add_resource(&self, title: &str, data: &[u8]) -> Resource {
        let mut r = Resource {
            id: nisshoku_core::ids::new_id(),
            title: title.into(),
            mime: "application/octet-stream".into(),
            file_extension: "bin".into(),
            size: data.len() as i64,
            blob_updated_time: nisshoku_core::time::now_ms(),
            ..Default::default()
        };
        std::fs::write(self.resource_path(&r), data).unwrap();
        self.db().save(&mut r).unwrap();
        r
    }

    pub fn set_resource_data(&self, id: &str, data: &[u8]) {
        let mut r = self.db().load::<Resource>(id).unwrap().unwrap();
        std::fs::write(self.resource_path(&r), data).unwrap();
        r.size = data.len() as i64;
        r.blob_updated_time = nisshoku_core::time::now_ms().max(r.blob_updated_time + 1);
        self.db().save(&mut r).unwrap();
    }

    pub fn resource_path(&self, r: &Resource) -> std::path::PathBuf {
        self.resource_dir.path().join(r.file_name())
    }

    pub fn resource_data(&self, r: &Resource) -> Option<Vec<u8>> {
        std::fs::read(self.resource_path(r)).ok()
    }

    /// Everything that should be identical on all clients once they are in
    /// sync: serialized items (conflict copies excluded) and resource data.
    pub fn snapshot(&self) -> BTreeMap<String, String> {
        let db = self.db();
        let mut out = BTreeMap::new();
        fn add<M: Model>(out: &mut BTreeMap<String, String>, items: Vec<M>) {
            for item in items {
                out.insert(format!("{:?}:{}", M::ITEM_TYPE, item.id()), item.to_text());
            }
        }
        add(&mut out, db.all::<Folder>().unwrap());
        add(
            &mut out,
            db.query::<Note, _>("is_conflict = 0", []).unwrap(),
        );
        add(&mut out, db.all::<Tag>().unwrap());
        add(&mut out, db.all::<NoteTag>().unwrap());
        add(&mut out, db.all::<Revision>().unwrap());
        for r in db.all::<Resource>().unwrap() {
            let data = std::fs::read(self.resource_dir.path().join(r.file_name()));
            out.insert(format!("blob:{}", r.id), format!("{:?}", data.ok()));
            out.insert(format!("Resource:{}", r.id), r.to_text());
        }
        out
    }
}

/// Syncs all clients in rounds until a whole round changes nothing.
pub async fn sync_until_stable(clients: &[&Client]) {
    for _ in 0..8 {
        let mut changed = false;
        for c in clients {
            changed |= !c.sync().await.is_noop();
        }
        if !changed {
            return;
        }
    }
    panic!("clients did not reach a stable state");
}

pub fn assert_converged(clients: &[&Client]) {
    let first = clients[0].snapshot();
    for (i, c) in clients.iter().enumerate().skip(1) {
        let other = c.snapshot();
        if other != first {
            let keys: HashSet<_> = first.keys().chain(other.keys()).collect();
            let mut diff: Vec<_> = keys
                .into_iter()
                .filter(|k| first.get(*k) != other.get(*k))
                .map(|k| {
                    format!(
                        "{k}:\n  client 0: {:?}\n  client {i}: {:?}",
                        first.get(k),
                        other.get(k)
                    )
                })
                .collect();
            diff.sort();
            panic!("client {i} differs from client 0:\n{}", diff.join("\n"));
        }
    }
    for (i, c) in clients.iter().enumerate() {
        let orphans = c.db().orphaned_notes().unwrap();
        assert!(
            orphans.is_empty(),
            "client {i} has orphaned notes: {orphans:?}"
        );
    }
}

// -------------------------------------------------------------------------
// Fault injection

/// Small deterministic PRNG (xorshift64*), so failures are reproducible.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn chance(&mut self, per_mille: u32) -> bool {
        self.next() % 1000 < per_mille as u64
    }
}

/// Wraps a storage and makes operations fail at random, either before they
/// happen or after (as when a response is lost).
pub struct Flaky {
    inner: Arc<dyn RemoteStorage>,
    rng: Mutex<Rng>,
    per_mille: AtomicU32,
}

impl Flaky {
    pub fn new(inner: Arc<dyn RemoteStorage>, seed: u64, per_mille: u32) -> Arc<Self> {
        Arc::new(Self {
            inner,
            rng: Mutex::new(Rng::new(seed)),
            per_mille: AtomicU32::new(per_mille),
        })
    }

    pub fn set_failure_rate(&self, per_mille: u32) {
        self.per_mille.store(per_mille, Ordering::SeqCst);
    }

    fn roll(&self) -> bool {
        let p = self.per_mille.load(Ordering::SeqCst);
        p > 0 && self.rng.lock().unwrap().chance(p)
    }

    fn fail() -> SyncError {
        SyncError::Remote("injected failure".into())
    }

    async fn run<T>(
        &self,
        op: impl std::future::Future<Output = nisshoku_sync::Result<T>>,
    ) -> nisshoku_sync::Result<T> {
        if self.roll() {
            return Err(Self::fail());
        }
        let result = op.await?;
        if self.roll() {
            return Err(Self::fail());
        }
        Ok(result)
    }
}

#[async_trait]
impl RemoteStorage for Flaky {
    fn unstable_since(&self) -> Option<i64> {
        self.inner.unstable_since()
    }
    async fn stat(&self, path: &str) -> nisshoku_sync::Result<Option<RemoteStat>> {
        self.run(self.inner.stat(path)).await
    }
    async fn get(&self, path: &str) -> nisshoku_sync::Result<Option<Vec<u8>>> {
        self.run(self.inner.get(path)).await
    }
    async fn put(&self, path: &str, data: &[u8]) -> nisshoku_sync::Result<()> {
        self.run(self.inner.put(path, data)).await
    }
    async fn delete(&self, path: &str) -> nisshoku_sync::Result<()> {
        self.run(self.inner.delete(path)).await
    }
    async fn mkdir(&self, path: &str) -> nisshoku_sync::Result<()> {
        self.run(self.inner.mkdir(path)).await
    }
    async fn list(&self, dir: &str) -> nisshoku_sync::Result<Vec<RemoteStat>> {
        self.run(self.inner.list(dir)).await
    }
    async fn mkdir_exclusive(&self, path: &str) -> nisshoku_sync::Result<bool> {
        self.run(self.inner.mkdir_exclusive(path)).await
    }
    async fn delete_dir(&self, path: &str) -> nisshoku_sync::Result<()> {
        self.run(self.inner.delete_dir(path)).await
    }
    async fn get_versioned(
        &self,
        path: &str,
    ) -> nisshoku_sync::Result<Option<(Vec<u8>, Option<String>)>> {
        self.run(self.inner.get_versioned(path)).await
    }
    async fn put_if(
        &self,
        path: &str,
        data: &[u8],
        p: &Precondition,
    ) -> nisshoku_sync::Result<bool> {
        self.run(self.inner.put_if(path, data, p)).await
    }
    async fn delete_if(&self, path: &str, p: &Precondition) -> nisshoku_sync::Result<bool> {
        self.run(self.inner.delete_if(path, p)).await
    }
    async fn get_to_file(&self, path: &str, dest: &Path) -> nisshoku_sync::Result<bool> {
        self.run(self.inner.get_to_file(path, dest)).await
    }
    async fn put_file(&self, path: &str, src: &Path) -> nisshoku_sync::Result<()> {
        self.run(self.inner.put_file(path, src)).await
    }
    async fn delta(
        &self,
        context: Option<&str>,
        synced: &HashSet<String>,
    ) -> nisshoku_sync::Result<DeltaPage> {
        if self.roll() {
            return Err(Self::fail());
        }
        nisshoku_sync::storage::basic_delta(self, context, synced).await
    }
}

/// Wraps a storage and calls a hook before every operation (with the number
/// of operations so far), e.g. to cancel a sync or edit data mid-sync.
type Hook = Box<dyn Fn(usize, &str, &str) + Send + Sync>;

pub struct Hooked {
    inner: Arc<dyn RemoteStorage>,
    count: AtomicUsize,
    hook: Hook,
}

impl Hooked {
    pub fn new(
        inner: Arc<dyn RemoteStorage>,
        hook: impl Fn(usize, &str, &str) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner,
            count: AtomicUsize::new(0),
            hook: Box::new(hook),
        })
    }
    fn call(&self, op: &str, path: &str) {
        let n = self.count.fetch_add(1, Ordering::SeqCst);
        (self.hook)(n, op, path);
    }
}

#[async_trait]
impl RemoteStorage for Hooked {
    fn unstable_since(&self) -> Option<i64> {
        self.inner.unstable_since()
    }
    async fn stat(&self, path: &str) -> nisshoku_sync::Result<Option<RemoteStat>> {
        self.call("stat", path);
        self.inner.stat(path).await
    }
    async fn get(&self, path: &str) -> nisshoku_sync::Result<Option<Vec<u8>>> {
        self.call("get", path);
        self.inner.get(path).await
    }
    async fn put(&self, path: &str, data: &[u8]) -> nisshoku_sync::Result<()> {
        self.call("put", path);
        self.inner.put(path, data).await
    }
    async fn delete(&self, path: &str) -> nisshoku_sync::Result<()> {
        self.call("delete", path);
        self.inner.delete(path).await
    }
    async fn mkdir(&self, path: &str) -> nisshoku_sync::Result<()> {
        self.call("mkdir", path);
        self.inner.mkdir(path).await
    }
    async fn list(&self, dir: &str) -> nisshoku_sync::Result<Vec<RemoteStat>> {
        self.call("list", dir);
        self.inner.list(dir).await
    }
    async fn mkdir_exclusive(&self, path: &str) -> nisshoku_sync::Result<bool> {
        self.call("mkdir", path);
        self.inner.mkdir_exclusive(path).await
    }
    async fn delete_dir(&self, path: &str) -> nisshoku_sync::Result<()> {
        self.call("delete", path);
        self.inner.delete_dir(path).await
    }
    async fn get_versioned(
        &self,
        path: &str,
    ) -> nisshoku_sync::Result<Option<(Vec<u8>, Option<String>)>> {
        self.call("get", path);
        self.inner.get_versioned(path).await
    }
    async fn put_if(
        &self,
        path: &str,
        data: &[u8],
        p: &Precondition,
    ) -> nisshoku_sync::Result<bool> {
        self.call("put", path);
        self.inner.put_if(path, data, p).await
    }
    async fn delete_if(&self, path: &str, p: &Precondition) -> nisshoku_sync::Result<bool> {
        self.call("delete", path);
        self.inner.delete_if(path, p).await
    }
    async fn get_to_file(&self, path: &str, dest: &Path) -> nisshoku_sync::Result<bool> {
        self.call("get", path);
        self.inner.get_to_file(path, dest).await
    }
    async fn put_file(&self, path: &str, src: &Path) -> nisshoku_sync::Result<()> {
        self.call("put", path);
        self.inner.put_file(path, src).await
    }
}

pub fn cancel_after(flag: Arc<AtomicBool>, n: usize) -> impl Fn(usize, &str, &str) + Send + Sync {
    move |count, _, _| {
        if count == n {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

pub fn item_type_count(c: &Client, t: ItemType) -> usize {
    let db = c.db();
    match t {
        ItemType::Note => db.all::<Note>().unwrap().len(),
        ItemType::Folder => db.all::<Folder>().unwrap().len(),
        ItemType::Resource => db.all::<Resource>().unwrap().len(),
        ItemType::Tag => db.all::<Tag>().unwrap().len(),
        ItemType::NoteTag => db.all::<NoteTag>().unwrap().len(),
        _ => 0,
    }
}
