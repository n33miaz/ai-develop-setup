//! Contract for secrets owned by the product.

use std::fmt;

use zeroize::Zeroizing;

/// Stable names reserved for the product. Add new slots explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretName {
    StorageKey,
    AuditAnchor,
}

impl SecretName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StorageKey => "storage-key",
            Self::AuditAnchor => "audit-anchor",
        }
    }
}

impl TryFrom<&str> for SecretName {
    type Error = VaultError;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        match name {
            name if name == Self::StorageKey.as_str() => Ok(Self::StorageKey),
            name if name == Self::AuditAnchor.as_str() => Ok(Self::AuditAnchor),
            _ => Err(VaultError::InvalidName),
        }
    }
}

/// Owned binary secret. The supplied allocation is cleared on drop.
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    /// Maximum size supported by the vault contract, in bytes.
    pub const MAX_BYTES: usize = 2560;

    pub fn new(bytes: Vec<u8>) -> Result<Self, VaultError> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty() || bytes.len() > Self::MAX_BYTES {
            return Err(VaultError::InvalidSecretSize);
        }
        Ok(Self(bytes))
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultError {
    InvalidName,
    InvalidSecretSize,
    Unsupported,
    Unavailable,
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidName => "unrecognized product secret name",
            Self::InvalidSecretSize => "invalid product secret size",
            Self::Unsupported => "the operating system vault is not supported on this platform",
            Self::Unavailable => "the operating system vault could not complete the operation",
        })
    }
}

impl std::error::Error for VaultError {}

pub trait Vault {
    /// Create or replace the named secret.
    fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError>;
    /// Return None for an absent entry.
    fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError>;
    /// Remove the entry, succeeding even if already absent.
    fn remove(&self, name: SecretName) -> Result<(), VaultError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_exact_product_names() {
        for name in [SecretName::StorageKey, SecretName::AuditAnchor] {
            assert_eq!(SecretName::try_from(name.as_str()), Ok(name));
        }
        for invalid in [
            "",
            "Storage-Key",
            "storage-key ",
            "storage-key\0",
            "../storage-key",
            "urn:ads:vault:storage-key:1",
            "ads-test-storage-key",
            "other",
        ] {
            assert_eq!(SecretName::try_from(invalid), Err(VaultError::InvalidName));
        }
    }

    #[test]
    fn secret_validates_size_and_redacts_debug() {
        let secret = Secret::new(vec![42; Secret::MAX_BYTES]).unwrap();
        assert_eq!(secret.expose().len(), Secret::MAX_BYTES);
        assert_eq!(format!("{secret:?}"), "Secret([redacted])");
        for size in [0, Secret::MAX_BYTES + 1] {
            assert_eq!(
                Secret::new(vec![42; size]).unwrap_err(),
                VaultError::InvalidSecretSize
            );
        }
    }
}
