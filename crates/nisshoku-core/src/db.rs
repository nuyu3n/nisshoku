//! Local SQLite storage.
//!
//! Besides the item tables, two tables drive sync:
//!
//! - `sync_items` records, per sync target, the `updated_time` of the version of
//!   each item that was last exchanged with that target (`sync_time`). An item
//!   needs uploading when it has no row or its `updated_time` is newer.
//! - `deleted_items` records items deleted locally that still exist on a target.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ids::new_id;
use crate::item_type::ItemType;
use crate::models::{AnyItem, Folder, Model, Note, NoteTag, Resource, Revision, Tag};
use crate::time::now_ms;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletedItem {
    pub item_type: ItemType,
    pub item_id: String,
    pub deleted_time: i64,
    /// `updated_time` of the item when it was deleted: a remote version newer
    /// than this has changes this client never saw.
    pub item_updated_time: i64,
    /// The item as it was when deleted (serialized), so that it can be
    /// recreated if something that was kept still needs it.
    pub item_data: String,
}

pub struct Database {
    conn: Connection,
}

pub fn table_for(item_type: ItemType) -> Option<&'static str> {
    Some(match item_type {
        ItemType::Note => Note::TABLE,
        ItemType::Folder => Folder::TABLE,
        ItemType::Resource => Resource::TABLE,
        ItemType::Tag => Tag::TABLE,
        ItemType::NoteTag => NoteTag::TABLE,
        ItemType::Revision => Revision::TABLE,
        _ => return None,
    })
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    fn init(conn: Connection) -> Result<Self> {
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.ensure_table::<Folder>()?;
        self.ensure_table::<Note>()?;
        self.ensure_table::<Resource>()?;
        self.ensure_table::<Tag>()?;
        self.ensure_table::<NoteTag>()?;
        self.ensure_table::<Revision>()?;
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS notes_parent_id ON notes(parent_id);
             CREATE INDEX IF NOT EXISTS folders_parent_id ON folders(parent_id);
             CREATE INDEX IF NOT EXISTS note_tags_note_id ON note_tags(note_id);
             CREATE INDEX IF NOT EXISTS note_tags_tag_id ON note_tags(tag_id);
             CREATE INDEX IF NOT EXISTS revisions_item_id ON revisions(item_id);
             CREATE TABLE IF NOT EXISTS sync_items (
                 id INTEGER PRIMARY KEY,
                 sync_target INTEGER NOT NULL,
                 item_type INTEGER NOT NULL,
                 item_id TEXT NOT NULL,
                 sync_time INTEGER NOT NULL DEFAULT 0,
                 force_sync INTEGER NOT NULL DEFAULT 0,
                 UNIQUE(sync_target, item_type, item_id)
             );
             CREATE INDEX IF NOT EXISTS sync_items_item_id ON sync_items(item_id);
             CREATE TABLE IF NOT EXISTS deleted_items (
                 id INTEGER PRIMARY KEY,
                 sync_target INTEGER NOT NULL,
                 item_type INTEGER NOT NULL,
                 item_id TEXT NOT NULL,
                 deleted_time INTEGER NOT NULL,
                 item_updated_time INTEGER NOT NULL DEFAULT 0,
                 item_data TEXT NOT NULL DEFAULT '',
                 UNIQUE(sync_target, item_type, item_id)
             );
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;
        for (table, column, definition) in [
            (
                "deleted_items",
                "item_updated_time",
                "INTEGER NOT NULL DEFAULT 0",
            ),
            ("deleted_items", "item_data", "TEXT NOT NULL DEFAULT ''"),
            ("sync_items", "force_sync", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            let has_column: bool = self.conn.query_row(
                &format!(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1)"
                ),
                [column],
                |row| row.get(0),
            )?;
            if !has_column {
                self.conn.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                    [],
                )?;
            }
        }
        Ok(())
    }

    /// Creates the table for `M`, adding any columns missing from an older schema.
    fn ensure_table<M: Model>(&self) -> Result<()> {
        let columns: Vec<String> = M::FIELDS
            .iter()
            .map(|(name, kind)| {
                if *name == "id" {
                    "\"id\" TEXT PRIMARY KEY".to_string()
                } else {
                    format!("\"{name}\" {}", kind.sql_definition())
                }
            })
            .collect();
        self.conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS \"{}\" ({}, \"extra_props\" TEXT NOT NULL DEFAULT '{{}}')",
                M::TABLE,
                columns.join(", ")
            ),
            [],
        )?;

        let existing: HashSet<String> = {
            let mut stmt = self
                .conn
                .prepare(&format!("PRAGMA table_info(\"{}\")", M::TABLE))?;
            stmt.query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<_>>()?
        };
        for (name, kind) in M::FIELDS {
            if !existing.contains(*name) {
                self.conn.execute(
                    &format!(
                        "ALTER TABLE \"{}\" ADD COLUMN \"{name}\" {}",
                        M::TABLE,
                        kind.sql_definition()
                    ),
                    [],
                )?;
            }
        }
        if !existing.contains("extra_props") {
            self.conn.execute(
                &format!(
                    "ALTER TABLE \"{}\" ADD COLUMN \"extra_props\" TEXT NOT NULL DEFAULT '{{}}'",
                    M::TABLE
                ),
                [],
            )?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Generic item access

    pub fn load<M: Model>(&self, id: &str) -> Result<Option<M>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT * FROM \"{}\" WHERE id = ?1", M::TABLE),
                [id],
                |row| M::from_row(row),
            )
            .optional()?)
    }

    /// Loads items matching an SQL condition, e.g. `query::<Note>("parent_id = ?1", [id])`.
    pub fn query<M: Model, P: rusqlite::Params>(
        &self,
        condition: &str,
        params: P,
    ) -> Result<Vec<M>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT * FROM \"{}\" WHERE {condition}", M::TABLE))?;
        let rows = stmt.query_map(params, |row| M::from_row(row))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn all<M: Model>(&self) -> Result<Vec<M>> {
        self.query::<M, _>("1", [])
    }

    /// Saves a local modification: assigns an ID to new items and bumps the
    /// timestamps so that the item is picked up by the next sync.
    pub fn save<M: Model>(&self, item: &mut M) -> Result<()> {
        let existing = if item.id().is_empty() {
            item.set_id(new_id());
            None
        } else {
            self.load::<M>(item.id())?
        };
        // Strictly increasing, so a change is detected even if the clock went
        // backwards since the last sync.
        let t = match &existing {
            Some(e) => now_ms().max(e.updated_time() + 1),
            None => now_ms(),
        };
        item.touch(t, existing.is_none());
        self.write(item)
    }

    /// Saves a change of metadata (pin, order, notebook, app data…): synced
    /// like any change, but the time the user last edited the item
    /// (`user_updated_time`) stays, so lists sorted by it do not move.
    pub fn save_meta<M: Model>(&self, item: &mut M) -> Result<()> {
        if item.id().is_empty() {
            return self.save(item);
        }
        let t = match self.load::<M>(item.id())? {
            Some(e) => now_ms().max(e.updated_time() + 1),
            None => return self.save(item),
        };
        item.touch_meta(t);
        self.write(item)
    }

    /// Writes an item as is, without touching its timestamps.
    pub fn write<M: Model>(&self, item: &M) -> Result<()> {
        let mut columns: Vec<String> = M::FIELDS.iter().map(|(n, _)| format!("\"{n}\"")).collect();
        columns.push("\"extra_props\"".into());
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
        self.conn.execute(
            &format!(
                "INSERT OR REPLACE INTO \"{}\" ({}) VALUES ({})",
                M::TABLE,
                columns.join(", "),
                placeholders.join(", ")
            ),
            params_from_iter(item.sql_values()),
        )?;
        Ok(())
    }

    pub fn load_any(&self, item_type: ItemType, id: &str) -> Result<Option<AnyItem>> {
        Ok(match item_type {
            ItemType::Note => self.load::<Note>(id)?.map(AnyItem::Note),
            ItemType::Folder => self.load::<Folder>(id)?.map(AnyItem::Folder),
            ItemType::Resource => self.load::<Resource>(id)?.map(AnyItem::Resource),
            ItemType::Tag => self.load::<Tag>(id)?.map(AnyItem::Tag),
            ItemType::NoteTag => self.load::<NoteTag>(id)?.map(AnyItem::NoteTag),
            ItemType::Revision => self.load::<Revision>(id)?.map(AnyItem::Revision),
            _ => None,
        })
    }

    /// Finds an item by ID in any table.
    pub fn find_any(&self, id: &str) -> Result<Option<AnyItem>> {
        for t in ItemType::SYNCED {
            if let Some(item) = self.load_any(t, id)? {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    pub fn write_any(&self, item: &AnyItem) -> Result<()> {
        match item {
            AnyItem::Note(x) => self.write(x),
            AnyItem::Folder(x) => self.write(x),
            AnyItem::Resource(x) => self.write(x),
            AnyItem::Tag(x) => self.write(x),
            AnyItem::NoteTag(x) => self.write(x),
            AnyItem::Revision(x) => self.write(x),
        }
    }

    /// Deletes an item as a user action: children are deleted too (sub-folders
    /// and notes of a folder, tag links of a note or tag), and deletions are
    /// recorded so that sync removes the items from the targets.
    ///
    /// Resource files are not touched; that is up to the caller.
    pub fn delete(&self, item_type: ItemType, id: &str) -> Result<()> {
        self.delete_inner(item_type, id, &mut HashSet::new())
    }

    fn delete_inner(&self, item_type: ItemType, id: &str, seen: &mut HashSet<String>) -> Result<()> {
        // Folders whose parents form a loop (damaged data) end the recursion.
        if !seen.insert(id.to_string()) {
            return Ok(());
        }
        match item_type {
            ItemType::Folder => {
                for f in self.query::<Folder, _>("parent_id = ?1", [id])? {
                    self.delete_inner(ItemType::Folder, &f.id, seen)?;
                }
                for n in self.query::<Note, _>("parent_id = ?1", [id])? {
                    self.delete(ItemType::Note, &n.id)?;
                }
            }
            ItemType::Note => {
                for nt in self.query::<NoteTag, _>("note_id = ?1", [id])? {
                    self.delete(ItemType::NoteTag, &nt.id)?;
                }
            }
            ItemType::Tag => {
                for nt in self.query::<NoteTag, _>("tag_id = ?1", [id])? {
                    self.delete(ItemType::NoteTag, &nt.id)?;
                }
            }
            _ => {}
        }
        if let Some(item) = self.load_any(item_type, id)? {
            self.conn.execute(
                "INSERT OR REPLACE INTO deleted_items
                     (sync_target, item_type, item_id, deleted_time, item_updated_time, item_data)
                 SELECT sync_target, item_type, item_id, ?3, ?4, ?5
                 FROM sync_items WHERE item_type = ?1 AND item_id = ?2",
                params![
                    item_type.as_i64(),
                    id,
                    now_ms(),
                    item.updated_time(),
                    item.to_text()
                ],
            )?;
        }
        self.delete_row(item_type, id)
    }

    /// Deletes just the item's row, without cascading or recording the deletion.
    pub fn delete_row(&self, item_type: ItemType, id: &str) -> Result<()> {
        if let Some(table) = table_for(item_type) {
            self.conn
                .execute(&format!("DELETE FROM \"{table}\" WHERE id = ?1"), [id])?;
        }
        Ok(())
    }

    /// Whether a folder contains sub-folders or (non-conflict) notes.
    pub fn folder_has_children(&self, id: &str) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM folders WHERE parent_id = ?1)
                 OR EXISTS(SELECT 1 FROM notes WHERE parent_id = ?1 AND is_conflict = 0)",
            [id],
            |row| row.get(0),
        )?)
    }

    /// Number of ancestors of a folder.
    pub fn folder_depth(&self, id: &str) -> Result<usize> {
        let mut depth = 0;
        let mut seen = HashSet::new();
        let mut current = self.load::<Folder>(id)?;
        while let Some(f) = current {
            if f.parent_id.is_empty() || !seen.insert(f.id.clone()) {
                break;
            }
            current = self.load::<Folder>(&f.parent_id)?;
            if current.is_some() {
                depth += 1;
            }
        }
        Ok(depth)
    }

    /// Notes whose folder does not exist (for example because another client
    /// deleted it while this one moved a note into it).
    pub fn orphaned_notes(&self) -> Result<Vec<Note>> {
        self.query::<Note, _>(
            "is_conflict = 0 AND parent_id != '' AND parent_id NOT IN (SELECT id FROM folders)",
            [],
        )
    }

    // ---------------------------------------------------------------------
    // Sync bookkeeping

    /// Items whose current version has not been sent to `target`, parents first.
    /// Conflict copies are local only and never synced.
    pub fn items_needing_sync(&self, target: i64) -> Result<Vec<(ItemType, String)>> {
        let mut out = Vec::new();
        for t in ItemType::SYNCED {
            let table = table_for(t).unwrap();
            let extra = if t == ItemType::Note {
                "AND i.is_conflict = 0"
            } else {
                ""
            };
            let mut stmt = self.conn.prepare(&format!(
                "SELECT i.id FROM \"{table}\" i
                 LEFT JOIN sync_items s
                   ON s.item_id = i.id AND s.item_type = ?1 AND s.sync_target = ?2
                 WHERE (s.id IS NULL OR i.updated_time > s.sync_time OR s.force_sync = 1) {extra}
                 ORDER BY i.updated_time"
            ))?;
            let ids = stmt.query_map(params![t.as_i64(), target], |row| row.get::<_, String>(0))?;
            for id in ids {
                out.push((t, id?));
            }
        }
        Ok(out)
    }

    /// The `updated_time` of the version last exchanged with `target`, or
    /// `None` if the item has not been confirmed on the target.
    pub fn sync_time(&self, target: i64, item_type: ItemType, id: &str) -> Result<Option<i64>> {
        let t: Option<i64> = self
            .conn
            .query_row(
                "SELECT sync_time FROM sync_items WHERE sync_target = ?1 AND item_type = ?2 AND item_id = ?3",
                params![target, item_type.as_i64(), id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(t.filter(|t| *t > 0))
    }

    /// Records that an item may be on `target` (an upload is being attempted)
    /// without confirming a version, so that deleting it is tracked even if
    /// the upload's response is lost.
    pub fn mark_sync_pending(&self, target: i64, item_type: ItemType, id: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO sync_items (sync_target, item_type, item_id, sync_time) VALUES (?1, ?2, ?3, 0)",
            params![target, item_type.as_i64(), id],
        )?;
        Ok(())
    }

    pub fn set_sync_time(
        &self,
        target: i64,
        item_type: ItemType,
        id: &str,
        sync_time: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_items (sync_target, item_type, item_id, sync_time) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(sync_target, item_type, item_id)
             DO UPDATE SET sync_time = excluded.sync_time, force_sync = 0",
            params![target, item_type.as_i64(), id, sync_time],
        )?;
        Ok(())
    }

    /// Makes the next sync upload every item synced with `target` again (e.g.
    /// after turning encryption on or off), without marking them as changed.
    pub fn mark_all_for_reupload(&self, target: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE sync_items SET force_sync = 1 WHERE sync_target = ?1 AND sync_time > 0",
            [target],
        )?;
        Ok(())
    }

    pub fn remove_sync_item(&self, target: i64, item_type: ItemType, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM sync_items WHERE sync_target = ?1 AND item_type = ?2 AND item_id = ?3",
            params![target, item_type.as_i64(), id],
        )?;
        Ok(())
    }

    /// IDs of all items confirmed to have been exchanged with `target`.
    pub fn synced_item_ids(&self, target: i64) -> Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT item_id FROM sync_items WHERE sync_target = ?1 AND sync_time > 0")?;
        let ids = stmt.query_map([target], |row| row.get::<_, String>(0))?;
        Ok(ids.collect::<rusqlite::Result<_>>()?)
    }

    /// Item types recorded in `sync_items` for `id` on `target`.
    pub fn synced_types(&self, target: i64, id: &str) -> Result<Vec<ItemType>> {
        let mut stmt = self
            .conn
            .prepare("SELECT item_type FROM sync_items WHERE sync_target = ?1 AND item_id = ?2")?;
        let types = stmt.query_map(params![target, id], |row| row.get::<_, i64>(0))?;
        Ok(types
            .filter_map(|t| t.ok().and_then(ItemType::from_i64))
            .collect())
    }

    pub fn deleted_items(&self, target: i64) -> Result<Vec<DeletedItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_type, item_id, deleted_time, item_updated_time, item_data FROM deleted_items
             WHERE sync_target = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([target], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (t, item_id, deleted_time, item_updated_time, item_data) = row?;
            if let Some(item_type) = ItemType::from_i64(t) {
                out.push(DeletedItem {
                    item_type,
                    item_id,
                    deleted_time,
                    item_updated_time,
                    item_data,
                });
            }
        }
        Ok(out)
    }

    pub fn remove_deleted_item(&self, target: i64, item_type: ItemType, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM deleted_items WHERE sync_target = ?1 AND item_type = ?2 AND item_id = ?3",
            params![target, item_type.as_i64(), id],
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Settings

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// Writes a consistent copy of the database to `path` (which must not
    /// exist), while it stays in use.
    pub fn backup_to(&self, path: &Path) -> Result<()> {
        self.conn
            .execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    /// A random ID identifying this installation, used in sync locks.
    pub fn client_id(&self) -> Result<String> {
        if let Some(id) = self.setting("clientId")? {
            return Ok(id);
        }
        let id = new_id();
        self.set_setting("clientId", &id)?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_and_change_tracking() {
        let db = Database::open_in_memory().unwrap();
        let mut folder = Folder::new("", "Book");
        db.save(&mut folder).unwrap();
        let mut note = Note::new(&folder.id, "Chapter 1", "It was a dark night.");
        db.save(&mut note).unwrap();

        assert_eq!(db.load::<Note>(&note.id).unwrap().unwrap(), note);
        assert_eq!(db.items_needing_sync(1).unwrap().len(), 2);

        db.set_sync_time(1, ItemType::Folder, &folder.id, folder.updated_time)
            .unwrap();
        db.set_sync_time(1, ItemType::Note, &note.id, note.updated_time)
            .unwrap();
        assert!(db.items_needing_sync(1).unwrap().is_empty());

        let before = note.updated_time;
        note.body.push_str(" Edited.");
        db.save(&mut note).unwrap();
        assert!(note.updated_time > before);
        assert_eq!(
            db.items_needing_sync(1).unwrap(),
            vec![(ItemType::Note, note.id.clone())]
        );
    }

    #[test]
    fn delete_cascades_and_tracks_synced_items_only() {
        let db = Database::open_in_memory().unwrap();
        let mut folder = Folder::new("", "Book");
        db.save(&mut folder).unwrap();
        let mut synced = Note::new(&folder.id, "Synced", "");
        db.save(&mut synced).unwrap();
        let mut local_only = Note::new(&folder.id, "Local", "");
        db.save(&mut local_only).unwrap();
        db.set_sync_time(1, ItemType::Folder, &folder.id, 1)
            .unwrap();
        db.set_sync_time(1, ItemType::Note, &synced.id, 1).unwrap();

        db.delete(ItemType::Folder, &folder.id).unwrap();

        assert!(db.all::<Note>().unwrap().is_empty());
        let deleted: Vec<String> = db
            .deleted_items(1)
            .unwrap()
            .into_iter()
            .map(|d| d.item_id)
            .collect();
        assert_eq!(deleted.len(), 2);
        assert!(deleted.contains(&synced.id) && deleted.contains(&folder.id));
    }

    #[test]
    fn reopening_adds_missing_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE tags (id TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '')",
            )
            .unwrap();
        }
        let db = Database::open(&path).unwrap();
        let mut tag = Tag {
            title: "draft".into(),
            ..Default::default()
        };
        db.save(&mut tag).unwrap();
        assert_eq!(db.load::<Tag>(&tag.id).unwrap().unwrap().title, "draft");
    }
}
