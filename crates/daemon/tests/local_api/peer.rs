use super::{Fixture, TestLauncher};
use ads_daemon::api::{ApiConfig, LocalApi};
use ads_daemon::pty_client::{ClientConfig, PtyClient};
use ads_ptyhost::{FakeBackend, HostConfig, PtyHost};
use std::sync::Arc;
use std::time::Duration;

// A peer implements the public codec and injects refusals independent of terminal timing.
pub(super) async fn refusing_fixture(
    overrides: &str,
    target: &'static str,
    code: ads_ptyhost::wire::ErrorCode,
    burst: usize,
) -> (Fixture, Arc<std::sync::Mutex<Vec<String>>>) {
    scripted_fixture(overrides, target, code, burst, &[(target, 1)]).await
}

// `refusals` gives how many calls of each method the peer refuses ("recover" counts resubscriptions).
pub(super) async fn scripted_fixture(
    overrides: &str,
    target: &'static str,
    code: ads_ptyhost::wire::ErrorCode,
    burst: usize,
    refusals: &[(&'static str, usize)],
) -> (Fixture, Arc<std::sync::Mutex<Vec<String>>>) {
    use ads_ptyhost::wire::{self, Call, ResultBody};
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
    let mut budget: std::collections::HashMap<&'static str, usize> =
        refusals.iter().copied().collect();
    let peer = tokio::spawn(async move {
        let mut released = false;
        let mut dropped = false;
        let mut held = false;
        let mut other_exited = false;
        let mut subscriptions = 0;
        while let Ok(Some(wire::Frame::Json(payload))) = wire::read_frame(&mut far, 131072).await {
            let request = wire::Request::parse(&payload).unwrap();
            let method = match &request.call {
                Call::Hello(_) => "hello",
                Call::Open(_) => "open",
                Call::Write(_) => "write",
                Call::Resize(_) => "resize",
                Call::Close(_) => "close",
                Call::Subscribe(_) => "subscribe",
                Call::List(_) => "list",
            };
            observed.lock().unwrap().push(method.to_owned());
            if method == "subscribe" {
                subscriptions += 1;
            }
            let key = if target == "recover" && method == "subscribe" && subscriptions > 1 {
                "recover"
            } else {
                method
            };
            let refuse = budget.get_mut(key).is_some_and(|left| {
                let refuse = *left > 0;
                *left = left.saturating_sub(1);
                refuse
            });
            if method == "subscribe" && target == "host-hold-subscribe" && !held {
                // The host opened the terminal but never answers the subscription.
                held = true;
                continue;
            }
            if method == "subscribe" && target == "host-lost-on-subscribe" {
                // The host goes away after opening the terminal, before answering the subscription.
                break;
            }
            if method == "subscribe" && target == "flood-on-subscribe" {
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
            if method == "subscribe" && target == "busy-on-open" {
                // Resubscriptions stay unanswered and take every pending call of the client.
                continue;
            }
            if method == "close" && target.starts_with("exit-other-") && !other_exited {
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
            if method == "close" && target.ends_with("drop-close") && !dropped {
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
                && (target.starts_with("gap-")
                    || target.starts_with("host-")
                    || target == "recover")
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
                    target,
                    "exit-before-close-response"
                        | "gap-before-close-response"
                        | "exit-then-hold-close"
                )
            {
                far.write_all(
                    &wire::encode_json(&wire::Notification::SessionExited(wire::SessionExited {
                        session_id: params.session_id.clone(),
                        last_seq: if target == "gap-before-close-response" {
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
                && matches!(
                    target,
                    "closed-before-close-response" | "closed-then-hold-close"
                )
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
            if matches!(request.call, Call::Close(_))
                && target.ends_with("-then-hold-close")
                && !released
            {
                // The first close reply waits until the test saw the terminal event processed.
                released = true;
                peer_release.notified().await;
            }
            if matches!(request.call, Call::Open(_)) && target == "busy-on-open" {
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
                if target == "stall" {
                    std::future::pending::<()>().await;
                }
                if target == "recover" {
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
                match target {
                    "exit-other-then-hold-close" | "exit-other-then-drop-close"
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
                    "gap-output" | "gap-output-drop-close" => far
                        .write_all(
                            &wire::encode_output(&params.session_id, 2, b"terminal-sentinel")
                                .unwrap(),
                        )
                        .await
                        .unwrap(),
                    "exit-on-write" => far
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
                    "gap-exit" => far
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
                    "gap-close" => far
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
