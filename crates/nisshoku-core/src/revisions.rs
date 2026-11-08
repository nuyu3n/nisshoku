//! Note history, compatible with Joplin's revisions.
//!
//! A revision stores what changed since the previous revision of the same
//! note: diffs of the title and body (see [`crate::dmp`]) and an object diff of
//! the other properties, `{"new": {changed properties}, "deleted": [names]}`.
//! The first revision of a note diffs from an empty note, so a version is
//! rebuilt by applying every revision from the oldest one.
//!
//! When revisions are made:
//! - [`Database::collect_revisions`]: notes changed since their last revision,
//!   if that revision is older than [`MIN_INTERVAL_MS`], get a new one with
//!   their current state.
//! - [`Database::save_note`]: before saving an edit, a note whose last
//!   revision is older than [`OLD_NOTE_INTERVAL_MS`] (or that has none) gets
//!   one with its state before the edit, so the last stable version survives
//!   a burst of edits.
//!
//! Revisions outlive their note (a deleted note can be restored) until they
//! expire, see [`Database::delete_old_revisions`].

use serde_json::{Map, Value};

use crate::db::Database;
use crate::dmp;
use crate::error::{Error, Result};
use crate::item_type::ItemType;
use crate::models::{FieldKind, Model, Note, Revision};
use crate::raw::{self, RawItem};
use crate::time::now_ms;

pub const MIN_INTERVAL_MS: i64 = 10 * 60 * 1000;
pub const OLD_NOTE_INTERVAL_MS: i64 = 7 * 24 * 3600 * 1000;
pub const DEFAULT_TTL_MS: i64 = 90 * 24 * 3600 * 1000;

/// Properties not recorded in metadata diffs.
const NOT_METADATA: [&str; 9] = [
    "title",
    "body",
    "created_time",
    "updated_time",
    "user_created_time",
    "user_updated_time",
    "encryption_cipher_text",
    "encryption_applied",
    "is_conflict",
];

/// A note as rebuilt from revisions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteState {
    pub title: String,
    pub body: String,
    pub metadata: Map<String, Value>,
}

impl NoteState {
    pub fn of(note: &Note) -> Self {
        let mut metadata = Map::new();
        for (key, value) in note.to_raw().props {
            if NOT_METADATA.contains(&key.as_str()) {
                continue;
            }
            let kind = Note::FIELDS
                .iter()
                .find(|(n, _)| *n == key)
                .map(|(_, k)| *k);
            let value = raw::unescape_prop(&value);
            let json = match (key.as_str(), kind) {
                // As Joplin writes them.
                ("latitude" | "longitude", _) => {
                    Value::String(format!("{:.8}", raw::parse_float(&value)))
                }
                ("altitude", _) => Value::String(format!("{:.4}", raw::parse_float(&value))),
                (_, Some(FieldKind::Int)) => Value::from(raw::parse_int(&value)),
                (_, Some(FieldKind::Float)) => {
                    serde_json::Number::from_f64(raw::parse_float(&value))
                        .map(|n| {
                            // Whole numbers as integers, like JavaScript.
                            n.as_f64()
                                .filter(|f| f.fract() == 0.0 && f.abs() < 1e15)
                                .map_or(Value::Number(n.clone()), |f| Value::from(f as i64))
                        })
                        .unwrap_or(Value::from(0))
                }
                _ => Value::String(value),
            };
            metadata.insert(key, json);
        }
        NoteState {
            title: note.title.clone(),
            body: note.body.clone(),
            metadata,
        }
    }

    fn apply(&mut self, r: &Revision) -> Result<()> {
        self.title = dmp::apply_json(&r.title_diff, &self.title)?;
        self.body = dmp::apply_json(&r.body_diff, &self.body)?;
        if !r.metadata_diff.trim().is_empty() {
            let diff: Value = serde_json::from_str(&r.metadata_diff)
                .map_err(|e| Error::Parse(format!("invalid metadata diff: {e}")))?;
            if let Some(new) = diff.get("new").and_then(Value::as_object) {
                for (k, v) in new {
                    self.metadata.insert(k.clone(), v.clone());
                }
            }
            if let Some(deleted) = diff.get("deleted").and_then(Value::as_array) {
                for k in deleted.iter().filter_map(Value::as_str) {
                    self.metadata.remove(k);
                }
            }
        }
        Ok(())
    }

    /// The diffs from `previous` to `self`, or `None` if nothing changed.
    fn diff_from(&self, previous: &NoteState) -> Option<(String, String, String)> {
        let title = dmp::diff_json(&previous.title, &self.title);
        let body = dmp::diff_json(&previous.body, &self.body);
        let new: Map<String, Value> = self
            .metadata
            .iter()
            .filter(|(k, v)| previous.metadata.get(*k) != Some(*v))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let deleted: Vec<Value> = previous
            .metadata
            .keys()
            .filter(|k| !self.metadata.contains_key(*k))
            .map(|k| Value::String(k.clone()))
            .collect();
        if title == "[]" && body == "[]" && new.is_empty() && deleted.is_empty() {
            return None;
        }
        let metadata = serde_json::json!({ "new": new, "deleted": deleted }).to_string();
        Some((title, body, metadata))
    }

    /// The note this state describes. `base` supplies what revisions do not
    /// record (creation times); `updated_time` is the version's time.
    pub fn to_note(&self, id: &str, base: Option<&Note>, updated_time: i64) -> Result<Note> {
        let mut raw = RawItem::new(ItemType::Note);
        raw.title = Some(raw::escape_prop(&self.title));
        raw.body = Some(self.body.clone());
        for (k, v) in &self.metadata {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Bool(b) => (*b as i64).to_string(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            raw.props.push((k.clone(), raw::escape_prop(&s)));
        }
        raw.props.retain(|(k, _)| k != "id");
        raw.props.push(("id".into(), id.into()));
        let mut note = Note::from_raw(&raw)?;
        // Set directly: the property escaping cannot represent a literal "\n".
        note.title = self.title.clone();
        note.body = self.body.clone();
        if let Some(base) = base {
            note.created_time = base.created_time;
            note.user_created_time = base.user_created_time;
        }
        note.updated_time = updated_time;
        note.user_updated_time = updated_time;
        Ok(note)
    }
}

impl Database {
    /// Revisions of a note, oldest first.
    pub fn revisions(&self, note_id: &str) -> Result<Vec<Revision>> {
        let mut revisions = self.query::<Revision, _>("item_id = ?1", [note_id])?;
        revisions.sort_by_key(|r| (r.item_updated_time, r.created_time));
        Ok(revisions)
    }

    /// The state recorded by `revision_id` (the latest one if `None`).
    pub fn note_state_at(&self, note_id: &str, revision_id: Option<&str>) -> Result<NoteState> {
        let mut state = NoteState::default();
        for r in self.revisions(note_id)? {
            // A corrupt revision is skipped rather than losing the history.
            let mut next = state.clone();
            if next.apply(&r).is_ok() {
                state = next;
            }
            if Some(r.id.as_str()) == revision_id {
                return Ok(state);
            }
        }
        match revision_id {
            Some(id) => Err(Error::InvalidItem(format!(
                "no revision {id} for note {note_id}"
            ))),
            None => Ok(state),
        }
    }

    /// The note as it was at a revision.
    pub fn note_at_revision(&self, note_id: &str, revision_id: &str) -> Result<Note> {
        let revision = self
            .load::<Revision>(revision_id)?
            .ok_or_else(|| Error::InvalidItem(format!("no revision {revision_id}")))?;
        let state = self.note_state_at(note_id, Some(revision_id))?;
        let current = self.load::<Note>(note_id)?;
        state.to_note(note_id, current.as_ref(), revision.item_updated_time)
    }

    /// Makes a revision's version the current one (re-creating the note if it
    /// was deleted). The current version is recorded first.
    pub fn restore_revision(&self, note_id: &str, revision_id: &str) -> Result<Note> {
        let old = self.note_at_revision(note_id, revision_id)?;
        let mut restored = match self.load::<Note>(note_id)? {
            // Only the text goes back: where the note is, its tags, comments,
            // pin and other app data stay as they are now.
            Some(current) => {
                self.create_revision(&current)?;
                Note {
                    title: old.title,
                    body: old.body,
                    ..current
                }
            }
            None => {
                // Re-created: in its notebook if that still exists.
                let mut n = old;
                if self.load::<crate::models::Folder>(&n.parent_id)?.is_none_or(|f| f.deleted_time != 0) {
                    n.parent_id = String::new();
                }
                n.deleted_time = 0;
                n.is_conflict = 0;
                n
            }
        };
        self.save(&mut restored)?;
        Ok(restored)
    }

    /// Records `note`'s state as a new revision, unless it equals the latest.
    pub fn create_revision(&self, note: &Note) -> Result<Option<Revision>> {
        let previous = self.note_state_at(&note.id, None)?;
        let Some((title_diff, body_diff, metadata_diff)) = NoteState::of(note).diff_from(&previous)
        else {
            return Ok(None);
        };
        let mut revision = Revision {
            item_type: ItemType::Note.as_i64(),
            item_id: note.id.clone(),
            item_updated_time: note.updated_time,
            title_diff,
            body_diff,
            metadata_diff,
            ..Default::default()
        };
        self.save(&mut revision)?;
        Ok(Some(revision))
    }

    fn latest_revision(&self, note_id: &str) -> Result<Option<Revision>> {
        Ok(self.revisions(note_id)?.pop())
    }

    /// Saves a note edit, first recording the previous version if the note
    /// had no recent revision.
    pub fn save_note(&self, note: &mut Note) -> Result<()> {
        if !note.id.is_empty()
            && note.is_conflict == 0
            && let Some(before) = self.load::<Note>(&note.id)?
        {
            let latest = self.latest_revision(&note.id)?;
            let stale = latest
                .as_ref()
                .is_none_or(|r| now_ms() - r.created_time > OLD_NOTE_INTERVAL_MS);
            let unrecorded = latest
                .as_ref()
                .is_none_or(|r| before.updated_time > r.item_updated_time);
            if stale && unrecorded {
                self.create_revision(&before)?;
            }
        }
        self.save(note)
    }

    /// Notes changed since their latest revision, when that revision is old
    /// enough to take another ([`MIN_INTERVAL_MS`]). One pass over the
    /// tables, so that it can run often on large libraries.
    pub fn notes_needing_revision(&self) -> Result<Vec<String>> {
        let now = now_ms();
        let mut stmt = self.conn().prepare(
            "SELECT n.id FROM notes n
             LEFT JOIN (SELECT item_id, MAX(item_updated_time) AS t, MAX(created_time) AS c
                        FROM revisions GROUP BY item_id) r ON r.item_id = n.id
             WHERE n.is_conflict = 0
               AND (r.item_id IS NULL OR (n.updated_time > r.t AND ?1 - r.c >= ?2))",
        )?;
        let ids = stmt.query_map(rusqlite::params![now, MIN_INTERVAL_MS], |row| row.get(0))?;
        Ok(ids.collect::<rusqlite::Result<_>>()?)
    }

    /// Records the current version of notes changed since their latest
    /// revision, when that revision is old enough. Returns how many.
    pub fn collect_revisions(&self) -> Result<usize> {
        let mut created = 0;
        for id in self.notes_needing_revision()? {
            if let Some(note) = self.load::<Note>(&id)?
                && self.create_revision(&note)?.is_some()
            {
                created += 1;
            }
        }
        Ok(created)
    }

    /// Deletes every revision of a note (for example once it is locked, so
    /// that its earlier text does not stay readable in the history).
    pub fn delete_revisions(&self, note_id: &str) -> Result<usize> {
        let revisions = self.revisions(note_id)?;
        for r in &revisions {
            self.delete(ItemType::Revision, &r.id)?;
        }
        Ok(revisions.len())
    }

    /// Deletes revisions older than `ttl_ms`. When a note keeps newer
    /// revisions, the oldest kept one is rewritten to hold the full version,
    /// so that the chain still starts from an empty note.
    pub fn delete_old_revisions(&self, ttl_ms: i64) -> Result<usize> {
        let cutoff = now_ms() - ttl_ms;
        let note_ids: Vec<String> = self
            .query::<Revision, _>("created_time < ?1", [cutoff])?
            .into_iter()
            .map(|r| r.item_id)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut deleted = 0;
        for note_id in note_ids {
            let revisions = self.revisions(&note_id)?;
            let split = revisions.iter().position(|r| r.created_time >= cutoff);
            if let Some(i) = split.filter(|&i| i > 0) {
                let mut first_kept = revisions[i].clone();
                let state = self.note_state_at(&note_id, Some(&first_kept.id))?;
                let (title, body, metadata) =
                    state.diff_from(&NoteState::default()).unwrap_or_else(|| {
                        (
                            "[]".into(),
                            "[]".into(),
                            "{\"new\":{},\"deleted\":[]}".into(),
                        )
                    });
                first_kept.title_diff = title;
                first_kept.body_diff = body;
                first_kept.metadata_diff = metadata;
                self.save(&mut first_kept)?;
            }
            let end = split.unwrap_or(revisions.len());
            for r in &revisions[..end] {
                self.delete(ItemType::Revision, &r.id)?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Folder;

    fn note(db: &Database, title: &str, body: &str) -> Note {
        let mut f = Folder::new("", "f");
        db.save(&mut f).unwrap();
        let mut n = Note::new(&f.id, title, body);
        db.save(&mut n).unwrap();
        n
    }

    #[test]
    fn revisions_rebuild_every_version() {
        let db = Database::open_in_memory().unwrap();
        let mut n = note(&db, "v1", "body 1 🌒");
        let r1 = db.create_revision(&n).unwrap().unwrap();
        n.title = "v2".into();
        n.body = "body 2 🌒 changed\nsecond line".into();
        n.is_todo = 1;
        n.author = "作者".into();
        db.save(&mut n).unwrap();
        let r2 = db.create_revision(&n).unwrap().unwrap();
        assert!(
            db.create_revision(&n).unwrap().is_none(),
            "no change, no revision"
        );
        n.body.clear();
        n.author.clear();
        db.save(&mut n).unwrap();
        let r3 = db.create_revision(&n).unwrap().unwrap();

        let v1 = db.note_at_revision(&n.id, &r1.id).unwrap();
        assert_eq!(
            (v1.title.as_str(), v1.body.as_str(), v1.is_todo),
            ("v1", "body 1 🌒", 0)
        );
        let v2 = db.note_at_revision(&n.id, &r2.id).unwrap();
        assert_eq!(
            (v2.title.as_str(), v2.is_todo, v2.author.as_str()),
            ("v2", 1, "作者")
        );
        assert_eq!(v2.body, "body 2 🌒 changed\nsecond line");
        let v3 = db.note_at_revision(&n.id, &r3.id).unwrap();
        assert_eq!((v3.body.as_str(), v3.author.as_str()), ("", ""));
        assert_eq!(v3.parent_id, n.parent_id);

        // Restoring makes the old version current and keeps history.
        let restored = db.restore_revision(&n.id, &r1.id).unwrap();
        assert_eq!(restored.body, "body 1 🌒");
        assert_eq!(db.load::<Note>(&n.id).unwrap().unwrap().body, "body 1 🌒");
    }

    #[test]
    fn deleted_notes_can_be_restored() {
        let db = Database::open_in_memory().unwrap();
        let n = note(&db, "lost", "precious");
        let r = db.create_revision(&n).unwrap().unwrap();
        db.delete(ItemType::Note, &n.id).unwrap();
        let restored = db.restore_revision(&n.id, &r.id).unwrap();
        assert_eq!(restored.body, "precious");
        assert!(db.load::<Note>(&n.id).unwrap().is_some());
    }

    #[test]
    fn first_edit_of_an_old_note_records_the_previous_version() {
        let db = Database::open_in_memory().unwrap();
        let mut n = note(&db, "t", "original");
        n.body = "edited".into();
        db.save_note(&mut n).unwrap();
        let revisions = db.revisions(&n.id).unwrap();
        assert_eq!(revisions.len(), 1);
        assert_eq!(
            db.note_at_revision(&n.id, &revisions[0].id).unwrap().body,
            "original"
        );
        // Not again for the next edit: the last revision is recent.
        n.body = "edited twice".into();
        db.save_note(&mut n).unwrap();
        assert_eq!(db.revisions(&n.id).unwrap().len(), 1);
        // The collector records the current version once the interval has passed.
        let mut r = revisions[0].clone();
        r.created_time -= MIN_INTERVAL_MS;
        db.write(&r).unwrap();
        assert_eq!(db.collect_revisions().unwrap(), 1);
        let last = db.revisions(&n.id).unwrap().pop().unwrap();
        assert_eq!(
            db.note_at_revision(&n.id, &last.id).unwrap().body,
            "edited twice"
        );
    }

    #[test]
    fn expiry_keeps_the_chain_valid() {
        let db = Database::open_in_memory().unwrap();
        let mut n = note(&db, "t", "a");
        let mut ids = Vec::new();
        for body in ["a", "ab", "abc", "abcd"] {
            n.body = body.into();
            db.save(&mut n).unwrap();
            ids.push(db.create_revision(&n).unwrap().unwrap().id);
        }
        // The first two are old.
        for id in &ids[..2] {
            let mut r = db.load::<Revision>(id).unwrap().unwrap();
            r.created_time -= DEFAULT_TTL_MS + 1000;
            db.write(&r).unwrap();
        }
        assert_eq!(db.delete_old_revisions(DEFAULT_TTL_MS).unwrap(), 2);
        let left = db.revisions(&n.id).unwrap();
        assert_eq!(left.len(), 2);
        assert_eq!(db.note_at_revision(&n.id, &ids[2]).unwrap().body, "abc");
        assert_eq!(db.note_at_revision(&n.id, &ids[3]).unwrap().body, "abcd");
    }

    #[test]
    fn diff_fields_round_trip_through_the_sync_format() {
        let db = Database::open_in_memory().unwrap();
        let mut n = note(&db, "title \\n", "line\nline \\n \\ \"q\" \r\n end");
        n.body.push_str(" 🌒");
        db.save(&mut n).unwrap();
        let r = db.create_revision(&n).unwrap().unwrap();
        let parsed = Revision::from_raw(&RawItem::parse(&r.to_text()).unwrap()).unwrap();
        assert_eq!(parsed, r);
    }
}
