//! Composition of the persistent audit log with the product vault.

use std::{
    fs::{self, File, TryLockError},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use ads_adapter_vault::OsVault;
use ads_core::vault::{Secret, SecretName, Vault};
use ads_private_dir::{ControlDir, Identity, is_network_path};
use ads_storage::{
    EventLog, StorageConfig, StorageError,
    anchor::{
        AnchorStore, COMMITMENT_VERSION, Commitment, DatabaseId, ReanchorPlan, RecoveryAnchor,
        genesis_id,
    },
    cipher::KeyProvider,
};
use sha2::{Digest, Sha256};

use crate::{gate::ProductState, startup::InstanceLock};

struct VaultAnchor<V> {
    vault: V,
    location: [u8; 32],
    canonical: bool,
    _directory: Option<ControlDir>,
}

fn normalized_path(path: &Path) -> Result<PathBuf, StorageError> {
    let parent = path.parent().ok_or(StorageError::CantOpen)?;
    let name = path.file_name().ok_or(StorageError::CantOpen)?;
    let normalized = fs::canonicalize(parent)?.join(name);
    match fs::canonicalize(&normalized) {
        Ok(resolved) if resolved == normalized => Ok(resolved),
        Ok(_) => Err(StorageError::NonCanonicalDatabase),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(normalized),
        Err(error) => Err(error.into()),
    }
}

impl<V: Vault> VaultAnchor<V> {
    fn new(vault: V, path: &Path, canonical: bool) -> Result<Self, StorageError> {
        let location = path.to_str().ok_or(StorageError::CantOpen)?;
        Ok(Self {
            vault,
            location: Sha256::digest(location.as_bytes()).into(),
            canonical,
            _directory: None,
        })
    }

    fn read_secret(&self, name: SecretName) -> Result<Option<Secret>, StorageError> {
        self.vault
            .read(name)
            .map_err(|_| StorageError::AnchorUnavailable)
    }

    fn store_secret(&self, name: SecretName, bytes: Vec<u8>) -> Result<(), StorageError> {
        let secret = Secret::new(bytes).map_err(|_| StorageError::AnchorUnavailable)?;
        self.vault
            .store(name, &secret)
            .map_err(|_| StorageError::AnchorUnavailable)
    }

    fn pinned_id(&self) -> Result<Option<DatabaseId>, StorageError> {
        if !self.canonical {
            return Ok(None);
        }
        self.read_secret(SecretName::AuditDatabase)?
            .map(|secret| {
                secret
                    .expose()
                    .try_into()
                    .map_err(|_| StorageError::AnchorMismatch)
            })
            .transpose()
    }

    fn scoped(&self, id: DatabaseId) -> Result<Option<([u8; 32], Commitment)>, StorageError> {
        self.read_secret(SecretName::AuditAnchorFor(id))?
            .map(|secret| {
                let (location, commitment) = secret
                    .expose()
                    .split_at_checked(self.location.len())
                    .ok_or(StorageError::AnchorMismatch)?;
                Ok((
                    location.try_into().expect("location is 32 bytes"),
                    decode(commitment)?,
                ))
            })
            .transpose()
    }

    fn check_pin(&self, id: DatabaseId) -> Result<(), StorageError> {
        if self.pinned_id()?.is_some_and(|pinned| pinned != id) {
            return Err(StorageError::AnchorMismatch);
        }
        Ok(())
    }
}

fn decode(bytes: &[u8]) -> Result<Commitment, StorageError> {
    let commitment: Commitment = bytes.try_into().map_err(|_| StorageError::AnchorMismatch)?;
    if commitment[0] != COMMITMENT_VERSION {
        return Err(StorageError::UnsupportedSchema);
    }
    Ok(commitment)
}

impl<V: Vault + Send> AnchorStore for VaultAnchor<V> {
    fn can_create(&self) -> Result<bool, StorageError> {
        Ok(self.pinned_id()?.is_none() && self.legacy()?.is_none())
    }

    fn read(&self, id: DatabaseId) -> Result<Option<Commitment>, StorageError> {
        self.check_pin(id)?;
        self.scoped(id)?
            .map(|(location, commitment)| {
                if location != self.location {
                    return Err(StorageError::DuplicateDatabaseIdentity);
                }
                Ok(commitment)
            })
            .transpose()
    }

    fn store(&self, id: DatabaseId, commitment: &Commitment) -> Result<(), StorageError> {
        self.store_secret(
            SecretName::AuditAnchorFor(id),
            [self.location.as_slice(), commitment].concat(),
        )
    }

    fn legacy(&self) -> Result<Option<Commitment>, StorageError> {
        if !self.canonical || self.pinned_id()?.is_some() {
            return Ok(None);
        }
        self.read_secret(SecretName::AuditAnchor)?
            .map(|secret| decode(secret.expose()))
            .transpose()
    }

    fn recovery_anchor(&self, id: DatabaseId) -> Result<RecoveryAnchor, StorageError> {
        let pinned = self.pinned_id()?;
        let scoped = self.scoped(pinned.unwrap_or(id))?;
        let (previous_location, commitment) = match scoped {
            Some((location, commitment)) => (Some(location), Some(commitment)),
            None => (None, self.legacy()?),
        };
        Ok(RecoveryAnchor {
            previous_id: pinned
                .or_else(|| scoped.map(|_| id))
                .or_else(|| commitment.as_ref().and_then(genesis_id)),
            commitment,
            previous_location,
            proposed_location: Some(self.location),
        })
    }

    fn recover(&self, plan: &ReanchorPlan) -> Result<(), StorageError> {
        let id = plan.summary().proposed_id;
        self.store(id, plan.proposed_commitment())?;
        if self.canonical && self.pinned_id()? != Some(id) {
            self.store_secret(SecretName::AuditDatabase, id.to_vec())?;
        }
        Ok(())
    }

    fn bind(&self, id: DatabaseId) -> Result<(), StorageError> {
        if !self.canonical {
            return Ok(());
        }
        match self.pinned_id()? {
            Some(pinned) if pinned != id => Err(StorageError::AnchorMismatch),
            Some(_) => Ok(()),
            None => self.store_secret(SecretName::AuditDatabase, id.to_vec()),
        }
    }
}

/// The per-user audit database used by the product on Windows.
pub fn canonical_path(config: &StorageConfig) -> Result<PathBuf, StorageError> {
    if let Some(path) = config.audit_database_path() {
        return Ok(path.to_path_buf());
    }
    Ok(vault_directory()?.join("audit.db"))
}

fn vault_directory() -> Result<PathBuf, StorageError> {
    if !cfg!(windows) {
        return Err(StorageError::AnchorUnavailable);
    }
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|root| root.is_absolute())
        .ok_or(StorageError::Configuration)?;
    Ok(root.join("ads"))
}

fn audit_access_error(error: std::io::Error) -> StorageError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotADirectory => {
            StorageError::AuditDirectoryNotPrivate
        }
        _ => error.into(),
    }
}

#[cfg(test)]
thread_local! {
    /// Stands in for the identity of the session, which a test cannot elevate.
    static SESSION: std::cell::RefCell<Option<Identity>> = const { std::cell::RefCell::new(None) };
}

fn session_identity() -> std::io::Result<Identity> {
    #[cfg(test)]
    if let Some(local) = SESSION.with(|session| session.borrow().clone()) {
        return Ok(local);
    }
    Identity::current()
}

/// Refuses a session whose new files would not belong to the user: the database and the
/// files SQLite adds next to it get the session's default owner, which the next open refuses.
pub fn check_session() -> Result<(), StorageError> {
    if session_identity()?.creates_as_user() {
        Ok(())
    } else {
        Err(StorageError::ElevatedSession)
    }
}

fn lock_binding(directory: &Path) -> Result<File, StorageError> {
    // SQLite serializes a single file. The vault pin spans all configured paths.
    let directory = ControlDir::open_user_owned(directory, true).map_err(audit_access_error)?;
    let lock = directory
        .lock_file("audit-binding.lock")
        .map_err(audit_access_error)?;

    lock.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => StorageError::Busy,
        TryLockError::Error(error) => error.into(),
    })?;
    Ok(lock)
}

fn check_audit_location(path: &Path, workdirs: &[PathBuf]) -> Result<(), StorageError> {
    if is_network_path(path) {
        return Err(StorageError::AuditDatabaseOnNetwork);
    }
    let directory = path.parent().ok_or(StorageError::CantOpen)?;
    // Resolve the deepest existing ancestor before creating anything.
    for ancestor in directory.ancestors() {
        match fs::canonicalize(ancestor) {
            Ok(resolved) => {
                if is_network_path(&resolved) {
                    return Err(StorageError::AuditDatabaseOnNetwork);
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if !workdirs.is_empty() {
        let paths = crate::gate::spellings(directory).map_err(|_| StorageError::CantOpen)?;
        for workdir in workdirs {
            let roots = crate::gate::spellings(workdir).map_err(|_| StorageError::Configuration)?;
            if paths
                .iter()
                .any(|path| roots.iter().any(|root| path.matches_denied_root(root)))
            {
                return Err(StorageError::AuditDatabaseInWorkdir);
            }
        }
    }
    Ok(())
}

fn canonical_anchor<V: Vault>(
    path: &Path,
    config: &StorageConfig,
    create_dir: bool,
    vault: V,
    workdirs: &[PathBuf],
) -> Result<(PathBuf, VaultAnchor<V>), StorageError> {
    check_session()?;
    let configured = canonical_path(config)?;
    if !path.is_absolute() || path.file_name() != configured.file_name() {
        return Err(StorageError::NonCanonicalDatabase);
    }
    check_audit_location(&configured, workdirs)?;
    if path != configured {
        check_audit_location(path, workdirs)?;
    }
    let directory = configured.parent().ok_or(StorageError::CantOpen)?;
    if create_dir
        && path == configured
        && let Some(parent) = directory.parent()
    {
        fs::create_dir_all(parent)?;
    }
    let resolved = match directory.canonicalize() {
        Ok(resolved) => resolved,
        Err(error)
            if create_dir && path == configured && error.kind() == std::io::ErrorKind::NotFound =>
        {
            let name = directory.file_name().ok_or(StorageError::CantOpen)?;
            directory
                .parent()
                .ok_or(StorageError::CantOpen)?
                .canonicalize()?
                .join(name)
        }
        Err(error) => return Err(error.into()),
    };
    check_audit_location(&resolved.join("audit.db"), workdirs)?;
    let held = ControlDir::open_user_owned(&resolved, create_dir && path == configured)
        .map_err(audit_access_error)?;
    #[cfg(windows)]
    {
        let name = configured
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(StorageError::CantOpen)?;
        for suffix in ["", "-wal", "-shm", "-journal"] {
            held.check_user_owned_file_if_present(&format!("{name}{suffix}"))
                .map_err(audit_access_error)?;
        }
    }

    let canonical = normalized_path(&configured)?;
    if canonical.parent() != Some(resolved.as_path()) || normalized_path(path)? != canonical {
        return Err(StorageError::NonCanonicalDatabase);
    }
    let mut anchor = VaultAnchor::new(vault, &canonical, true)?;
    anchor._directory = Some(held);
    Ok((canonical, anchor))
}

pub fn open(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    workdirs: &[PathBuf],
) -> Result<EventLog, StorageError> {
    let (path, anchor) = canonical_anchor(path, config, true, OsVault, workdirs)?;
    let _binding = lock_binding(&vault_directory()?)?;
    EventLog::open(&path, config, keys, Box::new(anchor))
}

pub(crate) fn lock_with_timeout<T>(
    mutex: &Mutex<T>,
    timeout: Duration,
) -> Result<MutexGuard<'_, T>, StorageError> {
    use std::sync::TryLockError;

    let deadline = Instant::now() + timeout;
    loop {
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => return Err(StorageError::Unavailable),
            Err(TryLockError::WouldBlock) => {}
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(StorageError::Busy);
        }
        std::thread::sleep(remaining.min(Duration::from_millis(1)));
        if Instant::now() >= deadline {
            return Err(StorageError::Busy);
        }
    }
}

/// The audit log with the product state it was opened with. A session gate is built from it
/// and only acts on a log with the same state, so what the gate protects is the log it writes.
pub struct ProductLog {
    log: Arc<Mutex<EventLog>>,
    state: ProductState,
    busy_timeout: Duration,
}

impl ProductLog {
    pub fn log(&self) -> Result<MutexGuard<'_, EventLog>, StorageError> {
        lock_with_timeout(&self.log, self.busy_timeout)
    }

    pub(crate) fn busy_timeout(&self) -> Duration {
        self.busy_timeout
    }

    /// Gives another component access to the same audit connection.
    pub fn shared_log(&self) -> Arc<Mutex<EventLog>> {
        Arc::clone(&self.log)
    }

    pub fn state(&self) -> &ProductState {
        &self.state
    }
}

#[derive(Debug)]
pub enum ProductLogError {
    Storage(StorageError),
    /// A product path that cannot be resolved safely (see `ProductState`).
    State(String),
}

impl From<StorageError> for ProductLogError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

/// Opens the audit log like [`open`] and binds it to the product state: the database as
/// configured with its side files, the vault folder and, when the daemon holds the launcher's
/// instance lock, its folder.
pub fn open_product(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    instance: Option<&InstanceLock>,
    workdirs: &[PathBuf],
) -> Result<ProductLog, ProductLogError> {
    open_product_with(path, config, keys, OsVault, instance, workdirs)
}

fn open_product_with<V: Vault + Send + 'static>(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    vault: V,
    instance: Option<&InstanceLock>,
    workdirs: &[PathBuf],
) -> Result<ProductLog, ProductLogError> {
    open_product_in(
        &vault_directory()?,
        path,
        config,
        keys,
        vault,
        instance,
        workdirs,
    )
}

fn open_product_in<V: Vault + Send + 'static>(
    vault_dir: &Path,
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    vault: V,
    instance: Option<&InstanceLock>,
    workdirs: &[PathBuf],
) -> Result<ProductLog, ProductLogError> {
    let (canonical, anchor) = canonical_anchor(path, config, true, vault, workdirs)?;
    let _binding = lock_binding(vault_dir)?;
    let log = EventLog::open(&canonical, config, keys, Box::new(anchor))?;
    // The configured spelling, so that every link on its way is protected too.
    let state = ProductState::resolve(
        &canonical_path(config)?,
        vault_dir,
        instance.map(InstanceLock::folder),
    )
    .map_err(ProductLogError::State)?;
    Ok(ProductLog {
        log: Arc::new(Mutex::new(log)),
        state,
        busy_timeout: Duration::from_millis(config.busy_timeout_ms().into()),
    })
}

/// Prepares a plan against the OS vault. Review its summary before confirming.
pub fn plan_reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    workdirs: &[PathBuf],
) -> Result<ReanchorPlan, StorageError> {
    let (path, anchor) = canonical_anchor(path, config, false, OsVault, workdirs)?;
    EventLog::plan_reanchor(&path, config, keys, &anchor)
}

/// Applies the exact plan confirmed by the owner after making a backup.
pub fn reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    confirmed: ReanchorPlan,
    workdirs: &[PathBuf],
) -> Result<(), StorageError> {
    reanchor_with(path, config, keys, confirmed, OsVault, workdirs)
}

fn reanchor_with<V: Vault + Send>(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    confirmed: ReanchorPlan,
    vault: V,
    workdirs: &[PathBuf],
) -> Result<(), StorageError> {
    let (path, anchor) = canonical_anchor(path, config, false, vault, workdirs)?;
    let _binding = lock_binding(&vault_directory()?)?;
    EventLog::reanchor(&path, config, keys, &anchor, confirmed)
}

/// A product log in `root` (an existing, resolved folder) with an in-memory vault, for the
/// tests of the modules that compose the gate.
#[cfg(test)]
pub(crate) fn test_product_log(root: &Path, storage: &str, vault: &TestVault) -> ProductLog {
    tests::product_log(root, storage, vault)
}

#[cfg(test)]
pub(crate) use tests::TestVault;

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use ads_core::vault::VaultError;
    use ads_storage::cipher::{CipherError, KeyId, SecretKey};

    use super::*;

    #[derive(Clone, Default)]
    struct MemoryVault(Arc<Mutex<BTreeMap<String, Secret>>>);

    impl Vault for MemoryVault {
        fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError> {
            self.0.lock().unwrap().insert(
                name.as_str().into_owned(),
                Secret::new(secret.expose().to_vec())?,
            );
            Ok(())
        }

        fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError> {
            self.0
                .lock()
                .unwrap()
                .get(name.as_str().as_ref())
                .map(|secret| Secret::new(secret.expose().to_vec()))
                .transpose()
        }

        fn remove(&self, _: SecretName) -> Result<(), VaultError> {
            panic!("the audit anchor must never be removed")
        }
    }

    struct Keys;

    impl KeyProvider for Keys {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            let id = KeyId::new("audit-key")?;
            let key = self.by_id(&id)?;
            Ok((id, key))
        }

        fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
            if *id != KeyId::new("audit-key")? {
                return Err(CipherError::KeyUnavailable);
            }
            Ok(SecretKey::new(&[17; 32]))
        }
    }

    struct Fixture {
        path: PathBuf,
        directories: Vec<PathBuf>,
    }

    impl Fixture {
        fn new(name: &str, subdirs: &[&str]) -> Self {
            let worktree = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap();
            let root = worktree.join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            assert_eq!(root.canonicalize().unwrap(), root);
            let mut unique = [0_u8; 16];
            getrandom::fill(&mut unique).unwrap();
            let path = root.join(format!("{name}-{:032x}", u128::from_be_bytes(unique)));
            ads_private_dir::create_dir(&path, &ads_private_dir::Identity::current().unwrap())
                .unwrap();
            let directories = subdirs.iter().map(|subdir| path.join(subdir)).collect();
            Self { path, directories }
        }

        fn track_staging(&mut self, report: &ads_adapter_fs::Report) {
            let directories = report
                .staging
                .iter()
                .rev()
                .filter(|entry| {
                    entry.what == ads_adapter_fs::Scaffold::Folder
                        && matches!(
                            entry.status,
                            ads_adapter_fs::Laid::Made | ads_adapter_fs::Laid::Unconfirmed { .. }
                        )
                })
                .filter_map(|entry| Path::new(&entry.path).canonicalize().ok())
                .filter(|path| path != &self.path && path.starts_with(&self.path));
            self.directories.splice(..0, directories);
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if self.path.canonicalize().as_ref().ok() != Some(&self.path) {
                return;
            }
            for dir in self.directories.iter().chain([&self.path]) {
                if dir.canonicalize().as_ref().ok() != Some(dir) {
                    continue;
                }
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Ok(kind) = entry.file_type() {
                            if kind.is_file() {
                                let _ = fs::remove_file(entry.path());
                            } else if kind.is_symlink() {
                                #[cfg(windows)]
                                {
                                    use std::os::windows::fs::FileTypeExt;
                                    if kind.is_symlink_dir() {
                                        let _ = fs::remove_dir(entry.path());
                                        continue;
                                    }
                                }
                                let _ = fs::remove_file(entry.path());
                            }
                        }
                    }
                }
                let _ = fs::remove_dir(dir);
            }
        }
    }

    impl std::ops::Deref for Fixture {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.path
        }
    }

    fn fixture() -> Fixture {
        Fixture::new(
            "audit-anchor",
            &[
                "ads",
                "custom",
                "first",
                "moved",
                "old",
                "private",
                "profile/ads",
                "profile",
                "second",
                "work/private",
                "work",
            ],
        )
    }

    #[test]
    fn a_poisoned_audit_connection_refuses_both_gate_access_and_queue_writes() {
        use crate::api::ApiError;
        use crate::auth::Token;
        use crate::queue::{LoggedQueue, QueueControl, QueueOwner};
        use ads_contracts::{CommonIdV1, RecordTaskQueueV1State};

        let root = Fixture::new("poisoned-audit", &["audit", "vault"]);
        let log = test_product_log(&root, "", &TestVault::default());
        let token = Token::generate().unwrap();
        let bootstrap = token.desktop_bootstrap(0);
        let session = token
            .open_session(bootstrap.split_once(' ').unwrap().1)
            .unwrap();
        let id = CommonIdV1::new("poisoned-log-queue").unwrap();
        let shared = log.shared_log();
        let queue = LoggedQueue::open(
            &session,
            shared.clone(),
            id.clone(),
            QueueOwner {
                owner_id: CommonIdV1::new("owner").unwrap(),
                device_id: CommonIdV1::new("device").unwrap(),
            },
        )
        .unwrap();
        let cause = crate::events::new_id().unwrap();
        queue.start(&session, &id, &cause).unwrap();

        assert!(
            std::thread::spawn(move || {
                let _guard = shared.lock().unwrap();
                panic!("poison the audit connection");
            })
            .join()
            .is_err()
        );

        assert_eq!(log.log().unwrap_err(), StorageError::Unavailable);
        assert_eq!(queue.stop(&session, &id, &cause), Err(ApiError::Storage));
        assert_eq!(
            queue.state(&session, &id),
            Ok(RecordTaskQueueV1State::Stopped)
        );
    }

    #[test]
    fn audit_fixture_removes_its_listed_directories() {
        let sibling = fixture();
        let keep = sibling.join("keep");
        fs::write(&keep, b"keep").unwrap();
        let folder = fixture();
        let path = folder.to_path_buf();
        let database = folder.join("private").join("audit.db");
        drop(
            open_configured(
                &database,
                &configured_for(&database),
                &MemoryVault::default(),
            )
            .unwrap(),
        );
        drop(folder);
        assert!(!path.exists(), "fixture remained at {path:?}");
        assert_eq!(fs::read(keep).unwrap(), b"keep");
    }

    fn open_with(
        path: &Path,
        vault: &MemoryVault,
        canonical: bool,
    ) -> Result<EventLog, StorageError> {
        let anchor = VaultAnchor::new(vault.clone(), &normalized_path(path)?, canonical)?;
        EventLog::open(
            path,
            &StorageConfig::defaults(),
            Box::new(Keys),
            Box::new(anchor),
        )
    }

    fn event(seq: u64) -> ads_contracts::EventEnvelopeV1 {
        serde_json::from_value(serde_json::json!({
            "id": format!("01J{seq:023}"),
            "type": "urn:ads:event:task.noted:1",
            "ts": "2026-11-19T12:00:00Z",
            "stream": "task/t1",
            "seq": seq,
            "actor": {"kind": "device", "id": "test-device"},
            "cause": null,
            "data_class": "private",
            "data": {}
        }))
        .unwrap()
    }

    #[test]
    fn file_execution_uses_the_anchored_gate() {
        use std::num::NonZeroU64;

        use crate::api::Launcher;
        use crate::tool_launcher::ToolLauncher;
        use ads_core::gate::{
            Actor, ActorKind, Command, DataClass, Estimate, EventMeta, GateJournal, GatePolicy,
            HostFacts, Id, JournalError, OperationRequest, PlanRequest, State, TargetRequest,
            Timestamp, Ulid,
        };
        use ads_core::tool_launch::{DiskProbe, HostRules};
        use ads_storage::cipher::{CipherError, KeyId, SecretKey};

        struct FixtureKey;
        impl KeyProvider for FixtureKey {
            fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
                Ok((KeyId::new("fixture")?, SecretKey::new(&[9; 32])))
            }

            fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
                let (expected, key) = self.current()?;
                if *id != expected {
                    return Err(CipherError::KeyUnavailable);
                }
                Ok(key)
            }
        }

        let mut root = Fixture::new(
            "file-gate",
            &[
                "audit",
                "vault",
                "fence/work/pack/project",
                "fence/work/pack",
                "fence/work",
                "fence",
            ],
        );
        let fence = root.join("fence");
        let work = fence.join("work");
        fs::create_dir(&fence).unwrap();
        fs::create_dir(&work).unwrap();
        let file = work.join("example.txt");
        fs::write(&file, "example").unwrap();

        let fence_path = fs::canonicalize(&fence).unwrap();
        let fence_path = fence_path.to_str().unwrap();
        let fence_path = fence_path.strip_prefix(r"\\?\").unwrap_or(fence_path);
        let child = |name| format!("{fence_path}{}{name}", std::path::MAIN_SEPARATOR);
        let home = child("fake-home");
        let work = child("work");
        let target = format!("{work}{}example.txt", std::path::MAIN_SEPARATOR);
        let policy = GatePolicy::defaults(&HostFacts { user_home: &home }).unwrap();
        let program = ads_adapter_claude::HARNESS_ID;
        let launcher = ToolLauncher::from_toml_str(
            &format!("[workdirs]\nworkspace = '{work}'\nother = '{fence_path}'\n[tools.{program}]\nprogram = '{target}'"),
            HostRules::host(),
            std::iter::empty::<(&str, &str)>(),
            DiskProbe,
            DiskProbe,
        ).unwrap();
        let opened = launcher
            .prepare(
                &serde_json::from_value(serde_json::json!({
                    "session_id": "example",
                    "harness": program,
                    "workdir_alias": "workspace",
                    "cols": 80,
                    "rows": 24,
                    "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}
                }))
                .unwrap(),
            )
            .unwrap();
        let real_root = root.to_path_buf();
        let database = real_root.join("audit").join("events.db");
        let mut log = open_product_in(
            &real_root.join("vault"),
            &database,
            &StorageConfig::from_toml_str(&format!(
                "audit_database_path = '{}'\nbusy_timeout_ms = 50",
                database.display()
            ))
            .unwrap(),
            Box::new(FixtureKey),
            MemoryVault::default(),
            None,
            &[],
        )
        .unwrap();
        let product = &log;
        let mut gate = launcher
            .session_gate(&opened, policy.clone(), product, &fence)
            .unwrap();
        let open_in = |cwd| {
            let mut params = opened.clone();
            params.cwd = Some(cwd);
            params
        };
        assert!(
            launcher
                .session_gate(
                    &open_in(child("unconfigured").try_into().unwrap()),
                    policy.clone(),
                    product,
                    &fence
                )
                .is_err()
        );
        let mut other = launcher
            .session_gate(
                &open_in(fence_path.to_owned().try_into().unwrap()),
                policy,
                product,
                &fence,
            )
            .unwrap();
        let request = PlanRequest {
            operation: OperationRequest::Delete,
            work_root: child("forged"),
            targets: vec![TargetRequest {
                path: target,
                repo_root: false,
            }],
            estimate: Some(Estimate { items: 1, bytes: 7 }),
        };
        let meta = |seq| EventMeta {
            seq: NonZeroU64::new(seq).unwrap(),
            id: Ulid::new(format!("01J{seq:023}")).unwrap(),
            ts: Timestamp::new("2030-01-02T03:04:05Z").unwrap(),
            actor: Actor {
                kind: ActorKind::Daemon,
                id: Id::new("daemon").unwrap(),
            },
            cause: None,
            data_class: DataClass::Internal,
        };

        let id = Id::new("example").unwrap();
        gate.plan(&mut log, id.clone(), request, meta(1)).unwrap();
        assert_eq!(
            log.log()
                .unwrap()
                .load(&id)
                .unwrap()
                .plan
                .fields()
                .work_root
                .as_str(),
            work
        );
        assert_eq!(other.inspect(&mut log, &id), Err(JournalError::Missing));
        assert_eq!(
            other.apply(
                &mut log,
                &id,
                Command::Fail {
                    error_code: ads_adapter_fs::Reason::Io.error_code()
                },
                meta(2)
            ),
            Err(JournalError::Missing),
        );
        assert!(matches!(
            other.execute(&mut log, &id, meta(2), meta(3)),
            Err(JournalError::Missing)
        ));
        assert!(file.is_file());
        assert_eq!(log.log().unwrap().verify_chain(), Ok(1));
        use crate::auth::Token;
        use crate::queue::{LoggedQueue, QueueControl, QueueOwner};
        use ads_contracts::CommonIdV1;

        let token = Token::generate().unwrap();
        let bootstrap = token.desktop_bootstrap(0);
        let session = token
            .open_session(bootstrap.split_once(' ').unwrap().1)
            .unwrap();
        let queue_id = CommonIdV1::new("file-gate-queue").unwrap();
        let log_handle = log.shared_log();
        let queue = LoggedQueue::open(
            &session,
            Arc::clone(&log_handle),
            queue_id.clone(),
            QueueOwner {
                owner_id: CommonIdV1::new("owner").unwrap(),
                device_id: CommonIdV1::new("device").unwrap(),
            },
        )
        .unwrap();
        let cause = crate::events::new_id().unwrap();
        queue.start(&session, &queue_id, &cause).unwrap();

        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel();
        gate.before_execute = Some(Box::new(move || {
            entered.send(()).unwrap();
            let _ = released.recv_timeout(Duration::from_secs(10));
        }));
        let start = meta(2);
        let mut finish = meta(3);
        finish.ts = Timestamp::new("2030-01-02T03:04:04.999999Z").unwrap();
        let result = std::thread::scope(|scope| {
            let executing = scope.spawn(|| gate.execute(&mut log, &id, start.clone(), finish));
            waiting.recv_timeout(Duration::from_secs(10)).unwrap();
            let (done, stopped) = std::sync::mpsc::channel();
            let (queue, session, queue_id, cause) = (&queue, &session, &queue_id, &cause);
            scope.spawn(move || done.send(queue.stop(session, queue_id, cause)).unwrap());
            let outcome = stopped.recv_timeout(Duration::from_secs(2));
            let _ = release.send(());
            assert_eq!(outcome.unwrap(), Err(crate::api::ApiError::StorageBusy));
            assert_eq!(
                queue.state(session, queue_id),
                Ok(ads_contracts::RecordTaskQueueV1State::Stopped)
            );
            executing.join().unwrap().unwrap()
        });
        root.track_staging(match &result.operation {
            Ok(report) => report,
            Err(failure) => &failure.report,
        });
        assert!(result.operation.is_ok(), "{:?}", result.operation);
        assert!(!file.exists());
        assert_eq!(result.recorded.unwrap().state, State::Executed);
        assert_eq!(gate.inspect(&mut log, &id).unwrap().state, State::Executed);
        assert_eq!(
            log.log().unwrap().load(&id).unwrap().events[2].ts(),
            &start.ts
        );

        assert_eq!(log.log().unwrap().verify_chain(), Ok(5));
        assert!(matches!(
            gate.execute(&mut log, &id, meta(4), meta(5)),
            Err(JournalError::Transition)
        ));

        let project = fence.join("work/pack/project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".git"), "gitdir: elsewhere").unwrap();
        let denied_id = Id::new("repository").unwrap();
        let pack = format!("{work}{}pack", std::path::MAIN_SEPARATOR);
        let request = PlanRequest {
            operation: OperationRequest::DeleteRecursive,
            work_root: child("forged"),
            targets: vec![TargetRequest {
                path: pack.clone(),
                repo_root: false,
            }],
            estimate: None,
        };
        let mut denied_meta = meta(6);
        denied_meta.seq = NonZeroU64::MIN;
        assert_eq!(
            gate.plan(&mut log, denied_id.clone(), request, denied_meta)
                .unwrap()
                .state,
            State::Denied
        );
        let history = log.log().unwrap().load(&denied_id).unwrap();
        let repo = format!("{pack}{}project", std::path::MAIN_SEPARATOR);
        assert!(
            history
                .plan
                .fields()
                .targets
                .iter()
                .any(|target| target.repo_root
                    && target.pattern.is_none()
                    && target.path.as_str() == repo)
        );
        assert_eq!(
            gate.inspect(&mut log, &denied_id).unwrap().state,
            State::Denied
        );
        assert!(matches!(
            gate.execute(&mut log, &denied_id, meta(7), meta(8)),
            Err(JournalError::Transition)
        ));
        assert!(project.join(".git").is_file());

        assert!(!queue.stop(&session, &queue_id, &cause).unwrap());
        let queue_events = log.log().unwrap().read_queue(&queue_id).unwrap();
        assert_eq!(queue_events.len(), 3);
        assert_eq!(log.log().unwrap().verify_chain(), Ok(7));
        assert_eq!(gate.inspect(&mut log, &id).unwrap().state, State::Executed);

        drop(other);
        drop(gate);
        drop(queue);
        drop(log_handle);
        drop(log);
        let path = root.to_path_buf();
        drop(root);
        assert!(!path.exists(), "fixture remained at {path:?}");
    }

    /// Creates a link at `link` to the folder `target`: a junction on Windows.
    fn link_folder(link: &Path, target: &Path) {
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .unwrap();
            assert!(made.status.success(), "{made:?}");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    /// Removes a link made by [`link_folder`], never what it points to.
    fn unlink_folder(link: &Path) {
        #[cfg(windows)]
        fs::remove_dir(link).unwrap();
        #[cfg(unix)]
        fs::remove_file(link).unwrap();
    }

    /// A session whose work root holds the product state (the audit database reached through
    /// two links, the vault folder and the launcher control folder) never gets a plan that
    /// removes or moves any of it, and the executor refuses such a permit on its own.
    #[test]
    fn session_actions_never_target_the_product_state() {
        use std::num::NonZeroU64;

        use crate::api::Launcher;
        use crate::gate::{PlanActionError, ProductState};
        use crate::startup::InstanceLock;
        use crate::tool_launcher::ToolLauncher;
        use ads_adapter_fs::{Executor, Reason};
        use ads_core::gate::{
            AbsPath, ActionPlan, Actor, ActorKind, DataClass, Estimate, EventMeta, GateJournal,
            GatePolicy, HostFacts, Id, JournalError, JournalGate, OperationRequest, PlanRequest,
            State, TargetRequest, Timestamp, Ulid,
        };
        use ads_core::tool_launch::{DiskProbe, HostRules};

        let fixture = Fixture::new(
            "product-gate",
            &[
                "fence/work/other",
                "fence/work/data",
                "fence/work/vault",
                "fence/work/control",
                "fence/work",
                "fence",
                "gone",
            ],
        );
        fs::create_dir_all(fixture.join("fence").join("work")).unwrap();
        let resolved = fixture.to_path_buf();
        let resolved = resolved.to_str().unwrap();
        let root = PathBuf::from(resolved.strip_prefix(r"\\?\").unwrap_or(resolved));
        let fence = root.join("fence");
        let work_dir = fence.join("work");
        let work = work_dir.to_str().unwrap().to_owned();
        let sep = std::path::MAIN_SEPARATOR;
        let at = |rest: &str| format!("{work}{sep}{rest}");

        // The configured path reaches the database through two links: `linked-data` (outside
        // the work root) to `bridge` (inside it) to `data`.
        let data = work_dir.join("data");
        ads_private_dir::create_dir(&data, &ads_private_dir::Identity::current().unwrap()).unwrap();
        link_folder(&work_dir.join("bridge"), &data);
        link_folder(&root.join("linked-data"), &work_dir.join("bridge"));
        let configured = root.join("linked-data").join("audit.db");
        let database = data.join("audit.db");
        let vault_dir = work_dir.join("vault");
        fs::create_dir(work_dir.join("control")).unwrap();
        let instance =
            InstanceLock::acquire(&work_dir.join("control").join("daemon.lock")).unwrap();
        fs::write(work_dir.join("control").join("state.json"), "{}").unwrap();
        let note = work_dir.join("note.txt");
        fs::write(&note, "note").unwrap();

        let config = configured_for(&configured);
        let vault = MemoryVault::default();
        let open = |instance| {
            open_product_in(
                &vault_dir,
                &configured,
                &config,
                Box::new(Keys),
                vault.clone(),
                instance,
                &[],
            )
        };
        let mut log = open(Some(&instance)).unwrap();
        assert!(database.is_file());
        // SQLite runs in WAL mode: a side file that does not exist yet is accepted.
        assert!(!data.join("audit.db-journal").exists());

        let program = ads_adapter_claude::HARNESS_ID;
        let launcher = ToolLauncher::from_toml_str(
            &format!(
                "[workdirs]\nworkspace = '{work}'\n[tools.{program}]\nprogram = '{}'",
                note.display()
            ),
            HostRules::host(),
            std::iter::empty::<(&str, &str)>(),
            DiskProbe,
            DiskProbe,
        )
        .unwrap();
        let opened = launcher
            .prepare(
                &serde_json::from_value(serde_json::json!({
                    "session_id": "example",
                    "harness": program,
                    "workdir_alias": "workspace",
                    "cols": 80,
                    "rows": 24,
                    "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}
                }))
                .unwrap(),
            )
            .unwrap();
        let home = format!("{}{sep}fake-home", fence.display());
        let policy_with = |overrides: &str| {
            GatePolicy::from_toml_str(overrides, &HostFacts { user_home: &home }).unwrap()
        };
        let meta = |event: u64, seq: u64| EventMeta {
            seq: NonZeroU64::new(seq).unwrap(),
            id: Ulid::new(format!("01J{event:023}")).unwrap(),
            ts: Timestamp::new("2030-01-02T03:04:05Z").unwrap(),
            actor: Actor {
                kind: ActorKind::Daemon,
                id: Id::new("daemon").unwrap(),
            },
            cause: None,
            data_class: DataClass::Internal,
        };
        let request = |operation, target: &str, bytes| PlanRequest {
            operation,
            work_root: work.clone(),
            targets: vec![TargetRequest {
                path: target.to_owned(),
                repo_root: false,
            }],
            estimate: Some(Estimate { items: 1, bytes }),
        };
        let moved_to = |folder: &str| OperationRequest::Move {
            destination: at(folder),
        };
        let mut planned = 0;
        // A permit issued under a policy that does not know the product state: the executor
        // of the session judges it again and refuses before touching anything.
        let work_root = AbsPath::parse(&work).unwrap();
        let blind = policy_with("").with_work_roots(vec![work_root.clone()]);
        let plan =
            ActionPlan::normalize(&request(OperationRequest::Delete, &at("data/audit.db"), 1))
                .unwrap();
        let observed = Executor::new(&fence, blind.clone())
            .unwrap()
            .observe_plan(&plan)
            .unwrap();
        planned += 1;
        let id = Id::new(format!("product-{planned}")).unwrap();
        let snapshot = JournalGate::new(&blind, &mut *log.log().unwrap())
            .for_work_root(&work_root)
            .plan(id.clone(), &observed, meta(planned, 1))
            .unwrap();
        assert_eq!(snapshot.state, State::Planned);
        planned += 1;
        let started = JournalGate::new(&blind, &mut *log.log().unwrap())
            .for_work_root(&work_root)
            .authorize(&id, meta(planned, 2))
            .unwrap();
        let mut gate = launcher
            .session_gate(&opened, policy_with(""), &log, &fence)
            .unwrap();
        let failure = gate.executor_mut().execute_recorded(started).err().unwrap();
        assert_eq!(failure.reason, Reason::PolicyDenies, "{}", failure.detail);
        assert!(
            failure.detail.contains("protected.product_data"),
            "{}",
            failure.detail
        );

        for overrides in ["", "quarantine_enabled = false"] {
            let gate = launcher
                .session_gate(&opened, policy_with(overrides), &log, &fence)
                .unwrap();
            for (operation, target) in [
                (OperationRequest::Delete, at("data/audit.db")),
                (OperationRequest::Delete, at("data/audit.db-wal")),
                (OperationRequest::Delete, at("Data/AUDIT.DB")),
                (OperationRequest::Delete, at("data/*.db")),
                (OperationRequest::DeleteRecursive, at("data")),
                (moved_to("moved"), at("data/audit.db")),
                // The link on the way from the configured path to the database.
                (OperationRequest::Delete, at("bridge")),
                (OperationRequest::Delete, at("vault/audit-binding.lock")),
                (OperationRequest::Delete, at("control/state.json")),
                (OperationRequest::DeleteRecursive, at("control")),
                (moved_to("control"), at("note.txt")),
            ] {
                planned += 1;
                let id = Id::new(format!("product-{planned}")).unwrap();
                let snapshot = gate
                    .plan(
                        &mut log,
                        id.clone(),
                        request(operation, &target, 1),
                        meta(planned, 1),
                    )
                    .unwrap();
                assert_eq!(snapshot.state, State::Denied, "{overrides:?} {target}");
                let plan = log.log().unwrap().load(&id).unwrap().plan;
                let reasons = &plan.fields().verdict.reasons;
                assert!(
                    reasons
                        .iter()
                        .any(|reason| reason.rule.as_str().ends_with(".product_data")),
                    "{target}: {reasons:?}"
                );
            }
            // What sits beside the product state is still an ordinary target.
            planned += 1;
            let id = Id::new(format!("product-{planned}")).unwrap();
            let beside = request(OperationRequest::Delete, &at("note.txt"), 4);
            let snapshot = gate.plan(&mut log, id, beside, meta(planned, 1)).unwrap();
            assert_ne!(snapshot.state, State::Denied, "{overrides:?}");
        }

        // A gate acts only on the log it was built for.
        let other_db = work_dir.join("other").join("audit.db");
        let mut other = open_product_in(
            &vault_dir,
            &other_db,
            &configured_for(&other_db),
            Box::new(Keys),
            MemoryVault::default(),
            Some(&instance),
            &[],
        )
        .unwrap();
        planned += 1;
        let id = Id::new(format!("product-{planned}")).unwrap();
        assert!(matches!(
            gate.plan(
                &mut other,
                id.clone(),
                request(OperationRequest::Delete, &at("other/audit.db"), 1),
                meta(planned, 1)
            ),
            Err(PlanActionError::Journal(JournalError::PolicyChanged))
        ));
        assert_eq!(
            gate.inspect(&mut other, &id),
            Err(JournalError::PolicyChanged)
        );
        drop(other);

        // Nothing moved: the database and the link are in place and open again.
        assert!(database.is_file());
        assert!(fs::symlink_metadata(work_dir.join("bridge")).is_ok());
        drop(log);
        drop(open(Some(&instance)).unwrap());

        // A link whose target is gone is refused, not taken for a file to come.
        let gone = root.join("gone");
        fs::create_dir(&gone).unwrap();
        link_folder(&root.join("dangling"), &gone);
        fs::remove_dir(&gone).unwrap();
        assert!(
            ProductState::resolve(&root.join("dangling").join("audit.db"), &vault_dir, None)
                .is_err()
        );
        for link in [
            root.join("dangling"),
            root.join("linked-data"),
            work_dir.join("bridge"),
        ] {
            unlink_folder(&link);
        }
    }

    #[test]
    fn binding_lock_refuses_a_second_holder_until_release() {
        let directory = fixture();
        let first = lock_binding(&directory).unwrap();
        assert_eq!(lock_binding(&directory).unwrap_err(), StorageError::Busy);
        drop(first);
        assert!(lock_binding(&directory).is_ok());
    }

    /// The part this test binary plays in `pin_is_serialized_across_processes_and_database_paths`.
    #[cfg(windows)]
    const PIN_ROLE: &str = "ADS_TEST_PIN_ROLE";
    #[cfg(windows)]
    const PIN_ROOT: &str = "ADS_TEST_PIN_ROOT";

    /// Holds the pin of a fresh database until the other process says to go on.
    #[cfg(windows)]
    struct PausedVault {
        inner: MemoryVault,
        root: PathBuf,
    }

    #[cfg(windows)]
    impl Vault for PausedVault {
        fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError> {
            if matches!(name, SecretName::AuditDatabase) {
                fs::write(self.root.join("pinning"), b"").unwrap();
                wait_for(&self.root.join("release"));
            }
            self.inner.store(name, secret)
        }

        fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError> {
            self.inner.read(name)
        }

        fn remove(&self, name: SecretName) -> Result<(), VaultError> {
            self.inner.remove(name)
        }
    }

    #[cfg(windows)]
    fn wait_for(path: &Path) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !path.exists() {
            assert!(std::time::Instant::now() < deadline, "{path:?} never came");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Ends a process of the test if an assertion fails while it waits.
    #[cfg(windows)]
    struct Reaped(std::process::Child);

    #[cfg(windows)]
    impl Drop for Reaped {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[cfg(windows)]
    fn vault_contents(vault: &MemoryVault) -> BTreeMap<String, Vec<u8>> {
        vault
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|(name, secret)| (name.clone(), secret.expose().to_vec()))
            .collect()
    }

    /// Runs this test binary as one process of the scenario, with its own data folder.
    #[cfg(windows)]
    fn spawn_role(role: &str, root: &Path) -> Reaped {
        Reaped(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "audit::tests::pin_role", "--test-threads=1"])
                .env(PIN_ROLE, role)
                .env(PIN_ROOT, root)
                .env("LOCALAPPDATA", root.join("profile"))
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        )
    }

    /// A process of `pin_is_serialized_across_processes_and_database_paths`; idle otherwise.
    #[cfg(windows)]
    #[test]
    fn pin_role() {
        let Some(root) = std::env::var_os(PIN_ROOT).map(PathBuf::from) else {
            return;
        };
        match std::env::var(PIN_ROLE).unwrap().as_str() {
            "pin" => pin_first(&root),
            "contend" => contend_second(&root),
            role => panic!("unknown role {role}"),
        }
    }

    #[cfg(windows)]
    fn pin_first(root: &Path) {
        let path = root.join("first").join("audit.db");
        let vault = PausedVault {
            inner: MemoryVault::default(),
            root: root.to_path_buf(),
        };
        let log = open_product_with(
            &path,
            &configured_for(&path),
            Box::new(Keys),
            vault,
            None,
            &[],
        );
        assert!(log.is_ok(), "the first process pins its database");
        drop(log);
        fs::write(root.join("pinned"), b"").unwrap();
    }

    #[cfg(windows)]
    fn contend_second(root: &Path) {
        // The owner confirms a move of another database before the first process pins.
        let old = root.join("old").join("audit.db");
        let moved = root.join("moved").join("audit.db");
        let moved_config = configured_for(&moved);
        let moving = MemoryVault::default();
        drop(open_configured(&old, &configured_for(&old), &moving).unwrap());
        fs::create_dir(moved.parent().unwrap()).unwrap();
        fs::copy(&old, &moved).unwrap();
        let (path, anchor) =
            canonical_anchor(&moved, &moved_config, false, moving.clone(), &[]).unwrap();
        let plan = || EventLog::plan_reanchor(&path, &moved_config, Box::new(Keys), &anchor);
        let (refused, confirmed) = (plan().unwrap(), plan().unwrap());
        assert_eq!(refused, confirmed);
        drop(anchor);
        let before_move = vault_contents(&moving);
        fs::write(root.join("planned"), b"").unwrap();
        wait_for(&root.join("pinning"));

        // Another database path is refused at once while the first process pins.
        let second = root.join("second").join("audit.db");
        let opening = MemoryVault::default();
        let open_second = || {
            open_product_with(
                &second,
                &configured_for(&second),
                Box::new(Keys),
                opening.clone(),
                None,
                &[],
            )
        };
        assert!(matches!(
            open_second(),
            Err(ProductLogError::Storage(StorageError::Busy))
        ));
        assert!(vault_contents(&opening).is_empty());
        assert!(!second.exists());
        let move_with = |plan| {
            reanchor_with(
                &moved,
                &moved_config,
                Box::new(Keys),
                plan,
                moving.clone(),
                &[],
            )
        };
        assert_eq!(move_with(refused), Err(StorageError::Busy));
        assert_eq!(vault_contents(&moving), before_move);

        fs::write(root.join("release"), b"").unwrap();
        wait_for(&root.join("pinned"));
        drop(open_second().unwrap());
        assert!(vault_contents(&opening).contains_key("audit-database"));
        move_with(confirmed).unwrap();
        assert_ne!(vault_contents(&moving), before_move);
    }

    #[cfg(windows)]
    #[test]
    fn pin_is_serialized_across_processes_and_database_paths() {
        let root = fixture();
        fs::create_dir(root.join("profile")).unwrap();
        let mut second = spawn_role("contend", &root);
        wait_for(&root.join("planned"));
        let mut first = spawn_role("pin", &root);
        assert!(second.0.wait().unwrap().success());
        assert!(first.0.wait().unwrap().success());
    }

    #[test]
    fn product_uses_only_the_canonical_database_path() {
        let root = fixture();
        let canonical = root.join("ads").join("audit.db");
        let other = root.join("audit.db");
        let config = configured_for(&canonical);
        let vault = MemoryVault::default();
        assert_eq!(canonical_path(&config), Ok(canonical.clone()));
        drop(open_configured(&canonical, &config, &vault).unwrap());
        assert_eq!(
            open_configured(&other, &config, &vault).unwrap_err(),
            StorageError::NonCanonicalDatabase
        );
        assert!(!other.exists());
    }

    #[test]
    fn independent_databases_keep_separate_anchors() {
        let root = fixture();
        let first = root.join("first.db");
        let second = root.join("second.db");
        let vault = MemoryVault::default();
        std::thread::scope(|scope| {
            let one = scope.spawn(|| {
                open_with(&first, &vault, false)
                    .unwrap()
                    .append(&event(1))
                    .unwrap()
            });
            open_with(&second, &vault, false)
                .unwrap()
                .append(&event(1))
                .unwrap();
            one.join().unwrap();
        });
        assert_eq!(vault.0.lock().unwrap().len(), 2);
        open_with(&first, &vault, false)
            .unwrap()
            .append(&event(2))
            .unwrap();
        assert_eq!(
            open_with(&second, &vault, false)
                .unwrap()
                .head(&event(1).stream)
                .unwrap(),
            1
        );
    }

    #[test]
    fn copied_database_identity_is_rejected_at_a_different_location() {
        let root = fixture();
        let original = root.join("original.db");
        let copy = root.join("copy.db");
        let vault = MemoryVault::default();
        drop(open_with(&original, &vault, false).unwrap());
        fs::copy(&original, &copy).unwrap();
        assert_eq!(
            open_with(&copy, &vault, false).unwrap_err(),
            StorageError::DuplicateDatabaseIdentity
        );
        open_with(&original, &vault, false).unwrap();
    }

    #[test]
    fn canonical_database_adopts_only_its_matching_legacy_anchor_once() {
        let root = fixture();
        let original = root.join("original.db");
        let unrelated = root.join("unrelated.db");
        let source = MemoryVault::default();
        open_with(&original, &source, false)
            .unwrap()
            .append(&event(1))
            .unwrap();
        drop(open_with(&unrelated, &source, false).unwrap());
        let anchor = VaultAnchor::new(source, &original, false).unwrap();
        let slots = anchor.vault.0.lock().unwrap();
        let commitment = slots
            .values()
            .find_map(|secret| {
                let (location, bytes) = secret.expose().split_at(32);
                (location == anchor.location).then_some(bytes)
            })
            .unwrap();
        let migrated = MemoryVault::default();
        migrated
            .store(
                SecretName::AuditAnchor,
                &Secret::new(commitment.to_vec()).unwrap(),
            )
            .unwrap();
        assert_eq!(
            open_with(&unrelated, &migrated, true).unwrap_err(),
            StorageError::AnchorMismatch
        );
        assert_eq!(migrated.0.lock().unwrap().len(), 1);
        open_with(&original, &migrated, true)
            .unwrap()
            .append(&event(2))
            .unwrap();
        assert_eq!(migrated.0.lock().unwrap().len(), 3);
        migrated
            .store(SecretName::AuditAnchor, &Secret::new(vec![0]).unwrap())
            .unwrap();
        drop(open_with(&original, &migrated, true).unwrap());
        let pinned = VaultAnchor::new(migrated.clone(), &original, true).unwrap();
        assert_eq!(pinned.legacy(), Ok(None));
        assert_eq!(pinned.can_create(), Ok(false));
        fs::copy(&unrelated, &original).unwrap();
        assert_eq!(
            open_with(&original, &migrated, true).unwrap_err(),
            StorageError::AnchorMismatch
        );
    }

    /// An in-memory vault whose stores can be made to fail, now or after a number of them:
    /// a refused store refuses the append that publishes the anchor.
    #[derive(Clone, Default)]
    pub(crate) struct TestVault {
        vault: MemoryVault,
        /// Stores still accepted; `usize::MAX` (the default after `accept_all`) never fails.
        accepted: Arc<std::sync::atomic::AtomicUsize>,
        armed: Arc<std::sync::atomic::AtomicBool>,
        /// Stores still to refuse before the others are accepted again.
        refused: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl TestVault {
        /// Accepts `stores` more stores, then refuses every other one.
        pub(crate) fn fail_after(&self, stores: usize) {
            use std::sync::atomic::Ordering;
            self.accepted.store(stores, Ordering::SeqCst);
            self.armed.store(true, Ordering::SeqCst);
        }

        /// Refuses the next `stores` stores and accepts the ones after them.
        pub(crate) fn refuse_next(&self, stores: usize) {
            self.refused
                .store(stores, std::sync::atomic::Ordering::SeqCst);
        }

        pub(crate) fn accept_all(&self) {
            self.armed.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl Vault for TestVault {
        fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError> {
            use std::sync::atomic::Ordering;
            if self
                .refused
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(VaultError::Unavailable);
            }
            if self.armed.load(Ordering::SeqCst)
                && self
                    .accepted
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_err()
            {
                return Err(VaultError::Unavailable);
            }
            self.vault.store(name, secret)
        }

        fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError> {
            self.vault.read(name)
        }

        fn remove(&self, name: SecretName) -> Result<(), VaultError> {
            self.vault.remove(name)
        }
    }

    pub(super) fn product_log(root: &Path, storage: &str, vault: &TestVault) -> ProductLog {
        let database = root.join("audit").join("audit.db");
        let config = StorageConfig::from_toml_str(&format!(
            "audit_database_path = {}\n{storage}",
            serde_json::to_string(&database).unwrap()
        ))
        .unwrap();
        open_product_in(
            &root.join("vault"),
            &database,
            &config,
            Box::new(Keys),
            vault.clone(),
            None,
            &[],
        )
        .unwrap()
    }

    fn configured_for(path: &Path) -> StorageConfig {
        StorageConfig::from_toml_str(&format!(
            "audit_database_path = {}",
            serde_json::to_string(path).unwrap()
        ))
        .unwrap()
    }

    fn open_configured(
        path: &Path,
        config: &StorageConfig,
        vault: &MemoryVault,
    ) -> Result<EventLog, StorageError> {
        let (path, anchor) = canonical_anchor(path, config, true, vault.clone(), &[])?;
        EventLog::open(&path, config, Box::new(Keys), Box::new(anchor))
    }

    fn grant_access_to_others(path: &Path) {
        #[cfg(windows)]
        assert!(
            std::process::Command::new("icacls")
                .arg(path)
                .args(["/grant", "*S-1-1-0:(OI)(CI)RX"])
                .output()
                .unwrap()
                .status
                .success()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn audit_folder_new_has_private_permissions() {
        let root = fixture();
        grant_access_to_others(&root);
        let directory = root.join("private");
        let path = directory.join("audit.db");
        let log = open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap();
        assert!(path.exists());
        assert!(ControlDir::open_user_owned(&directory, false).is_ok());
        #[cfg(windows)]
        {
            use ads_private_dir::{Identity, access_list_text, canonical_access_list};
            let user = Identity::current().unwrap().user;
            let expected = canonical_access_list(&format!("D:P(A;OICI;FA;;;{user})")).unwrap();
            assert_eq!(access_list_text(&directory).unwrap(), expected);
        }
        drop(log);
    }

    #[cfg(windows)]
    #[test]
    fn audit_folder_unc_is_refused_before_filesystem_access() {
        for path in [
            Path::new(r"\\localhost\ads-audit-regression-missing\audit.db"),
            Path::new(r"\\?\UNC\localhost\ads-audit-regression-missing\audit.db"),
        ] {
            assert_eq!(
                open_configured(path, &configured_for(path), &MemoryVault::default()).unwrap_err(),
                StorageError::AuditDatabaseOnNetwork
            );
        }
    }

    #[test]
    fn session_that_gives_new_files_another_owner_is_refused_before_creating_them() {
        let root = fixture();
        let path = root.join("private").join("audit.db");
        let user = Identity::current().unwrap().user;
        SESSION.with(|session| {
            *session.borrow_mut() = Some(Identity {
                user: user.clone(),
                default_owner: "S-1-5-32-544".into(),
            });
        });
        let refused = open_configured(&path, &configured_for(&path), &MemoryVault::default());
        SESSION.with(|session| session.borrow_mut().take());
        assert_eq!(refused.unwrap_err(), StorageError::ElevatedSession);
        assert!(!path.exists());
        assert!(
            StorageError::ElevatedSession
                .to_string()
                .contains("non-elevated")
        );
        drop(open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap());
    }

    #[test]
    fn audit_folder_existing_open_to_others_is_refused() {
        let root = fixture();
        grant_access_to_others(&root);
        let path = root.join("audit.db");
        assert_eq!(
            open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap_err(),
            StorageError::AuditDirectoryNotPrivate
        );
        assert!(!path.exists());
    }

    #[cfg(windows)]
    fn grant_windows_access(path: &Path, ace: &str) {
        let output = std::process::Command::new("icacls")
            .arg(path)
            .args(["/grant", ace])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    #[cfg(windows)]
    #[test]
    fn audit_folder_open_to_administrators_is_refused() {
        let root = fixture();
        grant_windows_access(&root, "*S-1-5-32-544:(OI)(CI)F");
        let path = root.join("audit.db");
        assert_eq!(
            open_configured(&path, &configured_for(&path), &MemoryVault::default()).unwrap_err(),
            StorageError::AuditDirectoryNotPrivate
        );
    }

    #[cfg(windows)]
    #[test]
    fn audit_files_open_to_other_accounts_are_refused() {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let root = fixture();
            let path = root.join("custom.db");
            let exposed = root.join(format!("custom.db{suffix}"));
            fs::write(&exposed, []).unwrap();
            grant_windows_access(&exposed, "*S-1-1-0:R");
            assert_eq!(
                open_configured(&path, &configured_for(&path), &MemoryVault::default())
                    .unwrap_err(),
                StorageError::AuditDirectoryNotPrivate,
                "{suffix}"
            );
            assert_eq!(fs::metadata(&exposed).unwrap().len(), 0, "{suffix}");
        }
    }

    #[test]
    fn custom_audit_path_leaves_the_default_folder_private() {
        let root = fixture();
        grant_access_to_others(&root);
        let vault_dir = root.join("ads");
        let custom = root.join("custom").join("audit.db");
        drop(
            open_product_in(
                &vault_dir,
                &custom,
                &configured_for(&custom),
                Box::new(Keys),
                MemoryVault::default(),
                None,
                &[],
            )
            .unwrap(),
        );

        let default = vault_dir.join("audit.db");
        drop(
            open_configured(&default, &configured_for(&default), &MemoryVault::default()).unwrap(),
        );
    }

    #[test]
    fn audit_folder_resolving_inside_workdir_is_refused() {
        let root = fixture();
        let work = root.join("work");
        let destination = work.join("private");
        fs::create_dir_all(&destination).unwrap();
        let link = root.join("link");
        link_folder(&link, &destination);
        let path = link.join("audit.db");
        let result = canonical_anchor(
            &path,
            &configured_for(&path),
            true,
            MemoryVault::default(),
            &[work],
        );
        unlink_folder(&link);
        assert!(matches!(result, Err(StorageError::AuditDatabaseInWorkdir)));
        assert!(!destination.join("audit.db").exists());
    }

    #[test]
    fn audit_folder_inside_workdir_is_refused_before_creation() {
        let root = fixture();
        let work = root.join("work");
        fs::create_dir(&work).unwrap();
        let path = work.join("private").join("audit.db");
        assert!(matches!(
            canonical_anchor(
                &path,
                &configured_for(&path),
                true,
                MemoryVault::default(),
                &[work]
            ),
            Err(StorageError::AuditDatabaseInWorkdir)
        ));
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn confirmed_move_recognizes_only_the_current_database() {
        let root = fixture();
        let old = root.join("old.db");
        let moved = root.join("moved.db");
        let old_config = configured_for(&old);
        let config = configured_for(&moved);
        let vault = MemoryVault::default();
        drop(open_configured(&old, &old_config, &vault).unwrap());
        fs::copy(&old, &moved).unwrap();
        open_configured(&old, &old_config, &vault)
            .unwrap()
            .append(&event(1))
            .unwrap();
        let (path, anchor) = canonical_anchor(&moved, &config, false, vault.clone(), &[]).unwrap();
        let plan = || EventLog::plan_reanchor(&path, &config, Box::new(Keys), &anchor);
        assert_eq!(plan(), Err(StorageError::AnchorMismatch));
        fs::copy(&old, &moved).unwrap();
        let proposed = plan().unwrap();
        let summary = proposed.summary();
        assert_eq!(summary.previous_id, Some(summary.proposed_id));
        assert_eq!(summary.location_changed, Some(true));
        assert_eq!(
            open_configured(&moved, &config, &vault).unwrap_err(),
            StorageError::DuplicateDatabaseIdentity
        );
        EventLog::reanchor(&path, &config, Box::new(Keys), &anchor, proposed).unwrap();
        assert_eq!(
            open_configured(&moved, &config, &vault)
                .unwrap()
                .head(&event(1).stream),
            Ok(1)
        );
        assert_eq!(
            open_configured(&old, &old_config, &vault).unwrap_err(),
            StorageError::DuplicateDatabaseIdentity
        );
    }

    #[test]
    fn confirmed_empty_replacement_keeps_the_old_database_in_quarantine() {
        let root = fixture();
        let path = root.join("audit.db");
        let quarantine = root.join("quarantined.db");
        let config = configured_for(&path);
        let vault = MemoryVault::default();
        open_configured(&path, &config, &vault)
            .unwrap()
            .append(&event(1))
            .unwrap();
        fs::rename(&path, &quarantine).unwrap();
        assert!(open_configured(&path, &config, &vault).is_err());
        assert!(!path.exists());
        drop(ads_storage::AuditStore::open(&path, &config).unwrap());
        let (canonical, anchor) =
            canonical_anchor(&path, &config, false, vault.clone(), &[]).unwrap();
        let plan = EventLog::plan_reanchor(&canonical, &config, Box::new(Keys), &anchor).unwrap();
        let summary = plan.summary();
        assert_ne!(summary.previous_id, Some(summary.proposed_id));
        assert_eq!(summary.previous_position, Some(1));
        assert_eq!(summary.proposed_position, 0);
        assert_eq!(
            open_configured(&path, &config, &vault).unwrap_err(),
            StorageError::AnchorMismatch
        );
        EventLog::reanchor(&canonical, &config, Box::new(Keys), &anchor, plan).unwrap();
        let log = open_configured(&path, &config, &vault).unwrap();
        assert_eq!(log.head(&event(1).stream), Ok(0));
        drop(log);
        assert!(quarantine.exists());
        fs::copy(&quarantine, &path).unwrap();
        assert_eq!(
            open_configured(&path, &config, &vault).unwrap_err(),
            StorageError::AnchorMismatch
        );
    }

    #[test]
    fn interrupted_legacy_adoption_finishes_binding_on_reopen() {
        let root = fixture();
        let path = root.join("audit.db");
        let config = configured_for(&path);
        let vault = MemoryVault::default();
        let log = open_with(&path, &vault, false).unwrap();
        drop(log);
        let (_, anchor) = canonical_anchor(&path, &config, false, vault.clone(), &[]).unwrap();
        let plan = EventLog::plan_reanchor(&path, &config, Box::new(Keys), &anchor).unwrap();
        let id = plan.summary().proposed_id;
        anchor
            .store_secret(SecretName::AuditAnchor, plan.proposed_commitment().to_vec())
            .unwrap();
        assert_eq!(anchor.pinned_id(), Ok(None));
        drop(open_configured(&path, &config, &vault).unwrap());
        assert_eq!(anchor.pinned_id(), Ok(Some(id)));
        assert_eq!(anchor.legacy(), Ok(None));
    }

    #[test]
    fn scoped_record_validates_commitment_encoding() {
        let root = fixture();
        let store =
            VaultAnchor::new(MemoryVault::default(), &root.join("audit.db"), false).unwrap();
        let id = [17; 32];
        assert_eq!(store.read(id), Ok(None));
        let mut commitment = [17; size_of::<Commitment>()];
        commitment[0] = COMMITMENT_VERSION;
        store.store(id, &commitment).unwrap();
        assert_eq!(store.read(id), Ok(Some(commitment)));
        store
            .store_secret(SecretName::AuditAnchorFor(id), vec![1])
            .unwrap();
        assert_eq!(store.read(id), Err(StorageError::AnchorMismatch));
        commitment[0] += 1;
        store.store(id, &commitment).unwrap();
        assert_eq!(store.read(id), Err(StorageError::UnsupportedSchema));
    }
}
