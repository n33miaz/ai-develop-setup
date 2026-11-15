//! Minimal `pty-host/1` client for the tests: frames in, frames out, nothing else.

use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Instant;

use ads_ptyhost::wire::{
    self, Call, ErrorBody, Frame, HelloParams, HostMessage, Notification, OutputHeader,
    ReplayBounds, Request, Response, ResultBody, SessionId, SubscribeParams,
};
use ads_ptyhost::{PtyHost, server};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};

use super::STEP_TIMEOUT;

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[cfg(windows)]
pub type ChannelStream = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
pub type ChannelStream = tokio::net::UnixStream;

/// A channel path no other test or real host uses.
#[cfg(windows)]
pub fn unique_endpoint(label: &str) -> String {
    format!(r"\\.\pipe\ads-pty-{}-{label}", std::process::id())
}

/// A channel path no other test or real host uses, inside the Cargo build folder.
#[cfg(unix)]
pub fn unique_endpoint(label: &str) -> String {
    let dir = env!("CARGO_TARGET_TMPDIR");
    format!("{dir}/ads-pty-{}-{label}.sock", std::process::id())
}

/// Opens the channel, waiting while the host is busy with another client or still starting.
pub async fn open_channel(endpoint: &str) -> ChannelStream {
    let deadline = Instant::now() + STEP_TIMEOUT;
    loop {
        #[cfg(windows)]
        let attempt = tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint);
        #[cfg(unix)]
        let attempt = tokio::net::UnixStream::connect(endpoint).await;
        match attempt {
            Ok(stream) => return stream,
            Err(err) if Instant::now() < deadline => {
                let _ = err;
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
            Err(err) => panic!("could not open the channel {endpoint}: {err}"),
        }
    }
}

/// What the host sends without being asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    Output {
        header: OutputHeader,
        bytes: Vec<u8>,
    },
    Notification(Notification),
}

pub struct Client<S> {
    stream: S,
    next_id: u64,
    inbox: VecDeque<Incoming>,
}

async fn step<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(STEP_TIMEOUT, future)
        .await
        .unwrap_or_else(|_| panic!("{what}: nothing within the step timeout"))
}

impl Client<ChannelStream> {
    /// Connects through the real channel and says hello.
    pub async fn connect(endpoint: &str) -> Self {
        let mut client = Self::over(open_channel(endpoint).await);
        client.hello().await;
        client
    }
}

impl Client<DuplexStream> {
    /// A client wired straight to the connection handler, with no channel in between.
    /// `pipe_bytes` is how much the host can write before the client has to read.
    pub fn attach(host: &Arc<PtyHost>, pipe_bytes: usize) -> Self {
        let (near, far) = tokio::io::duplex(pipe_bytes);
        tokio::spawn(server::connection(Arc::clone(host), far));
        Self::over(near)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Client<S> {
    pub fn over(stream: S) -> Self {
        Self {
            stream,
            next_id: 1,
            inbox: VecDeque::new(),
        }
    }

    pub async fn hello(&mut self) {
        let hello = Call::Hello(HelloParams {
            contract: wire::CONTRACT.to_owned(),
        });
        self.call(hello).await.expect("hello refused");
    }

    pub async fn send_bytes(&mut self, bytes: &[u8]) {
        step("send", self.stream.write_all(bytes))
            .await
            .expect("the channel refused the bytes");
    }

    async fn read_any(&mut self) -> Option<Result<Response, Incoming>> {
        let frame = step("read", wire::read_frame(&mut self.stream, MAX_FRAME_BYTES))
            .await
            .ok()??;
        Some(match frame {
            Frame::Output { header, bytes } => Err(Incoming::Output { header, bytes }),
            Frame::Json(payload) => match HostMessage::parse(&payload) {
                Ok(HostMessage::Response(response)) => Ok(response),
                Ok(HostMessage::Notification(notification)) => {
                    Err(Incoming::Notification(notification))
                }
                Err(err) => panic!(
                    "the host sent a message outside the contract: {err}: {}",
                    String::from_utf8_lossy(&payload)
                ),
            },
        })
    }

    /// The next response, keeping aside whatever arrives before it.
    pub async fn response(&mut self) -> Response {
        loop {
            match self
                .read_any()
                .await
                .expect("channel closed before the response")
            {
                Ok(response) => return response,
                Err(incoming) => self.inbox.push_back(incoming),
            }
        }
    }

    pub async fn call(&mut self, call: Call) -> Result<ResultBody, ErrorBody> {
        let id = NonZeroU64::new(self.next_id).unwrap();
        self.next_id += 1;
        let frame = wire::encode_json(&Request { id, call }).unwrap();
        self.send_bytes(&frame).await;
        let response = self.response().await;
        assert_eq!(response.id, Some(id), "response to another request");
        response.outcome
    }

    pub async fn subscribe(
        &mut self,
        session_id: &SessionId,
        from_seq: Option<u64>,
    ) -> Result<ReplayBounds, ErrorBody> {
        let call = Call::Subscribe(SubscribeParams {
            session_id: session_id.clone(),
            from_seq: from_seq.map(|seq| NonZeroU64::new(seq).unwrap()),
        });
        match self.call(call).await? {
            ResultBody::ReplayBounds(bounds) => Ok(bounds),
            other => panic!("unexpected subscribe result: {other:?}"),
        }
    }

    /// The next output or notification; `None` when the host closed the channel.
    pub async fn next(&mut self) -> Option<Incoming> {
        if let Some(incoming) = self.inbox.pop_front() {
            return Some(incoming);
        }
        match self.read_any().await? {
            Ok(response) => panic!("response nobody asked for: {response:?}"),
            Err(incoming) => Some(incoming),
        }
    }

    /// Reads exactly `bytes` more bytes of the channel and returns the whole frames in them:
    /// what a client saw before it died. The flag says the cut fell in the middle of a frame.
    pub async fn read_cut_at(&mut self, bytes: u64) -> (Vec<Incoming>, bool) {
        let mut seen: Vec<Incoming> = self.inbox.drain(..).collect();
        let mut cut = (&mut self.stream).take(bytes);
        loop {
            let frame = match step("read", wire::read_frame(&mut cut, MAX_FRAME_BYTES)).await {
                Ok(Some(frame)) => frame,
                Ok(None) => return (seen, false),
                Err(_) => return (seen, true),
            };
            match frame {
                Frame::Output { header, bytes } => seen.push(Incoming::Output { header, bytes }),
                Frame::Json(payload) => match HostMessage::parse(&payload) {
                    Ok(HostMessage::Notification(notification)) => {
                        seen.push(Incoming::Notification(notification));
                    }
                    other => panic!("unexpected message in the output stream: {other:?}"),
                },
            }
        }
    }
}

/// Follows the output of one session across reconnections, refusing any gap or repeat.
#[derive(Debug, Default)]
pub struct Transcript {
    pub last_seq: u64,
    pub bytes: Vec<u8>,
}

impl Transcript {
    pub fn accept(&mut self, session_id: &SessionId, header: &OutputHeader, bytes: &[u8]) {
        assert_eq!(&header.session_id, session_id, "output of another session");
        assert_eq!(
            header.seq.get(),
            self.last_seq + 1,
            "gap or repeat in the stream"
        );
        assert_eq!(header.length.get() as usize, bytes.len());
        self.last_seq = header.seq.get();
        self.bytes.extend_from_slice(bytes);
    }
}
