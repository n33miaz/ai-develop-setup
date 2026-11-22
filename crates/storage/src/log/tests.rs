use super::*;
use crate::cipher::{CipherError, SecretKey};

struct TwoKeys(u8);
impl KeyProvider for TwoKeys {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
        Ok((KeyId::new("key-a")?, SecretKey::new(&[self.0; 32])))
    }
    fn by_id(&self, id: &KeyId) -> Result<SecretKey, CipherError> {
        match id.as_str() {
            "key-a" => Ok(SecretKey::new(&[self.0; 32])),
            "key-b" => Ok(SecretKey::new(&[2; 32])),
            _ => Err(CipherError::KeyUnavailable),
        }
    }
}

fn row() -> Row {
    Row {
        position: 7,
        format: ROW_FORMAT,
        id: "01J00000000000000000000007".into(),
        kind: "urn:ads:event:action.started:1".into(),
        ts: "2026-11-19T12:00:00Z".into(),
        stream: "action/a1".into(),
        seq: 3,
        actor_kind: "device".into(),
        actor_id: "phone".into(),
        cause: Some("01J00000000000000000000006".into()),
        data_class: "private".into(),
        action_id: Some("a1".into()),
        approval_id: Some("ap1".into()),
        key_id: "key-a".into(),
        nonce: vec![],
        data_ct: vec![],
        prev_chain: vec![9; 32],
        chain: vec![],
    }
}

fn seal(row: &mut Row, keys: &dyn KeyProvider, data: &[u8]) {
    let id = KeyId::new(&row.key_id).unwrap();
    let key = keys.by_id(&id).unwrap();
    let ct = row
        .associated(|f| cipher::encrypt_with(&id, &key, &AssociatedData(f), data))
        .unwrap();
    row.nonce = ct.nonce.to_vec();
    row.data_ct = ct.bytes;
    let chain_key = ChainKey::derive(&key);
    row.chain = row.chained(|f| chain_key.mac(f)).to_vec();
}

fn insert_authenticated_rows(
    connection: &mut rusqlite::Connection,
    ids: &[&str],
    mut previous: Vec<u8>,
) {
    let keys = TwoKeys(1);
    let tx = connection.transaction().unwrap();
    for id in ["key-a", "key-b"] {
        let id = KeyId::new(id).unwrap();
        register(&tx, &id, &keys.by_id(&id).unwrap(), None).unwrap();
    }
    for (position, id) in (1_i64..).zip(ids) {
        let mut row = row();
        row.position = position;
        row.seq = position;
        row.id = format!("01J{position:023}");
        row.key_id = (*id).to_owned();
        row.prev_chain = previous;
        seal(
            &mut row,
            &keys,
            br#"{"action_id":"a1","approval_id":"ap1","plan_hash":"x"}"#,
        );
        tx.execute(
            &format!(
                "INSERT INTO event ({ROW_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"
            ),
            params![
                row.position,
                row.format,
                row.id,
                row.kind,
                row.ts,
                row.stream,
                row.seq,
                row.actor_kind,
                row.actor_id,
                row.cause,
                row.data_class,
                row.action_id,
                row.approval_id,
                row.key_id,
                row.nonce,
                row.data_ct,
                row.prev_chain,
                row.chain,
            ],
        )
        .unwrap();
        previous = row.chain;
    }
    tx.commit().unwrap();
}

#[test]
fn verify_chain_refuses_authenticated_return_to_an_older_key() {
    let path = crate::open::tests::test_directory().join("audit.db");
    let config = StorageConfig::defaults();
    drop(AuditStore::open(&path, &config).unwrap());
    let mut connection = rusqlite::Connection::open(&path).unwrap();
    let link = genesis(&connection).unwrap();
    insert_authenticated_rows(&mut connection, &["key-a", "key-a", "key-b", "key-a"], link);
    drop(connection);

    let log = EventLog::open_unanchored(&path, &config, Box::new(TwoKeys(1))).unwrap();
    assert_eq!(
        log.verify_chain(),
        Err(StorageError::ChainBroken { position: 4 })
    );
}

#[test]
fn migration_refuses_version_three_key_reuse_without_changing_the_log() {
    let path = crate::open::tests::test_directory().join("audit.db");
    let config = StorageConfig::defaults();
    drop(AuditStore::open(&path, &config).unwrap());
    let mut connection = rusqlite::Connection::open(&path).unwrap();
    let old_schema = crate::migrate::expected_schema(3).unwrap();
    let old_trigger = old_schema
        .iter()
        .find(|(_, name, _, _)| name == "event_append_order")
        .unwrap()
        .3
        .as_deref()
        .unwrap();
    connection
        .execute_batch(crate::migrate::DROP_QUEUE_BINDING_SQL)
        .unwrap();
    connection
        .execute_batch(crate::migrate::DROP_GATE_JOURNAL_SQL)
        .unwrap();
    connection
        .execute_batch("DROP TRIGGER event_append_order; DROP TABLE log_genesis;")
        .unwrap();
    connection.execute_batch(old_trigger).unwrap();
    connection.pragma_update(None, "user_version", 3).unwrap();
    insert_authenticated_rows(&mut connection, &["key-a", "key-b", "key-a"], vec![0; 32]);
    drop(connection);
    let before = std::fs::read(&path).unwrap();

    assert_eq!(
        AuditStore::open(&path, &config).unwrap_err(),
        StorageError::LegacyKeyReuse
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

fn copy(r: &Row) -> Row {
    Row {
        nonce: r.nonce.clone(),
        data_ct: r.data_ct.clone(),
        chain: r.chain.clone(),
        ..row()
    }
}

fn open(r: &Row) -> Result<Zeroizing<Vec<u8>>, StorageError> {
    let mut r = copy_all(r);
    r.known_format()?;
    let ct = r.take_ciphertext()?;
    Ok(r.associated(|f| cipher::decrypt(&TwoKeys(1), &AssociatedData(f), &ct))?)
}

fn copy_all(r: &Row) -> Row {
    Row {
        position: r.position,
        format: r.format,
        id: r.id.clone(),
        kind: r.kind.clone(),
        ts: r.ts.clone(),
        stream: r.stream.clone(),
        seq: r.seq,
        actor_kind: r.actor_kind.clone(),
        actor_id: r.actor_id.clone(),
        cause: r.cause.clone(),
        data_class: r.data_class.clone(),
        action_id: r.action_id.clone(),
        approval_id: r.approval_id.clone(),
        key_id: r.key_id.clone(),
        nonce: r.nonce.clone(),
        data_ct: r.data_ct.clone(),
        prev_chain: r.prev_chain.clone(),
        chain: r.chain.clone(),
    }
}

fn chain_ok(r: &Row) -> bool {
    let key = cipher::chain_key(&TwoKeys(1), &KeyId::new("key-a").unwrap()).unwrap();
    r.chained(|f| key.verify(f, &r.chain))
}

#[test]
fn every_clear_column_is_authenticated_and_chained() {
    let mut sealed = row();
    seal(&mut sealed, &TwoKeys(1), b"{}");
    assert_eq!(&**open(&sealed).unwrap(), b"{}");
    assert!(chain_ok(&sealed));
    let changes: Vec<fn(&mut Row)> = vec![
        |r| r.position += 1,
        |r| r.id = "01J00000000000000000000008".into(),
        |r| r.kind = "urn:ads:event:action.executed:1".into(),
        |r| r.ts = "2026-11-19T12:00:01Z".into(),
        |r| r.stream = "action/a2".into(),
        |r| r.seq += 1,
        |r| r.actor_kind = "agent".into(),
        |r| r.actor_id = "laptop".into(),
        |r| r.cause = None,
        |r| r.cause = Some(String::new()),
        |r| r.data_class = "public".into(),
        |r| r.action_id = Some("a2".into()),
        |r| r.action_id = None,
        |r| r.approval_id = Some("ap2".into()),
        |r| r.approval_id = None,
        |r| r.prev_chain = vec![8; 32],
        |r| r.key_id = "key-b".into(),
        |r| r.nonce[0] ^= 1,
        |r| r.data_ct[0] ^= 1,
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut altered = copy(&sealed);
        change(&mut altered);
        assert_eq!(
            open(&altered).unwrap_err(),
            StorageError::Tampered,
            "{index}"
        );
        assert!(!chain_ok(&altered), "{index}");
    }
    // An unknown format is refused before decrypting, and breaks the chain.
    let mut other = copy(&sealed);
    other.format = 2;
    assert_eq!(open(&other).unwrap_err(), StorageError::UnsupportedSchema);
    assert!(!chain_ok(&other));
    // The format is authenticated too, not only checked before decrypting.
    let ct = copy(&sealed).take_ciphertext().unwrap();
    let decrypted = other.associated(|f| cipher::decrypt(&TwoKeys(1), &AssociatedData(f), &ct));
    assert_eq!(decrypted.unwrap_err(), CipherError::Authentication);
}

#[test]
fn chain_value_depends_on_the_key_not_only_on_the_row() {
    let mut first = row();
    seal(&mut first, &TwoKeys(1), b"{}");
    let mut second = copy_all(&first);
    let key = cipher::chain_key(&TwoKeys(5), &KeyId::new("key-a").unwrap()).unwrap();
    second.chain = second.chained(|f| key.mac(f)).to_vec();
    // Same columns, same key id, other key bytes: a writer without the key
    // cannot compute the link.
    assert_ne!(first.chain, second.chain);
    assert!(!chain_ok(&second));
}

#[test]
fn a_queue_row_naming_another_queue_is_refused_on_read() {
    let queue_row = || Row {
        kind: "urn:ads:event:queue.created:1".into(),
        stream: "queue/q1".into(),
        action_id: None,
        approval_id: None,
        ..row()
    };
    let mut good = queue_row();
    seal(&mut good, &TwoKeys(1), br#"{"queue_id":"q1"}"#);
    good.envelope(&open(&good).unwrap()).unwrap();
    // Sealed by a key holder: the cipher and the chain pass, the binding does not.
    for data in [&br#"{"queue_id":"q2"}"#[..], br#"{}"#, br#"{"queue_id":1}"#] {
        let mut liar = queue_row();
        seal(&mut liar, &TwoKeys(1), data);
        assert!(chain_ok(&liar));
        let plain = open(&liar).unwrap();
        assert_eq!(liar.envelope(&plain).unwrap_err(), StorageError::Tampered);
    }
}

#[test]
fn clear_ids_must_match_the_decrypted_data() {
    let data = br#"{"action_id":"a1","approval_id":"ap1","plan_hash":"x"}"#;
    let mut good = row();
    seal(&mut good, &TwoKeys(1), data);
    let plain = open(&good).unwrap();
    good.envelope(&plain).unwrap();
    // Sealed by a key holder with clear ids its data does not name.
    let mut liar = row();
    liar.approval_id = None;
    seal(&mut liar, &TwoKeys(1), data);
    let plain = open(&liar).unwrap();
    assert_eq!(liar.envelope(&plain).unwrap_err(), StorageError::Tampered);
}

#[test]
fn gate_ids_come_from_data_and_match_the_stream() {
    let data = |v: serde_json::Value| match v {
        Value::Object(map) => map,
        _ => unreachable!(),
    };
    let planned = "urn:ads:event:action.planned:1";
    let started = "urn:ads:event:action.started:1";
    let granted = "urn:ads:event:approval.granted:2";
    let granted_v1 = "urn:ads:event:approval.granted:1";
    let noted = "urn:ads:event:task.noted:1";
    let ok = |kind, stream, v, a: Option<&str>, b: Option<&str>| {
        assert_eq!(
            gate_ids(kind, stream, &data(v)),
            Ok((a.map(String::from), b.map(String::from))),
            "{kind} {stream}"
        )
    };
    ok(
        planned,
        "action/a1",
        serde_json::json!({ "action_id": "a1" }),
        Some("a1"),
        None,
    );
    ok(
        started,
        "action/a1",
        serde_json::json!({ "action_id": "a1", "approval_id": null }),
        Some("a1"),
        None,
    );
    ok(
        started,
        "action/a1",
        serde_json::json!({ "action_id": "a1", "approval_id": "ap1" }),
        Some("a1"),
        Some("ap1"),
    );
    ok(
        granted,
        "approval/ap1",
        serde_json::json!({ "action_id": "a1", "approval_id": "ap1" }),
        Some("a1"),
        Some("ap1"),
    );
    ok(
        granted_v1,
        "approval/ap1",
        serde_json::json!({ "approval_id": "ap1" }),
        None,
        Some("ap1"),
    );
    ok(
        noted,
        "task/t1",
        serde_json::json!({ "action_id": "a1" }),
        None,
        None,
    );
    for (kind, stream, v) in [
        (
            planned,
            "action/a1",
            serde_json::json!({ "action_id": "a2" }),
        ),
        (planned, "action/a1", serde_json::json!({})),
        (planned, "action/a1", serde_json::json!({ "action_id": 1 })),
        (
            planned,
            "action/a1",
            serde_json::json!({ "action_id": "a b" }),
        ),
        (
            started,
            "action/a1",
            serde_json::json!({ "action_id": "a1", "approval_id": 7 }),
        ),
        (planned, "task/a1", serde_json::json!({ "action_id": "a1" })),
        (noted, "action/a1", serde_json::json!({ "action_id": "a1" })),
        (
            granted,
            "approval/ap1",
            serde_json::json!({ "approval_id": "ap2" }),
        ),
        (
            granted,
            "action/ap1",
            serde_json::json!({ "action_id": "ap1" }),
        ),
        (noted, "noslash", serde_json::json!({})),
    ] {
        assert_eq!(
            gate_ids(kind, stream, &data(v)),
            Err(StorageError::InvalidEvent),
            "{kind} {stream}"
        );
    }
}
