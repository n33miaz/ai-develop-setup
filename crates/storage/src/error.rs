use std::fmt;

/// Categories only: never retain database diagnostics or caller data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    Configuration,
    Io,
    Busy,
    Corrupt,
    ForeignApplication,
    SchemaTooNew,
    UnsupportedSchema,
    PragmaMismatch,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "storage: {self:?}")
    }
}

impl std::error::Error for StorageError {}

impl From<rusqlite::Error> for StorageError {
    fn from(value: rusqlite::Error) -> Self {
        use rusqlite::{Error, ErrorCode};
        match value {
            Error::SqliteFailure(e, _) => match e.code {
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => Self::Busy,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => Self::Corrupt,
                _ => Self::Io,
            },
            _ => Self::Corrupt,
        }
    }
}
