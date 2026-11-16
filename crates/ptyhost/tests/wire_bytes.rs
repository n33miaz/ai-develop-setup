//! The exact bytes the host puts on the channel and the exact refusal each malformed message
//! gets. Everything here goes in and out as raw bytes, so the message types can change
//! underneath without this file changing: if it has to be edited, the wire format moved.

use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Duration;

use ads_ptyhost::wire::{self, HostMessage, Request};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

const STEP_TIMEOUT: Duration = Duration::from_secs(30);

struct Raw {
    stream: DuplexStream,
}

impl Raw {
    fn attach(host: &Arc<PtyHost>) -> Self {
        let (near, far) = tokio::io::duplex(64 * 1024);
        tokio::spawn(server::connection(Arc::clone(host), far));
        Self { stream: near }
    }

    async fn send(&mut self, json: &str) {
        let mut frame = vec![0x01];
        frame.extend_from_slice(&(json.len() as u32).to_be_bytes());
        frame.extend_from_slice(json.as_bytes());
        self.stream.write_all(&frame).await.unwrap();
    }

    /// The next frame as it came: kind and payload.
    async fn frame(&mut self) -> (u8, Vec<u8>) {
        let read = async {
            let mut prefix = [0_u8; 5];
            self.stream.read_exact(&mut prefix).await.unwrap();
            let len = u32::from_be_bytes(prefix[1..].try_into().unwrap()) as usize;
            let mut payload = vec![0_u8; len];
            self.stream.read_exact(&mut payload).await.unwrap();
            (prefix[0], payload)
        };
        tokio::time::timeout(STEP_TIMEOUT, read)
            .await
            .expect("no frame within the step timeout")
    }

    /// The next frame, which has to be a JSON one, as text.
    async fn json(&mut self) -> String {
        let (kind, payload) = self.frame().await;
        assert_eq!(kind, 0x01, "expected a JSON frame");
        String::from_utf8(payload).unwrap()
    }

    async fn ask(&mut self, json: &str) -> String {
        self.send(json).await;
        self.json().await
    }
}

fn host_with(backend: &Arc<FakeBackend>, overrides: &str) -> Arc<PtyHost> {
    let config = HostConfig::from_toml_str(overrides).unwrap();
    Arc::new(PtyHost::new(Arc::clone(backend) as _, config).unwrap())
}

const HELLO: &str =
    r#"{"jsonrpc":"2.0","id":1,"method":"host.hello","params":{"contract":"pty-host/1"}}"#;

fn open(id: u64, session: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"session.open","params":{{"session_id":"{session}","program":"fake-shell","args":["--flag"],"cwd":null,"env":{{"LANG":"C"}},"cols":120,"rows":40}}}}"#
    )
}

#[tokio::test]
async fn responses_have_exactly_these_bytes() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "max_sessions = 1");
    let mut raw = Raw::attach(&host);

    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":7,"method":"session.list","params":{}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32600,"message":"host.hello must be the first request of a connection"}}"#
    );
    assert_eq!(
        raw.ask(
            r#"{"jsonrpc":"2.0","id":8,"method":"host.hello","params":{"contract":"pty-host/2"}}"#
        )
        .await,
        r#"{"jsonrpc":"2.0","id":8,"error":{"code":-32009,"message":"this host speaks pty-host/1 only"}}"#
    );
    // A refused hello does not open the door.
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":9,"method":"session.list","params":{}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":9,"error":{"code":-32600,"message":"host.hello must be the first request of a connection"}}"#
    );
    assert_eq!(
        raw.ask(HELLO).await,
        r#"{"jsonrpc":"2.0","id":1,"result":{"contract":"pty-host/1"}}"#
    );
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":2,"method":"session.list","params":{}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":2,"result":{"sessions":[]}}"#
    );
    assert_eq!(
        raw.ask(&open(3, "sess-a")).await,
        r#"{"jsonrpc":"2.0","id":3,"result":{}}"#
    );
    assert_eq!(
        raw.ask(&open(4, "sess-a")).await,
        r#"{"jsonrpc":"2.0","id":4,"error":{"code":-32002,"message":"session sess-a already exists"}}"#
    );
    assert_eq!(
        raw.ask(&open(5, "sess-b")).await,
        r#"{"jsonrpc":"2.0","id":5,"error":{"code":-32003,"message":"session limit reached (1)","data":{"limit":1}}}"#
    );
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":6,"method":"session.write","params":{"session_id":"sess-a","data":"ZGlyDQ=="}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":6,"result":{}}"#
    );
    assert_eq!(backend.terminals()[0].input(), b"dir\r");
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":10,"method":"session.resize","params":{"session_id":"sess-a","cols":80,"rows":24}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":10,"result":{}}"#
    );
    assert_eq!(
        raw.ask(
            r#"{"jsonrpc":"2.0","id":11,"method":"session.close","params":{"session_id":"nobody"}}"#
        )
        .await,
        r#"{"jsonrpc":"2.0","id":11,"error":{"code":-32001,"message":"unknown session nobody"}}"#
    );
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":12,"method":"session.list","params":{}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":12,"result":{"sessions":[{"session_id":"sess-a","running":true,"exit_code":null,"oldest_seq":1,"next_seq":1}]}}"#
    );
    assert_eq!(
        raw.ask(
            r#"{"jsonrpc":"2.0","id":13,"method":"session.close","params":{"session_id":"sess-a"}}"#
        )
        .await,
        r#"{"jsonrpc":"2.0","id":13,"result":{"exit_code":1}}"#
    );
}

#[tokio::test]
async fn output_notifications_and_replay_refusals_have_exactly_these_bytes() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(
        &backend,
        "replay_capacity_bytes = 16\nread_buffer_bytes = 8",
    );
    let mut raw = Raw::attach(&host);
    raw.ask(HELLO).await;
    raw.ask(&open(2, "sess-a")).await;
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":3,"method":"session.subscribe","params":{"session_id":"sess-a","from_seq":null}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":3,"result":{"oldest_seq":1,"next_seq":1}}"#
    );

    let terminal = backend.terminals()[0].clone();
    terminal.emit(b"chunk000");
    let (kind, payload) = raw.frame().await;
    let header = br#"{"session_id":"sess-a","seq":1,"length":8}"#;
    assert_eq!(kind, 0x02);
    assert_eq!(
        payload,
        [
            &(header.len() as u16).to_be_bytes()[..],
            header,
            b"chunk000"
        ]
        .concat()
    );

    for n in 1..10 {
        terminal.emit(format!("chunk{n:03}").as_bytes());
        assert_eq!(raw.frame().await.0, 0x02);
    }
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":4,"method":"session.subscribe","params":{"session_id":"sess-a","from_seq":3}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":4,"error":{"code":-32004,"message":"seq 3 left the replay ring; oldest available is 9","data":{"oldest_seq":9}}}"#
    );
    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":5,"method":"session.subscribe","params":{"session_id":"sess-a","from_seq":50}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":5,"error":{"code":-32005,"message":"seq 50 is ahead of the next seq 11","data":{"next_seq":11}}}"#
    );

    terminal.exit(7);
    assert_eq!(
        raw.json().await,
        r#"{"jsonrpc":"2.0","method":"session.exited","params":{"exit_code":7,"last_seq":10,"session_id":"sess-a"}}"#
    );

    // A second session, closed while this client follows it: the answer and the notice may
    // come in either order.
    raw.ask(&open(6, "sess-b")).await;
    raw.ask(r#"{"jsonrpc":"2.0","id":7,"method":"session.subscribe","params":{"session_id":"sess-b","from_seq":null}}"#)
        .await;
    raw.send(
        r#"{"jsonrpc":"2.0","id":8,"method":"session.close","params":{"session_id":"sess-b"}}"#,
    )
    .await;
    let mut both = [raw.json().await, raw.json().await];
    both.sort();
    assert_eq!(
        both,
        [
            r#"{"jsonrpc":"2.0","id":8,"result":{"exit_code":1}}"#,
            r#"{"jsonrpc":"2.0","method":"subscription.closed","params":{"last_seq":0,"reason":"session_closed","session_id":"sess-b"}}"#,
        ]
    );
}

#[tokio::test]
async fn slow_subscriber_notice_has_exactly_these_bytes() {
    let backend = Arc::new(FakeBackend::new());
    let host = host_with(&backend, "subscriber_queue_chunks = 2");
    let (near, far) = tokio::io::duplex(4096);
    tokio::spawn(server::connection(Arc::clone(&host), far));
    let mut raw = Raw { stream: near };
    raw.ask(HELLO).await;
    raw.ask(&open(2, "sess-a")).await;
    raw.ask(r#"{"jsonrpc":"2.0","id":3,"method":"session.subscribe","params":{"session_id":"sess-a","from_seq":null}}"#)
        .await;
    let terminal = backend.terminals()[0].clone();
    for _ in 0..200 {
        terminal.emit(&[b'a'; 1000]);
    }
    let mut observer = Raw::attach(&host);
    observer.ask(HELLO).await;
    loop {
        let list = observer
            .ask(r#"{"jsonrpc":"2.0","id":2,"method":"session.list","params":{}}"#)
            .await;
        if list.contains(r#""next_seq":201"#) {
            break;
        }
        tokio::task::yield_now().await;
    }

    let mut last_seq = 0_u64;
    let notice = loop {
        let (kind, payload) = raw.frame().await;
        if kind == 0x01 {
            break String::from_utf8(payload).unwrap();
        }
        last_seq += 1;
    };
    assert!(last_seq < 200, "the slow client was never dropped");
    assert_eq!(
        notice,
        format!(
            r#"{{"jsonrpc":"2.0","method":"subscription.closed","params":{{"last_seq":{last_seq},"reason":"slow_subscriber","session_id":"sess-a"}}}}"#
        )
    );
}

#[tokio::test]
async fn refusals_that_never_reach_a_session_have_exactly_these_bytes() {
    let host = host_with(&Arc::new(FakeBackend::new()), "");
    let mut raw = Raw::attach(&host);
    raw.ask(HELLO).await;

    assert_eq!(
        raw.ask(r#"{"jsonrpc":"2.0","id":7,"method":"session.kill","params":{}}"#)
            .await,
        r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32601,"message":"unknown method \"session.kill\""}}"#
    );
    // What the text of these says is not pinned, only that the answer has this shape.
    for (request, code, id) in [
        ("{not json", -32700, Value::Null),
        (
            r#"{"jsonrpc":"2.0","id":0,"method":"session.list","params":{}}"#,
            -32600,
            Value::Null,
        ),
        (
            r#"{"jsonrpc":"2.0","id":8,"method":"session.close","params":{"session_id":"a b"}}"#,
            -32602,
            Value::from(8),
        ),
        (
            r#"{"jsonrpc":"2.0","id":9,"method":"host.hello","params":{"contract":"pty-host/9"}}"#,
            -32009,
            Value::from(9),
        ),
    ] {
        let answer: Value = serde_json::from_str(&raw.ask(request).await).unwrap();
        let object = answer.as_object().unwrap();
        assert_eq!(
            object.keys().collect::<Vec<_>>(),
            ["error", "id", "jsonrpc"],
            "{request}"
        );
        assert_eq!(answer["jsonrpc"], "2.0", "{request}");
        assert_eq!(answer["id"], id, "{request}");
        assert_eq!(answer["error"]["code"], code, "{request}");
        assert!(answer["error"].get("data").is_none(), "{request}");
        let message = answer["error"]["message"].as_str().unwrap();
        assert!((1..=500).contains(&message.chars().count()), "{request}");
    }
}

/// `(payload, error code, request number that could be read)`.
fn refused_requests() -> Vec<(String, i64, Option<u64>)> {
    let request = |id: &str, method: &str, params: &str| {
        format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#)
    };
    let open = |change: &dyn Fn(&mut Value)| {
        let mut params = serde_json::json!({
            "session_id": "sess-a", "program": "sh", "args": [], "cwd": null,
            "env": {}, "cols": 80, "rows": 24
        });
        change(&mut params);
        request("5", "session.open", &params.to_string())
    };
    let mut cases: Vec<(String, i64, Option<u64>)> = vec![
        // Not JSON at all.
        ("{nope".to_owned(), -32700, None),
        (String::new(), -32700, None),
        // The envelope.
        ("[]".to_owned(), -32600, None),
        ("null".to_owned(), -32600, None),
        (
            r#"{"jsonrpc":"1.0","id":4,"method":"session.list","params":{}}"#.to_owned(),
            -32600,
            Some(4),
        ),
        (
            r#"{"id":4,"method":"session.list","params":{}}"#.to_owned(),
            -32600,
            Some(4),
        ),
        (
            r#"{"jsonrpc":"2.0","method":"session.list","params":{}}"#.to_owned(),
            -32600,
            None,
        ),
        (request("0", "session.list", "{}"), -32600, None),
        (request("-1", "session.list", "{}"), -32600, None),
        (request("1.5", "session.list", "{}"), -32600, None),
        (request("null", "session.list", "{}"), -32600, None),
        (request(r#""7""#, "session.list", "{}"), -32600, None),
        (
            r#"{"jsonrpc":"2.0","id":4,"method":"session.list"}"#.to_owned(),
            -32600,
            Some(4),
        ),
        (
            r#"{"jsonrpc":"2.0","id":4,"params":{}}"#.to_owned(),
            -32600,
            Some(4),
        ),
        (
            r#"{"jsonrpc":"2.0","id":4,"method":7,"params":{}}"#.to_owned(),
            -32600,
            Some(4),
        ),
        (
            r#"{"jsonrpc":"2.0","id":4,"method":"session.list","params":{},"_meta":{}}"#.to_owned(),
            -32600,
            Some(4),
        ),
        // The method.
        (request("5", "session.kill", "{}"), -32601, Some(5)),
        (request("5", "", "{}"), -32601, Some(5)),
        (request("5", "HOST.HELLO", "{}"), -32601, Some(5)),
        // The params of each method.
        (request("5", "session.list", "null"), -32602, Some(5)),
        (request("5", "session.list", r#"{"x":1}"#), -32602, Some(5)),
        (request("5", "host.hello", "{}"), -32602, Some(5)),
        (
            request("5", "host.hello", r#"{"contract":1}"#),
            -32602,
            Some(5),
        ),
        (
            request("5", "host.hello", r#"{"contract":null}"#),
            -32602,
            Some(5),
        ),
        (
            request("5", "host.hello", r#"{"contract":"pty-host/1","x":1}"#),
            -32602,
            Some(5),
        ),
        (
            request("5", "host.hello", r#"{"contract":"pty-host/2","x":1}"#),
            -32602,
            Some(5),
        ),
        (request("5", "session.close", "{}"), -32602, Some(5)),
        (
            request("5", "session.close", r#"{"session_id":"a b"}"#),
            -32602,
            Some(5),
        ),
        (
            request("5", "session.close", r#"{"session_id":""}"#),
            -32602,
            Some(5),
        ),
        (
            request("5", "session.close", r#"{"session_id":"-a"}"#),
            -32602,
            Some(5),
        ),
        (
            request(
                "5",
                "session.close",
                &format!(r#"{{"session_id":"{}"}}"#, "x".repeat(65)),
            ),
            -32602,
            Some(5),
        ),
        (
            request("5", "session.close", r#"{"session_id":"a","x":1}"#),
            -32602,
            Some(5),
        ),
        (
            request("5", "session.subscribe", r#"{"session_id":"a"}"#),
            -32602,
            Some(5),
        ),
        (
            request(
                "5",
                "session.subscribe",
                r#"{"session_id":"a","from_seq":0}"#,
            ),
            -32602,
            Some(5),
        ),
        (
            request(
                "5",
                "session.subscribe",
                r#"{"session_id":"a","from_seq":-1}"#,
            ),
            -32602,
            Some(5),
        ),
        (
            request(
                "5",
                "session.subscribe",
                r#"{"session_id":"a","from_seq":"1"}"#,
            ),
            -32602,
            Some(5),
        ),
        (
            request("5", "session.resize", r#"{"session_id":"a","cols":80}"#),
            -32602,
            Some(5),
        ),
        (
            request(
                "5",
                "session.resize",
                r#"{"session_id":"a","cols":0,"rows":24}"#,
            ),
            -32602,
            Some(5),
        ),
        (
            request(
                "5",
                "session.resize",
                r#"{"session_id":"a","cols":80,"rows":1001}"#,
            ),
            -32602,
            Some(5),
        ),
        (
            request(
                "5",
                "session.resize",
                r#"{"session_id":"a","cols":80.5,"rows":24}"#,
            ),
            -32602,
            Some(5),
        ),
        (
            request("5", "session.write", r#"{"session_id":"a"}"#),
            -32602,
            Some(5),
        ),
        (
            request("5", "session.write", r#"{"session_id":"a","data":7}"#),
            -32602,
            Some(5),
        ),
    ];
    for loose in [
        "Zg", "Zg=", "Z===", "Zm9v\\n", "Zg==Zg==", "Zm 9", "=Zg=", "Zm9-",
    ] {
        cases.push((
            request(
                "5",
                "session.write",
                &format!(r#"{{"session_id":"a","data":"{loose}"}}"#),
            ),
            -32602,
            Some(5),
        ));
    }
    let changes: [&dyn Fn(&mut Value); 16] = [
        &|p| p["cols"] = 0.into(),
        &|p| p["cols"] = 1001.into(),
        &|p| p["rows"] = 0.into(),
        &|p| p["rows"] = 1001.into(),
        &|p| p["rows"] = 70000.into(),
        &|p| p["program"] = "".into(),
        &|p| p["program"] = "x".repeat(1025).into(),
        &|p| p["cwd"] = "".into(),
        &|p| p["cwd"] = "x".repeat(1025).into(),
        &|p| p["args"] = serde_json::json!(["x".repeat(8193)]),
        &|p| p["args"] = serde_json::json!([1]),
        &|p| p["env"] = serde_json::json!({"LANG": 1}),
        &|p| p["_meta"] = serde_json::json!({}),
        &|p| {
            p.as_object_mut().unwrap().remove("cwd");
        },
        &|p| {
            p.as_object_mut().unwrap().remove("program");
        },
        &|p| p["session_id"] = "a@b".into(),
    ];
    for change in changes {
        cases.push((open(change), -32602, Some(5)));
    }
    cases
}

#[test]
fn every_malformed_request_gets_its_error_code_and_request_number() {
    for (payload, code, id) in refused_requests() {
        let rejection = match Request::parse(payload.as_bytes()) {
            Ok(_) => panic!("accepted: {payload}"),
            Err(rejection) => rejection,
        };
        assert_eq!(rejection.code.number(), code, "{payload}");
        assert_eq!(rejection.id, id.and_then(NonZeroU64::new), "{payload}");
        let length = rejection.message.chars().count();
        assert!(length >= 1, "{payload}");
    }
}

#[test]
fn requests_at_the_edge_of_each_limit_are_accepted() {
    let open = |params: Value| {
        format!(r#"{{"jsonrpc":"2.0","id":5,"method":"session.open","params":{params}}}"#)
    };
    let accepted = [
        open(serde_json::json!({
            "session_id": "x".repeat(64), "program": "x".repeat(1024),
            "args": ["x".repeat(8192), ""], "cwd": "x".repeat(1024),
            "env": {"": ""}, "cols": 1000, "rows": 1
        })),
        open(serde_json::json!({
            "session_id": "A.b_c:d-9", "program": "é", "args": [], "cwd": null,
            "env": {}, "cols": 1, "rows": 1000
        })),
        r#"{"jsonrpc":"2.0","id":18446744073709551615,"method":"session.list","params":{}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","id":5,"method":"session.write","params":{"session_id":"a","data":""}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","id":5,"method":"session.write","params":{"session_id":"a","data":"Zm9vYg=="}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","id":5,"method":"session.subscribe","params":{"session_id":"a","from_seq":1}}"#
            .to_owned(),
        // The version is negotiated by the host, which answers it with its own error code.
        r#"{"jsonrpc":"2.0","id":5,"method":"host.hello","params":{"contract":"pty-host/2"}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","id":5,"method":"host.hello","params":{"contract":""}}"#.to_owned(),
    ];
    for payload in accepted {
        assert!(Request::parse(payload.as_bytes()).is_ok(), "{payload}");
    }
}

#[test]
fn the_fourteen_error_codes_keep_their_numbers() {
    let numbers = [
        -32700, -32600, -32601, -32602, -32603, -32001, -32002, -32003, -32004, -32005, -32006,
        -32007, -32008, -32009,
    ];
    for number in numbers {
        let payload = format!(
            r#"{{"jsonrpc":"2.0","id":1,"error":{{"code":{number},"message":"m","data":{{"oldest_seq":1,"next_seq":2}}}}}}"#
        );
        let Ok(HostMessage::Response(response)) = HostMessage::parse(payload.as_bytes()) else {
            panic!("refused: {payload}");
        };
        assert_eq!(response.outcome.unwrap_err().code.number(), number);
    }
    for number in [-32010, -32000, -32604, 0, 1, -32699] {
        let payload =
            format!(r#"{{"jsonrpc":"2.0","id":1,"error":{{"code":{number},"message":"m"}}}}"#);
        assert!(HostMessage::parse(payload.as_bytes()).is_err(), "{number}");
    }
}

#[test]
fn host_messages_outside_the_contract_are_refused_on_the_way_in() {
    let long = "x".repeat(501);
    let refused = [
        r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{"code":-32603,"message":"m"}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"result":null}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"result":{},"error":null}"#.to_owned(),
        r#"{"jsonrpc":"2.0","result":{}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":0,"result":{}}"#.to_owned(),
        r#"{"jsonrpc":"1.0","id":1,"result":{}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"contract":"pty-host/1","x":1}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"oldest_seq":0,"next_seq":1}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"exit_code":"0"}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32004,"message":"m"}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32004,"message":"m","data":{"next_seq":1}}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32005,"message":"m","data":{"oldest_seq":1}}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":""}}"#.to_owned(),
        format!(r#"{{"jsonrpc":"2.0","id":1,"error":{{"code":-32603,"message":"{long}"}}}}"#),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"m","data":null}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"m","data":{"limit":0}}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","id":1,"method":"session.exited","params":{"session_id":"a","exit_code":0,"last_seq":1}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","method":"session.exited","params":{"session_id":"a","last_seq":1}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","method":"session.exited","params":{"session_id":"a","exit_code":0,"last_seq":-1}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","method":"subscription.closed","params":{"session_id":"a","reason":"bored","last_seq":1}}"#
            .to_owned(),
        r#"{"jsonrpc":"2.0","method":"session.started","params":{}}"#.to_owned(),
        r#"{"jsonrpc":"1.0","method":"session.exited","params":{"session_id":"a","exit_code":0,"last_seq":1}}"#
            .to_owned(),
    ];
    for payload in refused {
        assert!(HostMessage::parse(payload.as_bytes()).is_err(), "{payload}");
    }
    let accepted = [
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"m"}}"#,
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32004,"message":"m","data":{"oldest_seq":1}}}"#,
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"m","data":{}}}"#,
        r#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
        r#"{"jsonrpc":"2.0","id":1,"result":{"exit_code":null}}"#,
        r#"{"jsonrpc":"2.0","method":"session.exited","params":{"session_id":"a","exit_code":null,"last_seq":0}}"#,
    ];
    for payload in accepted {
        assert!(HostMessage::parse(payload.as_bytes()).is_ok(), "{payload}");
    }
}

#[test]
fn output_frames_outside_the_layout_are_refused_on_the_way_in() {
    let payload = |header: &str, bytes: &[u8]| {
        [
            &(header.len() as u16).to_be_bytes()[..],
            header.as_bytes(),
            bytes,
        ]
        .concat()
    };
    let good = payload(r#"{"session_id":"s","seq":1,"length":3}"#, b"abc");
    assert!(wire::decode_payload(wire::KIND_OUTPUT, good).is_ok());
    for header in [
        r#"{"session_id":"s","seq":0,"length":3}"#,
        r#"{"session_id":"s","seq":1,"length":0}"#,
        r#"{"session_id":"s","seq":1,"length":2}"#,
        r#"{"session_id":"s","seq":1,"length":4}"#,
        r#"{"session_id":"a b","seq":1,"length":3}"#,
        r#"{"session_id":"s","seq":1,"length":3,"x":1}"#,
        r#"{"session_id":"s","seq":1}"#,
        r#"{"session_id":"s","seq":"1","length":3}"#,
        // More than 32 bits can say: the frame prefix could never carry it.
        r#"{"session_id":"s","seq":1,"length":4294967296}"#,
        r#"{"session_id":"s","seq":1,"length":4294967299}"#,
    ] {
        assert!(
            wire::decode_payload(wire::KIND_OUTPUT, payload(header, b"abc")).is_err(),
            "{header}"
        );
    }
}
