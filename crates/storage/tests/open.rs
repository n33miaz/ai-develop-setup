use ads_storage::{
    AuditStore, StorageConfig, StorageError,
    open::{APPLICATION_ID, SCHEMA_VERSION},
};
use rusqlite::Connection;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn fixture() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    assert!(root.is_absolute());
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    allocate_fixture(&root, &NEXT)
}

fn allocate_fixture(root: &std::path::Path, next: &AtomicU64) -> PathBuf {
    loop {
        let dir = root.join(format!(
            "storage-{}-{}",
            std::process::id(),
            next.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(dir.starts_with(root));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir.join("audit.db"),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("fixture directory: {error}"),
        }
    }
}

#[test]
fn fixture_collisions_preserve_existing_directory() {
    let parent = fixture().parent().unwrap().to_path_buf();
    let first = allocate_fixture(&parent, &AtomicU64::new(0));
    std::fs::write(&first, b"preserved fixture").unwrap();
    let second = allocate_fixture(&parent, &AtomicU64::new(0));
    assert_ne!(first, second);
    assert_eq!(std::fs::read(first).unwrap(), b"preserved fixture");
}

#[test]
fn reopen_preserves_meta_and_reads_every_pragma() {
    let path = fixture();
    let config = StorageConfig::from_toml_str("busy_timeout_ms = 123").unwrap();
    let store = AuditStore::open(&path, &config).unwrap();
    store.verify_pragmas(&config).unwrap();
    let different_timeout = StorageConfig::from_toml_str("busy_timeout_ms = 124").unwrap();
    assert_eq!(
        store.verify_pragmas(&different_timeout),
        Err(StorageError::PragmaMismatch)
    );
    let diagnostic = format!("{store:?} {config:?}");
    assert!(!diagnostic.contains(&path.to_string_lossy().to_string()));
    drop(store);
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("INSERT INTO meta VALUES ('sentinel', x'010203')", [])
        .unwrap();
    drop(connection);
    let store = AuditStore::open(&path, &config).unwrap();
    store.verify_pragmas(&config).unwrap();
    drop(store);
    let connection = Connection::open(&path).unwrap();
    let value: Vec<u8> = connection
        .query_row("SELECT value FROM meta WHERE key='sentinel'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, [1, 2, 3]);
    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(tables, ["meta"]);
}

#[test]
fn refuses_foreign_newer_and_truncated_files_without_modification() {
    for (id, version, expected) in [
        (99, 0, StorageError::ForeignApplication),
        (
            APPLICATION_ID,
            SCHEMA_VERSION + 1,
            StorageError::SchemaTooNew,
        ),
    ] {
        let path = fixture();
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "application_id", id)
            .unwrap();
        connection
            .pragma_update(None, "user_version", version)
            .unwrap();
        drop(connection);
        let before = std::fs::read(&path).unwrap();
        let directory_before = snapshot(&path);
        assert_eq!(
            AuditStore::open(&path, &StorageConfig::defaults()).unwrap_err(),
            expected
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(snapshot(&path), directory_before);
    }
    for bytes in [
        b"SQLite format 3\0".as_slice(),
        b"not a database".as_slice(),
        &[0; 100],
    ] {
        let path = fixture();
        assert!(
            path.starts_with(
                PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                    .canonicalize()
                    .unwrap()
            )
        );
        std::fs::write(&path, bytes).unwrap();
        refused(&path, &StorageConfig::defaults(), StorageError::Corrupt);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let bytes = std::fs::read(&path).unwrap();
    for length in [16, 99, 100, bytes.len() / 2] {
        let truncated = fixture();
        assert!(
            truncated.starts_with(
                PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                    .canonicalize()
                    .unwrap()
            )
        );
        std::fs::write(&truncated, &bytes[..length]).unwrap();
        refused(
            &truncated,
            &StorageConfig::defaults(),
            StorageError::Corrupt,
        );
        assert_eq!(std::fs::read(&truncated).unwrap(), bytes[..length]);
    }
}

fn snapshot(path: &std::path::Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut entries: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name(),
                if entry.path().is_file() {
                    std::fs::read(entry.path()).unwrap()
                } else {
                    vec![]
                },
            )
        })
        .collect();
    entries.sort();
    entries
}

fn refused(path: &std::path::Path, config: &StorageConfig, expected: StorageError) {
    let before = snapshot(path);
    assert_eq!(AuditStore::open(path, config).unwrap_err(), expected);
    assert!(
        snapshot(path) == before,
        "directory entries or bytes changed"
    );
}

#[test]
fn rejects_entire_unknown_schema_without_directory_changes() {
    for sql in [
        "CREATE TABLE extra (x)",
        "CREATE VIEW extra AS SELECT * FROM meta",
        "CREATE INDEX extra ON meta(value)",
        "CREATE TRIGGER extra BEFORE INSERT ON meta BEGIN SELECT RAISE(IGNORE); END",
        "DROP TABLE meta",
        "DROP TABLE meta; CREATE TABLE meta (key TEXT, value BLOB)",
    ] {
        let path = fixture();
        drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
        let c = Connection::open(&path).unwrap();
        c.execute_batch(sql).unwrap();
        refused(
            &path,
            &StorageConfig::defaults(),
            StorageError::UnsupportedSchema,
        );
    }
}

#[test]
fn header_identity_and_format_refusals_preserve_directory() {
    for (id, version, expected) in [
        (0, 1, StorageError::UnsupportedSchema),
        (APPLICATION_ID, -1, StorageError::UnsupportedSchema),
        (APPLICATION_ID, 0, StorageError::UnsupportedSchema),
        (APPLICATION_ID, 2, StorageError::SchemaTooNew),
        (99, 0, StorageError::ForeignApplication),
        (0, 0, StorageError::ForeignApplication),
    ] {
        let path = fixture();
        let c = Connection::open(&path).unwrap();
        c.execute_batch("CREATE TABLE foreign_data (x)").unwrap();
        c.pragma_update(None, "application_id", id).unwrap();
        c.pragma_update(None, "user_version", version).unwrap();
        drop(c);
        refused(&path, &StorageConfig::defaults(), expected);
    }
    for offset in [0, 16, 18, 19] {
        let path = fixture();
        drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[offset] = 0;
        std::fs::write(&path, bytes).unwrap();
        refused(&path, &StorageConfig::defaults(), StorageError::Corrupt);
    }
}

#[test]
fn foreign_wal_with_pending_frames_is_untouched() {
    for id in [0, 99] {
        let path = fixture();
        let c = Connection::open(&path).unwrap();
        c.pragma_update(None, "application_id", id).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        c.execute_batch("CREATE TABLE foreign_data (x); INSERT INTO foreign_data VALUES (1)")
            .unwrap();
        assert!(
            std::fs::metadata(path.with_extension("db-wal"))
                .unwrap()
                .len()
                > 32
        );
        refused(
            &path,
            &StorageConfig::defaults(),
            StorageError::ForeignApplication,
        );
    }
}

#[test]
fn empty_file_is_initialized_and_directory_is_not_corrupt() {
    let path = fixture();
    std::fs::write(&path, []).unwrap();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let directory = fixture();
    std::fs::create_dir(&directory).unwrap();
    refused(
        &directory,
        &StorageConfig::defaults(),
        StorageError::CantOpen,
    );
}

#[test]
fn repeated_migration_preserves_file_bytes() {
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let before = snapshot(&path);
    for _ in 0..2 {
        drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
        assert_eq!(snapshot(&path), before);
    }
}

#[test]
fn busy_preserves_all_files() {
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&path).unwrap();
    c.pragma_update(None, "journal_mode", "DELETE").unwrap();
    c.execute_batch("BEGIN IMMEDIATE").unwrap();
    let config = StorageConfig::from_toml_str("busy_timeout_ms = 0").unwrap();
    refused(&path, &config, StorageError::Busy);
}

#[test]
fn corrupt_internal_page_obeys_integrity_setting() {
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&path).unwrap();
    c.execute("INSERT INTO meta VALUES ('payload', zeroblob(20000))", [])
        .unwrap();
    drop(c);
    let mut bytes = std::fs::read(&path).unwrap();
    // Overflow page points to itself; schema and header remain readable.
    let page = u16::from_be_bytes([bytes[16], bytes[17]]) as usize;
    bytes[2 * page..2 * page + 4].copy_from_slice(&3u32.to_be_bytes());
    std::fs::write(&path, bytes).unwrap();
    refused(&path, &StorageConfig::defaults(), StorageError::Corrupt);
    let config = StorageConfig::from_toml_str("integrity_check_on_open = false").unwrap();
    drop(AuditStore::open(&path, &config).unwrap());
}

#[test]
fn simultaneous_creation_leaves_usable_database() {
    let path = fixture();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let barrier = barrier.clone();
                let path = &path;
                scope.spawn(move || {
                    barrier.wait();
                    match AuditStore::open(path, &StorageConfig::defaults()) {
                        Ok(store) => drop(store),
                        Err(error) => assert_eq!(error, StorageError::Busy),
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
    });
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
}

#[test]
fn inspection_limit_refuses_without_writes() {
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let config = StorageConfig::from_toml_str("inspection_max_bytes = 512").unwrap();
    refused(&path, &config, StorageError::Full);
}

#[test]
fn committed_owned_wal_can_be_reopened() {
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&path).unwrap();
    c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    c.execute("INSERT INTO meta VALUES ('pending', x'01')", [])
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let value: Vec<u8> = c
        .query_row("SELECT value FROM meta WHERE key='pending'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, [1]);
}

#[test]
fn invalid_path_is_cant_open() {
    let path = fixture().with_file_name("invalid\0.db");
    refused(&path, &StorageConfig::defaults(), StorageError::CantOpen);
}
