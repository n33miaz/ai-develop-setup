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
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(root.starts_with(&workspace));
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    assert!(root.starts_with(workspace.canonicalize().unwrap()));
    let dir = root.join(format!(
        "storage-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(dir.starts_with(&root));
    std::fs::create_dir(&dir).unwrap();
    dir.join("audit.db")
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
        assert_eq!(
            AuditStore::open(&path, &StorageConfig::defaults()).unwrap_err(),
            expected
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
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
        assert_eq!(
            AuditStore::open(&path, &StorageConfig::defaults()).unwrap_err(),
            StorageError::Corrupt
        );
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
        assert_eq!(
            AuditStore::open(&truncated, &StorageConfig::defaults()).unwrap_err(),
            StorageError::Corrupt
        );
        assert_eq!(std::fs::read(&truncated).unwrap(), bytes[..length]);
    }
}
