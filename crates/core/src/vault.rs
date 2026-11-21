//! Contract for secrets owned by the product.

use std::fmt;

use zeroize::Zeroizing;

/// Stable names reserved for the product. Add new slots explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretName {
    StorageKey,
    AuditAnchor,
    AuditDatabase,
    AuditAnchorFor([u8; 32]),
}

impl SecretName {
    pub fn as_str(self) -> std::borrow::Cow<'static, str> {
        match self {
            Self::StorageKey => "storage-key".into(),
            Self::AuditAnchor => "audit-anchor".into(),
            Self::AuditDatabase => "audit-database".into(),
            Self::AuditAnchorFor(id) => {
                use fmt::Write;
                let mut name = String::with_capacity("audit-anchor-".len() + 64);
                name.push_str("audit-anchor-");
                for byte in id {
                    write!(name, "{byte:02x}").expect("writing to String cannot fail");
                }
                name.into()
            }
        }
    }
}

impl TryFrom<&str> for SecretName {
    type Error = VaultError;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        match name {
            "storage-key" => Ok(Self::StorageKey),
            "audit-anchor" => Ok(Self::AuditAnchor),
            "audit-database" => Ok(Self::AuditDatabase),
            _ => {
                let id = name
                    .strip_prefix("audit-anchor-")
                    .ok_or(VaultError::InvalidName)?;
                if id.len() != 64
                    || !id
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(VaultError::InvalidName);
                }
                let mut bytes = [0; 32];
                for (index, byte) in bytes.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&id[index * 2..][..2], 16)
                        .map_err(|_| VaultError::InvalidName)?;
                }
                Ok(Self::AuditAnchorFor(bytes))
            }
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

impl<V: Vault> Vault for std::sync::Arc<V> {
    fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError> {
        (**self).store(name, secret)
    }

    fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError> {
        (**self).read(name)
    }

    fn remove(&self, name: SecretName) -> Result<(), VaultError> {
        (**self).remove(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_exact_product_names() {
        for name in [
            SecretName::StorageKey,
            SecretName::AuditAnchor,
            SecretName::AuditDatabase,
            SecretName::AuditAnchorFor([0xab; 32]),
        ] {
            assert_eq!(SecretName::try_from(name.as_str().as_ref()), Ok(name));
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
            "audit-anchor-a",
            concat!(
                "audit-anchor-",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            ),
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
