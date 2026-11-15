//! Shared by the gate tests. Every path is synthetic text: nothing here touches the file system.
#![allow(dead_code)]

use std::num::NonZeroU64;

use ads_core::gate::{
    ActionPlan, Actor, ActorKind, DataClass, Estimate, EventMeta, GatePolicy, HostFacts, Id,
    OperationRequest, PlanRequest, TargetRequest, Timestamp, Ulid, Verdict, classify,
};

pub const WIN_HOME: &str = "C:\\Users\\user-a";
pub const WIN_ROOT: &str = "C:\\Users\\user-a\\work\\proj";
pub const POSIX_HOME: &str = "/home/user-a";
pub const POSIX_ROOT: &str = "/home/user-a/work/proj";

pub const SMALL: Option<Estimate> = Some(Estimate {
    items: 3,
    bytes: 4096,
});

pub fn policy(home: &str, overrides: &str) -> GatePolicy {
    GatePolicy::from_toml_str(overrides, &HostFacts { user_home: home }).unwrap()
}

pub fn request(operation: OperationRequest, work_root: &str, targets: &[&str]) -> PlanRequest {
    PlanRequest {
        operation,
        targets: targets
            .iter()
            .map(|path| TargetRequest {
                path: (*path).to_owned(),
                repo_root: false,
            })
            .collect(),
        work_root: work_root.to_owned(),
        estimate: SMALL,
    }
}

pub fn plan(operation: OperationRequest, work_root: &str, targets: &[&str]) -> ActionPlan {
    ActionPlan::normalize(&request(operation, work_root, targets)).unwrap()
}

/// The verdict for a Windows plan under the shipped defaults.
pub fn win(operation: OperationRequest, targets: &[&str]) -> Verdict {
    classify(&plan(operation, WIN_ROOT, targets), &policy(WIN_HOME, ""))
}

/// The verdict for a POSIX plan under the shipped defaults.
pub fn posix(operation: OperationRequest, targets: &[&str]) -> Verdict {
    classify(
        &plan(operation, POSIX_ROOT, targets),
        &policy(POSIX_HOME, ""),
    )
}

pub fn id(text: &str) -> Id {
    Id::new(text).unwrap()
}

pub fn at(text: &str) -> Timestamp {
    Timestamp::new(text).unwrap()
}

pub fn meta(ts: &str, seq: u64) -> EventMeta {
    EventMeta {
        id: Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").unwrap(),
        ts: at(ts),
        seq: NonZeroU64::new(seq).unwrap(),
        actor: Actor {
            kind: ActorKind::Daemon,
            id: id("core-01"),
        },
        cause: None,
        data_class: DataClass::Internal,
    }
}
