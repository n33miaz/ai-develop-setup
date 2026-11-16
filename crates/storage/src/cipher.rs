//! Authenticated column cipher, independent of SQLite.
use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead, KeyInit, Payload},
};
use std::fmt;
use zeroize::Zeroize;

const DATA_DOMAIN: &[u8] = b"urn:ads:cipher:column:1";
const CHECK_DOMAIN: &[u8] = b"urn:ads:cipher:key-check:1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherError {
    KeyUnavailable,
    InvalidKeyId,
    Random,
    Authentication,
    Encryption,
}
impl fmt::Display for CipherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cipher: {self:?}")
    }
}
impl std::error::Error for CipherError {}

pub struct SecretKey([u8; 32]);
impl SecretKey {
    /// Transfers the key into owned, zeroized storage. The caller owns other copies.
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}
impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(PartialEq, Eq)]
pub struct KeyId(String);
impl KeyId {
    pub fn new(id: &str) -> Result<Self, CipherError> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(CipherError::InvalidKeyId);
        }
        Ok(Self(id.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyId { .. }")
    }
}

pub trait KeyProvider {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError>;
    fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError>;
}

/// Ordered public columns. Callers must preserve their schema-defined order.
/// None and an empty field have distinct encodings; the key id is added internally.
pub struct AssociatedData<'a>(pub &'a [Option<&'a [u8]>]);
impl fmt::Debug for AssociatedData<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AssociatedData { .. }")
    }
}

pub struct Ciphertext {
    pub key_id: KeyId,
    pub nonce: [u8; 24],
    /// Ciphertext followed by the 16-byte authentication tag.
    pub bytes: Vec<u8>,
}
impl fmt::Debug for Ciphertext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Ciphertext { .. }")
    }
}

fn field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}
fn encode(domain: &[u8], id: &KeyId, aad: &AssociatedData<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    field(&mut out, domain);
    field(&mut out, id.as_str().as_bytes());
    out.extend_from_slice(&(aad.0.len() as u64).to_be_bytes());
    for item in aad.0 {
        out.push(u8::from(item.is_some()));
        if let Some(bytes) = item {
            field(&mut out, bytes);
        }
    }
    out
}
fn nonce() -> Result<[u8; 24], CipherError> {
    let mut value = [0; 24];
    getrandom::fill(&mut value).map_err(|_| CipherError::Random)?;
    Ok(value)
}
fn seal(
    keys: &dyn KeyProvider,
    domain: &[u8],
    aad: &AssociatedData<'_>,
    plaintext: &[u8],
) -> Result<Ciphertext, CipherError> {
    let (key_id, key) = keys.current()?;
    let nonce = nonce()?;
    let encoded = encode(domain, &key_id, aad);
    let cipher = XChaCha20Poly1305::new((&key.0).into());
    let bytes = cipher
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: plaintext,
                aad: &encoded,
            },
        )
        .map_err(|_| CipherError::Encryption)?;
    Ok(Ciphertext {
        key_id,
        nonce,
        bytes,
    })
}
fn unseal(
    keys: &dyn KeyProvider,
    domain: &[u8],
    aad: &AssociatedData<'_>,
    ciphertext: &Ciphertext,
) -> Result<Vec<u8>, CipherError> {
    let key = keys.by_id(&ciphertext.key_id)?;
    let encoded = encode(domain, &ciphertext.key_id, aad);
    XChaCha20Poly1305::new((&key.0).into())
        .decrypt(
            (&ciphertext.nonce).into(),
            Payload {
                msg: &ciphertext.bytes,
                aad: &encoded,
            },
        )
        .map_err(|_| CipherError::Authentication)
}
pub fn encrypt(
    keys: &dyn KeyProvider,
    aad: &AssociatedData<'_>,
    plaintext: &[u8],
) -> Result<Ciphertext, CipherError> {
    seal(keys, DATA_DOMAIN, aad, plaintext)
}
pub fn decrypt(
    keys: &dyn KeyProvider,
    aad: &AssociatedData<'_>,
    ciphertext: &Ciphertext,
) -> Result<Vec<u8>, CipherError> {
    unseal(keys, DATA_DOMAIN, aad, ciphertext)
}
pub fn key_check(keys: &dyn KeyProvider) -> Result<Ciphertext, CipherError> {
    seal(keys, CHECK_DOMAIN, &AssociatedData(&[]), &[])
}
pub fn verify_key_check(keys: &dyn KeyProvider, check: &Ciphertext) -> Result<(), CipherError> {
    let result = unseal(keys, CHECK_DOMAIN, &AssociatedData(&[]), check)?;
    if result.is_empty() {
        Ok(())
    } else {
        Err(CipherError::Authentication)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct MemoryKeys {
        id: KeyId,
        key: SecretKey,
    }
    impl MemoryKeys {
        fn new(byte: u8) -> Self {
            Self {
                id: KeyId::new("test-key").unwrap(),
                key: SecretKey::new([byte; 32]),
            }
        }
    }
    impl KeyProvider for MemoryKeys {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            Ok((KeyId::new(self.id.as_str())?, SecretKey::new(self.key.0)))
        }
        fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
            if id == &self.id {
                Ok(SecretKey::new(self.key.0))
            } else {
                Err(CipherError::KeyUnavailable)
            }
        }
    }

    #[test]
    fn round_trip_and_all_authenticated_inputs() {
        let keys = MemoryKeys::new(42);
        let fields: Vec<Option<&[u8]>> = vec![
            Some(b"position"),
            Some(b"id"),
            Some(b"type"),
            Some(b"ts"),
            Some(b"stream"),
            Some(b"seq"),
            Some(b"actor_kind"),
            Some(b"actor_id"),
            None,
            Some(b"data_class"),
            Some(b"action_id"),
            None,
        ];
        let aad = AssociatedData(&fields);
        let mut ct = encrypt(&keys, &aad, b"private sentinel").unwrap();
        assert_eq!(decrypt(&keys, &aad, &ct).unwrap(), b"private sentinel");
        assert_eq!(
            decrypt(&MemoryKeys::new(43), &aad, &ct),
            Err(CipherError::Authentication)
        );
        ct.key_id = KeyId::new("replacement-id").unwrap();
        let renamed = MemoryKeys {
            id: KeyId::new("replacement-id").unwrap(),
            key: SecretKey::new([42; 32]),
        };
        assert_eq!(
            decrypt(&renamed, &aad, &ct),
            Err(CipherError::Authentication)
        );
        assert_eq!(decrypt(&keys, &aad, &ct), Err(CipherError::KeyUnavailable));
        ct.key_id = KeyId::new("test-key").unwrap();
        for index in 0..fields.len() {
            let mut changed = fields.clone();
            changed[index] = Some(b"changed");
            assert_eq!(
                decrypt(&keys, &AssociatedData(&changed), &ct),
                Err(CipherError::Authentication)
            );
        }
        ct.nonce[0] ^= 1;
        assert_eq!(decrypt(&keys, &aad, &ct), Err(CipherError::Authentication));
        ct.nonce[0] ^= 1;
        ct.bytes[0] ^= 1;
        assert_eq!(decrypt(&keys, &aad, &ct), Err(CipherError::Authentication));
        ct.bytes[0] ^= 1;
        while ct.bytes.pop().is_some() {
            assert_eq!(decrypt(&keys, &aad, &ct), Err(CipherError::Authentication));
        }
        let check = key_check(&keys).unwrap();
        verify_key_check(&keys, &check).unwrap();
        assert_eq!(
            verify_key_check(&MemoryKeys::new(43), &check),
            Err(CipherError::Authentication)
        );
        assert_eq!(
            decrypt(&keys, &AssociatedData(&[]), &check),
            Err(CipherError::Authentication)
        );
    }

    #[test]
    fn aad_encoding_has_no_concatenation_or_null_ambiguity() {
        let id = KeyId::new("test-key").unwrap();
        let encode_fields = |fields| encode(DATA_DOMAIN, &id, &AssociatedData(fields));
        assert_ne!(
            encode_fields(&[Some(b"ab"), Some(b"c")]),
            encode_fields(&[Some(b"a"), Some(b"bc")])
        );
        assert_ne!(encode_fields(&[None]), encode_fields(&[Some(b"")]));
        assert_ne!(encode_fields(&[]), encode_fields(&[None]));
    }

    #[test]
    fn hundred_thousand_system_nonces_are_distinct() {
        let mut seen = HashSet::with_capacity(100_000);
        for _ in 0..100_000 {
            assert!(seen.insert(nonce().unwrap()));
        }
    }

    #[test]
    fn primitive_matches_published_xchacha_vector() {
        // Public numeric vector: draft-irtf-cfrg-xchacha-03, appendix A.3.1.
        // https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03#appendix-A.3.1
        let key: [u8; 32] = std::array::from_fn(|i| 0x80 + i as u8);
        let nonce: [u8; 24] = std::array::from_fn(|i| 0x40 + i as u8);
        let aad = [
            0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
        ];
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let expected_hex = concat!(
            "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb",
            "731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452",
            "2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9",
            "21f9664c97637da9768812f615c68b13b52ec0875924c1c7987947deafd8780acf49"
        );
        let expected: Vec<u8> = (0..expected_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&expected_hex[i..i + 2], 16).unwrap())
            .collect();
        let cipher = XChaCha20Poly1305::new((&key).into());
        let result = cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .unwrap();
        assert_eq!(result, expected);
        assert_eq!(
            cipher
                .decrypt(
                    (&nonce).into(),
                    Payload {
                        msg: &expected,
                        aad: &aad
                    }
                )
                .unwrap(),
            plaintext
        );
    }

    #[test]
    fn diagnostics_do_not_disclose_key_plaintext_or_aad() {
        let keys = MemoryKeys {
            id: KeyId::new("diagnostic-key").unwrap(),
            key: SecretKey::new(*b"0123456789abcdef0123456789abcdef"),
        };
        let aad = AssociatedData(&[Some(b"private-aad-sentinel")]);
        let ct = encrypt(&keys, &aad, b"private-plaintext-sentinel").unwrap();
        let mut output = format!("{ct:?} {:?} {aad:?}", ct.key_id);
        for error in [
            CipherError::KeyUnavailable,
            CipherError::InvalidKeyId,
            CipherError::Random,
            CipherError::Authentication,
            CipherError::Encryption,
        ] {
            output.push_str(&format!("{error} {error:?}"));
        }
        for error in [
            crate::StorageError::Configuration,
            crate::StorageError::Io,
            crate::StorageError::Busy,
            crate::StorageError::Corrupt,
            crate::StorageError::ForeignApplication,
            crate::StorageError::SchemaTooNew,
            crate::StorageError::UnsupportedSchema,
            crate::StorageError::PragmaMismatch,
        ] {
            output.push_str(&format!("{error} {error:?}"));
        }
        for sentinel in [
            "0123456789abcdef0123456789abcdef",
            "private-plaintext-sentinel",
            "private-aad-sentinel",
            "diagnostic-key",
        ] {
            assert!(!output.contains(sentinel));
        }
        assert!(!output.contains(&format!("{:?}", keys.key.0)));
    }
}
