//! Composition of the persistent audit log with the product vault.

use std::{
    fs::{self, File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
};

use ads_adapter_vault::OsVault;
use ads_core::vault::{Secret, SecretName, Vault};
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

fn lock_binding(directory: &Path) -> Result<File, StorageError> {
    // SQLite serializes a single file. The vault pin spans all configured paths.
    fs::create_dir_all(directory)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("audit-binding.lock"))?;
    lock.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => StorageError::Busy,
        TryLockError::Error(error) => error.into(),
    })?;
    Ok(lock)
}

fn require_canonical_path(
    path: &Path,
    canonical: &Path,
    create_dir: bool,
) -> Result<PathBuf, StorageError> {
    if !path.is_absolute() || path.file_name() != canonical.file_name() {
        return Err(StorageError::NonCanonicalDatabase);
    }
    if create_dir && path == canonical {
        fs::create_dir_all(canonical.parent().ok_or(StorageError::CantOpen)?)?;
    }
    let canonical = normalized_path(canonical)?;
    if normalized_path(path)? != canonical {
        return Err(StorageError::NonCanonicalDatabase);
    }
    Ok(canonical)
}

fn canonical_anchor<V: Vault>(
    path: &Path,
    config: &StorageConfig,
    create_dir: bool,
    vault: V,
) -> Result<(PathBuf, VaultAnchor<V>), StorageError> {
    let canonical = require_canonical_path(path, &canonical_path(config)?, create_dir)?;
    let anchor = VaultAnchor::new(vault, &canonical, true)?;
    Ok((canonical, anchor))
}

pub fn open(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
) -> Result<EventLog, StorageError> {
    let _binding = lock_binding(&vault_directory()?)?;
    let (path, anchor) = canonical_anchor(path, config, true, OsVault)?;
    EventLog::open(&path, config, keys, Box::new(anchor))
}

/// The audit log with the product state it was opened with. A session gate is built from it
/// and only acts on a log with the same state, so what the gate protects is the log it writes.
pub struct ProductLog {
    log: EventLog,
    state: ProductState,
}

impl ProductLog {
    pub fn log(&mut self) -> &mut EventLog {
        &mut self.log
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
) -> Result<ProductLog, ProductLogError> {
    open_product_in(&vault_directory()?, path, config, keys, OsVault, instance)
}

fn open_product_in<V: Vault + Send + 'static>(
    vault_dir: &Path,
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    vault: V,
    instance: Option<&InstanceLock>,
) -> Result<ProductLog, ProductLogError> {
    let _binding = lock_binding(vault_dir)?;
    let (canonical, anchor) = canonical_anchor(path, config, true, vault)?;
    let log = EventLog::open(&canonical, config, keys, Box::new(anchor))?;
    // The configured spelling, so that every link on its way is protected too.
    let state = ProductState::resolve(
        &canonical_path(config)?,
        vault_dir,
        instance.map(InstanceLock::folder),
    )
    .map_err(ProductLogError::State)?;
    Ok(ProductLog { log, state })
}

/// Prepares a plan against the OS vault. Review its summary before confirming.
pub fn plan_reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
) -> Result<ReanchorPlan, StorageError> {
    let (path, anchor) = canonical_anchor(path, config, false, OsVault)?;
    EventLog::plan_reanchor(&path, config, keys, &anchor)
}

/// Applies the exact plan confirmed by the owner after making a backup.
pub fn reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    confirmed: ReanchorPlan,
) -> Result<(), StorageError> {
    let _binding = lock_binding(&vault_directory()?)?;
    let (path, anchor) = canonical_anchor(path, config, false, OsVault)?;
    EventLog::reanchor(&path, config, keys, &anchor, confirmed)
}

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

    fn fixture() -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::current_exe()
            .unwrap()
            .ancestors()
            .nth(3)
            .unwrap()
            .join("tmp")
            .join(format!("audit-anchor-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
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
        use std::{fs, num::NonZeroU64, path::PathBuf};

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

        let mut unique = [0_u8; 16];
        getrandom::fill(&mut unique).unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.test-tmp")
            .join(format!("file-gate-{:032x}", u128::from_be_bytes(unique)));
        fs::create_dir_all(&root).unwrap();
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
        let real_root = fs::canonicalize(&root).unwrap();
        let database = real_root.join("events.db");
        let mut log = open_product_in(
            &real_root.join("vault"),
            &database,
            &configured_for(&database),
            Box::new(FixtureKey),
            MemoryVault::default(),
            None,
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
        assert_eq!(log.log().verify_chain(), Ok(1));
        let start = meta(2);
        let mut finish = meta(3);
        finish.ts = Timestamp::new("2030-01-02T03:04:04.999999Z").unwrap();
        let result = gate.execute(&mut log, &id, start.clone(), finish).unwrap();
        assert!(result.operation.is_ok(), "{:?}", result.operation);
        assert!(!file.exists());
        assert_eq!(result.recorded.unwrap().state, State::Executed);
        assert_eq!(gate.inspect(&mut log, &id).unwrap().state, State::Executed);
        assert_eq!(log.log().load(&id).unwrap().events[2].ts(), &start.ts);

        assert_eq!(log.log().verify_chain(), Ok(3));
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
        let history = log.log().load(&denied_id).unwrap();
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

        let mut unique = [0_u8; 16];
        getrandom::fill(&mut unique).unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.test-tmp")
            .join(format!("product-gate-{:032x}", u128::from_be_bytes(unique)));
        fs::create_dir_all(root.join("fence").join("work")).unwrap();
        let resolved = fs::canonicalize(&root).unwrap();
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
        fs::create_dir(&data).unwrap();
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
        let snapshot = JournalGate::new(&blind, log.log())
            .for_work_root(&work_root)
            .plan(id.clone(), &observed, meta(planned, 1))
            .unwrap();
        assert_eq!(snapshot.state, State::Planned);
        planned += 1;
        let started = JournalGate::new(&blind, log.log())
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
                let plan = log.log().load(&id).unwrap().plan;
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
    fn binding_lock_serializes_distinct_database_paths() {
        let directory = fixture();
        let first = lock_binding(&directory).unwrap();
        assert_eq!(lock_binding(&directory).unwrap_err(), StorageError::Busy);
        drop(first);
        assert!(lock_binding(&directory).is_ok());
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
        let (path, anchor) = canonical_anchor(path, config, true, vault.clone())?;
        EventLog::open(&path, config, Box::new(Keys), Box::new(anchor))
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
        let (path, anchor) = canonical_anchor(&moved, &config, false, vault.clone()).unwrap();
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
        let (canonical, anchor) = canonical_anchor(&path, &config, false, vault.clone()).unwrap();
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
        let path = fixture().join("audit.db");
        let config = configured_for(&path);
        let vault = MemoryVault::default();
        let log = open_with(&path, &vault, false).unwrap();
        drop(log);
        let (_, anchor) = canonical_anchor(&path, &config, false, vault.clone()).unwrap();
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
