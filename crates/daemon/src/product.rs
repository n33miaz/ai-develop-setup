use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use ads_contracts::CommonIdV1;
use ads_core::vault::{Secret, SecretName, Vault};
use ads_private_dir::ControlDir;
use ads_storage::{
    StorageError,
    cipher::{CipherError, KeyId, KeyProvider, SecretKey},
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    approvals::{ApprovalDesk, ApprovalOwner},
    audit::{ProductLog, ProductLogError},
    gate::{SessionGate, spellings},
    queue::{LocalQueue, QueueOpenError, QueueOwner},
};

const IDENTITIES: &str = "local-identities.toml";
const MAX_IDENTITY_BYTES: u64 = 1024;

#[derive(Debug)]
pub(crate) enum IdentityError {
    Unreadable,
    Invalid,
    Missing,
    Random,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalIdentities {
    pub(crate) queue_id: CommonIdV1,
    owner_id: CommonIdV1,
    device_id: CommonIdV1,
}

impl LocalIdentities {
    pub(crate) fn load(dir: &ControlDir, first_use: bool) -> Result<Self, IdentityError> {
        let mut contents = String::new();
        match dir.open_private_file(IDENTITIES) {
            Ok(file) => {
                file.take(MAX_IDENTITY_BYTES + 1)
                    .read_to_string(&mut contents)
                    .map_err(|_| IdentityError::Invalid)?;
                if contents.len() as u64 > MAX_IDENTITY_BYTES {
                    return Err(IdentityError::Invalid);
                }
                toml::from_str(&contents).map_err(|_| IdentityError::Invalid)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !first_use {
                    return Err(IdentityError::Missing);
                }
                let ids = Self {
                    queue_id: random_id()?,
                    owner_id: random_id()?,
                    device_id: random_id()?,
                };
                let contents = toml::to_string(&ids).map_err(|_| IdentityError::Invalid)?;
                dir.publish(IDENTITIES, contents.as_bytes())
                    .map_err(|_| IdentityError::Unreadable)?;
                Ok(ids)
            }
            Err(_) => Err(IdentityError::Unreadable),
        }
    }
}

fn random_id() -> Result<CommonIdV1, IdentityError> {
    use std::fmt::Write;
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| IdentityError::Random)?;
    let mut id = String::with_capacity(32);
    for byte in bytes {
        write!(id, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(CommonIdV1::new(id).expect("hexadecimal id is valid"))
}

pub(crate) fn first_use(path: &Path, vault: &impl Vault) -> Result<bool, StorageError> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = path.as_os_str().to_owned();
        candidate.push(suffix);
        match fs::symlink_metadata(candidate) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    for name in [SecretName::AuditDatabase, SecretName::AuditAnchor] {
        if vault
            .read(name)
            .map_err(|_| StorageError::AnchorUnavailable)?
            .is_some()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) struct VaultKeys<V>(pub(crate) V);

impl<V: Vault> VaultKeys<V> {
    pub(crate) fn provision(&self, first_use: bool) -> Result<(), StorageError> {
        let name = SecretName::StorageKey;
        match self
            .0
            .read(name)
            .map_err(|_| StorageError::KeyUnavailable)?
        {
            Some(secret) if secret.expose().len() == 32 => Ok(()),
            Some(_) => Err(StorageError::WrongKey),
            None if first_use => {
                let mut bytes = Zeroizing::new(vec![0; 32]);
                getrandom::fill(&mut bytes).map_err(|_| CipherError::Random)?;
                let secret = Secret::new(std::mem::take(&mut *bytes))
                    .map_err(|_| StorageError::KeyUnavailable)?;
                self.0
                    .store(name, &secret)
                    .map_err(|_| StorageError::KeyUnavailable)
            }
            None => Err(StorageError::KeyUnavailable),
        }
    }
}

impl<V: Vault> KeyProvider for VaultKeys<V> {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
        let id = KeyId::new(SecretName::StorageKey.as_str().as_ref())?;
        let key = self.by_id(&id)?;
        Ok((id, key))
    }

    fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
        let name = SecretName::StorageKey;
        if id.as_str() != name.as_str() {
            return Err(CipherError::KeyUnavailable);
        }
        let secret = self
            .0
            .read(name)
            .ok()
            .flatten()
            .ok_or(CipherError::KeyUnavailable)?;
        let bytes = secret
            .expose()
            .try_into()
            .map_err(|_| CipherError::KeyUnavailable)?;
        Ok(SecretKey::new(bytes))
    }
}

#[derive(Debug)]
pub(crate) enum StartupError {
    Audit(ProductLogError),
    Identity(IdentityError),
    Queue(QueueOpenError),
    GateConfiguration,
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Audit(ProductLogError::Storage(error)) => write!(f, "{error}"),
            Self::Audit(ProductLogError::State(_)) => f.write_str("invalid audit product paths"),
            Self::Identity(error) => write!(f, "local identities: {error:?}"),
            Self::Queue(QueueOpenError::Storage(error)) => write!(f, "queue: {error}"),
            Self::Queue(error) => write!(f, "queue history could not be restored: {error:?}"),
            Self::GateConfiguration => f.write_str("invalid approval gate configuration"),
        }
    }
}

impl From<ProductLogError> for StartupError {
    fn from(error: ProductLogError) -> Self {
        Self::Audit(error)
    }
}

impl From<StorageError> for StartupError {
    fn from(error: StorageError) -> Self {
        Self::Audit(error.into())
    }
}

pub(crate) fn controls(
    log: ProductLog,
    ids: LocalIdentities,
    workdirs: &[PathBuf],
    max_list_bytes: usize,
) -> Result<(Arc<LocalQueue>, Arc<ApprovalDesk>), StartupError> {
    let gates = gates(&log, workdirs)?;
    let queue = LocalQueue::bind_or_pending(
        log.shared_log(),
        ids.queue_id,
        QueueOwner {
            owner_id: ids.owner_id.clone(),
            device_id: ids.device_id.clone(),
        },
    )
    .map_err(StartupError::Queue)?;
    let approvals = ApprovalDesk::new(
        log,
        gates,
        ApprovalOwner {
            owner_id: ids.owner_id,
            device_id: ids.device_id,
        },
        max_list_bytes,
    );
    Ok((Arc::new(queue), Arc::new(approvals)))
}

fn gates(log: &ProductLog, workdirs: &[PathBuf]) -> Result<Vec<SessionGate>, StartupError> {
    use ads_core::gate::{GatePolicy, HostFacts};

    if workdirs.is_empty() {
        return Ok(Vec::new());
    }
    let invalid = |_| StartupError::GateConfiguration;
    let home = std::env::home_dir().ok_or(StartupError::GateConfiguration)?;
    let user_home = home.to_str().ok_or(StartupError::GateConfiguration)?;
    let policy = GatePolicy::defaults(&HostFacts { user_home }).map_err(invalid)?;
    let mut roots = Vec::new();
    let mut gates = Vec::new();
    for path in workdirs {
        for root in spellings(path).map_err(|_| StartupError::GateConfiguration)? {
            if !roots.contains(&root) {
                roots.push(root.clone());
                gates.push((root, path));
            }
        }
    }
    gates
        .into_iter()
        .map(|(root, fence)| {
            SessionGate::new(root, roots.clone(), policy.clone(), log, fence)
                .map_err(|_| StartupError::GateConfiguration)
        })
        .collect()
}
