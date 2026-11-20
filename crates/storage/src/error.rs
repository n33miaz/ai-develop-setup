use std::fmt;

/// Categories only: never retain database diagnostics or caller data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    Configuration,
    Io,
    Busy,
    Constraint,
    ReadOnly,
    Permission,
    CantOpen,
    Full,
    Corrupt,
    ForeignApplication,
    SchemaTooNew,
    UnsupportedSchema,
    /// Written by a pre-release format no build reads any more; move the file aside.
    RetiredSchema,
    /// A version 3 log returned to a previously used key; migration would break verification.
    LegacyKeyReuse,
    PragmaMismatch,
    /// The product connection refused a statement outside its allow-list.
    Denied,
    /// The event's `seq` is not the next one of its stream.
    Conflict,
    /// A key with this id was registered with different key material.
    WrongKey,
    KeyUnavailable,
    /// Ciphertext, nonce or an authenticated clear column was altered.
    Tampered,
    TooLarge,
    InvalidEvent,
    Cipher,
    /// First row whose chain link does not match; carries only its position.
    ChainBroken {
        position: i64,
    },
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
            // A denied function is reported as a generic error with this message.
            Error::SqliteFailure(e, Some(message))
                if e.code == ErrorCode::Unknown && message.starts_with("not authorized") =>
            {
                Self::Denied
            }
            Error::SqlInputError { error, msg, .. }
                if error.code == ErrorCode::Unknown && msg.starts_with("not authorized") =>
            {
                Self::Denied
            }
            Error::SqliteFailure(e, _) => match e.code {
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => Self::Busy,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => Self::Corrupt,
                ErrorCode::ConstraintViolation => Self::Constraint,
                ErrorCode::AuthorizationForStatementDenied => Self::Denied,
                ErrorCode::ReadOnly => Self::ReadOnly,
                ErrorCode::PermissionDenied => Self::Permission,
                ErrorCode::CannotOpen => Self::CantOpen,
                ErrorCode::DiskFull => Self::Full,
                _ => Self::Io,
            },
            Error::InvalidPath(_) | Error::NulError(_) => Self::CantOpen,
            Error::QueryReturnedNoRows
            | Error::InvalidColumnType(..)
            | Error::IntegralValueOutOfRange(..) => Self::UnsupportedSchema,
            _ => Self::Io,
        }
    }
}

impl From<crate::cipher::CipherError> for StorageError {
    fn from(value: crate::cipher::CipherError) -> Self {
        use crate::cipher::CipherError;
        match value {
            CipherError::KeyUnavailable | CipherError::InvalidKeyId => Self::KeyUnavailable,
            CipherError::Authentication => Self::Tampered,
            CipherError::Random | CipherError::Encryption => Self::Cipher,
        }
    }
}

impl From<std::io::Error> for StorageError {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::PermissionDenied => Self::Permission,
            std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput => Self::CantOpen,
            std::io::ErrorKind::StorageFull => Self::Full,
            _ => Self::Io,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sqlite_error_categories_are_distinct() {
        for (code, expected) in [
            (19, StorageError::Constraint),
            (8, StorageError::ReadOnly),
            (3, StorageError::Permission),
            (14, StorageError::CantOpen),
            (13, StorageError::Full),
            (5, StorageError::Busy),
            (11, StorageError::Corrupt),
            (23, StorageError::Denied),
        ] {
            assert_eq!(
                StorageError::from(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(code),
                    None
                )),
                expected
            );
        }
        for (kind, expected) in [
            (
                std::io::ErrorKind::PermissionDenied,
                StorageError::Permission,
            ),
            (std::io::ErrorKind::NotFound, StorageError::CantOpen),
            (std::io::ErrorKind::InvalidInput, StorageError::CantOpen),
            (std::io::ErrorKind::StorageFull, StorageError::Full),
        ] {
            assert_eq!(StorageError::from(std::io::Error::from(kind)), expected);
        }
        assert_eq!(
            StorageError::from(rusqlite::Error::QueryReturnedNoRows),
            StorageError::UnsupportedSchema
        );
        let denied = |message: &str| {
            StorageError::from(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(message.to_owned()),
            ))
        };
        assert_eq!(
            denied("not authorized to use function: hex"),
            StorageError::Denied
        );
        assert_eq!(denied("no such table: x"), StorageError::Io);
    }
}
