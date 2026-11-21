//! Owner control of the task queue through the authenticated local API, with the audit record
//! read back from the anchored log.

use ads_contracts::{CommonIdV1, RecordTaskQueueV1State};
use ads_daemon::api::{ApiConfig, ApiError, DisabledLauncher, LocalApi};
use ads_daemon::auth::LocalSession;
use ads_daemon::events::new_id;
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_daemon::queue::{LoggedQueue, QueueControl, QueueOpenError, QueueOwner};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
use ads_storage::{
    EventLog, StorageConfig, StorageError,
    anchor::{AnchorStore, Commitment, DatabaseId},
    cipher::{CipherError, KeyId, KeyProvider, SecretKey},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

type Socket = WebSocketStream<TcpStream>;

/// A database folder inside the repository's `.test-tmp`, removed file by file on drop.
struct Folder {
    dir: PathBuf,
}
impl Folder {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        assert!(root.ends_with(".test-tmp"));
        let dir = root.join(format!("{name}-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self { dir }
    }
    fn db(&self) -> PathBuf {
        self.dir.join("audit.db")
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        for name in [
            "audit.db",
            "audit.db-wal",
            "audit.db-shm",
            "audit.db-journal",
        ] {
            let _ = std::fs::remove_file(self.dir.join(name));
        }
        let _ = std::fs::remove_dir(&self.dir);
    }
}

struct Keys;
impl KeyProvider for Keys {
    fn current(&self) -> Result<(KeyId, SecretKey), CipherError> {
        Ok((KeyId::new("key-a")?, SecretKey::new(&[1; 32])))
    }
    fn by_id(&self, _: &KeyId) -> Result<SecretKey, CipherError> {
        Ok(SecretKey::new(&[1; 32]))
    }
}

/// The vault slot; the flag makes every store refuse, so the append rolls back.
#[derive(Clone, Default)]
struct Anchor(Arc<Mutex<Option<Commitment>>>, Arc<AtomicBool>);
impl AnchorStore for Anchor {
    fn can_create(&self) -> Result<bool, StorageError> {
        Ok(self.0.lock().unwrap().is_none())
    }
    fn read(&self, _: DatabaseId) -> Result<Option<Commitment>, StorageError> {
        Ok(*self.0.lock().unwrap())
    }
    fn store(&self, _: DatabaseId, commitment: &Commitment) -> Result<(), StorageError> {
        if self.1.load(Ordering::SeqCst) {
            return Err(StorageError::AnchorUnavailable);
        }
        *self.0.lock().unwrap() = Some(*commitment);
        Ok(())
    }
}

fn open_log(path: &Path, anchor: &Anchor) -> EventLog {
    EventLog::open(
        path,
        &StorageConfig::defaults(),
        Box::new(Keys),
        Box::new(anchor.clone()),
    )
    .unwrap()
}

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

/// A database, its vault slot and a queue id of its own: queue ids are held per process.
struct Bench {
    folder: Folder,
    anchor: Anchor,
    queue: String,
}
impl Bench {
    fn new(name: &str) -> Self {
        Self {
            folder: Folder::new(name),
            anchor: Anchor::default(),
            queue: name.to_owned(),
        }
    }
    fn open_as(&self, owner_id: &str) -> Result<LoggedQueue, QueueOpenError> {
        LoggedQueue::open(
            open_log(&self.folder.db(), &self.anchor),
            id(&self.queue),
            QueueOwner {
                owner_id: id(owner_id),
                device_id: id("device-01"),
            },
        )
    }
    fn open(&self) -> Arc<LoggedQueue> {
        Arc::new(self.open_as("owner-01").unwrap())
    }
    /// Opens again once the previous handle, held by aborted API tasks, is gone.
    async fn reopen(&self) -> Arc<LoggedQueue> {
        for _ in 0..200 {
            match self.open_as("owner-01") {
                Err(QueueOpenError::AlreadyOpen) => {
                    tokio::time::sleep(Duration::from_millis(10)).await
                }
                other => return Arc::new(other.unwrap()),
            }
        }
        panic!("the previous handle was never released")
    }
    fn fail_writes(&self, fail: bool) {
        self.anchor.1.store(fail, Ordering::SeqCst);
    }
    /// Every event of the queue stream as JSON, read through a second connection.
    fn audit(&self) -> Vec<Value> {
        open_log(&self.folder.db(), &self.anchor)
            .read_queue(&id(&self.queue))
            .unwrap()
            .into_iter()
            .map(|stored| serde_json::to_value(stored.event).unwrap())
            .collect()
    }
    fn kinds(&self) -> Vec<String> {
        self.audit()
            .iter()
            .map(|e| e["type"].as_str().unwrap()["urn:ads:event:queue.".len()..].to_owned())
            .collect()
    }
    fn params(&self) -> Value {
        json!({"queue_id": self.queue})
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

async fn fixture(queue: Option<Arc<dyn QueueControl>>) -> Fixture {
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
    if let Some(queue) = queue {
        api = api.with_queue(queue);
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
        valid("urn:ads:contract:local-api.server-message:3", &reply),
        "{reply}"
    );
    if let Some(code) = reply["error"].get("code") {
        assert!(
            valid("urn:ads:contract:local-api.error-code:2", code),
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

async fn state(ws: &mut Socket, params: &Value) -> Value {
    let reply = exchange(ws, &command("queue.get", params.clone())).await;
    assert!(
        valid("urn:ads:command:queue.get.response:1", &reply),
        "{reply}"
    );
    reply["result"]["state"].clone()
}

#[tokio::test]
async fn start_records_the_owner_and_stop_reports_whether_it_stopped() {
    let bench = Bench::new("queue-start");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    let params = bench.params();
    assert_eq!(state(&mut ws, &params).await, "stopped");

    let reply = exchange(&mut ws, &command("queue.stop", params.clone())).await;
    assert_eq!(reply["result"], json!({"accepted": true, "replayed": true}));
    assert_eq!(
        bench.kinds(),
        ["created:1"],
        "a stopped queue gets no event"
    );

    let start = command("queue.start", params.clone());
    let reply = exchange(&mut ws, &start).await;
    assert_eq!(
        reply["result"],
        json!({"accepted": true, "replayed": false})
    );
    assert_eq!(state(&mut ws, &params).await, "enabled");
    assert_eq!(exchange(&mut ws, &start).await["result"]["replayed"], true);
    let again = exchange(&mut ws, &command("queue.start", params.clone())).await;
    assert_eq!(again["error"]["code"], -32016);

    assert_eq!(bench.kinds(), ["created:1", "enabled:1"]);
    let enabled = &bench.audit()[1];
    assert_eq!(
        enabled["actor"],
        json!({"kind": "device", "id": "device-01"})
    );
    assert_eq!(enabled["cause"], start["id"]);
    assert_eq!(
        enabled["data"],
        json!({"queue_id": bench.queue, "owner_id": "owner-01", "via": "local_ui"})
    );
    assert!(enabled["ts"].as_str().unwrap().ends_with('Z'));

    let stop = command("queue.stop", params.clone());
    let reply = exchange(&mut ws, &stop).await;
    assert_eq!(
        reply["result"],
        json!({"accepted": true, "replayed": false})
    );
    assert_eq!(state(&mut ws, &params).await, "stopped");
    let last = bench.audit().pop().unwrap();
    assert_eq!(last["cause"], stop["id"]);
    assert_eq!(
        last["data"],
        json!({"queue_id": bench.queue, "reason": "owner", "owner_id": "owner-01", "via": "local_ui"})
    );
}

#[tokio::test]
async fn an_old_stop_sent_again_after_a_restart_stops_the_queue() {
    let bench = Bench::new("queue-old-stop");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    let params = bench.params();
    let first_start = command("queue.start", params.clone());
    exchange(&mut ws, &first_start).await;
    let stop = command("queue.stop", params.clone());
    assert_eq!(exchange(&mut ws, &stop).await["result"]["replayed"], false);
    exchange(&mut ws, &command("queue.start", params.clone())).await;
    assert_eq!(state(&mut ws, &params).await, "enabled");

    let reply = exchange(&mut ws, &stop).await;
    assert_eq!(
        reply["result"],
        json!({"accepted": true, "replayed": false})
    );
    assert_eq!(state(&mut ws, &params).await, "stopped");
    assert_eq!(
        bench.kinds(),
        [
            "created:1",
            "enabled:1",
            "stopped:1",
            "enabled:1",
            "stopped:1"
        ]
    );
    assert_eq!(bench.audit()[4]["cause"], stop["id"]);

    // A key already taken by another command does not block a stop either.
    exchange(&mut ws, &command("queue.start", params.clone())).await;
    let mut reused = command("queue.stop", params.clone());
    reused["params"]["_meta"]["idempotency_key"] =
        first_start["params"]["_meta"]["idempotency_key"].clone();
    assert_eq!(exchange(&mut ws, &reused).await["result"]["accepted"], true);
    assert_eq!(state(&mut ws, &params).await, "stopped");
}

/// Counts every call, to prove refused commands never reach the queue.
#[derive(Default)]
struct Counting(AtomicUsize);
impl QueueControl for Counting {
    fn state(&self, _: &LocalSession, _: &CommonIdV1) -> Result<RecordTaskQueueV1State, ApiError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(RecordTaskQueueV1State::Stopped)
    }
    fn start(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn stop(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<bool, ApiError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(false)
    }
}

#[tokio::test]
async fn refused_queue_commands_never_reach_the_queue() {
    let counting = Arc::new(Counting::default());
    let f = fixture(Some(counting.clone())).await;
    let params = json!({"queue_id": "queue-refusals"});
    for method in ["queue.get", "queue.start", "queue.stop"] {
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
    for method in ["queue.get", "queue.start", "queue.stop"] {
        let reply = exchange(&mut ws, &remote(command(method, params.clone()))).await;
        assert_eq!(reply["error"]["code"], -32010, "{method}");
        let forged = json!({"queue_id": "queue-refusals", "owner_id": "owner-01"});
        let reply = exchange(&mut ws, &command(method, forged)).await;
        assert_eq!(reply["error"]["code"], -32602, "{method}");
    }
    assert_eq!(counting.0.load(Ordering::SeqCst), 0);
    exchange(&mut ws, &command("queue.get", params)).await;
    assert_eq!(counting.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_unknown_or_unbound_queue_is_refused() {
    let bench = Bench::new("queue-unknown");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    let other = json!({"queue_id": "queue-elsewhere"});
    let reply = exchange(&mut ws, &command("queue.start", other)).await;
    assert_eq!(reply["error"]["code"], -32015);
    assert_eq!(bench.kinds(), ["created:1"]);

    let f = fixture(None).await;
    let mut ws = authenticated(&f).await;
    for method in ["queue.get", "queue.start", "queue.stop"] {
        let reply = exchange(&mut ws, &command(method, bench.params())).await;
        assert_eq!(reply["error"]["code"], -32015, "{method}");
    }
}

#[tokio::test]
async fn a_failed_start_write_leaves_the_queue_stopped() {
    let bench = Bench::new("queue-failed-start");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    let params = bench.params();
    bench.fail_writes(true);
    let reply = exchange(&mut ws, &command("queue.start", params.clone())).await;
    assert_eq!(reply["error"]["code"], -32603);
    assert_eq!(state(&mut ws, &params).await, "stopped");
    assert_eq!(bench.kinds(), ["created:1"]);
    bench.fail_writes(false);
    let reply = exchange(&mut ws, &command("queue.start", params.clone())).await;
    assert_eq!(reply["result"]["accepted"], true);
    assert_eq!(bench.kinds(), ["created:1", "enabled:1"]);
}

#[tokio::test]
async fn a_failed_stop_write_still_stops_and_is_recorded_before_the_next_start() {
    let bench = Bench::new("queue-failed-stop");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    let params = bench.params();
    exchange(&mut ws, &command("queue.start", params.clone())).await;

    bench.fail_writes(true);
    let stop = command("queue.stop", params.clone());
    assert_eq!(exchange(&mut ws, &stop).await["error"]["code"], -32603);
    assert_eq!(
        state(&mut ws, &params).await,
        "stopped",
        "the service keeps serving"
    );
    assert_eq!(bench.kinds(), ["created:1", "enabled:1"]);

    bench.fail_writes(false);
    let restart = command("queue.start", params.clone());
    assert_eq!(
        exchange(&mut ws, &restart).await["result"]["accepted"],
        true
    );
    let events = bench.audit();
    assert_eq!(
        bench.kinds(),
        ["created:1", "enabled:1", "stopped:1", "enabled:1"]
    );
    assert_eq!(events[2]["cause"], stop["id"]);
    assert_eq!(events[2]["data"]["reason"], "owner");
    assert_eq!(events[3]["cause"], restart["id"]);
}

#[tokio::test]
async fn a_stop_left_unwritten_at_exit_is_recorded_as_a_restart() {
    let bench = Bench::new("queue-unwritten-stop");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    let params = bench.params();
    exchange(&mut ws, &command("queue.start", params.clone())).await;
    bench.fail_writes(true);
    let stop = command("queue.stop", params.clone());
    assert_eq!(exchange(&mut ws, &stop).await["error"]["code"], -32603);
    drop(ws);
    drop(f);
    bench.fail_writes(false);

    let f = fixture(Some(bench.reopen().await)).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(state(&mut ws, &params).await, "stopped");
    let events = bench.audit();
    assert_eq!(bench.kinds(), ["created:1", "enabled:1", "stopped:1"]);
    assert_eq!(events[2]["actor"]["kind"], "daemon");
    assert_eq!(events[2]["data"]["reason"], "restart");
    assert_eq!(events[2]["cause"], Value::Null);
}

#[tokio::test]
async fn reopening_restores_the_queue_stopped_and_refuses_another_owner() {
    let bench = Bench::new("queue-reopen");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    exchange(&mut ws, &command("queue.start", bench.params())).await;
    drop(ws);
    drop(f);

    let queue = bench.reopen().await;
    assert_eq!(bench.kinds(), ["created:1", "enabled:1", "stopped:1"]);
    assert_eq!(bench.audit()[2]["data"]["reason"], "restart");
    drop(queue);

    assert!(matches!(
        bench.open_as("owner-02"),
        Err(QueueOpenError::ForeignOwner)
    ));
    assert_eq!(bench.kinds().len(), 3);
}

#[test]
fn a_process_holds_one_handle_per_queue() {
    let bench = Bench::new("queue-single-handle");
    let first = bench.open();
    assert!(matches!(
        bench.open_as("owner-01"),
        Err(QueueOpenError::AlreadyOpen)
    ));
    assert_eq!(bench.kinds(), ["created:1"]);
    drop(first);
    drop(bench.open());
}

#[test]
fn only_version_2_of_the_error_catalog_names_the_queue_codes() {
    for code in [-32015, -32016] {
        assert!(!valid(
            "urn:ads:contract:local-api.error-code:1",
            &json!(code)
        ));
        assert!(valid(
            "urn:ads:contract:local-api.error-code:2",
            &json!(code)
        ));
        let response = json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "error": {"code": code, "message": "queue"}});
        assert!(valid("urn:ads:command:response:2", &response));
    }
    let listed = json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "result": {"queue_id": "queue-01", "state": "enabled"}});
    assert!(valid(
        "urn:ads:contract:local-api.server-message:3",
        &listed
    ));
    assert!(!valid(
        "urn:ads:contract:local-api.server-message:2",
        &listed
    ));
}
