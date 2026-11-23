use super::{
    PeerCall, PeerScenario, TestLauncher, authenticated, bounded, call, command, fixture,
    host_sessions_settle, open_params, receive, refusing_fixture, scripted_fixture, transmit,
    wait_until,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn failed_open_subscription_closes_host_and_allows_retry() {
    use ads_ptyhost::wire::ErrorCode;
    let (f, calls) = refusing_fixture("", PeerCall::Subscribe, ErrorCode::SessionEnded, 0).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32014
    );
    bounded(async {
        while calls.lock().unwrap().len() < 4 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert_eq!(
        &*calls.lock().unwrap(),
        &[
            PeerCall::Hello,
            PeerCall::Open,
            PeerCall::Subscribe,
            PeerCall::Close
        ]
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn unknown_session_on_close_releases_the_slot() {
    use ads_ptyhost::wire::ErrorCode::UnknownSession;
    let (f, _) = refusing_fixture("max_sessions = 1", PeerCall::Close, UnknownSession, 0).await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "session"})).await["error"]["code"],
        UnknownSession.number()
    );
    // Without the host the stream would stay open forever in the interface.
    let ended = receive(&mut ws).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["seq"], 2);
    assert_eq!(ended["data"]["reason"], "error");
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

fn close_calls(calls: &std::sync::Mutex<Vec<PeerCall>>) -> usize {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| *c == &PeerCall::Close)
        .count()
}
async fn wait_closes(calls: &std::sync::Mutex<Vec<PeerCall>>, count: usize) {
    bounded(async {
        while close_calls(calls) < count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn ended_session_returns_its_slot_before_any_client_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1",
        PeerScenario::ExitOnWrite,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("first")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "first", "data": "YQ=="}),
    )
    .await;
    // The host sees the close before answering it; the daemon reads the answer before the next open.
    wait_closes(&calls, 1).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    // A client that still closes the ended session is answered as before, without reaching the host.
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "first"})).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), 1);
    // A session the client closed itself still refuses a second close.
    assert_eq!(
        call(
            &mut ws,
            "session.close",
            json!({"session_id": "replacement"})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(
            &mut ws,
            "session.close",
            json!({"session_id": "replacement"})
        )
        .await["error"]["code"],
        -32602
    );
}

#[tokio::test]
async fn exhausted_release_of_an_ended_session_keeps_its_stream_and_client_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1
close_retries = 2
close_retry_ms = 20",
        PeerScenario::ExitOnWrite,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Close, 2)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    let ended = receive(&mut ws).await;
    assert_eq!(ended["seq"], 2);
    assert_eq!(ended["data"]["reason"], "exit");
    let closes = || {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count()
    };
    bounded(async {
        while closes() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    // Both automatic attempts were refused: the stream stays whole and the slot stays taken.
    // The next frame answers this request while the existing subscription stays open.
    assert_eq!(
        call(
            &mut ws,
            "session.resize",
            json!({"session_id": "session", "cols": 100, "rows": 30})
        )
        .await["error"]["code"],
        -32602
    );
    let mut late = authenticated(&f).await;
    assert_eq!(
        call(
            &mut late,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0}),
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(receive(&mut late).await["seq"], 1);
    assert_eq!(receive(&mut late).await, ended);
    assert_eq!(
        call(&mut late, "session.open", open_params("other")).await["error"]["code"],
        -32003
    );
    assert_eq!(closes(), 2);
    assert_eq!(
        call(&mut late, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(&mut late, "session.open", open_params("other")).await["result"]["accepted"],
        true
    );
}

async fn terminal_event_during_a_refused_client_close_still_releases(
    scenario: PeerScenario,
    reason: &str,
) {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retry_ms = 20",
        scenario,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Close, 1)],
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    call(
        &mut watcher,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    let close = command("session.close", json!({"session_id": "session"}));
    transmit(&mut control, &close).await;
    // The daemon recorded the terminal event while the client's close was still waiting on the host.
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["data"]["reason"], reason);
    f.release.notify_one();
    let refused = receive(&mut control).await;
    assert_eq!(refused["id"], close["id"]);
    assert_eq!(refused["error"]["code"], -32603);
    wait_closes(&calls, 2).await;
    assert_eq!(
        call(&mut control, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), 2);
    assert_eq!(f.hooks.automatic_closes(), 1);
}

#[tokio::test]
async fn exit_during_a_refused_client_close_still_releases_the_slot() {
    terminal_event_during_a_refused_client_close_still_releases(
        PeerScenario::ExitThenHoldClose,
        "killed",
    )
    .await;
}

#[tokio::test]
async fn session_closed_during_a_refused_client_close_still_releases_the_slot() {
    terminal_event_during_a_refused_client_close_still_releases(
        PeerScenario::ClosedBeforeCloseResponse { hold: true },
        "killed",
    )
    .await;
}

#[tokio::test]
async fn unknown_session_on_the_automatic_release_keeps_the_natural_end() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1",
        PeerScenario::ExitOnWrite,
        ads_ptyhost::wire::ErrorCode::UnknownSession,
        0,
        &[(PeerCall::Close, 1)],
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
    wait_closes(&calls, 1).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    call(
        &mut ws,
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut ws).await["seq"], 1);
    let ended = receive(&mut ws).await;
    assert_eq!(ended["seq"], 2);
    assert_eq!(ended["data"]["reason"], "exit");
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), 1);
}

#[tokio::test]
async fn refused_release_of_an_ended_session_never_reaches_a_reopened_id() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nretired_sessions = 1\nclose_retry_ms = 20",
        PeerScenario::ExitOnWrite,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Close, 1)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    // Only the first automatic attempt passes; the repeated one waits at the barrier.
    f.hooks.automatic_close.hold();
    f.hooks.automatic_close.admit_one();
    call(&mut ws, "session.open", open_params("a")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "a", "data": "YQ=="}),
    )
    .await;
    wait_closes(&calls, 1).await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["result"]["accepted"],
        true
    );
    // The second close of "a" was the client's: the daemon sent only the refused one.
    assert_eq!(close_calls(&calls), 2);
    assert_eq!(f.hooks.automatic_closes(), 1);
    // Returning the slot dropped the held attempt.
    assert_eq!(f.hooks.queued_closes(), 0);
    f.hooks.automatic_close.release();
    // Another released session prunes "a"; its tombstone keeps the id taken.
    call(&mut ws, "session.open", open_params("b")).await;
    call(&mut ws, "session.close", json!({"session_id": "b"})).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("a")).await["error"]["code"],
        -32012
    );
    // One more pushes the tombstone out.
    call(&mut ws, "session.open", open_params("c")).await;
    call(&mut ws, "session.close", json!({"session_id": "c"})).await;
    // A late close of the pruned id, with a fresh key, is refused instead of accepted without effect.
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["error"]["code"],
        -32602
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    let closes = close_calls(&calls);
    assert_eq!(f.hooks.queued_closes(), 0);
    // The new generation is still active.
    let resize = json!({"session_id": "a", "cols": 100, "rows": 30});
    assert_eq!(
        call(&mut ws, "session.resize", resize.clone()).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), closes);
    assert_eq!(f.hooks.automatic_closes(), 1);
    // As documented, a close by the reused id refers to the current session.
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["result"]["accepted"],
        true
    );
    assert_eq!(close_calls(&calls), closes + 1);
    assert_eq!(
        call(&mut ws, "session.resize", resize).await["error"]["code"],
        -32602
    );
}

#[tokio::test]
async fn more_sessions_than_the_limit_can_end_on_their_own() {
    let f = fixture("", Arc::new(TestLauncher)).await;
    let mut ws = authenticated(&f).await;
    // The default limit is 32; ending each one on its own must keep the next open working.
    let total = 40;
    for index in 0..total {
        let id = format!("natural-{index}");
        assert_eq!(
            call(&mut ws, "session.open", open_params(&id)).await["result"]["accepted"],
            true,
            "open {index}"
        );
        f.backend.terminals()[index].exit(0);
        host_sessions_settle(&f, 0).await;
    }
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        call(&mut ws, "session.open", open_params("after")).await["result"]["accepted"],
        true
    );
    // The last ended session keeps its full stream for a late subscriber.
    let mut late = authenticated(&f).await;
    call(
        &mut late,
        "events.subscribe",
        json!({"stream": format!("session/natural-{}", total - 1), "after_seq": 0}),
    )
    .await;
    assert_eq!(
        receive(&mut late).await["type"],
        "urn:ads:event:session.created:1"
    );
    let ended = receive(&mut late).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["data"]["reason"], "exit");
}

#[tokio::test]
async fn refused_automatic_close_is_retried_until_the_host_closes() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retry_ms = 20",
        PeerScenario::GapOutput,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Close, 2)],
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
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    let closes = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| *c == &PeerCall::Close)
        .count();
    assert_eq!(closes, 3);
    // The slot came back without any session.close from the client.
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn automatic_close_timeout_is_retried_without_stopping_the_service() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nhost_timeout_ms = 200\nclose_retry_ms = 20",
        PeerScenario::GapOutputDropClose,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
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
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count(),
        2
    );
    assert!(!f.api.is_finished());
    assert_eq!(
        call(&mut ws, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn invalidation_racing_a_client_close_sends_one_close_to_the_host() {
    let (f, calls) = scripted_fixture(
        "",
        PeerScenario::GapBeforeCloseResponse,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("session")).await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "session"})).await["result"]["accepted"],
        true
    );
    // The queued automatic close runs on the next turns of the service loop.
    for _ in 0..3 {
        assert_eq!(
            call(&mut ws, "unknown", json!({})).await["error"]["code"],
            -32601
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count(),
        1
    );
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn exhausted_automatic_close_reaches_the_original_subscriber() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retries = 3\nclose_retry_ms = 20",
        PeerScenario::GapOutput,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Close, 3)],
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut watcher = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    let subscription = command(
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    );
    transmit(&mut watcher, &subscription).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    call(
        &mut control,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    // While the retries run the subscription waits; the final word is the exhausted close.
    let notice = receive(&mut watcher).await;
    assert_eq!(notice["id"], subscription["id"]);
    assert_eq!(notice["error"]["code"], -32013);
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count(),
        3
    );
    assert_eq!(
        call(&mut watcher, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["error"]["code"],
        -32013
    );
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "session"})
        )
        .await["result"]["accepted"],
        true
    );
    assert!(f.backend.terminals()[0].was_killed());
    assert_eq!(
        call(&mut control, "session.open", open_params("replacement")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn failed_open_cleanup_timeout_keeps_the_record_until_the_host_closes() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nhost_timeout_ms = 200\nclose_retry_ms = 20",
        PeerScenario::HostDropClose,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Subscribe, 1)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32014
    );
    // The host opened a real terminal; the first close never gets a reply.
    assert_eq!(f.host.sessions().len(), 1);
    assert_eq!(
        call(&mut ws, "session.open", open_params("other")).await["error"]["code"],
        -32003
    );
    bounded(async {
        while !f.backend.terminals()[0].was_killed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count(),
        2
    );
    assert!(!f.api.is_finished());
    // Once the host confirmed, the failed open gives its id back.
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn stale_automatic_close_never_reaches_a_reopened_id() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nretired_sessions = 1\nclose_retry_ms = 400",
        PeerScenario::GapOutput,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Close, 1)],
    )
    .await;
    let mut ws = authenticated(&f).await;
    call(&mut ws, "session.open", open_params("a")).await;
    call(
        &mut ws,
        "session.write",
        json!({"session_id": "a", "data": "YQ=="}),
    )
    .await;
    // The first automatic close is refused; the next attempt waits 400 ms.
    bounded(async {
        while calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count()
            < 1
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert_eq!(
        call(&mut ws, "session.close", json!({"session_id": "a"})).await["result"]["accepted"],
        true
    );
    // Two more closed sessions prune "a" and push its tombstone out, so the id can be reused.
    for id in ["b", "c"] {
        call(&mut ws, "session.open", open_params(id)).await;
        call(&mut ws, "session.close", json!({"session_id": id})).await;
    }
    assert_eq!(
        call(&mut ws, "session.open", open_params("a")).await["result"]["accepted"],
        true
    );
    let closes = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| *c == &PeerCall::Close)
        .count();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        call(&mut ws, "unknown", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count(),
        closes
    );
    let reopened = f.backend.terminals().last().unwrap().clone();
    assert!(!reopened.was_killed());
    assert_eq!(f.host.sessions().len(), 1);
}

#[tokio::test]
async fn subscribe_without_host_reply_keeps_the_record_and_schedules_the_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nhost_timeout_ms = 200\nclose_retry_ms = 20",
        PeerScenario::HostHoldSubscribe,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut ws = authenticated(&f).await;
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["error"]["code"],
        -32014
    );
    assert!(!f.api.is_finished());
    wait_until(|| f.backend.terminals()[0].was_killed()).await;
    assert!(f.host.sessions().is_empty());
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count(),
        1
    );
    assert_eq!(
        call(&mut ws, "session.open", open_params("session")).await["result"]["accepted"],
        true
    );
}

#[tokio::test]
async fn delayed_subscriber_gets_the_exhausted_close_after_a_manual_close() {
    let (f, calls) = scripted_fixture(
        "max_sessions = 1\nclose_retries = 3\nclose_retry_ms = 20",
        PeerScenario::GapOutput,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[(PeerCall::Close, 3)],
    )
    .await;
    let mut control = authenticated(&f).await;
    let mut watcher = authenticated(&f).await;
    call(&mut control, "session.open", open_params("session")).await;
    f.hooks.before_delivery.hold();
    let subscription = command(
        "events.subscribe",
        json!({"stream": "session/session", "after_seq": 0}),
    );
    transmit(&mut watcher, &subscription).await;
    assert_eq!(receive(&mut watcher).await["result"]["accepted"], true);
    call(
        &mut control,
        "session.write",
        json!({"session_id": "session", "data": "YQ=="}),
    )
    .await;
    wait_until(|| {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| *c == &PeerCall::Close)
            .count()
            == 3
    })
    .await;
    // Wait for the exhausted state, then close by hand before the subscriber reads anything.
    bounded(async {
        while call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0}),
        )
        .await["error"]["code"]
            != -32013
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert_eq!(
        call(
            &mut control,
            "session.close",
            json!({"session_id": "session"})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        call(
            &mut control,
            "events.subscribe",
            json!({"stream": "session/session", "after_seq": 0})
        )
        .await["error"]["code"],
        -32004
    );
    f.hooks.before_delivery.release();
    let notice = receive(&mut watcher).await;
    assert_eq!(notice["id"], subscription["id"]);
    assert_eq!(notice["error"]["code"], -32013);
}

async fn host_event_during_an_automatic_close(scenario: PeerScenario) {
    let (f, calls) = scripted_fixture(
        "max_sessions = 2\nhost_timeout_ms = 300\nclose_retry_ms = 20",
        scenario,
        ads_ptyhost::wire::ErrorCode::InternalError,
        0,
        &[],
    )
    .await;
    let mut watcher = authenticated(&f).await;
    let mut control = authenticated(&f).await;
    call(&mut control, "session.open", open_params("a")).await;
    call(&mut control, "session.open", open_params("b")).await;
    call(
        &mut watcher,
        "events.subscribe",
        json!({"stream": "session/b", "after_seq": 0}),
    )
    .await;
    assert_eq!(receive(&mut watcher).await["seq"], 1);
    // "a" ends; the host holds the automatic close of "a" and reports the end of "b" meanwhile.
    call(
        &mut control,
        "session.write",
        json!({"session_id": "a", "data": "YQ=="}),
    )
    .await;
    let ended = receive(&mut watcher).await;
    assert_eq!(ended["type"], "urn:ads:event:session.ended:1");
    assert_eq!(ended["data"]["reason"], "exit");
    let holds_close = scenario == PeerScenario::ExitOtherThenHoldClose;
    if holds_close {
        // The event came while the close of "a" still had no reply.
        assert_eq!(close_calls(&calls), 1);
        f.release.notify_one();
    }
    // After the reply, or after the deadline and one more attempt, both slots come back.
    let expected = if holds_close { 2 } else { 3 };
    wait_closes(&calls, expected).await;
    for id in ["c", "d"] {
        assert_eq!(
            call(&mut control, "session.open", open_params(id)).await["result"]["accepted"],
            true,
            "{scenario:?}"
        );
    }
    assert_eq!(close_calls(&calls), expected);
    assert_eq!(f.hooks.automatic_closes(), expected);
    assert!(!f.api.is_finished());
}

#[tokio::test]
async fn host_event_during_an_automatic_close_is_handled() {
    host_event_during_an_automatic_close(PeerScenario::ExitOtherThenHoldClose).await;
}

#[tokio::test]
async fn host_event_during_a_timed_out_automatic_close_is_handled() {
    host_event_during_an_automatic_close(PeerScenario::ExitOtherThenDropClose).await;
}
