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
