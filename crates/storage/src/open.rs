use crate::{StorageConfig, StorageError, migrate};
use rusqlite::{
    Connection, OpenFlags,
    config::DbConfig,
    hooks::{AuthAction, AuthContext, Authorization},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const APPLICATION_ID: i64 = 0x41445331;
pub const SCHEMA_VERSION: i64 = 5;

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
    check_identity(id, version)?;
    let count: i64 = c.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
    if id == 0 && count != 0 {
        return Err(StorageError::ForeignApplication);
    }
    if id == APPLICATION_ID && migrate::schema(c)? != migrate::expected_schema(version)? {
        return Err(StorageError::UnsupportedSchema);
    }
    if config.integrity_check_on_open {
        let result: String = c.pragma_query_value(None, "integrity_check", |r| r.get(0))?;
        if result != "ok" {
            return Err(StorageError::Corrupt);
        }
    }
    Ok(())
}

fn check_identity(id: i64, version: i64) -> Result<(), StorageError> {
    if id != APPLICATION_ID && id != 0 {
        return Err(StorageError::ForeignApplication);
    }
    if version > SCHEMA_VERSION {
        return Err(StorageError::SchemaTooNew);
    }
    if version < 0 || (id == 0 && version != 0) || (id == APPLICATION_ID && version < 1) {
        return Err(StorageError::UnsupportedSchema);
    }
    if id == APPLICATION_ID {
        migrate::readable(version)?;
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

#[derive(Debug, PartialEq, Eq)]
enum Header {
    Empty,
    Unmarked,
    Product { page: usize },
}

fn header(path: &Path) -> Result<Header, StorageError> {
    let mut file = std::fs::File::open(path).map_err(StorageError::from)?;
    let meta = file.metadata().map_err(StorageError::from)?;
    if !meta.is_file() {
        return Err(StorageError::CantOpen);
    }
    if meta.len() == 0 {
        return Ok(Header::Empty);
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
    let pages = u32::from_be_bytes(bytes[28..32].try_into().unwrap());
    if pages != 0 && bytes[92..96] == bytes[24..28] && u64::from(pages) * page != meta.len() {
        return Err(StorageError::Corrupt);
    }
    let version = i32::from_be_bytes(bytes[60..64].try_into().unwrap()) as i64;
    let id = i32::from_be_bytes(bytes[68..72].try_into().unwrap()) as i64;
    check_identity(id, version)?;
    Ok(if id == APPLICATION_ID {
        Header::Product {
            page: page as usize,
        }
    } else {
        Header::Unmarked
    })
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

fn present(path: &Path) -> Result<bool, StorageError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Files SQLite would replay or delete on open: any rollback journal, or a
/// WAL beside an empty or missing main file.
fn residue(path: &Path) -> Result<bool, StorageError> {
    let empty = match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.len() == 0,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => return Err(e.into()),
    };
    Ok(present(&sibling(path, "-journal"))? || (empty && present(&sibling(path, "-wal"))?))
}

type Stamp = Option<(u64, Option<std::time::SystemTime>)>;

fn file_stamp(path: &Path) -> Stamp {
    std::fs::metadata(path)
        .ok()
        .map(|meta| (meta.len(), meta.modified().ok()))
}

fn stamp(path: &Path) -> [Stamp; 3] {
    [
        file_stamp(path),
        file_stamp(&sibling(path, "-wal")),
        file_stamp(&sibling(path, "-journal")),
    ]
}

/// Returns whether the main file exists. Only unmarked files get the private
/// in-memory image; product files are inspected by the write connection.
fn inspect(path: &Path, config: &StorageConfig) -> Result<bool, StorageError> {
    match std::fs::metadata(path) {
        Ok(meta) => {
            if !meta.is_file() {
                return Err(StorageError::CantOpen);
            }
            let kind = header(path)?;
            if let Header::Product { page } = kind
                && let Some((id, version)) = crate::snapshot::check_product_wal(path, page)?
            {
                if i64::from(id) != APPLICATION_ID {
                    return Err(StorageError::ForeignApplication);
                }
                check_identity(i64::from(id), i64::from(version))?;
            }
            if kind == Header::Unmarked {
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
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Waits up to the busy timeout for a concurrent creator's journal to clear.
/// Leftovers that stay unchanged through the second half of the wait refuse as
/// `Corrupt`; ones that changed in it, or any seen with a zero timeout, refuse
/// as `Busy`. SQLite is never reached.
fn settle(path: &Path, config: &StorageConfig) -> Result<bool, StorageError> {
    let wait = Duration::from_millis(config.busy_timeout_ms.into());
    let start = Instant::now();
    let deadline = start + wait;
    let mut seen = stamp(path);
    let mut changed = None;
    loop {
        let mut outcome = Err(StorageError::Corrupt);
        let mut stable = false;
        let mut blocked = residue(path)?;
        if !blocked {
            let before = stamp(path);
            outcome = inspect(path, config);
            blocked = residue(path)?;
            if blocked {
                outcome = Err(StorageError::Corrupt);
            } else {
                stable = outcome.is_ok() || stamp(path) == before;
            }
        }
        let now = stamp(path);
        if blocked && now != seen {
            changed = Some(Instant::now());
        }
        seen = now;
        if stable || Instant::now() >= deadline {
            let moving = changed.is_some_and(|at| at >= start + wait / 2);
            if blocked && (moving || config.busy_timeout_ms == 0) {
                return Err(StorageError::Busy);
            }
            return outcome;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Functions the product's statements, `CHECK`s and triggers call.
const FUNCTIONS: [&str; 6] = ["coalesce", "count", "length", "max", "substr", "zeroblob"];
/// Pragmas the product reads back after migration (`verify_pragmas`), plus
/// `data_version` and `shrink_memory` for the check before an anchored append.
const PRAGMAS: [&str; 10] = [
    "data_version",
    "shrink_memory",
    "user_version",
    "journal_mode",
    "synchronous",
    "foreign_keys",
    "trusted_schema",
    "cell_size_check",
    "busy_timeout",
    "application_id",
];

/// Statements the product connection may prepare once migrated: reads, appends,
/// transactions, the listed functions and queries of the listed pragmas. Schema changes, ATTACH, VACUUM, savepoints, other functions,
/// pragma writes, UPDATE and DELETE are denied even through the product's own
/// connection.
fn allowed(context: AuthContext<'_>) -> Authorization {
    let listed = |list: &[&str], name: &str| list.iter().any(|n| n.eq_ignore_ascii_case(name));
    match context.action {
        AuthAction::Select | AuthAction::Read { .. } | AuthAction::Transaction { .. } => {
            Authorization::Allow
        }
        AuthAction::Insert { table_name } if !table_name.eq_ignore_ascii_case("log_genesis") => {
            Authorization::Allow
        }
        AuthAction::Function { function_name } if listed(&FUNCTIONS, function_name) => {
            Authorization::Allow
        }
        AuthAction::Pragma {
            pragma_name,
            pragma_value: None,
        } if listed(&PRAGMAS, pragma_name) => Authorization::Allow,
        _ => Authorization::Deny,
    }
}

fn existing_flags() -> OpenFlags {
    OpenFlags::SQLITE_OPEN_READ_WRITE
}

impl AuditStore {
    /// Validates the header before SQLite can access the file.
    pub fn open(path: &Path, config: &StorageConfig) -> Result<Self, StorageError> {
        Self::open_after_inspection(path, config, true, || {})
    }

    pub(crate) fn open_existing(path: &Path, config: &StorageConfig) -> Result<Self, StorageError> {
        Self::open_after_inspection(path, config, false, || {})
    }

    fn open_after_inspection(
        path: &Path,
        config: &StorageConfig,
        create: bool,
        after: impl FnOnce(),
    ) -> Result<Self, StorageError> {
        let exists = settle(path, config)?;
        after();
        let flags = if exists {
            existing_flags()
        } else if create {
            existing_flags() | OpenFlags::SQLITE_OPEN_CREATE
        } else {
            return Err(StorageError::AnchorMismatch);
        };
        let mut connection = Connection::open_with_flags(path, flags)?;
        // A refusal must neither checkpoint the frames it refused nor delete the WAL,
        // whatever the WAL held when it was looked at.
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)?;
        let inspection =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)?;
        preflight(&inspection, config)?;
        inspection.commit()?;
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, false)?;
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
        connection.authorizer(Some(allowed))?;
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
        if !self
            .connection
            .db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE)?
        {
            return Err(StorageError::PragmaMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn test_directory() -> std::path::PathBuf {
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
        let result = AuditStore::open_after_inspection(&path, &config, true, || {
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
    fn defensive_mode_is_read_back() {
        let root = test_directory();
        let path = root.join(format!("defensive-{}.db", std::process::id()));
        let config = StorageConfig::defaults();
        let store = AuditStore::open(&path, &config).unwrap();
        store.verify_pragmas(&config).unwrap();
        store
            .connection
            .set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE, false)
            .unwrap();
        assert_eq!(
            store.verify_pragmas(&config),
            Err(StorageError::PragmaMismatch)
        );
    }

    #[test]
    fn product_connection_refuses_schema_changes_after_migration() {
        let root = test_directory();
        let path = root.join(format!("authorizer-{}.db", std::process::id()));
        let config = StorageConfig::defaults();
        let store = AuditStore::open(&path, &config).unwrap();
        for sql in [
            "DROP TRIGGER event_no_update",
            "CREATE TRIGGER t BEFORE INSERT ON meta BEGIN SELECT 1; END",
            "CREATE TABLE extra (x)",
            "CREATE TEMP TABLE extra (x)",
            "CREATE INDEX extra ON event(stream)",
            "DROP TABLE meta",
            "ALTER TABLE meta ADD COLUMN x",
            "ATTACH DATABASE ':memory:' AS other",
            "PRAGMA writable_schema = 1",
            "PRAGMA journal_mode = DELETE",
            "UPDATE meta SET value = value",
            "DELETE FROM meta",
            "INSERT OR REPLACE INTO log_genesis VALUES (1, zeroblob(32))",
            "REINDEX",
            "ANALYZE",
            "VACUUM",
            "SELECT load_extension('x')",
            "SELECT hex(1)",
            "SAVEPOINT s",
            "PRAGMA wal_checkpoint",
            "PRAGMA optimize",
        ] {
            let error = store.connection.execute_batch(sql).unwrap_err();
            assert_eq!(StorageError::from(error), StorageError::Denied, "{sql}");
        }
        let copy = root.join("vacuum-copy.db");
        let error = store
            .connection
            .execute_batch(&format!("VACUUM INTO '{}'", copy.display()))
            .unwrap_err();
        assert_eq!(StorageError::from(error), StorageError::Denied);
        assert!(!copy.exists());
        // Reads, appends and pragma queries still work, and so does reopening.
        store.verify_pragmas(&config).unwrap();
        store
            .connection
            .execute("INSERT INTO meta VALUES ('k', x'01')", [])
            .unwrap();
        drop(store);
        AuditStore::open(&path, &config).unwrap();
        let triggers: i64 = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'trigger'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(triggers, 10);
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
            (
                APPLICATION_ID as i32,
                SCHEMA_VERSION as i32 + 1,
                StorageError::SchemaTooNew,
            ),
            (APPLICATION_ID as i32, -1, StorageError::UnsupportedSchema),
            (APPLICATION_ID as i32, 2, StorageError::RetiredSchema),
            (0, 2, StorageError::UnsupportedSchema),
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
