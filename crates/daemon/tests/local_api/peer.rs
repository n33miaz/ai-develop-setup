use super::{Fixture, TestLauncher};
use ads_daemon::api::{ApiConfig, LocalApi};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::wire::{self, Call, ResultBody};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PeerScenario {
    Plain,
    Recover,
    Stall,
    HostHoldSubscribe,
    HostLostOnSubscribe,
    FloodOnSubscribe,
    BusyOnOpen,
    ExitOtherThenHoldClose,
    ExitOtherThenDropClose,
    HostDropClose,
    ExitBeforeCloseResponse,
    GapBeforeCloseResponse,
    ExitThenHoldClose,
    ClosedBeforeCloseResponse { hold: bool },
    GapOutput,
    GapOutputDropClose,
    ExitOnWrite,
    GapExit,
    GapClose,
}

impl PeerScenario {
    fn uses_host(self) -> bool {
        matches!(
            self,
            Self::GapOutput
                | Self::GapOutputDropClose
                | Self::GapExit
                | Self::GapClose
                | Self::GapBeforeCloseResponse
                | Self::HostHoldSubscribe
                | Self::HostLostOnSubscribe
                | Self::HostDropClose
                | Self::Recover
        )
    }

    fn holds_close(self) -> bool {
        matches!(
            self,
            Self::ExitThenHoldClose
                | Self::ExitOtherThenHoldClose
                | Self::ClosedBeforeCloseResponse { hold: true }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum PeerCall {
    Hello,
    Open,
    Write,
    Resize,
    Close,
    Subscribe,
    Resubscribe,
    List,
}

impl From<&Call> for PeerCall {
    fn from(call: &Call) -> Self {
        match call {
            Call::Hello(_) => Self::Hello,
            Call::Open(_) => Self::Open,
            Call::Write(_) => Self::Write,
            Call::Resize(_) => Self::Resize,
            Call::Close(_) => Self::Close,
            Call::Subscribe(_) => Self::Subscribe,
            Call::List(_) => Self::List,
        }
    }
}

struct ScriptedPeer {
    scenario: PeerScenario,
    stream: DuplexStream,
    host: Arc<PtyHost>,
    release: Arc<Notify>,
    calls: Arc<Mutex<Vec<PeerCall>>>,
    budget: HashMap<PeerCall, usize>,
    code: wire::ErrorCode,
    error_text: Arc<Mutex<String>>,
    burst: usize,
}

enum SubscribeAction {
    Reply,
    SkipReply,
    Disconnect,
}

impl ScriptedPeer {
    async fn run(mut self) {
        let mut seen = HashSet::new();
        while let Ok(Some(wire::Frame::Json(payload))) =
            wire::read_frame(&mut self.stream, 131072).await
        {
            let request = wire::Request::parse(&payload).unwrap();
            let method = PeerCall::from(&request.call);
            self.calls.lock().unwrap().push(method);
            let first = seen.insert(method);
            let refuse = self.take_refusal(method, first);

            if method == PeerCall::Subscribe {
                match self.on_subscribe(first).await {
                    SubscribeAction::Reply => (),
                    SubscribeAction::SkipReply => continue,
                    SubscribeAction::Disconnect => break,
                }
            }
            if method == PeerCall::Close && first && self.drop_first_close().await {
                continue;
            }

            let outcome = self.outcome(&request.call, refuse);
            match &request.call {
                Call::Open(params) => self.on_open(params, outcome.is_ok()).await,
                Call::Close(params) => {
                    self.on_close(&params.session_id, outcome.is_ok(), first)
                        .await
                }
                Call::Write(params) => self.on_write(&params.session_id).await,
                _ => (),
            }
            let response = wire::Response {
                id: Some(request.id),
                outcome,
            };
            if self
                .stream
                .write_all(&wire::encode_json(&response).unwrap())
                .await
                .is_err()
            {
                break;
            }
        }
    }

    fn take_refusal(&mut self, method: PeerCall, first: bool) -> bool {
        let key = match (self.scenario, method) {
            (PeerScenario::Recover, PeerCall::Subscribe) if !first => PeerCall::Resubscribe,
            _ => method,
        };
        self.budget.get_mut(&key).is_some_and(|left| {
            let refuse = *left > 0;
            *left = left.saturating_sub(1);
            refuse
        })
    }

    fn outcome(&self, call: &Call, refuse: bool) -> Result<ResultBody, wire::ErrorBody> {
        if refuse {
            return Err(wire::ErrorBody::new(
                self.code,
                &self.error_text.lock().unwrap(),
                Some(wire::ErrorData {
                    oldest_seq: Some(1.try_into().unwrap()),
                    next_seq: Some(1.try_into().unwrap()),
                    limit: None,
                }),
            ));
        }
        Ok(match call {
            Call::Hello(_) => ResultBody::Hello(wire::HelloResult {
                contract: Default::default(),
            }),
            Call::Subscribe(_) => ResultBody::ReplayBounds(wire::ReplayBounds {
                oldest_seq: 1.try_into().unwrap(),
                next_seq: 1.try_into().unwrap(),
            }),
            Call::Close(_) => ResultBody::Closed(wire::Closed { exit_code: None }),
            _ => ResultBody::Empty(wire::Empty {}),
        })
    }

    async fn on_subscribe(&mut self, first: bool) -> SubscribeAction {
        match self.scenario {
            PeerScenario::HostHoldSubscribe if first => SubscribeAction::SkipReply,
            PeerScenario::HostLostOnSubscribe => SubscribeAction::Disconnect,
            PeerScenario::FloodOnSubscribe => {
                // Stop reading until resubscriptions fill the client's outgoing queue.
                let notice = subscription_closed(
                    &wire::SessionId::new("x").unwrap(),
                    0,
                    wire::CloseReason::SlowSubscriber,
                );
                while self.stream.write_all(&notice).await.is_ok() {}
                SubscribeAction::Disconnect
            }
            PeerScenario::BusyOnOpen => SubscribeAction::SkipReply,
            _ => SubscribeAction::Reply,
        }
    }

    async fn on_open(&mut self, params: &wire::OpenParams, accepted: bool) {
        if accepted && self.scenario.uses_host() {
            self.host
                .open(
                    &params.session_id,
                    &ads_ptyhost::SpawnSpec {
                        program: params.program.clone().into(),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        if self.scenario == PeerScenario::BusyOnOpen {
            for name in ["x", "y"] {
                self.send(&subscription_closed(
                    &wire::SessionId::new(name).unwrap(),
                    0,
                    wire::CloseReason::SlowSubscriber,
                ))
                .await;
            }
        }
    }

    async fn drop_first_close(&mut self) -> bool {
        if matches!(
            self.scenario,
            PeerScenario::ExitOtherThenHoldClose | PeerScenario::ExitOtherThenDropClose
        ) {
            self.send(&session_exited(&wire::SessionId::new("b").unwrap(), 0))
                .await;
        }
        matches!(
            self.scenario,
            PeerScenario::GapOutputDropClose
                | PeerScenario::HostDropClose
                | PeerScenario::ExitOtherThenDropClose
        )
    }

    async fn on_close(&mut self, id: &wire::SessionId, accepted: bool, first: bool) {
        if accepted && self.scenario.uses_host() {
            self.host.close(id).await.unwrap();
        }
        let notice = match self.scenario {
            PeerScenario::ExitBeforeCloseResponse | PeerScenario::ExitThenHoldClose => {
                Some(session_exited(id, 0))
            }
            PeerScenario::GapBeforeCloseResponse => Some(session_exited(id, 7)),
            PeerScenario::ClosedBeforeCloseResponse { .. } => {
                Some(subscription_closed(id, 0, wire::CloseReason::SessionClosed))
            }
            _ => None,
        };
        if let Some(notice) = notice {
            self.send(&notice).await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        if first && self.scenario.holds_close() {
            // Wait until the test has seen the terminal event processed.
            self.release.notified().await;
        }
    }

    async fn on_write(&mut self, id: &wire::SessionId) {
        match self.scenario {
            PeerScenario::Stall => std::future::pending::<()>().await,
            PeerScenario::Recover => {
                self.send(&subscription_closed(
                    id,
                    0,
                    wire::CloseReason::SlowSubscriber,
                ))
                .await
            }
            PeerScenario::ExitOtherThenHoldClose | PeerScenario::ExitOtherThenDropClose
                if id.as_str() == "a" =>
            {
                self.send(&session_exited(id, 0)).await
            }
            PeerScenario::GapOutput | PeerScenario::GapOutputDropClose => {
                self.send_output(id, 2).await
            }
            PeerScenario::ExitOnWrite => self.send(&session_exited(id, 0)).await,
            PeerScenario::GapExit => self.send(&session_exited(id, 1)).await,
            PeerScenario::GapClose => {
                self.send(&subscription_closed(
                    id,
                    1,
                    wire::CloseReason::SessionClosed,
                ))
                .await
            }
            _ => (),
        }
        for seq in 1..=self.burst {
            self.send_output(id, seq as u64).await;
        }
        if self.burst > 0 {
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    }

    async fn send(&mut self, frame: &[u8]) {
        self.stream.write_all(frame).await.unwrap();
    }

    async fn send_output(&mut self, id: &wire::SessionId, seq: u64) {
        self.send(&wire::encode_output(id, seq, b"terminal-sentinel").unwrap())
            .await;
    }
}

fn session_exited(id: &wire::SessionId, last_seq: u64) -> Vec<u8> {
    wire::encode_json(&wire::Notification::SessionExited(wire::SessionExited {
        session_id: id.clone(),
        last_seq,
        exit_code: Some(0),
    }))
    .unwrap()
}

fn subscription_closed(id: &wire::SessionId, last_seq: u64, reason: wire::CloseReason) -> Vec<u8> {
    wire::encode_json(&wire::Notification::SubscriptionClosed(
        wire::SubscriptionClosed {
            session_id: id.clone(),
            last_seq,
            reason,
        },
    ))
    .unwrap()
}

// A peer implements the public codec and injects refusals independent of terminal timing.
pub(super) async fn refusing_fixture(
    overrides: &str,
    method: PeerCall,
    code: wire::ErrorCode,
    burst: usize,
) -> (Fixture, Arc<Mutex<Vec<PeerCall>>>) {
    scripted_fixture(overrides, PeerScenario::Plain, code, burst, &[(method, 1)]).await
}

// In the recovery scenario, subscriptions after the first use the resubscribe budget.
pub(super) async fn scripted_fixture(
    overrides: &str,
    scenario: PeerScenario,
    code: wire::ErrorCode,
    burst: usize,
    refusals: &[(PeerCall, usize)],
) -> (Fixture, Arc<Mutex<Vec<PeerCall>>>) {
    let backend = Arc::new(FakeBackend::new());
    let host = Arc::new(PtyHost::new(backend.clone(), HostConfig::defaults()).unwrap());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let error_text = Arc::new(Mutex::new(String::new()));
    let (near, far) = tokio::io::duplex(4096);
    let release: Arc<Notify> = Arc::default();
    let peer = ScriptedPeer {
        scenario,
        stream: far,
        host: host.clone(),
        release: release.clone(),
        calls: calls.clone(),
        budget: refusals.iter().copied().collect(),
        code,
        error_text: error_text.clone(),
        burst,
    };
    let peer = tokio::spawn(peer.run());
    let client = PtyClient::over(
        near,
        ClientConfig {
            max_frame_bytes: 131072,
            queue_capacity: 2,
        },
    )
    .await
    .unwrap();
    let api = LocalApi::bind(
        ApiConfig::from_toml_str(overrides).unwrap(),
        client,
        Arc::new(TestLauncher),
    )
    .await
    .unwrap();
    let token = api
        .launch_url()
        .unwrap()
        .split_once("&token=")
        .unwrap()
        .1
        .to_owned();
    *error_text.lock().unwrap() = format!("{token} credential-and-terminal-sentinel");
    (
        Fixture {
            port: api.port().unwrap(),
            token,
            backend,
            host,
            hooks: api.test_hooks(),
            release,
            api: tokio::spawn(api.run()),
            peer,
        },
        calls,
    )
}
