//! Authenticated column cipher, independent of SQLite.
use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead, AeadCore, KeyInit, Payload},
};
use std::fmt;
use storage_sha2::{Digest, Sha256, digest::typenum::Unsigned};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

/// Bytes added to the plaintext in the encrypted data column. The nonce is separate.
pub const DATA_TAG_BYTES: usize = <XChaCha20Poly1305 as AeadCore>::TagSize::USIZE;

const DATA_DOMAIN: &[u8] = b"urn:ads:cipher:column:1";
const CHECK_DOMAIN: &[u8] = b"urn:ads:cipher:key-check:1";
/// HKDF-Expand `info` labels: the provider key only feeds this derivation.
const AEAD_SUBKEY: &[u8] = b"urn:ads:cipher:subkey:aead:1";
const CHAIN_SUBKEY: &[u8] = b"urn:ads:cipher:subkey:chain:1";
const CHECK_SUBKEY: &[u8] = b"urn:ads:cipher:subkey:key-check:1";

/// RFC 2104 for the fixed 32-byte keys used by storage.
fn hmac_sha256(key: &[u8; 32], feed: impl FnOnce(&mut Sha256)) -> Zeroizing<[u8; 32]> {
    let mut pad = Zeroizing::new([0x36; 64]);
    for (byte, key_byte) in pad.iter_mut().zip(key) {
        *byte ^= key_byte;
    }
    let mut inner = Sha256::new();
    inner.update(pad.as_slice());
    feed(&mut inner);
    let mut digest = Zeroizing::new([0; 32]);
    inner.finalize_into_reset((&mut *digest).into());

    for byte in pad.iter_mut() {
        *byte ^= 0x36 ^ 0x5c;
    }
    let mut outer = Sha256::new();
    outer.update(pad.as_slice());
    outer.update(digest.as_slice());
    let mut output = Zeroizing::new([0; 32]);
    outer.finalize_into_reset((&mut *output).into());
    output
}

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

/// HKDF-Expand (RFC 5869) of one 32-byte block. `KeyProvider` keys must be
/// uniformly random, so the provider key serves as the pseudorandom key without
/// Extract.
fn subkey(master: &SecretKey, info: &[u8]) -> Zeroizing<[u8; 32]> {
    hmac_sha256(&master.0, |hash| {
        hash.update(info);
        hash.update([1]);
    })
}

/// Key of the row chain, derived from one provider key; never the AEAD key.
pub struct ChainKey(Zeroizing<[u8; 32]>);
impl ChainKey {
    /// HMAC-SHA256 of the length-prefixed `fields`.
    pub fn mac(&self, fields: &[Option<&[u8]>]) -> [u8; 32] {
        *self.tag(fields)
    }
    /// Constant-time comparison of `expected` with the MAC of `fields`.
    pub fn verify(&self, fields: &[Option<&[u8]>], expected: &[u8]) -> bool {
        bool::from(self.tag(fields)[..].ct_eq(expected))
    }

    fn tag(&self, fields: &[Option<&[u8]>]) -> Zeroizing<[u8; 32]> {
        hmac_sha256(&self.0, |hash| {
            encode_fields(fields, |bytes| hash.update(bytes))
        })
    }
}
impl fmt::Debug for ChainKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChainKey { .. }")
    }
}

impl ChainKey {
    /// Chain key derived from `key`.
    pub fn derive(key: &SecretKey) -> Self {
        Self(subkey(key, CHAIN_SUBKEY))
    }
}

/// Chain key of the provider key registered under `id`.
pub fn chain_key(keys: &dyn KeyProvider, id: &KeyId) -> Result<ChainKey, CipherError> {
    Ok(ChainKey::derive(&keys.by_id(id)?))
}

/// Constant-time comparison of two keys' material.
pub fn same_key(a: &SecretKey, b: &SecretKey) -> bool {
    bool::from(a.0[..].ct_eq(&b.0[..]))
}

/// Whether `current()` and `by_id` of the same id hand out the same key material.
/// A provider that disagrees would write rows its own `by_id` can never read.
pub fn provider_is_consistent(keys: &dyn KeyProvider) -> Result<bool, CipherError> {
    let (id, current) = keys.current()?;
    let stored = keys.by_id(&id)?;
    Ok(same_key(&current, &stored))
}

/// Source of column keys. Every key must be 32 bytes from a cryptographic random
/// generator (uniform, 256 bits): derivation skips the HKDF Extract step, so a key
/// made from a password or other non-uniform material is not acceptable.
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
fn encode_fields(items: &[Option<&[u8]>], mut write: impl FnMut(&[u8])) {
    write(&(items.len() as u64).to_be_bytes());
    for item in items {
        write(&[u8::from(item.is_some())]);
        if let Some(bytes) = item {
            write(&(bytes.len() as u64).to_be_bytes());
            write(bytes);
        }
    }
}
fn encode(domain: &[u8], id: &KeyId, aad: &AssociatedData<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    field(&mut out, domain);
    field(&mut out, id.as_str().as_bytes());
    encode_fields(aad.0, |bytes| out.extend_from_slice(bytes));
    out
}
fn nonce() -> Result<[u8; 24], CipherError> {
    let mut value = [0; 24];
    getrandom::fill(&mut value).map_err(|_| CipherError::Random)?;
    Ok(value)
}
/// Subkey of the AEAD for `domain`: data and key check never share one.
fn aead_label(domain: &[u8]) -> &'static [u8] {
    if domain == CHECK_DOMAIN {
        CHECK_SUBKEY
    } else {
        AEAD_SUBKEY
    }
}
fn seal_with(
    key_id: &KeyId,
    key: &SecretKey,
    domain: &[u8],
    aad: &AssociatedData<'_>,
    plaintext: &[u8],
) -> Result<Ciphertext, CipherError> {
    let nonce = nonce()?;
    let encoded = encode(domain, key_id, aad);
    let cipher = XChaCha20Poly1305::new((&*subkey(key, aead_label(domain))).into());
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
        key_id: KeyId::new(key_id.as_str())?,
        nonce,
        bytes,
    })
}
fn unseal_with(
    key: &SecretKey,
    domain: &[u8],
    aad: &AssociatedData<'_>,
    ciphertext: &Ciphertext,
) -> Result<Zeroizing<Vec<u8>>, CipherError> {
    let encoded = encode(domain, &ciphertext.key_id, aad);
    XChaCha20Poly1305::new((&*subkey(key, aead_label(domain))).into())
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
    let (key_id, key) = keys.current()?;
    seal_with(&key_id, &key, DATA_DOMAIN, aad, plaintext)
}
/// `encrypt` with key material the caller already fetched once.
pub fn encrypt_with(
    key_id: &KeyId,
    key: &SecretKey,
    aad: &AssociatedData<'_>,
    plaintext: &[u8],
) -> Result<Ciphertext, CipherError> {
    seal_with(key_id, key, DATA_DOMAIN, aad, plaintext)
}
pub fn decrypt(
    keys: &dyn KeyProvider,
    aad: &AssociatedData<'_>,
    ciphertext: &Ciphertext,
) -> Result<Zeroizing<Vec<u8>>, CipherError> {
    let key = keys.by_id(&ciphertext.key_id)?;
    decrypt_with(&key, aad, ciphertext)
}

pub(crate) fn decrypt_with(
    key: &SecretKey,
    aad: &AssociatedData<'_>,
    ciphertext: &Ciphertext,
) -> Result<Zeroizing<Vec<u8>>, CipherError> {
    unseal_with(key, DATA_DOMAIN, aad, ciphertext)
}
pub fn key_check(keys: &dyn KeyProvider) -> Result<Ciphertext, CipherError> {
    let (key_id, key) = keys.current()?;
    key_check_with(&key_id, &key)
}
pub fn key_check_with(key_id: &KeyId, key: &SecretKey) -> Result<Ciphertext, CipherError> {
    seal_with(key_id, key, CHECK_DOMAIN, &AssociatedData(&[]), &[])
}
pub fn verify_key_check(keys: &dyn KeyProvider, check: &Ciphertext) -> Result<(), CipherError> {
    verify_key_check_with(&keys.by_id(&check.key_id)?, check)
}
pub fn verify_key_check_with(key: &SecretKey, check: &Ciphertext) -> Result<(), CipherError> {
    let result = unseal_with(key, CHECK_DOMAIN, &AssociatedData(&[]), check)?;
    if result.is_empty() {
        Ok(())
    } else {
        Err(CipherError::Authentication)
    }
}

#[cfg(test)]
mod tests;
