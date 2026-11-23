use super::{
    TestLauncher, authenticated, bounded, call, closed, command, connect, fixture,
    fixture_with_config, open_params, receive, transmit,
};
use ads_daemon::api::{ApiConfig, ApiError, DisabledLauncher};
use ads_daemon::auth::Token;
use ads_daemon::events::EventLog;
use ads_ptyhost::HostConfig;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

#[tokio::test]
async fn desktop_listener_requires_native_upgrade_and_token() {
    let config = ApiConfig::from_toml_str("allowed_origins = ['http://127.0.0.1:5173']")
        .unwrap()
        .for_desktop();
    let f = fixture_with_config(config, Arc::new(DisabledLauncher), HostConfig::defaults()).await;
    let request = format!("ws://127.0.0.1:{}/", f.port)
        .into_client_request()
        .unwrap();
    let nonce = request.headers()["sec-websocket-key"].clone();
    let stream = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    let client = stream.local_addr().unwrap();
    let server = stream.peer_addr().unwrap();
    let (mut socket, response) = bounded(tokio_tungstenite::client_async(request, stream))
        .await
        .unwrap();
    let proof = &response.headers()[ads_contracts::desktop_proof::HEADER];
    assert!(ads_contracts::desktop_proof::verify(
        &f.token,
        nonce.as_bytes(),
        client,
        server,
        proof.as_bytes(),
    ));
    transmit(
        &mut socket,
        &command("auth.hello", json!({"token": &f.token})),
    )
    .await;
    assert!(receive(&mut socket).await.get("result").is_some());
    let mut unauthenticated = connect(&f, None, None).await.unwrap();
    transmit(&mut unauthenticated, &command("harness.list", json!({}))).await;
    closed(&mut unauthenticated).await;
    for wrong in [
        "http://tauri.localhost",
        "http://127.0.0.1:5173",
        "null",
        "",
    ] {
        let error = connect(&f, Some(wrong), None).await.unwrap_err();
        assert!(matches!(
            error,
            tokio_tungstenite::tungstenite::Error::Http(response)
                if response.status().as_u16() == 403
        ));
    }
}

#[tokio::test]
async fn upgrade_rejects_wrong_or_missing_origin_and_host() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    for (origin, host) in [
        (Some("https://untrusted.invalid"), None),
        (None, None),
        (Some("http://127.0.0.1:5173"), Some("untrusted.invalid")),
        (Some("null"), None),
        (Some("http://127.0.0.1:5173.attacker.invalid"), None),
    ] {
        let error = connect(&f, origin, host)
            .await
            .expect_err("upgrade accepted");
        match error {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), 403)
            }
            _ => panic!("expected 403"),
        }
    }
    for origin in ["http://127.0.0.1:5173"] {
        for host in [
            format!("localhost:{}", f.port),
            format!("127.0.0.1:{}", f.port),
        ] {
            let mut ws = connect(&f, Some(origin), Some(&host)).await.unwrap();
            ws.close(None).await.unwrap();
        }
    }
}

#[tokio::test]
async fn authentication_rejects_missing_wrong_late_token_and_command_before_hello() {
    let f = fixture("auth_timeout_ms = 50", Arc::new(TestLauncher)).await;
    for value in [
        command("auth.hello", json!({})),
        command("auth.hello", json!({"token": "A".repeat(43)})),
        command("session.open", open_params("unauthenticated")),
        command("auth.hello", json!({"token": f.token, "extra": true})),
        command("harness.list", json!({})),
    ] {
        let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .unwrap();
        transmit(&mut ws, &value).await;
        closed(&mut ws).await;
    }
    let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    closed(&mut ws).await;
    assert!(f.backend.terminals().is_empty());
}

#[test]
fn credentials_only_appear_in_launch_fragment_and_diagnostics_are_redacted() {
    let token = Token::generate().unwrap();
    let url = token.launch_url("http://127.0.0.1:5173", 12345);
    let (http, fragment) = url.split_once('#').unwrap();
    let secret = fragment.split_once("&token=").unwrap().1;
    assert_eq!(http, "http://127.0.0.1:5173/");
    assert!(!http.contains(secret));
    assert!(token.matches(secret));
    assert!(!token.matches(&"X".repeat(43)));
    assert!(!token.matches(""));
    assert_ne!(
        Token::generate()
            .unwrap()
            .launch_url("http://127.0.0.1:5173", 12345),
        url
    );
    let mut log = EventLog::new(2, 4);
    log.append("secret", "output", json!({"session_id": "secret", "offset": 0, "length": secret.len(), "encoding": "base64", "payload": STANDARD.encode(secret)}), None).unwrap();
    let mut diagnostics = format!("{token:?} {log:?}");
    let event = ads_daemon::pty_client::Event::Output {
        header: ads_daemon::pty_client::OutputHeader {
            session_id: "secret".to_owned().try_into().unwrap(),
            seq: 1.try_into().unwrap(),
            length: (secret.len() as u32).try_into().unwrap(),
        },
        bytes: secret.as_bytes().to_vec(),
    };
    diagnostics.push_str(&format!("{event:?}"));
    for error in [
        ApiError::Invalid,
        ApiError::Method,
        ApiError::Remote,
        ApiError::HarnessNotConfigured,
        ApiError::Capacity,
        ApiError::Conflict,
        ApiError::Replay,
        ApiError::Unavailable,
    ] {
        diagnostics.push_str(&format!("{error:?} {error}"));
    }
    let terminal = "private-terminal-output-sentinel";
    let error = log.append("secret", "output", json!({"session_id": "secret", "offset": 0, "length": 1, "encoding": "base64", "payload": STANDARD.encode(format!("{terminal}{secret}")), "unexpected": secret}), None).unwrap_err();
    diagnostics.push_str(&format!("{error:?} {error}"));
    assert!(!diagnostics.contains(terminal));
    assert!(!diagnostics.contains(&STANDARD.encode(format!("{terminal}{secret}"))));
    assert!(!diagnostics.contains(secret));
    assert!(!diagnostics.contains(&STANDARD.encode(secret)));
    assert!(!diagnostics.contains(&format!("{:?}", secret.as_bytes())));
}

#[test]
fn dependency_logging_cannot_emit_credentials_or_terminal_bytes() {
    assert_eq!(log::STATIC_MAX_LEVEL, log::LevelFilter::Off);
}

#[tokio::test]
async fn connection_and_message_limits_close_excess_clients() {
    let f = fixture(
        "max_connections = 1\nmax_message_bytes = 1024",
        Arc::new(TestLauncher),
    )
    .await;
    let mut ws = authenticated(&f).await;
    let mut excess = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    transmit(
        &mut excess,
        &command("auth.hello", json!({"token": f.token})),
    )
    .await;
    closed(&mut excess).await;
    transmit(
        &mut ws,
        &command("unknown", json!({"padding": "x".repeat(2048)})),
    )
    .await;
    closed(&mut ws).await;
}

#[tokio::test]
async fn missing_duplicate_headers_and_query_credentials_are_rejected_before_upgrade() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let f = fixture("", Arc::new(TestLauncher)).await;
    for (target, headers) in [
        ("/", "Origin: http://127.0.0.1:5173\r\n".to_owned()),
        (
            "/",
            format!(
                "Host: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:5173\r\nOrigin: http://127.0.0.1:5173\r\n",
                f.port
            ),
        ),
        (
            "/",
            format!(
                "Host: 127.0.0.1:{}\r\nHost: localhost:{}\r\nOrigin: http://127.0.0.1:5173\r\n",
                f.port, f.port
            ),
        ),
        (
            "/?token=refused",
            format!(
                "Host: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:5173\r\n",
                f.port
            ),
        ),
    ] {
        let mut stream = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
        let request = format!(
            "GET {target} HTTP/1.1\r\n{headers}Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        bounded(stream.read_to_end(&mut response)).await.unwrap();
        assert!(response.starts_with(b"HTTP/1.1 403"));
    }
}

#[tokio::test]
async fn exact_host_and_origin_variants_are_refused() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    for host in [
        format!("LOCALHOST:{}", f.port),
        format!("localhost.:{}", f.port),
        format!("[::1]:{}", f.port),
        "127.0.0.1".into(),
        "localhost".into(),
        format!("localhost:{}", if f.port == 65535 { 1 } else { f.port + 1 }),
    ] {
        assert!(
            connect(&f, Some("http://127.0.0.1:5173"), Some(&host))
                .await
                .is_err(),
            "accepted {host}"
        );
    }
    for origin in [
        "http://localhost:5173",
        "http://127.0.0.1:5174",
        "HTTP://127.0.0.1:5173",
        "http://127.0.0.1.:5173",
        "http://[::1]:5173",
        "http://127.0.0.1",
        "http://127.0.0.1:5173/",
        "https://127.0.0.1:5173",
        "null",
    ] {
        assert!(
            connect(&f, Some(origin), None).await.is_err(),
            "accepted {origin}"
        );
    }
}

#[tokio::test]
async fn authentication_frames_are_single_attempt_and_ping_works_after_authentication() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    for frame in [
        Message::Ping(vec![1].into()),
        Message::Binary(vec![1].into()),
    ] {
        let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .unwrap();
        ws.send(frame).await.unwrap();
        closed(&mut ws).await;
    }
    for token in [
        "".to_owned(),
        "x".repeat(42),
        "x".repeat(44),
        "x".repeat(43),
    ] {
        let mut ws = connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .unwrap();
        transmit(&mut ws, &command("auth.hello", json!({"token": token}))).await;
        closed(&mut ws).await;
    }
    let mut ws = authenticated(&f).await;
    ws.send(Message::Ping(vec![7, 8].into())).await.unwrap();
    assert_eq!(
        bounded(ws.next()).await.unwrap().unwrap(),
        Message::Pong(vec![7, 8].into())
    );
    transmit(&mut ws, &command("auth.hello", json!({"token": f.token}))).await;
    closed(&mut ws).await;
    let mut ws = authenticated(&f).await;
    ws.send(Message::Binary(vec![1].into())).await.unwrap();
    closed(&mut ws).await;
}

#[tokio::test]
async fn unauthenticated_sockets_have_separate_bounded_slots() {
    let f = fixture(
        "max_connections = 1\nmax_pending_auth = 2\nauth_timeout_ms = 200",
        Arc::new(TestLauncher),
    )
    .await;
    let mut ws = authenticated(&f).await;
    let mut a = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let b = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    // A full pending queue makes room by dropping its oldest entry, never the newcomer.
    let mut newest = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    assert_eof(&mut a).await;
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    newest.close(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(220)).await;
    drop(b);
    let mut available = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    available.close(None).await.unwrap();
}

async fn assert_eof(stream: &mut TcpStream) {
    use tokio::io::AsyncReadExt;
    let mut byte = [0u8; 1];
    assert!(matches!(
        bounded(stream.read(&mut byte)).await,
        Ok(0) | Err(_)
    ));
}

#[tokio::test]
async fn thirty_third_pending_connection_drops_the_oldest() {
    use tokio::io::AsyncReadExt;
    // A long deadline keeps the authentication timeout from closing the oldest socket instead.
    let f = fixture("auth_timeout_ms = 60000", Arc::new(TestLauncher)).await;
    let mut pending = Vec::new();
    for _ in 0..32 {
        pending.push(TcpStream::connect(("127.0.0.1", f.port)).await.unwrap());
        // Spacing keeps the accept order equal to the connect order.
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eof(&mut pending[0]).await;
    let mut byte = [0u8; 1];
    for stream in &mut pending[1..] {
        assert!(
            tokio::time::timeout(Duration::from_millis(5), stream.read(&mut byte))
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn handshake_and_hello_share_one_deadline() {
    use tokio::io::AsyncWriteExt;
    let f = fixture("auth_timeout_ms = 400", Arc::new(TestLauncher)).await;
    let mut stream = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    stream.write_all(b"G").await.unwrap();
    tokio::time::sleep(Duration::from_millis(280)).await;
    // Complete the pending HTTP request with the client codec without writing its G twice.
    let request = format!(
        "ET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:5173\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        f.port
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    use tokio::io::AsyncReadExt;
    let start = tokio::time::Instant::now();
    let mut bytes = Vec::new();
    bounded(stream.read_to_end(&mut bytes)).await.unwrap();
    assert!(bytes.starts_with(b"HTTP/1.1 101"));
    assert!(start.elapsed() < Duration::from_millis(300));
}

#[tokio::test]
async fn upgraded_connections_keep_their_pending_slot() {
    let f = fixture(
        "max_pending_auth = 2\nauth_timeout_ms = 60000",
        Arc::new(TestLauncher),
    )
    .await;
    // The legitimate client finished the upgrade and is about to send auth.hello.
    let mut legit = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    let mut raw = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    // The queue is full: the newcomer evicts the raw socket, never the upgraded connection.
    let newest = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    assert_eof(&mut raw).await;
    assert_eq!(
        call(&mut legit, "auth.hello", json!({"token": f.token})).await["result"]["accepted"],
        true
    );
    drop(newest);
    // With every slot taken by an upgraded connection, the next one is refused.
    let mut first = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    let _second = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    assert!(
        connect(&f, Some("http://127.0.0.1:5173"), None)
            .await
            .is_err()
    );
    assert_eq!(
        call(&mut first, "auth.hello", json!({"token": f.token})).await["result"]["accepted"],
        true
    );
    let mut third = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    third.close(None).await.unwrap();
}

#[tokio::test]
async fn connection_is_not_evicted_after_its_upgrade_response() {
    let f = fixture(
        "max_pending_auth = 1\nauth_timeout_ms = 60000",
        Arc::new(TestLauncher),
    )
    .await;
    // The client already has its 101; the server side is held right after the upgrade.
    f.hooks.after_upgrade.hold();
    let mut legit = connect(&f, Some("http://127.0.0.1:5173"), None)
        .await
        .unwrap();
    let mut raw = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    assert_eof(&mut raw).await;
    f.hooks.after_upgrade.release();
    assert_eq!(
        call(&mut legit, "auth.hello", json!({"token": f.token})).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn sequential_admissions_keep_the_eviction_queue_small() {
    let f = fixture("max_pending_auth = 4", Arc::new(TestLauncher)).await;
    for _ in 0..40 {
        let mut ws = authenticated(&f).await;
        ws.close(None).await.unwrap();
    }
    let (live, peak, waiting) = f.hooks.pending();
    assert!(live <= 1, "{live}");
    assert_eq!(peak, 1);
    assert!(waiting <= 2, "{waiting}");
}
