//! Lock files that keep clients from syncing while a target is being upgraded.
//!
//! Each syncing client writes `locks/<type>_<clientType>_<clientId>.json` and
//! refreshes it periodically. A lock older than [`LOCK_TTL_MS`] is considered
//! stale. Several clients may hold sync locks at the same time; an exclusive
//! lock (taken for target upgrades) excludes everything else.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SyncError};
use crate::storage::{LOCK_DIR, RemoteStorage};
use nisshoku_core::time::now_ms;

pub const LOCK_TTL_MS: i64 = 3 * 60 * 1000;
/// How often a held lock is refreshed.
pub const LOCK_REFRESH_MS: i64 = 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "i64", try_from = "i64")]
pub enum LockType {
    Sync = 1,
    Exclusive = 2,
}

impl From<LockType> for i64 {
    fn from(t: LockType) -> i64 {
        t as i64
    }
}

impl TryFrom<i64> for LockType {
    type Error = String;
    fn try_from(v: i64) -> std::result::Result<Self, String> {
        match v {
            1 => Ok(LockType::Sync),
            2 => Ok(LockType::Exclusive),
            _ => Err(format!("unknown lock type {v}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lock {
    #[serde(rename = "type")]
    pub lock_type: LockType,
    pub client_type: String,
    pub client_id: String,
    pub updated_time: i64,
}

impl Lock {
    pub fn file_path(&self) -> String {
        format!(
            "{LOCK_DIR}/{}_{}_{}.json",
            self.lock_type as i64, self.client_type, self.client_id
        )
    }

    pub fn is_active(&self, now: i64) -> bool {
        now - self.updated_time < LOCK_TTL_MS
    }
}

/// Parses `<type>_<clientType>_<clientId>.json`.
fn parse_lock_name(name: &str) -> Option<(LockType, String, String)> {
    let stem = name.strip_suffix(".json")?;
    let mut parts = stem.splitn(3, '_');
    let lock_type = LockType::try_from(parts.next()?.parse::<i64>().ok()?).ok()?;
    let client_type = parts.next()?.to_string();
    let client_id = parts.next()?.to_string();
    Some((lock_type, client_type, client_id))
}

pub async fn list_locks(storage: &dyn RemoteStorage) -> Result<Vec<Lock>> {
    let mut out = Vec::new();
    for stat in storage.list(LOCK_DIR).await? {
        if stat.is_dir {
            continue;
        }
        let name = stat.path.rsplit('/').next().unwrap_or(&stat.path);
        let Some((lock_type, client_type, client_id)) = parse_lock_name(name) else {
            continue;
        };
        // The content's timestamp is preferred: some backends do not preserve
        // modification times reliably.
        let updated_time = match storage.get(&stat.path).await? {
            Some(data) => serde_json::from_slice::<Lock>(&data)
                .map(|l| l.updated_time)
                .unwrap_or(stat.updated_time),
            None => continue,
        };
        out.push(Lock {
            lock_type,
            client_type,
            client_id,
            updated_time,
        });
    }
    Ok(out)
}

async fn write_lock(storage: &dyn RemoteStorage, lock: &Lock) -> Result<()> {
    let json = serde_json::to_vec(lock).expect("serializable");
    storage.put(&lock.file_path(), &json).await
}

/// Takes a sync lock, failing if another client holds an exclusive lock.
pub async fn acquire_sync_lock(
    storage: &dyn RemoteStorage,
    client_type: &str,
    client_id: &str,
) -> Result<Lock> {
    let now = now_ms();
    let blocking = list_locks(storage).await?.into_iter().find(|l| {
        l.lock_type == LockType::Exclusive && l.client_id != client_id && l.is_active(now)
    });
    if let Some(l) = blocking {
        return Err(SyncError::Locked(format!(
            "exclusive lock held by {} client {}",
            l.client_type, l.client_id
        )));
    }
    let lock = Lock {
        lock_type: LockType::Sync,
        client_type: client_type.into(),
        client_id: client_id.into(),
        updated_time: now,
    };
    write_lock(storage, &lock).await?;
    Ok(lock)
}

pub async fn refresh_lock(storage: &dyn RemoteStorage, lock: &mut Lock) -> Result<()> {
    lock.updated_time = now_ms();
    write_lock(storage, lock).await
}

pub async fn release_lock(storage: &dyn RemoteStorage, lock: &Lock) -> Result<()> {
    storage.delete(&lock.file_path()).await
}

// -------------------------------------------------------------------------
// Write mutex

/// Directory whose existence means a client is syncing. Joplin's sync locks
/// let clients sync at the same time and rely on the backend to reject
/// conflicting writes; Nextcloud's conditional requests turned out not to be
/// atomic (two writes with the same `If-Match` can both succeed), so clients
/// of this app additionally take turns. Directory creation is atomic.
pub const MUTEX_DIR: &str = ".nisshoku/sync";
const MUTEX_OWNER: &str = ".nisshoku/sync/owner.json";

#[derive(Debug, Clone)]
pub struct MutexTiming {
    /// A mutex whose owner file has not changed for this long is abandoned.
    pub stale_after: std::time::Duration,
    /// How often the holder rewrites its owner file.
    pub refresh_every: std::time::Duration,
    /// How long to wait for another client before giving up.
    pub max_wait: std::time::Duration,
}

impl Default for MutexTiming {
    fn default() -> Self {
        Self {
            stale_after: std::time::Duration::from_secs(60),
            refresh_every: std::time::Duration::from_secs(15),
            max_wait: std::time::Duration::from_secs(150),
        }
    }
}

pub struct WriteMutex {
    client_id: String,
    refreshed: std::time::Instant,
    timing: MutexTiming,
}

async fn write_owner(storage: &dyn RemoteStorage, client_id: &str) -> Result<()> {
    let owner = serde_json::json!({ "clientId": client_id, "updatedTime": now_ms(), "nonce": nisshoku_core::ids::new_id() });
    storage.put(MUTEX_OWNER, owner.to_string().as_bytes()).await
}

/// Errors worth waiting out (as opposed to bad credentials or cancellation).
fn is_transient(e: &SyncError) -> bool {
    matches!(e, SyncError::Remote(_) | SyncError::Io(_))
}

impl WriteMutex {
    /// Waits until no other client holds the mutex, then takes it.
    pub async fn acquire(
        storage: &dyn RemoteStorage,
        client_id: &str,
        timing: MutexTiming,
    ) -> Result<Self> {
        let start = std::time::Instant::now();
        // The owner file as last seen, and since when it has not changed.
        let mut watched: Option<(Option<Vec<u8>>, std::time::Instant)> = None;
        let mut delay = std::time::Duration::from_millis(50);
        storage.mkdir(".nisshoku").await?;
        loop {
            // Clients compete for these paths, and Nextcloud answers some of
            // the collisions with `423 Locked`: a failed step means "busy".
            match Self::attempt(storage, client_id, &timing, &mut watched).await {
                Ok(true) => {
                    return Ok(Self {
                        client_id: client_id.into(),
                        refreshed: std::time::Instant::now(),
                        timing,
                    });
                }
                Ok(false) => {}
                Err(e) if is_transient(&e) => tracing::debug!("sync mutex busy: {e}"),
                Err(e) => return Err(e),
            }
            if start.elapsed() > timing.max_wait {
                return Err(SyncError::Locked(
                    "another device is syncing; try again shortly".into(),
                ));
            }
            let jitter = std::time::Duration::from_millis(
                nisshoku_core::ids::new_id().as_bytes()[0] as u64 % 50,
            );
            tokio::time::sleep(delay + jitter).await;
            delay = (delay * 2).min(std::time::Duration::from_secs(2));
        }
    }

    /// One attempt at taking the mutex; `false` means it is held by someone else.
    async fn attempt(
        storage: &dyn RemoteStorage,
        client_id: &str,
        timing: &MutexTiming,
        watched: &mut Option<(Option<Vec<u8>>, std::time::Instant)>,
    ) -> Result<bool> {
        if storage.mkdir_exclusive(MUTEX_DIR).await? {
            if let Err(e) = write_owner(storage, client_id).await {
                // Without an owner file the mutex would only expire after
                // `stale_after`; give it back instead.
                let _ = storage.delete_dir(MUTEX_DIR).await;
                return Err(e);
            }
            return Ok(true);
        }
        let owner = storage.get(MUTEX_OWNER).await?;
        let ours = owner
            .as_deref()
            .and_then(|d| serde_json::from_slice::<serde_json::Value>(d).ok())
            .is_some_and(|v| v["clientId"] == client_id);
        let stale = match &*watched {
            Some((seen, since)) if *seen == owner => since.elapsed() > timing.stale_after,
            _ => {
                *watched = Some((owner, std::time::Instant::now()));
                false
            }
        };
        // Left behind by a sync of ours that failed to release it, or abandoned.
        if ours || stale {
            storage.delete_dir(MUTEX_DIR).await?;
            *watched = None;
        }
        Ok(false)
    }

    /// Rewrites the owner file when due. A failed refresh is not fatal: the
    /// mutex only goes stale after `stale_after`, and the next call retries.
    pub async fn refresh_if_due(&mut self, storage: &dyn RemoteStorage) -> Result<()> {
        if self.refreshed.elapsed() >= self.timing.refresh_every {
            match write_owner(storage, &self.client_id).await {
                Ok(()) => self.refreshed = std::time::Instant::now(),
                Err(e) if is_transient(&e) => tracing::debug!("sync mutex refresh failed: {e}"),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Gives the mutex back. A failure is retried a few times, since other
    /// clients would otherwise wait until the mutex goes stale.
    pub async fn release(self, storage: &dyn RemoteStorage) -> Result<()> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match storage.delete_dir(MUTEX_DIR).await {
                Err(e) if is_transient(&e) && attempt < 5 => {
                    tracing::debug!("could not release the sync mutex: {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(200 * attempt)).await;
                }
                result => return result,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_file_format() {
        let lock = Lock {
            lock_type: LockType::Sync,
            client_type: "desktop".into(),
            client_id: "abc".into(),
            updated_time: 123,
        };
        assert_eq!(lock.file_path(), "locks/1_desktop_abc.json");
        assert_eq!(
            serde_json::to_string(&lock).unwrap(),
            r#"{"type":1,"clientType":"desktop","clientId":"abc","updatedTime":123}"#
        );
        assert_eq!(
            parse_lock_name("2_mobile_abc.json"),
            Some((LockType::Exclusive, "mobile".into(), "abc".into()))
        );
    }
}
