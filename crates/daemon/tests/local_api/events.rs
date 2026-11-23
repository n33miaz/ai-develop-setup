use super::{
    Fixture, PeerCall, PeerScenario, Socket, TestLauncher, authenticated, bounded, call, closed,
    command, fixture, host_sessions_settle, open_params, receive, refusing_fixture,
    scripted_fixture, transmit, wait_until,
};
use ads_daemon::api::ApiError;
use ads_daemon::events::{EventLog, Poll};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn fake_session_emits_valid_ordered_events_and_replays_after_sequence() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut control = authenticated(&f).await;
    let mut events = authenticated(&f).await;
    assert_eq!(
        call(
            &mut events,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    call(&mut control, "session.open", open_params("session")).await;
    let created = receive(&mut events).await;
    assert_eq!(created["seq"], 1);
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
    for (offset, bytes) in [(0, b"hello".as_slice()), (5, &[0, 255, 1][..])] {
        call(
            &mut control,
            "session.write",
            json!({"session_id": "session", "data": STANDARD.encode(bytes)}),
        )
        .await;
        let event = receive(&mut events).await;
        assert_eq!(event["data"]["offset"], offset);
        assert_eq!(event["data"]["length"], bytes.len());
        assert_eq!(event["data"]["payload"], STANDARD.encode(bytes));
        assert_eq!(event["seq"], if offset == 0 { 2 } else { 3 });
    }
    call(
        &mut control,
        "session.resize",
        json!({"session_id": "session", "cols": 100, "rows": 30}),
    )
    .await;
    let resized = receive(&mut events).await;
    assert_eq!(resized["seq"], 4);
    assert_eq!(resized["type"], "urn:ads:event:session.resized:1");
    assert_eq!(f.backend.terminals()[0].size().cols, 100);
    call(
        &mut control,
        "session.close",
        json!({"session_id": "session"}),
    )
    .await;
    let ended = receive(&mut events).await;
    assert_eq!(ended["seq"], 5);
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert!(f.backend.terminals()[0].was_killed());
    let mut replay = authenticated(&f).await;
    call(
        &mut replay,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 3}),
    )
    .await;
    assert_eq!(receive(&mut replay).await, resized);
    assert_eq!(receive(&mut replay).await, ended);
}

#[test]
fn event_log_rejects_invalid_schema_and_preserves_replay_bounds() {
    let mut log = EventLog::new(2, 4);
    assert_eq!(log.append("valid", "output", json!({"session_id": "valid", "offset": -1, "length": 0, "encoding": "base64", "payload": "%%%"}), None), Err(ApiError::Invalid));
    for cols in [80, 81, 82] {
        log.append(
            "valid",
            "resized",
            json!({"session_id": "valid", "cols": cols, "rows": 24}),
            None,
        )
        .unwrap();
    }
    assert_eq!(log.after("session/valid", 0).unwrap().len(), 2);
    assert_eq!(log.after("session/valid", 1).unwrap()[0]["seq"], 2);
    assert_eq!(log.after("session/valid", 2).unwrap()[0]["seq"], 3);
    assert!(log.after("session/valid", 3).unwrap().is_empty());
    assert_eq!(log.after("session/valid", 4), Err(ApiError::Replay));
    assert_eq!(log.after("session/missing", 1), Err(ApiError::Replay));
}

#[tokio::test]
async fn subscriptions_have_a_per_connection_limit() {
    let f = fixture("max_sessions = 1", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    for stream in ["session/a", "session/a"] {
        assert_eq!(
            call(
                &mut ws,
                "events.subscribe",
                json!({"stream": stream, "after_seq": 0})
            )
            .await["result"]["accepted"],
            true
        );
    }
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0})
        )
        .await["error"]["code"],
        -32003
    );
    let mut other = authenticated(&f).await;
    assert_eq!(
        call(
            &mut other,
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn output_is_consumed_during_host_wait_and_overflow_notifies_slow_subscriber() {
    let (f, _) = scripted_fixture(
        "events_per_stream = 2\nhost_timeout_ms = 500",
        PeerScenario::Plain,
        ads_ptyhost::wire::ErrorCode::InternalError,
        40,
        &[],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    let subscription = command(
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    );
    transmit(&mut ws, &subscription).await;
    assert_eq!(receive(&mut ws).await["result"]["accepted"], true);
    assert_eq!(receive(&mut ws).await["seq"], 1);
    let result = call(
        &mut ws,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    assert_eq!(result["result"]["accepted"], true);
    let error = receive(&mut ws).await;
    assert_eq!(error["id"], subscription["id"]);
    assert_eq!(error["error"]["code"], -32004);
    closed(&mut ws).await;
    let mut other = authenticated(&f).await;
    assert_eq!(
        call(&mut other, "unknown", json!({})).await["error"]["code"],
        -32601
    );
}

#[tokio::test]
async fn released_streams_stay_whole_and_pruning_ends_only_its_subscription() {
    let f = fixture("max_sessions = 2", Arc::new(TestLauncher)).await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let mut subscriptions = Vec::new();
    for (index, id) in ["first", "second"].into_iter().enumerate() {
        call(&mut control, "session.open", open_params(id)).await;
        subscriptions.push(
            call(
                &mut watcher,
                "events.subscribe",
                json!({"stream": format!("session/{id}"), "after_seq": 0}),
            )
            .await["id"]
                .clone(),
        );
        assert_eq!(receive(&mut watcher).await["seq"], 1);
        f.backend.terminals()[index].exit(0);
        let ended = receive(&mut watcher).await;
        assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
        assert_eq!(ended["data"]["reason"], "exit");
    }
    host_sessions_settle(&f, 0).await;
    assert!(f.host.sessions().is_empty());
    call(
        &mut control,
        "events.subscribe",
        json!({"stream": "session/first", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut control).await["seq"], 1);
    assert_eq!(receive(&mut control).await["seq"], 2);
    call(&mut control, "session.open", open_params("third")).await;
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "third"})
        )
        .await["result"]["accepted"],
        true
    );
    // Three released records exceed max_sessions = 2: only the oldest is pruned, as before this change.
    let pruned = receive(&mut watcher).await;
    assert_eq!(pruned["id"], subscriptions[0]);
    assert_eq!(pruned["error"]["code"], -32004);
    assert_eq!(receive(&mut control).await["error"]["code"], -32004);
    // The next frame on the watcher answers its own request: the second subscription did not end.
    assert_eq!(
        call(
            &mut watcher,
            "session.close",
            json!({"session_id": "first"})
        )
        .await["error"]["code"],
        -32602
    );
    // The second stream is still retained and replays whole.
    call(
        &mut control,
        "events.subscribe",
        json!({"stream": "session/second", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut control).await["seq"], 1);
    assert_eq!(receive(&mut control).await["seq"], 2);
}

async fn assert_invalidated_session_closed(f: &Fixture, ws: &mut Socket) {
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert_eq!(
        call(ws, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn failed_host_resubscribe_invalidates_only_the_affected_stream() {
    let (f, _) = scripted_fixture(
        "max_sessions = 1",
        PeerScenario::Recover,
        ads_ptyhost::wire::ErrorCode::ReplayEvicted,
        0,
        &[(PeerCall::Resubscribe, 1)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    // A following request also acts as a barrier after the recovery refusal.
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["error"]["code"],
        -32004
    );
    assert_eq!(
        call(
            &mut ws,
            "events.subscribe",
            json!({"stream": "session/other", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert_invalidated_session_closed(&f, &mut ws).await;
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn host_event_gaps_refuse_replay_without_stopping_the_service() {
    for scenario in [
        PeerScenario::GapOutput,
        PeerScenario::GapExit,
        PeerScenario::GapClose,
    ] {
        let (f, _) = scripted_fixture(
            "max_sessions = 1",
            scenario,
            ads_ptyhost::wire::ErrorCode::InternalError,
            0,
            &[],
        )
        .await;
        let mut ws = authenticated(&f).await;
        call(&mut ws, "session.open", open_params("session")).await;
        assert_eq!(
            call(
                &mut ws,
                "session.write",
                json!({"session_id": "session", "data": "YQ=="})
            )
            .await["result"]["accepted"],
            true
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            call(
                &mut ws,
                "events.subscribe",
                json!({"stream": "session/session", "after_seq": 0})
            )
            .await["error"]["code"],
            -32004
        );
        assert_eq!(
            call(&mut ws, "unknown", json!({})).await["error"]["code"],
            -32601
        );
        assert_invalidated_session_closed(&f, &mut ws).await;
        assert!(!f.api.is_finished());
    }
}

#[tokio::test]
async fn pruned_stream_ends_only_its_subscription_and_its_id_is_not_reused() {
    // Two sessions per run also allow two subscriptions per connection.
    let f = fixture("max_sessions = 2", Arc::new(TestLauncher)).await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let pruned = command(
        "events.subscribe",
        json!({"stream": "session/first", "after_seq": 0}),
    );
    transmit(&mut watcher, &pruned).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/later", "after_seq": 0}),
        )
        .await["result"]["accepted"],
        true
    );
    // The watcher does not read while the control connection prunes and tries to reopen the id.
    for id in ["first", "second", "third"] {
        call(&mut control, "session.open", open_params(id)).await;
        assert_eq!(
            call(&mut control, "session.close", json!({"session_id": id})).await["result"]["accepted"],
            true
        );
    }
    assert_eq!(
        call(&mut control, "session.open", open_params("first")).await["error"]["code"],
        -32012
    );
    bounded(async {
        loop {
            let message = receive(&mut watcher).await;
            if message["id"] == pruned["id"] {
                assert_eq!(message["error"]["code"], -32004);
                break;
            }
            assert_eq!(message["stream"], "session/first");
        }
    })
    .await;
    let opened = call(&mut control, "session.open", open_params("later")).await;
    assert_eq!(opened["result"]["accepted"], true);
    let created = receive(&mut watcher).await;
    assert_eq!(created["stream"], "session/later");
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
}

#[test]
fn retired_stream_refuses_replay_and_keeps_its_id_taken() {
    let mut log = EventLog::new(4, 1);
    log.append(
        "a",
        "resized",
        json!({"session_id": "a", "cols": 80, "rows": 24}),
        None,
    )
    .unwrap();
    // A subscriber that had read seq 1 and did not wake up before the stream was pruned.
    log.retire("a");
    assert!(log.is_retired("session/a"));
    assert_eq!(log.after("session/a", 1), Err(ApiError::Replay));
    assert_eq!(log.after("session/a", 0), Err(ApiError::Replay));
    log.retire("b");
    // The tombstone bound evicts the oldest id.
    assert!(!log.is_retired("session/a"));
    assert!(log.is_retired("session/b"));
}

#[test]
fn subscription_keeps_its_incarnation_after_the_tombstone_expires() {
    let mut log = EventLog::new(4, 1);
    let data = json!({"session_id": "a", "cols": 80, "rows": 24});
    log.append("a", "resized", data.clone(), None).unwrap();
    let advanced = log.subscribe("session/a", 1).unwrap();
    let from_start = log.subscribe("session/a", 0).unwrap();
    // The subscribers are held elsewhere while two prunes push out the first tombstone.
    log.retire("a");
    log.retire("b");
    assert!(!log.is_retired("session/a"));
    log.append("a", "resized", data, None).unwrap();
    for stream in [&advanced, &from_start] {
        assert_eq!(stream.poll(0), Poll::End(ApiError::Replay));
        assert_eq!(stream.poll(1), Poll::End(ApiError::Replay));
    }
    // A subscription made before the open follows the incarnation the open fills, and only that one.
    let early = log.subscribe("session/c", 0).unwrap();
    log.append(
        "c",
        "resized",
        json!({"session_id": "c", "cols": 80, "rows": 24}),
        None,
    )
    .unwrap();
    assert!(matches!(early.poll(0), Poll::Events(events) if events.len() == 1));
}

#[test]
fn first_final_result_wins_for_existing_subscribers_only() {
    let mut log = EventLog::new(4, 4);
    let data = json!({"session_id": "a", "cols": 80, "rows": 24});
    log.append("a", "resized", data, None).unwrap();
    let stream = log.subscribe("session/a", 0).unwrap();
    log.invalidate("a", ads_daemon::events::Closing::Pending);
    assert_eq!(stream.poll(0), Poll::Wait);
    log.invalidate("a", ads_daemon::events::Closing::Failed);
    log.invalidate("a", ads_daemon::events::Closing::Closed);
    // The subscriber keeps the exhausted result; a new subscription sees the current state.
    assert_eq!(stream.poll(0), Poll::End(ApiError::ClosePending));
    assert_eq!(log.subscribe("session/a", 0).unwrap_err(), ApiError::Replay);
}

#[tokio::test]
async fn waiting_stream_eviction_wakes_an_idle_websocket() {
    let f = fixture("retired_sessions = 1", Arc::new(TestLauncher)).await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let first = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &first).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    bounded(watcher.send(Message::Ping(vec![1].into())))
        .await
        .unwrap();
    assert_eq!(
        bounded(watcher.next()).await.unwrap().unwrap(),
        Message::Pong(vec![1].into())
    );
    assert_eq!(
        call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0}),
        )
        .await["result"]["accepted"],
        true
    );
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], first["id"]);
    assert_eq!(ended["error"]["code"], -32004);
}

#[tokio::test]
async fn waiting_stream_survives_cleanup_and_resubscription_to_the_same_id() {
    let (f, _) = refusing_fixture(
        "retired_sessions = 2",
        PeerCall::Subscribe,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let first = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &first).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["error"]["code"],
        -32014
    );
    loop {
        let event = receive(&mut watcher).await;
        if event["id"] == first["id"] {
            assert_eq!(event["error"]["code"], -32004);
            break;
        }
        assert_eq!(event["type"], "urn:ads:event:session.created:1");
    }
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/a", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    let mut other = authenticated(&f).await;
    assert_eq!(
        call(
            &mut other,
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    let created = receive(&mut watcher).await;
    assert_eq!(created["stream"], "session/a");
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
}

#[tokio::test]
async fn early_subscriber_held_across_reuse_never_follows_the_new_session() {
    let f = fixture(
        "max_sessions = 2\nretired_sessions = 2",
        Arc::new(TestLauncher),
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    // The watcher subscribes before the open and is then held before it delivers anything.
    let early = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &early).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/later", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    f.hooks.before_delivery.hold();
    // While it is held, "a" opens, closes, is pruned and its tombstone expires before the id is opened again.
    for id in ["a", "b", "c", "d", "e"] {
        call(&mut control, "session.open", open_params(id)).await;
        call(&mut control, "session.close", json!({"session_id": id})).await;
    }
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    f.hooks.before_delivery.release();
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], early["id"]);
    assert_eq!(ended["error"]["code"], -32004);
    call(&mut control, "session.close", json!({"session_id": "a"})).await;
    call(&mut control, "session.open", open_params("later")).await;
    let created = receive(&mut watcher).await;
    assert_eq!(created["stream"], "session/later");
    assert_eq!(created["type"], "urn:ads:event:session.created:1");
}

#[tokio::test]
async fn queued_host_output_stays_with_the_session_closed_before_id_reuse() {
    let f = fixture(
        "max_sessions = 1\nretired_sessions = 1",
        Arc::new(TestLauncher),
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut watcher = authenticated(&f).await;
    let subscribe = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &subscribe).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    f.hooks.before_delivery.hold();
    f.backend.terminals()[0].emit(b"previous");
    for id in ["a", "b", "c"] {
        if id != "a" {
            assert_eq!(
                call(&mut control, "session.open", open_params(id)).await["result"]["accepted"],
                true
            );
        }
        assert_eq!(
            call(&mut control, "session.close", json!({"session_id": id})).await["result"]["accepted"],
            true
        );
    }
    assert_eq!(
        call(&mut control, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    f.hooks.before_delivery.release();
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], subscribe["id"]);
    assert_eq!(ended["error"]["code"], -32004);
    assert_eq!(
        call(
            &mut watcher,
            "events.subscribe",
            json!({"stream": "session/a", "after_seq": 0})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        receive(&mut watcher).await["type"],
        "urn:ads:event:session.created:1"
    );
    assert_eq!(
        call(
            &mut control,
            "session.write",
            json!({"session_id": "a", "data": STANDARD.encode(b"current")})
        )
        .await["result"]["accepted"],
        true
    );
    let output = receive(&mut watcher).await;
    assert_eq!(output["seq"], 2);
    assert_eq!(output["data"]["offset"], 0);
    assert_eq!(output["data"]["payload"], STANDARD.encode(b"current"));
}

#[tokio::test]
async fn repeated_subscription_on_one_connection_gets_every_final_result() {
    let f = fixture("max_sessions = 1", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let subscribe = || {
        command(
            "events.subscribe",
            json!({"stream": "session/a", "after_seq": 0}),
        )
    };
    let (first, second) = (subscribe(), subscribe());
    for request in [&first, &second] {
        transmit(&mut ws, request).await;
        let reply = receive(&mut ws).await;
        assert_eq!(reply["id"], request["id"]);
        assert_eq!(reply["result"]["accepted"], true);
    }
    // Ids per connection are bounded too.
    let third = subscribe();
    transmit(&mut ws, &third).await;
    let refused = receive(&mut ws).await;
    assert_eq!(refused["id"], third["id"]);
    assert_eq!(refused["error"]["code"], -32003);
    call(&mut control, "session.open", open_params("a")).await;
    call(&mut control, "session.close", json!({"session_id": "a"})).await;
    // Another closed session prunes "a": every subscription gets its final result.
    call(&mut control, "session.open", open_params("b")).await;
    call(&mut control, "session.close", json!({"session_id": "b"})).await;
    let mut seqs = Vec::new();
    let mut ended = Vec::new();
    while ended.len() < 2 {
        let frame = receive(&mut ws).await;
        if let Some(seq) = frame["seq"].as_u64() {
            seqs.push(seq);
        } else {
            assert_eq!(frame["error"]["code"], -32004);
            ended.push(frame["id"].clone());
        }
    }
    // Each event went out once, and each id got its own end.
    assert!(!seqs.is_empty());
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");
    assert!(ended.contains(&first["id"]) && ended.contains(&second["id"]));
}

#[tokio::test]
async fn repeated_subscription_with_a_lower_cursor_sends_events_again() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    call(&mut control, "session.open", open_params("a")).await;
    let resize = |cols: u64| json!({"session_id": "a", "cols": cols, "rows": 24});
    call(&mut control, "session.resize", resize(90)).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    assert_eq!(receive(&mut ws).await["seq"], 2);
    // The second subscription asks from the start: the shared cursor goes back and 1 and 2 go out again.
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    assert_eq!(receive(&mut ws).await["seq"], 2);
    call(&mut control, "session.resize", resize(100)).await;
    assert_eq!(receive(&mut ws).await["seq"], 3);
    // Nothing else is pending: the next frame answers this request.
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
}

#[tokio::test]
async fn ended_incarnation_answers_its_ids_before_the_new_subscription() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nretired_sessions = 1",
        PeerScenario::ExitThenHoldClose,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut other = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    let first = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &first).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    // The service waits for the host while the next commands queue behind the close.
    call(&mut control, "session.open", open_params("s")).await;
    let close = command("session.close", json!({"session_id": "s"}));
    transmit(&mut control, &close).await;
    wait_until(|| calls.lock().unwrap().contains(&PeerCall::Close)).await;
    let queued = f.hooks.queued_commands();
    // Queued first: a subscription that pushes the waiting "a" out of the quota.
    transmit(
        &mut other,
        &command(
            "events.subscribe",
            json!({"stream": "session/b", "after_seq": 0}),
        ),
    )
    .await;
    wait_until(|| f.hooks.queued_commands() == queued + 1).await;
    // Queued second: the watcher subscribes "a" again and waits for the answer, without delivering.
    let second = command(
        "events.subscribe",
        json!({"stream": "session/a", "after_seq": 0}),
    );
    transmit(&mut watcher, &second).await;
    wait_until(|| f.hooks.queued_commands() == queued + 2).await;
    f.release.notify_one();
    assert_eq!(receive(&mut control).await["id"], close["id"]);
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["id"], first["id"]);
    assert_eq!(ended["error"]["code"], -32004);
    let confirmed = receive(&mut watcher).await;
    assert_eq!(confirmed["id"], second["id"]);
    assert_eq!(confirmed["result"]["accepted"], true);
}
