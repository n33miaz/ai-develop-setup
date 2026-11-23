use super::{
    PeerScenario, TestLauncher, authenticated, call, fixture, fixture_with_host, open_params,
    receive, scripted_fixture,
};
use ads_ptyhost::HostConfig;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn natural_exit_follows_all_output() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("natural")).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/natural", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    f.backend.terminals()[0].emit(b"last");
    f.backend.terminals()[0].exit(7);
    assert_eq!(
        receive(&mut ws).await["type"],
        "urn:ads:event:session.output:1"
    );
    let ended = receive(&mut ws).await;
    assert_eq!(ended["seq"], 3);
    assert_eq!(ended["data"]["exit_code"], 7);
    assert_eq!(ended["data"]["reason"], "exit");
}

#[tokio::test]
async fn session_write_and_replay_limits_are_enforced() {
    let f = fixture(
        "max_sessions = 1\nmax_write_bytes = 4\nevents_per_stream = 2",
        Arc::new(TestLauncher),
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("limited")).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("extra")).await["error"]["code"],
        -32003
    );
    assert_eq!(
        call(
            &mut ws,
            "session.write",
            json!({"session_id": "limited", "data": STANDARD.encode(b"large")})
        )
        .await["error"]["code"],
        -32003
    );
    assert!(f.backend.terminals()[0].input().is_empty());
    for cols in [81, 82, 83] {
        call(
            &mut ws,
            "session.resize",
            json!({"session_id": "limited", "cols": cols, "rows": 24}),
        )
        .await;
    }
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/limited", "after_seq": 1})
        )
        .await["error"]["code"],
        -32004
    );
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/limited", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 3);
    assert_eq!(receive(&mut ws).await["seq"], 4);
    assert_eq!(f.host.sessions().len(), 1);
}

#[tokio::test]
async fn decoded_write_limit_rejects_base64_that_fits_schema_but_exceeds_bytes() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("limit")).await;
    assert_eq!(
        call(
            &mut ws,
            "session.write",
            json!({"session_id": "limit", "data": STANDARD.encode(vec![1; 65536])})
        )
        .await["result"]["accepted"],
        true
    );
    let response = call(
        &mut ws,
        "session.write",
        json!({"session_id": "limit", "data": STANDARD.encode(vec![1; 65538])}),
    )
    .await;
    assert_eq!(response["error"]["code"], -32003);
    assert_eq!(f.backend.terminals()[0].input().len(), 65536);
}

#[tokio::test]
async fn unsafe_open_parameters_and_invalid_sizes_never_reach_the_host() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    for extra in ["program", "args", "cwd", "env"] {
        let mut params = open_params("unsafe");
        params[extra] = json!("refused");
        assert_eq!(
            call(&mut ws, "session.open", params).await["error"]["code"],
            -32602
        );
    }
    for cols in [0, 1001] {
        let mut params = open_params("size");
        params["cols"] = json!(cols);
        assert_eq!(
            call(&mut ws, "session.open", params).await["error"]["code"],
            -32602
        );
    }
    assert!(f.backend.terminals().is_empty());
}

#[tokio::test]
async fn fake_backend_existing_session_and_full_host_do_not_stop_service() {
    let f = fixture_with_host(
        "",
        Arc::new(TestLauncher),
        HostConfig::from_toml_str("max_sessions = 1").unwrap(),
    )
    .await;
    let id = ads_ptyhost::wire::SessionId::new("existing").unwrap();
    f.host
        .open(
            &id,
            &ads_ptyhost::SpawnSpec {
                program: "fake-terminal".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("existing")).await["error"]["code"],
        -32002
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("extra")).await["error"]["code"],
        -32003
    );
    f.host.close(&id).await.unwrap();
    assert_eq!(
        call(&mut ws, "session.open", open_params("new")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn exit_before_close_reply_is_reported_as_requested_termination() {
    let (f, _) = scripted_fixture(
        "",
        PeerScenario::ExitBeforeCloseResponse,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut events = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    call(
        &mut events,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut events).await["seq"], 1);
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "session"})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(receive(&mut events).await["data"]["reason"], "killed");
}
