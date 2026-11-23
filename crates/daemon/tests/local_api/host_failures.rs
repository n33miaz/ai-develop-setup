use super::{
    TestLauncher, authenticated, bounded, call, closed, command, fixture, open_params,
    refusing_fixture, transmit, wait_until,
};
use ads_daemon::api::ApiError;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn host_disconnect_ends_the_api_and_all_authenticated_connections() {
    let mut f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    f.peer.abort();
    assert_eq!(
        bounded(&mut f.api).await.unwrap(),
        Err(ApiError::Unavailable)
    );
    closed(&mut ws).await;
}

#[tokio::test]
async fn every_host_refusal_returns_its_code_and_service_survives() {
    use ads_ptyhost::wire::ErrorCode::*;
    for code in [
        ParseError,
        InvalidRequest,
        MethodNotFound,
        InvalidParams,
        InternalError,
        UnknownSession,
        SessionExists,
        TooManySessions,
        ReplayEvicted,
        ReplayAhead,
        SessionEnded,
        SpawnFailed,
        TerminalIo,
        UnsupportedContract,
    ] {
        for target in ["open", "write", "resize", "close", "subscribe"] {
            let (f, _) = refusing_fixture("", target, code, 0).await;
            let mut ws = authenticated(&f).await;
            let result = call(&mut ws, "session.open", open_params("session")).await;
            let result = match target {
                "write" => call(
                    &mut ws,
                    "session.write",
                    json!({"session_id": "session", "data": STANDARD.encode(f.token.as_bytes())}),
                )
                .await,
                "resize" => {
                    call(
                        &mut ws,
                        "session.resize",
                        json!({"session_id": "session", "cols": 90, "rows": 30}),
                    )
                    .await
                }
                "close" => call(&mut ws, "session.close", json!({"session_id": "session"})).await,
                _ => result,
            };
            // The terminal is already open when the subscription is refused: setup failed, close scheduled.
            let expected = if target == "subscribe" {
                -32014
            } else {
                code.number()
            };
            assert_eq!(result["error"]["code"], expected, "{target}: {code:?}");
            if target == "close" && code != UnknownSession {
                assert_eq!(
                    call(&mut ws, "session.close", json!({"session_id": "session"})).await["result"]
                        ["accepted"],
                    true
                );
            }
            let diagnostics = result.to_string();
            assert!(!diagnostics.contains(&f.token));
            assert!(!diagnostics.contains("credential-and-terminal-sentinel"));
            assert_eq!(
                call(
                    &mut ws,
                    "events.subscribe",
                    json!({"stream": "session/alive", "after_seq": 0})
                )
                .await["result"]["accepted"],
                true
            );
            assert!(!f.api.is_finished());
        }
    }
}

#[test]
fn unavailable_service_and_host_timeout_share_the_internal_error_code() {
    for error in [ApiError::Unavailable, ApiError::HostTimeout] {
        assert_eq!(error.code(), -32603);
    }
}

#[tokio::test]
async fn host_timeout_ends_the_service_after_an_uncertain_command() {
    let (mut f, _) = refusing_fixture(
        "host_timeout_ms = 50",
        "stall",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    transmit(
        &mut ws,
        &command(
            "session.write",
            json!({"session_id": "session", "data": "YQ=="}),
        ),
    )
    .await;
    assert_eq!(
        bounded(&mut f.api).await.unwrap(),
        Err(ApiError::Unavailable)
    );
    closed(&mut ws).await;
}

#[tokio::test]
async fn host_lost_during_setup_answers_unavailable() {
    // Either the channel drops, or the client stops on its own full queue: the service ends.
    for target in ["host-lost-on-subscribe", "flood-on-subscribe"] {
        let (mut f, _) =
            refusing_fixture("", target, ads_ptyhost::wire::ErrorCode::InternalError, 0).await;
        let mut ws = authenticated(&f).await;
        transmit(&mut ws, &command("session.open", open_params("session"))).await;
        // The command itself stops the service, instead of a later turn of the loop.
        assert_eq!(
            bounded(&mut f.api).await.unwrap(),
            Err(ApiError::Unavailable),
            "{target}"
        );
        // The connection may close before the reply goes out; a reply that does says unavailable.
        if let Some(Ok(Message::Text(text))) = bounded(ws.next()).await {
            let reply: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(reply["error"]["code"], -32603, "{target}");
        }
    }
}

#[tokio::test]
async fn full_host_queue_during_setup_keeps_the_service() {
    let (f, calls) = refusing_fixture(
        "",
        "busy-on-open",
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
    )
    .await;
    let mut ws = authenticated(&f).await;
    // The client refuses the subscription itself: setup failed, close scheduled, service alive.
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32014
    );
    wait_until(|| calls.lock().unwrap().len() >= 4).await;
    assert_eq!(
        &*calls.lock().unwrap(),
        &["hello", "open", "subscribe", "subscribe"]
    );
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/alive", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert!(!f.api.is_finished());
}
