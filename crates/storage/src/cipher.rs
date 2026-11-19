//! Authenticated column cipher, independent of SQLite.
use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead, KeyInit, Payload},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::fmt;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const DATA_DOMAIN: &[u8] = b"urn:ads:cipher:column:1";
const CHECK_DOMAIN: &[u8] = b"urn:ads:cipher:key-check:1";
/// HKDF-Expand `info` labels: the provider key only feeds this derivation.
const AEAD_SUBKEY: &[u8] = b"urn:ads:cipher:subkey:aead:1";
const CHAIN_SUBKEY: &[u8] = b"urn:ads:cipher:subkey:chain:1";

type HmacSha256 = Hmac<Sha256>;

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

pub struct SecretKey(Box<Zeroizing<[u8; 32]>>);
impl SecretKey {
    /// Copies from caller storage; the caller must erase its own copy.
    pub fn new(bytes: &[u8; 32]) -> Self {
        let mut key = Box::new(Zeroizing::new([0; 32]));
        key.copy_from_slice(bytes);
        Self(key)
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

/// HKDF-Expand (RFC 5869) of one 32-byte block. The provider key is already
/// uniformly random, so it serves as the pseudorandom key without Extract.
fn subkey(master: &SecretKey, info: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(&**master.0).expect("HMAC accepts a key of any length");
    mac.update(info);
    mac.update(&[1]);
    let mut out = Zeroizing::new([0; 32]);
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// Key of the row chain, derived from one provider key; never the AEAD key.
pub struct ChainKey(Zeroizing<[u8; 32]>);
impl ChainKey {
    /// HMAC-SHA256 of the length-prefixed `fields`.
    pub fn mac(&self, fields: &[Option<&[u8]>]) -> [u8; 32] {
        let mut input = Vec::new();
        encode_fields(&mut input, fields);
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&*self.0)
            .expect("HMAC accepts a key of any length");
        mac.update(&input);
        mac.finalize().into_bytes().into()
    }
    /// Constant-time comparison of `expected` with the MAC of `fields`.
    pub fn verify(&self, fields: &[Option<&[u8]>], expected: &[u8]) -> bool {
        bool::from(self.mac(fields)[..].ct_eq(expected))
    }
}
impl fmt::Debug for ChainKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChainKey { .. }")
    }
}

/// Chain key of the provider key registered under `id`.
pub fn chain_key(keys: &dyn KeyProvider, id: &KeyId) -> Result<ChainKey, CipherError> {
    Ok(ChainKey(subkey(&keys.by_id(id)?, CHAIN_SUBKEY)))
}

/// Whether `current()` and `by_id` of the same id hand out the same key material.
/// A provider that disagrees would write rows its own `by_id` can never read.
pub fn provider_is_consistent(keys: &dyn KeyProvider) -> Result<bool, CipherError> {
    let (id, current) = keys.current()?;
    let stored = keys.by_id(&id)?;
    Ok(bool::from(current.0[..].ct_eq(&stored.0[..])))
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
/// Length-prefixed list where None and an empty field differ; shared with the chain hash.
pub(crate) fn encode_fields(out: &mut Vec<u8>, items: &[Option<&[u8]>]) {
    out.extend_from_slice(&(items.len() as u64).to_be_bytes());
    for item in items {
        out.push(u8::from(item.is_some()));
        if let Some(bytes) = item {
            field(out, bytes);
        }
    }
}
fn encode(domain: &[u8], id: &KeyId, aad: &AssociatedData<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    field(&mut out, domain);
    field(&mut out, id.as_str().as_bytes());
    encode_fields(&mut out, aad.0);
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
    let cipher = XChaCha20Poly1305::new((&*subkey(&key, AEAD_SUBKEY)).into());
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
) -> Result<Zeroizing<Vec<u8>>, CipherError> {
    let key = keys.by_id(&ciphertext.key_id)?;
    let encoded = encode(domain, &ciphertext.key_id, aad);
    XChaCha20Poly1305::new((&*subkey(&key, AEAD_SUBKEY)).into())
        .decrypt(
            (&ciphertext.nonce).into(),
            Payload {
                msg: &ciphertext.bytes,
                aad: &encoded,
            },
        )
        .map(Zeroizing::new)
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
) -> Result<Zeroizing<Vec<u8>>, CipherError> {
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
                key: SecretKey::new(&[byte; 32]),
            }
        }
    }
    impl KeyProvider for MemoryKeys {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            Ok((KeyId::new(self.id.as_str())?, SecretKey::new(&self.key.0)))
        }
        fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
            if id == &self.id {
                Ok(SecretKey::new(&self.key.0))
            } else {
                Err(CipherError::KeyUnavailable)
            }
        }
    }

    #[test]
    fn repeated_plaintext_uses_distinct_nonce_and_ciphertext() {
        let keys = MemoryKeys::new(42);
        let aad = AssociatedData(&[]);
        let first = encrypt(&keys, &aad, b"same plaintext").unwrap();
        let second = encrypt(&keys, &aad, b"same plaintext").unwrap();
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.bytes, second.bytes);
        let _: Zeroizing<Vec<u8>> = decrypt(&keys, &aad, &first).unwrap();
        let _: Box<Zeroizing<[u8; 32]>> = keys.key.0;
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
        assert_eq!(&**decrypt(&keys, &aad, &ct).unwrap(), b"private sentinel");
        assert_eq!(
            decrypt(&MemoryKeys::new(43), &aad, &ct),
            Err(CipherError::Authentication)
        );
        ct.key_id = KeyId::new("replacement-id").unwrap();
        let renamed = MemoryKeys {
            id: KeyId::new("replacement-id").unwrap(),
            key: SecretKey::new(&[42; 32]),
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
    fn provider_key_only_feeds_separate_derived_keys() {
        let master = SecretKey::new(&[42; 32]);
        let aead = subkey(&master, AEAD_SUBKEY);
        let chain = subkey(&master, CHAIN_SUBKEY);
        assert_ne!(*aead, *chain);
        assert_ne!(*aead, [42; 32]);
        assert_ne!(*chain, [42; 32]);
        // RFC 5869 Expand of one block: HMAC(prk, info || 0x01).
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&[42; 32]).unwrap();
        mac.update(CHAIN_SUBKEY);
        mac.update(&[1]);
        assert_eq!(&*chain, mac.finalize().into_bytes().as_slice());
        // The AEAD runs under the derived key, never the provider key itself.
        let keys = MemoryKeys::new(42);
        let ct = encrypt(&keys, &AssociatedData(&[]), b"x").unwrap();
        let raw = XChaCha20Poly1305::new((&[42u8; 32]).into());
        let encoded = encode(DATA_DOMAIN, &keys.id, &AssociatedData(&[]));
        let payload = Payload {
            msg: &ct.bytes,
            aad: &encoded,
        };
        assert!(raw.decrypt((&ct.nonce).into(), payload).is_err());
    }

    struct Split;
    impl KeyProvider for Split {
        fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
            Ok((KeyId::new("test-key")?, SecretKey::new(&[1; 32])))
        }
        fn by_id(&self, _: &KeyId) -> Result<SecretKey, CipherError> {
            Ok(SecretKey::new(&[2; 32]))
        }
    }

    #[test]
    fn provider_consistency_compares_current_with_by_id() {
        assert_eq!(provider_is_consistent(&MemoryKeys::new(7)), Ok(true));
        assert_eq!(provider_is_consistent(&Split), Ok(false));
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
            key: SecretKey::new(b"0123456789abcdef0123456789abcdef"),
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
            crate::StorageError::RetiredSchema,
            crate::StorageError::PragmaMismatch,
            crate::StorageError::Denied,
            crate::StorageError::Conflict,
            crate::StorageError::WrongKey,
            crate::StorageError::KeyUnavailable,
            crate::StorageError::Tampered,
            crate::StorageError::TooLarge,
            crate::StorageError::InvalidEvent,
            crate::StorageError::Cipher,
            crate::StorageError::ChainBroken { position: 7 },
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
