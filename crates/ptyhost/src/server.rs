//! Serves the `pty-host/1` contract: one task per client, sessions shared by all of them.

use std::collections::HashMap;
use std::io;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

use crate::channel::Listener;
use crate::host::PtyHost;
use crate::session::Subscription;
use crate::types::{PtyError, ReplayFrom, Seq, SpawnSpec, SubscriptionEnd, TerminalSize};
use crate::wire::{
    self, Call, CloseReason, Closed, ErrorBody, ErrorCode, ErrorData, Frame, HelloResult, NoParams,
    Notification, OpenParams, ReplayBounds, Request, Response, ResultBody, SessionEntry,
    SessionExited, SessionId, SessionList, SubscriptionClosed,
};

/// Accepts clients until the channel fails. Sessions outlive every client.
pub async fn serve(host: Arc<PtyHost>, mut listener: Listener) -> io::Result<()> {
    loop {
        let stream = listener.accept().await?;
        tokio::spawn(connection(Arc::clone(&host), stream));
    }
}

/// Talks to one client until it leaves or breaks the framing.
pub async fn connection<S>(host: Arc<PtyHost>, stream: S)
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut reader, writer) = tokio::io::split(stream);
    let max_frame_bytes = host.config().max_frame_bytes;
    let mut client = Client {
        host,
        writer: Arc::new(Mutex::new(writer)),
        greeted: false,
        forwarders: HashMap::new(),
    };
    // Anything but a JSON frame ends the connection: end of stream, unknown kind, a frame
    // larger than the limit, or an output frame, which only the host may send.
    while let Ok(Some(Frame::Json(payload))) = wire::read_frame(&mut reader, max_frame_bytes).await
    {
        let (response, subscription) = client.handle(&payload).await;
        if client.send(&response).await.is_err() {
            break;
        }
        // Output starts only after the client has the answer with the replay bounds.
        if let Some((session_id, subscription)) = subscription {
            client.forward(session_id, subscription).await;
        }
    }
    for (_, forwarder) in client.forwarders.drain() {
        forwarder.task.abort();
    }
}

type Writer<S> = Arc<Mutex<WriteHalf<S>>>;

/// The task that carries one subscription to the client.
struct Forwarder {
    cancel: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

struct Client<S> {
    host: Arc<PtyHost>,
    writer: Writer<S>,
    greeted: bool,
    forwarders: HashMap<SessionId, Forwarder>,
}

type Outcome = Result<ResultBody, ErrorBody>;

fn empty() -> Outcome {
    Ok(ResultBody::Empty(NoParams {}))
}

fn refuse(code: ErrorCode, message: &str) -> ErrorBody {
    ErrorBody::new(code, message, None)
}

fn wire_seq(seq: Seq) -> NonZeroU64 {
    NonZeroU64::new(seq.0).unwrap_or(NonZeroU64::MIN)
}

fn error_body(err: &PtyError) -> ErrorBody {
    let (code, data) = match err {
        PtyError::Config(_) => (ErrorCode::InternalError, None),
        PtyError::Spawn(_) => (ErrorCode::SpawnFailed, None),
        PtyError::Io(_) => (ErrorCode::TerminalIo, None),
        PtyError::UnknownSession(_) => (ErrorCode::UnknownSession, None),
        PtyError::SessionExists(_) => (ErrorCode::SessionExists, None),
        PtyError::TooManySessions { limit } => (
            ErrorCode::TooManySessions,
            Some(ErrorData {
                limit: NonZeroU64::new(*limit as u64),
                ..ErrorData::default()
            }),
        ),
        PtyError::ReplayEvicted { oldest, .. } => (
            ErrorCode::ReplayEvicted,
            Some(ErrorData {
                oldest_seq: Some(wire_seq(*oldest)),
                ..ErrorData::default()
            }),
        ),
        PtyError::ReplayAhead { next, .. } => (
            ErrorCode::ReplayAhead,
            Some(ErrorData {
                next_seq: Some(wire_seq(*next)),
                ..ErrorData::default()
            }),
        ),
        PtyError::SessionEnded => (ErrorCode::SessionEnded, None),
    };
    ErrorBody::new(code, &err.to_string(), data)
}

fn spawn_spec(params: OpenParams) -> SpawnSpec {
    SpawnSpec {
        program: params.program,
        args: params.args,
        cwd: params.cwd.map(PathBuf::from),
        env: params.env.into_iter().collect(),
        size: TerminalSize {
            rows: params.rows.get(),
            cols: params.cols.get(),
        },
    }
}

async fn write_frame<S: AsyncWrite>(writer: &Writer<S>, frame: &[u8]) -> io::Result<()> {
    // One frame per lock: responses, notifications and output never interleave.
    let mut writer = writer.lock().await;
    writer.write_all(frame).await?;
    writer.flush().await
}

/// Runs a call that may block on the terminal off the async threads.
async fn blocking<T, F>(work: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, PtyError> + Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result.map_err(|err| error_body(&err)),
        Err(err) => Err(refuse(ErrorCode::InternalError, &err.to_string())),
    }
}

impl<S> Client<S>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    async fn send(&self, response: &Response) -> io::Result<()> {
        let frame = wire::encode_json(response).map_err(io::Error::other)?;
        write_frame(&self.writer, &frame).await
    }

    async fn handle(&mut self, payload: &[u8]) -> (Response, Option<(SessionId, Subscription)>) {
        let request = match Request::parse(payload) {
            Ok(request) => request,
            Err(rejection) => {
                let error = refuse(rejection.code, &rejection.message);
                return (
                    Response {
                        id: rejection.id,
                        outcome: Err(error),
                    },
                    None,
                );
            }
        };
        let mut subscription = None;
        let outcome = self.call(request.call, &mut subscription).await;
        (
            Response {
                id: Some(request.id),
                outcome,
            },
            subscription,
        )
    }

    async fn call(
        &mut self,
        call: Call,
        subscription: &mut Option<(SessionId, Subscription)>,
    ) -> Outcome {
        if !self.greeted && !matches!(call, Call::Hello(_)) {
            return Err(refuse(
                ErrorCode::InvalidRequest,
                "host.hello must be the first request of a connection",
            ));
        }
        match call {
            Call::Hello(params) => {
                if params.contract != wire::CONTRACT {
                    return Err(refuse(
                        ErrorCode::UnsupportedContract,
                        &format!("this host speaks {} only", wire::CONTRACT),
                    ));
                }
                self.greeted = true;
                Ok(ResultBody::Hello(HelloResult {
                    contract: wire::CONTRACT.to_owned(),
                }))
            }
            Call::Open(params) => {
                let host = Arc::clone(&self.host);
                let id = params.session_id.clone();
                let spec = spawn_spec(params);
                blocking(move || host.open(&id, &spec)).await?;
                empty()
            }
            Call::Write(params) => {
                let host = Arc::clone(&self.host);
                blocking(move || host.write(&params.session_id, &params.data.0)).await?;
                empty()
            }
            Call::Resize(params) => {
                let size = TerminalSize {
                    rows: params.rows.get(),
                    cols: params.cols.get(),
                };
                self.host
                    .resize(&params.session_id, size)
                    .map_err(|err| error_body(&err))?;
                empty()
            }
            Call::Subscribe(params) => {
                let from = params
                    .from_seq
                    .map_or(ReplayFrom::Oldest, |seq| ReplayFrom::Seq(Seq(seq.get())));
                let opened = self
                    .host
                    .subscribe(&params.session_id, from)
                    .map_err(|err| error_body(&err))?;
                // The previous subscription of this client to the session stops here, between two
                // frames: after the answer, every output frame belongs to the new one.
                self.stop_forwarding(&params.session_id).await;
                let bounds = ReplayBounds {
                    oldest_seq: wire_seq(opened.oldest_seq()),
                    next_seq: wire_seq(opened.next_seq()),
                };
                *subscription = Some((params.session_id, opened));
                Ok(ResultBody::ReplayBounds(bounds))
            }
            Call::Close(params) => {
                let exit_code = match self.host.close(&params.session_id).await {
                    Ok(status) => Some(i64::from(status.code)),
                    // Closed and gone, but the process never reported how it ended.
                    Err(PtyError::SessionEnded) => None,
                    Err(err) => return Err(error_body(&err)),
                };
                Ok(ResultBody::Closed(Closed { exit_code }))
            }
            Call::List(_) => Ok(ResultBody::SessionList(SessionList {
                sessions: self
                    .host
                    .sessions()
                    .into_iter()
                    .map(|info| SessionEntry {
                        session_id: info.id,
                        running: info.exit.is_none(),
                        exit_code: info.exit.map(|status| i64::from(status.code)),
                        oldest_seq: wire_seq(info.oldest_seq),
                        next_seq: wire_seq(info.next_seq),
                    })
                    .collect(),
            })),
        }
    }

    async fn stop_forwarding(&mut self, session_id: &SessionId) {
        if let Some(previous) = self.forwarders.remove(session_id) {
            let _ = previous.cancel.send(());
            let _ = previous.task.await;
        }
    }

    /// Starts carrying a subscription. A client has one per session.
    async fn forward(&mut self, session_id: SessionId, subscription: Subscription) {
        self.stop_forwarding(&session_id).await;
        let (cancel, cancelled) = oneshot::channel();
        let task = tokio::spawn(forward(
            Arc::clone(&self.writer),
            session_id.clone(),
            subscription,
            cancelled,
        ));
        self.forwarders
            .insert(session_id, Forwarder { cancel, task });
    }
}

async fn forward<S: AsyncWrite>(
    writer: Writer<S>,
    session_id: SessionId,
    mut subscription: Subscription,
    mut cancelled: oneshot::Receiver<()>,
) {
    loop {
        let chunk = tokio::select! {
            biased;
            _ = &mut cancelled => return,
            chunk = subscription.recv() => chunk,
        };
        let Some(chunk) = chunk else { break };
        let Ok(frame) = wire::encode_output(&session_id, chunk.seq.0, &chunk.bytes) else {
            return;
        };
        if write_frame(&writer, &frame).await.is_err() {
            return;
        }
    }
    let last_seq = subscription.last_seq();
    let closed = |reason| {
        Notification::SubscriptionClosed(SubscriptionClosed {
            session_id: session_id.clone(),
            reason,
            last_seq,
        })
    };
    let notification = match subscription.end().await {
        SubscriptionEnd::Exited(status) => Notification::SessionExited(SessionExited {
            session_id: session_id.clone(),
            exit_code: Some(i64::from(status.code)),
            last_seq,
        }),
        SubscriptionEnd::SessionClosed => closed(CloseReason::SessionClosed),
        SubscriptionEnd::Lagged => closed(CloseReason::SlowSubscriber),
    };
    if let Ok(frame) = wire::encode_json(&notification) {
        let _ = write_frame(&writer, &frame).await;
    }
}
