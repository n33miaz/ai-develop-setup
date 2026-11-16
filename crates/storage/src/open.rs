use crate::{StorageConfig, StorageError, migrate};
use rusqlite::{Connection, OpenFlags};
use std::{path::Path, time::Duration};

pub const APPLICATION_ID: i64 = 0x41445331;
pub const SCHEMA_VERSION: i64 = 1;

/// Owns the connection; raw SQL remains private to storage.
pub struct AuditStore {
    pub(crate) connection: Connection,
}

impl std::fmt::Debug for AuditStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuditStore { .. }")
    }
}

fn integer(c: &Connection, name: &str) -> Result<i64, StorageError> {
    Ok(c.pragma_query_value(None, name, |r| r.get(0))?)
}

fn preflight(c: &Connection, config: &StorageConfig) -> Result<(), StorageError> {
    let id = integer(c, "application_id")?;
    let version = integer(c, "user_version")?;
    if id != APPLICATION_ID && id != 0 {
        return Err(StorageError::ForeignApplication);
    }
    if version > SCHEMA_VERSION {
        return Err(StorageError::SchemaTooNew);
    }
    if version < 0
        || (id == 0 && version != 0)
        || (id == APPLICATION_ID && version != SCHEMA_VERSION)
    {
        return Err(StorageError::UnsupportedSchema);
    }
    let count: i64 = c.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
    if id == 0 && count != 0 {
        return Err(StorageError::ForeignApplication);
    }
    if id == APPLICATION_ID {
        let schema: String = c.query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name='meta'",
            [],
            |r| r.get(0),
        )?;
        if schema
            != "CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL) STRICT, WITHOUT ROWID"
        {
            return Err(StorageError::UnsupportedSchema);
        }
    }
    if config.integrity_check_on_open {
        let result: String = c.pragma_query_value(None, "integrity_check", |r| r.get(0))?;
        if result != "ok" {
            return Err(StorageError::Corrupt);
        }
    }
    Ok(())
}

impl AuditStore {
    /// Existing files are checked read-only before any persistent pragma is set.
    pub fn open(path: &Path, config: &StorageConfig) -> Result<Self, StorageError> {
        match std::fs::metadata(path) {
            Ok(meta) => {
                if !meta.is_file() || meta.len() < 100 {
                    return Err(StorageError::Corrupt);
                }
                let read = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                read.busy_timeout(Duration::from_millis(config.busy_timeout_ms.into()))?;
                preflight(&read, config)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(StorageError::Io),
        }
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        connection.busy_timeout(Duration::from_millis(config.busy_timeout_ms.into()))?;
        preflight(&connection, config)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        for (name, value) in [
            ("synchronous", 2),
            ("foreign_keys", 1),
            ("trusted_schema", 0),
            ("cell_size_check", 1),
        ] {
            connection.pragma_update(None, name, value)?;
        }
        migrate::migrate(&mut connection)?;
        let store = Self { connection };
        store.verify_pragmas(config)?;
        Ok(store)
    }

    pub fn verify_pragmas(&self, config: &StorageConfig) -> Result<(), StorageError> {
        let mode: String = self
            .connection
            .pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        if mode != "wal" {
            return Err(StorageError::PragmaMismatch);
        }
        for (name, expected) in [
            ("synchronous", 2),
            ("foreign_keys", 1),
            ("trusted_schema", 0),
            ("cell_size_check", 1),
            ("busy_timeout", i64::from(config.busy_timeout_ms)),
            ("application_id", APPLICATION_ID),
            ("user_version", SCHEMA_VERSION),
        ] {
            if integer(&self.connection, name)? != expected {
                return Err(StorageError::PragmaMismatch);
            }
        }
        Ok(())
    }
}
