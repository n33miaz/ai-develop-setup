//! Adapter for the operating system secret vault.

use ads_core::vault::{Secret, SecretName, Vault, VaultError};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub(super) fn store(_: SecretName, _: &Secret) -> Result<(), VaultError> {
        Err(VaultError::Unsupported)
    }

    pub(super) fn read(_: SecretName) -> Result<Option<Secret>, VaultError> {
        Err(VaultError::Unsupported)
    }

    pub(super) fn remove(_: SecretName) -> Result<(), VaultError> {
        Err(VaultError::Unsupported)
    }
}

#[cfg(all(windows, feature = "test-support"))]
pub use windows::TestVault;

/// Credential vault of the current operating system user.
pub struct OsVault;

impl Vault for OsVault {
    fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError> {
        platform::store(name, secret)
    }

    fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError> {
        platform::read(name)
    }

    fn remove(&self, name: SecretName) -> Result<(), VaultError> {
        platform::remove(name)
    }
}

/// Package name as declared in the manifest.
pub const fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platform_has_no_fallback() {
        let name = SecretName::StorageKey;
        let secret = Secret::new(vec![1]).unwrap();
        assert_eq!(OsVault.store(name, &secret), Err(VaultError::Unsupported));
        assert_eq!(OsVault.read(name).unwrap_err(), VaultError::Unsupported);
        assert_eq!(OsVault.remove(name), Err(VaultError::Unsupported));
    }

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(crate_name(), "ads-adapter-vault");
    }
}
