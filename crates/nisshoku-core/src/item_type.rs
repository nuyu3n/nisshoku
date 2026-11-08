use serde::{Deserialize, Serialize};

/// Numeric item types, as written in the `type_` property of serialized items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum ItemType {
    Note = 1,
    Folder = 2,
    Setting = 3,
    Resource = 4,
    Tag = 5,
    NoteTag = 6,
    Search = 7,
    Alarm = 8,
    MasterKey = 9,
    ItemChange = 10,
    NoteResource = 11,
    ResourceLocalState = 12,
    Revision = 13,
    Migration = 14,
    SmartFilter = 15,
    Command = 16,
}

impl ItemType {
    /// Item types this implementation stores and syncs.
    pub const SYNCED: [ItemType; 6] = [
        ItemType::Folder,
        ItemType::Tag,
        ItemType::Resource,
        ItemType::Note,
        ItemType::NoteTag,
        ItemType::Revision,
    ];

    pub fn from_i64(v: i64) -> Option<Self> {
        use ItemType::*;
        Some(match v {
            1 => Note,
            2 => Folder,
            3 => Setting,
            4 => Resource,
            5 => Tag,
            6 => NoteTag,
            7 => Search,
            8 => Alarm,
            9 => MasterKey,
            10 => ItemChange,
            11 => NoteResource,
            12 => ResourceLocalState,
            13 => Revision,
            14 => Migration,
            15 => SmartFilter,
            16 => Command,
            _ => return None,
        })
    }

    pub fn as_i64(self) -> i64 {
        self as i64
    }
}
