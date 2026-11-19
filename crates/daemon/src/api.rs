//! Composição dos comandos da API local autenticada com o host de terminais.

use crate::auth::{Token, permits_upgrade};
use crate::events::EventLog;
use crate::pty_client::{ClientError, ErrorCode, Event, PtyClient};
use ads_contracts::CommandSessionOpenV1Params;
use ads_ptyhost::wire::{
    Notification, OpenParams, ResizeParams, SessionId, SessionParams, WriteParams,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
use tokio::time::{Instant, timeout, timeout_at};
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiError {
    Invalid,
    Method,
    Remote,
    HarnessNotConfigured,
    Capacity,
    Conflict,
    Replay,
    Unavailable,
    Host(ErrorCode),
}
impl ApiError {
    pub fn code(self) -> i64 {
        match self {
            Self::Invalid => -32602,
            Self::Method => -32601,
            Self::Remote => -32010,
            Self::HarnessNotConfigured => -32011,
            Self::Capacity => -32003,
            Self::Conflict => -32012,
            Self::Replay => -32004,
            Self::Unavailable => -32603,
            Self::Host(code) => code.number(),
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Invalid => "invalid command",
            Self::Method => "unknown method",
            Self::Remote => "remote origin refused",
            Self::HarnessNotConfigured => "harness not configured",
            Self::Capacity => "capacity exceeded",
            Self::Conflict => "idempotency key conflict",
            Self::Replay => "replay unavailable",
            Self::Unavailable => "service unavailable",
            Self::Host(_) => "host refused command",
        }
    }
}
impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}
impl std::error::Error for ApiError {}

pub(crate) fn checked(schema: &str, value: Value) -> Result<Value, ApiError> {
    ads_contracts::reencode(schema, value)
        .ok_or(ApiError::Invalid)?
        .map_err(|_| ApiError::Invalid)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    pub ui_base_url: String,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    pub auth_timeout_ms: u64,
    pub max_sessions: usize,
    pub max_write_bytes: usize,
    pub events_per_stream: usize,
    pub idempotency_keys: usize,
    pub command_queue: usize,
    pub max_connections: usize,
    pub max_pending_auth: usize,
    pub accept_retry_ms: u64,
    pub max_message_bytes: usize,
    pub send_timeout_ms: u64,
    pub host_timeout_ms: u64,
}
impl ApiConfig {
    pub fn from_toml_str(overrides: &str) -> Result<Self, ApiError> {
        let mut base: toml::Value = toml::from_str(include_str!("../../../defaults/daemon.toml"))
            .map_err(|_| ApiError::Invalid)?;
        let over: toml::Value = toml::from_str(overrides).map_err(|_| ApiError::Invalid)?;
        base.as_table_mut()
            .ok_or(ApiError::Invalid)?
            .extend(over.as_table().ok_or(ApiError::Invalid)?.clone());
        let mut config: Self = base.try_into().map_err(|_| ApiError::Invalid)?;
        if config.allowed_origins.is_empty() {
            config
                .allowed_origins
                .push(config.ui_base_url.trim_end_matches('/').to_owned());
        }
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), ApiError> {
        let positive = [
            self.max_sessions,
            self.max_write_bytes,
            self.events_per_stream,
            self.idempotency_keys,
            self.command_queue,
            self.max_connections,
            self.max_pending_auth,
            self.max_message_bytes,
        ];
        if positive.iter().any(|n| *n == 0 || *n > 1_048_576)
            || self.max_write_bytes > 65536
            || self.max_message_bytes < 1024
            || [
                self.accept_retry_ms,
                self.auth_timeout_ms,
                self.send_timeout_ms,
                self.host_timeout_ms,
            ]
            .iter()
            .any(|n| *n == 0 || *n > 300_000)
            || !valid_ui_base(&self.ui_base_url)
            || self.allowed_origins.is_empty()
            || self.allowed_origins.iter().any(|s| {
                let Ok(uri) = s.parse::<tokio_tungstenite::tungstenite::http::Uri>() else {
                    return true;
                };
                !matches!(uri.scheme_str(), Some("http" | "https"))
                    || uri
                        .authority()
                        .is_none_or(|a| a.as_str().contains(['*', '@']))
                    || uri.path_and_query().is_some_and(|p| p.as_str() != "/")
                    || s.ends_with('/')
            })
        {
            return Err(ApiError::Invalid);
        }
        Ok(())
    }
}

fn valid_ui_base(base: &str) -> bool {
    let Ok(uri) = base.parse::<tokio_tungstenite::tungstenite::http::Uri>() else {
        return false;
    };
    uri.scheme_str() == Some("http")
        && matches!(uri.host(), Some("localhost" | "127.0.0.1" | "[::1]"))
        && uri
            .authority()
            .is_some_and(|a| !a.as_str().contains(['*', '@']))
        && uri.path_and_query().is_none_or(|p| p.as_str() == "/")
        && !base.contains('#')
}
fn host_error(error: ClientError) -> ApiError {
    match error {
        ClientError::Host { code, .. } => ApiError::Host(code),
        ClientError::Capacity => ApiError::Capacity,
        ClientError::SequenceOverflow | ClientError::InvalidConfig => ApiError::Invalid,
        _ => ApiError::Unavailable,
    }
}

pub trait Launcher: Send + Sync + 'static {
    fn prepare(&self, request: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError>;
}
pub struct DisabledLauncher;
impl Launcher for DisabledLauncher {
    fn prepare(&self, _: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError> {
        Err(ApiError::HarnessNotConfigured)
    }
}

struct Request {
    command: Value,
    reply: oneshot::Sender<Result<bool, ApiError>>,
}
struct Session {
    offset: u64,
    last_host_seq: u64,
    ended: bool,
    closing: bool,
    invalidated: bool,
    released: bool,
}
struct Entry {
    key: String,
    digest: [u8; 32],
}
struct Service {
    host: PtyClient,
    launcher: Arc<dyn Launcher>,
    config: ApiConfig,
    log: Arc<Mutex<EventLog>>,
    sessions: HashMap<String, Session>,
    keys: VecDeque<Entry>,
    invalidated: VecDeque<SessionId>,
    released: VecDeque<SessionId>,
}
impl Service {
    async fn command(&mut self, command: &Value) -> Result<bool, ApiError> {
        let method = command["method"].as_str().ok_or(ApiError::Invalid)?;
        let params = &command["params"];
        if params["_meta"]["origin"] != "local" {
            return Err(ApiError::Remote);
        }
        let key = params["_meta"]["idempotency_key"]
            .as_str()
            .ok_or(ApiError::Invalid)?;
        let digest: [u8; 32] = Sha256::digest(
            serde_json::to_vec(&json!([method, params])).map_err(|_| ApiError::Invalid)?,
        )
        .into();
        if let Some(entry) = self.keys.iter().find(|e| e.key == key) {
            return if entry.digest == digest {
                Ok(true)
            } else {
                Err(ApiError::Conflict)
            };
        }
        self.execute(method, params, command["id"].as_str()).await?;
        self.keys.push_back(Entry {
            key: key.to_owned(),
            digest,
        });
        while self.keys.len() > self.config.idempotency_keys {
            self.keys.pop_front();
        }
        Ok(false)
    }
    async fn execute(
        &mut self,
        method: &str,
        params: &Value,
        cause: Option<&str>,
    ) -> Result<(), ApiError> {
        let host = self.host.commands();
        let mut host_params = params.clone();
        host_params
            .as_object_mut()
            .ok_or(ApiError::Invalid)?
            .remove("_meta");
        let id = params["session_id"].as_str().unwrap_or("");
        match method {
            "session.open" => {
                if self.sessions.values().filter(|s| !s.released).count()
                    >= self.config.max_sessions
                {
                    return Err(ApiError::Capacity);
                }
                if self.sessions.contains_key(id) {
                    return Err(ApiError::Conflict);
                }
                let request: CommandSessionOpenV1Params =
                    serde_json::from_value(params.clone()).map_err(|_| ApiError::Invalid)?;
                let open = self.launcher.prepare(&request)?;
                if open.session_id != request.session_id
                    || open.cols.get() != request.cols.get()
                    || open.rows.get() != request.rows.get()
                {
                    return Err(ApiError::Invalid);
                }
                self.wait_host(host.open(open)).await?;
                self.sessions.insert(
                    id.to_owned(),
                    Session {
                        offset: 0,
                        last_host_seq: 0,
                        ended: false,
                        closing: false,
                        invalidated: false,
                        released: false,
                    },
                );
                let setup = async {
                    self.log.lock().await.append(id, "created", json!({"session_id": id, "task_id": null, "harness": params["harness"], "mode": "interactive", "cols": params["cols"], "rows": params["rows"], "workdir_alias": params["workdir_alias"]}), cause)?;
                    self.wait_host(host.subscribe(request.session_id.clone(), Some(0))).await?;
                    Ok(())
                }.await;
                if let Err(error) = setup {
                    let cleanup = self
                        .wait_host(host.close(SessionParams {
                            session_id: request.session_id,
                        }))
                        .await;
                    self.sessions.remove(id);
                    self.log.lock().await.remove(id);
                    cleanup?;
                    return Err(error);
                }
            }
            "session.write" => {
                self.active(id)?;
                let write: WriteParams =
                    serde_json::from_value(host_params).map_err(|_| ApiError::Invalid)?;
                let bytes = STANDARD
                    .decode(params["data"].as_str().ok_or(ApiError::Invalid)?)
                    .map_err(|_| ApiError::Invalid)?;
                if bytes.is_empty() || bytes.len() > self.config.max_write_bytes {
                    return Err(ApiError::Capacity);
                }
                self.wait_host(host.write(write)).await?;
            }
            "session.resize" => {
                self.active(id)?;
                let resize: ResizeParams =
                    serde_json::from_value(host_params).map_err(|_| ApiError::Invalid)?;
                self.wait_host(host.resize(resize)).await?;
                self.log.lock().await.append(
                    id,
                    "resized",
                    json!({"session_id": id, "cols": params["cols"], "rows": params["rows"]}),
                    cause,
                )?;
            }
            "session.close" => {
                let close: SessionParams =
                    serde_json::from_value(host_params).map_err(|_| ApiError::Invalid)?;
                self.close(close).await?;
            }
            "events.subscribe" => {
                self.log.lock().await.after(
                    params["stream"].as_str().ok_or(ApiError::Invalid)?,
                    params["after_seq"].as_u64().ok_or(ApiError::Invalid)?,
                )?;
            }
            _ => return Err(ApiError::Method),
        }
        Ok(())
    }
    async fn close(&mut self, params: SessionParams) -> Result<(), ApiError> {
        let id = params.session_id.clone();
        let session = self
            .sessions
            .get_mut(id.as_str())
            .ok_or(ApiError::Invalid)?;
        if session.invalidated && session.released {
            return Ok(());
        }
        if session.ended || session.closing || session.released {
            return Err(ApiError::Invalid);
        }
        session.closing = true;
        let host = self.host.commands();
        let result = self.wait_host(host.close(params)).await;
        let session = self
            .sessions
            .get_mut(id.as_str())
            .ok_or(ApiError::Invalid)?;
        session.closing = false;
        if result.is_ok() || result == Err(ApiError::Host(ErrorCode::UnknownSession)) {
            session.released = true;
            self.released.push_back(id);
            if self.released.len() > self.config.max_sessions
                && let Some(expired) = self.released.pop_front()
            {
                self.sessions.remove(expired.as_str());
                self.log.lock().await.remove(expired.as_str());
            }
        }
        result.map(|_| ())
    }
    async fn invalidate(&mut self, id: SessionId) {
        if let Some(session) = self.sessions.get_mut(id.as_str())
            && !session.invalidated
        {
            session.invalidated = true;
            // A invalidação pode ocorrer enquanto o daemon espera outra resposta do host.
            if !session.released {
                self.invalidated.push_back(id.clone());
            }
            self.log.lock().await.invalidate(id.as_str());
        }
    }
    async fn wait_host<T>(
        &mut self,
        future: impl Future<Output = Result<T, ClientError>>,
    ) -> Result<T, ApiError> {
        tokio::pin!(future);
        loop {
            tokio::select! {
                result = &mut future => return result.map_err(host_error),
                event = self.host.next() => self.event(event.map_err(host_error)?).await?,
            }
        }
    }
    fn active(&self, id: &str) -> Result<(), ApiError> {
        if self
            .sessions
            .get(id)
            .is_some_and(|s| !s.ended && !s.closing && !s.invalidated && !s.released)
        {
            Ok(())
        } else {
            Err(ApiError::Invalid)
        }
    }
    async fn event(&mut self, event: Event) -> Result<(), ApiError> {
        match event {
            Event::Output { header, bytes } => {
                let id = header.session_id.as_str();
                let Some(session) = self.sessions.get_mut(id) else {
                    return Ok(());
                };
                if session.ended || session.invalidated {
                    return Ok(());
                }
                if header.seq.get() != session.last_host_seq + 1 {
                    self.invalidate(header.session_id).await;
                    return Ok(());
                }
                self.log.lock().await.append(id, "output", json!({"session_id": id, "offset": session.offset, "length": bytes.len(), "encoding": "base64", "payload": STANDARD.encode(&bytes)}), None)?;
                session.offset = session
                    .offset
                    .checked_add(bytes.len() as u64)
                    .ok_or(ApiError::Capacity)?;
                session.last_host_seq = header.seq.get();
            }
            Event::Notification(Notification::SessionExited(exit)) => {
                let id = exit.session_id.as_str();
                let Some(session) = self.sessions.get_mut(id) else {
                    return Ok(());
                };
                if session.ended || session.invalidated {
                    return Ok(());
                }
                if exit.last_seq != session.last_host_seq {
                    self.invalidate(exit.session_id).await;
                    return Ok(());
                }
                session.ended = true;
                self.log.lock().await.append(id, "ended", json!({"session_id": id, "reason": if session.closing || session.released { "killed" } else if exit.exit_code.is_some() { "exit" } else { "error" }, "exit_code": exit.exit_code}), None)?;
            }
            Event::Notification(Notification::SubscriptionClosed(closed))
                if closed.reason == ads_ptyhost::wire::CloseReason::SessionClosed =>
            {
                let id = closed.session_id.as_str();
                let Some(session) = self.sessions.get_mut(id) else {
                    return Ok(());
                };
                if !session.ended && !session.invalidated {
                    if closed.last_seq != session.last_host_seq {
                        self.invalidate(closed.session_id).await;
                        return Ok(());
                    }
                    session.ended = true;
                    self.log.lock().await.append(
                        id,
                        "ended",
                        json!({"session_id": id, "reason": "killed", "exit_code": null}),
                        None,
                    )?;
                }
            }
            Event::ResubscribeFailed { session_id, error }
                if self
                    .sessions
                    .get(session_id.as_str())
                    .is_some_and(|s| !s.ended) =>
            {
                self.invalidate(session_id).await;
                if host_error(error) == ApiError::Unavailable {
                    return Err(ApiError::Unavailable);
                }
            }
            _ => (),
        }
        Ok(())
    }
}

pub struct LocalApi {
    listener: TcpListener,
    token: Token,
    config: ApiConfig,
    service: Service,
}
impl LocalApi {
    pub async fn bind(
        config: ApiConfig,
        host: PtyClient,
        launcher: Arc<dyn Launcher>,
    ) -> Result<Self, ApiError> {
        config.validate()?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| ApiError::Unavailable)?;
        host.enable_backpressure();
        let token = Token::generate()?;
        let log = Arc::new(Mutex::new(EventLog::new(config.events_per_stream)));
        let service = Service {
            host,
            launcher,
            config: config.clone(),
            log,
            sessions: HashMap::new(),
            keys: VecDeque::new(),
            invalidated: VecDeque::new(),
            released: VecDeque::new(),
        };
        Ok(Self {
            listener,
            token,
            config,
            service,
        })
    }
    pub fn port(&self) -> Result<u16, ApiError> {
        self.listener
            .local_addr()
            .map(|a| a.port())
            .map_err(|_| ApiError::Unavailable)
    }
    pub fn launch_url(&self) -> Result<String, ApiError> {
        Ok(self
            .token
            .launch_url(&self.config.ui_base_url, self.port()?))
    }
    pub async fn run(self) -> Result<(), ApiError> {
        let Self {
            listener,
            token,
            config,
            mut service,
        } = self;
        let token = Arc::new(token);
        let config = Arc::new(config);
        let port = listener
            .local_addr()
            .map_err(|_| ApiError::Unavailable)?
            .port();
        let log = service.log.clone();
        let permits = Arc::new(Semaphore::new(config.max_connections));
        let pending = Arc::new(Semaphore::new(config.max_pending_auth));
        let (commands, mut incoming) = mpsc::channel::<Request>(config.command_queue);
        let mut connections = tokio::task::JoinSet::new();
        let mut accepting = Box::pin(accept_retry(&listener, config.accept_retry_ms));
        loop {
            while let Some(session_id) = service.invalidated.pop_front() {
                let outcome = timeout(
                    Duration::from_millis(config.host_timeout_ms),
                    service.close(SessionParams { session_id }),
                )
                .await;
                if matches!(outcome, Err(_) | Ok(Err(ApiError::Unavailable))) {
                    return Err(ApiError::Unavailable);
                }
            }
            tokio::select! {
                event = service.host.next() => {
                    if service.event(event.map_err(host_error)?).await == Err(ApiError::Unavailable) { return Err(ApiError::Unavailable); }
                },
                Some(request) = incoming.recv() => {
                    let outcome = timeout(Duration::from_millis(config.host_timeout_ms), service.command(&request.command)).await;
                    match outcome {
                        Ok(result) => {
                            let _ = request.reply.send(result);
                            if result == Err(ApiError::Unavailable) { return Err(ApiError::Unavailable); }
                        }
                        // Uma chamada cancelada pode ter executado; encerrar impede repetir efeito incerto.
                        Err(_) => return Err(ApiError::Unavailable),
                    }
                }
                accepted = &mut accepting => {
                    accepting.set(accept_retry(&listener, config.accept_retry_ms));
                    let (stream, _) = accepted;
                    if let Ok(permit) = pending.clone().try_acquire_owned() {
                        let (token, config, commands, log) = (token.clone(), config.clone(), commands.clone(), log.clone());
                        let authenticated = permits.clone();
                        connections.spawn(async move {
                            let _ = connection(stream, port, token, config, commands, log, (permit, authenticated)).await;
                        });
                    }
                }
                Some(_) = connections.join_next(), if !connections.is_empty() => (),
            }
        }
    }
}

async fn retry_io<T, F, Fut>(mut operation: F, delay_ms: u64) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<T>>,
{
    loop {
        match operation().await {
            Ok(value) => return value,
            Err(_) => {
                eprintln!("ads-daemon: local accept failed; retrying");
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}
async fn accept_retry(listener: &TcpListener, delay_ms: u64) -> (TcpStream, std::net::SocketAddr) {
    retry_io(|| listener.accept(), delay_ms).await
}

fn response(id: &Value, result: Result<bool, ApiError>) -> Value {
    match result {
        Ok(replayed) => {
            json!({"jsonrpc": "2.0", "id": id, "result": {"accepted": true, "replayed": replayed}})
        }
        Err(error) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": error.code(), "message": error.message()}})
        }
    }
}
fn validate_command(value: Value) -> Result<Value, ApiError> {
    let method = value["method"].as_str().ok_or(ApiError::Invalid)?;
    if !matches!(
        method,
        "auth.hello"
            | "session.open"
            | "session.write"
            | "session.resize"
            | "session.close"
            | "events.subscribe"
    ) {
        return Err(ApiError::Method);
    }
    checked(&format!("urn:ads:command:{method}:1"), value)
}

fn consume_hello(hello: &mut Option<Value>) -> Result<Value, ApiError> {
    let mut value = hello.take().ok_or(ApiError::Invalid)?;
    let id = value["id"].take();
    if let Value::String(secret) = value["params"]["token"].take() {
        drop(Zeroizing::new(secret));
    }
    Ok(id)
}

#[allow(clippy::result_large_err)] // A assinatura do callback é definida pela biblioteca WebSocket.
async fn connection(
    stream: TcpStream,
    port: u16,
    token: Arc<Token>,
    config: Arc<ApiConfig>,
    commands: mpsc::Sender<Request>,
    log: Arc<Mutex<EventLog>>,
    permits: (tokio::sync::OwnedSemaphorePermit, Arc<Semaphore>),
) -> Result<(), ApiError> {
    use tokio_tungstenite::tungstenite::handshake::server::{Request as HttpRequest, Response};
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(config.max_message_bytes))
        .max_frame_size(Some(config.max_message_bytes));
    let deadline = Instant::now() + Duration::from_millis(config.auth_timeout_ms);
    let mut ws = timeout_at(
        deadline,
        tokio_tungstenite::accept_hdr_async_with_config(
            stream,
            |request: &HttpRequest, response: Response| {
                if permits_upgrade(request, port, &config.allowed_origins) {
                    Ok(response)
                } else {
                    Err(tokio_tungstenite::tungstenite::http::Response::builder()
                        .status(403)
                        .body(None)
                        .expect("static response"))
                }
            },
            Some(ws_config),
        ),
    )
    .await
    .map_err(|_| ApiError::Unavailable)?
    .map_err(|_| ApiError::Invalid)?;
    let first = timeout_at(deadline, ws.next()).await;
    let mut hello = match first {
        Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| validate_command(v).ok()),
        _ => None,
    };
    let valid = hello.as_ref().is_some_and(|v| {
        v["method"] == "auth.hello"
            && v["params"]["_meta"]["origin"] == "local"
            && v["params"]["token"]
                .as_str()
                .is_some_and(|s| token.matches(s))
    });
    if !valid {
        let _ = timeout(
            Duration::from_millis(config.send_timeout_ms),
            ws.close(None),
        )
        .await;
        return Err(ApiError::Invalid);
    }
    let id = consume_hello(&mut hello)?;
    let (pending, authenticated) = permits;
    let _authenticated = authenticated
        .try_acquire_owned()
        .map_err(|_| ApiError::Capacity)?;
    drop(pending);
    send(&mut ws, response(&id, Ok(false)), &config).await?;
    let mut changed = log.lock().await.watch();
    let mut subscriptions = HashMap::<String, (u64, Value)>::new();
    loop {
        let message = tokio::select! {
            message = ws.next() => Some(message),
            result = changed.changed() => { result.map_err(|_| ApiError::Unavailable)?; None }
        };
        if let Some(message) = message {
            let value = match message {
                Some(Ok(Message::Text(text))) => {
                    serde_json::from_str::<Value>(&text).map_err(|_| ApiError::Invalid)?
                }
                Some(Ok(Message::Ping(bytes))) => {
                    timeout(
                        Duration::from_millis(config.send_timeout_ms),
                        ws.send(Message::Pong(bytes)),
                    )
                    .await
                    .map_err(|_| ApiError::Unavailable)?
                    .map_err(|_| ApiError::Unavailable)?;
                    continue;
                }
                Some(Ok(Message::Pong(_))) => continue,
                _ => return Ok(()),
            };
            let id = value["id"].clone();
            checked("urn:ads:common:ulid:1", id.clone())?;
            match validate_command(value) {
                Err(error) => send(&mut ws, response(&id, Err(error)), &config).await?,
                Ok(command) => {
                    if command["method"] == "auth.hello" {
                        return Err(ApiError::Invalid);
                    }
                    let subscribe = command["method"] == "events.subscribe";
                    let stream = command["params"]["stream"]
                        .as_str()
                        .unwrap_or("")
                        .to_owned();
                    if subscribe
                        && !subscriptions.contains_key(&stream)
                        && subscriptions.len() >= config.max_sessions
                    {
                        send(&mut ws, response(&id, Err(ApiError::Capacity)), &config).await?;
                        continue;
                    }
                    let after = command["params"]["after_seq"].as_u64().unwrap_or(0);
                    let (reply, answer) = oneshot::channel();
                    commands
                        .send(Request { command, reply })
                        .await
                        .map_err(|_| ApiError::Unavailable)?;
                    let outcome = answer.await.map_err(|_| ApiError::Unavailable)?;
                    send(&mut ws, response(&id, outcome), &config).await?;
                    if subscribe && outcome.is_ok() {
                        subscriptions.insert(stream, (after, id));
                    }
                }
            }
        }
        for (stream, (after, subscription_id)) in &mut subscriptions {
            let events = log.lock().await.after(stream, *after);
            let events = match events {
                Ok(events) => events,
                Err(error) => {
                    send(&mut ws, response(subscription_id, Err(error)), &config).await?;
                    let _ = timeout(
                        Duration::from_millis(config.send_timeout_ms),
                        ws.close(None),
                    )
                    .await;
                    return Err(error);
                }
            };
            for event in events {
                *after = event["seq"].as_u64().ok_or(ApiError::Unavailable)?;
                send(&mut ws, event, &config).await?;
            }
        }
    }
}

async fn send(
    ws: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    value: Value,
    config: &ApiConfig,
) -> Result<(), ApiError> {
    let value = checked("urn:ads:contract:local-api.server-message:1", value)?;
    timeout(
        Duration::from_millis(config.send_timeout_ms),
        ws.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| ApiError::Unavailable)?
    .map_err(|_| ApiError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_credentials_never_escape_validation_errors() {
        let token = Token::generate().unwrap();
        let url = token.launch_url("http://localhost:5173", 1234);
        let secret = url.split_once("&token=").unwrap().1;
        let terminal = "private-terminal-output-sentinel";
        let value = json!({"jsonrpc": "2.0", "id": "01ARZ3NDEKTSV4RRFFQ69G5FAB", "method": "auth.hello", "params": {"token": secret, "terminal": terminal, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}}});
        let error = validate_command(value).unwrap_err();
        let diagnostics = format!("{error:?} {error} {token:?}");
        assert!(!diagnostics.contains(secret));
        assert!(!diagnostics.contains(terminal));
    }
    #[tokio::test]
    async fn transient_accept_errors_retry_with_delay() {
        let mut attempts = 0;
        let start = Instant::now();
        let value = retry_io(
            || {
                attempts += 1;
                std::future::ready(if attempts < 3 {
                    Err(std::io::Error::from(std::io::ErrorKind::ConnectionAborted))
                } else {
                    Ok(42)
                })
            },
            10,
        )
        .await;
        assert_eq!(value, 42);
        assert_eq!(attempts, 3);
        assert!(start.elapsed() >= Duration::from_millis(20));
    }

    #[test]
    fn hello_is_consumed_before_the_connection_loop() {
        let mut hello = Some(json!({"id": "request", "params": {"token": "credential-sentinel"}}));
        assert_eq!(consume_hello(&mut hello).unwrap(), "request");
        assert!(hello.is_none());
    }
    #[tokio::test]
    async fn failed_created_event_closes_the_opened_host_session() {
        use crate::pty_client::ClientConfig;
        use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
        struct FakeLauncher;
        impl Launcher for FakeLauncher {
            fn prepare(
                &self,
                request: &CommandSessionOpenV1Params,
            ) -> Result<OpenParams, ApiError> {
                serde_json::from_value(json!({"session_id": request.session_id, "cols": request.cols, "rows": request.rows, "program": "fake", "args": [], "env": {}, "cwd": null})).map_err(|_| ApiError::Invalid)
            }
        }
        let backend = Arc::new(FakeBackend::new());
        let host = Arc::new(PtyHost::new(backend.clone(), HostConfig::defaults()).unwrap());
        let (near, far) = tokio::io::duplex(4096);
        let peer = tokio::spawn(server::connection(host.clone(), far));
        let client = PtyClient::over(near, ClientConfig::recommended(host.config()))
            .await
            .unwrap();
        let mut api = LocalApi::bind(
            ApiConfig::from_toml_str("").unwrap(),
            client,
            Arc::new(FakeLauncher),
        )
        .await
        .unwrap();
        api.service.log.lock().await.exhaust_sequence("session");
        let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
        assert_eq!(
            api.service.execute("session.open", &params, None).await,
            Err(ApiError::Capacity)
        );
        assert!(backend.terminals()[0].was_killed());
        assert!(host.sessions().is_empty());
        assert!(api.service.sessions.is_empty());
        assert!(
            api.service
                .log
                .lock()
                .await
                .after("session/session", 0)
                .unwrap()
                .is_empty()
        );
        peer.abort();
    }
    #[tokio::test]
    async fn bounded_host_queue_survives_a_paused_consumer() {
        use crate::pty_client::ClientConfig;
        use ads_ptyhost::wire::{self, Call, ResultBody};
        use tokio::io::AsyncWriteExt;
        let (near, mut far) = tokio::io::duplex(4096);
        let (begin, ready) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let wire::Frame::Json(payload) =
                wire::read_frame(&mut far, 131072).await.unwrap().unwrap()
            else {
                panic!()
            };
            let hello = wire::Request::parse(&payload).unwrap();
            far.write_all(
                &wire::encode_json(&wire::Response {
                    id: Some(hello.id),
                    outcome: Ok(ResultBody::Hello(wire::HelloResult {
                        contract: Default::default(),
                    })),
                })
                .unwrap(),
            )
            .await
            .unwrap();
            ready.await.unwrap();
            let id = "unknown".to_owned().try_into().unwrap();
            for seq in 1..=40 {
                far.write_all(&wire::encode_output(&id, seq, b"terminal-sentinel").unwrap())
                    .await
                    .unwrap();
            }
            let wire::Frame::Json(payload) =
                wire::read_frame(&mut far, 131072).await.unwrap().unwrap()
            else {
                panic!()
            };
            let request = wire::Request::parse(&payload).unwrap();
            assert!(matches!(request.call, Call::List(_)));
            far.write_all(
                &wire::encode_json(&wire::Response {
                    id: Some(request.id),
                    outcome: Ok(ResultBody::SessionList(wire::SessionList {
                        sessions: vec![],
                    })),
                })
                .unwrap(),
            )
            .await
            .unwrap();
            std::future::pending::<()>().await;
        });
        let client = PtyClient::over(
            near,
            ClientConfig {
                max_frame_bytes: 131072,
                queue_capacity: 2,
            },
        )
        .await
        .unwrap();
        let mut api = LocalApi::bind(
            ApiConfig::from_toml_str("").unwrap(),
            client,
            Arc::new(DisabledLauncher),
        )
        .await
        .unwrap();
        begin.send(()).unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        let host = api.service.host.commands();
        let result = timeout(Duration::from_secs(2), api.service.wait_host(host.list()))
            .await
            .unwrap();
        peer.abort();
        assert!(result.unwrap().sessions.is_empty());
    }
}
