use super::{
    TestLauncher, authenticated, call, closed, command, connect, fixture, open_params, receive,
    remote, transmit,
};
use ads_daemon::api::DisabledLauncher;
use ads_daemon::events::new_id;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn remote_and_unknown_methods_are_refused_and_default_launcher_is_disabled() {
    let f = fixture("", Arc::new(DisabledLauncher)).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("disabled")).await["error"]["code"],
        -32011
    );
    for method in ["fs.delete", "fs.write", "unknown"] {
        assert_eq!(
            call(&mut ws, method, json!({})).await["error"]["code"],
            -32601
        );
    }
    let mut remote = command("session.open", open_params("remote"));
    remote["params"]["_meta"]["origin"] = json!("remote");
    remote["params"]["_meta"]["remote"] = json!({"device_id": "other", "signature": STANDARD.encode([0;64]), "nonce": STANDARD.encode([0;16]), "counter": 1, "expires_at": "2030-01-01T00:00:00Z"});
    transmit(&mut ws, &remote).await;
    assert_eq!(receive(&mut ws).await["error"]["code"], -32010);
    assert!(f.backend.terminals().is_empty());
}

#[tokio::test]
async fn idempotency_prevents_duplicate_writes_across_connections_and_conflicting_payloads() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
    let mut write = command(
        "session.write",
        json!({"session_id": "session", "data": STANDARD.encode(b"once")}),
    );
    transmit(&mut ws, &write).await;
    assert_eq!(receive(&mut ws).await["result"]["replayed"], false);
    let mut second = authenticated(&f).await;
    write["id"] = json!(new_id().unwrap());
    transmit(&mut second, &write).await;
    assert_eq!(receive(&mut second).await["result"]["replayed"], true);
    assert_eq!(f.backend.terminals()[0].input(), b"once");
    write["params"]["data"] = json!(STANDARD.encode(b"different"));
    transmit(&mut second, &write).await;
    assert_eq!(receive(&mut second).await["error"]["code"], -32012);
    assert_eq!(f.backend.terminals()[0].input(), b"once");
    call(&mut ws, "session.close", json!({"session_id": "session"})).await;
}

#[tokio::test]
async fn idempotency_cache_evicts_oldest_key() {
    let f = fixture("idempotency_keys = 1", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("cache")).await;
    let a = command(
        "session.write",
        json!({"session_id": "cache", "data": "YQ=="}),
    );
    transmit(&mut ws, &a).await;
    receive(&mut ws).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "cache", "data": "Yg=="}),
    )
    .await;
    transmit(&mut ws, &a).await;
    assert_eq!(receive(&mut ws).await["result"]["replayed"], false);
    assert_eq!(f.backend.terminals()[0].input(), b"aba");
}

#[tokio::test]
async fn remote_commands_never_change_sessions_or_subscriptions() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("local")).await;
    for (method, params) in [
        (
            "session.write",
            json!({"session_id": "local", "data": STANDARD.encode(b"secret-terminal")}),
        ),
        (
            "session.resize",
            json!({"session_id": "local", "cols": 90, "rows": 30}),
        ),
        ("session.close", json!({"session_id": "local"})),
        (
            "events.subscribe",
            json!({"stream": "session/local", "after_seq": 0}),
        ),
        ("harness.list", json!({})),
    ] {
        transmit(&mut ws, &remote(command(method, params))).await;
        assert_eq!(receive(&mut ws).await["error"]["code"], -32010);
    }
    assert!(f.backend.terminals()[0].input().is_empty());
    assert_eq!(f.backend.terminals()[0].size().cols, 80);
    assert!(!f.backend.terminals()[0].was_killed());
    let mut unauth = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    transmit(
        &mut unauth,
        &remote(command("auth.hello", json!({"token": f.token}))),
    )
    .await;
    closed(&mut unauth).await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "local"})).await["result"]["accepted"],
        true
    );
}
