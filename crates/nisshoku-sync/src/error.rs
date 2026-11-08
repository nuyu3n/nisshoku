#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Core(#[from] nisshoku_core::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("remote storage error: {0}")]
    Remote(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("sync target is locked by another client: {0}")]
    Locked(String),
    #[error("incompatible sync target: {0}")]
    IncompatibleTarget(String),
    #[error("the sync target is encrypted: the master password is required")]
    EncryptionPasswordRequired,
    #[error("the master password is wrong")]
    WrongEncryptionPassword,
    /// A problem with a single item. Sync continues with the other items.
    #[error("{0}")]
    Item(String),
    #[error("sync cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, SyncError>;
