use crate::{StorageConfig, StorageError, migrate};
use rusqlite::{Connection, OpenFlags};
use std::{io::Read, path::Path, time::Duration};

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
    harden(c, config)?;
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
        let mut statement =
            c.prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows
            != vec![(
                "table".into(),
                "meta".into(),
                "meta".into(),
                migrate::META_SQL.into(),
            )]
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

fn harden(c: &Connection, config: &StorageConfig) -> Result<(), StorageError> {
    c.busy_timeout(Duration::from_millis(config.busy_timeout_ms.into()))?;
    c.pragma_update(None, "trusted_schema", 0)?;
    c.pragma_update(None, "cell_size_check", 1)?;
    c.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    Ok(())
}

fn header(path: &Path) -> Result<bool, StorageError> {
    let mut file = std::fs::File::open(path).map_err(StorageError::from)?;
    let meta = file.metadata().map_err(StorageError::from)?;
    if !meta.is_file() {
        return Err(StorageError::CantOpen);
    }
    if meta.len() == 0 {
        return Ok(false);
    }
    let mut bytes = [0; 100];
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            StorageError::Corrupt
        } else {
            error.into()
        }
    })?;
    let page = u16::from_be_bytes([bytes[16], bytes[17]]);
    let page = if page == 1 { 65536 } else { u64::from(page) };
    if &bytes[..16] != b"SQLite format 3\0"
        || !(512..=65536).contains(&page)
        || !page.is_power_of_two()
        || meta.len() % page != 0
        || ![1, 2].contains(&bytes[18])
        || ![1, 2].contains(&bytes[19])
    {
        return Err(StorageError::Corrupt);
    }
    let version = i32::from_be_bytes(bytes[60..64].try_into().unwrap()) as i64;
    let id = i32::from_be_bytes(bytes[68..72].try_into().unwrap()) as i64;
    if id != 0 && id != APPLICATION_ID {
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
    Ok(true)
}

fn existing_flags() -> OpenFlags {
    OpenFlags::SQLITE_OPEN_READ_WRITE
}

impl AuditStore {
    /// Validates the header before SQLite can access the file.
    pub fn open(path: &Path, config: &StorageConfig) -> Result<Self, StorageError> {
        Self::open_after_inspection(path, config, || {})
    }

    fn open_after_inspection(
        path: &Path,
        config: &StorageConfig,
        after: impl FnOnce(),
    ) -> Result<Self, StorageError> {
        let exists = match std::fs::metadata(path) {
            Ok(meta) => {
                if !meta.is_file() {
                    return Err(StorageError::CantOpen);
                }
                if header(path)? {
                    let image = crate::snapshot::image(path, config.inspection_max_bytes)?;
                    let mut read = Connection::open_in_memory()?;
                    harden(&read, config)?;
                    read.deserialize_read_exact(
                        rusqlite::MAIN_DB,
                        image.as_slice(),
                        image.len(),
                        true,
                    )?;
                    preflight(&read, config)?;
                }
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
        };
        after();
        let flags = if exists {
            existing_flags()
        } else {
            existing_flags() | OpenFlags::SQLITE_OPEN_CREATE
        };
        let mut connection = Connection::open_with_flags(path, flags)?;
        let inspection =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)?;
        preflight(&inspection, config)?;
        inspection.commit()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory() -> std::path::PathBuf {
        let executable = std::env::current_exe().unwrap();
        let root = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tmp");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = root.join(format!("storage-unit-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap()
    }

    #[test]
    fn disappeared_file_is_not_recreated() {
        let root = test_directory();
        let path = root.join(format!("disappeared-{}.db", std::process::id()));
        let moved = root.join(format!("disappeared-{}.saved", std::process::id()));
        let config = StorageConfig::defaults();
        drop(AuditStore::open(&path, &config).unwrap());
        let result = AuditStore::open_after_inspection(&path, &config, || {
            std::fs::rename(&path, &moved).unwrap()
        });
        assert_eq!(result.unwrap_err(), StorageError::CantOpen);
        assert!(!path.exists());
        assert!(moved.exists());
    }

    #[test]
    fn memory_journal_reports_pragma_mismatch() {
        let config = StorageConfig::defaults();
        let mut connection = Connection::open_in_memory().unwrap();
        harden(&connection, &config).unwrap();
        connection.pragma_update(None, "synchronous", 2).unwrap();
        connection.pragma_update(None, "foreign_keys", 1).unwrap();
        migrate::migrate(&mut connection).unwrap();
        let store = AuditStore { connection };
        assert_eq!(
            store.verify_pragmas(&StorageConfig::defaults()),
            Err(StorageError::PragmaMismatch)
        );
    }

    #[test]
    fn raw_header_checks_identity_before_sqlite() {
        let root = test_directory();
        let path = root.join(format!("header-{}.db", std::process::id()));
        let mut bytes = [0; 512];
        bytes[..16].copy_from_slice(b"SQLite format 3\0");
        bytes[16..18].copy_from_slice(&512u16.to_be_bytes());
        bytes[18] = 2;
        bytes[19] = 2;
        for (id, version, expected) in [
            (99i32, 0i32, StorageError::ForeignApplication),
            (APPLICATION_ID as i32, 2, StorageError::SchemaTooNew),
            (APPLICATION_ID as i32, -1, StorageError::UnsupportedSchema),
        ] {
            bytes[60..64].copy_from_slice(&version.to_be_bytes());
            bytes[68..72].copy_from_slice(&id.to_be_bytes());
            std::fs::write(&path, bytes).unwrap();
            assert_eq!(header(&path), Err(expected));
        }
    }

    #[test]
    fn inspection_connections_are_hardened() {
        let c = Connection::open_in_memory().unwrap();
        preflight(&c, &StorageConfig::defaults()).unwrap();
        assert_eq!(integer(&c, "trusted_schema").unwrap(), 0);
        assert_eq!(integer(&c, "cell_size_check").unwrap(), 1);
        assert!(
            c.db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE)
                .unwrap()
        );
    }
}
