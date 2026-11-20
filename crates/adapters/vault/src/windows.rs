#![allow(unsafe_code)]

use std::{iter, ptr, slice};

use ::windows::{
    Win32::{
        Foundation::ERROR_NOT_FOUND,
        Security::Credentials::{
            CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
            CredReadW, CredWriteW,
        },
    },
    core::{Error, HRESULT, PCWSTR, PWSTR},
};
use zeroize::Zeroize;

use super::*;

pub(super) fn store(name: SecretName, secret: &Secret) -> Result<(), VaultError> {
    Target::product(name).store(secret)
}

pub(super) fn read(name: SecretName) -> Result<Option<Secret>, VaultError> {
    Target::product(name).read()
}

pub(super) fn remove(name: SecretName) -> Result<(), VaultError> {
    Target::product(name).remove()
}

struct Target(Vec<u16>);

impl Target {
    fn product(name: SecretName) -> Self {
        Self::new(&format!("urn:ads:vault:{}:1", name.as_str()))
    }

    fn new(name: &str) -> Self {
        Self(name.encode_utf16().chain(iter::once(0)).collect())
    }

    fn store(&self, secret: &Secret) -> Result<(), VaultError> {
        let bytes = secret.expose();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(self.0.as_ptr().cast_mut()),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // SAFETY: CredWriteW only reads the input, including the live name and blob.
        unsafe { CredWriteW(&credential, 0) }.map_err(|_| VaultError::Unavailable)
    }

    fn read(&self) -> Result<Option<Secret>, VaultError> {
        let mut raw = ptr::null_mut();
        // SAFETY: the name is live and NUL-terminated, and raw is writable.
        match unsafe { CredReadW(PCWSTR(self.0.as_ptr()), CRED_TYPE_GENERIC, None, &mut raw) } {
            Ok(()) => {
                let mut credential = ReadCredential(raw);
                Ok(Some(Secret::new(credential.blob().to_vec())?))
            }
            Err(error) if is_missing(&error) => Ok(None),
            Err(_) => Err(VaultError::Unavailable),
        }
    }

    fn remove(&self) -> Result<(), VaultError> {
        // SAFETY: the name is live and NUL-terminated.
        match unsafe { CredDeleteW(PCWSTR(self.0.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if is_missing(&error) => Ok(()),
            Err(_) => Err(VaultError::Unavailable),
        }
    }
}

fn is_missing(error: &Error) -> bool {
    error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0)
}

struct ReadCredential(*mut CREDENTIALW);

impl ReadCredential {
    fn blob(&mut self) -> &mut [u8] {
        // SAFETY: CredReadW succeeded, and this guard uniquely owns the result.
        let credential = unsafe { &mut *self.0 };
        if credential.CredentialBlobSize == 0 {
            return &mut [];
        }
        // SAFETY: the nonempty blob is writable within the same owned allocation.
        unsafe {
            slice::from_raw_parts_mut(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        }
    }
}

impl Drop for ReadCredential {
    fn drop(&mut self) {
        self.blob().zeroize();
        // SAFETY: this is the base pointer returned by CredReadW, freed just once.
        unsafe { CredFree(self.0.cast()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    struct TestEntry(Target);

    impl TestEntry {
        fn create(secret: &Secret) -> Self {
            let mut nonce = [0_u8; 16];
            getrandom::fill(&mut nonce).unwrap();
            let mut name = String::from("ads-test-");
            for byte in nonce {
                write!(name, "{byte:02x}").unwrap();
            }

            let target = Target::new(&name);
            target.store(secret).unwrap();
            Self(target)
        }
    }

    impl Drop for TestEntry {
        fn drop(&mut self) {
            if self.0.remove().is_err() {
                eprintln!("could not clean up the vault test entry");
            }
        }
    }

    #[test]
    #[ignore = "requires a Windows logon session with a usable credential set"]
    fn credential_manager_round_trip() {
        let original = Secret::new(vec![0, 128, 255]).unwrap();
        let replacement = Secret::new(vec![37; Secret::MAX_BYTES]).unwrap();
        let entry = TestEntry::create(&original);
        let target = &entry.0;
        assert!(target.read().unwrap().unwrap().expose() == original.expose());
        target.store(&replacement).unwrap();
        assert!(target.read().unwrap().unwrap().expose() == replacement.expose());
        target.remove().unwrap();
        assert!(target.read().unwrap().is_none());
        target.remove().unwrap();
    }
}
