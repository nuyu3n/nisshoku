//! Synchronization engine, compatible with Joplin sync targets (version 3).
//!
//! A sync target is a directory-like store ([`RemoteStorage`]) laid out as:
//!
//! ```text
//! info.json          sync target version and encryption settings
//! <id>.md            one file per item, see nisshoku_core::raw
//! .resource/<id>     attachment data
//! locks/             one file per client currently syncing
//! temp/
//! ```
//!
//! A sync run has three steps, each of which can be interrupted and resumed:
//!
//! 1. Upload local items changed since their last sync, resolving conflicts
//!    with remote changes.
//! 2. Delete remote items that were deleted locally.
//! 3. Apply remote changes ("delta") to the local database.

pub mod drivers;
pub mod e2ee;
mod error;
pub mod locks;
pub mod storage;
pub mod sync_info;
mod synchronizer;

pub use error::{Result, SyncError};
pub use storage::{DeltaChange, DeltaPage, Precondition, RemoteStat, RemoteStorage};
pub use synchronizer::{ProgressHook, SharedDb, SyncOptions, SyncReport, SyncStep, Synchronizer};
