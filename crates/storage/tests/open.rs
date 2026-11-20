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
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(tables, ["cipher_key", "event", "meta", "sqlite_sequence"]);
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

/// Like `refused`, but a WAL-format product file that SQLite inspected may gain an
/// empty `-wal` and a `-shm`: closing without a checkpoint does not remove them.
fn refused_by_sqlite(path: &std::path::Path, config: &StorageConfig, expected: StorageError) {
    let before = snapshot(path);
    assert_eq!(AuditStore::open(path, config).unwrap_err(), expected);
    let name = |suffix| sibling(path, suffix).file_name().unwrap().to_os_string();
    let after: Vec<_> = snapshot(path)
        .into_iter()
        .filter(|(entry, bytes)| {
            let new = !before.iter().any(|(old, _)| old == entry);
            !(new && (*entry == name("-shm") || (*entry == name("-wal") && bytes.is_empty())))
        })
        .collect();
    assert!(after == before, "directory entries or bytes changed");
}

#[test]
fn rejects_entire_unknown_schema_keeping_main_file() {
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
        refused_by_sqlite(
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
        (
            APPLICATION_ID,
            SCHEMA_VERSION + 1,
            StorageError::SchemaTooNew,
        ),
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
    refused_by_sqlite(&path, &StorageConfig::defaults(), StorageError::Corrupt);
    // The leftover empty `-wal` and `-shm` do not block the next open.
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

/// Short wait so leftover files are judged as stuck rather than in progress.
fn short() -> StorageConfig {
    StorageConfig::from_toml_str("busy_timeout_ms = 50").unwrap()
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
        refused(&path, &short(), StorageError::Corrupt);
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
        refused(&path, &short(), StorageError::Corrupt);
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

fn reseal_header(wal: &mut [u8]) -> [u32; 2] {
    let big = word(wal) & 1 == 1;
    let sum = wal_checksum(&wal[..24], [0, 0], big);
    wal[24..28].copy_from_slice(&sum[0].to_be_bytes());
    wal[28..32].copy_from_slice(&sum[1].to_be_bytes());
    sum
}

/// Recomputes header and frame checksums so only the edited field is wrong.
fn reseal(wal: &mut [u8]) {
    let big = word(wal) & 1 == 1;
    let page = word(&wal[8..]) as usize;
    let mut sum = reseal_header(wal);
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
    // A valid but different page size would make SQLite misalign every frame.
    let mut page = wal.clone();
    page[8..12].copy_from_slice(&(word(&wal[8..]) * 2).to_be_bytes());
    reseal_header(&mut page);
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

/// Main file and pending WAL of a product file, copied while the writer is open.
fn pending_product_wal(statements: &[&str]) -> (Vec<u8>, Vec<u8>) {
    let source = fixture();
    drop(AuditStore::open(&source, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&source).unwrap();
    c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    for sql in statements {
        c.execute_batch(sql).unwrap();
    }
    let pair = (
        std::fs::read(&source).unwrap(),
        std::fs::read(sibling(&source, "-wal")).unwrap(),
    );
    drop(c);
    pair
}

/// Refuses and leaves the main file, the WAL and every other entry byte-identical;
/// only a `-shm` may appear.
fn refused_keeping_pair(path: &std::path::Path, expected: StorageError) {
    assert!(sibling(path, "-wal").exists());
    refused_by_sqlite(path, &StorageConfig::defaults(), expected);
}

#[test]
fn refusal_keeps_pending_product_wal_out_of_main_file() {
    let (main, wal) = pending_product_wal(&["CREATE TABLE extra (x)"]);
    refused_keeping_pair(&write_pair(&main, &wal), StorageError::UnsupportedSchema);
}

/// Frame offsets of each transaction, in order.
fn transactions(wal: &[u8]) -> Vec<Vec<usize>> {
    let page = word(&wal[8..]) as usize;
    let mut all = vec![vec![]];
    for (index, frame) in wal[32..].chunks_exact(24 + page).enumerate() {
        all.last_mut().unwrap().push(32 + index * (24 + page));
        if word(&frame[4..]) != 0 {
            all.push(vec![]);
        }
    }
    all.pop();
    all
}

#[test]
fn damaged_product_wal_frame_before_later_commit_is_refused() {
    let (main, wal) = pending_product_wal(&[
        "INSERT INTO meta VALUES ('first', x'01')",
        "INSERT INTO meta VALUES ('second', zeroblob(20000))",
        "INSERT INTO meta VALUES ('third', x'03')",
    ]);
    let all = transactions(&wal);
    assert_eq!(all.len(), 3);
    assert!(all[1].len() > 1);
    // Break in the middle of a transaction, and in its commit frame.
    for frame in [all[1][0], *all[1].last().unwrap()] {
        let mut damaged = wal.clone();
        damaged[frame + 24 + 100] ^= 1;
        refused_keeping_pair(&write_pair(&main, &damaged), StorageError::Corrupt);
    }
    // A torn last transaction is an ordinary crash: earlier commits survive.
    let mut torn = wal.clone();
    torn[all[2][0] + 24 + 100] ^= 1;
    let path = write_pair(&main, &torn);
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&path).unwrap();
    let keys: Vec<String> = c
        .prepare("SELECT key FROM meta ORDER BY key")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(keys, ["first", "second"]);
}

fn keys(path: &std::path::Path) -> Vec<String> {
    Connection::open(path)
        .unwrap()
        .prepare("SELECT key FROM meta ORDER BY key")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn product_wal_with_older_generation_tail_opens() {
    let source = fixture();
    drop(AuditStore::open(&source, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&source).unwrap();
    c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    for key in ["a", "b", "c"] {
        c.execute("INSERT INTO meta VALUES (?1, zeroblob(5000))", [key])
            .unwrap();
    }
    c.query_row("PRAGMA wal_checkpoint(RESTART)", [], |_| Ok(()))
        .unwrap();
    c.execute("INSERT INTO meta VALUES ('d', x'04')", [])
        .unwrap();
    let main = std::fs::read(&source).unwrap();
    let wal = std::fs::read(sibling(&source, "-wal")).unwrap();
    drop(c);
    let page = word(&wal[8..]) as usize;
    let frames: Vec<&[u8]> = wal[32..].chunks_exact(24 + page).collect();
    let current = frames
        .iter()
        .take_while(|f| f[8..16] == wal[16..24])
        .count();
    let older = &frames[current..];
    assert!(current > 0);
    assert!(older.iter().all(|f| f[8..16] != wal[16..24]));
    assert!(older.iter().filter(|f| word(&f[4..]) != 0).count() >= 2);
    let path = write_pair(&main, &wal);
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    assert_eq!(keys(&path), ["a", "b", "c", "d"]);
}

#[test]
fn product_wal_frame_with_damaged_salt_before_later_commit_is_refused() {
    let (main, wal) = pending_product_wal(&[
        "INSERT INTO meta VALUES ('first', x'01')",
        "INSERT INTO meta VALUES ('second', zeroblob(20000))",
        "INSERT INTO meta VALUES ('third', zeroblob(20000))",
    ]);
    let all = transactions(&wal);
    assert_eq!(all.len(), 3);
    assert!(all[1].len() > 1 && all[2].len() > 1);
    // The salt is outside the frame checksum, so no reseal is needed.
    let mut middle = wal.clone();
    middle[all[1][0] + 8] ^= 1;
    refused_keeping_pair(&write_pair(&main, &middle), StorageError::Corrupt);
    let mut last = wal.clone();
    last[all[2][0] + 8] ^= 1;
    refused_keeping_pair(&write_pair(&main, &last), StorageError::Corrupt);
}

#[test]
fn product_wal_invalid_frame_before_final_transaction_commit_is_refused() {
    let (main, wal) = pending_product_wal(&[
        "INSERT INTO meta VALUES ('first', x'01')",
        "INSERT INTO meta VALUES ('second', zeroblob(20000))",
    ]);
    let all = transactions(&wal);
    assert_eq!(all.len(), 2);
    assert!(all[1].len() >= 3);
    let frame = all[1][1];
    let frame_size = 24 + word(&wal[8..]) as usize;

    let mut damaged = wal.clone();
    damaged[frame + 24 + 100] ^= 1;
    let mut zeroed = wal.clone();
    zeroed[frame..frame + frame_size].fill(0);

    let mut older = wal.clone();
    let previous_salt = word(&wal[16..]).wrapping_sub(1).to_be_bytes();
    older[16..20].copy_from_slice(&previous_salt);
    let salt: [u8; 8] = older[16..24].try_into().unwrap();
    for old_frame in older[32..].chunks_exact_mut(frame_size) {
        old_frame[8..16].copy_from_slice(&salt);
    }
    reseal(&mut older);
    let mut reused = wal.clone();
    reused[frame..frame + frame_size].copy_from_slice(&older[frame..frame + frame_size]);

    for invalid in [damaged, zeroed, reused] {
        refused_keeping_pair(&write_pair(&main, &invalid), StorageError::Corrupt);
    }
}

#[test]
fn product_wal_with_big_endian_checksums_opens_and_detects_damage() {
    let (main, mut wal) = pending_product_wal(&[
        "INSERT INTO meta VALUES ('first', x'01')",
        "INSERT INTO meta VALUES ('second', x'02')",
    ]);
    wal[..4].copy_from_slice(&0x377f0683u32.to_be_bytes());
    reseal(&mut wal);
    let path = write_pair(&main, &wal);
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    assert_eq!(keys(&path), ["first", "second"]);

    let all = transactions(&wal);
    wal[all[0][0] + 24 + 100] ^= 1;
    refused_keeping_pair(&write_pair(&main, &wal), StorageError::Corrupt);
}

#[test]
fn product_wal_with_partial_last_frame_opens() {
    let (main, wal) = pending_product_wal(&[
        "INSERT INTO meta VALUES ('first', x'01')",
        "INSERT INTO meta VALUES ('second', x'02')",
    ]);
    let path = write_pair(&main, &wal[..wal.len() - 100]);
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    assert_eq!(keys(&path), ["first"]);
}

#[test]
fn product_wal_commit_with_damaged_salt_before_one_commit_is_refused() {
    let (main, mut wal) = pending_product_wal(&[
        "INSERT INTO meta VALUES ('first', x'01')",
        "INSERT INTO meta VALUES ('second', x'02')",
        "INSERT INTO meta VALUES ('third', x'03')",
    ]);
    let all = transactions(&wal);
    assert_eq!(all.len(), 3);
    let commit = *all[1].last().unwrap();
    wal[commit + 8] ^= 1;
    refused_keeping_pair(&write_pair(&main, &wal), StorageError::Corrupt);
}

#[test]
fn product_wal_commit_with_zeroed_marker_before_one_commit_is_refused() {
    let (main, mut wal) = pending_product_wal(&[
        "INSERT INTO meta VALUES ('first', x'01')",
        "INSERT INTO meta VALUES ('second', x'02')",
        "INSERT INTO meta VALUES ('third', x'03')",
    ]);
    let all = transactions(&wal);
    assert_eq!(all.len(), 3);
    let commit = *all[1].last().unwrap();
    wal[commit + 4..commit + 8].fill(0);
    wal[commit + 24 + 100] ^= 1;
    refused_keeping_pair(&write_pair(&main, &wal), StorageError::Corrupt);
}

#[test]
fn integrity_refusal_with_pending_wal_keeps_main_file_and_wal() {
    let source = fixture();
    drop(AuditStore::open(&source, &StorageConfig::defaults()).unwrap());
    let c = Connection::open(&source).unwrap();
    c.execute("INSERT INTO meta VALUES ('payload', zeroblob(20000))", [])
        .unwrap();
    drop(c);
    // The last page is the end of the payload's overflow chain: make it loop.
    let mut bytes = std::fs::read(&source).unwrap();
    let page = u16::from_be_bytes([bytes[16], bytes[17]]) as usize;
    let last = bytes.len() / page;
    bytes[(last - 1) * page..(last - 1) * page + 4].copy_from_slice(&(last as u32).to_be_bytes());
    std::fs::write(&source, &bytes).unwrap();
    let c = Connection::open(&source).unwrap();
    c.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    c.execute("INSERT INTO meta VALUES ('pending', x'01')", [])
        .unwrap();
    let main = std::fs::read(&source).unwrap();
    let wal = std::fs::read(sibling(&source, "-wal")).unwrap();
    drop(c);
    assert_eq!(main, bytes);
    assert!(wal.len() > 32);
    assert!(
        wal[32..]
            .chunks_exact(24 + page)
            .all(|frame| word(frame) as usize != last)
    );
    refused_keeping_pair(&write_pair(&main, &wal), StorageError::Corrupt);
    let config = StorageConfig::from_toml_str("integrity_check_on_open = false").unwrap();
    drop(AuditStore::open(&write_pair(&main, &wal), &config).unwrap());
}

#[test]
fn short_wal_beside_refused_product_file_is_kept() {
    let (_, wal) = pending_product_wal(&["INSERT INTO meta VALUES ('pending', x'01')"]);
    let source = fixture();
    drop(AuditStore::open(&source, &StorageConfig::defaults()).unwrap());
    Connection::open(&source)
        .unwrap()
        .execute_batch("CREATE TABLE extra (x)")
        .unwrap();
    assert!(!sibling(&source, "-wal").exists());
    let main = std::fs::read(&source).unwrap();
    // A valid header with no frames, and lengths SQLite reads as no WAL at all.
    for short in [wal[..32].to_vec(), vec![], vec![7], vec![7; 31]] {
        refused_keeping_pair(&write_pair(&main, &short), StorageError::UnsupportedSchema);
    }
}

#[test]
fn leftover_journal_with_zero_timeout_is_busy() {
    let path = fixture();
    std::fs::write(&path, []).unwrap();
    std::fs::write(sibling(&path, "-journal"), b"in progress").unwrap();
    refused(&path, &immediate(), StorageError::Busy);
}

#[test]
fn journal_still_changing_at_deadline_is_busy() {
    let path = fixture();
    std::fs::write(&path, []).unwrap();
    let journal = sibling(&path, "-journal");
    std::fs::write(&journal, b"0").unwrap();
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut length = 1;
            while !done.load(Ordering::Relaxed) {
                length += 1;
                std::fs::write(&journal, vec![1; length]).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });
        let config = StorageConfig::from_toml_str("busy_timeout_ms = 200").unwrap();
        let result = AuditStore::open(&path, &config).map(drop);
        done.store(true, Ordering::Relaxed);
        assert_eq!(result, Err(StorageError::Busy));
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"");
}

#[test]
fn journal_changed_once_then_still_until_deadline_is_corrupt() {
    let path = fixture();
    std::fs::write(&path, []).unwrap();
    let journal = sibling(&path, "-journal");
    std::fs::write(&journal, b"0").unwrap();
    let begin = std::time::Instant::now();
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::write(&journal, b"01").unwrap();
            std::time::Instant::now()
        });
        let config = StorageConfig::from_toml_str("busy_timeout_ms = 2000").unwrap();
        let result = AuditStore::open(&path, &config).map(drop);
        let written = writer.join().unwrap() - begin;
        assert!(
            written < std::time::Duration::from_millis(1000),
            "environment too slow: write landed after {written:?}"
        );
        assert_eq!(result, Err(StorageError::Corrupt));
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"");
    assert_eq!(std::fs::read(&journal).unwrap(), b"01");
}

#[test]
fn journal_changed_only_in_second_half_of_wait_is_busy() {
    let path = fixture();
    std::fs::write(&path, []).unwrap();
    let journal = sibling(&path, "-journal");
    std::fs::write(&journal, b"0").unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(650));
            std::fs::write(&journal, b"01").unwrap();
        });
        let config = StorageConfig::from_toml_str("busy_timeout_ms = 1000").unwrap();
        let result = AuditStore::open(&path, &config).map(drop);
        assert_eq!(result, Err(StorageError::Busy));
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"");
    assert_eq!(std::fs::read(&journal).unwrap(), b"01");
}

#[test]
fn empty_wal_left_by_refusal_blocks_a_new_file_once_main_is_moved() {
    let path = fixture();
    drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE extra (x)")
        .unwrap();
    assert!(!sibling(&path, "-wal").exists());
    refused_by_sqlite(
        &path,
        &StorageConfig::defaults(),
        StorageError::UnsupportedSchema,
    );
    assert_eq!(std::fs::read(sibling(&path, "-wal")).unwrap(), b"");
    std::fs::rename(&path, sibling(&path, ".moved")).unwrap();
    refused(&path, &short(), StorageError::Corrupt);
    assert!(!path.exists());
}

#[test]
fn inert_journal_beside_product_file_is_refused() {
    for journal in [vec![], vec![0; 512]] {
        let path = fixture();
        drop(AuditStore::open(&path, &StorageConfig::defaults()).unwrap());
        std::fs::write(sibling(&path, "-journal"), journal).unwrap();
        refused(&path, &short(), StorageError::Corrupt);
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
