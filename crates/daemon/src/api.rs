//! Composição dos comandos da API local autenticada com o host de terminais.

use crate::approvals::{ApprovalControl, DisabledApprovals};
use crate::auth::{LocalSession, Token, permits_upgrade};
use crate::events::{Closing, EventLog, Poll, Stream};
use crate::pty_client::{ClientError, ErrorCode, Event, PtyClient};
use crate::queue::{DisabledQueue, QueueControl};
use ads_contracts::{
    CommandApprovalAnswerV1Params, CommandSessionOpenV1Params,
    ContractLocalApiErrorCodeV3ApprovalAlreadyAnswered as ApprovalAlreadyAnswered,
    ContractLocalApiErrorCodeV3ApprovalNotApprovable as ApprovalNotApprovable,
    ContractLocalApiErrorCodeV3ApprovalNotConfigured as ApprovalNotConfigured,
    ContractLocalApiErrorCodeV3ApprovalNotFound as ApprovalNotFound,
    ContractLocalApiErrorCodeV3ApprovalOutOfOrder as ApprovalOutOfOrder,
    ContractLocalApiErrorCodeV3ApprovalPolicyChanged as ApprovalPolicyChanged,
    ContractLocalApiErrorCodeV3ApprovalStale as ApprovalStale,
    ContractLocalApiErrorCodeV3CapacityExceeded as CapacityExceeded,
    ContractLocalApiErrorCodeV3Conflict as Conflict,
    ContractLocalApiErrorCodeV3HarnessNotConfigured as HarnessNotConfigured,
    ContractLocalApiErrorCodeV3InternalError as InternalError,
    ContractLocalApiErrorCodeV3InvalidParams as InvalidParams,
    ContractLocalApiErrorCodeV3MethodNotFound as MethodNotFound,
    ContractLocalApiErrorCodeV3QueueAlreadyEnabled as QueueAlreadyEnabled,
    ContractLocalApiErrorCodeV3QueueNotConfigured as QueueNotConfigured,
    ContractLocalApiErrorCodeV3RemoteOriginRefused as RemoteOriginRefused,
    ContractLocalApiErrorCodeV3SessionClosePending as SessionClosePending,
    ContractLocalApiErrorCodeV3SessionSetupFailed as SessionSetupFailed,
    ContractLocalApiErrorCodeV3SubscriptionReplayUnavailable as SubscriptionReplayUnavailable,
};
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
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify, Semaphore, mpsc, oneshot};
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
    ClosePending,
    /// A preparação de um `session.open` já aberto no host falhou; o fechamento fica agendado.
    SetupFailed,
    /// Prazo do host esgotado num comando; o efeito é incerto e o serviço encerra.
    HostTimeout,
    Host(ErrorCode),
    /// No queue bound to the audit log, or a queue id it does not serve.
    QueueNotConfigured,
    QueueAlreadyEnabled,
    /// The audit log refused a queue or gate record; unlike `Unavailable`, the service keeps running.
    Storage,
    StorageBusy,
    /// No gate bound to the audit log.
    ApprovalNotConfigured,
    /// No action of a gate served here with that id, or no approval asked for it.
    ApprovalNotFound,
    /// The action no longer waits for an answer.
    ApprovalAlreadyAnswered,
    /// Another approval or plan hash than the action's, or a validity that is over.
    ApprovalStale,
    /// The policy or the clock tolerance changed since the plan; an approval became a denial.
    ApprovalPolicyChanged,
    /// The core clock is before the last step of the action; nothing was decided.
    ApprovalOutOfOrder,
    /// The record does not fix the concrete effect of the plan: the API does not approve it.
    ApprovalNotApprovable,
}
impl ApiError {
    pub(crate) fn from_storage(error: ads_storage::StorageError) -> Self {
        match error {
            ads_storage::StorageError::Busy => Self::StorageBusy,
            _ => Self::Storage,
        }
    }

    pub fn code(self) -> i64 {
        match self {
            Self::Invalid => InvalidParams::VALUE,
            Self::Method => MethodNotFound::VALUE,
            Self::Remote => RemoteOriginRefused::VALUE,
            Self::HarnessNotConfigured => HarnessNotConfigured::VALUE,
            Self::Capacity => CapacityExceeded::VALUE,
            Self::Conflict => Conflict::VALUE,
            Self::Replay => SubscriptionReplayUnavailable::VALUE,
            Self::ClosePending => SessionClosePending::VALUE,
            Self::SetupFailed => SessionSetupFailed::VALUE,
            Self::Unavailable | Self::HostTimeout | Self::Storage | Self::StorageBusy => {
                InternalError::VALUE
            }
            Self::QueueNotConfigured => QueueNotConfigured::VALUE,
            Self::QueueAlreadyEnabled => QueueAlreadyEnabled::VALUE,
            Self::ApprovalNotConfigured => ApprovalNotConfigured::VALUE,
            Self::ApprovalNotFound => ApprovalNotFound::VALUE,
            Self::ApprovalAlreadyAnswered => ApprovalAlreadyAnswered::VALUE,
            Self::ApprovalStale => ApprovalStale::VALUE,
            Self::ApprovalPolicyChanged => ApprovalPolicyChanged::VALUE,
            Self::ApprovalOutOfOrder => ApprovalOutOfOrder::VALUE,
            Self::ApprovalNotApprovable => ApprovalNotApprovable::VALUE,
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
            Self::ClosePending => "session close pending",
            Self::SetupFailed => "session setup failed",
            Self::HostTimeout => "service unavailable",
            Self::Host(_) => "host refused command",
            Self::QueueNotConfigured => "queue not configured",
            Self::QueueAlreadyEnabled => "queue already enabled",
            Self::Storage => "service unavailable",
            Self::StorageBusy => "audit log busy",
            Self::ApprovalNotConfigured => "approvals not configured",
            Self::ApprovalNotFound => "approval not found",
            Self::ApprovalAlreadyAnswered => "approval already answered",
            Self::ApprovalStale => "approval stale",
            Self::ApprovalPolicyChanged => "policy changed since the plan",
            Self::ApprovalOutOfOrder => "clock before the last step",
            Self::ApprovalNotApprovable => "approval not allowed for this plan",
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
    #[serde(skip)]
    desktop: bool,
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
    pub retired_sessions: usize,
    pub subscription_ids_per_stream: usize,
    pub close_retries: usize,
    pub close_retry_ms: u64,
}
impl ApiConfig {
    pub fn for_desktop(mut self) -> Self {
        self.desktop = true;
        self
    }

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
            self.retired_sessions,
            self.subscription_ids_per_stream,
            self.close_retries,
        ];
        if positive.iter().any(|n| *n == 0 || *n > 1_048_576)
            || self.max_write_bytes > 65536
            || self.max_message_bytes < 1024
            || [
                self.accept_retry_ms,
                self.auth_timeout_ms,
                self.send_timeout_ms,
                self.host_timeout_ms,
                self.close_retry_ms,
            ]
            .iter()
            .any(|n| *n == 0 || *n > 300_000)
            || !valid_ui_base(&self.ui_base_url)
            || self.allowed_origins.is_empty()
            || self.allowed_origins.iter().any(|s| {
                let Ok(uri) = s.parse::<tokio_tungstenite::tungstenite::http::Uri>() else {
                    return true;
                };
                !(s.starts_with("http://") || s.starts_with("https://"))
                    || !matches!(uri.scheme_str(), Some("http" | "https"))
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
    // O navegador manda a origem em minúsculas; esquema em outra caixa nunca casaria com ela.
    base.starts_with("http://")
        && uri.scheme_str() == Some("http")
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

/// What `harness.list` answers: names only, never programs, arguments, environment or paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarnessCatalog {
    pub harnesses: Vec<String>,
    pub workdir_aliases: Vec<String>,
}

pub trait Launcher: Send + Sync + 'static {
    fn prepare(&self, request: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError>;
    /// Read once when the API binds; restart the daemon to change the configuration.
    fn catalog(&self) -> HarnessCatalog {
        HarnessCatalog::default()
    }
}
pub struct DisabledLauncher;
impl Launcher for DisabledLauncher {
    fn prepare(&self, _: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError> {
        Err(ApiError::HarnessNotConfigured)
    }
}

struct Request {
    command: Value,
    /// Proof that the connection passed `auth.hello`; the queue controls require it.
    session: Arc<LocalSession>,
    reply: oneshot::Sender<Result<Reply, ApiError>>,
}
struct PendingMutation {
    job: tokio::task::JoinHandle<Result<bool, ApiError>>,
    entry: Option<Entry>,
    reply: oneshot::Sender<Result<Reply, ApiError>>,
}

impl PendingMutation {
    // Preserve mutation order and hold requests that depend on the pending result or audit lock.
    fn defers(&self, command: &Value) -> bool {
        matches!(
            command["method"].as_str(),
            Some("queue.start" | "queue.stop" | "approval.answer")
        ) || self.entry.as_ref().is_some_and(|entry| {
            command["params"]["_meta"]["idempotency_key"] == entry.key.as_str()
        })
    }
}

struct Reply {
    replayed: bool,
    /// Encarnação do stream assinado; a conexão só entrega eventos dela.
    stream: Option<Arc<Stream>>,
}
struct Session {
    generation: u64,
    offset: u64,
    last_host_seq: u64,
    ended: bool,
    closing: bool,
    invalidated: bool,
    released: bool,
    /// O `open` falhou depois de chegar ao host: o fechamento confirmado remove o registro e devolve o id.
    failed_open: bool,
    /// Terminou sozinha e o daemon devolveu a vaga: um `session.close` depois disso é aceito sem efeito.
    self_released: bool,
}
struct Entry {
    key: String,
    digest: [u8; 32],
}
struct Retry {
    id: SessionId,
    generation: u64,
    attempts: usize,
    due: Instant,
}
struct Service {
    #[cfg(test)]
    before_setup: Arc<Gate>,
    /// Roda com a tentativa automática em curso; `Some` substitui a resposta do host.
    #[cfg(test)]
    during_attempt: Option<fn(&mut Service) -> Option<ApiError>>,
    hooks: Arc<TestHooks>,
    /// Sessão e geração da tentativa automática que está fora da fila, em execução.
    in_flight: Option<(SessionId, u64)>,
    host: PtyClient,
    launcher: Arc<dyn Launcher>,
    queue: Arc<dyn QueueControl>,
    approvals: Arc<dyn ApprovalControl>,
    config: ApiConfig,
    log: Arc<Mutex<EventLog>>,
    sessions: HashMap<String, Session>,
    keys: VecDeque<Entry>,
    retries: VecDeque<Retry>,
    released: VecDeque<SessionId>,
    generations: u64,
}
impl Service {
    fn idempotency(&self, method: &str, params: &Value) -> Result<Option<Entry>, ApiError> {
        let key = params["_meta"]["idempotency_key"]
            .as_str()
            .ok_or(ApiError::Invalid)?;
        let digest: [u8; 32] = Sha256::digest(
            serde_json::to_vec(&json!([method, params])).map_err(|_| ApiError::Invalid)?,
        )
        .into();
        match self.keys.iter().find(|entry| entry.key == key) {
            Some(entry) if entry.digest == digest => Ok(None),
            Some(_) => Err(ApiError::Conflict),
            None => Ok(Some(Entry {
                key: key.to_owned(),
                digest,
            })),
        }
    }

    fn remember(&mut self, entry: Entry) {
        self.keys.push_back(entry);
        while self.keys.len() > self.config.idempotency_keys {
            self.keys.pop_front();
        }
    }

    fn dispatch_mutation(&self, request: Request) -> Option<PendingMutation> {
        let Request {
            command,
            session,
            reply,
        } = request;
        let params = &command["params"];
        let method = command["method"].as_str().expect("audit mutation");
        let stop = method == "queue.stop";
        let prepare = || {
            if params["_meta"]["origin"] != "local" {
                return Err(ApiError::Remote);
            }
            // Repeating a stop must still stop a queue that has started again.
            let entry = if stop {
                None
            } else if let Some(entry) = self.idempotency(method, params)? {
                Some(entry)
            } else {
                return Ok(None);
            };
            let cause = command["id"].as_str().ok_or(ApiError::Invalid)?.to_owned();
            let job = if method == "approval.answer" {
                // A key this cache no longer holds may still be in the log.
                let answer: CommandApprovalAnswerV1Params =
                    serde_json::from_value(params.clone()).map_err(|_| ApiError::Invalid)?;
                let approvals = self.approvals.clone();
                tokio::task::spawn_blocking(move || approvals.answer(&session, &answer, &cause))
            } else {
                let id = queue_id(params)?;
                let queue = self.queue.clone();
                tokio::task::spawn_blocking(move || {
                    if stop {
                        queue.stop(&session, &id, &cause).map(|stopped| !stopped)
                    } else {
                        queue.start(&session, &id, &cause).map(|()| false)
                    }
                })
            };
            Ok(Some((entry, job)))
        };
        match prepare() {
            Ok(Some((entry, job))) => Some(PendingMutation { job, entry, reply }),
            outcome => {
                let _ = reply.send(outcome.map(|_| Reply {
                    replayed: true,
                    stream: None,
                }));
                None
            }
        }
    }

    async fn command(&mut self, command: &Value, deadline: Instant) -> Result<Reply, ApiError> {
        let method = command["method"].as_str().ok_or(ApiError::Invalid)?;
        let params = &command["params"];
        if params["_meta"]["origin"] != "local" {
            return Err(ApiError::Remote);
        }
        let entry = self.idempotency(method, params)?;
        let replayed = entry.is_none();
        let mut stream = None;
        if let Some(entry) = entry {
            stream = self
                .execute(method, params, command["id"].as_str(), deadline)
                .await?;
            self.remember(entry);
        } else if method == "events.subscribe" {
            // Assinar não tem efeito; a repetição confere o stream atual como uma assinatura nova.
            stream = self.execute(method, params, None, deadline).await?;
        }
        Ok(Reply { replayed, stream })
    }
    async fn execute(
        &mut self,
        method: &str,
        params: &Value,
        cause: Option<&str>,
        deadline: Instant,
    ) -> Result<Option<Arc<Stream>>, ApiError> {
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
                // Um id podado nesta execução não volta: um assinante atrasado misturaria os dois streams.
                if self.sessions.contains_key(id)
                    || self.log.lock().await.is_retired(&format!("session/{id}"))
                {
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
                self.wait_host(host.open(open), deadline).await?;
                self.generations += 1;
                self.sessions.insert(
                    id.to_owned(),
                    Session {
                        generation: self.generations,
                        failed_open: false,
                        offset: 0,
                        last_host_seq: 0,
                        ended: false,
                        closing: false,
                        invalidated: false,
                        released: false,
                        self_released: false,
                    },
                );
                let setup = async {
                    #[cfg(test)]
                    self.before_setup.pass().await;
                    self.log.lock().await.append(id, "created", json!({"session_id": id, "task_id": null, "harness": params["harness"], "mode": "interactive", "cols": params["cols"], "rows": params["rows"], "workdir_alias": params["workdir_alias"]}), cause)?;
                    self.wait_host(host.subscribe(request.session_id.clone(), Some(0)), deadline).await?;
                    Ok(())
                }.await;
                if let Err(error) = setup {
                    // Qualquer falha aqui, inclusive o prazo, tem o mesmo desfecho: registro guardado e fechamento
                    // agendado. Só o host perdido continua derrubando o serviço.
                    let error = if error == ApiError::Unavailable {
                        error
                    } else {
                        ApiError::SetupFailed
                    };
                    // O host já abriu o terminal: o fechamento segue pela fila, com prazo próprio por tentativa,
                    // e o registro fica até o host confirmar. Esperar aqui deixaria o prazo do comando derrubar o serviço.
                    if let Some(session) = self.sessions.get_mut(id) {
                        session.failed_open = true;
                    }
                    self.invalidate(request.session_id).await;
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
                self.wait_host(host.write(write), deadline).await?;
            }
            "session.resize" => {
                self.active(id)?;
                let resize: ResizeParams =
                    serde_json::from_value(host_params).map_err(|_| ApiError::Invalid)?;
                self.wait_host(host.resize(resize), deadline).await?;
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
                self.close(close, deadline).await?;
            }
            "events.subscribe" => {
                return self
                    .log
                    .lock()
                    .await
                    .subscribe(
                        params["stream"].as_str().ok_or(ApiError::Invalid)?,
                        params["after_seq"].as_u64().ok_or(ApiError::Invalid)?,
                    )
                    .map(Some);
            }
            _ => return Err(ApiError::Method),
        }
        Ok(None)
    }
    async fn close(&mut self, params: SessionParams, deadline: Instant) -> Result<(), ApiError> {
        let id = params.session_id.clone();
        let session = self
            .sessions
            .get_mut(id.as_str())
            .ok_or(ApiError::Invalid)?;
        if session.released && (session.invalidated || session.self_released) {
            return Ok(());
        }
        // Uma sessão encerrada segue no host com a vaga e possíveis filhos vivos até o fechamento.
        if session.closing || session.released {
            return Err(ApiError::Invalid);
        }
        session.closing = true;
        let host = self.host.commands();
        let result = self.wait_host(host.close(params), deadline).await;
        let session = self
            .sessions
            .get_mut(id.as_str())
            .ok_or(ApiError::Invalid)?;
        session.closing = false;
        if result.is_ok() || result == Err(ApiError::Host(ErrorCode::UnknownSession)) {
            // Nenhuma tentativa antiga sobrevive à liberação: um id reaberto nunca recebe trabalho velho.
            self.retries.retain(|r| r.id != id);
            let mut log = self.log.lock().await;
            if session.failed_open {
                self.sessions.remove(id.as_str());
                log.remove(id.as_str());
                return result.map(|_| ());
            }
            if result.is_err() && !session.ended && !session.invalidated {
                // O host não conhece a sessão: nenhum outro evento vai encerrar o stream na interface.
                session.ended = true;
                // Só falha com a sequência esgotada; a vaga é devolvida mesmo assim.
                let _ = log.append(
                    id.as_str(),
                    "ended",
                    json!({"session_id": id.as_str(), "reason": "error", "exit_code": null}),
                    None,
                );
            }
            if session.invalidated {
                log.invalidate(id.as_str(), Closing::Closed);
            }
            session.released = true;
            self.released.push_back(id);
            if self.released.len() > self.config.max_sessions
                && let Some(expired) = self.released.pop_front()
            {
                self.sessions.remove(expired.as_str());
                log.retire(expired.as_str());
            }
        }
        result.map(|_| ())
    }
    /// Repete, com limite, o fechamento automático de sessão invalidada ou de `open` que falhou.
    async fn retry_close(&mut self) -> Result<(), ApiError> {
        let Some(index) = (0..self.retries.len()).min_by_key(|i| self.retries[*i].due) else {
            return Ok(());
        };
        let Some(mut retry) = self.retries.remove(index) else {
            return Ok(());
        };
        // Só a mesma instância da sessão, ainda no host, recebe a tentativa.
        if !self
            .sessions
            .get(retry.id.as_str())
            .is_some_and(|s| s.generation == retry.generation && !s.released)
        {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_millis(self.config.host_timeout_ms);
        // Enquanto a tentativa está fora da fila, um novo agendamento da mesma geração a reconhece.
        self.in_flight = Some((retry.id.clone(), retry.generation));
        #[cfg(test)]
        let injected = self.during_attempt.and_then(|during| during(self));
        #[cfg(not(test))]
        let injected: Option<ApiError> = None;
        let outcome = match injected {
            Some(error) => Err(error),
            None => {
                self.hooks.automatic_closes.fetch_add(1, Ordering::AcqRel);
                self.close(
                    SessionParams {
                        session_id: retry.id.clone(),
                    },
                    deadline,
                )
                .await
            }
        };
        self.in_flight = None;
        match outcome {
            // Liberada, já fechada pelo cliente ou podada: nada mais a fazer.
            Ok(()) | Err(ApiError::Invalid | ApiError::Host(ErrorCode::UnknownSession)) => {
                if let Some(session) = self.sessions.get_mut(retry.id.as_str())
                    && session.generation == retry.generation
                    && session.released
                    && !session.invalidated
                {
                    session.self_released = true;
                }
                return Ok(());
            }
            Err(ApiError::Unavailable) => return Err(ApiError::Unavailable),
            // Fechar de novo é seguro: se o pedido sem resposta chegou, o host responde UnknownSession.
            Err(ApiError::HostTimeout) => {
                if let Some(session) = self.sessions.get_mut(retry.id.as_str()) {
                    session.closing = false;
                }
            }
            Err(_) => (),
        }
        retry.attempts += 1;
        if retry.attempts < self.config.close_retries {
            eprintln!("ads-daemon: automatic session close failed; retrying");
            retry.due = Instant::now() + Duration::from_millis(self.config.close_retry_ms);
            self.retries.push_back(retry);
        } else {
            eprintln!("ads-daemon: automatic session close failed; session.close is required");
            // Uma sessão que só terminou guarda o stream inteiro; o `session.close` do cliente segue aceito.
            if self
                .sessions
                .get(retry.id.as_str())
                .is_some_and(|s| s.invalidated)
            {
                self.log
                    .lock()
                    .await
                    .invalidate(retry.id.as_str(), Closing::Failed);
            }
        }
        Ok(())
    }
    /// Uma sessão que terminou sozinha devolve a vaga pela mesma fila do fechamento automático.
    /// Vale também com um `session.close` em andamento: se o host o recusar, a tentativa já está na fila;
    /// se confirmar, o fechamento descarta a tentativa.
    fn release_ended(&mut self, id: &SessionId) {
        if let Some(session) = self.sessions.get(id.as_str())
            && !session.released
            && !session.invalidated
        {
            let generation = session.generation;
            self.schedule_close(id, generation);
        }
    }
    /// Uma entrada por geração, na fila ou em execução: a tentativa existente mantém saldo e prazo.
    fn schedule_close(&mut self, id: &SessionId, generation: u64) {
        let running = self
            .in_flight
            .as_ref()
            .is_some_and(|(r, g)| r == id && *g == generation);
        if !running
            && !self
                .retries
                .iter()
                .any(|r| r.id == *id && r.generation == generation)
        {
            self.retries.push_back(Retry {
                id: id.clone(),
                generation,
                attempts: 0,
                due: Instant::now(),
            });
        }
    }
    async fn invalidate(&mut self, id: SessionId) {
        if let Some(session) = self.sessions.get_mut(id.as_str())
            && !session.invalidated
        {
            session.invalidated = true;
            // A invalidação pode ocorrer enquanto o daemon espera outra resposta do host.
            let closing = if session.released {
                Closing::Closed
            } else {
                let generation = session.generation;
                self.schedule_close(&id, generation);
                Closing::Pending
            };
            self.log.lock().await.invalidate(id.as_str(), closing);
        }
    }
    /// Espera o host até `deadline`, o prazo único do comando ou da tentativa em curso.
    async fn wait_host<T>(
        &mut self,
        future: impl Future<Output = Result<T, ClientError>>,
        deadline: Instant,
    ) -> Result<T, ApiError> {
        // Só a primeira sondagem confere o prazo: uma chamada nunca começa depois dele.
        if Instant::now() >= deadline {
            return Err(ApiError::HostTimeout);
        }
        tokio::pin!(future);
        let expired = tokio::time::sleep_until(deadline);
        tokio::pin!(expired);
        loop {
            // A resposta pronta vence o prazo, e o prazo vence os eventos, que não podem adiá-lo.
            tokio::select! {
                biased;
                result = &mut future => return result.map_err(host_error),
                _ = &mut expired => return Err(ApiError::HostTimeout),
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
                self.release_ended(&exit.session_id);
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
                    self.release_ended(&closed.session_id);
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
    catalog: Value,
    service: Service,
    pending: Arc<Semaphore>,
    hooks: Arc<TestHooks>,
}

/// Ponto de espera que só um teste fecha; aberto, `pass` retorna na hora.
#[derive(Default)]
pub struct Gate {
    closed: AtomicBool,
    /// Passagens liberadas uma a uma com o portão fechado.
    passes: AtomicUsize,
    opened: Notify,
    #[cfg(test)]
    reached: Notify,
}
impl Gate {
    pub fn hold(&self) {
        self.closed.store(true, Ordering::Release);
    }
    pub fn release(&self) {
        self.closed.store(false, Ordering::Release);
        self.opened.notify_waiters();
    }
    /// Deixa passar uma espera, atual ou futura, sem abrir o portão.
    pub fn admit_one(&self) {
        self.passes.fetch_add(1, Ordering::AcqRel);
        self.opened.notify_waiters();
    }
    async fn pass(&self) {
        loop {
            let opened = self.opened.notified();
            tokio::pin!(opened);
            opened.as_mut().enable();
            if !self.closed.load(Ordering::Acquire)
                || self
                    .passes
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
                    .is_ok()
            {
                return;
            }
            #[cfg(test)]
            self.reached.notify_one();
            opened.await;
        }
    }
}

/// Barreiras e medidas para os testes; inertes fora deles e fora do alcance do socket e da configuração.
#[doc(hidden)]
#[derive(Default)]
pub struct TestHooks {
    accept_faults: AtomicUsize,
    accept_retry: Gate,
    /// Depois do upgrade, antes do `auth.hello`.
    pub after_upgrade: Gate,
    /// Na tarefa expulsa, antes de ela terminar e devolver a vaga.
    pub evicted_exit: Gate,
    /// Antes de cada entrega a uma conexão com assinaturas.
    pub before_delivery: Gate,
    /// Antes de cada tentativa automática de fechamento, sem parar o resto do serviço.
    pub automatic_close: Gate,
    queued_closes: AtomicUsize,
    queued_commands: AtomicUsize,
    pending_live: AtomicUsize,
    pending_peak: AtomicUsize,
    waiting_peak: AtomicUsize,
    automatic_closes: AtomicUsize,
}
impl TestHooks {
    pub fn fail_accepts(&self, count: usize) {
        self.accept_faults.store(count, Ordering::Release);
    }
    /// Closes the daemon sent to the host on its own, without a client command.
    pub fn automatic_closes(&self) -> usize {
        self.automatic_closes.load(Ordering::Acquire)
    }
    /// Commands the connections have handed to the service so far.
    pub fn queued_commands(&self) -> usize {
        self.queued_commands.load(Ordering::Acquire)
    }
    /// Automatic closes still queued when the last command was answered.
    pub fn queued_closes(&self) -> usize {
        self.queued_closes.load(Ordering::Acquire)
    }
    pub fn accept_faults_left(&self) -> usize {
        self.accept_faults.load(Ordering::Acquire)
    }
    /// Pending connection tasks alive now, the most seen at once, and the longest the eviction queue grew.
    pub fn pending(&self) -> (usize, usize, usize) {
        (
            self.pending_live.load(Ordering::Acquire),
            self.pending_peak.load(Ordering::Acquire),
            self.waiting_peak.load(Ordering::Acquire),
        )
    }
}
/// A vaga pendente vive até a tarefa autenticar ou terminar, mesmo depois da expulsão.
struct PendingSlot {
    _permit: tokio::sync::OwnedSemaphorePermit,
    hooks: Arc<TestHooks>,
}
impl Drop for PendingSlot {
    fn drop(&mut self) {
        self.hooks.pending_live.fetch_sub(1, Ordering::AcqRel);
    }
}
const BEFORE_UPGRADE: u8 = 0;
const UPGRADED: u8 = 1;
const EVICTED: u8 = 2;
struct Waiting {
    evict: oneshot::Sender<()>,
    state: Arc<AtomicU8>,
}
struct Admission {
    stream: TcpStream,
    evicted: oneshot::Receiver<()>,
    state: Arc<AtomicU8>,
    slot: PendingSlot,
}
struct Admitter<'a> {
    listener: &'a TcpListener,
    config: &'a ApiConfig,
    pending: Arc<Semaphore>,
    waiting: std::sync::Mutex<VecDeque<Waiting>>,
    hooks: Arc<TestHooks>,
}
impl Admitter<'_> {
    /// Fila cheia: expulsa a pendente mais antiga que ainda não fez upgrade e espera a vaga dela voltar.
    /// Conexão que já fez upgrade conserva a vaga até autenticar ou vencer o prazo; se todas fizeram, a nova é recusada.
    async fn admit(&self) -> Admission {
        loop {
            let (stream, _) =
                accept_retry(self.listener, self.config.accept_retry_ms, &self.hooks).await;
            let permit = match self.pending.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    if !self.evict_oldest() {
                        continue;
                    }
                    match timeout(
                        Duration::from_millis(self.config.auth_timeout_ms),
                        self.pending.clone().acquire_owned(),
                    )
                    .await
                    {
                        Ok(Ok(permit)) => permit,
                        _ => continue,
                    }
                }
            };
            let live = self.hooks.pending_live.fetch_add(1, Ordering::AcqRel) + 1;
            self.hooks.pending_peak.fetch_max(live, Ordering::AcqRel);
            let (evict, evicted) = oneshot::channel();
            let state = Arc::new(AtomicU8::new(BEFORE_UPGRADE));
            let mut waiting = self.waiting.lock().unwrap_or_else(|e| e.into_inner());
            // Quem já saiu da fase de upgrade larga o receptor; limpar antes de inserir mantém a fila no tamanho da cota.
            waiting.retain(|w| !w.evict.is_closed());
            waiting.push_back(Waiting {
                evict,
                state: state.clone(),
            });
            self.hooks
                .waiting_peak
                .fetch_max(waiting.len(), Ordering::AcqRel);
            drop(waiting);
            return Admission {
                stream,
                evicted,
                state,
                slot: PendingSlot {
                    _permit: permit,
                    hooks: self.hooks.clone(),
                },
            };
        }
    }
    fn evict_oldest(&self) -> bool {
        let mut waiting = self.waiting.lock().unwrap_or_else(|e| e.into_inner());
        waiting.retain(|w| !w.evict.is_closed());
        let chosen = waiting.iter().position(|w| {
            w.state
                .compare_exchange(BEFORE_UPGRADE, EVICTED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        });
        match chosen.and_then(|index| waiting.remove(index)) {
            Some(oldest) => {
                let _ = oldest.evict.send(());
                true
            }
            None => false,
        }
    }
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
        let hooks = Arc::new(TestHooks::default());
        let catalog = launcher.catalog();
        let catalog =
            json!({"harnesses": catalog.harnesses, "workdir_aliases": catalog.workdir_aliases});
        let log = Arc::new(Mutex::new(EventLog::new(
            config.events_per_stream,
            config.retired_sessions,
        )));
        let service = Service {
            #[cfg(test)]
            before_setup: Arc::default(),
            #[cfg(test)]
            during_attempt: None,
            hooks: hooks.clone(),
            in_flight: None,
            host,
            launcher,
            queue: Arc::new(DisabledQueue),
            approvals: Arc::new(DisabledApprovals),
            config: config.clone(),
            log,
            sessions: HashMap::new(),
            keys: VecDeque::new(),
            retries: VecDeque::new(),
            released: VecDeque::new(),
            generations: 0,
        };
        Ok(Self {
            listener,
            token,
            pending: Arc::new(Semaphore::new(config.max_pending_auth)),
            config,
            catalog,
            service,
            hooks,
        })
    }
    /// Serves `queue.get`, `queue.start` and `queue.stop` from `queue`; without it they answer
    /// `QUEUE_NOT_CONFIGURED`.
    pub fn with_queue(mut self, queue: Arc<dyn QueueControl>) -> Self {
        self.service.queue = queue;
        self
    }
    /// Serves `approval.list` and `approval.answer` from `approvals`; without it they answer
    /// `APPROVAL_NOT_CONFIGURED`.
    pub fn with_approvals(mut self, approvals: Arc<dyn ApprovalControl>) -> Self {
        self.service.approvals = approvals;
        self
    }
    #[doc(hidden)]
    pub fn test_hooks(&self) -> Arc<TestHooks> {
        self.hooks.clone()
    }
    pub fn port(&self) -> Result<u16, ApiError> {
        self.listener
            .local_addr()
            .map(|a| a.port())
            .map_err(|_| ApiError::Unavailable)
    }
    pub fn desktop_bootstrap(&self) -> Result<String, ApiError> {
        Ok(self.token.desktop_bootstrap(self.port()?))
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
            catalog,
            mut service,
            pending,
            hooks,
        } = self;
        let token = Arc::new(token);
        let catalog = Arc::new(catalog);
        let config = Arc::new(config);
        let port = listener
            .local_addr()
            .map_err(|_| ApiError::Unavailable)?
            .port();
        let log = service.log.clone();
        let queue = service.queue.clone();
        let approvals = service.approvals.clone();
        let permits = Arc::new(Semaphore::new(config.max_connections));
        let admitter = Admitter {
            listener: &listener,
            config: &config,
            pending,
            waiting: std::sync::Mutex::new(VecDeque::new()),
            hooks: hooks.clone(),
        };
        let (commands, mut incoming) = mpsc::channel::<Request>(config.command_queue);
        let mut connections = tokio::task::JoinSet::new();
        let mut accepting = Box::pin(admitter.admit());
        let mut pending_mutation: Option<PendingMutation> = None;
        let mut deferred = VecDeque::new();
        loop {
            let retry_due = service.retries.iter().map(|r| r.due).min();
            let mutation_idle = pending_mutation.is_none();
            tokio::select! {
                _ = async {
                    tokio::time::sleep_until(retry_due.unwrap_or_else(Instant::now)).await;
                    hooks.automatic_close.pass().await;
                }, if retry_due.is_some() => {
                    service.retry_close().await?;
                },
                event = service.host.next() => {
                    if service.event(event.map_err(host_error)?).await == Err(ApiError::Unavailable) { return Err(ApiError::Unavailable); }
                },
                replayed = async {
                    let pending = pending_mutation.as_mut().expect("audit mutation is pending");
                    (&mut pending.job).await.unwrap_or(Err(ApiError::Storage))
                }, if !mutation_idle => {
                    let pending = pending_mutation.take().expect("audit mutation completed");
                    if matches!(replayed, Err(ApiError::Unavailable | ApiError::HostTimeout)) {
                        let _ = pending.reply.send(Err(ApiError::Unavailable));
                        return Err(ApiError::Unavailable);
                    }
                    if replayed.is_ok() && let Some(entry) = pending.entry {
                        service.remember(entry);
                    }
                    let _ = pending.reply.send(replayed.map(|replayed| Reply { replayed, stream: None }));
                }
                Some(request) = async {
                    if mutation_idle && let Some(request) = deferred.pop_front() {
                        return Some(request);
                    }
                    incoming.recv().await
                }, if deferred.len() < config.command_queue || mutation_idle => {
                    if let Some(pending) = &pending_mutation && pending.defers(&request.command) {
                        deferred.push_back(request);
                        continue;
                    }
                    if matches!(request.command["method"].as_str(), Some("queue.start" | "queue.stop" | "approval.answer")) {
                        pending_mutation = service.dispatch_mutation(request);
                        continue;
                    }
                    let deadline = Instant::now() + Duration::from_millis(config.host_timeout_ms);
                    let result = service.command(&request.command, deadline).await;
                    // Um comando sem resposta do host pode ter executado; encerrar impede repetir efeito incerto.
                    if let Err(ApiError::Unavailable | ApiError::HostTimeout) = result {
                        let _ = request.reply.send(Err(ApiError::Unavailable));
                        return Err(ApiError::Unavailable);
                    }
                    hooks.queued_closes.store(service.retries.len(), Ordering::Release);
                    let _ = request.reply.send(result);
                }
                admission = &mut accepting => {
                    accepting.set(admitter.admit());
                    let (token, config, commands, log, hooks, catalog, queue, approvals) = (token.clone(), config.clone(), commands.clone(), log.clone(), hooks.clone(), catalog.clone(), queue.clone(), approvals.clone());
                    let authenticated = permits.clone();
                    connections.spawn(async move {
                        let _ = connection(admission, port, token, config, commands, log, (authenticated, hooks, catalog, queue, approvals)).await;
                    });
                }
                Some(_) = connections.join_next(), if !connections.is_empty() => (),
            }
        }
    }
}

async fn retry_io<T, F, Fut>(mut operation: F, delay_ms: u64, retry: &Gate) -> T
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
                retry.pass().await;
            }
        }
    }
}
async fn accept_retry(
    listener: &TcpListener,
    delay_ms: u64,
    hooks: &TestHooks,
) -> (TcpStream, std::net::SocketAddr) {
    retry_io(
        || async {
            if hooks
                .accept_faults
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
                .is_ok()
            {
                return Err(std::io::Error::from(std::io::ErrorKind::ConnectionAborted));
            }
            listener.accept().await
        },
        delay_ms,
        &hooks.accept_retry,
    )
    .await
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
fn queue_id(params: &Value) -> Result<ads_contracts::CommonIdV1, ApiError> {
    serde_json::from_value(params["queue_id"].clone()).map_err(|_| ApiError::Invalid)
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
            | "harness.list"
            | "queue.get"
            | "queue.start"
            | "queue.stop"
            | "approval.list"
            | "approval.answer"
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

/// What every connection shares: the authenticated slots, the hooks, the catalog and the owner controls.
type Shared = (
    Arc<Semaphore>,
    Arc<TestHooks>,
    Arc<Value>,
    Arc<dyn QueueControl>,
    Arc<dyn ApprovalControl>,
);

#[allow(clippy::result_large_err)] // A assinatura do callback é definida pela biblioteca WebSocket.
async fn connection(
    admission: Admission,
    port: u16,
    token: Arc<Token>,
    config: Arc<ApiConfig>,
    commands: mpsc::Sender<Request>,
    log: Arc<Mutex<EventLog>>,
    (authenticated, hooks, catalog, queue, approvals): Shared,
) -> Result<(), ApiError> {
    use tokio_tungstenite::tungstenite::handshake::server::{Request as HttpRequest, Response};
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(config.max_message_bytes))
        .max_frame_size(Some(config.max_message_bytes));
    let Admission {
        stream,
        evicted,
        state,
        slot,
    } = admission;
    let endpoints = if config.desktop {
        Some((
            stream.peer_addr().map_err(|_| ApiError::Unavailable)?,
            stream.local_addr().map_err(|_| ApiError::Unavailable)?,
        ))
    } else {
        None
    };
    let deadline = Instant::now() + Duration::from_millis(config.auth_timeout_ms);
    let upgrade = async {
        let ws = timeout_at(
            deadline,
            tokio_tungstenite::accept_hdr_async_with_config(
                stream,
                |request: &HttpRequest, mut response: Response| {
                    // A troca para UPGRADED acontece antes de o 101 sair: depois dele a conexão não pode ser expulsa.
                    if permits_upgrade(
                        request,
                        port,
                        (!config.desktop).then_some(config.allowed_origins.as_slice()),
                    ) && state
                        .compare_exchange(
                            BEFORE_UPGRADE,
                            UPGRADED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        if let Some((client, server)) = endpoints {
                            let nonce = &request.headers()["sec-websocket-key"];
                            let proof = token.desktop_proof(nonce.as_bytes(), client, server);
                            response.headers_mut().insert(
                                ads_contracts::desktop_proof::HEADER,
                                proof.parse().expect("base64url header"),
                            );
                        }
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
        // O 101 já saiu, mas a expulsão ainda é ouvida até aqui: a troca de estado no callback é o que protege.
        hooks.after_upgrade.pass().await;
        Ok(ws)
    };
    // Só a fase antes do upgrade pode ser expulsa; quem perder a disputa pela troca de estado sai.
    let mut ws = tokio::select! {
        biased;
        _ = evicted => {
            hooks.evicted_exit.pass().await;
            return Err(ApiError::Capacity);
        }
        result = upgrade => result?,
    };
    let hello = async {
        let first = timeout_at(deadline, ws.next()).await;
        let hello = match first {
            Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| validate_command(v).ok()),
            _ => None,
        };
        let session = hello
            .as_ref()
            .filter(|v| v["method"] == "auth.hello" && v["params"]["_meta"]["origin"] == "local")
            .and_then(|v| v["params"]["token"].as_str())
            .and_then(|s| token.open_session(s));
        let Some(session) = session else {
            let _ = timeout(
                Duration::from_millis(config.send_timeout_ms),
                ws.close(None),
            )
            .await;
            return Err(ApiError::Invalid);
        };
        Ok((hello, Arc::new(session)))
    };
    let (mut hello, session) = hello.await?;
    let id = consume_hello(&mut hello)?;
    let _authenticated = authenticated
        .try_acquire_owned()
        .map_err(|_| ApiError::Capacity)?;
    drop(slot);
    send(&mut ws, response(&id, Ok(false)), &config).await?;
    let mut changed = log.lock().await.watch();
    // Por stream: cursor, ids das assinaturas e a encarnação assinada.
    let mut subscriptions = HashMap::<String, (u64, Vec<Value>, Arc<Stream>)>::new();
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
                    if command["method"] == "harness.list" {
                        // Só leitura: fora da cache de idempotência, que guardaria só `replayed`.
                        let reply = if command["params"]["_meta"]["origin"] == "local" {
                            json!({"jsonrpc": "2.0", "id": id, "result": *catalog})
                        } else {
                            response(&id, Err(ApiError::Remote))
                        };
                        send(&mut ws, reply, &config).await?;
                        continue;
                    }
                    if command["method"] == "queue.get" {
                        // Só leitura, como harness.list; a origem é conferida aqui porque não passa pelo serviço.
                        let queue_id = command["params"]["queue_id"].clone();
                        let state = if command["params"]["_meta"]["origin"] == "local" {
                            match serde_json::from_value(queue_id.clone()) {
                                Ok(id) => {
                                    let (queue, session) = (queue.clone(), session.clone());
                                    tokio::task::spawn_blocking(move || queue.state(&session, &id))
                                        .await
                                        .unwrap_or(Err(ApiError::Storage))
                                }
                                Err(_) => Err(ApiError::Invalid),
                            }
                        } else {
                            Err(ApiError::Remote)
                        };
                        let reply = match state {
                            Ok(state) => {
                                json!({"jsonrpc": "2.0", "id": id, "result": {"queue_id": queue_id, "state": state}})
                            }
                            Err(error) => response(&id, Err(error)),
                        };
                        send(&mut ws, reply, &config).await?;
                        continue;
                    }
                    if command["method"] == "approval.list" {
                        // Só leitura, como queue.get: fora da cache de idempotência, origem conferida aqui.
                        let pending = if command["params"]["_meta"]["origin"] == "local" {
                            let (approvals, session) = (approvals.clone(), session.clone());
                            tokio::task::spawn_blocking(move || approvals.pending(&session))
                                .await
                                .unwrap_or(Err(ApiError::Storage))
                        } else {
                            Err(ApiError::Remote)
                        };
                        let reply = match pending {
                            Ok(approvals) => {
                                json!({"jsonrpc": "2.0", "id": id, "result": {"approvals": approvals}})
                            }
                            Err(error) => response(&id, Err(error)),
                        };
                        send(&mut ws, reply, &config).await?;
                        continue;
                    }
                    let subscribe = command["method"] == "events.subscribe";
                    let stream = command["params"]["stream"]
                        .as_str()
                        .unwrap_or("")
                        .to_owned();
                    // Ids de uma encarnação já terminada saem na próxima entrega e não contam.
                    let ids = subscriptions
                        .values()
                        .filter(|s| !s.2.ended())
                        .map(|s| s.1.len())
                        .sum::<usize>();
                    if subscribe
                        && ((!subscriptions.contains_key(&stream)
                            && subscriptions.len() >= config.max_sessions)
                            || ids
                                >= config
                                    .subscription_ids_per_stream
                                    .saturating_mul(config.max_sessions))
                    {
                        send(&mut ws, response(&id, Err(ApiError::Capacity)), &config).await?;
                        continue;
                    }
                    let after = command["params"]["after_seq"].as_u64().unwrap_or(0);
                    let (reply, answer) = oneshot::channel();
                    commands
                        .send(Request {
                            command,
                            session: session.clone(),
                            reply,
                        })
                        .await
                        .map_err(|_| ApiError::Unavailable)?;
                    hooks.queued_commands.fetch_add(1, Ordering::AcqRel);
                    let outcome = answer.await.map_err(|_| ApiError::Unavailable)?;
                    let replayed = outcome.as_ref().map(|r| r.replayed).map_err(|e| *e);
                    if subscribe
                        && let Ok(Reply {
                            stream: Some(handle),
                            ..
                        }) = &outcome
                        && let Some((cursor, mut ids, current)) = subscriptions.remove(&stream)
                    {
                        if Arc::ptr_eq(&current, handle) {
                            // A mesma encarnação de novo: um cursor só, e o resultado final vai a cada id.
                            if !ids.contains(&id) {
                                ids.push(id.clone());
                            }
                            subscriptions.insert(stream.clone(), (cursor.min(after), ids, current));
                        } else {
                            // A encarnação anterior já terminou: o fim dela sai antes da confirmação da nova.
                            let error = match current.poll(cursor) {
                                Poll::End(error) | Poll::Fatal(error) => error,
                                Poll::Events(_) | Poll::Wait => ApiError::Replay,
                            };
                            for old in &ids {
                                send(&mut ws, response(old, Err(error)), &config).await?;
                            }
                        }
                    }
                    send(&mut ws, response(&id, replayed), &config).await?;
                    if subscribe
                        && let Ok(Reply {
                            stream: Some(handle),
                            ..
                        }) = outcome
                        && !subscriptions.contains_key(&stream)
                    {
                        subscriptions.insert(stream, (after, vec![id], handle));
                    }
                }
            }
        }
        if !subscriptions.is_empty() {
            hooks.before_delivery.pass().await;
        }
        let mut finished = Vec::new();
        for (name, (after, subscription_ids, stream)) in &mut subscriptions {
            let events = match stream.poll(*after) {
                // Fechamento automático em curso: a entrega espera o resultado final.
                Poll::Wait => continue,
                // Encarnação podada, removida ou com o fechamento resolvido: termina só esta assinatura.
                Poll::End(error) => {
                    for subscription_id in subscription_ids.iter() {
                        send(&mut ws, response(subscription_id, Err(error)), &config).await?;
                    }
                    finished.push(name.clone());
                    continue;
                }
                Poll::Events(events) => events,
                Poll::Fatal(error) => {
                    for subscription_id in subscription_ids.iter() {
                        send(&mut ws, response(subscription_id, Err(error)), &config).await?;
                    }
                    let _ = timeout(
                        Duration::from_millis(config.send_timeout_ms),
                        ws.close(None),
                    )
                    .await;
                    return Err(error);
                }
            };
            deliver(&mut ws, events, after, stream, &config).await?;
        }
        for name in finished {
            subscriptions.remove(&name);
        }
    }
}

/// Envia um lote em ordem; uma invalidação no meio dele suspende o resto e o cursor fica no último quadro enviado.
async fn deliver<S>(
    ws: &mut S,
    events: Vec<Value>,
    after: &mut u64,
    stream: &Stream,
    config: &ApiConfig,
) -> Result<(), ApiError>
where
    S: futures_util::Sink<Message> + Unpin,
{
    for event in events {
        if !stream.deliverable() {
            break;
        }
        *after = event["seq"].as_u64().ok_or(ApiError::Unavailable)?;
        send(ws, event, config).await?;
    }
    Ok(())
}

async fn send<S>(ws: &mut S, value: Value, config: &ApiConfig) -> Result<(), ApiError>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let value = checked("urn:ads:contract:local-api.server-message:4", value)?;
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
            &Gate::default(),
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
    async fn fake_service() -> (
        LocalApi,
        Arc<ads_ptyhost::FakeBackend>,
        Arc<ads_ptyhost::PtyHost>,
        tokio::task::JoinHandle<()>,
    ) {
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
        let api = LocalApi::bind(
            ApiConfig::from_toml_str("max_sessions = 1").unwrap(),
            client,
            Arc::new(FakeLauncher),
        )
        .await
        .unwrap();
        (api, backend, host, peer)
    }

    #[tokio::test(start_paused = true)]
    async fn confirmed_open_with_expired_setup_budget_schedules_cleanup() {
        let (mut api, backend, host, peer) = fake_service().await;
        let gate = api.service.before_setup.clone();
        gate.hold();
        let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
        let budget = Duration::from_secs(5);
        let deadline = Instant::now() + budget;
        let (result, ()) = timeout(Duration::from_secs(10), async {
            tokio::join!(
                api.service.execute("session.open", &params, None, deadline),
                async {
                    gate.reached.notified().await;
                    assert_eq!(host.sessions().len(), 1);
                    tokio::time::advance(budget).await;
                    gate.release();
                }
            )
        })
        .await
        .unwrap();
        assert_eq!(result.map(|_| ()), Err(ApiError::SetupFailed));
        // Let the host serve anything already sent: the subscription never reached it.
        tokio::time::sleep(Duration::from_millis(1)).await;
        let id: SessionId = "session".to_owned().try_into().unwrap();
        assert_eq!(host.subscriber_count(&id), Ok(0));
        assert_eq!(api.service.retries.len(), 1);
        let mut other = params.clone();
        other["session_id"] = json!("other");
        assert_eq!(
            api.service
                .execute("session.open", &other, None, Instant::now() + budget)
                .await
                .map(|_| ()),
            Err(ApiError::Capacity)
        );
        api.service.retry_close().await.unwrap();
        assert!(backend.terminals()[0].was_killed());
        assert!(host.sessions().is_empty());
        assert!(api.service.sessions.is_empty());
        assert!(
            api.service
                .execute("session.open", &params, None, Instant::now() + budget)
                .await
                .is_ok()
        );
        assert_eq!(host.sessions().len(), 1);
        assert!(api.service.retries.is_empty());
        peer.abort();
    }

    async fn open_fake(api: &mut LocalApi) -> SessionId {
        let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
        api.service
            .execute(
                "session.open",
                &params,
                None,
                Instant::now() + Duration::from_secs(5),
            )
            .await
            .unwrap();
        "session".to_owned().try_into().unwrap()
    }
    fn reschedule(service: &mut Service) {
        service.release_ended(&"session".to_owned().try_into().unwrap());
    }

    #[tokio::test]
    async fn rescheduling_during_a_refused_attempt_keeps_one_entry_and_its_budget() {
        let (mut api, _, _, peer) = fake_service().await;
        let id = open_fake(&mut api).await;
        api.service.release_ended(&id);
        api.service.during_attempt = Some(|service| {
            reschedule(service);
            Some(ApiError::Host(ErrorCode::InternalError))
        });
        let before = Instant::now();
        api.service.retry_close().await.unwrap();
        assert_eq!(api.service.retries.len(), 1);
        assert_eq!(api.service.retries[0].attempts, 1);
        assert!(
            api.service.retries[0].due
                >= before + Duration::from_millis(api.service.config.close_retry_ms)
        );
        // The remaining budget runs out with no other entry left behind.
        for _ in 1..api.service.config.close_retries {
            api.service.retry_close().await.unwrap();
        }
        assert!(api.service.retries.is_empty());
        assert!(api.service.in_flight.is_none());
        peer.abort();
    }

    #[tokio::test]
    async fn rescheduling_during_a_confirmed_attempt_returns_one_slot() {
        let (mut api, backend, host, peer) = fake_service().await;
        let id = open_fake(&mut api).await;
        api.service.release_ended(&id);
        api.service.during_attempt = Some(|service| {
            reschedule(service);
            None
        });
        api.service.retry_close().await.unwrap();
        assert!(backend.terminals()[0].was_killed());
        assert!(host.sessions().is_empty());
        assert!(api.service.retries.is_empty());
        assert_eq!(api.service.released.len(), 1);
        assert!(api.service.sessions["session"].released);
        peer.abort();
    }

    #[tokio::test]
    async fn one_pending_close_per_generation() {
        let (mut api, _, _, peer) = fake_service().await;
        let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
        api.service
            .execute(
                "session.open",
                &params,
                None,
                Instant::now() + Duration::from_secs(5),
            )
            .await
            .unwrap();
        let id: SessionId = "session".to_owned().try_into().unwrap();
        api.service.release_ended(&id);
        api.service.retries[0].attempts = 2;
        api.service.release_ended(&id);
        api.service.invalidate(id).await;
        assert_eq!(api.service.retries.len(), 1);
        // The attempt already queued keeps its budget.
        assert_eq!(api.service.retries[0].attempts, 2);
        peer.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn ready_host_call_is_not_polled_at_the_deadline() {
        let (mut api, _, _, peer) = fake_service().await;
        let deadline = Instant::now() + Duration::from_secs(5);
        let ready = || std::future::ready(Ok::<_, ClientError>(()));
        assert_eq!(api.service.wait_host(ready(), deadline).await, Ok(()));
        tokio::time::advance(Duration::from_secs(5)).await;
        let mut polls = 0;
        for _ in 0..32 {
            let response = std::future::poll_fn(|_| {
                polls += 1;
                std::task::Poll::Ready(Ok::<_, ClientError>(()))
            });
            assert_eq!(
                api.service.wait_host(response, deadline).await,
                Err(ApiError::HostTimeout)
            );
        }
        assert_eq!(polls, 0);
        peer.abort();
    }

    /// A service whose host side the test writes by hand; `host.hello` is already answered.
    async fn hand_driven_service() -> (LocalApi, tokio::io::DuplexStream) {
        use crate::pty_client::ClientConfig;
        use ads_ptyhost::wire::{self, ResultBody};
        use tokio::io::AsyncWriteExt;
        let (near, mut far) = tokio::io::duplex(65536);
        let hello = async {
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
        };
        let config = ClientConfig {
            max_frame_bytes: 131072,
            queue_capacity: 8,
        };
        let (client, ()) = tokio::join!(PtyClient::over(near, config), hello);
        let api = LocalApi::bind(
            ApiConfig::from_toml_str("").unwrap(),
            client.unwrap(),
            Arc::new(DisabledLauncher),
        )
        .await
        .unwrap();
        (api, far)
    }

    #[tokio::test(start_paused = true)]
    async fn ready_reply_wins_over_a_deadline_crossed_by_a_host_event() {
        use tokio::io::AsyncWriteExt;
        let (mut api, mut far) = hand_driven_service().await;
        api.service.sessions.insert(
            "session".to_owned(),
            Session {
                generation: 1,
                offset: 0,
                last_host_seq: 0,
                ended: false,
                closing: false,
                invalidated: false,
                released: false,
                failed_open: false,
                self_released: false,
            },
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let (reply, answer) = oneshot::channel::<()>();
        let response = async { answer.await.map_err(|_| ClientError::Disconnected) };
        let log = api.service.log.clone();
        let (result, ()) = tokio::join!(api.service.wait_host(response, deadline), async {
            // The output handler waits for the log while the deadline passes and the reply arrives.
            let held = log.lock().await;
            let id = "session".to_owned().try_into().unwrap();
            far.write_all(&ads_ptyhost::wire::encode_output(&id, 1, b"terminal-sentinel").unwrap())
                .await
                .unwrap();
            // The clock only moves once every task waits: the handler is blocked on the log.
            tokio::time::sleep(Duration::from_millis(1)).await;
            tokio::time::advance(Duration::from_secs(5)).await;
            reply.send(()).unwrap();
            drop(held);
        });
        assert_eq!(result, Ok(()));
        assert_eq!(
            log.lock().await.after("session/session", 0).unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn failed_created_event_closes_the_opened_host_session() {
        let (mut api, backend, host, peer) = fake_service().await;
        api.service.log.lock().await.exhaust_sequence("session");
        let params = json!({"session_id": "session", "harness": "test", "workdir_alias": "workspace", "cols": 80, "rows": 24, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}});
        assert_eq!(
            api.service
                .execute(
                    "session.open",
                    &params,
                    None,
                    Instant::now() + Duration::from_secs(5)
                )
                .await
                .map(|_| ()),
            Err(ApiError::SetupFailed)
        );
        // The record stays until the queued close is confirmed by the host.
        assert!(!backend.terminals()[0].was_killed());
        assert_eq!(api.service.sessions.len(), 1);
        api.service.retry_close().await.unwrap();
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
        let result = timeout(
            Duration::from_secs(2),
            api.service
                .wait_host(host.list(), Instant::now() + Duration::from_secs(2)),
        )
        .await
        .unwrap();
        peer.abort();
        assert!(result.unwrap().sessions.is_empty());
    }

    async fn authenticate(port: u16, token: &str) -> tokio_tungstenite::WebSocketStream<TcpStream> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut request = format!("ws://127.0.0.1:{port}/")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("Origin", "http://127.0.0.1:5173".parse().unwrap());
        let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (mut ws, _) = tokio_tungstenite::client_async(request, stream)
            .await
            .unwrap();
        let hello = json!({"jsonrpc": "2.0", "id": "01ARZ3NDEKTSV4RRFFQ69G5FAB", "method": "auth.hello", "params": {"token": token, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAB"}}});
        ws.send(Message::Text(hello.to_string().into()))
            .await
            .unwrap();
        let reply: Value =
            serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(reply["result"]["accepted"], true);
        ws
    }

    #[tokio::test]
    async fn run_keeps_serving_while_accept_fails() {
        use crate::pty_client::ClientConfig;
        use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
        let host =
            Arc::new(PtyHost::new(Arc::new(FakeBackend::new()), HostConfig::defaults()).unwrap());
        let (near, far) = tokio::io::duplex(4096);
        let peer = tokio::spawn(server::connection(host.clone(), far));
        let client = PtyClient::over(near, ClientConfig::recommended(host.config()))
            .await
            .unwrap();
        let api = LocalApi::bind(
            ApiConfig::from_toml_str("accept_retry_ms = 20").unwrap(),
            client,
            Arc::new(DisabledLauncher),
        )
        .await
        .unwrap();
        let port = api.port().unwrap();
        let token = api
            .launch_url()
            .unwrap()
            .split_once("&token=")
            .unwrap()
            .1
            .to_owned();
        let hooks = api.test_hooks();
        let running = tokio::spawn(api.run());
        let mut first = timeout(Duration::from_secs(5), authenticate(port, &token))
            .await
            .unwrap();
        let start = Instant::now();
        hooks.accept_retry.hold();
        hooks.fail_accepts(25);
        // The accept already waiting takes this socket; the next ones hit the injected failures.
        let _armed = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let late = tokio::spawn({
            let token = token.clone();
            async move { authenticate(port, &token).await }
        });
        timeout(
            Duration::from_secs(5),
            hooks.accept_retry.reached.notified(),
        )
        .await
        .unwrap();
        assert_eq!(hooks.accept_faults_left(), 24);
        // The service answers while the next accept attempt is held.
        let subscribe = json!({"jsonrpc": "2.0", "id": "01ARZ3NDEKTSV4RRFFQ69G5FAC", "method": "events.subscribe", "params": {"stream": "session/none", "after_seq": 0, "_meta": {"origin": "local", "idempotency_key": "01ARZ3NDEKTSV4RRFFQ69G5FAC"}}});
        first
            .send(Message::Text(subscribe.to_string().into()))
            .await
            .unwrap();
        let reply = timeout(Duration::from_secs(5), first.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let reply: Value = serde_json::from_str(reply.to_text().unwrap()).unwrap();
        assert_eq!(reply["result"]["accepted"], true);
        assert_eq!(hooks.accept_faults_left(), 24);
        hooks.accept_retry.release();
        timeout(Duration::from_secs(5), late)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hooks.accept_faults_left(), 0);
        assert!(start.elapsed() >= Duration::from_millis(500));
        assert!(!running.is_finished());
        running.abort();
        peer.abort();
    }

    #[tokio::test]
    async fn evicted_connections_still_count_against_the_pending_cap() {
        use crate::pty_client::ClientConfig;
        use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, server};
        let host =
            Arc::new(PtyHost::new(Arc::new(FakeBackend::new()), HostConfig::defaults()).unwrap());
        let (near, far) = tokio::io::duplex(4096);
        let peer = tokio::spawn(server::connection(host.clone(), far));
        let client = PtyClient::over(near, ClientConfig::recommended(host.config()))
            .await
            .unwrap();
        let api = LocalApi::bind(
            ApiConfig::from_toml_str("max_pending_auth = 1\nauth_timeout_ms = 60000").unwrap(),
            client,
            Arc::new(DisabledLauncher),
        )
        .await
        .unwrap();
        let port = api.port().unwrap();
        let hooks = api.test_hooks();
        let pending = api.pending.clone();
        let running = tokio::spawn(api.run());
        let wait_live = |count: usize| {
            let hooks = hooks.clone();
            async move {
                timeout(Duration::from_secs(5), async {
                    while hooks.pending().0 != count {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap()
            }
        };
        let mut oldest = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        wait_live(1).await;
        // The evicted task is held before it ends, so its slot is still taken.
        hooks.evicted_exit.hold();
        let _newest = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        timeout(
            Duration::from_secs(5),
            hooks.evicted_exit.reached.notified(),
        )
        .await
        .unwrap();
        assert_eq!(pending.available_permits(), 0);
        assert_eq!(hooks.pending().0, 1);
        assert_eq!(hooks.pending().1, 1);
        hooks.evicted_exit.release();
        use tokio::io::AsyncReadExt;
        let mut byte = [0u8; 1];
        let read = timeout(Duration::from_secs(5), oldest.read(&mut byte))
            .await
            .unwrap();
        assert!(matches!(read, Ok(0) | Err(_)));
        wait_live(1).await;
        assert_eq!(hooks.pending().1, 1);
        running.abort();
        peer.abort();
    }

    #[tokio::test]
    async fn invalidation_during_a_batch_suspends_the_remaining_frames() {
        let log = Arc::new(Mutex::new(EventLog::new(8, 4)));
        let stream = {
            let mut log = log.lock().await;
            for _ in 0..3 {
                log.append(
                    "s",
                    "resized",
                    json!({"session_id": "s", "cols": 80, "rows": 24}),
                    None,
                )
                .unwrap();
            }
            log.subscribe("session/s", 0).unwrap()
        };
        let Poll::Events(events) = stream.poll(0) else {
            panic!("expected events")
        };
        assert_eq!(events.len(), 3);
        let sent = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let (shared_log, shared_sent) = (log.clone(), sent.clone());
        // The first frame is still being sent when the session is invalidated.
        let mut sink = Box::pin(futures_util::sink::unfold(
            (),
            move |(), message: Message| {
                let (log, sent) = (shared_log.clone(), shared_sent.clone());
                async move {
                    sent.lock()
                        .unwrap()
                        .push(message.to_text().unwrap().to_owned());
                    log.lock().await.invalidate("s", Closing::Pending);
                    Ok::<_, std::convert::Infallible>(())
                }
            },
        ));
        let mut after = 0;
        deliver(
            &mut sink,
            events,
            &mut after,
            &stream,
            &ApiConfig::from_toml_str("").unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(sent.lock().unwrap().len(), 1);
        assert_eq!(after, 1);
        assert_eq!(stream.poll(after), Poll::Wait);
    }
}
