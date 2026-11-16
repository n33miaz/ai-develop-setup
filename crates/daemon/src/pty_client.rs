//! Asynchronous client of the host's public `pty-host/1` codec.

use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU64;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use ads_ptyhost::HostConfig;
use ads_ptyhost::channel::{self, ConnectError};
use ads_ptyhost::wire::{
    self, Call, Frame, HelloParams, HostMessage, Notification, Request, ResultBody,
};
pub use ads_ptyhost::wire::{
    Closed, ErrorCode, ErrorData, OpenParams, OutputHeader, ReplayBounds, ResizeParams, SessionId,
    SessionList, SessionParams, SubscribeParams, WriteParams,
};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch};

/// Errors contain only codes and replay limits, never host messages or terminal data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    Host {
        code: ErrorCode,
        data: Option<ErrorData>,
    },
    Transport,
    /// The channel is not served by the current user; nothing was sent to it.
    NotOwnedByUser,
    Protocol,
    Disconnected,
    Capacity,
    SequenceOverflow,
    InvalidConfig,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host { code, .. } => write!(f, "pty host refused request ({})", code.number()),
            other => f.write_str(match other {
                Self::Transport => "pty channel failed",
                Self::NotOwnedByUser => "pty channel is not served by the current user",
                Self::Protocol => "invalid pty host message",
                Self::Disconnected => "pty client disconnected",
                Self::Capacity => "pty client capacity exceeded",
                Self::SequenceOverflow => "pty sequence exhausted",
                Self::InvalidConfig => "invalid pty client configuration",
                Self::Host { .. } => unreachable!(),
            }),
        }
    }
}
impl std::error::Error for ClientError {}

/// Bounds are supplied by the caller; `recommended` reuses the host's frame/queue limits.
#[derive(Debug, Clone, Copy)]
pub struct ClientConfig {
    pub max_frame_bytes: usize,
    pub queue_capacity: usize,
}
impl ClientConfig {
    pub fn recommended(host: &HostConfig) -> Self {
        Self {
            max_frame_bytes: host.max_frame_bytes,
            queue_capacity: host.subscriber_queue_chunks,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Event {
    Output {
        header: OutputHeader,
        bytes: Vec<u8>,
    },
    Notification(Notification),
    ResubscribeFailed {
        session_id: SessionId,
        error: ClientError,
    },
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Output { bytes, .. } => f
                .debug_struct("Output")
                .field("length", &bytes.len())
                .finish_non_exhaustive(),
            Self::Notification(_) => f.write_str("Notification([redacted])"),
            Self::ResubscribeFailed { error, .. } => f
                .debug_struct("ResubscribeFailed")
                .field("error", error)
                .finish_non_exhaustive(),
        }
    }
}

type Reply = oneshot::Sender<Result<ResultBody, ClientError>>;
struct Command {
    call: Call,
    reply: Reply,
}

/// Dropping the client closes its connection, leaving the host's sessions alive.
/// A dedicated reader preserves partial frames when a caller cancels an operation.
pub struct PtyClient {
    handle: PtyCommands,
    events: mpsc::Receiver<Event>,
    worker: tokio::task::JoinHandle<()>,
    backpressure: Arc<AtomicBool>,
}
#[derive(Clone)]
pub(crate) struct PtyCommands {
    commands: mpsc::Sender<Command>,
    failure: watch::Receiver<Option<ClientError>>,
}
impl Drop for PtyClient {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

impl PtyClient {
    pub async fn connect(host: &HostConfig, config: ClientConfig) -> Result<Self, ClientError> {
        let endpoint = channel::endpoint(host).map_err(|_| ClientError::Transport)?;
        // On Windows the channel checks who serves the pipe before handing the stream over.
        let stream = channel::connect(&endpoint)
            .await
            .map_err(|error| match error {
                ConnectError::NotOwnedByUser => ClientError::NotOwnedByUser,
                ConnectError::Unavailable(_) => ClientError::Transport,
            })?;
        Self::over(stream, config).await
    }

    /// Attaches to a transport and completes `host.hello` before returning.
    pub async fn over<S>(stream: S, config: ClientConfig) -> Result<Self, ClientError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        if config.max_frame_bytes == 0
            || config.queue_capacity == 0
            || config.queue_capacity > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(ClientError::InvalidConfig);
        }
        let (commands, rx) = mpsc::channel(config.queue_capacity);
        let (tx, events) = mpsc::channel(config.queue_capacity);
        let (failure_tx, failure) = watch::channel(None);
        let backpressure = Arc::new(AtomicBool::new(false));
        let mode = backpressure.clone();
        let worker = tokio::spawn(async move {
            let result = run(stream, config, rx, tx, mode).await;
            failure_tx.send_replace(Some(result.err().unwrap_or(ClientError::Disconnected)));
        });
        let client = Self {
            handle: PtyCommands {
                commands,
                failure: failure.clone(),
            },
            events,
            worker,
            backpressure,
        };
        match client
            .handle
            .call(Call::Hello(HelloParams {
                contract: wire::CONTRACT.to_owned(),
            }))
            .await?
        {
            ResultBody::Hello(_) => Ok(client),
            _ => Err(ClientError::Protocol),
        }
    }

    pub async fn open(&self, params: OpenParams) -> Result<(), ClientError> {
        self.handle.open(params).await
    }
    pub async fn write(&self, params: WriteParams) -> Result<(), ClientError> {
        self.handle.write(params).await
    }
    pub async fn resize(&self, params: ResizeParams) -> Result<(), ClientError> {
        self.handle.resize(params).await
    }
    pub async fn close(&self, params: SessionParams) -> Result<Closed, ClientError> {
        self.handle.close(params).await
    }
    pub async fn subscribe(
        &self,
        session_id: SessionId,
        after_seq: Option<u64>,
    ) -> Result<ReplayBounds, ClientError> {
        self.handle.subscribe(session_id, after_seq).await
    }
    pub async fn list(&self) -> Result<SessionList, ClientError> {
        self.handle.list().await
    }
    pub(crate) fn enable_backpressure(&self) {
        self.backpressure.store(true, Ordering::Release);
    }
    pub(crate) fn commands(&self) -> PtyCommands {
        self.handle.clone()
    }
    /// Queue overflow terminates this connection explicitly; reconnect using the last consumed seq.
    pub async fn next(&mut self) -> Result<Event, ClientError> {
        match self.events.recv().await {
            Some(event) => Ok(event),
            None => Err(self.handle.failure().await),
        }
    }
}

fn subscription(
    session_id: SessionId,
    after_seq: Option<u64>,
) -> Result<SubscribeParams, ClientError> {
    let from_seq = after_seq
        .map(|n| {
            n.checked_add(1)
                .and_then(NonZeroU64::new)
                .ok_or(ClientError::SequenceOverflow)
        })
        .transpose()?;
    Ok(SubscribeParams {
        session_id,
        from_seq,
    })
}

struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn run<S>(
    stream: S,
    config: ClientConfig,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    backpressure: Arc<AtomicBool>,
) -> Result<(), ClientError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (frames_tx, mut frames) = mpsc::channel(1);
    let errors = frames_tx.clone();
    let (writes, mut outbound) = mpsc::channel::<Vec<u8>>(config.queue_capacity);
    let _writer = AbortTask(tokio::spawn(async move {
        while let Some(frame) = outbound.recv().await {
            if writer.write_all(&frame).await.is_err() {
                let _ = errors.send(Err(ClientError::Transport)).await;
                break;
            }
        }
    }));
    let _reader = AbortTask(tokio::spawn(async move {
        loop {
            let frame = wire::read_frame(&mut reader, config.max_frame_bytes)
                .await
                .map_err(|_| ClientError::Protocol)
                .and_then(|f| f.ok_or(ClientError::Disconnected));
            let ended = frame.is_err();
            if frames_tx.send(frame).await.is_err() || ended {
                break;
            }
        }
    }));
    let mut pending = HashMap::<NonZeroU64, Reply>::new();
    let mut recovery = HashMap::<NonZeroU64, SessionId>::new();
    let mut next_id = Some(NonZeroU64::MIN);
    loop {
        let (call, reply, recovering) = tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { return Ok(()); };
                if pending.len() + recovery.len() >= config.queue_capacity {
                    let _ = command.reply.send(Err(ClientError::Capacity));
                    continue;
                }
                (command.call, Some(command.reply), None)
            }
            frame = frames.recv() => {
                let frame = frame.ok_or(ClientError::Disconnected)??;
                let notification = match frame {
                    Frame::Output { header, bytes } => {
                        deliver(&events, &backpressure, Event::Output { header, bytes }).await?;
                        continue;
                    }
                    Frame::Json(payload) => match HostMessage::parse(&payload).map_err(|_| ClientError::Protocol)? {
                        HostMessage::Response(response) => {
                            let id = response.id.ok_or(ClientError::Protocol)?;
                            let outcome = response.outcome.map_err(|e| ClientError::Host { code: e.code, data: e.data });
                            if let Some(reply) = pending.remove(&id) { let _ = reply.send(outcome); }
                            else if let Some(session_id) = recovery.remove(&id) {
                                match outcome {
                                    Ok(ResultBody::ReplayBounds(_)) => (),
                                    Ok(_) => return Err(ClientError::Protocol),
                                    Err(error) => deliver(&events, &backpressure, Event::ResubscribeFailed { session_id, error }).await?,
                                }
                            } else { return Err(ClientError::Protocol); }
                            continue;
                        }
                        HostMessage::Notification(notification) => notification,
                    }
                };
                deliver(&events, &backpressure, Event::Notification(notification.clone())).await?;
                match notification {
                    Notification::SubscriptionClosed(closed) => {
                        let session_id = closed.session_id;
                        let call = Call::Subscribe(subscription(session_id.clone(), Some(closed.last_seq))?);
                        (call, None, Some(session_id))
                    }
                    _ => continue,
                }
            }
        };
        let id = next_id.ok_or(ClientError::SequenceOverflow)?;
        next_id = id.get().checked_add(1).and_then(NonZeroU64::new);
        let frame = wire::encode_json(&Request { id, call }).map_err(|_| ClientError::Protocol)?;
        writes.try_send(frame).map_err(|_| ClientError::Capacity)?;
        if let Some(reply) = reply {
            pending.insert(id, reply);
        }
        if let Some(session_id) = recovering {
            recovery.insert(id, session_id);
        }
    }
}

impl PtyCommands {
    async fn failure(&self) -> ClientError {
        let mut failure = self.failure.clone();
        loop {
            if let Some(error) = failure.borrow().clone() {
                return error;
            }
            if failure.changed().await.is_err() {
                return ClientError::Disconnected;
            }
        }
    }

    async fn call(&self, call: Call) -> Result<ResultBody, ClientError> {
        let (reply, answer) = oneshot::channel();
        if self.commands.send(Command { call, reply }).await.is_err() {
            return Err(self.failure().await);
        }
        match answer.await {
            Ok(result) => result,
            Err(_) => Err(self.failure().await),
        }
    }
    pub async fn open(&self, params: OpenParams) -> Result<(), ClientError> {
        match self.call(Call::Open(params)).await? {
            ResultBody::Empty(_) => Ok(()),
            _ => Err(ClientError::Protocol),
        }
    }
    pub async fn write(&self, params: WriteParams) -> Result<(), ClientError> {
        match self.call(Call::Write(params)).await? {
            ResultBody::Empty(_) => Ok(()),
            _ => Err(ClientError::Protocol),
        }
    }
    pub async fn resize(&self, params: ResizeParams) -> Result<(), ClientError> {
        match self.call(Call::Resize(params)).await? {
            ResultBody::Empty(_) => Ok(()),
            _ => Err(ClientError::Protocol),
        }
    }
    /// `None` replays the oldest available chunk; `Some(n)` starts strictly after n.
    pub async fn subscribe(
        &self,
        session_id: SessionId,
        after_seq: Option<u64>,
    ) -> Result<ReplayBounds, ClientError> {
        match self
            .call(Call::Subscribe(subscription(session_id, after_seq)?))
            .await?
        {
            ResultBody::ReplayBounds(bounds) => Ok(bounds),
            _ => Err(ClientError::Protocol),
        }
    }
    pub async fn close(&self, params: SessionParams) -> Result<Closed, ClientError> {
        match self.call(Call::Close(params)).await? {
            ResultBody::Closed(closed) => Ok(closed),
            _ => Err(ClientError::Protocol),
        }
    }
    pub async fn list(&self) -> Result<SessionList, ClientError> {
        match self.call(Call::List(wire::NoParams {})).await? {
            ResultBody::SessionList(list) => Ok(list),
            _ => Err(ClientError::Protocol),
        }
    }
}

async fn deliver(
    events: &mpsc::Sender<Event>,
    backpressure: &AtomicBool,
    event: Event,
) -> Result<(), ClientError> {
    if backpressure.load(Ordering::Acquire) {
        events
            .send(event)
            .await
            .map_err(|_| ClientError::Disconnected)
    } else {
        events.try_send(event).map_err(|_| ClientError::Capacity)
    }
}
