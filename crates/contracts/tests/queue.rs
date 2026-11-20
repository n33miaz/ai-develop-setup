use ads_contracts::{
    CommonIdV1,
    CommonQueueTaskStateV1::{self, *},
    RecordQueueTaskV1, RecordQueueTaskV1Attempts, RecordTaskQueueV1,
    RecordTaskQueueV1State::{Enabled, Stopped},
};

fn task() -> RecordQueueTaskV1 {
    serde_json::from_str(include_str!(
        "../../../contracts/golden/record.queue-task.v1/valid/01-base.json"
    ))
    .unwrap()
}

#[test]
fn queue_starts_stopped_and_only_the_owner_can_change_it() {
    let owner = CommonIdV1::new("owner-01").unwrap();
    let other = CommonIdV1::new("owner-02").unwrap();
    let mut queue =
        RecordTaskQueueV1::new_stopped(CommonIdV1::new("queue-01").unwrap(), owner.clone());

    assert_eq!(queue.state, Stopped);
    assert!(queue.permits_state_change(&owner, Enabled));
    assert!(!queue.permits_state_change(&owner, Stopped));
    assert!(!queue.permits_state_change(&other, Enabled));

    queue.state = Enabled;
    assert!(queue.permits_state_change(&owner, Stopped));
    assert!(!queue.permits_state_change(&other, Stopped));
}

#[test]
fn only_the_listed_state_pairs_are_allowed_with_attempts_remaining() {
    let mut task = task();
    task.attempts = RecordQueueTaskV1Attempts::new(1).unwrap();

    let transitions: [(CommonQueueTaskStateV1, &[_]); 6] = [
        (Queued, &[(Running, 2), (Cancelled, 1)]),
        (Running, &[(Review, 1), (Failed, 1), (Cancelled, 1)]),
        (Review, &[(Done, 1), (Failed, 1), (Cancelled, 1)]),
        (Done, &[]),
        (Failed, &[(Queued, 1)]),
        (Cancelled, &[]),
    ];
    for (from, allowed) in transitions {
        task.state = from;
        for (to, _) in transitions {
            let expected = allowed
                .iter()
                .find(|(state, _)| *state == to)
                .map(|(_, count)| *count);
            assert_eq!(
                task.next_attempt_count(to)
                    .map(RecordQueueTaskV1Attempts::get),
                expected,
                "{from:?} -> {to:?}"
            );
        }
    }
}

#[test]
fn only_starting_consumes_an_attempt_and_exhaustion_blocks_retry() {
    let mut task = task();
    let first = task.next_attempt_count(Running).unwrap();
    assert_eq!(first.get(), 1);
    task.state = Failed;
    assert_eq!(task.next_attempt_count(Queued), None);

    task.attempts = first;
    assert_eq!(task.next_attempt_count(Queued), Some(first));
    task.state = Queued;
    task.attempts = task.next_attempt_count(Running).unwrap();
    assert_eq!(task.attempts.get(), 2);
    task.state = Running;
    assert_eq!(task.next_attempt_count(Failed), Some(task.attempts));
    task.state = Failed;
    assert_eq!(task.next_attempt_count(Queued), None);
    task.state = Queued;
    assert_eq!(task.next_attempt_count(Running), None);

    task.attempts = RecordQueueTaskV1Attempts::new(3).unwrap();
    assert_eq!(task.next_attempt_count(Cancelled), None);
}
