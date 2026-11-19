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
        drop(c);
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
    for _ in 0..30 {
        let path = fixture();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let results: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let barrier = barrier.clone();
                    let path = &path;
                    scope.spawn(move || {
                        barrier.wait();
                        AuditStore::open(path, &StorageConfig::defaults()).map(drop)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(results.contains(&Ok(())), "{results:?}");
        assert!(
            results
                .iter()
                .all(|r| matches!(r, Ok(()) | Err(StorageError::Busy))),
            "{results:?}"
        );
        drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    }
}

#[test]
fn inspection_limit_refuses_without_writes() {
    let path = fixture();
    let c = Connection::open(&path).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    drop(c);
    assert!(std::fs::metadata(&path).unwrap().len() > 512);
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

fn sibling(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

fn immediate() -> StorageConfig {
    StorageConfig::from_toml_str("busy_timeout_ms = 0").unwrap()
}

#[test]
fn auxiliary_file_beside_empty_or_missing_main_is_refused() {
    for (main, suffix) in [
        (Some(b"".as_slice()), "-wal"),
        (Some(b"".as_slice()), "-journal"),
        (None, "-wal"),
        (None, "-journal"),
    ] {
        let path = fixture();
        if let Some(bytes) = main {
            std::fs::write(&path, bytes).unwrap();
        }
        std::fs::write(sibling(&path, suffix), b"committed frames").unwrap();
        refused(&path, &immediate(), StorageError::Corrupt);
        assert_eq!(path.exists(), main.is_some());
    }
}

#[test]
fn creation_waits_for_journal_of_concurrent_creator() {
    let path = fixture();
    std::fs::write(&path, []).unwrap();
    let journal = sibling(&path, "-journal");
    std::fs::write(&journal, b"in progress").unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = std::fs::remove_file(&journal);
        });
        let config = StorageConfig::from_toml_str("busy_timeout_ms = 5000").unwrap();
        drop(AuditStore::open(&path, &config).unwrap());
    });
}

const JOURNAL_MAGIC: [u8; 8] = [0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7];

#[test]
fn interrupted_rollback_journal_is_refused_before_replay() {
    // Product file with a transaction spilled to the main file mid-write.
    let source = fixture();
    drop(AuditStore::open(&source, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&source).unwrap();
    c.pragma_update(None, "journal_mode", "DELETE").unwrap();
    c.execute("INSERT INTO meta VALUES ('payload', zeroblob(200000))", [])
        .unwrap();
    c.pragma_update(None, "cache_size", 2).unwrap();
    c.execute_batch("BEGIN; UPDATE meta SET value = randomblob(200000)")
        .unwrap();
    let product = fixture();
    std::fs::write(&product, std::fs::read(&source).unwrap()).unwrap();
    let journal = std::fs::read(sibling(&source, "-journal")).unwrap();
    assert_eq!(journal[..8], JOURNAL_MAGIC);
    std::fs::write(sibling(&product, "-journal"), journal).unwrap();
    drop(c);
    // Unmarked file left by a creation interrupted while switching to WAL.
    let unmarked = fixture();
    let c = Connection::open(&unmarked).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    drop(c);
    let mut header = vec![0; 512];
    header[..8].copy_from_slice(&JOURNAL_MAGIC);
    header[20..24].copy_from_slice(&512u32.to_be_bytes());
    header[24..28].copy_from_slice(&4096u32.to_be_bytes());
    std::fs::write(sibling(&unmarked, "-journal"), header).unwrap();
    for path in [product, unmarked] {
        refused(&path, &immediate(), StorageError::Corrupt);
    }
}

fn word(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap())
}

fn wal_checksum(bytes: &[u8], mut sum: [u32; 2], big: bool) -> [u32; 2] {
    for pair in bytes.chunks_exact(8) {
        let read = |b: &[u8]| {
            let b: [u8; 4] = b.try_into().unwrap();
            if big {
                u32::from_be_bytes(b)
            } else {
                u32::from_le_bytes(b)
            }
        };
        sum[0] = sum[0].wrapping_add(read(&pair[..4])).wrapping_add(sum[1]);
        sum[1] = sum[1].wrapping_add(read(&pair[4..])).wrapping_add(sum[0]);
    }
    sum
}

/// Recomputes header and frame checksums so only the edited field is wrong.
fn reseal(wal: &mut [u8]) {
    let big = word(wal) & 1 == 1;
    let page = word(&wal[8..]) as usize;
    let mut sum = wal_checksum(&wal[..24], [0, 0], big);
    wal[24..28].copy_from_slice(&sum[0].to_be_bytes());
    wal[28..32].copy_from_slice(&sum[1].to_be_bytes());
    for frame in wal[32..].chunks_exact_mut(24 + page) {
        sum = wal_checksum(&frame[24..], wal_checksum(&frame[..8], sum, big), big);
        frame[16..20].copy_from_slice(&sum[0].to_be_bytes());
        frame[20..24].copy_from_slice(&sum[1].to_be_bytes());
    }
}

/// Unmarked main file plus a WAL whose second transaction adds a foreign table.
fn unmarked_wal() -> (Vec<u8>, Vec<u8>, usize, usize) {
    let source = fixture();
    let c = Connection::open(&source).unwrap();
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    c.pragma_update(None, "user_version", 0).unwrap();
    c.execute_batch("CREATE TABLE foreign_data (x)").unwrap();
    let main = std::fs::read(&source).unwrap();
    let wal = std::fs::read(sibling(&source, "-wal")).unwrap();
    drop(c);
    let page = word(&wal[8..]) as usize;
    let commits: Vec<usize> = wal[32..]
        .chunks_exact(24 + page)
        .enumerate()
        .filter(|(_, frame)| word(&frame[4..]) != 0)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(commits.len(), 2);
    let second = 32 + (commits[0] + 1) * (24 + page);
    let last = 32 + commits[1] * (24 + page);
    (main, wal, second, last)
}

fn write_pair(main: &[u8], wal: &[u8]) -> PathBuf {
    let path = fixture();
    std::fs::write(&path, main).unwrap();
    std::fs::write(sibling(&path, "-wal"), wal).unwrap();
    path
}

#[test]
fn invalid_wal_tail_is_ignored_by_inspection() {
    let (main, wal, second, last) = unmarked_wal();
    refused(
        &write_pair(&main, &wal),
        &immediate(),
        StorageError::ForeignApplication,
    );
    let mut checksum = wal.clone();
    checksum[second + 24 + 100] ^= 1;
    let mut salt = wal.clone();
    salt[second + 8] ^= 1;
    let mut uncommitted = wal.clone();
    uncommitted[last + 4..last + 8].copy_from_slice(&[0; 4]);
    reseal(&mut uncommitted);
    for wal in [checksum, salt, uncommitted] {
        let path = write_pair(&main, &wal);
        drop(AuditStore::open(&path, &immediate()).unwrap());
        let c = Connection::open(&path).unwrap();
        let tables: i64 = c
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'foreign_data'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0);
    }
}

#[test]
fn wal_with_wrong_magic_or_version_is_refused() {
    let (main, wal, _, _) = unmarked_wal();
    let mut magic = wal.clone();
    magic[3] = if wal[3] & 1 == 1 { 0x85 } else { 0x84 };
    reseal(&mut magic);
    let mut version = wal.clone();
    version[4..8].copy_from_slice(&3_007_001u32.to_be_bytes());
    reseal(&mut version);
    for wal in [magic, version] {
        refused(
            &write_pair(&main, &wal),
            &immediate(),
            StorageError::Corrupt,
        );
    }
}

#[test]
fn product_wal_with_damaged_header_is_refused() {
    let source = fixture();
    drop(AuditStore::open(&source, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&source).unwrap();
    c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    c.execute("INSERT INTO meta VALUES ('pending', x'01')", [])
        .unwrap();
    let main = std::fs::read(&source).unwrap();
    let wal = std::fs::read(sibling(&source, "-wal")).unwrap();
    drop(c);
    let mut magic = wal.clone();
    magic[3] = if wal[3] & 1 == 1 { 0x85 } else { 0x84 };
    reseal(&mut magic);
    let mut version = wal.clone();
    version[4..8].copy_from_slice(&3_007_001u32.to_be_bytes());
    reseal(&mut version);
    let mut page = wal.clone();
    page[8..12].copy_from_slice(&(word(&wal[8..]) * 2).to_be_bytes());
    let mut checksum = wal.clone();
    checksum[24] ^= 1;
    for wal in [magic, version, page, checksum] {
        refused(
            &write_pair(&main, &wal),
            &immediate(),
            StorageError::Corrupt,
        );
    }
}

#[test]
fn product_file_larger_than_inspection_limit_opens() {
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&path).unwrap();
    c.execute("INSERT INTO meta VALUES ('payload', zeroblob(20000))", [])
        .unwrap();
    drop(c);
    assert!(std::fs::metadata(&path).unwrap().len() > 512);
    let config = StorageConfig::from_toml_str("inspection_max_bytes = 512").unwrap();
    drop(AuditStore::open(&path, &config).unwrap());
}

/// Cost probe: `cargo test -p ads-storage --release -- --ignored --nocapture`.
#[test]
#[ignore]
fn open_time_of_large_product_file() {
    let mebibytes: usize = std::env::var("ADS_STORAGE_PROBE_MIB")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(48);
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&path).unwrap();
    for index in 0..mebibytes {
        c.execute(
            "INSERT INTO meta VALUES (?1, zeroblob(1048000))",
            [format!("payload-{index}")],
        )
        .unwrap();
    }
    drop(c);
    let size = std::fs::metadata(&path).unwrap().len();
    // Shipped limits; set ADS_STORAGE_PROBE_LIMIT to compare with a wider image cap.
    let limit = std::env::var("ADS_STORAGE_PROBE_LIMIT")
        .map(|bytes| format!("inspection_max_bytes = {bytes}\n"))
        .unwrap_or_default();
    for (label, text) in [
        ("integrity on", limit.clone()),
        (
            "integrity off",
            format!("{limit}integrity_check_on_open = false"),
        ),
    ] {
        let config = StorageConfig::from_toml_str(&text).unwrap();
        let start = std::time::Instant::now();
        let result = AuditStore::open(&path, &config).map(drop);
        println!("{size} bytes, {label}: {:?} {result:?}", start.elapsed());
    }
}
