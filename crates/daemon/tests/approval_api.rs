//! Owner answers to the action gate through the authenticated local API: routing, refusals and
//! the replies, against a control that counts what reaches it.

use ads_contracts::{CommandApprovalAnswerV1Params, CommandApprovalCloseV1Params};
use ads_daemon::api::{ApiConfig, ApiError, DisabledLauncher, LocalApi};
use ads_daemon::approvals::{ApprovalControl, Pending};
use ads_daemon::auth::LocalSession;
use ads_daemon::events::new_id;
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

type Socket = WebSocketStream<TcpStream>;

const HASH: &str = "sha256:5486cc58827bffeb2204a994db3d77fe6c8f2c863eb73de6e2a60802784293cb";

/// Records every call that reaches it and answers with `refusal`, when set.
#[derive(Default)]
struct Counting {
    calls: Mutex<Vec<(String, String)>>,
    refusal: Mutex<Option<ApiError>>,
    /// Answers as if the log already held this key.
    from_log: std::sync::atomic::AtomicBool,
}
impl ApprovalControl for Counting {
    fn pending(&self, _: &LocalSession) -> Result<Pending, ApiError> {
        self.calls
            .lock()
            .unwrap()
            .push(("list".into(), String::new()));
        let plan: Value = serde_json::from_str(include_str!(
            "../../../contracts/golden/record.action-plan.v2/valid/02-requires-approval.json"
        ))
        .unwrap();
        let approvals = vec![json!({
            "action_id": "act-03", "approval_id": "appr-03", "plan_hash": HASH,
            "risk": "medium", "summary": "Limpar temporários",
            "requested_at": "2030-01-02T03:04:05Z", "expires_at": "2030-01-02T03:19:05Z",
            "not_approvable": ["estimate_unknown", "wildcard_target", "definitive_recursive_delete"], "plan": plan,
            "effects_hash": null, "effects": null, "effects_error": null
        })];
        Ok(Pending {
            approvals,
            more: false,
            unreadable: 2,
        })
    }
    fn answer(
        &self,
        _: &LocalSession,
        answer: &CommandApprovalAnswerV1Params,
        cause: &str,
    ) -> Result<bool, ApiError> {
        self.calls
            .lock()
            .unwrap()
            .push((answer.decision.as_str().into(), cause.into()));
        match *self.refusal.lock().unwrap() {
            Some(error) => Err(error),
            None => Ok(self.from_log.load(std::sync::atomic::Ordering::SeqCst)),
        }
    }
    fn close(
        &self,
        _: &LocalSession,
        close: &CommandApprovalCloseV1Params,
        cause: &str,
    ) -> Result<bool, ApiError> {
        self.calls
            .lock()
            .unwrap()
            .push((format!("close {}", close.decision.as_str()), cause.into()));
        match *self.refusal.lock().unwrap() {
            Some(error) => Err(error),
            None => Ok(false),
        }
    }
}
impl Counting {
    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

struct Fixture {
    port: u16,
    token: String,
    api: tokio::task::JoinHandle<Result<(), ApiError>>,
    peer: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.api.abort();
        self.peer.abort();
    }
}

async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .expect("test timeout")
}

async fn fixture(approvals: Option<Arc<dyn ApprovalControl>>) -> Fixture {
    let backend = Arc::new(FakeBackend::with_echo());
    let host = Arc::new(PtyHost::new(backend, HostConfig::defaults()).unwrap());
    let (near, far) = tokio::io::duplex(262144);
    let peer = tokio::spawn(server::connection(host.clone(), far));
    let client = bounded(PtyClient::over(
        near,
        ClientConfig::recommended(host.config()),
    ))
    .await
    .unwrap();
    let mut api = LocalApi::bind(
        ApiConfig::from_toml_str("").unwrap(),
        client,
        Arc::new(DisabledLauncher),
    )
    .await
    .unwrap();
    if let Some(approvals) = approvals {
        api = api.with_approvals(approvals);
    }
    let port = api.port().unwrap();
    let url = api.launch_url().unwrap();
    let token = url.split_once("&token=").unwrap().1.to_owned();
    Fixture {
        port,
        token,
        api: tokio::spawn(api.run()),
        peer,
    }
}

async fn connect(f: &Fixture) -> Socket {
    let mut request = format!("ws://127.0.0.1:{}/", f.port)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Origin", "http://127.0.0.1:5173".parse().unwrap());
    let stream = TcpStream::connect(("127.0.0.1", f.port)).await.unwrap();
    bounded(tokio_tungstenite::client_async(request, stream))
        .await
        .unwrap()
        .0
}

fn command(method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({"idempotency_key": new_id().unwrap(), "origin": "local"});
    json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "method": method, "params": params})
}

fn remote(mut value: Value) -> Value {
    value["params"]["_meta"]["origin"] = json!("remote");
    value["params"]["_meta"]["remote"] = json!({"device_id": "other", "signature": STANDARD.encode([0;64]), "nonce": STANDARD.encode([0;16]), "counter": 1, "expires_at": "2030-01-01T00:00:00Z"});
    value
}

fn valid(schema: &str, value: &Value) -> bool {
    ads_contracts::reencode(schema, value.clone())
        .unwrap()
        .is_ok()
}

async fn exchange(ws: &mut Socket, value: &Value) -> Value {
    bounded(ws.send(Message::Text(value.to_string().into())))
        .await
        .unwrap();
    let Message::Text(text) = bounded(ws.next()).await.unwrap().unwrap() else {
        panic!("expected text")
    };
    let reply: Value = serde_json::from_str(&text).unwrap();
    assert!(
        valid("urn:ads:contract:local-api.server-message:10", &reply),
        "{reply}"
    );
    if let Some(code) = reply["error"].get("code") {
        assert!(
            valid("urn:ads:contract:local-api.error-code:6", code),
            "{reply}"
        );
    }
    assert_eq!(reply["id"], value["id"]);
    reply
}

async fn authenticated(f: &Fixture) -> Socket {
    let mut ws = connect(f).await;
    let hello = exchange(&mut ws, &command("auth.hello", json!({"token": f.token}))).await;
    assert_eq!(hello["result"]["accepted"], true);
    ws
}

fn close(decision: &str) -> Value {
    json!({"action_id": "act-03", "decision": decision})
}

fn answer(decision: &str) -> Value {
    json!({"action_id": "act-03", "approval_id": "appr-03", "plan_hash": HASH, "decision": decision})
}

#[tokio::test]
async fn refused_approval_commands_never_reach_the_gate() {
    let counting = Arc::new(Counting::default());
    let f = fixture(Some(counting.clone())).await;
    for (method, params) in [
        ("approval.list", json!({})),
        ("approval.answer", answer("approve")),
        ("approval.close", close("deny")),
    ] {
        // No session: the first message must be auth.hello, and the connection closes.
        let mut unauthenticated = connect(&f).await;
        let first = command(method, params.clone()).to_string();
        bounded(unauthenticated.send(Message::Text(first.into())))
            .await
            .unwrap();
        assert!(matches!(
            bounded(unauthenticated.next()).await,
            None | Some(Err(_)) | Some(Ok(Message::Close(_)))
        ));
    }
    let mut ws = authenticated(&f).await;
    for (method, params) in [
        ("approval.list", json!({})),
        ("approval.answer", answer("approve")),
        ("approval.answer", answer("deny")),
        ("approval.close", close("deny")),
        ("approval.close", close("expire")),
    ] {
        let reply = exchange(&mut ws, &remote(command(method, params.clone()))).await;
        assert_eq!(reply["error"]["code"], -32010, "{method}");
        let mut forged = params.clone();
        forged["owner_id"] = json!("owner-01");
        let reply = exchange(&mut ws, &command(method, forged)).await;
        assert_eq!(reply["error"]["code"], -32602, "{method}");
    }
    assert!(counting.calls().is_empty());
}

#[tokio::test]
async fn list_answers_with_the_plan_and_answer_reaches_the_gate_once() {
    let counting = Arc::new(Counting::default());
    let f = fixture(Some(counting.clone())).await;
    let mut ws = authenticated(&f).await;
    let list = command("approval.list", json!({}));
    let reply = exchange(&mut ws, &list).await;
    assert!(
        valid("urn:ads:command:approval.list.response:4", &reply),
        "{reply}"
    );
    assert_eq!(reply["result"]["unreadable"], 2);
    assert_eq!(
        reply["result"]["approvals"][0]["plan"]["action_id"],
        "act-03"
    );
    // Read only: the same command again is read again, outside the idempotency cache.
    exchange(&mut ws, &list).await;
    assert_eq!(counting.calls().len(), 2);

    let approve = command("approval.answer", answer("approve"));
    let reply = exchange(&mut ws, &approve).await;
    assert_eq!(
        reply["result"],
        json!({"accepted": true, "replayed": false})
    );
    let reply = exchange(&mut ws, &approve).await;
    assert_eq!(reply["result"], json!({"accepted": true, "replayed": true}));
    let calls = counting.calls();
    assert_eq!(calls.len(), 3, "a repeated key never answers twice");
    assert_eq!(
        calls[2],
        (
            "approve".to_owned(),
            approve["id"].as_str().unwrap().to_owned()
        )
    );

    // A key the cache never saw but the log holds is reported as a repetition.
    counting
        .from_log
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let reply = exchange(&mut ws, &command("approval.answer", answer("approve"))).await;
    assert_eq!(reply["result"], json!({"accepted": true, "replayed": true}));
}

#[tokio::test]
async fn gate_refusals_keep_their_codes() {
    let counting = Arc::new(Counting::default());
    let f = fixture(Some(counting.clone())).await;
    let mut ws = authenticated(&f).await;
    for (error, code) in [
        (ApiError::ApprovalNotFound, -32018),
        (ApiError::ApprovalAlreadyAnswered, -32019),
        (ApiError::ApprovalStale, -32020),
        (ApiError::ApprovalPolicyChanged, -32021),
        (ApiError::ApprovalOutOfOrder, -32022),
        (ApiError::ApprovalNotApprovable, -32023),
        (ApiError::Storage, -32603),
    ] {
        *counting.refusal.lock().unwrap() = Some(error);
        let reply = exchange(&mut ws, &command("approval.answer", answer("approve"))).await;
        assert_eq!(reply["error"]["code"], code, "{error:?}");
    }
    // A storage refusal does not take the service down.
    *counting.refusal.lock().unwrap() = None;
    let reply = exchange(&mut ws, &command("approval.answer", answer("deny"))).await;
    assert_eq!(reply["result"]["accepted"], true);
}

#[tokio::test]
async fn without_a_gate_every_approval_command_is_not_configured() {
    let f = fixture(None).await;
    let mut ws = authenticated(&f).await;
    for (method, params) in [
        ("approval.list", json!({})),
        ("approval.answer", answer("approve")),
    ] {
        let reply = exchange(&mut ws, &command(method, params)).await;
        assert_eq!(reply["error"]["code"], -32017, "{method}");
    }
}

#[tokio::test]
async fn a_closing_reaches_the_gate_once_and_keeps_its_refusals() {
    let counting = Arc::new(Counting::default());
    let f = fixture(Some(counting.clone())).await;
    let mut ws = authenticated(&f).await;
    let expire = command("approval.close", close("expire"));
    let reply = exchange(&mut ws, &expire).await;
    assert_eq!(
        reply["result"],
        json!({"accepted": true, "replayed": false})
    );
    let reply = exchange(&mut ws, &expire).await;
    assert_eq!(reply["result"], json!({"accepted": true, "replayed": true}));
    assert_eq!(
        counting.calls(),
        [(
            "close expire".to_owned(),
            expire["id"].as_str().unwrap().to_owned()
        )]
    );
    // Closing never approves.
    let reply = exchange(&mut ws, &command("approval.close", close("approve"))).await;
    assert_eq!(reply["error"]["code"], -32602);
    for (error, code) in [
        (ApiError::ApprovalReadable, -32025),
        (ApiError::ApprovalNotFound, -32018),
        (ApiError::ApprovalAlreadyAnswered, -32019),
    ] {
        *counting.refusal.lock().unwrap() = Some(error);
        let reply = exchange(&mut ws, &command("approval.close", close("deny"))).await;
        assert_eq!(reply["error"]["code"], code, "{error:?}");
    }
    assert!(!valid(
        "urn:ads:contract:local-api.error-code:4",
        &json!(-32025)
    ));
}

#[tokio::test]
async fn without_a_gate_a_closing_is_not_configured() {
    let f = fixture(None).await;
    let mut ws = authenticated(&f).await;
    let reply = exchange(&mut ws, &command("approval.close", close("deny"))).await;
    assert_eq!(reply["error"]["code"], -32017);
}
