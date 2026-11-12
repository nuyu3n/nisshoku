use std::cmp::Reverse;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use nisshoku_core::ids::new_id;
use nisshoku_core::time::now_ms;
use nisshoku_core::{AnyItem, Database, DeletedItem, Folder, ItemType, Note, RawItem, Resource};
use serde::Serialize;

use crate::e2ee::Keyring;
use crate::error::{Result, SyncError};
use crate::locks::{self, LOCK_REFRESH_MS, Lock, MutexTiming, WriteMutex};
use crate::storage::{
    DeltaChange, Precondition, RESOURCE_DIR, RemoteStorage, item_path, resource_blob_path,
};
use crate::sync_info;

/// The database, shared between the UI and the synchronizer. The lock is only
/// ever held for the duration of individual database calls.
pub type SharedDb = Arc<Mutex<Database>>;

/// How many times an item is re-evaluated when another client changes it
/// between our read and our write.
const MAX_ATTEMPTS: usize = 3;

/// What a sync is doing, for showing its progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncStep {
    /// Sending local changes.
    Upload,
    /// Sending local deletions.
    Delete,
    /// Receiving changes from the target (the total is not known ahead).
    Download,
    /// Transferring attachments' data.
    Attachments,
}

/// Called as a sync goes: the step, items done in it, and their total (0
/// when not known).
#[derive(Clone)]
pub struct ProgressHook(pub Arc<dyn Fn(SyncStep, usize, usize) + Send + Sync>);

impl std::fmt::Debug for ProgressHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProgressHook")
    }
}

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Identifies the target in `sync_items`, so that a profile can sync with
    /// several targets independently.
    pub target_id: i64,
    /// Local directory holding resource files.
    pub resource_dir: PathBuf,
    /// Written into lock files: `desktop`, `mobile` or `cli`.
    pub client_type: String,
    /// Opens the target's master keys. Required when the target is encrypted.
    pub master_password: Option<String>,
    /// Timing of the mutex that makes clients of this app sync one at a time.
    pub mutex_timing: MutexTiming,
    /// Told of the progress of each sync.
    pub progress: Option<ProgressHook>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct SyncReport {
    pub created_remote: usize,
    pub updated_remote: usize,
    pub deleted_remote: usize,
    pub created_local: usize,
    pub updated_local: usize,
    pub deleted_local: usize,
    /// Conflicts that produced a conflict copy.
    pub conflicts: usize,
    /// Items deleted on one side but kept because the other side still needed
    /// them (edited notes, folders that still have contents).
    pub kept: usize,
    /// Resources whose data is not on the target yet (the client that has it
    /// will upload it on its next sync).
    pub pending_blobs: usize,
    /// Problems with individual items, which were skipped.
    pub errors: Vec<String>,
}

impl SyncReport {
    /// Whether the sync changed anything on either side.
    pub fn is_noop(&self) -> bool {
        let SyncReport { errors, .. } = self;
        *self
            == SyncReport {
                errors: errors.clone(),
                ..Default::default()
            }
    }
}

pub struct Synchronizer {
    db: SharedDb,
    storage: Arc<dyn RemoteStorage>,
    options: SyncOptions,
    cancelled: Arc<AtomicBool>,
    /// Master keys for the current sync; `active` set when uploads are encrypted.
    keyring: Mutex<Arc<Keyring>>,
}

struct Run {
    report: SyncReport,
    lock: Lock,
    mutex: WriteMutex,
}

enum Download {
    Done,
    Missing,
    /// The data could not be decrypted.
    Failed(String),
}

enum Remote {
    Missing,
    Unsupported,
    Item(Box<AnyItem>),
}

/// A remote item together with the precondition to use when overwriting or
/// deleting it.
struct Fetched {
    remote: Remote,
    precondition: Precondition,
}

fn context_key(target: i64) -> String {
    format!("sync.{target}.context")
}

impl Synchronizer {
    pub fn new(db: SharedDb, storage: Arc<dyn RemoteStorage>, options: SyncOptions) -> Self {
        Self {
            db,
            storage,
            options,
            cancelled: Arc::new(AtomicBool::new(false)),
            keyring: Mutex::new(Arc::new(Keyring::default())),
        }
    }

    /// Changes the master password used by the next syncs.
    pub fn set_master_password(&mut self, password: Option<String>) {
        self.options.master_password = password;
    }

    /// Encrypts everything on the target from now on (generating a master key
    /// if needed). All items are re-uploaded, encrypted, by the next sync.
    pub async fn enable_encryption(&mut self, password: &str) -> Result<()> {
        self.with_mutex(sync_info::enable_encryption(&*self.storage, password))
            .await?;
        self.options.master_password = Some(password.to_string());
        self.db().mark_all_for_reupload(self.target())?;
        Ok(())
    }

    /// Stops encrypting. All items are re-uploaded, unencrypted, by the next sync.
    pub async fn disable_encryption(&self) -> Result<()> {
        self.with_mutex(sync_info::disable_encryption(&*self.storage))
            .await?;
        self.db().mark_all_for_reupload(self.target())?;
        Ok(())
    }

    /// Re-encrypts the master keys with a new password. Content is unchanged.
    pub async fn change_master_password(&mut self, old: &str, new: &str) -> Result<()> {
        self.with_mutex(sync_info::change_master_password(&*self.storage, old, new))
            .await?;
        self.options.master_password = Some(new.to_string());
        Ok(())
    }

    fn temp_path(&self, r: &Resource) -> PathBuf {
        self.options
            .resource_dir
            .join(format!(".{}{}", r.id, crate::storage::temp_suffix()))
    }

    /// Uploads a resource's data, encrypted if its metadata says so.
    async fn upload_blob(&self, r: &Resource, path: &std::path::Path) -> Result<()> {
        if r.encryption_blob_encrypted == 0 {
            return self
                .storage
                .put_file(&resource_blob_path(&r.id), path)
                .await;
        }
        let tmp = self.temp_path(r);
        let result = async {
            self.keyring().encrypt_file(path, &tmp).await?;
            self.storage
                .put_file(&resource_blob_path(&r.id), &tmp)
                .await
        }
        .await;
        let _ = std::fs::remove_file(&tmp);
        result
    }

    /// Downloads a resource's data, decrypting it if needed. The format is
    /// recognized from the data itself: a client that lost a race to update
    /// the resource may have left data in the other format than the metadata
    /// says (the hash check then sorts it out).
    async fn download_blob(&self, r: &Resource, dest: &std::path::Path) -> Result<Download> {
        let tmp = self.temp_path(r);
        let result = async {
            if !self
                .storage
                .get_to_file(&resource_blob_path(&r.id), &tmp)
                .await?
            {
                return Ok(Download::Missing);
            }
            let mut head = [0u8; 5];
            let is_encrypted = {
                use std::io::Read;
                let mut f = std::fs::File::open(&tmp)?;
                f.read(&mut head).unwrap_or(0) == 5 && &head == b"JED01"
            };
            if !is_encrypted {
                std::fs::rename(&tmp, dest)?;
                return Ok(Download::Done);
            }
            match self.keyring().decrypt_file(&tmp, dest).await {
                Ok(()) => Ok(Download::Done),
                Err(SyncError::Item(msg)) => Ok(Download::Failed(msg)),
                Err(e) => Err(e),
            }
        }
        .await;
        let _ = std::fs::remove_file(&tmp);
        if !matches!(result, Ok(Download::Done)) {
            let _ = std::fs::remove_file(dest);
        }
        result
    }

    fn keyring(&self) -> Arc<Keyring> {
        self.keyring
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The text to store on the target for an item: encrypted if enabled.
    fn remote_text(&self, item: &AnyItem) -> Result<String> {
        let keyring = self.keyring();
        if keyring.active().is_none() {
            return Ok(item.to_text());
        }
        let cipher = keyring.encrypt_string(&item.to_text())?;
        // Only what is needed to sync stays readable.
        const PLAIN: [&str; 6] = [
            "id",
            "parent_id",
            "updated_time",
            "deleted_time",
            "share_id",
            "is_locked",
        ];
        let inner = item.to_raw();
        let mut shell = RawItem::new(item.item_type());
        shell.props = inner
            .props
            .into_iter()
            .map(|(k, v)| match k.as_str() {
                "encryption_cipher_text" => (k, cipher.clone()),
                "encryption_applied" => (k, "1".into()),
                key if PLAIN.contains(&key) => (k, v),
                _ => (k, String::new()),
            })
            .collect();
        Ok(shell.to_text())
    }

    /// Setting the returned flag stops a running sync at the next item.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    fn db(&self) -> MutexGuard<'_, Database> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn target(&self) -> i64 {
        self.options.target_id
    }

    fn report_progress(&self, step: SyncStep, done: usize, total: usize) {
        if let Some(ProgressHook(f)) = &self.options.progress {
            f(step, done, total);
        }
    }

    fn resource_path(&self, r: &Resource) -> PathBuf {
        self.options.resource_dir.join(r.file_name())
    }

    pub async fn sync(&self) -> Result<SyncReport> {
        self.cancelled.store(false, Ordering::SeqCst);
        let storage = &*self.storage;
        let client_id = self.db().client_id()?;
        let mutex =
            WriteMutex::acquire(storage, &client_id, self.options.mutex_timing.clone()).await?;

        self.sync_holding_mutex(&client_id, mutex).await
    }

    async fn sync_holding_mutex(&self, client_id: &str, mutex: WriteMutex) -> Result<SyncReport> {
        let storage = &*self.storage;
        let prepared = async {
            let info = sync_info::prepare_target(storage).await?;
            let keyring = info.keyring(self.options.master_password.as_deref())?;
            *self.keyring.lock().unwrap_or_else(|e| e.into_inner()) = Arc::new(keyring);
            locks::acquire_sync_lock(storage, &self.options.client_type, client_id).await
        }
        .await;
        let lock = match prepared {
            Ok(lock) => lock,
            Err(e) => {
                let _ = mutex.release(storage).await;
                return Err(e);
            }
        };
        let mut run = Run {
            report: SyncReport::default(),
            lock,
            mutex,
        };
        let result = self.run_steps(&mut run).await;
        if let Err(e) = locks::release_lock(storage, &run.lock).await {
            tracing::warn!("could not release sync lock: {e}");
        }
        if let Err(e) = run.mutex.release(storage).await {
            tracing::warn!("could not release the write mutex: {e}");
        }
        result.map(|()| run.report)
    }

    /// Runs `f` while holding the write mutex (for changes to `info.json`).
    async fn with_mutex<T>(&self, f: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        let storage = &*self.storage;
        let client_id = self.db().client_id()?;
        let mutex =
            WriteMutex::acquire(storage, &client_id, self.options.mutex_timing.clone()).await?;
        let result = f.await;
        let _ = mutex.release(storage).await;
        result
    }

    async fn run_steps(&self, run: &mut Run) -> Result<()> {
        self.upload_step(run).await?;
        self.delete_remote_step(run).await?;
        self.delta_step(run).await?;
        self.sync_blobs(run).await
    }

    /// Called between items: honours cancellation and keeps the lock fresh.
    async fn tick(&self, run: &mut Run) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(SyncError::Cancelled);
        }
        if now_ms() - run.lock.updated_time > LOCK_REFRESH_MS {
            locks::refresh_lock(&*self.storage, &mut run.lock).await?;
        }
        run.mutex.refresh_if_due(&*self.storage).await
    }

    /// Records item-level errors in the report; anything else aborts the sync.
    fn record(&self, run: &mut Run, result: Result<()>) -> Result<bool> {
        match result {
            Ok(()) => Ok(true),
            Err(SyncError::Item(msg)) => {
                tracing::warn!("{msg}");
                run.report.errors.push(msg);
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    async fn fetch_remote(&self, path: &str) -> Result<Fetched> {
        let Some((data, version)) = self.storage.get_versioned(path).await? else {
            return Ok(Fetched {
                remote: Remote::Missing,
                precondition: Precondition::Absent,
            });
        };
        let precondition = version.map_or(Precondition::None, Precondition::Version);
        let text = String::from_utf8(data)
            .map_err(|_| SyncError::Item(format!("{path} is not valid UTF-8")))?;
        let raw = RawItem::parse(&text).map_err(|e| SyncError::Item(format!("{path}: {e}")))?;
        let remote = match AnyItem::from_raw(&raw)
            .map_err(|e| SyncError::Item(format!("{path}: {e}")))?
        {
            None => Remote::Unsupported,
            Some(item) if item.is_encrypted() => {
                let cipher = raw.prop("encryption_cipher_text").unwrap_or_default();
                let inner = self
                    .keyring()
                    .decrypt_string(cipher)
                    .map_err(|e| SyncError::Item(format!("{path}: {e}")))?;
                match AnyItem::parse(&inner).map_err(|e| SyncError::Item(format!("{path}: {e}")))? {
                    Some(i) if i.id() == item.id() && i.item_type() == item.item_type() => {
                        Remote::Item(Box::new(i))
                    }
                    _ => {
                        return Err(SyncError::Item(format!(
                            "{path}: encrypted content does not match"
                        )));
                    }
                }
            }
            Some(item) => Remote::Item(Box::new(item)),
        };
        Ok(Fetched {
            remote,
            precondition,
        })
    }

    // ---------------------------------------------------------------------
    // Step 1: upload local changes

    async fn upload_step(&self, run: &mut Run) -> Result<()> {
        let pending = self.db().items_needing_sync(self.target())?;
        let total = pending.len();
        for (i, (item_type, id)) in pending.into_iter().enumerate() {
            self.report_progress(SyncStep::Upload, i, total);
            self.tick(run).await?;
            let result = self.upload_item(run, item_type, &id).await;
            self.record(run, result)?;
        }
        Ok(())
    }

    async fn upload_item(&self, run: &mut Run, item_type: ItemType, id: &str) -> Result<()> {
        for _ in 0..MAX_ATTEMPTS {
            if self.try_upload_item(run, item_type, id).await? {
                return Ok(());
            }
        }
        Err(SyncError::Item(format!(
            "item {id} kept changing on the sync target; it will be retried on the next sync"
        )))
    }

    /// Returns `false` if the remote item changed while we were writing it.
    async fn try_upload_item(&self, run: &mut Run, item_type: ItemType, id: &str) -> Result<bool> {
        let local = self.db().load_any(item_type, id)?;
        let Some(local) = local else { return Ok(true) };
        let sync_time = self.db().sync_time(self.target(), item_type, id)?;

        let Fetched {
            remote,
            precondition,
        } = self.fetch_remote(&item_path(id)).await?;
        match remote {
            Remote::Missing => match sync_time {
                None => {
                    if !self.push(&local, sync_time, false, &precondition).await? {
                        return Ok(false);
                    }
                    run.report.created_remote += 1;
                }
                // Deleted remotely. Only real local changes are worth keeping (the
                // item may just be flagged for re-upload).
                Some(t) if local.updated_time() > t => {
                    return self.handle_remote_deleted(run, local).await;
                }
                // A folder still holding notes here is kept, as everywhere else.
                Some(_)
                    if matches!(local, AnyItem::Folder(_))
                        && self.db().folder_has_children(id)? =>
                {
                    if !self
                        .push(&local, None, false, &Precondition::Absent)
                        .await?
                    {
                        return Ok(false);
                    }
                    run.report.kept += 1;
                }
                Some(_) => self.delete_local(run, &local)?,
            },
            Remote::Unsupported => {
                return Err(SyncError::Item(format!(
                    "remote item {id} has a different, unsupported type"
                )));
            }
            Remote::Item(remote) => {
                // The same version as ours is not a conflict (our earlier upload
                // whose response was lost, or a re-upload, e.g. to encrypt).
                let same_version = *remote == local;
                if remote.updated_time() > sync_time.unwrap_or(0) && !same_version {
                    self.resolve_conflict(run, local, *remote, sync_time)
                        .await?;
                } else {
                    if !self.push(&local, sync_time, false, &precondition).await? {
                        return Ok(false);
                    }
                    run.report.updated_remote += 1;
                }
            }
        }
        Ok(true)
    }

    /// Uploads an item (and, for resources, its data if it changed). Returns
    /// `false` if `precondition` failed.
    async fn push(
        &self,
        item: &AnyItem,
        sync_time: Option<i64>,
        force_blob: bool,
        precondition: &Precondition,
    ) -> Result<bool> {
        let mut item = item.clone();
        if let AnyItem::Resource(r) = &mut item {
            let path = self.resource_path(r);
            let data_changed = force_blob || sync_time.is_none_or(|t| r.blob_updated_time > t);
            // The data on the target is encrypted or not depending on when it
            // was uploaded; switching encryption on or off re-uploads it.
            let encrypt = self.keyring().active().is_some();
            let mode_changed = (r.encryption_blob_encrypted != 0) != encrypt;
            if data_changed && !path.exists() {
                return Err(SyncError::Item(format!(
                    "the file of resource {} ({}) is missing",
                    r.id, r.title
                )));
            }
            if data_changed || (mode_changed && path.exists()) {
                // Record the data's hash so that downloads can be verified, and
                // the encryption state of the uploaded data. This is metadata
                // about the current version, not a modification.
                let hash = file_sha256(&path).await?;
                if r.extra.get(BLOB_HASH_PROP) != Some(&hash) || mode_changed {
                    r.extra.insert(BLOB_HASH_PROP.into(), hash);
                    r.encryption_blob_encrypted = encrypt as i64;
                    self.db().write(&*r)?;
                }
                // Data goes first, so that clients never see metadata without it.
                self.upload_blob(r, &path).await?;
            }
        }
        // From here on the item may exist on the target even if we never learn
        // the outcome, so a later local deletion must be propagated.
        self.db()
            .mark_sync_pending(self.target(), item.item_type(), item.id())?;
        let written = self
            .storage
            .put_if(
                &item_path(item.id()),
                self.remote_text(&item)?.as_bytes(),
                precondition,
            )
            .await?;
        if written {
            self.db().set_sync_time(
                self.target(),
                item.item_type(),
                item.id(),
                item.updated_time(),
            )?;
        }
        Ok(written)
    }

    /// The local item changed, but it was deleted on the target.
    async fn handle_remote_deleted(&self, run: &mut Run, local: AnyItem) -> Result<bool> {
        match &local {
            AnyItem::Note(note) => {
                // Keep the local changes as a conflict copy.
                self.create_conflict_note(note)?;
                run.report.conflicts += 1;
                self.delete_local(run, &local)?;
            }
            // Without its data a resource cannot be restored: nobody has it.
            AnyItem::Resource(r) if !self.resource_path(r).exists() => {
                self.delete_local(run, &local)?
            }
            // Recreate these rather than lose their contents.
            AnyItem::Folder(_) | AnyItem::Resource(_) => {
                if !self.push(&local, None, true, &Precondition::Absent).await? {
                    return Ok(false);
                }
                run.report.kept += 1;
            }
            AnyItem::Tag(_) | AnyItem::NoteTag(_) | AnyItem::Revision(_) => {
                self.delete_local(run, &local)?
            }
        }
        Ok(true)
    }

    /// Both sides changed. The remote version wins; for notes and resources the
    /// local version is kept as a conflict copy if it differs.
    async fn resolve_conflict(
        &self,
        run: &mut Run,
        local: AnyItem,
        remote: AnyItem,
        sync_time: Option<i64>,
    ) -> Result<()> {
        match (&local, &remote) {
            (AnyItem::Note(l), AnyItem::Note(r)) => {
                if l.title != r.title || l.body != r.body {
                    self.create_conflict_note(l)?;
                    run.report.conflicts += 1;
                }
            }
            (AnyItem::Resource(l), AnyItem::Resource(r)) => {
                let local_blob_changed = sync_time.is_none_or(|t| l.blob_updated_time > t);
                if local_blob_changed && self.local_data_differs(l, r).await? {
                    self.create_conflict_resource(l).await?;
                    run.report.conflicts += 1;
                }
            }
            _ => {}
        }
        self.apply_remote(&remote, Some(&local))?;
        // App data (`user_data`) is merged key by key, so that what was
        // changed here and not there survives; the merge goes out next sync.
        let merged = match (&local, &remote) {
            (AnyItem::Note(l), AnyItem::Note(r)) => {
                Some(nisshoku_core::user_data::merge(&l.user_data, &r.user_data)).filter(|m| *m != r.user_data)
            }
            (AnyItem::Folder(l), AnyItem::Folder(r)) => {
                Some(nisshoku_core::user_data::merge(&l.user_data, &r.user_data)).filter(|m| *m != r.user_data)
            }
            _ => None,
        };
        if let Some(user_data) = merged {
            let db = self.db();
            match db.load_any(remote.item_type(), remote.id())? {
                Some(AnyItem::Note(mut n)) => {
                    n.user_data = user_data;
                    db.save_meta(&mut n)?;
                }
                Some(AnyItem::Folder(mut f)) => {
                    f.user_data = user_data;
                    db.save_meta(&mut f)?;
                }
                _ => {}
            }
        }
        run.report.updated_local += 1;
        Ok(())
    }

    fn create_conflict_note(&self, note: &Note) -> Result<()> {
        let mut copy = note.clone();
        copy.id = new_id();
        copy.is_conflict = 1;
        copy.conflict_original_id = note.id.clone();
        self.db().save(&mut copy)?;
        Ok(())
    }

    async fn create_conflict_resource(&self, resource: &Resource) -> Result<()> {
        let src = self.resource_path(resource);
        if !src.exists() {
            return Ok(());
        }
        let mut copy = resource.clone();
        copy.id = new_id();
        tokio::fs::copy(&src, self.resource_path(&copy)).await?;
        self.db().save(&mut copy)?;

        let name = if resource.title.is_empty() {
            &resource.filename
        } else {
            &resource.title
        };
        let mut note = Note::new(
            "",
            &format!("Conflict: {name}"),
            &format!("[{name}](:/{})", copy.id),
        );
        note.is_conflict = 1;
        self.db().save(&mut note)?;
        Ok(())
    }

    /// Whether the local file of `local` holds other data than `remote`
    /// describes: by hash when known, otherwise by data timestamp.
    async fn local_data_differs(&self, local: &Resource, remote: &Resource) -> Result<bool> {
        let path = self.resource_path(local);
        match remote.extra.get(BLOB_HASH_PROP) {
            Some(hash) if path.exists() => Ok(*hash != file_sha256(&path).await?),
            _ => Ok(local.blob_updated_time != remote.blob_updated_time),
        }
    }

    /// Stores a remote item locally, marking it as in sync.
    fn apply_remote(&self, remote: &AnyItem, previous: Option<&AnyItem>) -> Result<()> {
        if let (AnyItem::Resource(r), Some(AnyItem::Resource(p))) = (remote, previous) {
            // The data changed: drop the stale file so that it is downloaded again.
            let path = self.resource_path(p);
            let differs = match r.extra.get(BLOB_HASH_PROP) {
                Some(hash) if path.exists() => *hash != file_sha256_blocking(&path),
                _ => p.blob_updated_time != r.blob_updated_time,
            };
            if differs || p.file_name() != r.file_name() {
                remove_file_if_exists(&path)?;
            }
        }
        let db = self.db();
        db.write_any(remote)?;
        db.set_sync_time(
            self.target(),
            remote.item_type(),
            remote.id(),
            remote.updated_time(),
        )?;
        Ok(())
    }

    /// Deletes a local item because it was deleted on the target.
    fn delete_local(&self, run: &mut Run, item: &AnyItem) -> Result<()> {
        if let AnyItem::Resource(r) = item {
            remove_file_if_exists(&self.resource_path(r))?;
        }
        let db = self.db();
        db.delete_row(item.item_type(), item.id())?;
        for t in db.synced_types(self.target(), item.id())? {
            db.remove_sync_item(self.target(), t, item.id())?;
        }
        run.report.deleted_local += 1;
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Step 2: propagate local deletions

    async fn delete_remote_step(&self, run: &mut Run) -> Result<()> {
        // Children were recorded before their parents, so by the time a folder
        // is processed we know whether any of its contents were kept.
        let deleted = self.db().deleted_items(self.target())?;
        let total = deleted.len();
        for (i, item) in deleted.into_iter().enumerate() {
            self.report_progress(SyncStep::Delete, i, total);
            self.tick(run).await?;
            let result = self.delete_remote_item(run, &item).await;
            self.record(run, result)?;
        }
        Ok(())
    }

    async fn delete_remote_item(&self, run: &mut Run, item: &DeletedItem) -> Result<()> {
        for _ in 0..MAX_ATTEMPTS {
            if self.try_delete_remote_item(run, item).await? {
                return Ok(());
            }
        }
        Err(SyncError::Item(format!(
            "item {} kept changing on the sync target; it will be retried on the next sync",
            item.item_id
        )))
    }

    async fn try_delete_remote_item(&self, run: &mut Run, item: &DeletedItem) -> Result<bool> {
        let (target, id) = (self.target(), item.item_id.as_str());
        let path = item_path(id);
        let fetched = match self.fetch_remote(&path).await {
            Ok(f) => f,
            // Unreadable remote content: nothing to preserve, delete anyway.
            Err(SyncError::Item(_)) => Fetched {
                remote: Remote::Unsupported,
                precondition: Precondition::None,
            },
            Err(e) => return Err(e),
        };

        // Deletion loses against edits this client has not seen, and a folder
        // is kept while it still has contents.
        let keep = match &fetched.remote {
            Remote::Item(remote) => {
                // Newer than anything this client had: someone else's edit. (Not
                // just newer than the last sync, which would include our own
                // uploads whose responses were lost.)
                let sync_time = self.db().sync_time(target, item.item_type, id)?;
                let seen = sync_time.unwrap_or(0).max(item.item_updated_time);
                match remote.as_ref() {
                    AnyItem::Note(_) | AnyItem::Resource(_) => remote.updated_time() > seen,
                    AnyItem::Folder(_) => self.db().folder_has_children(id)?,
                    AnyItem::Tag(_) | AnyItem::NoteTag(_) | AnyItem::Revision(_) => false,
                }
            }
            _ => false,
        };

        // A folder already gone from the target that still has contents here
        // (notes kept above because of edits made elsewhere): recreate it.
        let recreate = item.item_type == ItemType::Folder
            && matches!(fetched.remote, Remote::Missing)
            && self.db().folder_has_children(id)?;

        if keep {
            let Remote::Item(remote) = &fetched.remote else {
                unreachable!()
            };
            self.apply_remote(remote, None)?;
            run.report.kept += 1;
        } else if recreate {
            if let Ok(Some(folder @ AnyItem::Folder(_))) = AnyItem::parse(&item.item_data) {
                self.db().write_any(&folder)?;
                if !self
                    .push(&folder, None, false, &Precondition::Absent)
                    .await?
                {
                    return Ok(false);
                }
                run.report.kept += 1;
            }
        } else {
            if !matches!(fetched.remote, Remote::Missing) {
                if !self.storage.delete_if(&path, &fetched.precondition).await? {
                    return Ok(false);
                }
                run.report.deleted_remote += 1;
            }
            if item.item_type == ItemType::Resource {
                self.storage.delete(&resource_blob_path(id)).await?;
            }
            self.db().remove_sync_item(target, item.item_type, id)?;
        }
        self.db().remove_deleted_item(target, item.item_type, id)?;
        Ok(true)
    }

    // ---------------------------------------------------------------------
    // Step 3: apply remote changes

    async fn delta_step(&self, run: &mut Run) -> Result<()> {
        let key = context_key(self.target());
        let mut context = self.db().setting(&key)?;
        // The cursor is only saved while every change so far was applied, so
        // that failed items are retried by the next sync.
        let mut all_applied = true;
        // Folder deletions wait until everything else is applied, as the
        // deletions of their contents may come later, even in a later page.
        let mut deleted_folders = Vec::new();
        let mut received = 0;
        loop {
            self.tick(run).await?;
            let synced_ids = self.db().synced_item_ids(self.target())?;
            let page = self.storage.delta(context.as_deref(), &synced_ids).await?;
            let (deletions, updates): (Vec<_>, Vec<_>) =
                page.changes.iter().partition(|c| c.deleted);
            for change in updates {
                received += 1;
                self.report_progress(SyncStep::Download, received, 0);
                self.tick(run).await?;
                let result = self.apply_change(run, change).await;
                all_applied &= self.record(run, result)?;
            }
            for change in deletions {
                self.tick(run).await?;
                self.apply_remote_deletion(run, &change.item_id, &mut deleted_folders)?;
            }
            context = Some(page.context);
            if all_applied && deleted_folders.is_empty() {
                self.db()
                    .set_setting(&key, context.as_deref().unwrap_or_default())?;
            }
            if !page.has_more {
                break;
            }
        }

        if !deleted_folders.is_empty() {
            self.apply_folder_deletions(run, deleted_folders).await?;
            if all_applied {
                self.db()
                    .set_setting(&key, context.as_deref().unwrap_or_default())?;
            }
        }
        Ok(())
    }

    async fn apply_change(&self, run: &mut Run, change: &DeltaChange) -> Result<()> {
        let Fetched {
            remote,
            precondition,
        } = self.fetch_remote(&change.path).await?;
        let remote = match remote {
            Remote::Item(item) => *item,
            // Deleted since it was listed, or a type this version ignores.
            Remote::Missing | Remote::Unsupported => return Ok(()),
        };
        let (item_type, id) = (remote.item_type(), remote.id().to_string());
        let local = self.db().load_any(item_type, &id)?;
        let sync_time = self.db().sync_time(self.target(), item_type, &id)?;

        let Some(local) = local else {
            self.apply_remote(&remote, None)?;
            run.report.created_local += 1;
            return Ok(());
        };
        let last = sync_time.unwrap_or(0);
        if remote.updated_time() > last {
            if local.updated_time() > last {
                // Changed locally while this sync was running.
                self.resolve_conflict(run, local, remote, sync_time).await?;
            } else {
                self.apply_remote(&remote, Some(&local))?;
                run.report.updated_local += 1;
            }
        } else if remote.updated_time() == last && local.updated_time() == last && remote != local {
            // Same version, different metadata: re-uploaded by another client
            // (e.g. after switching encryption on or off, which changes how a
            // resource's data is stored but not the version).
            self.apply_remote(&remote, Some(&local))?;
            run.report.updated_local += 1;
        } else if remote.updated_time() < last || sync_time.is_none() {
            // The target holds an older version than ours: another client that
            // had not seen our change restored the item (e.g. a folder it kept
            // for its contents). Or the item exists on both sides but was never
            // linked. Either way ours is newer: send it again.
            let force_blob = matches!(
                (&local, &remote),
                (AnyItem::Resource(l), AnyItem::Resource(r)) if l.blob_updated_time != r.blob_updated_time
            );
            if self
                .push(
                    &local,
                    Some(remote.updated_time()),
                    force_blob,
                    &precondition,
                )
                .await?
            {
                run.report.updated_remote += 1;
            }
        }
        Ok(())
    }

    fn apply_remote_deletion(
        &self,
        run: &mut Run,
        id: &str,
        deleted_folders: &mut Vec<Folder>,
    ) -> Result<()> {
        let local = self.db().find_any(id)?;
        match local {
            Some(AnyItem::Folder(folder)) => deleted_folders.push(folder),
            Some(local) => {
                if let AnyItem::Note(note) = &local {
                    let sync_time = self.db().sync_time(self.target(), ItemType::Note, id)?;
                    if sync_time.is_none_or(|t| note.updated_time > t) {
                        self.create_conflict_note(note)?;
                        run.report.conflicts += 1;
                    }
                }
                self.delete_local(run, &local)?;
            }
            None => {
                let db = self.db();
                for t in db.synced_types(self.target(), id)? {
                    db.remove_sync_item(self.target(), t, id)?;
                }
            }
        }
        Ok(())
    }

    /// Deletes folders deleted remotely, deepest first. A folder that still
    /// has contents here (for example a note another client did not know
    /// about) is uploaded again instead.
    async fn apply_folder_deletions(&self, run: &mut Run, folders: Vec<Folder>) -> Result<()> {
        let mut folders = folders
            .into_iter()
            .map(|f| Ok((self.db().folder_depth(&f.id)?, f)))
            .collect::<Result<Vec<_>>>()?;
        folders.sort_by_key(|(depth, _)| Reverse(*depth));
        for (_, folder) in folders {
            self.tick(run).await?;
            if self.db().folder_has_children(&folder.id)? {
                // If it was recreated meanwhile, the next delta brings it.
                self.push(&AnyItem::Folder(folder), None, false, &Precondition::Absent)
                    .await?;
                run.report.kept += 1;
            } else {
                self.delete_local(run, &AnyItem::Folder(folder))?;
            }
        }
        Ok(())
    }

    /// Makes sure every synced resource has its data on both sides: downloads
    /// what is missing locally and uploads what is missing on the target. The
    /// latter repairs data lost when a resource was deleted by one client
    /// while another was re-creating it.
    async fn sync_blobs(&self, run: &mut Run) -> Result<()> {
        let resources = self.db().all::<Resource>()?;
        let synced = self.db().synced_item_ids(self.target())?;
        let resources: Vec<Resource> = resources
            .into_iter()
            .filter(|r| synced.contains(&r.id))
            .collect();
        if resources.is_empty() {
            return Ok(());
        }
        let on_target: std::collections::HashSet<String> = self
            .storage
            .list(RESOURCE_DIR)
            .await?
            .into_iter()
            .filter(|s| !s.is_dir)
            .filter_map(|s| s.path.rsplit('/').next().map(str::to_string))
            .collect();

        let total = resources.len();
        for (i, r) in resources.into_iter().enumerate() {
            self.report_progress(SyncStep::Attachments, i, total);
            let path = self.resource_path(&r);
            let expected = r.extra.get(BLOB_HASH_PROP);
            match (path.exists(), on_target.contains(&r.id)) {
                (true, true) => {}
                (true, false) => {
                    // Only data matching the metadata is ours to restore.
                    if expected.is_some_and(|h| *h != file_sha256_blocking(&path)) {
                        run.report.pending_blobs += 1;
                        continue;
                    }
                    self.tick(run).await?;
                    tracing::info!("re-uploading missing data of resource {}", r.id);
                    self.upload_blob(&r, &path).await?;
                    run.report.updated_remote += 1;
                }
                (false, true) => {
                    self.tick(run).await?;
                    let downloaded = match self.download_blob(&r, &path).await? {
                        Download::Done => true,
                        Download::Missing => false,
                        // Our data (it has a hash) that cannot be decrypted was
                        // corrupted by a race: remove it so its owner re-uploads.
                        Download::Failed(_) if expected.is_some() => {
                            self.storage.delete(&resource_blob_path(&r.id)).await?;
                            run.report.pending_blobs += 1;
                            continue;
                        }
                        Download::Failed(msg) => {
                            run.report.errors.push(format!("resource {}: {msg}", r.id));
                            run.report.pending_blobs += 1;
                            continue;
                        }
                    };
                    if !downloaded {
                        run.report.pending_blobs += 1;
                    } else if expected.is_none() && !self.size_matches(&r, &path)? {
                        // Without a hash (data written by Joplin) only the size can
                        // be checked. Retry a few times in case the data is being
                        // replaced, then accept it: the size field may be wrong.
                        remove_file_if_exists(&path)?;
                        run.report.pending_blobs += 1;
                    } else if let Some(expected) = expected
                        && *expected != file_sha256(&path).await?
                    {
                        // Overwritten by a client that lost a race to update
                        // this resource. Remove it so that the client with
                        // the right data uploads it again.
                        tracing::warn!("data of resource {} does not match its metadata", r.id);
                        remove_file_if_exists(&path)?;
                        self.storage.delete(&resource_blob_path(&r.id)).await?;
                        run.report.pending_blobs += 1;
                    }
                }
                (false, false) => run.report.pending_blobs += 1,
            }
        }
        Ok(())
    }
}

impl Synchronizer {
    /// Whether a downloaded file has the size recorded in its metadata. After
    /// [`SIZE_MISMATCH_RETRIES`] mismatches the data is accepted anyway.
    fn size_matches(&self, r: &Resource, path: &std::path::Path) -> Result<bool> {
        let key = format!("sync.{}.sizeMismatch.{}", self.target(), r.id);
        let actual = std::fs::metadata(path)?.len() as i64;
        let db = self.db();
        if r.size < 0 || actual == r.size {
            db.delete_setting(&key)?;
            return Ok(true);
        }
        let attempts: u32 = db.setting(&key)?.and_then(|v| v.parse().ok()).unwrap_or(0) + 1;
        if attempts >= SIZE_MISMATCH_RETRIES {
            db.delete_setting(&key)?;
            tracing::warn!(
                "resource {}: data is {actual} bytes but metadata says {}; accepting it",
                r.id,
                r.size
            );
            return Ok(true);
        }
        db.set_setting(&key, &attempts.to_string())?;
        tracing::warn!(
            "resource {}: data is {actual} bytes, expected {}; will retry",
            r.id,
            r.size
        );
        Ok(false)
    }
}

const SIZE_MISMATCH_RETRIES: u32 = 3;

/// Property recording the SHA-256 of a resource's data (an extension to the
/// Joplin format; other clients ignore it).
pub const BLOB_HASH_PROP: &str = "nisshoku_blob_sha256";

fn file_sha256_blocking(path: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut hasher = Sha256::new();
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match file.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => hasher.update(&buf[..n]),
        }
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

async fn file_sha256(path: &std::path::Path) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || file_sha256_blocking(&path))
        .await
        .map_err(|e| SyncError::Remote(format!("hashing failed: {e}")))
}

fn remove_file_if_exists(path: &std::path::Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}
