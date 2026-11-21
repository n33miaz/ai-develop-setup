//! Owner control of the task queue through the authenticated local API, with the audit record
//! read back from the anchored log.

use ads_contracts::{CommonIdV1, RecordTaskQueueV1State};
use ads_core::queue::AuthenticatedDevice;
use ads_daemon::api::{ApiConfig, DisabledLauncher, LocalApi};
use ads_daemon::events::new_id;
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_daemon::queue::{LoggedQueue, QueueControl, QueueOpenError};
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

#[derive(Clone, Default)]
struct Anchor(Arc<Mutex<Option<Commitment>>>);
impl AnchorStore for Anchor {
    fn can_create(&self) -> Result<bool, StorageError> {
        Ok(self.0.lock().unwrap().is_none())
    }
    fn read(&self, _: DatabaseId) -> Result<Option<Commitment>, StorageError> {
        Ok(*self.0.lock().unwrap())
    }
    fn store(&self, _: DatabaseId, commitment: &Commitment) -> Result<(), StorageError> {
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

fn owner() -> AuthenticatedDevice {
    AuthenticatedDevice::local_owner(id("device-01"), id("owner-01"))
}

fn open_queue(path: &Path, anchor: &Anchor) -> Result<LoggedQueue, QueueOpenError> {
    LoggedQueue::open(open_log(path, anchor), id("queue-01"), owner())
}

/// Every event of the queue stream as JSON, read through a second connection.
fn audit(path: &Path, anchor: &Anchor) -> Vec<Value> {
    open_log(path, anchor)
        .read_queue(&id("queue-01"))
        .unwrap()
        .into_iter()
        .map(|stored| serde_json::to_value(stored.event).unwrap())
        .collect()
}

struct Fixture {
    port: u16,
    token: String,
    api: tokio::task::JoinHandle<Result<(), ads_daemon::api::ApiError>>,
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

async fn exchange(ws: &mut Socket, value: &Value) -> Value {
    bounded(ws.send(Message::Text(value.to_string().into())))
        .await
        .unwrap();
    let Message::Text(text) = bounded(ws.next()).await.unwrap().unwrap() else {
        panic!("expected text")
    };
    let reply: Value = serde_json::from_str(&text).unwrap();
    assert!(
        ads_contracts::reencode("urn:ads:contract:local-api.server-message:3", reply.clone())
            .unwrap()
            .is_ok(),
        "{reply}"
    );
    assert_eq!(reply["id"], value["id"]);
    reply
}

async fn authenticated(f: &Fixture) -> Socket {
    let mut ws = connect(f).await;
    let hello = exchange(&mut ws, &command("auth.hello", json!({"token": f.token}))).await;
    assert_eq!(hello["result"]["accepted"], true);
    ws
}

fn queue_params() -> Value {
    json!({"queue_id": "queue-01"})
}

async fn state(ws: &mut Socket) -> Value {
    let reply = exchange(ws, &command("queue.get", queue_params())).await;
    assert!(
        ads_contracts::reencode("urn:ads:command:queue.get.response:1", reply.clone())
            .unwrap()
            .is_ok()
    );
    reply["result"]["state"].clone()
}

#[tokio::test]
async fn start_records_the_owner_and_stop_always_succeeds() {
    let folder = Folder::new("queue-api-start");
    let anchor = Anchor::default();
    let queue: Arc<dyn QueueControl> = Arc::new(open_queue(&folder.db(), &anchor).unwrap());
    let f = fixture(Some(queue.clone())).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(state(&mut ws).await, "stopped");

    let stop = command("queue.stop", queue_params());
    assert_eq!(exchange(&mut ws, &stop).await["result"]["accepted"], true);
    assert_eq!(
        audit(&folder.db(), &anchor).len(),
        1,
        "a stopped queue gets no stop event"
    );

    let start = command("queue.start", queue_params());
    let reply = exchange(&mut ws, &start).await;
    assert_eq!(
        reply["result"],
        json!({"accepted": true, "replayed": false})
    );
    assert_eq!(state(&mut ws).await, "enabled");
    assert_eq!(
        queue.state(&id("queue-01")).unwrap(),
        RecordTaskQueueV1State::Enabled
    );

    let replay = exchange(&mut ws, &start).await;
    assert_eq!(replay["result"]["replayed"], true);
    let again = exchange(&mut ws, &command("queue.start", queue_params())).await;
    assert_eq!(again["error"]["code"], -32016);

    let events = audit(&folder.db(), &anchor);
    let enabled: Vec<_> = events
        .iter()
        .filter(|e| e["type"] == "urn:ads:event:queue.enabled:1")
        .collect();
    assert_eq!(enabled.len(), 1);
    let enabled = enabled[0];
    assert_eq!(
        enabled["actor"],
        json!({"kind": "device", "id": "device-01"})
    );
    assert_eq!(enabled["cause"], start["id"]);
    assert_eq!(
        enabled["data"],
        json!({"queue_id": "queue-01", "owner_id": "owner-01", "via": "local_ui"})
    );
    assert!(enabled["ts"].as_str().unwrap().ends_with('Z'));

    let stop = command("queue.stop", queue_params());
    assert_eq!(exchange(&mut ws, &stop).await["result"]["accepted"], true);
    assert_eq!(state(&mut ws).await, "stopped");
    let events = audit(&folder.db(), &anchor);
    let last = events.last().unwrap();
    assert_eq!(last["type"], "urn:ads:event:queue.stopped:1");
    assert_eq!(last["cause"], stop["id"]);
    assert_eq!(
        last["data"],
        json!({"queue_id": "queue-01", "reason": "owner", "owner_id": "owner-01", "via": "local_ui"})
    );
}

#[tokio::test]
async fn queue_commands_need_the_token_and_a_local_origin() {
    let folder = Folder::new("queue-api-refusals");
    let anchor = Anchor::default();
    let queue: Arc<dyn QueueControl> = Arc::new(open_queue(&folder.db(), &anchor).unwrap());
    let f = fixture(Some(queue.clone())).await;

    let mut unauthenticated = connect(&f).await;
    bounded(unauthenticated.send(Message::Text(
        command("queue.start", queue_params()).to_string().into(),
    )))
    .await
    .unwrap();
    assert!(matches!(
        bounded(unauthenticated.next()).await,
        None | Some(Err(_)) | Some(Ok(Message::Close(_)))
    ));

    let mut ws = authenticated(&f).await;
    for method in ["queue.get", "queue.start", "queue.stop"] {
        let reply = exchange(&mut ws, &remote(command(method, queue_params()))).await;
        assert_eq!(reply["error"]["code"], -32010, "{method}");
    }
    let unknown = exchange(
        &mut ws,
        &command("queue.start", json!({"queue_id": "queue-02"})),
    )
    .await;
    assert_eq!(unknown["error"]["code"], -32015);
    let forged = exchange(
        &mut ws,
        &command(
            "queue.start",
            json!({"queue_id": "queue-01", "owner_id": "owner-01"}),
        ),
    )
    .await;
    assert_eq!(forged["error"]["code"], -32602);

    assert_eq!(
        queue.state(&id("queue-01")).unwrap(),
        RecordTaskQueueV1State::Stopped
    );
    let events = audit(&folder.db(), &anchor);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "urn:ads:event:queue.created:1");
}

#[tokio::test]
async fn without_a_bound_queue_every_queue_command_is_refused() {
    let f = fixture(None).await;
    let mut ws = authenticated(&f).await;
    for method in ["queue.get", "queue.start", "queue.stop"] {
        let reply = exchange(&mut ws, &command(method, queue_params())).await;
        assert_eq!(reply["error"]["code"], -32015, "{method}");
    }
}

#[test]
fn reopening_restores_the_queue_stopped_and_records_the_restart() {
    let folder = Folder::new("queue-api-restart");
    let anchor = Anchor::default();
    let queue = open_queue(&folder.db(), &anchor).unwrap();
    queue.start(&id("queue-01"), &new_id().unwrap()).unwrap();
    drop(queue);

    let queue = open_queue(&folder.db(), &anchor).unwrap();
    assert_eq!(
        queue.state(&id("queue-01")).unwrap(),
        RecordTaskQueueV1State::Stopped
    );
    let events = audit(&folder.db(), &anchor);
    let kinds: Vec<_> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        [
            "urn:ads:event:queue.created:1",
            "urn:ads:event:queue.enabled:1",
            "urn:ads:event:queue.stopped:1"
        ]
    );
    assert_eq!(events[2]["actor"]["kind"], "daemon");
    assert_eq!(events[2]["data"]["reason"], "restart");
    drop(queue);

    let stranger = LoggedQueue::open(
        open_log(&folder.db(), &anchor),
        id("queue-01"),
        AuthenticatedDevice::local_owner(id("device-02"), id("owner-02")),
    );
    assert!(matches!(stranger, Err(QueueOpenError::ForeignOwner)));
    assert_eq!(audit(&folder.db(), &anchor).len(), 3);
}
