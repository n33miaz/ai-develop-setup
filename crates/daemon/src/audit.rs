//! Composition of the persistent audit log with the product vault.

use std::{
    fs,
    path::{Path, PathBuf},
};

use ads_adapter_vault::OsVault;
use ads_core::vault::{Secret, SecretName, Vault};
use ads_storage::{
    EventLog, StorageConfig, StorageError,
    anchor::{AnchorStore, COMMITMENT_VERSION, Commitment, DatabaseId, ReanchorPlan},
    cipher::KeyProvider,
};
use sha2::{Digest, Sha256};

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
        self.read_secret(SecretName::AuditAnchorFor(id))?
            .map(|secret| {
                let (location, commitment) = secret
                    .expose()
                    .split_at_checked(self.location.len())
                    .ok_or(StorageError::AnchorMismatch)?;
                if location != self.location {
                    return Err(StorageError::DuplicateDatabaseIdentity);
                }
                decode(commitment)
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
pub fn canonical_path() -> Result<PathBuf, StorageError> {
    if !cfg!(windows) {
        return Err(StorageError::AnchorUnavailable);
    }
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|root| root.is_absolute())
        .ok_or(StorageError::Configuration)?;
    Ok(root.join("ads").join("audit.db"))
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

fn canonical_anchor(
    path: &Path,
    create_dir: bool,
) -> Result<(PathBuf, VaultAnchor<OsVault>), StorageError> {
    let canonical = require_canonical_path(path, &canonical_path()?, create_dir)?;
    let anchor = VaultAnchor::new(OsVault, &canonical, true)?;
    Ok((canonical, anchor))
}

pub fn open(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
) -> Result<EventLog, StorageError> {
    let (path, anchor) = canonical_anchor(path, true)?;
    EventLog::open(&path, config, keys, Box::new(anchor))
}

/// Prepares a plan against the OS vault. Review its summary before confirming.
pub fn plan_reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
) -> Result<ReanchorPlan, StorageError> {
    let (path, anchor) = canonical_anchor(path, false)?;
    EventLog::plan_reanchor(&path, config, keys, &anchor)
}

/// Applies the exact plan confirmed by the owner after making a backup.
pub fn reanchor(
    path: &Path,
    config: &StorageConfig,
    keys: Box<dyn KeyProvider + Send>,
    confirmed: ReanchorPlan,
) -> Result<(), StorageError> {
    let (path, anchor) = canonical_anchor(path, false)?;
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
    fn product_uses_only_the_canonical_database_path() {
        let root = fixture();
        let canonical = root.join("ads").join("audit.db");
        let other = root.join("audit.db");
        assert_eq!(
            require_canonical_path(&canonical, &canonical, true),
            Ok(canonical.clone())
        );
        assert_eq!(
            require_canonical_path(&other, &canonical, true),
            Err(StorageError::NonCanonicalDatabase)
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
        fs::copy(&unrelated, &original).unwrap();
        assert_eq!(
            open_with(&original, &migrated, true).unwrap_err(),
            StorageError::AnchorMismatch
        );
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
