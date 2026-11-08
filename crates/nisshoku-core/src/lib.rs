//! Core data model for Nisshoku.
//!
//! The item model and on-disk serialization format are compatible with the
//! sync format used by Joplin (sync target version 3), so that a sync target
//! can be shared with Joplin clients. This is an independent implementation
//! written from the publicly documented format, not a port of Joplin's code.

pub mod db;
pub mod dmp;
pub mod error;
pub mod ids;
pub mod item_type;
pub mod models;
pub mod raw;
pub mod revisions;
pub mod time;

pub use db::{Database, DeletedItem};
pub use error::{Error, Result};
pub use item_type::ItemType;
pub use models::{AnyItem, Folder, Model, Note, NoteTag, Resource, Revision, Tag};
pub use raw::RawItem;
