use super::{Fixture, TestLauncher};
use ads_daemon::api::{ApiConfig, LocalApi};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost, wire::Call};
use std::sync::Arc;
use std::time::Duration;

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

// A peer implements the public codec and injects refusals independent of terminal timing.
pub(super) async fn refusing_fixture(
    overrides: &str,
    method: PeerCall,
    code: ads_ptyhost::wire::ErrorCode,
    burst: usize,
) -> (Fixture, Arc<std::sync::Mutex<Vec<PeerCall>>>) {
    scripted_fixture(overrides, PeerScenario::Plain, code, burst, &[(method, 1)]).await
}

// In the recovery scenario, subscriptions after the first use the resubscribe budget.
pub(super) async fn scripted_fixture(
    overrides: &str,
    scenario: PeerScenario,
    code: ads_ptyhost::wire::ErrorCode,
    burst: usize,
    refusals: &[(PeerCall, usize)],
) -> (Fixture, Arc<std::sync::Mutex<Vec<PeerCall>>>) {
    use ads_ptyhost::wire::{self, ResultBody};
    use tokio::io::AsyncWriteExt;
    let backend = Arc::new(FakeBackend::new());
    let host = Arc::new(PtyHost::new(backend.clone(), HostConfig::defaults()).unwrap());
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = calls.clone();
    let error_text = Arc::new(std::sync::Mutex::new(String::new()));
    let peer_error = error_text.clone();
    let (near, mut far) = tokio::io::duplex(4096);
    let peer_host = host.clone();
    let release: Arc<tokio::sync::Notify> = Arc::default();
    let peer_release = release.clone();
    let mut budget: std::collections::HashMap<PeerCall, usize> = refusals.iter().copied().collect();
    let peer = tokio::spawn(async move {
        let mut released = false;
        let mut dropped = false;
        let mut held = false;
        let mut other_exited = false;
        let mut subscriptions = 0;
        while let Ok(Some(wire::Frame::Json(payload))) = wire::read_frame(&mut far, 131072).await {
            let request = wire::Request::parse(&payload).unwrap();
            let method = PeerCall::from(&request.call);
            observed.lock().unwrap().push(method);
            if method == PeerCall::Subscribe {
                subscriptions += 1;
            }
            let key = if scenario == PeerScenario::Recover
                && method == PeerCall::Subscribe
                && subscriptions > 1
            {
                PeerCall::Resubscribe
            } else {
                method
            };
            let refuse = budget.get_mut(&key).is_some_and(|left| {
                let refuse = *left > 0;
                *left = left.saturating_sub(1);
                refuse
            });
            if method == PeerCall::Subscribe && scenario == PeerScenario::HostHoldSubscribe && !held
            {
                // The host opened the terminal but never answers the subscription.
                held = true;
                continue;
            }
            if method == PeerCall::Subscribe && scenario == PeerScenario::HostLostOnSubscribe {
                // The host goes away after opening the terminal, before answering the subscription.
                break;
            }
            if method == PeerCall::Subscribe && scenario == PeerScenario::FloodOnSubscribe {
                // The host stops reading and keeps asking for resubscriptions: the client's
                // outgoing queue fills and the client stops.
                let notice = wire::encode_json(&wire::Notification::SubscriptionClosed(
                    wire::SubscriptionClosed {
                        session_id: wire::SessionId::new("x").unwrap(),
                        last_seq: 0,
                        reason: wire::CloseReason::SlowSubscriber,
                    },
                ))
                .unwrap();
                while far.write_all(&notice).await.is_ok() {}
                break;
            }
            if method == PeerCall::Subscribe && scenario == PeerScenario::BusyOnOpen {
                // Resubscriptions stay unanswered and take every pending call of the client.
                continue;
            }
            if method == PeerCall::Close
                && matches!(
                    scenario,
                    PeerScenario::ExitOtherThenHoldClose | PeerScenario::ExitOtherThenDropClose
                )
                && !other_exited
            {
                // Another session ends while the daemon waits for this close.
                other_exited = true;
                far.write_all(
                    &wire::encode_json(&wire::Notification::SessionExited(wire::SessionExited {
                        session_id: wire::SessionId::new("b").unwrap(),
                        last_seq: 0,
                        exit_code: Some(0),
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();
            }
            if method == PeerCall::Close
                && matches!(
                    scenario,
                    PeerScenario::GapOutputDropClose
                        | PeerScenario::HostDropClose
                        | PeerScenario::ExitOtherThenDropClose
                )
                && !dropped
            {
                // The first close never gets a reply, as if the host stalled on it.
                dropped = true;
                continue;
            }
            let outcome = if refuse {
                Err(wire::ErrorBody::new(
                    code,
                    &peer_error.lock().unwrap(),
                    Some(wire::ErrorData {
                        oldest_seq: Some(1.try_into().unwrap()),
                        next_seq: Some(1.try_into().unwrap()),
                        limit: None,
                    }),
                ))
            } else {
                Ok(match &request.call {
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
            };
            if outcome.is_ok()
                && matches!(
                    scenario,
                    PeerScenario::GapOutput
                        | PeerScenario::GapOutputDropClose
                        | PeerScenario::GapExit
                        | PeerScenario::GapClose
                        | PeerScenario::GapBeforeCloseResponse
                        | PeerScenario::HostHoldSubscribe
                        | PeerScenario::HostLostOnSubscribe
                        | PeerScenario::HostDropClose
                        | PeerScenario::Recover
                )
            {
                match &request.call {
                    Call::Open(params) => {
                        peer_host
                            .open(
                                &params.session_id,
                                &ads_ptyhost::SpawnSpec {
                                    program: params.program.clone().into(),
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                    }
                    Call::Close(params) => {
                        peer_host.close(&params.session_id).await.unwrap();
                    }
                    _ => (),
                }
            }
            if let Call::Close(params) = &request.call
                && matches!(
                    scenario,
                    PeerScenario::ExitBeforeCloseResponse
                        | PeerScenario::GapBeforeCloseResponse
                        | PeerScenario::ExitThenHoldClose
                )
            {
                far.write_all(
                    &wire::encode_json(&wire::Notification::SessionExited(wire::SessionExited {
                        session_id: params.session_id.clone(),
                        last_seq: if scenario == PeerScenario::GapBeforeCloseResponse {
                            7
                        } else {
                            0
                        },
                        exit_code: Some(0),
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if let Call::Close(params) = &request.call
                && matches!(scenario, PeerScenario::ClosedBeforeCloseResponse { .. })
            {
                far.write_all(
                    &wire::encode_json(&wire::Notification::SubscriptionClosed(
                        wire::SubscriptionClosed {
                            session_id: params.session_id.clone(),
                            last_seq: 0,
                            reason: wire::CloseReason::SessionClosed,
                        },
                    ))
                    .unwrap(),
                )
                .await
                .unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if method == PeerCall::Close && scenario.holds_close() && !released {
                // The first close reply waits until the test saw the terminal event processed.
                released = true;
                peer_release.notified().await;
            }
            if method == PeerCall::Open && scenario == PeerScenario::BusyOnOpen {
                for name in ["x", "y"] {
                    far.write_all(
                        &wire::encode_json(&wire::Notification::SubscriptionClosed(
                            wire::SubscriptionClosed {
                                session_id: wire::SessionId::new(name).unwrap(),
                                last_seq: 0,
                                reason: wire::CloseReason::SlowSubscriber,
                            },
                        ))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                }
            }
            if let Call::Write(params) = &request.call {
                if scenario == PeerScenario::Stall {
                    std::future::pending::<()>().await;
                }
                if scenario == PeerScenario::Recover {
                    far.write_all(
                        &wire::encode_json(&wire::Notification::SubscriptionClosed(
                            wire::SubscriptionClosed {
                                session_id: params.session_id.clone(),
                                last_seq: 0,
                                reason: wire::CloseReason::SlowSubscriber,
                            },
                        ))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                }
                match scenario {
                    PeerScenario::ExitOtherThenHoldClose | PeerScenario::ExitOtherThenDropClose
                        if params.session_id.as_str() == "a" =>
                    {
                        far.write_all(
                            &wire::encode_json(&wire::Notification::SessionExited(
                                wire::SessionExited {
                                    session_id: params.session_id.clone(),
                                    last_seq: 0,
                                    exit_code: Some(0),
                                },
                            ))
                            .unwrap(),
                        )
                        .await
                        .unwrap()
                    }
                    PeerScenario::GapOutput | PeerScenario::GapOutputDropClose => far
                        .write_all(
                            &wire::encode_output(&params.session_id, 2, b"terminal-sentinel")
                                .unwrap(),
                        )
                        .await
                        .unwrap(),
                    PeerScenario::ExitOnWrite => far
                        .write_all(
                            &wire::encode_json(&wire::Notification::SessionExited(
                                wire::SessionExited {
                                    session_id: params.session_id.clone(),
                                    last_seq: 0,
                                    exit_code: Some(0),
                                },
                            ))
                            .unwrap(),
                        )
                        .await
                        .unwrap(),
                    PeerScenario::GapExit => far
                        .write_all(
                            &wire::encode_json(&wire::Notification::SessionExited(
                                wire::SessionExited {
                                    session_id: params.session_id.clone(),
                                    last_seq: 1,
                                    exit_code: Some(0),
                                },
                            ))
                            .unwrap(),
                        )
                        .await
                        .unwrap(),
                    PeerScenario::GapClose => far
                        .write_all(
                            &wire::encode_json(&wire::Notification::SubscriptionClosed(
                                wire::SubscriptionClosed {
                                    session_id: params.session_id.clone(),
                                    last_seq: 1,
                                    reason: wire::CloseReason::SessionClosed,
                                },
                            ))
                            .unwrap(),
                        )
                        .await
                        .unwrap(),
                    _ => (),
                }
                for seq in 1..=burst {
                    far.write_all(
                        &wire::encode_output(&params.session_id, seq as u64, b"terminal-sentinel")
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                }
                if burst > 0 {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
            }
            if far
                .write_all(
                    &wire::encode_json(&wire::Response {
                        id: Some(request.id),
                        outcome,
                    })
                    .unwrap(),
                )
                .await
                .is_err()
            {
                break;
            }
        }
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
