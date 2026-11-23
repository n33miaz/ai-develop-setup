use super::*;

#[test]
fn an_unreadable_request_does_not_keep_the_list_open() {
    let mut bench = Bench::new();
    let gate = bench.gate(OVER_LIMIT);
    let (broken, _, _) = bench.waiting(&gate, "act-broken");
    // More requests than a history holds: written by no gate of this core.
    {
        let mut log = bench.log.as_mut().unwrap().log().unwrap();
        let requested = log.load(&broken).unwrap().events[1].to_json();
        for extra in 0..4_usize {
            let approval = format!("appr-extra-{extra}");
            let mut json = requested.clone();
            json["id"] = json!(new_id().unwrap());
            json["stream"] = json!(format!("approval/{approval}"));
            json["data"]["approval_id"] = json!(approval);
            let event = Event::from_json(json).unwrap();
            GateJournal::append(&mut *log, &broken, 2 + extra, &event).unwrap();
        }
    }
    let (fine, approval, hash) = bench.waiting(&gate, "act-fine");
    let desk = bench.desk(vec![gate]);
    let page = desk.pending(&session()).unwrap();
    assert_eq!(only(&page.approvals)["action_id"], fine.as_str());
    assert!(!page.more);
    // Left out, but counted, so the owner learns it is there, and which one it is.
    assert_eq!(page.unreadable, 1);
    assert!(
        crate::diagnostic::seen()
            .contains(&"pending approval of act-broken not readable; not listed".to_owned())
    );
    let deny = params(&fine, &approval, &hash, "deny");
    assert_eq!(
        desk.answer(&session(), &deny, &new_id().unwrap()),
        Ok(false)
    );
    let page = desk.pending(&session()).unwrap();
    assert!(page.approvals.is_empty());
    assert!(!page.more);
}

/// An action waiting for approval whose history holds more requests than a history does:
/// four more, written by no gate of this core, asked `hours` ago with a validity of one
/// hour; `last_expires_at` replaces the deadline of the last one. Returns the action and its
/// five approvals.
fn unreadable_since(
    bench: &mut Bench,
    gate: &SessionGate,
    name: &str,
    hours: i64,
    last_expires_at: Option<&str>,
) -> (Id, Vec<String>) {
    let (action, approval, _) = bench.waiting_since(gate, name, hours);
    let mut approvals = vec![approval.as_str().to_owned()];
    let mut log = bench.log.as_mut().unwrap().log().unwrap();
    let requested = log.load(&action).unwrap().events[1].to_json();
    for extra in 0..4_usize {
        let approval = format!("appr-{name}-extra-{extra}");
        let mut json = requested.clone();
        json["id"] = json!(new_id().unwrap());
        json["stream"] = json!(format!("approval/{approval}"));
        json["data"]["approval_id"] = json!(approval);
        if let Some(expires_at) = last_expires_at.filter(|_| extra == 3) {
            json["data"]["expires_at"] = json!(expires_at);
        }
        let event = Event::from_json(json).unwrap();
        GateJournal::append(&mut *log, &action, 2 + extra, &event).unwrap();
        approvals.push(approval);
    }
    (action, approvals)
}

fn close_params(action: &Id, decision: &str) -> CommandApprovalCloseV1Params {
    serde_json::from_value(json!({
        "action_id": action, "decision": decision,
        "_meta": {"idempotency_key": new_id().unwrap(), "origin": "local"}
    }))
    .unwrap()
}

/// The actions the storage still reports as pending with a history that does not read.
fn unreadable_pending(desk: &ApprovalDesk) -> Vec<String> {
    desk.with_gates_ok(|log, _| {
        let mut seen = Vec::new();
        log.log()
            .unwrap()
            .visit_pending_approvals(100, &mut |entry, _, _| {
                if let StoredPending::Unreadable(id) = entry {
                    seen.push(id);
                }
                Visit::Skip
            })
            .unwrap();
        seen
    })
}

/// Every event of the log of these types, as JSON with its position.
fn of_type(desk: &ApprovalDesk, streams: &[String], kind: &str) -> Vec<(i64, Value)> {
    streams
        .iter()
        .flat_map(|name| stream(desk, name))
        .filter(|(_, event)| event["type"].as_str().is_some_and(|t| t.starts_with(kind)))
        .collect()
}

#[test]
fn closing_an_unreadable_request_records_it_first_and_never_licenses() {
    for decision in ["deny", "expire"] {
        let mut bench = Bench::new();
        let gate = bench.gate(OVER_LIMIT);
        // An expiry needs a validity that is over; a denial does not.
        let hours = if decision == "expire" { 2 } else { 0 };
        let (broken, approvals) = unreadable_since(&mut bench, &gate, "act-broken", hours, None);
        let (fine, _, _) = bench.waiting(&gate, "act-fine");
        let desk = bench.desk(vec![gate]);
        let session = session();
        assert_eq!(unreadable_pending(&desk), ["act-broken"]);

        let close = close_params(&broken, decision);
        let cause = new_id().unwrap();
        assert_eq!(desk.close(&session, &close, &cause), Ok(false));
        assert!(unreadable_pending(&desk).is_empty(), "{decision}");
        assert_eq!(
            only(&desk.pending(&session).unwrap().approvals)["action_id"],
            fine.as_str()
        );

        let records = stream(&desk, "closure/act-broken");
        let [(recorded_at, record)] = &records[..] else {
            panic!("one closing record: {records:?}");
        };
        assert_eq!(record["cause"], cause.as_str());
        assert_eq!(
            record["actor"],
            json!({"kind": "device", "id": "device-01"})
        );
        assert_eq!(record["data"]["decision"], decision);
        assert_eq!(record["data"]["approval_ids"], json!(approvals));
        let streams: Vec<String> = approvals
            .iter()
            .map(|approval| format!("approval/{approval}"))
            .collect();
        let kind = match decision {
            "deny" => "urn:ads:event:approval.denied:",
            _ => "urn:ads:event:approval.expired:",
        };
        let decided = of_type(&desk, &streams, kind);
        assert_eq!(decided.len(), approvals.len());
        for (position, event) in &decided {
            assert!(position > recorded_at, "recorded before any decision");
            assert_eq!(event["cause"], record["id"]);
        }
        // No grant and no license: the action never starts.
        assert!(of_type(&desk, &streams, "urn:ads:event:approval.granted:").is_empty());
        assert!(execute(&desk, &broken).is_err());
        let action_stream = vec!["action/act-broken".to_owned()];
        assert!(of_type(&desk, &action_stream, "urn:ads:event:action.started:").is_empty());

        // The same key again records nothing more; a new key finds nothing open.
        assert_eq!(desk.close(&session, &again_close(&close), &cause), Ok(true));
        assert_eq!(stream(&desk, "closure/act-broken").len(), 1);
        assert_eq!(
            desk.close(
                &session,
                &close_params(&broken, decision),
                &new_id().unwrap()
            ),
            Err(ApiError::ApprovalAlreadyAnswered)
        );
        assert_eq!(stream(&desk, "closure/act-broken").len(), 1);
    }
}

#[test]
fn an_unreadable_request_is_never_expired_before_its_deadline() {
    let mut bench = Bench::new();
    let gate = bench.gate(OVER_LIMIT);
    // Asked two hours ago with a validity of one hour, but one request is still valid.
    let (broken, approvals) = unreadable_since(
        &mut bench,
        &gate,
        "act-broken",
        2,
        Some("2999-01-01T00:00:00Z"),
    );
    let desk = bench.desk(vec![gate]);
    let session = session();
    let close = close_params(&broken, "expire");
    assert_eq!(
        desk.close(&session, &close, &new_id().unwrap()),
        Err(ApiError::Invalid)
    );
    // Refused before anything is written: no closing record and no decision.
    assert!(stream(&desk, "closure/act-broken").is_empty());
    let streams: Vec<String> = approvals
        .iter()
        .map(|approval| format!("approval/{approval}"))
        .collect();
    assert!(of_type(&desk, &streams, "urn:ads:event:approval.expired:").is_empty());
    assert_eq!(unreadable_pending(&desk), ["act-broken"]);
    // Denying stays allowed before the deadline.
    let deny = close_params(&broken, "deny");
    assert_eq!(desk.close(&session, &deny, &new_id().unwrap()), Ok(false));
    assert!(unreadable_pending(&desk).is_empty());
}

#[test]
fn a_recorded_expiry_resumes_after_the_clock_moves_back() {
    let mut bench = Bench::new();
    let gate = bench.gate(OVER_LIMIT);
    let deadline = "2099-01-01T00:00:00Z";
    let (action, approvals) = unreadable_since(&mut bench, &gate, "act-resume", 2, Some(deadline));
    let desk = bench.desk(vec![gate]);
    let close = close_params(&action, "expire");
    let streams: Vec<_> = approvals
        .iter()
        .map(|approval| format!("approval/{approval}"))
        .collect();
    let expired = "urn:ads:event:approval.expired:2";

    bench.vault.fail_after(2);
    assert_eq!(
        desk.close_unreadable(&close, &new_id().unwrap(), || Ok(deadline.to_owned())),
        Err(ApiError::Storage)
    );
    bench.vault.accept_all();
    let records = stream(&desk, "closure/act-resume");
    assert_eq!(records.len(), 1);
    let record = &records[0].1;
    assert_eq!(record["ts"], deadline);
    assert_eq!(of_type(&desk, &streams, expired).len(), 1);

    drop(desk);
    bench.log = Some(bench.reopen());
    let gate = bench.gate(OVER_LIMIT);
    let desk = bench.desk(vec![gate]);
    let rewound = || Ok("2098-12-31T23:59:59Z".to_owned());
    let new_close = close_params(&action, "expire");
    assert_eq!(
        desk.close_unreadable(&new_close, &new_id().unwrap(), rewound),
        Err(ApiError::Invalid)
    );
    assert_eq!(stream(&desk, "closure/act-resume"), records);
    assert_eq!(of_type(&desk, &streams, expired).len(), 1);
    assert_eq!(
        desk.close_unreadable(&close, &new_id().unwrap(), rewound),
        Ok(true)
    );
    assert_eq!(stream(&desk, "closure/act-resume"), records);
    for approval_stream in streams {
        let decisions = of_type(&desk, &[approval_stream], expired);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].1["cause"], record["id"]);
        assert_eq!(decisions[0].1["ts"], deadline);
    }
}

#[test]
fn a_legacy_expiry_resumes_once_the_remaining_requests_are_due() {
    let mut bench = Bench::new();
    let gate = bench.gate(OVER_LIMIT);
    let recorded_at = "2098-12-31T23:59:58Z";
    let deadline = "2099-01-01T00:00:00Z";
    let resumed_at = "2099-01-01T00:00:01Z";
    let (action, approvals) =
        unreadable_since(&mut bench, &gate, "act-legacy-expiry", 2, Some(deadline));
    let desk = bench.desk(vec![gate]);
    let close = close_params(&action, "expire");
    let closure_stream = "closure/act-legacy-expiry";
    let streams: Vec<_> = approvals
        .iter()
        .map(|approval| format!("approval/{approval}"))
        .collect();
    let expired = "urn:ads:event:approval.expired:2";

    let record = {
        let mut desk = desk.lock().unwrap();
        let Closable::Requests(open) = desk
            .log
            .log()
            .unwrap()
            .closable_requests(&action, MAX_CLOSED)
            .unwrap()
        else {
            panic!("expected unreadable requests");
        };
        let ids: Vec<_> = approvals.iter().map(String::as_str).collect();
        let record = desk
            .record_closing(&close, &ids, recorded_at, &new_id().unwrap())
            .unwrap();
        let first = open
            .iter()
            .find(|request| request.approval_id == approvals[0]);
        desk.close_request(&close, first.unwrap(), &record, recorded_at)
            .unwrap();
        record
    };
    let records = stream(&desk, closure_stream);
    assert_eq!(records.len(), 1);
    let decisions = of_type(&desk, &streams, expired);
    assert_eq!(decisions.len(), 1);

    drop(desk);
    bench.log = Some(bench.reopen());
    let gate = bench.gate(OVER_LIMIT);
    let desk = bench.desk(vec![gate]);
    let retry = |at: &str| desk.close_unreadable(&close, &new_id().unwrap(), || Ok(at.to_owned()));
    assert_eq!(retry("2098-12-31T23:59:59Z"), Err(ApiError::Invalid));
    assert_eq!(stream(&desk, closure_stream), records);
    assert_eq!(of_type(&desk, &streams, expired), decisions);

    assert_eq!(retry(resumed_at), Ok(true));
    assert_eq!(retry("2098-12-31T23:59:59Z"), Ok(true));
    assert_eq!(stream(&desk, closure_stream), records);
    for (index, approval_stream) in streams.into_iter().enumerate() {
        let decisions = of_type(&desk, &[approval_stream], expired);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].1["cause"], record);
        let expected_ts = if index == 0 { recorded_at } else { resumed_at };
        assert_eq!(decisions[0].1["ts"], expected_ts);
    }
}

fn again_close(close: &CommandApprovalCloseV1Params) -> CommandApprovalCloseV1Params {
    close.clone()
}

#[test]
fn a_readable_request_is_closed_only_by_an_answer() {
    let mut bench = Bench::new();
    let gate = bench.gate(OVER_LIMIT);
    let (fine, approval, hash) = bench.waiting(&gate, "act-fine");
    let desk = bench.desk(vec![gate]);
    let session = session();
    for decision in ["deny", "expire"] {
        assert_eq!(
            desk.close(&session, &close_params(&fine, decision), &new_id().unwrap()),
            Err(ApiError::ApprovalReadable)
        );
    }
    assert_eq!(
        desk.close(
            &session,
            &close_params(&Id::new("act-unknown").unwrap(), "deny"),
            &new_id().unwrap()
        ),
        Err(ApiError::ApprovalNotFound)
    );
    assert!(stream(&desk, "closure/act-fine").is_empty());
    assert!(matches!(
        state(&desk, &fine),
        State::AwaitingApproval { .. }
    ));
    // The normal path still answers it.
    let deny = params(&fine, &approval, &hash, "deny");
    assert_eq!(desk.answer(&session, &deny, &new_id().unwrap()), Ok(false));
    assert_eq!(state(&desk, &fine), State::ApprovalDenied);
}

#[test]
fn a_disabled_control_refuses_a_closing() {
    let close = close_params(&Id::new("act-01").unwrap(), "deny");
    assert_eq!(
        DisabledApprovals.close(&session(), &close, "cause"),
        Err(ApiError::ApprovalNotConfigured)
    );
}
