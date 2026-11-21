//! The concrete effects a rehearsal records are what an execution approved with them has to
//! find: an entry added to a tree, or an item added to a manifest, stops it before the disk.
//! Always inside the folder of the test (see `common`).

mod common;

use std::fs;

use ads_adapter_fs::{
    EffectKind, Effects, Executor, ITEMS_DIR, MANIFEST_NAME, Reason, Stage, Status,
};
use ads_core::gate::OperationRequest;

use common::{
    MemoryJournal, Request, Sandbox, batch_of, delete_of, id, limits, restore_of, run, through_gate,
};

const QUARANTINE_OFF: &str = "quarantine_enabled = false";

fn purge(sandbox: &Sandbox, action_id: &'static str) -> Request<'static> {
    Request {
        action_id,
        operation: OperationRequest::DeleteRecursive,
        work_root: sandbox.work_root(),
        targets: vec![sandbox.text("work/cache")],
        estimate: None,
    }
}

fn execute(
    executor: &mut Executor,
    flow: common::Flow,
    approved: &Effects,
) -> Result<ads_adapter_fs::Completed, Box<ads_adapter_fs::Failure>> {
    executor.execute_with_effects(
        flow.permit,
        &flow.started,
        &mut MemoryJournal::new(),
        approved,
    )
}

#[test]
fn an_entry_added_to_an_approved_tree_stops_the_purge_before_the_disk() {
    let sandbox = Sandbox::new("effects-purge");
    let policy = sandbox.policy(QUARANTINE_OFF);
    let seen = sandbox.file("work/cache/seen.txt", "seen");
    let mut executor = sandbox.executor(QUARANTINE_OFF);
    let flow = through_gate(&policy, purge(&sandbox, "act-1"));
    let approved = executor
        .rehearse_effects(&id("act-1"), flow.permit.plan())
        .unwrap();
    assert_eq!(approved.steps.len(), 1);
    assert_eq!(approved.steps[0].effect, EffectKind::DeleteRecursive);
    assert_eq!(approved.entries(), 2, "the folder and the file in it");

    let unseen = sandbox.file("work/cache/unseen.txt", "unseen");
    let failure = execute(&mut executor, flow, &approved).unwrap_err();
    assert_eq!(failure.reason, Reason::EffectsChanged, "{failure}");
    assert_eq!(failure.stage, Stage::Checks);
    assert!(failure.report.untouched());
    assert!(seen.is_file() && unseen.is_file());
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn an_item_added_to_an_approved_manifest_is_not_restored() {
    let sandbox = Sandbox::new("effects-restore");
    let policy = sandbox.policy("");
    sandbox.file("work/notes.txt", "notes");
    let flow = through_gate(&policy, delete_of(&sandbox, "act-1", &["work/notes.txt"]));
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    run(&mut sandbox.executor(""), flow).unwrap();

    let mut executor = sandbox.executor("");
    let flow = through_gate(&policy, restore_of(&sandbox, "act-2", &batch_text));
    let approved = executor
        .rehearse_effects(&id("act-2"), flow.permit.plan())
        .unwrap();
    assert_eq!(approved.steps.len(), 1);
    assert!(approved.manifest.is_some());

    // A valid item lands in the batch, and in its manifest, after the approval.
    let added = sandbox.file(&format!("{batch}/{ITEMS_DIR}/added.txt"), "added");
    let manifest_path = sandbox.path(&format!("{batch}/{MANIFEST_NAME}"));
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let mut item = manifest["items"][0].clone();
    item["original"] = sandbox.text("work/added.txt").into();
    item["path"] = serde_json::json!(["added.txt"]);
    manifest["items"].as_array_mut().unwrap().push(item);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let failure = execute(&mut executor, flow, &approved).unwrap_err();
    assert_eq!(failure.reason, Reason::EffectsChanged, "{failure}");
    assert_eq!(failure.stage, Stage::Checks);
    assert!(failure.report.untouched());
    assert!(added.is_file(), "the added item stays in quarantine");
    assert!(!sandbox.path("work/added.txt").exists());
    assert!(!sandbox.path("work/notes.txt").exists());
    assert!(
        sandbox
            .path(&format!("{batch}/{ITEMS_DIR}/notes.txt"))
            .is_file()
    );
}

#[test]
fn the_rehearsed_effects_are_the_steps_the_execution_reports() {
    let sandbox = Sandbox::new("effects-same");
    let policy = sandbox.policy(QUARANTINE_OFF);
    sandbox.file("work/cache/a.txt", "a");
    sandbox.file("work/cache/sub/b.txt", "bb");
    let mut executor = sandbox.executor(QUARANTINE_OFF);
    let flow = through_gate(&policy, purge(&sandbox, "act-1"));
    let approved = executor
        .rehearse_effects(&id("act-1"), flow.permit.plan())
        .unwrap();
    assert_eq!(approved.plan_hash, flow.permit.plan_hash().to_string());
    let completed = execute(&mut executor, flow, &approved).unwrap();
    let steps = &completed.report.steps;
    assert_eq!(steps.len(), approved.steps.len());
    for (step, effect) in steps.iter().zip(&approved.steps) {
        assert_eq!(step.status, Status::Done);
        assert_eq!(step.target, effect.target);
        assert_eq!(step.facts.items, effect.entries.len() as u64);
        let bytes: u64 = effect.entries.iter().map(|entry| entry.bytes).sum();
        assert_eq!(step.facts.bytes, bytes);
    }
    assert!(!sandbox.path("work/cache").exists());
}

#[test]
fn a_record_over_the_ceiling_is_refused() {
    let sandbox = Sandbox::new("effects-ceiling");
    let policy = sandbox.policy(QUARANTINE_OFF);
    sandbox.file("work/cache/a.txt", "a");
    let executor = sandbox
        .shipped_executor(QUARANTINE_OFF)
        .with_limits(limits("max_effects_bytes = 64"));
    let flow = through_gate(&policy, purge(&sandbox, "act-1"));
    let refused = executor
        .rehearse_effects(&id("act-1"), flow.permit.plan())
        .unwrap_err();
    assert_eq!(refused.reason, Reason::EffectsTooLarge, "{refused}");
    assert!(sandbox.path("work/cache/a.txt").is_file());
}
