use super::*;
use std::collections::HashSet;

#[test]
fn hmac_state_wipes_on_drop_and_matches_rfc_4231() {
    let _: &dyn zeroize::ZeroizeOnDrop = &Sha256::new();
    let mut key = [0; 32];
    key[..20].fill(0x0b);
    assert_eq!(
        *hmac_sha256(&key, |hash| hash.update(b"Hi There")),
        [
            0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
            0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
            0x2e, 0x32, 0xcf, 0xf7,
        ]
    );
}

#[test]
fn hmac_matches_rfc_4231_cases_two_through_four() {
    let sequential: Vec<u8> = (1..=25).collect();
    for (key, data, expected) in [
        (
            b"Jefe".as_slice(),
            b"what do ya want for nothing?".as_slice(),
            [
                0x5b, 0xdc, 0xc1, 0x46, 0xbf, 0x60, 0x75, 0x4e, 0x6a, 0x04, 0x24, 0x26, 0x08, 0x95,
                0x75, 0xc7, 0x5a, 0x00, 0x3f, 0x08, 0x9d, 0x27, 0x39, 0x83, 0x9d, 0xec, 0x58, 0xb9,
                0x64, 0xec, 0x38, 0x43,
            ],
        ),
        (
            &[0xaa; 20],
            &[0xdd; 50],
            [
                0x77, 0x3e, 0xa9, 0x1e, 0x36, 0x80, 0x0e, 0x46, 0x85, 0x4d, 0xb8, 0xeb, 0xd0, 0x91,
                0x81, 0xa7, 0x29, 0x59, 0x09, 0x8b, 0x3e, 0xf8, 0xc1, 0x22, 0xd9, 0x63, 0x55, 0x14,
                0xce, 0xd5, 0x65, 0xfe,
            ],
        ),
        (
            sequential.as_slice(),
            &[0xcd; 50],
            [
                0x82, 0x55, 0x8a, 0x38, 0x9a, 0x44, 0x3c, 0x0e, 0xa4, 0xcc, 0x81, 0x98, 0x99, 0xf2,
                0x08, 0x3a, 0x85, 0xf0, 0xfa, 0xa3, 0xe5, 0x78, 0xf8, 0x07, 0x7a, 0x2e, 0x3f, 0xf4,
                0x67, 0x29, 0x66, 0x5b,
            ],
        ),
    ] {
        let mut padded = [0; 32];
        padded[..key.len()].copy_from_slice(key);
        assert_eq!(*hmac_sha256(&padded, |hash| hash.update(data)), expected);
    }
}

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
    let check = subkey(&master, CHECK_SUBKEY);
    assert_ne!(*check, *aead);
    assert_ne!(*check, *chain);
    // RFC 5869, appendix A.1: the first 32 bytes of OKM from the published PRK.
    // https://www.rfc-editor.org/rfc/rfc5869#appendix-A.1
    let hex = |text: &str| -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    };
    let prk: [u8; 32] = hex("077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5")
        .try_into()
        .unwrap();
    let okm = subkey(&SecretKey::new(&prk), &hex("f0f1f2f3f4f5f6f7f8f9"));
    assert_eq!(
        okm.to_vec(),
        hex("3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf")
    );
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
    // The key check runs under its own subkey, not the data one.
    let check = key_check(&keys).unwrap();
    let data_key = XChaCha20Poly1305::new((&*subkey(&master, AEAD_SUBKEY)).into());
    let encoded = encode(CHECK_DOMAIN, &keys.id, &AssociatedData(&[]));
    let payload = Payload {
        msg: &check.bytes,
        aad: &encoded,
    };
    assert!(data_key.decrypt((&check.nonce).into(), payload).is_err());
    verify_key_check(&keys, &check).unwrap();
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
