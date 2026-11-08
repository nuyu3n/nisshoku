#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("could not parse item: {0}")]
    Parse(String),
    #[error("invalid item: {0}")]
    InvalidItem(String),
}

pub type Result<T> = std::result::Result<T, Error>;
