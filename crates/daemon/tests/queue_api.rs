//! Owner control of the task queue through the authenticated local API, with the audit record
//! read back from the anchored log.

use ads_contracts::{
    CommandQueueGetResponseV2Result, CommandQueueGetResponseV2ResultState, CommonIdV1,
    RecordTaskQueueV1State,
};
use ads_core::queue::{DEFAULT_POLICY, QueuePolicy, new_local_stopped};
use ads_daemon::api::{ApiConfig, ApiError, DisabledLauncher, LocalApi};
use ads_daemon::auth::LocalSession;
use ads_daemon::events::new_id;
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_daemon::queue::{LocalQueue, LoggedQueue, QueueControl, QueueOpenError, QueueOwner};
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

/// The vault slot, with controls for delaying or failing writes.
#[derive(Clone, Default)]
struct Anchor {
    commitment: Arc<Mutex<Option<Commitment>>>,
    fail_writes: Arc<AtomicBool>,
    hold_write: Arc<Mutex<Option<HeldWrite>>>,
}

struct HeldWrite {
    entered: tokio::sync::oneshot::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

impl AnchorStore for Anchor {
    fn can_create(&self) -> Result<bool, StorageError> {
        Ok(self.commitment.lock().unwrap().is_none())
    }
    fn read(&self, _: DatabaseId) -> Result<Option<Commitment>, StorageError> {
        Ok(*self.commitment.lock().unwrap())
    }
    fn store(&self, _: DatabaseId, commitment: &Commitment) -> Result<(), StorageError> {
        if self.fail_writes.load(Ordering::SeqCst) {
            return Err(StorageError::AnchorUnavailable);
        }
        let hold = self.hold_write.lock().unwrap().take();
        if let Some(hold) = hold {
            hold.entered
                .send(())
                .map_err(|_| StorageError::AnchorUnavailable)?;
            hold.release
                .recv_timeout(Duration::from_secs(15))
                .map_err(|_| StorageError::AnchorUnavailable)?;
        }
        *self.commitment.lock().unwrap() = Some(*commitment);
        Ok(())
    }
}

fn open_log(path: &Path, anchor: &Anchor, config: &StorageConfig) -> EventLog {
    EventLog::open(path, config, Box::new(Keys), Box::new(anchor.clone())).unwrap()
}

fn id(text: &str) -> CommonIdV1 {
    CommonIdV1::new(text).unwrap()
}

/// A database, its vault slot and a queue id of its own: queue ids are held per process.
struct Bench {
    folder: Folder,
    anchor: Anchor,
    queue: String,
    config: StorageConfig,
}
impl Bench {
    fn new(name: &str) -> Self {
        Self {
            folder: Folder::new(name),
            anchor: Anchor::default(),
            queue: name.to_owned(),
            config: StorageConfig::defaults(),
        }
    }
    fn bind_or_pending(&self) -> LocalQueue {
        self.try_bind_or_pending().unwrap()
    }

    fn try_bind_or_pending(&self) -> Result<LocalQueue, QueueOpenError> {
        self.bind_shared(Arc::new(Mutex::new(open_log(
            &self.folder.db(),
            &self.anchor,
            &self.config,
        ))))
    }

    fn bind_shared(&self, log: Arc<Mutex<EventLog>>) -> Result<LocalQueue, QueueOpenError> {
        LocalQueue::bind_or_pending(
            log,
            id(&self.queue),
            QueueOwner {
                owner_id: id("owner-01"),
                device_id: id("device-01"),
            },
            Duration::from_millis(self.config.busy_timeout_ms().into()),
        )
    }
    fn open_as(&self, owner_id: &str) -> Result<LoggedQueue, QueueOpenError> {
        LoggedQueue::bind(
            Arc::new(Mutex::new(open_log(
                &self.folder.db(),
                &self.anchor,
                &self.config,
            ))),
            id(&self.queue),
            QueueOwner {
                owner_id: id(owner_id),
                device_id: id("device-01"),
            },
            Duration::from_millis(self.config.busy_timeout_ms().into()),
        )
    }
    /// Binds the queue, first writing its creation by the owner's device when the stream is empty.
    fn open(&self) -> Arc<LoggedQueue> {
        if self.audit().is_empty() {
            let policy: QueuePolicy = toml::from_str(DEFAULT_POLICY).unwrap();
            let (_, created) = new_local_stopped(id(&self.queue), &id("owner-01"), &policy);
            self.seed(1, "created:2", serde_json::to_value(created).unwrap());
        }
        Arc::new(self.open_as("owner-01").unwrap())
    }
    /// Appends an event by the owner's device straight to the log.
    fn seed(&self, seq: u64, kind: &str, data: Value) {
        let event = serde_json::from_value(json!({
            "id": new_id().unwrap(),
            "type": format!("urn:ads:event:queue.{kind}"),
            "ts": "2030-01-02T03:04:05Z",
            "stream": format!("queue/{}", self.queue),
            "seq": seq,
            "actor": {"kind": "device", "id": "device-01"},
            "cause": null,
            "data_class": "internal",
            "data": data,
        }))
        .unwrap();
        open_log(&self.folder.db(), &self.anchor, &self.config)
            .append(&event)
            .unwrap();
    }
    /// Opens again once the previous handle, held by aborted API tasks, is gone.
    async fn reopen(&self) -> Arc<LoggedQueue> {
        Arc::new(wait_for_queue(|| self.open_as("owner-01")).await)
    }
    fn fail_writes(&self, fail: bool) {
        self.anchor.fail_writes.store(fail, Ordering::SeqCst);
    }
    /// Every event of the queue stream as JSON, read through a second connection.
    fn audit(&self) -> Vec<Value> {
        open_log(&self.folder.db(), &self.anchor, &self.config)
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

async fn queue_result(ws: &mut Socket, params: &Value) -> Value {
    let reply = exchange(ws, &command("queue.get", params.clone())).await;
    assert!(
        valid("urn:ads:command:queue.get.response:2", &reply),
        "{reply}"
    );
    reply["result"].clone()
}

async fn state(ws: &mut Socket, params: &Value) -> Value {
    queue_result(ws, params).await["state"].clone()
}

fn custom_creation(bench: &Bench) -> Value {
    let policy: QueuePolicy =
        toml::from_str("attempt_limit = 5\nadmission_limit = 77\nwindow_seconds = 1800").unwrap();
    let (_, created) = new_local_stopped(id(&bench.queue), &id("owner-01"), &policy);
    let created = serde_json::to_value(created).unwrap();
    bench.seed(1, "created:2", created.clone());
    created["policy"].clone()
}

#[tokio::test]
async fn get_reports_the_recorded_creation_policy() {
    let bench = Bench::new("queue-get-creation");
    let policy = custom_creation(&bench);
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        queue_result(&mut ws, &bench.params()).await,
        json!({"queue_id": bench.queue, "state": "stopped", "policy": policy})
    );
}

#[tokio::test]
async fn get_reports_the_latest_policy_change_without_rebinding() {
    let bench = Bench::new("queue-get-change");
    let mut policy = custom_creation(&bench);
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut ws = authenticated(&f).await;
    let request = command("queue.get", bench.params());
    assert_eq!(
        exchange(&mut ws, &request).await["result"]["policy"],
        policy
    );

    for (seq, window) in [(2, 900), (3, 450)] {
        policy["window_seconds"] = json!(window);
        bench.seed(
            seq,
            "policy_changed:1",
            json!({
                "queue_id": bench.queue, "owner_id": "owner-01",
                "via": "local_ui", "policy": policy
            }),
        );
    }

    assert_eq!(
        exchange(&mut ws, &request).await["result"]["policy"],
        policy
    );
}

#[tokio::test]
async fn get_reports_null_for_a_legacy_queue_without_a_policy() {
    let bench = Bench::new("queue-get-legacy");
    bench.seed(
        1,
        "created:1",
        json!({"queue_id": bench.queue, "owner_id": "owner-01", "via": "local_ui", "state": "stopped"}),
    );
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        queue_result(&mut ws, &bench.params()).await,
        json!({"queue_id": bench.queue, "state": "stopped", "policy": null})
    );
    assert_eq!(bench.kinds(), ["created:1"]);
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
        ["created:2"],
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

    assert_eq!(bench.kinds(), ["created:2", "enabled:1"]);
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
            "created:2",
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

struct ObservedStop {
    queue: Arc<LoggedQueue>,
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl QueueControl for ObservedStop {
    fn get(
        &self,
        session: &LocalSession,
        id: &CommonIdV1,
    ) -> Result<CommandQueueGetResponseV2Result, ApiError> {
        self.queue.get(session, id)
    }

    fn state(
        &self,
        session: &LocalSession,
        id: &CommonIdV1,
    ) -> Result<RecordTaskQueueV1State, ApiError> {
        self.queue.state(session, id)
    }

    fn start(&self, session: &LocalSession, id: &CommonIdV1, cause: &str) -> Result<(), ApiError> {
        self.queue.start(session, id, cause)
    }

    fn stop(&self, session: &LocalSession, id: &CommonIdV1, cause: &str) -> Result<bool, ApiError> {
        self.entered
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(())
            .unwrap();
        self.queue.stop(session, id, cause)
    }
}

#[tokio::test]
async fn a_locked_queue_stop_does_not_stall_other_connections() {
    let mut bench = Bench::new("queue-stop-lock");
    bench.config = StorageConfig::from_toml_str("busy_timeout_ms = 60000").unwrap();
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let f = fixture(Some(Arc::new(ObservedStop {
        queue: bench.open(),
        entered: Mutex::new(Some(entered)),
    })))
    .await;
    let mut ws = authenticated(&f).await;
    let mut other = authenticated(&f).await;
    assert_eq!(
        exchange(&mut ws, &command("queue.start", bench.params())).await["result"]["accepted"],
        true
    );

    let mut writer = rusqlite::Connection::open(bench.folder.db()).unwrap();
    let lock = writer
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    let stop = command("queue.stop", bench.params());
    let stop = tokio::spawn(async move { exchange(&mut ws, &stop).await });
    bounded(waiting).await.unwrap();

    let subscribe = command(
        "events.subscribe",
        json!({"stream": "session/absent", "after_seq": 0}),
    );
    let reply = bounded(exchange(&mut other, &subscribe)).await;
    assert_eq!(reply["result"]["accepted"], true, "{reply}");
    assert!(
        !stop.is_finished(),
        "the stop must still be waiting for the write lock"
    );
    lock.rollback().unwrap();
    assert_eq!(bounded(stop).await.unwrap()["result"]["accepted"], true);
}

/// Counts every call, to prove refused commands never reach the queue.
#[derive(Default)]
struct Counting(AtomicUsize);
impl QueueControl for Counting {
    fn get(
        &self,
        _: &LocalSession,
        id: &CommonIdV1,
    ) -> Result<CommandQueueGetResponseV2Result, ApiError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(CommandQueueGetResponseV2Result {
            queue_id: id.clone(),
            state: CommandQueueGetResponseV2ResultState::Stopped,
            policy: None,
        })
    }

    fn create(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
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
    for method in ["queue.get", "queue.create", "queue.start", "queue.stop"] {
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
    for method in ["queue.get", "queue.create", "queue.start", "queue.stop"] {
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
async fn a_mistyped_queue_id_is_refused_as_not_found() {
    let methods = ["queue.get", "queue.create", "queue.start", "queue.stop"];
    let other = json!({"queue_id": "queue-elsewhere"});
    let bench = Bench::new("queue-unknown");
    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    for method in methods {
        let reply = exchange(&mut ws, &command(method, other.clone())).await;
        assert_eq!(reply["error"]["code"], -32026, "{method}");
    }
    assert_eq!(bench.kinds(), ["created:2"]);

    let pending = Bench::new("queue-unknown-pending");
    let f = fixture(Some(Arc::new(pending.bind_or_pending()))).await;
    let mut ws = authenticated(&f).await;
    for method in methods {
        let reply = exchange(&mut ws, &command(method, other.clone())).await;
        assert_eq!(reply["error"]["code"], -32026, "{method}");
    }
    assert!(pending.audit().is_empty());
}

#[tokio::test]
async fn a_configured_queue_not_yet_created_is_refused_as_not_created() {
    let bench = Bench::new("queue-not-created");
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut ws = authenticated(&f).await;
    for method in ["queue.get", "queue.start", "queue.stop"] {
        let reply = exchange(&mut ws, &command(method, bench.params())).await;
        assert_eq!(reply["error"]["code"], -32027, "{method}");
    }
    assert!(bench.audit().is_empty());
}

#[tokio::test]
async fn queue_commands_without_queue_controls_are_refused_as_not_configured() {
    let f = fixture(None).await;
    let mut ws = authenticated(&f).await;
    let params = json!({"queue_id": "queue-01"});
    for method in ["queue.get", "queue.create", "queue.start", "queue.stop"] {
        let reply = exchange(&mut ws, &command(method, params.clone())).await;
        assert_eq!(reply["error"]["code"], -32015, "{method}");
    }
    let reply = exchange(&mut ws, &command("queue.configured", json!({}))).await;
    assert_eq!(reply["error"]["code"], -32015);
}

#[tokio::test]
async fn the_configured_queue_id_is_told_before_and_after_creation() {
    let bench = Bench::new("queue-configured");
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut ws = authenticated(&f).await;
    let ask = || command("queue.configured", json!({}));
    let expected = json!({"queue_id": bench.queue});
    let reply = exchange(&mut ws, &ask()).await;
    assert!(valid("urn:ads:command:queue.configured.response:1", &reply));
    assert_eq!(reply["result"], expected);

    let create = command("queue.create", bench.params());
    assert_eq!(exchange(&mut ws, &create).await["result"]["accepted"], true);
    assert_eq!(exchange(&mut ws, &ask()).await["result"], expected);

    let reply = exchange(&mut ws, &remote(ask())).await;
    assert_eq!(reply["error"]["code"], -32010);
    let forged = command("queue.configured", json!({"queue_id": "queue-elsewhere"}));
    assert_eq!(exchange(&mut ws, &forged).await["error"]["code"], -32602);
    assert_eq!(bench.kinds(), ["created:2"]);
}

#[tokio::test(flavor = "current_thread")]
async fn queue_discovery_answers_while_a_start_holds_the_write() {
    let bench = Bench::new("queue-configured-wait");
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut writer = authenticated(&f).await;
    let mut ws = authenticated(&f).await;
    let create = command("queue.create", bench.params());
    assert_eq!(
        exchange(&mut writer, &create).await["result"]["accepted"],
        true
    );

    // The anchor write hook signals `entered` only after `queue.start` has acquired the queue
    // write lock (the queue mutex) and called into anchor storage. Awaiting the `writing`
    // oneshot via `bounded(writing).await` guarantees that the write lock is already held
    // through `hold_write` in the anchor before `queue.configured` executes.
    let (entered, writing) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    *bench.anchor.hold_write.lock().unwrap() = Some(HeldWrite {
        entered,
        release: held,
    });
    let start = command("queue.start", bench.params());
    let start = tokio::spawn(async move { exchange(&mut writer, &start).await });
    bounded(writing).await.unwrap();

    let reply = bounded(exchange(&mut ws, &command("queue.configured", json!({})))).await;
    assert_eq!(reply["result"], json!({"queue_id": bench.queue}), "{reply}");
    // The `!start.is_finished()` assertion confirms that the write operation remained held
    // throughout the queue discovery response, proving discovery does not block on write locks.
    assert!(!start.is_finished(), "the start must still hold the write");
    release.send(()).unwrap();
    let reply = bounded(start).await.unwrap();
    assert_eq!(reply["result"]["accepted"], true, "{reply}");
}

/// Discovery that blocks its thread until released, to prove the API runs it off the runtime.
struct BlockingDiscovery {
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}
impl QueueControl for BlockingDiscovery {
    fn configured(&self, _: &LocalSession) -> Result<CommonIdV1, ApiError> {
        if let Some(entered) = self.entered.lock().unwrap().take() {
            entered.send(()).unwrap();
        }
        // The timeout matches the bounded deadline to free the thread if discovery runs on the runtime.
        let released = self
            .release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5));
        released.map_err(|_| ApiError::Storage)?;
        Ok(id("queue-blocked"))
    }
    fn get(
        &self,
        _: &LocalSession,
        _: &CommonIdV1,
    ) -> Result<CommandQueueGetResponseV2Result, ApiError> {
        Err(ApiError::QueueNotFound)
    }
    fn state(&self, _: &LocalSession, _: &CommonIdV1) -> Result<RecordTaskQueueV1State, ApiError> {
        Err(ApiError::QueueNotFound)
    }
    fn start(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<(), ApiError> {
        Err(ApiError::QueueNotFound)
    }
    fn stop(&self, _: &LocalSession, _: &CommonIdV1, _: &str) -> Result<bool, ApiError> {
        Err(ApiError::QueueNotFound)
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_blocking_queue_discovery_does_not_stall_other_requests() {
    let (entered, discovering) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    let f = fixture(Some(Arc::new(BlockingDiscovery {
        entered: Mutex::new(Some(entered)),
        release: Mutex::new(held),
    })))
    .await;
    let mut ws = authenticated(&f).await;
    let mut other = authenticated(&f).await;

    let ask = command("queue.configured", json!({}));
    let ask = tokio::spawn(async move { exchange(&mut ws, &ask).await });
    bounded(discovering).await.unwrap();
    let subscribe = command(
        "events.subscribe",
        json!({"stream": "session/absent", "after_seq": 0}),
    );
    let reply = exchange(&mut other, &subscribe).await;
    assert_eq!(reply["result"]["accepted"], true, "{reply}");
    assert!(!ask.is_finished(), "discovery must still be blocked");
    release.send(()).unwrap();

    let reply = bounded(ask).await.unwrap();
    assert_eq!(reply["result"], json!({"queue_id": "queue-blocked"}));
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
    assert_eq!(bench.kinds(), ["created:2"]);
    bench.fail_writes(false);
    let reply = exchange(&mut ws, &command("queue.start", params.clone())).await;
    assert_eq!(reply["result"]["accepted"], true);
    assert_eq!(bench.kinds(), ["created:2", "enabled:1"]);
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
    assert_eq!(bench.kinds(), ["created:2", "enabled:1"]);

    bench.fail_writes(false);
    let restart = command("queue.start", params.clone());
    assert_eq!(
        exchange(&mut ws, &restart).await["result"]["accepted"],
        true
    );
    let events = bench.audit();
    assert_eq!(
        bench.kinds(),
        ["created:2", "enabled:1", "stopped:1", "enabled:1"]
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
    assert_eq!(bench.kinds(), ["created:2", "enabled:1", "stopped:1"]);
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
    assert_eq!(bench.kinds(), ["created:2", "enabled:1", "stopped:1"]);
    assert_eq!(bench.audit()[2]["data"]["reason"], "restart");
    drop(queue);

    assert!(matches!(
        bench.open_as("owner-02"),
        Err(QueueOpenError::ForeignOwner)
    ));
    assert_eq!(bench.kinds().len(), 3);
}

#[tokio::test]
async fn opening_a_legacy_stream_restores_it_stopped_and_checks_the_owner() {
    let bench = Bench::new("queue-legacy");
    let owner = json!({"queue_id": bench.queue, "owner_id": "owner-01", "via": "local_ui"});
    let mut created = owner.clone();
    created["state"] = "stopped".into();
    bench.seed(1, "created:1", created);
    bench.seed(2, "enabled:1", owner);

    assert!(matches!(
        bench.open_as("owner-02"),
        Err(QueueOpenError::ForeignOwner)
    ));
    assert_eq!(bench.kinds(), ["created:1", "enabled:1"]);

    let f = fixture(Some(bench.open())).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(state(&mut ws, &bench.params()).await, "stopped");
    assert_eq!(bench.kinds(), ["created:1", "enabled:1", "stopped:1"]);
    assert_eq!(bench.audit()[2]["data"]["reason"], "restart");
}

#[test]
fn a_process_holds_one_handle_per_queue() {
    let bench = Bench::new("queue-single-handle");
    let first = bench.open();
    assert!(matches!(
        bench.open_as("owner-01"),
        Err(QueueOpenError::AlreadyOpen)
    ));
    assert_eq!(bench.kinds(), ["created:2"]);
    drop(first);
    drop(bench.open());
}

#[test]
fn binding_an_empty_stream_without_a_session_creates_nothing() {
    let bench = Bench::new("queue-no-session");
    for _ in 0..2 {
        assert!(matches!(
            bench.open_as("owner-01"),
            Err(QueueOpenError::NotCreated)
        ));
    }
    assert!(bench.kinds().is_empty());
    drop(bench.open());
    assert_eq!(bench.kinds(), ["created:2"]);
}

#[test]
fn versioned_error_catalogs_keep_the_published_queue_codes() {
    for code in [-32015, -32016] {
        assert!(!valid(
            "urn:ads:contract:local-api.error-code:1",
            &json!(code)
        ));
        assert!(valid(
            "urn:ads:contract:local-api.error-code:2",
            &json!(code)
        ));
        assert!(valid(
            "urn:ads:contract:local-api.error-code:4",
            &json!(code)
        ));
        assert!(valid(
            "urn:ads:contract:local-api.error-code:6",
            &json!(code)
        ));
        let response = json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "error": {"code": code, "message": "queue"}});
        assert!(valid("urn:ads:command:response:2", &response));
    }
    assert!(!valid(
        "urn:ads:contract:local-api.error-code:3",
        &json!(-32024)
    ));
    for code in [-32026, -32027] {
        assert!(!valid(
            "urn:ads:contract:local-api.error-code:5",
            &json!(code)
        ));
        assert!(valid(
            "urn:ads:contract:local-api.error-code:6",
            &json!(code)
        ));
    }
    let listed = json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "result": {
        "harnesses": ["test"], "workdir_aliases": ["home"], "missing_workdir_aliases": ["work"]
    }});
    assert!(!valid(
        "urn:ads:contract:local-api.server-message:7",
        &listed
    ));
    for schema in [
        "urn:ads:contract:local-api.server-message:8",
        "urn:ads:contract:local-api.server-message:9",
        "urn:ads:contract:local-api.server-message:10",
    ] {
        assert!(valid(schema, &listed), "{schema}");
    }
    let configured =
        json!({"jsonrpc": "2.0", "id": new_id().unwrap(), "result": {"queue_id": "queue-01"}});
    assert!(valid(
        "urn:ads:contract:local-api.server-message:10",
        &configured
    ));
    for schema in [
        "urn:ads:contract:local-api.server-message:8",
        "urn:ads:contract:local-api.server-message:9",
    ] {
        assert!(!valid(schema, &configured), "{schema}");
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

#[tokio::test]
async fn create_without_a_session_leaves_the_stream_empty() {
    let bench = Bench::new("queue-create-no-session");
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut ws = connect(&f).await;
    let create = command("queue.create", bench.params());
    bounded(ws.send(Message::Text(create.to_string().into())))
        .await
        .unwrap();
    assert!(matches!(
        bounded(ws.next()).await,
        None | Some(Err(_)) | Some(Ok(Message::Close(_)))
    ));
    assert!(bench.audit().is_empty());
}

#[tokio::test]
async fn create_records_the_default_policy_and_refuses_a_second_creation() {
    let bench = Bench::new("queue-create-policy");
    let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
    let mut ws = authenticated(&f).await;
    let create = command("queue.create", bench.params());
    assert_eq!(exchange(&mut ws, &create).await["result"]["accepted"], true);
    assert_eq!(state(&mut ws, &bench.params()).await, "stopped");

    let policy: QueuePolicy = toml::from_str(DEFAULT_POLICY).unwrap();
    let (_, expected) = new_local_stopped(id(&bench.queue), &id("owner-01"), &policy);
    let events = bench.audit();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "urn:ads:event:queue.created:2");
    assert_eq!(events[0]["data"], serde_json::to_value(expected).unwrap());
    assert_eq!(events[0]["actor"]["id"], "device-01");
    assert_eq!(events[0]["cause"], create["id"]);

    assert_eq!(exchange(&mut ws, &create).await["result"]["replayed"], true);
    let again = exchange(&mut ws, &command("queue.create", bench.params())).await;
    assert_eq!(again["error"]["code"], -32024);
    assert_eq!(again["error"]["message"], "queue already created");
    assert_eq!(bench.audit(), events);
}

async fn wait_for_queue<T>(mut bind: impl FnMut() -> Result<T, QueueOpenError>) -> T {
    for _ in 0..200 {
        match bind() {
            Err(QueueOpenError::AlreadyOpen) => tokio::time::sleep(Duration::from_millis(10)).await,
            other => return other.unwrap(),
        }
    }
    panic!("the previous handle was never released")
}

#[tokio::test]
async fn rebinding_waits_for_the_previous_queue_handle() {
    let bench = Bench::new("queue-rebind");
    let mut previous = Some(bench.open());
    wait_for_queue(|| {
        let result = bench.try_bind_or_pending();
        if previous.take().is_some() {
            assert!(matches!(result, Err(QueueOpenError::AlreadyOpen)));
        }
        result
    })
    .await;
}

#[tokio::test]
async fn startup_binds_an_existing_queue_stopped() {
    let bench = Bench::new("queue-create-restart");
    {
        let f = fixture(Some(Arc::new(bench.bind_or_pending()))).await;
        let mut ws = authenticated(&f).await;
        for method in ["queue.create", "queue.start"] {
            assert_eq!(
                exchange(&mut ws, &command(method, bench.params())).await["result"]["accepted"],
                true
            );
        }
        f.api.abort();
        drop(ws);
        bounded(async {
            while !f.api.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await;
    }
    let queue = wait_for_queue(|| bench.try_bind_or_pending()).await;
    let f = fixture(Some(Arc::new(queue))).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(state(&mut ws, &bench.params()).await, "stopped");
    let events = bench.audit();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["type"], "urn:ads:event:queue.stopped:1");
    assert_eq!(events[2]["data"]["reason"], "restart");
}

#[test]
fn startup_binding_times_out_while_the_shared_log_is_locked() {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Instant;

    let mut bench = Bench::new("queue-bind-busy");
    bench.config = StorageConfig::from_toml_str("busy_timeout_ms = 50").unwrap();
    drop(bench.open());
    let log = Arc::new(Mutex::new(open_log(
        &bench.folder.db(),
        &bench.anchor,
        &bench.config,
    )));
    let result = thread::scope(|scope| {
        let (locked, lock_ready) = mpsc::channel();
        let (release, released) = mpsc::channel::<()>();
        let held_log = log.clone();
        scope.spawn(move || {
            let _guard = held_log.lock().unwrap();
            locked.send(()).unwrap();
            let _ = released.recv();
        });
        let external_timeout = Duration::from_secs(2);
        lock_ready.recv_timeout(external_timeout).unwrap();

        let (finished, result) = mpsc::channel();
        let bench = &bench;
        let binding_log = log.clone();
        scope.spawn(move || {
            let started = Instant::now();
            let outcome = bench.bind_shared(binding_log);
            let _ = finished.send((outcome, started.elapsed()));
        });
        let result = result.recv_timeout(external_timeout);
        drop(release);
        result
    });
    let (outcome, elapsed) = result.expect("binding did not finish while the log lock was held");
    assert!(matches!(
        outcome,
        Err(QueueOpenError::Storage(StorageError::Busy))
    ));
    assert!(elapsed >= Duration::from_millis(50));
    assert!(bench.bind_shared(log).is_ok());
}
