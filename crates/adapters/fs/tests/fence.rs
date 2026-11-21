//! The fence holds. Nothing here executes: these tests go through the checks only (`rehearse`
//! and `measure`), and were green before the executor could move or delete anything.

mod common;

use std::path::Path;

use ads_adapter_fs::Executor;
use ads_adapter_fs::{Reason, Status};
use ads_core::gate::{AbsPath, Decision, OperationRequest, PlanRequest, TargetRequest, classify};

use common::{Request, Sandbox, estimate, through_gate, tree};

fn delete_of(sandbox: &Sandbox, action_id: &'static str, targets: &[&str]) -> Request<'static> {
    Request {
        action_id,
        operation: OperationRequest::DeleteRecursive,
        work_root: sandbox.work_root(),
        targets: targets.iter().map(|rel| sandbox.text(rel)).collect(),
        estimate: estimate(50, 4096),
    }
}

/// A folder link to `target`: a junction on Windows, a symbolic link elsewhere.
fn folder_link(link: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        common::junction(link, target);
        true
    }
    #[cfg(not(windows))]
    {
        common::symlink(link, target)
    }
}

#[test]
fn the_sandbox_is_under_the_temporary_folder_of_the_target_directory() {
    let sandbox = Sandbox::new("sandbox");
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    assert_eq!(sandbox.root.parent().unwrap(), tmp.join("ads-fs"));
    assert!(sandbox.fence.starts_with(tmp));
    assert!(sandbox.outside.starts_with(tmp));
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn a_target_outside_the_fence_is_refused() {
    let sandbox = Sandbox::new("outside");
    let policy = sandbox
        .policy("")
        .with_work_roots(vec![AbsPath::parse(&sandbox.outside_text(".")).unwrap()]);
    // The gate allows it: the work root declared is the sibling folder, and the text agrees.
    let flow = through_gate(
        &policy,
        Request {
            action_id: "act-outside",
            operation: OperationRequest::DeleteRecursive,
            work_root: sandbox.outside_text("."),
            targets: vec![sandbox.outside_text(common::SENTINEL)],
            estimate: estimate(1, 12),
        },
    );
    let before = tree(&sandbox.root);
    let failure = Executor::new(&sandbox.fence, policy)
        .unwrap()
        .rehearse(&flow.permit)
        .unwrap_err();
    assert_eq!(failure.reason, Reason::OutsideFence);
    assert_eq!(tree(&sandbox.root), before);
}

#[test]
fn the_fence_and_the_work_root_are_never_targets() {
    let sandbox = Sandbox::new("itself");
    let executor = sandbox.executor("");
    let path = |text: String| AbsPath::parse(&text).unwrap();
    let fence = path(sandbox.fence_text.clone());
    let work = path(sandbox.work_root());

    let stop = executor.measure(&fence, &fence).unwrap_err();
    assert_eq!(stop.reason, Reason::OutsideWorkRoot);
    let stop = executor.measure(&work, &work).unwrap_err();
    assert_eq!(stop.reason, Reason::OutsideWorkRoot);
    let stop = executor.measure(&work, &fence).unwrap_err();
    assert_eq!(stop.reason, Reason::OutsideWorkRoot);

    // And no permit for such a plan exists: the gate denies the work root as a target.
    let plan = ads_core::gate::ActionPlan::normalize(&PlanRequest {
        operation: OperationRequest::DeleteRecursive,
        targets: vec![TargetRequest {
            path: sandbox.work_root(),
            repo_root: false,
        }],
        work_root: sandbox.work_root(),
        estimate: estimate(1, 1),
    })
    .unwrap();
    assert_eq!(
        classify(&plan, &sandbox.policy("")).unwrap().decision(),
        Decision::Denied
    );
}

#[test]
fn a_parent_component_never_reaches_the_executor() {
    let sandbox = Sandbox::new("dotdot");
    // The executor takes only paths the core normalized, and the core refuses `..`.
    for rel in ["work/../../outside", "work/a/..", "../outside"] {
        assert!(AbsPath::parse(&sandbox.text(rel)).is_err(), "{rel}");
    }
}

#[test]
fn a_missing_target_is_refused() {
    let sandbox = Sandbox::new("missing");
    let flow = through_gate(
        &sandbox.policy(""),
        delete_of(&sandbox, "act-1", &["work/gone"]),
    );
    let failure = sandbox.executor("").rehearse(&flow.permit).unwrap_err();
    assert_eq!(failure.reason, Reason::Missing);
}

#[test]
fn a_symbolic_link_on_the_way_to_the_target_is_refused() {
    let sandbox = Sandbox::new("symlink");
    if !common::symlink(&sandbox.path("work/link"), &sandbox.outside) {
        return;
    }
    let target = format!("work/link/{}", common::SENTINEL);
    let flow = through_gate(
        &sandbox.policy(""),
        delete_of(&sandbox, "act-1", &[&target]),
    );
    let before = tree(&sandbox.root);
    let failure = sandbox.executor("").rehearse(&flow.permit).unwrap_err();
    assert_eq!(failure.reason, Reason::LinkInPath);
    assert_eq!(tree(&sandbox.root), before);
    assert!(sandbox.sentinel_is_intact());
}

#[cfg(windows)]
#[test]
fn a_junction_on_the_way_to_the_target_is_refused() {
    let sandbox = Sandbox::new("junction");
    common::junction(&sandbox.path("work/link"), &sandbox.outside);
    // Through the junction the sentinel is readable: the text alone would let it through.
    assert!(sandbox.path("work/link").join(common::SENTINEL).exists());
    let target = format!("work/link/{}", common::SENTINEL);
    let flow = through_gate(
        &sandbox.policy(""),
        delete_of(&sandbox, "act-1", &[&target]),
    );
    let before = tree(&sandbox.root);
    let executor = sandbox.executor("");
    let failure = executor.rehearse(&flow.permit).unwrap_err();
    assert_eq!(failure.reason, Reason::LinkInPath);
    let stop = executor
        .measure(
            &AbsPath::parse(&sandbox.work_root()).unwrap(),
            &AbsPath::parse(&sandbox.text(&target)).unwrap(),
        )
        .unwrap_err();
    assert_eq!(stop.reason, Reason::LinkInPath);
    assert_eq!(tree(&sandbox.root), before);
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn a_work_root_that_is_a_link_is_refused() {
    let sandbox = Sandbox::new("linkroot");
    if !folder_link(&sandbox.path("linked"), &sandbox.outside) {
        return;
    }
    let policy = sandbox
        .policy("")
        .with_work_roots(vec![AbsPath::parse(&sandbox.text("linked")).unwrap()]);
    let flow = through_gate(
        &policy,
        Request {
            action_id: "act-1",
            operation: OperationRequest::DeleteRecursive,
            work_root: sandbox.text("linked"),
            targets: vec![sandbox.text(&format!("linked/{}", common::SENTINEL))],
            estimate: estimate(1, 12),
        },
    );
    let failure = Executor::new(&sandbox.fence, policy)
        .unwrap()
        .rehearse(&flow.permit)
        .unwrap_err();
    assert_eq!(failure.reason, Reason::LinkInPath);
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn a_link_as_the_target_is_seen_as_a_link_and_never_entered() {
    let sandbox = Sandbox::new("linktarget");
    if !folder_link(&sandbox.path("work/link"), &sandbox.outside) {
        return;
    }
    let flow = through_gate(
        &sandbox.policy(""),
        delete_of(&sandbox, "act-1", &["work/link"]),
    );
    let report = sandbox.executor("").rehearse(&flow.permit).unwrap();
    assert_eq!(report.steps.len(), 1);
    // One entry, no bytes: what the link points to was not counted.
    assert_eq!(report.steps[0].facts.items, 1);
    assert_eq!(report.steps[0].facts.bytes, 0);
    assert!(!report.steps[0].facts.repo_root);
}

#[test]
fn a_rehearsal_says_what_would_be_done_and_changes_nothing() {
    let sandbox = Sandbox::new("rehearse");
    sandbox.file("work/a/build/out.txt", "12345");
    sandbox.file("work/b/build/out.txt", "123");
    let flow = through_gate(
        &sandbox.policy(""),
        delete_of(&sandbox, "act-1", &["work/a/build", "work/b/build"]),
    );
    let before = tree(&sandbox.root);
    let report = sandbox.executor("").rehearse(&flow.permit).unwrap();
    assert_eq!(tree(&sandbox.root), before);
    assert_eq!(report.steps.len(), 2);
    assert!(
        report
            .steps
            .iter()
            .all(|step| step.status == Status::Pending)
    );
    assert_eq!(report.steps[0].target, sandbox.text("work/a/build"));
    assert_eq!(report.steps[0].facts.items, 2);
    assert_eq!(report.steps[0].facts.bytes, 5);
    assert_eq!(report.steps[1].facts.bytes, 3);
}
