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
    pub(crate) static SESSION: std::cell::RefCell<Option<Identity>> = const { std::cell::RefCell::new(None) };
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
            // A folder out of reach cannot hold the database, which has just resolved.
            let roots = crate::gate::unreachable_workdir_spellings(workdir)
                .map_err(|_| StorageError::Configuration)?;
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
    _local_runtime: Option<File>,
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
    finish_product(vault_dir, canonical, config, keys, anchor, instance)
}

fn finish_product<V: Vault + Send + 'static>(
    vault_dir: &Path,
    canonical: PathBuf,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    anchor: VaultAnchor<V>,
    instance: Option<&InstanceLock>,
) -> Result<ProductLog, ProductLogError> {
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
        _local_runtime: None,
    })
}

pub(crate) fn open_local_product(
    config: &StorageConfig,
    vault: impl Vault + Send + Sync + 'static,
    instance: Option<&InstanceLock>,
    workdirs: &[PathBuf],
) -> Result<(ProductLog, crate::product::LocalIdentities), crate::product::StartupError> {
    open_local_product_in(
        &vault_directory()?,
        config,
        Arc::new(vault),
        instance,
        workdirs,
    )
}

fn open_local_product_in<V: Vault + Send + Sync + 'static>(
    directory: &Path,
    config: &StorageConfig,
    vault: Arc<V>,
    instance: Option<&InstanceLock>,
    workdirs: &[PathBuf],
) -> Result<(ProductLog, crate::product::LocalIdentities), crate::product::StartupError> {
    use crate::product::{LocalIdentities, StartupError, VaultKeys, first_use};

    check_session()?;
    check_audit_location(&directory.join("audit.db"), workdirs)?;
    let _binding = lock_binding(directory)?;
    let path = canonical_path(config)?;
    let (canonical, anchor) = canonical_anchor(&path, config, true, Arc::clone(&vault), workdirs)?;
    let first_use = first_use(&canonical, vault.as_ref())?;
    let private = ControlDir::open_user_owned(directory, false).map_err(audit_access_error)?;
    let runtime = private
        .lock_file("local-runtime.lock")
        .map_err(audit_access_error)?;
    runtime.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => StorageError::Busy,
        TryLockError::Error(error) => error.into(),
    })?;
    let ids = LocalIdentities::load(&private, first_use).map_err(StartupError::Identity)?;
    let keys = VaultKeys::load(vault.as_ref(), first_use)?;
    let mut log = finish_product(
        directory,
        canonical,
        config,
        Box::new(keys),
        anchor,
        instance,
    )?;
    log._local_runtime = Some(runtime);
    Ok((log, ids))
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
#[path = "audit_tests.rs"]
mod tests;
