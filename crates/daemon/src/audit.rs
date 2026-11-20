//! Composition of the persistent audit log with the product vault.

use std::path::Path;

use ads_adapter_vault::OsVault;
use ads_core::vault::{Secret, SecretName, Vault};
use ads_storage::{
    EventLog, StorageConfig, StorageError,
    anchor::{AnchorStore, Commitment},
    cipher::KeyProvider,
};

pub struct VaultAnchor<V = OsVault>(pub V);

impl<V: Vault + Send> AnchorStore for VaultAnchor<V> {
    fn read(&self) -> Result<Option<Commitment>, StorageError> {
        self.0
            .read(SecretName::AuditAnchor)
            .map_err(|_| StorageError::AnchorUnavailable)?
            .map(|secret| {
                secret
                    .expose()
                    .try_into()
                    .map_err(|_| StorageError::AnchorMismatch)
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
        let commitment = [17; size_of::<Commitment>()];
        store.store(&commitment).unwrap();
        assert_eq!(store.read(), Ok(Some(commitment)));
        store
            .0
            .store(SecretName::AuditAnchor, &Secret::new(vec![1]).unwrap())
            .unwrap();
        assert_eq!(store.read(), Err(StorageError::AnchorMismatch));
    }
}
