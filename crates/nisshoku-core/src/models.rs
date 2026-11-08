//! Synced item models.
//!
//! Each model is declared once with its fields in sync-format order. The macro
//! generates the struct, the SQLite mapping and the conversion to and from
//! [`RawItem`]. Properties this version does not know about are kept in
//! `extra` so that items written by newer clients survive a round trip.

use std::collections::BTreeMap;

use rusqlite::types::Value;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::item_type::ItemType;
use crate::raw::{self, RawItem};
use crate::time;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Int,
    Float,
    /// Unix milliseconds, serialized as an ISO 8601 string.
    Time,
    /// A revision diff (JSON), stored on the target as a JSON string.
    DiffJson,
}

impl FieldKind {
    pub fn sql_definition(self) -> &'static str {
        match self {
            FieldKind::Text | FieldKind::DiffJson => "TEXT NOT NULL DEFAULT ''",
            FieldKind::Int | FieldKind::Time => "INTEGER NOT NULL DEFAULT 0",
            FieldKind::Float => "REAL NOT NULL DEFAULT 0",
        }
    }
}

pub trait Model: Sized + Clone + Send + 'static {
    const ITEM_TYPE: ItemType;
    const TABLE: &'static str;
    const FIELDS: &'static [(&'static str, FieldKind)];

    fn id(&self) -> &str;
    fn set_id(&mut self, id: String);
    fn created_time(&self) -> i64;
    fn updated_time(&self) -> i64;
    /// Sets the timestamps for a local modification at time `t`.
    fn touch(&mut self, t: i64, is_new: bool);
    /// Sets only the sync timestamp (`updated_time`): a change of metadata
    /// that the user does not see as an edit of the item.
    fn touch_meta(&mut self, t: i64);

    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self>;
    /// Values for each of [`Self::FIELDS`], followed by the `extra_props` JSON.
    fn sql_values(&self) -> Vec<Value>;
    fn to_raw(&self) -> RawItem;
    fn from_raw(raw: &RawItem) -> Result<Self>;

    fn to_text(&self) -> String {
        self.to_raw().to_text()
    }
}

/// Default values for a model, for when they differ from the type defaults.
pub trait ModelDefaults: Default {
    fn defaults() -> Self {
        Self::default()
    }
}

/// Lets the generated `touch` treat every field uniformly; only integer
/// fields hold times.
trait TimeField {
    fn set(&mut self, _t: i64) {}
    fn set_if_unset(&mut self, _t: i64) {}
}

impl TimeField for i64 {
    fn set(&mut self, t: i64) {
        *self = t;
    }
    fn set_if_unset(&mut self, t: i64) {
        if *self == 0 {
            *self = t;
        }
    }
}

impl TimeField for String {}

impl TimeField for f64 {}

macro_rules! field_ty {
    (Text) => {
        String
    };
    (Int) => {
        i64
    };
    (Float) => {
        f64
    };
    (Time) => {
        i64
    };
    (DiffJson) => {
        String
    };
}

macro_rules! to_prop {
    (Text, $v:expr) => {
        raw::escape_prop($v)
    };
    (Int, $v:expr) => {
        ($v).to_string()
    };
    (Float, $v:expr) => {
        raw::format_float(*$v)
    };
    (Time, $v:expr) => {
        time::ms_to_iso(*$v)
    };
    (DiffJson, $v:expr) => {
        raw::encode_diff_prop($v)
    };
}

/// Like `to_prop!`, but text is written verbatim (used for the note body).
macro_rules! to_prop_verbatim {
    (Text, $v:expr) => {
        ($v).clone()
    };
    ($kind:ident, $v:expr) => {
        to_prop!($kind, $v)
    };
}

macro_rules! from_prop {
    (Text, $s:expr) => {
        raw::unescape_prop($s)
    };
    (Int, $s:expr) => {
        raw::parse_int($s)
    };
    (Float, $s:expr) => {
        raw::parse_float($s)
    };
    (Time, $s:expr) => {
        time::iso_to_ms($s).unwrap_or(0)
    };
    (DiffJson, $s:expr) => {
        raw::decode_diff_prop($s)
    };
}

macro_rules! from_prop_verbatim {
    (Text, $s:expr) => {
        ($s).to_string()
    };
    ($kind:ident, $s:expr) => {
        from_prop!($kind, $s)
    };
}

macro_rules! to_sql {
    (Text, $v:expr) => {
        Value::Text(($v).clone())
    };
    (Int, $v:expr) => {
        Value::Integer(*$v)
    };
    (Float, $v:expr) => {
        Value::Real(*$v)
    };
    (Time, $v:expr) => {
        Value::Integer(*$v)
    };
    (DiffJson, $v:expr) => {
        Value::Text(($v).clone())
    };
}

macro_rules! from_sql {
    (Text, $row:expr, $name:expr) => {
        $row.get::<_, Option<String>>($name)?.unwrap_or_default()
    };
    (Int, $row:expr, $name:expr) => {
        $row.get::<_, Option<i64>>($name)?.unwrap_or_default()
    };
    (Float, $row:expr, $name:expr) => {
        $row.get::<_, Option<f64>>($name)?.unwrap_or_default()
    };
    (Time, $row:expr, $name:expr) => {
        $row.get::<_, Option<i64>>($name)?.unwrap_or_default()
    };
    (DiffJson, $row:expr, $name:expr) => {
        $row.get::<_, Option<String>>($name)?.unwrap_or_default()
    };
}

macro_rules! define_model {
    (
        $(#[$meta:meta])*
        $name:ident, $item_type:expr, $table:literal, {
            $( $field:ident : $kind:ident ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
        pub struct $name {
            $( pub $field: field_ty!($kind), )*
            /// Unknown properties, preserved for round-tripping.
            #[serde(default)]
            pub extra: BTreeMap<String, String>,
        }

        impl Model for $name {
            const ITEM_TYPE: ItemType = $item_type;
            const TABLE: &'static str = $table;
            const FIELDS: &'static [(&'static str, FieldKind)] =
                &[$( (stringify!($field), FieldKind::$kind) ),*];

            fn id(&self) -> &str { &self.id }
            fn set_id(&mut self, id: String) { self.id = id; }
            fn created_time(&self) -> i64 { self.created_time }
            fn updated_time(&self) -> i64 { self.updated_time }

            fn touch(&mut self, t: i64, is_new: bool) {
                $(
                    match stringify!($field) {
                        "updated_time" | "user_updated_time" => TimeField::set(&mut self.$field, t),
                        "created_time" | "user_created_time" if is_new => TimeField::set_if_unset(&mut self.$field, t),
                        _ => {}
                    }
                )*
            }

            fn touch_meta(&mut self, t: i64) {
                $(
                    if stringify!($field) == "updated_time" {
                        TimeField::set(&mut self.$field, t);
                    }
                )*
            }

            fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
                let extra: Option<String> = row.get("extra_props")?;
                Ok(Self {
                    $( $field: from_sql!($kind, row, stringify!($field)), )*
                    extra: extra
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default(),
                })
            }

            fn sql_values(&self) -> Vec<Value> {
                vec![
                    $( to_sql!($kind, &self.$field), )*
                    Value::Text(serde_json::to_string(&self.extra).unwrap_or_else(|_| "{}".into())),
                ]
            }

            fn to_raw(&self) -> RawItem {
                let mut raw = RawItem::new(Self::ITEM_TYPE);
                $(
                    match stringify!($field) {
                        "title" => raw.title = Some(to_prop!($kind, &self.$field)),
                        "body" => raw.body = Some(to_prop_verbatim!($kind, &self.$field)),
                        key => raw.props.push((key.to_string(), to_prop!($kind, &self.$field))),
                    }
                )*
                for (k, v) in &self.extra {
                    raw.props.push((k.clone(), raw::escape_prop(v)));
                }
                raw
            }

            fn from_raw(raw: &RawItem) -> Result<Self> {
                if raw.item_type != Self::ITEM_TYPE.as_i64() {
                    return Err(Error::InvalidItem(format!(
                        "expected type {}, got {}", Self::ITEM_TYPE.as_i64(), raw.item_type
                    )));
                }
                let mut props: BTreeMap<&str, &str> =
                    raw.props.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                let mut out = <Self as ModelDefaults>::defaults();
                $(
                    match stringify!($field) {
                        "title" => if let Some(v) = &raw.title { out.$field = from_prop!($kind, v); },
                        "body" => if let Some(v) = &raw.body { out.$field = from_prop_verbatim!($kind, v); },
                        key => if let Some(v) = props.remove(key) { out.$field = from_prop!($kind, v); },
                    }
                )*
                out.extra = props
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), raw::unescape_prop(v)))
                    .collect();
                if !crate::ids::is_valid_id(&out.id) {
                    return Err(Error::InvalidItem(format!("invalid id: {:?}", out.id)));
                }
                Ok(out)
            }
        }
    };
}

define_model! {
    Folder, ItemType::Folder, "folders", {
        id: Text,
        title: Text,
        created_time: Time,
        updated_time: Time,
        user_created_time: Time,
        user_updated_time: Time,
        encryption_cipher_text: Text,
        encryption_applied: Int,
        parent_id: Text,
        is_shared: Int,
        share_id: Text,
        master_key_id: Text,
        icon: Text,
        user_data: Text,
        deleted_time: Int,
    }
}
impl ModelDefaults for Folder {}

define_model! {
    Note, ItemType::Note, "notes", {
        id: Text,
        parent_id: Text,
        title: Text,
        body: Text,
        created_time: Time,
        updated_time: Time,
        is_conflict: Int,
        latitude: Float,
        longitude: Float,
        altitude: Float,
        author: Text,
        source_url: Text,
        is_todo: Int,
        todo_due: Int,
        todo_completed: Int,
        source: Text,
        source_application: Text,
        application_data: Text,
        order: Float,
        user_created_time: Time,
        user_updated_time: Time,
        encryption_cipher_text: Text,
        encryption_applied: Int,
        markup_language: Int,
        is_shared: Int,
        share_id: Text,
        conflict_original_id: Text,
        master_key_id: Text,
        user_data: Text,
        deleted_time: Int,
    }
}

pub const MARKUP_LANGUAGE_MARKDOWN: i64 = 1;
pub const NOTE_SOURCE: &str = "nisshoku";
pub const NOTE_SOURCE_APPLICATION: &str = "app.nisshoku.desktop";

impl ModelDefaults for Note {
    fn defaults() -> Self {
        Self {
            markup_language: MARKUP_LANGUAGE_MARKDOWN,
            ..Default::default()
        }
    }
}

impl Note {
    pub fn new(parent_id: &str, title: &str, body: &str) -> Self {
        Self {
            parent_id: parent_id.into(),
            title: title.into(),
            body: body.into(),
            source: NOTE_SOURCE.into(),
            source_application: NOTE_SOURCE_APPLICATION.into(),
            ..Self::defaults()
        }
    }
}

impl Folder {
    pub fn new(parent_id: &str, title: &str) -> Self {
        Self {
            parent_id: parent_id.into(),
            title: title.into(),
            ..Self::defaults()
        }
    }
}

define_model! {
    /// Metadata for an attachment. The file itself is stored separately (see
    /// [`Resource::file_name`]).
    Resource, ItemType::Resource, "resources", {
        id: Text,
        title: Text,
        mime: Text,
        filename: Text,
        created_time: Time,
        updated_time: Time,
        user_created_time: Time,
        user_updated_time: Time,
        file_extension: Text,
        encryption_cipher_text: Text,
        encryption_applied: Int,
        encryption_blob_encrypted: Int,
        size: Int,
        is_shared: Int,
        share_id: Text,
        master_key_id: Text,
        user_data: Text,
        blob_updated_time: Int,
        ocr_text: Text,
        ocr_details: Text,
        ocr_status: Int,
        ocr_error: Text,
    }
}

impl ModelDefaults for Resource {
    fn defaults() -> Self {
        Self {
            size: -1,
            ..Default::default()
        }
    }
}

impl Resource {
    /// Name of the local file holding this resource's data.
    ///
    /// The extension comes from synced metadata, which anyone who can write
    /// to the sync target controls: only a short alphanumeric one is used,
    /// so that the name can never leave the resource directory.
    pub fn file_name(&self) -> String {
        match self.safe_extension() {
            Some(ext) => format!("{}.{ext}", self.id),
            None => self.id.clone(),
        }
    }

    /// The file extension if it is safe to use in a file name.
    pub fn safe_extension(&self) -> Option<&str> {
        let ext = self.file_extension.as_str();
        (!ext.is_empty() && ext.len() <= 16 && ext.bytes().all(|b| b.is_ascii_alphanumeric()))
            .then_some(ext)
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn file_names_stay_in_the_resource_directory() {
        let mut r = Resource {
            id: "0123456789abcdef0123456789abcdef".into(),
            file_extension: "png".into(),
            ..Default::default()
        };
        assert_eq!(r.file_name(), "0123456789abcdef0123456789abcdef.png");
        for evil in ["../../evil.bat", "x/../../y", "a\\..\\b", "png\0", "a.b", &"x".repeat(17)] {
            r.file_extension = evil.into();
            assert_eq!(r.file_name(), r.id, "{evil:?}");
        }
    }
}

define_model! {
    Tag, ItemType::Tag, "tags", {
        id: Text,
        title: Text,
        created_time: Time,
        updated_time: Time,
        user_created_time: Time,
        user_updated_time: Time,
        encryption_cipher_text: Text,
        encryption_applied: Int,
        is_shared: Int,
        parent_id: Text,
        user_data: Text,
    }
}
impl ModelDefaults for Tag {}

define_model! {
    /// Association between a note and a tag.
    NoteTag, ItemType::NoteTag, "note_tags", {
        id: Text,
        note_id: Text,
        tag_id: Text,
        created_time: Time,
        updated_time: Time,
        user_created_time: Time,
        user_updated_time: Time,
        encryption_cipher_text: Text,
        encryption_applied: Int,
        is_shared: Int,
    }
}
impl ModelDefaults for NoteTag {}

define_model! {
    /// A step in a note's history: diffs from the previous revision of the
    /// same note (the first one diffs from an empty note).
    Revision, ItemType::Revision, "revisions", {
        id: Text,
        parent_id: Text,
        item_type: Int,
        item_id: Text,
        item_updated_time: Int,
        title_diff: DiffJson,
        body_diff: DiffJson,
        metadata_diff: Text,
        encryption_cipher_text: Text,
        encryption_applied: Int,
        updated_time: Time,
        created_time: Time,
    }
}
impl ModelDefaults for Revision {}

/// Any synced item, for code that handles items generically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AnyItem {
    Note(Note),
    Folder(Folder),
    Resource(Resource),
    Tag(Tag),
    NoteTag(NoteTag),
    Revision(Revision),
}

macro_rules! dispatch {
    ($self:expr, $x:ident => $e:expr) => {
        match $self {
            AnyItem::Note($x) => $e,
            AnyItem::Folder($x) => $e,
            AnyItem::Resource($x) => $e,
            AnyItem::Tag($x) => $e,
            AnyItem::NoteTag($x) => $e,
            AnyItem::Revision($x) => $e,
        }
    };
}

impl AnyItem {
    pub fn item_type(&self) -> ItemType {
        match self {
            AnyItem::Note(_) => ItemType::Note,
            AnyItem::Folder(_) => ItemType::Folder,
            AnyItem::Resource(_) => ItemType::Resource,
            AnyItem::Tag(_) => ItemType::Tag,
            AnyItem::NoteTag(_) => ItemType::NoteTag,
            AnyItem::Revision(_) => ItemType::Revision,
        }
    }

    pub fn id(&self) -> &str {
        dispatch!(self, x => x.id())
    }

    pub fn updated_time(&self) -> i64 {
        dispatch!(self, x => x.updated_time())
    }

    pub fn is_encrypted(&self) -> bool {
        dispatch!(self, x => x.encryption_applied != 0)
    }

    pub fn to_raw(&self) -> RawItem {
        dispatch!(self, x => x.to_raw())
    }

    pub fn to_text(&self) -> String {
        self.to_raw().to_text()
    }

    /// Converts a parsed item. Returns `Ok(None)` for item types this version
    /// does not store (for example revisions or master keys).
    pub fn from_raw(raw: &RawItem) -> Result<Option<Self>> {
        Ok(Some(match raw.known_type() {
            Some(ItemType::Note) => AnyItem::Note(Note::from_raw(raw)?),
            Some(ItemType::Folder) => AnyItem::Folder(Folder::from_raw(raw)?),
            Some(ItemType::Resource) => AnyItem::Resource(Resource::from_raw(raw)?),
            Some(ItemType::Tag) => AnyItem::Tag(Tag::from_raw(raw)?),
            Some(ItemType::NoteTag) => AnyItem::NoteTag(NoteTag::from_raw(raw)?),
            Some(ItemType::Revision) => AnyItem::Revision(Revision::from_raw(raw)?),
            _ => return Ok(None),
        }))
    }

    pub fn parse(content: &str) -> Result<Option<Self>> {
        Self::from_raw(&RawItem::parse(content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A note as written by a Joplin desktop client.
    const JOPLIN_NOTE: &str = "\
My note

First paragraph.

- item

id: 0b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b
parent_id: 1c9b8f2b4d5e4f60918a2b3c4d5e6f70
created_time: 2024-03-01T10:00:00.000Z
updated_time: 2024-03-02T11:30:15.250Z
is_conflict: 0
latitude: 35.68950000
longitude: 139.69170000
altitude: 0.0000
author:
source_url:
is_todo: 0
todo_due: 0
todo_completed: 0
source: joplin-desktop
source_application: net.cozic.joplin-desktop
application_data:
order: 0
user_created_time: 2024-03-01T10:00:00.000Z
user_updated_time: 2024-03-02T11:30:15.250Z
encryption_cipher_text:
encryption_applied: 0
markup_language: 1
is_shared: 0
share_id:
conflict_original_id:
master_key_id:
user_data:
deleted_time: 0
some_future_field: hello\\nworld
type_: 1";

    #[test]
    fn parses_joplin_note() {
        let Some(AnyItem::Note(note)) = AnyItem::parse(JOPLIN_NOTE).unwrap() else {
            panic!("expected a note");
        };
        assert_eq!(note.title, "My note");
        assert_eq!(note.body, "First paragraph.\n\n- item");
        assert_eq!(note.id, "0b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b");
        assert_eq!(note.updated_time, 1_709_379_015_250);
        assert_eq!(note.latitude, 35.6895);
        assert_eq!(note.markup_language, 1);
        assert_eq!(
            note.extra.get("some_future_field").map(String::as_str),
            Some("hello\nworld")
        );

        // Re-serializing and parsing again gives the same note, unknown field included.
        let again = Note::from_raw(&RawItem::parse(&note.to_text()).unwrap()).unwrap();
        assert_eq!(again, note);
        assert!(
            note.to_text()
                .contains("some_future_field: hello\\nworld\ntype_: 1")
        );
    }

    #[test]
    fn folder_round_trip() {
        let mut folder = Folder::new("", "Novels");
        folder.id = crate::ids::new_id();
        folder.updated_time = 1_700_000_000_000;
        let text = folder.to_text();
        assert!(text.starts_with("Novels\n\nid: "));
        assert!(text.ends_with("type_: 2"));
        assert_eq!(
            Folder::from_raw(&RawItem::parse(&text).unwrap()).unwrap(),
            folder
        );
    }

    #[test]
    fn unknown_type_is_skipped() {
        let text = "id: 0b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b\ntype_: 9";
        assert!(AnyItem::parse(text).unwrap().is_none());
    }
}
