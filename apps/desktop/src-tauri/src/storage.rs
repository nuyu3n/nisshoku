//! Keeping the data safe and tidy: backups (automatic, of the database, and
//! complete ones on demand) and attachments no note uses any more.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use nisshoku_core::{Database, ItemType, Note, Resource, time::now_ms};
use serde::Serialize;
use tauri::State;

use crate::notes::{CmdResult, err};
use crate::state::AppState;

/// Daily copies of the database kept in the profile.
const KEEP_DAILY: usize = 7;

fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// Once a day (at start): a copy of the database in `backups/`, the last
/// [`KEEP_DAILY`] kept. Attachments are not copied (they are large and
/// rarely change); `backup_now` makes complete backups.
pub fn daily_backup(db: &Database, data_dir: &Path) -> std::io::Result<()> {
    let dir = backups_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let path = dir.join(format!("database-{today}.sqlite"));
    if !path.exists() {
        db.backup_to(&path).map_err(std::io::Error::other)?;
    }
    let mut old: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("database-") && n.ends_with(".sqlite"))
        })
        .collect();
    old.sort();
    let excess = old.len().saturating_sub(KEEP_DAILY);
    for p in &old[..excess] {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<u64> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        // Half-written downloads start with a dot.
        if name.to_string_lossy().starts_with('.') || !entry.file_type()?.is_file() {
            continue;
        }
        std::fs::copy(entry.path(), to.join(&name))?;
        n += 1;
    }
    Ok(n)
}

/// A complete backup in `dir`: a folder holding the database and the
/// attachments, which is itself a profile (open it with `NISSHOKU_PROFILE`,
/// or copy it over the profile to restore). Returns the folder.
#[tauri::command]
pub fn backup_now(state: State<'_, AppState>, dir: String) -> CmdResult<String> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let target = PathBuf::from(dir).join(format!("nisshoku-backup-{stamp}"));
    std::fs::create_dir_all(&target).map_err(err)?;
    state
        .db()
        .backup_to(&target.join("database.sqlite"))
        .map_err(err)?;
    copy_dir(&state.resource_dir, &target.join("resources")).map_err(err)?;
    Ok(target.to_string_lossy().into_owned())
}

/// Opens the folder of the automatic backups.
#[tauri::command]
pub fn open_backups<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = backups_dir(&state.data_dir);
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    database_bytes: u64,
    resource_bytes: u64,
    resource_count: usize,
    unused_count: usize,
    unused_bytes: u64,
}

/// Resource ids linked (`:/id`) from any note, the trash and conflicts included.
fn used_resources(db: &Database) -> CmdResult<HashSet<String>> {
    let mut used = HashSet::new();
    for n in db.all::<Note>().map_err(err)? {
        let mut rest = n.body.as_str();
        while let Some(i) = rest.find(":/") {
            let id = &rest[i + 2..];
            let len = id.bytes().take_while(|b| b.is_ascii_hexdigit()).count();
            if len == 32 {
                used.insert(id[..32].to_string());
            }
            rest = id;
        }
    }
    Ok(used)
}

/// Attachments no note links to, added more than a day ago (one just pasted
/// may not be saved in its note yet).
fn unused(db: &Database) -> CmdResult<Vec<Resource>> {
    let used = used_resources(db)?;
    let day_ago = now_ms() - 86_400_000;
    Ok(db
        .all::<Resource>()
        .map_err(err)?
        .into_iter()
        .filter(|r| !used.contains(&r.id) && r.created_time < day_ago)
        .collect())
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

#[tauri::command]
pub fn storage_info(state: State<'_, AppState>) -> CmdResult<StorageInfo> {
    let db = state.db();
    let resources = db.all::<Resource>().map_err(err)?;
    let unused = unused(&db)?;
    let size = |r: &Resource| file_size(&state.resource_dir.join(r.file_name()));
    Ok(StorageInfo {
        database_bytes: file_size(&state.data_dir.join("database.sqlite"))
            + file_size(&state.data_dir.join("database.sqlite-wal")),
        resource_bytes: resources.iter().map(size).sum(),
        resource_count: resources.len(),
        unused_count: unused.len(),
        unused_bytes: unused.iter().map(size).sum(),
    })
}

/// Deletes the attachments no note uses (here and, by the sync, everywhere).
#[tauri::command]
pub fn delete_unused_resources(state: State<'_, AppState>) -> CmdResult<usize> {
    let db = state.db();
    let list = unused(&db)?;
    for r in &list {
        db.delete(ItemType::Resource, &r.id).map_err(err)?;
        let _ = std::fs::remove_file(state.resource_dir.join(r.file_name()));
    }
    Ok(list.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_count_as_uses() {
        let db = Database::open_in_memory().unwrap();
        let id = "0123456789abcdef0123456789abcdef";
        let mut n = Note::new("", "t", &format!("![](:/{id}) and :/short"));
        db.save(&mut n).unwrap();
        let used = used_resources(&db).unwrap();
        assert!(used.contains(id));
        assert_eq!(used.len(), 1);
    }

    #[test]
    fn daily_backups_are_kept_for_a_week() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let backups = backups_dir(dir.path());
        std::fs::create_dir_all(&backups).unwrap();
        for d in 1..=9 {
            std::fs::write(backups.join(format!("database-2020-01-0{d}.sqlite")), b"").unwrap();
        }
        daily_backup(&db, dir.path()).unwrap();
        let left = std::fs::read_dir(&backups).unwrap().count();
        assert_eq!(left, KEEP_DAILY);
    }
}
