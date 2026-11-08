use std::collections::{HashMap, HashSet};
use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const INFO_FILE: &str = "info.json";
pub const RESOURCE_DIR: &str = ".resource";
pub const LOCK_DIR: &str = "locks";
pub const TEMP_DIR: &str = "temp";

pub fn item_path(id: &str) -> String {
    format!("{id}.md")
}

pub fn resource_blob_path(id: &str) -> String {
    format!("{RESOURCE_DIR}/{id}")
}

/// Returns the item ID if `path` is an item file at the root of the target.
pub fn item_id_from_path(path: &str) -> Option<&str> {
    let id = path.strip_suffix(".md")?;
    nisshoku_core::ids::is_valid_id(id).then_some(id)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteStat {
    /// Path relative to the target root, using `/` as separator.
    pub path: String,
    /// Last modification time, Unix milliseconds.
    pub updated_time: i64,
    pub is_dir: bool,
    /// Version tag, for backends that provide one. Used with `updated_time`
    /// to detect changes.
    pub etag: Option<String>,
}

/// Condition for a write or delete, used to avoid overwriting changes made by
/// another client since we read the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Precondition {
    None,
    /// The file must not exist.
    Absent,
    /// The file must still be at this version (as returned by `get_versioned`).
    Version(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeltaChange {
    pub path: String,
    pub item_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone)]
pub struct DeltaPage {
    pub changes: Vec<DeltaChange>,
    /// Opaque cursor to pass to the next [`RemoteStorage::delta`] call.
    pub context: String,
    pub has_more: bool,
}

/// A place items can be synced to. Paths are relative to the target root and
/// use `/` as separator. Missing files are reported as `None`, not as errors.
#[async_trait]
pub trait RemoteStorage: Send + Sync {
    async fn stat(&self, path: &str) -> Result<Option<RemoteStat>>;
    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, path: &str, data: &[u8]) -> Result<()>;
    /// Deletes a file. Deleting a missing file is not an error.
    async fn delete(&self, path: &str) -> Result<()>;
    /// Creates a directory if it does not exist.
    async fn mkdir(&self, path: &str) -> Result<()>;
    /// Lists the direct children of a directory (`""` for the root).
    async fn list(&self, dir: &str) -> Result<Vec<RemoteStat>>;

    /// Creates a directory, returning `false` if it already exists. Must be
    /// atomic: of several clients creating it at once, exactly one succeeds.
    async fn mkdir_exclusive(&self, path: &str) -> Result<bool>;

    /// Deletes a directory and its contents. Deleting a missing one is not an error.
    async fn delete_dir(&self, path: &str) -> Result<()>;

    /// Reads a file together with its version tag, for use in a [`Precondition`].
    async fn get_versioned(&self, path: &str) -> Result<Option<(Vec<u8>, Option<String>)>> {
        Ok(self.get(path).await?.map(|data| (data, None)))
    }

    /// Writes a file if `precondition` holds. Returns `false` if it did not.
    /// Backends that cannot check preconditions write unconditionally.
    async fn put_if(&self, path: &str, data: &[u8], precondition: &Precondition) -> Result<bool> {
        let _ = precondition;
        self.put(path, data).await?;
        Ok(true)
    }

    /// Deletes a file if `precondition` holds. Returns `false` if it did not.
    async fn delete_if(&self, path: &str, precondition: &Precondition) -> Result<bool> {
        let _ = precondition;
        self.delete(path).await?;
        Ok(true)
    }

    /// Downloads a file to `dest`. Returns `false` if the remote file is missing.
    async fn get_to_file(&self, path: &str, dest: &Path) -> Result<bool> {
        match self.get(path).await? {
            Some(data) => {
                write_file_atomic(dest, &data).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    async fn put_file(&self, path: &str, src: &Path) -> Result<()> {
        let data = tokio::fs::read(src).await?;
        self.put(path, &data).await
    }

    /// For backends whose modification times are coarse (whole seconds): the
    /// time (Unix ms, in the server's clock) from which files may still change
    /// without their listing signature changing. Such files are checked again
    /// by the next delta.
    fn unstable_since(&self) -> Option<i64> {
        None
    }

    /// Lists item changes since `context` (`None` for a first sync). Backends
    /// with a native change feed override this; the default compares full
    /// listings.
    ///
    /// `synced_ids` are the IDs of items known to have been on this target, so
    /// that backends without a change feed can report their deletion.
    async fn delta(
        &self,
        context: Option<&str>,
        synced_ids: &HashSet<String>,
    ) -> Result<DeltaPage> {
        basic_delta(self, context, synced_ids).await
    }
}

#[derive(Default, Serialize, Deserialize)]
struct BasicDeltaContext {
    /// Item path -> (updated_time, etag) at the previous delta, or `None` if
    /// the file was too recently modified for its signature to be trusted.
    files: HashMap<String, Option<(i64, Option<String>)>>,
}

/// Change detection by listing the root and comparing modification times with
/// the previous listing.
pub async fn basic_delta<S: RemoteStorage + ?Sized>(
    storage: &S,
    context: Option<&str>,
    synced_ids: &HashSet<String>,
) -> Result<DeltaPage> {
    let previous: BasicDeltaContext = context
        .and_then(|c| serde_json::from_str(c).ok())
        .unwrap_or_default();

    let mut current = BasicDeltaContext::default();
    let mut present = HashSet::new();
    let mut changes = Vec::new();
    let listing = storage.list("").await?;
    let unstable_since = storage.unstable_since();
    for stat in listing {
        if stat.is_dir {
            continue;
        }
        let Some(id) = item_id_from_path(&stat.path) else {
            continue;
        };
        present.insert(id.to_string());
        let signature = (stat.updated_time, stat.etag.clone());
        if previous.files.get(&stat.path) != Some(&Some(signature.clone())) {
            changes.push(DeltaChange {
                path: stat.path.clone(),
                item_id: id.to_string(),
                deleted: false,
            });
        }
        let trusted = unstable_since.is_none_or(|t| stat.updated_time < t);
        current
            .files
            .insert(stat.path, trusted.then_some(signature));
    }
    for id in synced_ids {
        if !present.contains(id) {
            changes.push(DeltaChange {
                path: item_path(id),
                item_id: id.clone(),
                deleted: true,
            });
        }
    }

    Ok(DeltaPage {
        changes,
        context: serde_json::to_string(&current).expect("serializable"),
        has_more: false,
    })
}

/// A unique suffix for temporary files, so that concurrent writers of the same
/// file never share one. Listings skip names ending in `.tmp`.
pub fn temp_suffix() -> String {
    format!(".{}.tmp", &nisshoku_core::ids::new_id()[..12])
}

/// Writes through a temporary file so that readers never see a partial file.
pub async fn write_file_atomic(dest: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(crate::storage::temp_suffix());
    tokio::fs::write(&tmp, data).await?;
    retry_sharing_violation(|| tokio::fs::rename(&tmp, dest)).await
}

/// Retries an operation that failed with "access denied". On Windows a file
/// that another process has open, or a directory being deleted, cannot be
/// renamed over, removed or created for a moment; the error is transient.
pub async fn retry_sharing_violation<T, F, Fut>(mut op: F) -> std::io::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::io::Result<T>>,
{
    const ATTEMPTS: u64 = 20;
    for attempt in 1.. {
        match op().await {
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && attempt < ATTEMPTS => {
                tokio::time::sleep(std::time::Duration::from_millis(5 * attempt)).await;
            }
            result => return result,
        }
    }
    unreachable!()
}
