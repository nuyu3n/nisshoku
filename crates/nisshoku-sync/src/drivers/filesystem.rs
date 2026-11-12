use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;

use crate::error::{Result, SyncError};
use crate::storage::{
    Precondition, RemoteStat, RemoteStorage, retry_sharing_violation as retry, write_file_atomic,
};

/// A sync target in a local directory, e.g. a folder synced by a desktop
/// cloud client or a network share. Same layout as Joplin's "File system" target.
///
/// Conditional writes hold a lock file (`<name>.lock`, created atomically)
/// while checking and writing, so they are exclusive between processes using
/// the same file system, including network shares. A folder kept in sync by
/// a cloud client (Dropbox, OneDrive...) and used from several machines at the
/// same time cannot be protected this way; such services keep their own
/// conflict copies.
pub struct FileSystemStorage {
    root: PathBuf,
}

impl FileSystemStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn full_path(&self, path: &str) -> Result<PathBuf> {
        let mut full = self.root.clone();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            if part == ".." || part.contains('\\') || part.contains(':') {
                return Err(SyncError::Remote(format!("invalid path: {path:?}")));
            }
            full.push(part);
        }
        Ok(full)
    }
}

/// A lock older than this is considered abandoned by a crashed process.
const STALE_LOCK: std::time::Duration = std::time::Duration::from_secs(30);

struct FileLock(PathBuf);

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl FileSystemStorage {
    async fn lock(&self, path: &str) -> Result<FileLock> {
        let mut lock_path = self.full_path(path)?.into_os_string();
        lock_path.push(".lock");
        let lock_path = PathBuf::from(lock_path);
        if let Some(parent) = lock_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => return Ok(FileLock(lock_path)),
                // A lock file being deleted reports "access denied" on Windows.
                Err(e)
                    if matches!(
                        e.kind(),
                        ErrorKind::AlreadyExists | ErrorKind::PermissionDenied
                    ) =>
                {
                    let stale = std::fs::metadata(&lock_path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age > STALE_LOCK);
                    if stale {
                        let _ = std::fs::remove_file(&lock_path);
                        continue;
                    }
                    if std::time::Instant::now() > deadline {
                        return Err(SyncError::Remote(format!(
                            "{path} stays locked by another process"
                        )));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                // Windows reports a file being deleted as "access denied".
                Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    async fn check(&self, path: &str, precondition: &Precondition) -> Result<bool> {
        let meta = not_found_as_none(tokio::fs::metadata(self.full_path(path)?).await)?;
        Ok(match precondition {
            Precondition::None => true,
            Precondition::Absent => meta.is_none(),
            Precondition::Version(v) => meta.is_some_and(|m| &version_of(&m) == v),
        })
    }
}

fn to_stat(path: String, meta: &std::fs::Metadata) -> RemoteStat {
    let updated_time = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64);
    RemoteStat {
        path,
        updated_time,
        is_dir: meta.is_dir(),
        etag: Some(version_of(meta)),
    }
}

/// Sub-millisecond precision helps tell apart writes in the same millisecond.
///
/// Linux file systems keep times in coarse steps (the kernel tick, a few
/// milliseconds), so two writes of the same size in a row can share both. As
/// every write replaces the file (a new file renamed over it, see
/// `write_file_atomic`), the file's inode and change time tell them apart there.
fn version_of(meta: &std::fs::Metadata) -> String {
    let nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!(
            "{nanos}-{}-{}-{}.{}",
            meta.len(),
            meta.ino(),
            meta.ctime(),
            meta.ctime_nsec()
        )
    }
    #[cfg(not(unix))]
    format!("{nanos}-{}", meta.len())
}

fn not_found_as_none<T>(r: std::io::Result<T>) -> Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[async_trait]
impl RemoteStorage for FileSystemStorage {
    async fn stat(&self, path: &str) -> Result<Option<RemoteStat>> {
        let meta = not_found_as_none(tokio::fs::metadata(self.full_path(path)?).await)?;
        Ok(meta.map(|m| to_stat(path.to_string(), &m)))
    }

    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        {
            let full = self.full_path(path)?;
            not_found_as_none(retry(|| tokio::fs::read(&full)).await)
        }
    }

    async fn put(&self, path: &str, data: &[u8]) -> Result<()> {
        Ok(write_file_atomic(&self.full_path(path)?, data).await?)
    }

    async fn delete(&self, path: &str) -> Result<()> {
        not_found_as_none({
            let full = self.full_path(path)?;
            retry(|| tokio::fs::remove_file(&full)).await
        })?;
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        let full = self.full_path(path)?;
        Ok(retry(|| tokio::fs::create_dir_all(&full)).await?)
    }

    async fn mkdir_exclusive(&self, path: &str) -> Result<bool> {
        let full = self.full_path(path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        match retry(|| tokio::fs::create_dir(&full)).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete_dir(&self, path: &str) -> Result<()> {
        not_found_as_none({
            let full = self.full_path(path)?;
            retry(|| tokio::fs::remove_dir_all(&full)).await
        })?;
        Ok(())
    }

    async fn list(&self, dir: &str) -> Result<Vec<RemoteStat>> {
        let Some(mut entries) = not_found_as_none(tokio::fs::read_dir(self.full_path(dir)?).await)?
        else {
            return Ok(Vec::new());
        };
        let prefix = dir.trim_matches('/');
        let mut out = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if name.ends_with(".tmp") || name.ends_with(".lock") {
                continue;
            }
            let Some(meta) = not_found_as_none(entry.metadata().await)? else {
                continue;
            };
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            out.push(to_stat(path, &meta));
        }
        Ok(out)
    }

    async fn get_versioned(&self, path: &str) -> Result<Option<(Vec<u8>, Option<String>)>> {
        let full = self.full_path(path)?;
        // Stat before reading: if the file changes in between, the version is
        // older than the data and a later conditional write fails safely.
        let Some(meta) = not_found_as_none(tokio::fs::metadata(&full).await)? else {
            return Ok(None);
        };
        let data = not_found_as_none(retry(|| tokio::fs::read(&full)).await)?;
        Ok(data.map(|d| (d, Some(version_of(&meta)))))
    }

    async fn put_if(&self, path: &str, data: &[u8], precondition: &Precondition) -> Result<bool> {
        let _lock = self.lock(path).await?;
        if !self.check(path, precondition).await? {
            return Ok(false);
        }
        self.put(path, data).await?;
        Ok(true)
    }

    async fn delete_if(&self, path: &str, precondition: &Precondition) -> Result<bool> {
        let _lock = self.lock(path).await?;
        if !self.check(path, precondition).await? {
            return Ok(false);
        }
        self.delete(path).await?;
        Ok(true)
    }

    async fn get_to_file(&self, path: &str, dest: &Path) -> Result<bool> {
        let src = self.full_path(path)?;
        if not_found_as_none(tokio::fs::metadata(&src).await)?.is_none() {
            return Ok(false);
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut tmp = dest.as_os_str().to_owned();
        tmp.push(crate::storage::temp_suffix());
        retry(|| tokio::fs::copy(&src, &tmp)).await?;
        retry(|| tokio::fs::rename(&tmp, dest)).await?;
        Ok(true)
    }

    async fn put_file(&self, path: &str, src: &Path) -> Result<()> {
        let dest = self.full_path(path)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut tmp = dest.as_os_str().to_owned();
        tmp.push(crate::storage::temp_suffix());
        tokio::fs::copy(src, &tmp).await?;
        retry(|| tokio::fs::rename(&tmp, &dest)).await?;
        Ok(())
    }
}
