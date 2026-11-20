//! Composition of the persistent audit log with the product vault.

use std::path::Path;

use ads_adapter_vault::OsVault;
use ads_core::vault::{Secret, SecretName, Vault};
use ads_storage::{
    EventLog, StorageConfig, StorageError,
    anchor::{AnchorStore, COMMITMENT_VERSION, Commitment, ReanchorPlan},
    cipher::KeyProvider,
};

pub struct VaultAnchor<V = OsVault>(pub V);

impl<V: Vault + Send> AnchorStore for VaultAnchor<V> {
    fn read(&self) -> Result<Option<Commitment>, StorageError> {
        self.0
            .read(SecretName::AuditAnchor)
            .map_err(|_| StorageError::AnchorUnavailable)?
            .map(|secret| {
                let commitment: Commitment = secret
                    .expose()
                    .try_into()
                    .map_err(|_| StorageError::AnchorMismatch)?;
                if commitment[0] != COMMITMENT_VERSION {
                    return Err(StorageError::UnsupportedSchema);
                }
                Ok(commitment)
            })
            .transpose()
    }

    fn store(&self, commitment: &Commitment) -> Result<(), StorageError> {
        let secret =
            Secret::new(commitment.to_vec()).map_err(|_| StorageError::AnchorUnavailable)?;
        self.0
            .store(SecretName::AuditAnchor, &secret)
            .map_err(|_| StorageError::AnchorUnavailable)
    }
}

pub fn open(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
) -> Result<EventLog, StorageError> {
    EventLog::open(path, config, keys, Box::new(VaultAnchor(OsVault)))
}

/// Prepares a plan against the OS vault. Review its summary before confirming.
pub fn plan_reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
) -> Result<ReanchorPlan, StorageError> {
    EventLog::plan_reanchor(path, config, keys, &VaultAnchor(OsVault))
}

/// Applies the exact plan confirmed by the owner after making a backup.
pub fn reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    confirmed: ReanchorPlan,
) -> Result<(), StorageError> {
    EventLog::reanchor(path, config, keys, &VaultAnchor(OsVault), confirmed)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use ads_core::vault::VaultError;

    use super::*;

    #[derive(Default)]
    struct MemoryVault(Mutex<Option<Secret>>);

    impl Vault for MemoryVault {
        fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError> {
            assert_eq!(name, SecretName::AuditAnchor);
            *self.0.lock().unwrap() = Some(Secret::new(secret.expose().to_vec())?);
            Ok(())
        }

        fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError> {
            assert_eq!(name, SecretName::AuditAnchor);
            self.0
                .lock()
                .unwrap()
                .as_ref()
                .map(|secret| Secret::new(secret.expose().to_vec()))
                .transpose()
        }

        fn remove(&self, _: SecretName) -> Result<(), VaultError> {
            panic!("the audit anchor must never be removed")
        }
    }

    #[test]
    fn file_execution_uses_the_anchored_gate() {
        use std::{fs, num::NonZeroU64, path::PathBuf};

        use ads_adapter_fs::Executor;
        use ads_core::gate::{
            AbsPath, ActionPlan, Actor, ActorKind, DataClass, Estimate, EventMeta, GatePolicy,
            HostFacts, Id, JournalError, JournalGate, OperationRequest, PlanRequest, State,
            TargetRequest, Timestamp, Ulid,
        };
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
        let mut executor = Executor::new(&fence, policy.clone()).unwrap();
        let facts = executor
            .measure(
                &AbsPath::parse(&work).unwrap(),
                &AbsPath::parse(&target).unwrap(),
            )
            .unwrap();
        let plan = ActionPlan::normalize(&PlanRequest {
            operation: OperationRequest::Delete,
            work_root: work,
            targets: vec![TargetRequest {
                path: target,
                repo_root: facts.repo_root,
            }],
            estimate: Some(Estimate {
                items: facts.items,
                bytes: facts.bytes,
            }),
        })
        .unwrap();
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

        let mut log = EventLog::open(
            &root.join("events.db"),
            &StorageConfig::defaults(),
            Box::new(FixtureKey),
            Box::new(VaultAnchor(MemoryVault::default())),
        )
        .unwrap();
        let id = Id::new("example").unwrap();
        JournalGate::new(&policy, &mut log)
            .plan(id.clone(), &plan, meta(1))
            .unwrap();
        let result =
            crate::gate::execute(&mut log, &policy, &mut executor, &id, meta(2), meta(3)).unwrap();
        assert!(result.operation.is_ok(), "{:?}", result.operation);
        assert_eq!(result.recorded.unwrap().state, State::Executed);
        assert!(!file.exists());
        assert_eq!(log.verify_chain(), Ok(3));
        assert!(matches!(
            crate::gate::execute(&mut log, &policy, &mut executor, &id, meta(4), meta(5)),
            Err(JournalError::Transition)
        ));
    }

    #[test]
    fn commitment_uses_only_the_reserved_audit_slot() {
        let store = VaultAnchor(MemoryVault::default());
        assert_eq!(store.read(), Ok(None));
        let mut commitment = [17; size_of::<Commitment>()];
        commitment[0] = COMMITMENT_VERSION;
        store.store(&commitment).unwrap();
        assert_eq!(store.read(), Ok(Some(commitment)));
        store
            .0
            .store(SecretName::AuditAnchor, &Secret::new(vec![1]).unwrap())
            .unwrap();
        assert_eq!(store.read(), Err(StorageError::AnchorMismatch));
        commitment[0] += 1;
        store.store(&commitment).unwrap();
        assert_eq!(store.read(), Err(StorageError::UnsupportedSchema));
    }
}
