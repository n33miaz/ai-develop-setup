//! Composição dos comandos da API local autenticada com o host de terminais.

use crate::auth::{Token, permits_upgrade};
use crate::events::EventLog;
use crate::pty_client::{Event, PtyClient};
use ads_contracts::CommandSessionOpenV1Params;
use ads_ptyhost::wire::{Notification, OpenParams, ResizeParams, SessionParams, WriteParams};
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
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};

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
    pub allowed_origins: Vec<String>,
    pub auth_timeout_ms: u64,
    pub max_sessions: usize,
    pub max_write_bytes: usize,
    pub events_per_stream: usize,
    pub idempotency_keys: usize,
    pub command_queue: usize,
    pub max_connections: usize,
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
        let config: Self = base.try_into().map_err(|_| ApiError::Invalid)?;
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
            self.max_message_bytes,
        ];
        if positive.iter().any(|n| *n == 0 || *n > 1_048_576)
            || self.max_write_bytes > 65536
            || self.max_message_bytes < 1024
            || [
                self.auth_timeout_ms,
                self.send_timeout_ms,
                self.host_timeout_ms,
            ]
            .iter()
            .any(|n| *n == 0 || *n > 300_000)
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
        let mut host_params = params.clone();
        host_params
            .as_object_mut()
            .ok_or(ApiError::Invalid)?
            .remove("_meta");
        let id = params["session_id"].as_str().unwrap_or("");
        match method {
            "session.open" => {
                if self.sessions.len() >= self.config.max_sessions {
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
                self.host
                    .open(open)
                    .await
                    .map_err(|_| ApiError::Unavailable)?;
                self.sessions.insert(
                    id.to_owned(),
                    Session {
                        offset: 0,
                        last_host_seq: 0,
                        ended: false,
                        closing: false,
                    },
                );
                self.log.lock().await.append(id, "created", json!({"session_id": id, "task_id": null, "harness": params["harness"], "mode": "interactive", "cols": params["cols"], "rows": params["rows"], "workdir_alias": params["workdir_alias"]}), cause)?;
                self.host
                    .subscribe(request.session_id, Some(0))
                    .await
                    .map_err(|_| ApiError::Unavailable)?;
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
                self.host
                    .write(write)
                    .await
                    .map_err(|_| ApiError::Unavailable)?;
            }
            "session.resize" => {
                self.active(id)?;
                let resize: ResizeParams =
                    serde_json::from_value(host_params).map_err(|_| ApiError::Invalid)?;
                self.host
                    .resize(resize)
                    .await
                    .map_err(|_| ApiError::Unavailable)?;
                self.log.lock().await.append(
                    id,
                    "resized",
                    json!({"session_id": id, "cols": params["cols"], "rows": params["rows"]}),
                    cause,
                )?;
            }
            "session.close" => {
                self.active(id)?;
                let close: SessionParams =
                    serde_json::from_value(host_params).map_err(|_| ApiError::Invalid)?;
                self.host
                    .close(close)
                    .await
                    .map_err(|_| ApiError::Unavailable)?;
                self.sessions.get_mut(id).ok_or(ApiError::Invalid)?.closing = true;
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
    fn active(&self, id: &str) -> Result<(), ApiError> {
        if self
            .sessions
            .get(id)
            .is_some_and(|s| !s.ended && !s.closing)
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
                let session = self.sessions.get_mut(id).ok_or(ApiError::Unavailable)?;
                if session.ended || header.seq.get() != session.last_host_seq + 1 {
                    return Err(ApiError::Replay);
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
                if session.ended {
                    return Ok(());
                }
                if exit.last_seq != session.last_host_seq {
                    return Err(ApiError::Replay);
                }
                session.ended = true;
                self.log.lock().await.append(id, "ended", json!({"session_id": id, "reason": if session.closing { "killed" } else if exit.exit_code.is_some() { "exit" } else { "error" }, "exit_code": exit.exit_code}), None)?;
            }
            Event::Notification(Notification::SubscriptionClosed(closed))
                if closed.reason == ads_ptyhost::wire::CloseReason::SessionClosed =>
            {
                let id = closed.session_id.as_str();
                let Some(session) = self.sessions.get_mut(id) else {
                    return Ok(());
                };
                if !session.ended {
                    if closed.last_seq != session.last_host_seq {
                        return Err(ApiError::Replay);
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
            Event::ResubscribeFailed { session_id, .. }
                if self
                    .sessions
                    .get(session_id.as_str())
                    .is_some_and(|s| !s.ended) =>
            {
                return Err(ApiError::Unavailable);
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
        let token = Token::generate()?;
        let log = Arc::new(Mutex::new(EventLog::new(config.events_per_stream)));
        let service = Service {
            host,
            launcher,
            config: config.clone(),
            log,
            sessions: HashMap::new(),
            keys: VecDeque::new(),
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
        Ok(self.token.launch_url(self.port()?))
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
        let (commands, mut incoming) = mpsc::channel::<Request>(config.command_queue);
        let mut connections = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                event = service.host.next() => service.event(event.map_err(|_| ApiError::Unavailable)?).await?,
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
                accepted = listener.accept() => {
                    let (stream, _) = accepted.map_err(|_| ApiError::Unavailable)?;
                    if let Ok(permit) = permits.clone().try_acquire_owned() {
                        let (token, config, commands, log) = (token.clone(), config.clone(), commands.clone(), log.clone());
                        connections.spawn(async move {
                            let _permit = permit;
                            let _ = connection(stream, port, token, config, commands, log).await;
                        });
                    }
                }
                Some(_) = connections.join_next(), if !connections.is_empty() => (),
            }
        }
    }
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

#[allow(clippy::result_large_err)] // A assinatura do callback é definida pela biblioteca WebSocket.
async fn connection(
    stream: TcpStream,
    port: u16,
    token: Arc<Token>,
    config: Arc<ApiConfig>,
    commands: mpsc::Sender<Request>,
    log: Arc<Mutex<EventLog>>,
) -> Result<(), ApiError> {
    use tokio_tungstenite::tungstenite::handshake::server::{Request as HttpRequest, Response};
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(config.max_message_bytes))
        .max_frame_size(Some(config.max_message_bytes));
    let deadline = Duration::from_millis(config.auth_timeout_ms);
    let mut ws = timeout(
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
    let first = timeout(deadline, ws.next()).await;
    let hello = match first {
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
    let hello = hello.ok_or(ApiError::Invalid)?;
    send(&mut ws, response(&hello["id"], Ok(false)), &config).await?;
    let mut changed = log.lock().await.watch();
    let mut subscriptions = HashMap::<String, u64>::new();
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
                        subscriptions.insert(stream, after);
                    }
                }
            }
        }
        for (stream, after) in &mut subscriptions {
            let events = log.lock().await.after(stream, *after)?;
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
        let secret = "private-credential-sentinel";
        let value = json!({"jsonrpc": "2.0", "id": "01ARZ3NDEKTSV4RRFFQ69G5FAB", "method": "auth.hello", "params": {"token": secret, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}}});
        let error = validate_command(value).unwrap_err();
        assert!(!format!("{error:?} {error}").contains(secret));
    }
}
