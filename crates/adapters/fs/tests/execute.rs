//! The executor moving and deleting for real, always inside the folder of the test (see
//! `common`).

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ads_adapter_fs::{
    Completed, Effect, Executor, Failure, ITEMS_DIR, Journal, JournalError, MANIFEST_NAME,
    Manifest, Mutation, Probe, Reason, Status,
};
use ads_core::gate::{Command, Event, OperationRequest};

use common::{
    Flow, MemoryJournal, Request, SEPARATOR, STARTED_AT, Sandbox, estimate, meta, through_gate,
    tree,
};

const QUARANTINE_OFF: &str = "quarantine_enabled = false";

fn request(
    sandbox: &Sandbox,
    action_id: &'static str,
    operation: OperationRequest,
    targets: &[&str],
) -> Request<'static> {
    Request {
        action_id,
        operation,
        work_root: sandbox.work_root(),
        targets: targets.iter().map(|rel| sandbox.text(rel)).collect(),
        estimate: estimate(50, 4096),
    }
}

fn delete(sandbox: &Sandbox, action_id: &'static str, targets: &[&str]) -> Request<'static> {
    request(
        sandbox,
        action_id,
        OperationRequest::DeleteRecursive,
        targets,
    )
}

fn execute(executor: &mut Executor, flow: Flow) -> (Result<Completed, Box<Failure>>, Flow2) {
    let mut journal = MemoryJournal::new();
    let outcome = executor.execute(flow.permit, &flow.started, &mut journal);
    (
        outcome,
        Flow2 {
            action: flow.action,
            journal,
        },
    )
}

/// What is left of a flow after the permit went into the executor.
struct Flow2 {
    action: ads_core::gate::Action,
    journal: MemoryJournal,
}

/// The work root without the quarantine.
fn work_tree(sandbox: &Sandbox) -> BTreeMap<String, String> {
    let mut entries = tree(&sandbox.path("work"));
    entries.retain(|rel, _| !rel.starts_with(".ads"));
    entries
}

/// The folder of the batch of a quarantine flow, on disk and as a plan writes it.
fn batch_of(sandbox: &Sandbox, flow: &Flow, action_id: &str) -> (PathBuf, String) {
    let destination = flow.permit.plan().destination().unwrap().to_string();
    assert!(destination.starts_with(&sandbox.work_root()));
    let text = format!("{destination}{SEPARATOR}{action_id}");
    (PathBuf::from(&text), text)
}

fn restore(sandbox: &Sandbox, action_id: &'static str, batch: &str) -> Request<'static> {
    Request {
        action_id,
        operation: OperationRequest::Move {
            destination: sandbox.work_root(),
        },
        work_root: sandbox.work_root(),
        targets: vec![batch.to_owned()],
        estimate: estimate(50, 4096),
    }
}

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

fn error_code_of(event: &Event) -> String {
    match event {
        Event::ActionFailed(envelope) => envelope.data().error_code.to_string(),
        other => panic!("not action.failed: {}", other.kind()),
    }
}

// ---- quarantine and restore --------------------------------------------------------------------

#[test]
fn a_delete_goes_to_quarantine_and_comes_back_whole() {
    let sandbox = Sandbox::new("roundtrip");
    sandbox.file("work/a/build/out.txt", "from a");
    sandbox.file("work/a/build/deep/x.txt", "deep");
    sandbox.dir("work/a/build/empty");
    sandbox.file("work/b/build/out.txt", "from b");
    sandbox.file("work/keep.txt", "kept");
    let before = work_tree(&sandbox);
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");

    let flow = through_gate(
        &policy,
        delete(&sandbox, "act-1", &["work/a/build", "work/b/build"]),
    );
    assert!(flow.verdict.is_quarantined());
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    let plan_hash = flow.permit.plan_hash().to_string();

    // The rehearsal says the same and changes nothing.
    let rehearsed = executor.rehearse(&flow.permit).unwrap();
    assert_eq!(work_tree(&sandbox), before);
    assert!(!sandbox.path("work/.ads").exists());

    let (outcome, rest) = execute(&mut executor, flow);
    let done = outcome.unwrap();
    assert_eq!(done.report.steps.len(), 2);
    assert!(done.report.not_done().next().is_none());
    for (step, rehearsed) in done.report.steps.iter().zip(&rehearsed.steps) {
        assert_eq!(step.target, rehearsed.target);
        assert_eq!(step.effect, rehearsed.effect);
    }

    // Gone from the work root, whole in quarantine, and the two `build` did not collide.
    assert!(!sandbox.path("work/a/build").exists());
    assert!(!sandbox.path("work/b/build").exists());
    assert_eq!(work_tree(&sandbox).get("keep.txt").unwrap(), "file:kept");
    let stored = tree(&batch.join(ITEMS_DIR));
    assert_eq!(stored.get("a/build/out.txt").unwrap(), "file:from a");
    assert_eq!(stored.get("a/build/deep/x.txt").unwrap(), "file:deep");
    assert_eq!(stored.get("a/build/empty").unwrap(), "dir");
    assert_eq!(stored.get("b/build/out.txt").unwrap(), "file:from b");

    let manifest: Manifest =
        serde_json::from_slice(&fs::read(batch.join(MANIFEST_NAME)).unwrap()).unwrap();
    assert_eq!(manifest.action_id, "act-1");
    assert_eq!(manifest.plan_hash, plan_hash);
    assert_eq!(manifest.started_at, STARTED_AT);
    assert_eq!(manifest.work_root, sandbox.work_root());
    let originals: Vec<&str> = manifest
        .items
        .iter()
        .map(|item| item.original.as_str())
        .collect();
    assert_eq!(
        originals,
        [sandbox.text("work/a/build"), sandbox.text("work/b/build")]
    );

    // The spent permit only closes the action; the journal saw the one start.
    assert_eq!(rest.journal.events().len(), 1);
    let (action, event, _) = done
        .close(rest.action, meta("2026-11-15T12:00:09Z", 3))
        .unwrap();
    assert!(action.state().is_terminal());
    assert!(matches!(event, Event::ActionExecuted(_)));

    // Back from the manifest: same structure, same content.
    let flow = through_gate(&policy, restore(&sandbox, "act-2", &batch_text));
    let rehearsed = executor.rehearse(&flow.permit).unwrap();
    assert!(!sandbox.path("work/a/build").exists());
    assert_eq!(rehearsed.steps.len(), 2);
    let (outcome, _) = execute(&mut executor, flow);
    let done = outcome.unwrap();
    assert!(
        done.report.steps.iter().all(
            |step| step.status == Status::Done && matches!(step.effect, Effect::Restore { .. })
        )
    );
    assert_eq!(work_tree(&sandbox), before);

    // Nothing is left to restore.
    let flow = through_gate(&policy, restore(&sandbox, "act-3", &batch_text));
    let (outcome, _) = execute(&mut executor, flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::NothingToRestore);
    assert_eq!(work_tree(&sandbox), before);
}

#[test]
fn the_same_request_again_gets_its_own_folder_and_a_replay_is_refused() {
    let sandbox = Sandbox::new("repeat");
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");

    sandbox.file("work/build/out.txt", "first");
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let (first, _) = batch_of(&sandbox, &flow, "act-1");
    execute(&mut executor, flow).0.unwrap();

    // The same request, word for word: same plan hash, same destination, another action.
    sandbox.file("work/build/out.txt", "second");
    let flow = through_gate(&policy, delete(&sandbox, "act-2", &["work/build"]));
    let (second, _) = batch_of(&sandbox, &flow, "act-2");
    assert_eq!(first.parent(), second.parent());
    execute(&mut executor, flow).0.unwrap();
    let stored = |batch: &Path| fs::read_to_string(batch.join("items/build/out.txt")).unwrap();
    assert_eq!(stored(&first), "first");
    assert_eq!(stored(&second), "second");

    // The same action again, on an executor and a journal that never saw it (as after a
    // restart that lost the journal): its folder exists, so nothing is touched.
    sandbox.file("work/build/out.txt", "third");
    let before = tree(&sandbox.root);
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let (outcome, _) = execute(&mut sandbox.executor(""), flow);
    let failure = outcome.unwrap_err();
    assert_eq!(failure.reason, Reason::DestinationTaken);
    assert!(failure.report.steps.is_empty());
    assert_eq!(tree(&sandbox.root), before);
}

#[test]
fn a_restore_refuses_a_place_that_is_taken() {
    let sandbox = Sandbox::new("taken");
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");
    sandbox.file("work/a/notes.txt", "old");
    sandbox.file("work/b/notes.txt", "other");
    let flow = through_gate(
        &policy,
        delete(&sandbox, "act-1", &["work/a/notes.txt", "work/b/notes.txt"]),
    );
    let (_, batch_text) = batch_of(&sandbox, &flow, "act-1");
    execute(&mut executor, flow).0.unwrap();

    sandbox.file("work/b/notes.txt", "new");
    let before = tree(&sandbox.root);
    let flow = through_gate(&policy, restore(&sandbox, "act-2", &batch_text));
    let (outcome, _) = execute(&mut executor, flow);
    let failure = outcome.unwrap_err();
    assert_eq!(failure.reason, Reason::RestorePlaceTaken);
    // Refused before the first move: not even the free place was filled.
    assert_eq!(tree(&sandbox.root), before);
    assert_eq!(
        fs::read_to_string(sandbox.path("work/b/notes.txt")).unwrap(),
        "new"
    );
}

#[test]
fn a_restore_makes_the_folders_that_went_missing() {
    let sandbox = Sandbox::new("parents");
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");
    sandbox.file("work/a/b/notes.txt", "text");
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/a/b/notes.txt"]));
    let (_, batch_text) = batch_of(&sandbox, &flow, "act-1");
    execute(&mut executor, flow).0.unwrap();
    // The folders it was in go to quarantine too, in another action.
    let flow = through_gate(&policy, delete(&sandbox, "act-2", &["work/a"]));
    execute(&mut executor, flow).0.unwrap();
    assert!(!sandbox.path("work/a").exists());

    let flow = through_gate(&policy, restore(&sandbox, "act-3", &batch_text));
    execute(&mut executor, flow).0.unwrap();
    assert_eq!(
        fs::read_to_string(sandbox.path("work/a/b/notes.txt")).unwrap(),
        "text"
    );
}

#[test]
fn a_manifest_changed_on_disk_is_not_obeyed() {
    let sandbox = Sandbox::new("tamper");
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    execute(&mut executor, flow).0.unwrap();
    let manifest_path = batch.join(MANIFEST_NAME);
    let good: Manifest = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();

    let tampered: [fn(&mut Manifest, &Sandbox); 4] = [
        // Out of the fence by names.
        |m, _| m.items[0].path = vec!["..".to_owned(), "..".to_owned(), "outside".to_owned()],
        // Out of the fence by text.
        |m, s| m.items[0].original = s.outside_text("build"),
        // Another place inside the work root than the one it is stored under.
        |m, s| m.items[0].original = s.text("work/elsewhere"),
        |m, _| m.action_id = "act-9".to_owned(),
    ];
    for (at, change) in tampered.iter().enumerate() {
        let mut manifest = good.clone();
        change(&mut manifest, &sandbox);
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let before = tree(&sandbox.root);
        let action_id = ["act-2", "act-3", "act-4", "act-5"][at];
        let flow = through_gate(&policy, restore(&sandbox, action_id, &batch_text));
        let (outcome, _) = execute(&mut executor, flow);
        assert_eq!(outcome.unwrap_err().reason, Reason::ManifestInvalid, "{at}");
        assert_eq!(tree(&sandbox.root), before, "{at}");
    }
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn a_quarantine_folder_that_is_a_link_is_refused() {
    let sandbox = Sandbox::new("qlink");
    sandbox.file("work/build/out.txt", "text");
    if !folder_link(&sandbox.path("work/.ads"), &sandbox.outside) {
        return;
    }
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-1", &["work/build"]),
    );
    let before = tree(&sandbox.root);
    let (outcome, _) = execute(&mut sandbox.executor(""), flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::LinkInPath);
    assert_eq!(tree(&sandbox.root), before);
    assert_eq!(tree(&sandbox.outside).len(), 1);
}

#[test]
fn an_action_id_that_cannot_name_a_folder_is_refused() {
    let sandbox = Sandbox::new("badid");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act:1", &["work/build"]),
    );
    let before = tree(&sandbox.root);
    let (outcome, _) = execute(&mut sandbox.executor(""), flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::ActionIdUnusable);
    assert_eq!(tree(&sandbox.root), before);
}

// ---- links --------------------------------------------------------------------------------------

#[test]
fn a_target_that_is_a_link_goes_away_alone() {
    let sandbox = Sandbox::new("linkonly");
    if !folder_link(&sandbox.path("work/link"), &sandbox.outside) {
        return;
    }
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/link"]),
    );
    let (outcome, _) = execute(&mut sandbox.executor(QUARANTINE_OFF), flow);
    let done = outcome.unwrap();
    assert_eq!(done.report.steps[0].status, Status::Done);
    assert!(fs::symlink_metadata(sandbox.path("work/link")).is_err());
    assert_eq!(tree(&sandbox.outside), outside_before);
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn a_link_inside_a_deleted_tree_is_removed_and_never_entered() {
    let sandbox = Sandbox::new("linkintree");
    sandbox.file("work/tree/a.txt", "a");
    sandbox.file("work/tree/sub/b.txt", "b");
    if !folder_link(&sandbox.path("work/tree/sub/link"), &sandbox.outside) {
        return;
    }
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/tree"]),
    );
    let (outcome, _) = execute(&mut sandbox.executor(QUARANTINE_OFF), flow);
    outcome.unwrap();
    assert!(!sandbox.path("work/tree").exists());
    assert_eq!(tree(&sandbox.outside), outside_before);
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn a_link_sent_to_quarantine_is_moved_as_a_link() {
    let sandbox = Sandbox::new("linkmove");
    if !folder_link(&sandbox.path("work/link"), &sandbox.outside) {
        return;
    }
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-1", &["work/link"]),
    );
    let (batch, _) = batch_of(&sandbox, &flow, "act-1");
    let (outcome, _) = execute(&mut sandbox.executor(""), flow);
    outcome.unwrap();
    assert!(fs::symlink_metadata(sandbox.path("work/link")).is_err());
    let moved = fs::symlink_metadata(batch.join("items/link")).unwrap();
    assert!(common::is_link(&moved));
    assert_eq!(tree(&sandbox.outside), outside_before);
}

/// Puts a link where a folder of the path was, between the first check and the mutation.
struct SwapForLink {
    folder: PathBuf,
    aside: PathBuf,
    points_to: PathBuf,
    done: bool,
}

impl Probe for SwapForLink {
    fn before(&mut self, mutation: &Mutation) -> io::Result<()> {
        let first = matches!(
            mutation,
            Mutation::Rename { .. } | Mutation::Remove(_) | Mutation::RemoveTree(_)
        );
        if !self.done && first {
            self.done = true;
            fs::rename(&self.folder, &self.aside)?;
            assert!(folder_link(&self.folder, &self.points_to));
        }
        Ok(())
    }
}

fn swap_test(name: &str, overrides: &'static str) {
    let sandbox = Sandbox::new(name);
    sandbox.file("work/a/data/file.txt", "inside");
    // The same names exist outside: following the link would reach them.
    fs::create_dir(sandbox.outside.join("data")).unwrap();
    fs::write(sandbox.outside.join("data").join("file.txt"), "outside").unwrap();
    // Is a folder link possible here at all?
    if !folder_link(&sandbox.path("work/probe"), &sandbox.outside) {
        return;
    }
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(overrides),
        delete(&sandbox, "act-1", &["work/a/data"]),
    );
    let mut executor = sandbox
        .executor(overrides)
        .with_probe(Box::new(SwapForLink {
            folder: sandbox.path("work/a"),
            aside: sandbox.path("work/a-real"),
            points_to: sandbox.outside.clone(),
            done: false,
        }));
    let (outcome, _) = execute(&mut executor, flow);
    let failure = outcome.unwrap_err();
    assert_eq!(failure.reason, Reason::LinkInPath);
    assert!(matches!(
        failure.report.steps[0].status,
        Status::Failed {
            reason: Reason::LinkInPath,
            ..
        }
    ));
    assert_eq!(tree(&sandbox.outside), outside_before);
    assert_eq!(
        fs::read_to_string(sandbox.path("work/a-real/data/file.txt")).unwrap(),
        "inside"
    );
}

#[test]
fn a_link_swapped_in_before_a_move_is_caught_by_the_last_check() {
    swap_test("swapmove", "");
}

#[test]
fn a_link_swapped_in_before_a_delete_is_caught_by_the_last_check() {
    swap_test("swapdelete", QUARANTINE_OFF);
}

// ---- facts --------------------------------------------------------------------------------------

#[test]
fn more_on_disk_than_declared_refuses_before_any_mutation() {
    let sandbox = Sandbox::new("facts");
    const LIMIT: &str = "approval_item_limit = 5";
    for n in 0..8 {
        sandbox.file(&format!("work/big/f{n}.txt"), "x");
    }
    // Declared: 3 items. The gate allows it without approval; on disk there are 9.
    let mut small = delete(&sandbox, "act-1", &["work/big"]);
    small.estimate = estimate(3, 10);
    let flow = through_gate(&sandbox.policy(LIMIT), small);
    assert!(flow.permit.approval_id().is_none());
    let before = tree(&sandbox.root);

    let mut executor = sandbox.executor(LIMIT);
    let rehearsal = executor.rehearse(&flow.permit).unwrap_err();
    assert_eq!(rehearsal.reason, Reason::FactsContradictPlan);

    let (outcome, rest) = execute(&mut executor, flow);
    let failure = outcome.unwrap_err();
    assert_eq!(failure.reason, Reason::FactsContradictPlan);
    assert!(failure.report.steps.is_empty());
    assert_eq!(tree(&sandbox.root), before);

    // The action ends as `action.failed`, with the code of this refusal.
    let (action, event) = rest
        .action
        .apply(
            Command::Fail {
                error_code: failure.reason.error_code(),
            },
            meta("2026-11-15T12:00:09Z", 3),
        )
        .unwrap();
    assert_eq!(error_code_of(&event), "E_FS_FACTS_CONTRADICT_PLAN");
    assert!(action.state().is_terminal());
}

#[test]
fn an_approval_does_not_cover_more_than_it_was_given_for() {
    let sandbox = Sandbox::new("approved");
    const LIMIT: &str = "approval_item_limit = 2";
    for n in 0..8 {
        sandbox.file(&format!("work/big/f{n}.txt"), "x");
    }
    let mut asked = delete(&sandbox, "act-1", &["work/big"]);
    asked.estimate = estimate(4, 100);
    let flow = through_gate(&sandbox.policy(LIMIT), asked);
    assert!(flow.permit.approval_id().is_some());
    let before = tree(&sandbox.root);
    let (outcome, _) = execute(&mut sandbox.executor(LIMIT), flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::FactsContradictPlan);
    assert_eq!(tree(&sandbox.root), before);
}

#[test]
fn a_repository_root_the_plan_did_not_declare_is_refused() {
    let sandbox = Sandbox::new("repo");
    sandbox.file("work/repo/.git/HEAD", "ref: refs/heads/main");
    sandbox.file("work/repo/src/main.rs", "fn main() {}");
    // A worktree has `.git` as a file.
    sandbox.file("work/tree/.git", "gitdir: elsewhere");
    for (at, target) in ["work/repo", "work/tree"].into_iter().enumerate() {
        for overrides in ["", QUARANTINE_OFF] {
            let action_id = ["act-a", "act-b"][at];
            let flow = through_gate(
                &sandbox.policy(overrides),
                delete(&sandbox, action_id, &[target]),
            );
            let before = tree(&sandbox.root);
            let (outcome, _) = execute(&mut sandbox.executor(overrides), flow);
            let failure = outcome.unwrap_err();
            assert_eq!(failure.reason, Reason::FactsContradictPlan, "{target}");
            assert!(failure.detail.contains("repository_root"));
            assert_eq!(tree(&sandbox.root), before);
        }
    }
}

// ---- real delete --------------------------------------------------------------------------------

#[test]
fn a_real_delete_is_refused_while_the_policy_has_the_quarantine_on() {
    let sandbox = Sandbox::new("bypass");
    sandbox.file("work/build/out.txt", "text");
    // The permit comes from a gate with the quarantine off; the executor trusts its own policy.
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/build"]),
    );
    assert!(!flow.verdict.is_quarantined());
    let before = tree(&sandbox.root);
    let (outcome, _) = execute(&mut sandbox.executor(""), flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::QuarantineBypassed);
    assert_eq!(tree(&sandbox.root), before);
}

#[test]
fn the_quarantine_is_emptied_only_with_the_purge_rule_and_an_approval() {
    let sandbox = Sandbox::new("purge");
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");
    sandbox.file("work/build/out.txt", "text");
    sandbox.file("work/keep.txt", "kept");
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    execute(&mut executor, flow).0.unwrap();
    assert!(batch.join("items/build/out.txt").exists());

    // A permit without the approval the purge needs (a gate that knows no quarantine).
    let unapproved = Request {
        targets: vec![batch_text.clone()],
        ..delete(&sandbox, "act-2", &[])
    };
    let flow = through_gate(&sandbox.policy(QUARANTINE_OFF), unapproved);
    assert!(flow.permit.approval_id().is_none());
    let before = tree(&sandbox.root);
    let (outcome, _) = execute(&mut executor, flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::ApprovalMissing);
    assert_eq!(tree(&sandbox.root), before);

    // Through the real gate: the purge rule fires and the approval is granted.
    let approved = Request {
        targets: vec![batch_text],
        ..delete(&sandbox, "act-3", &[])
    };
    let flow = through_gate(&policy, approved);
    assert!(flow.verdict.has(ads_core::gate::Rule::QuarantinePurge));
    assert!(flow.permit.approval_id().is_some());
    let rehearsed = executor.rehearse(&flow.permit).unwrap();
    assert_eq!(rehearsed.steps[0].effect, Effect::DeleteRecursive);
    assert_eq!(tree(&sandbox.root), before);
    let (outcome, _) = execute(&mut executor, flow);
    let done = outcome.unwrap();
    assert_eq!(done.report.steps[0].status, Status::Done);
    assert!(!batch.exists());
    assert!(batch.parent().unwrap().exists());
    assert_eq!(
        fs::read_to_string(sandbox.path("work/keep.txt")).unwrap(),
        "kept"
    );
}

#[test]
fn with_the_quarantine_off_a_tree_is_deleted_entry_by_entry() {
    let sandbox = Sandbox::new("realdelete");
    sandbox.file("work/tree/a.txt", "a");
    sandbox.file("work/tree/sub/deep/b.txt", "b");
    sandbox.dir("work/tree/empty");
    sandbox.file("work/keep/c.txt", "c");
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/tree"]),
    );
    let (outcome, _) = execute(&mut sandbox.executor(QUARANTINE_OFF), flow);
    outcome.unwrap();
    let left = work_tree(&sandbox);
    assert_eq!(
        left.keys().collect::<Vec<_>>(),
        ["keep", "keep/c.txt"],
        "only the target went away"
    );
    // The folder the tree was held in while it went is gone too.
    assert!(tree(&sandbox.path(sandbox.private_dir())).is_empty());
}

#[test]
fn a_plain_delete_takes_a_file_or_an_empty_folder_only() {
    let sandbox = Sandbox::new("plaindelete");
    sandbox.file("work/full/a.txt", "a");
    sandbox.file("work/single.txt", "s");
    sandbox.dir("work/empty");
    let policy = sandbox.policy(QUARANTINE_OFF);
    let mut executor = sandbox.executor(QUARANTINE_OFF);
    let plain = |action_id, targets: &[&str]| {
        request(&sandbox, action_id, OperationRequest::Delete, targets)
    };

    let before = tree(&sandbox.root);
    let flow = through_gate(&policy, plain("act-1", &["work/single.txt", "work/full"]));
    let (outcome, _) = execute(&mut executor, flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::NotEmpty);
    assert_eq!(tree(&sandbox.root), before);

    let flow = through_gate(&policy, plain("act-2", &["work/single.txt", "work/empty"]));
    execute(&mut executor, flow).0.unwrap();
    assert!(!sandbox.path("work/single.txt").exists());
    assert!(!sandbox.path("work/empty").exists());
    assert!(sandbox.path("work/full/a.txt").exists());
}

// ---- failure in the middle ----------------------------------------------------------------------

/// Fails the `nth` mutation of one kind, as the operating system would.
struct FailAt {
    renames: bool,
    nth: u32,
    seen: u32,
}

impl Probe for FailAt {
    fn before(&mut self, mutation: &Mutation) -> io::Result<()> {
        let counted = match mutation {
            Mutation::Rename { .. } => self.renames,
            Mutation::Remove(_) => !self.renames,
            Mutation::CreateDir(_)
            | Mutation::WriteFile(_)
            | Mutation::RemoveTree(_)
            | Mutation::Sync(_) => false,
        };
        if counted {
            self.seen += 1;
            if self.seen == self.nth {
                return Err(io::Error::other("injected failure"));
            }
        }
        Ok(())
    }
}

#[test]
fn a_batch_that_fails_in_the_middle_stops_and_loses_nothing() {
    let sandbox = Sandbox::new("midfail");
    for name in ["a", "b", "c"] {
        sandbox.file(&format!("work/{name}/data.txt"), name);
    }
    let before = work_tree(&sandbox);
    let policy = sandbox.policy("");
    let flow = through_gate(
        &policy,
        delete(&sandbox, "act-1", &["work/a", "work/b", "work/c"]),
    );
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    let mut executor = sandbox.executor("").with_probe(Box::new(FailAt {
        renames: true,
        nth: 2,
        seen: 0,
    }));
    let (outcome, rest) = execute(&mut executor, flow);
    let failure = outcome.unwrap_err();

    // Exactly what was and was not done.
    assert_eq!(failure.reason, Reason::Io);
    let statuses: Vec<&Status> = failure
        .report
        .steps
        .iter()
        .map(|step| &step.status)
        .collect();
    assert_eq!(statuses[0], &Status::Done);
    assert!(matches!(
        statuses[1],
        Status::Failed {
            reason: Reason::Io,
            ..
        }
    ));
    assert_eq!(statuses[2], &Status::Pending);
    assert_eq!(failure.report.done().count(), 1);
    assert_eq!(failure.report.not_done().count(), 2);

    // On disk: `a` in quarantine with the manifest, `b` and `c` where they were.
    assert_eq!(
        fs::read_to_string(batch.join("items/a/data.txt")).unwrap(),
        "a"
    );
    assert!(batch.join(MANIFEST_NAME).exists());
    assert!(!sandbox.path("work/a").exists());
    for name in ["b", "c"] {
        let kept = fs::read_to_string(sandbox.path(&format!("work/{name}/data.txt"))).unwrap();
        assert_eq!(kept, name);
        assert!(!batch.join("items").join(name).exists());
    }

    let (_, event) = rest
        .action
        .apply(
            Command::Fail {
                error_code: failure.reason.error_code(),
            },
            meta("2026-11-15T12:00:09Z", 3),
        )
        .unwrap();
    assert_eq!(error_code_of(&event), "E_FS_IO");

    // The manifest of the half-done batch restores what did go.
    let flow = through_gate(&policy, restore(&sandbox, "act-2", &batch_text));
    let (outcome, _) = execute(&mut sandbox.executor(""), flow);
    let done = outcome.unwrap();
    let statuses: Vec<&Status> = done.report.steps.iter().map(|step| &step.status).collect();
    assert_eq!(statuses, [&Status::Done, &Status::Absent, &Status::Absent]);
    assert_eq!(work_tree(&sandbox), before);
}

#[test]
fn a_recursive_delete_that_fails_in_the_middle_says_how_far_it_went() {
    let sandbox = Sandbox::new("middelete");
    for name in ["a", "b", "c"] {
        sandbox.file(&format!("work/tree/{name}.txt"), name);
    }
    sandbox.file("work/other/d.txt", "d");
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/other", "work/tree"]),
    );
    let mut executor = sandbox
        .executor(QUARANTINE_OFF)
        .with_probe(Box::new(FailAt {
            renames: false,
            nth: 4,
            seen: 0,
        }));
    let (outcome, _) = execute(&mut executor, flow);
    let failure = outcome.unwrap_err();
    // `other` (two entries) went; `tree` was moved to its holding folder, where one file
    // went and the second removal failed. What is left of it is there, and the report says
    // where.
    assert_eq!(failure.report.steps[0].status, Status::Done);
    assert!(matches!(
        failure.report.steps[1].status,
        Status::Held {
            removed: 1,
            confirmed: 1,
            ..
        }
    ));
    assert!(!failure.report.untouched());
    assert!(work_tree(&sandbox).is_empty());
    let held = PathBuf::from(failure.report.steps[1].held.as_ref().unwrap());
    assert!(held.starts_with(sandbox.path(sandbox.private_dir())));
    assert_eq!(tree(&held).keys().collect::<Vec<_>>(), ["b.txt", "c.txt"]);
}

// ---- order with the log -------------------------------------------------------------------------

struct Refusing;

impl Journal for Refusing {
    fn consume(&mut self, _started: &Event) -> Result<(), JournalError> {
        Err(JournalError("the log is not writable".to_owned()))
    }
}

/// Confirms, and remembers what the disk looked like at that instant.
struct Watching {
    root: PathBuf,
    seen: Option<BTreeMap<String, String>>,
}

impl Journal for Watching {
    fn consume(&mut self, _started: &Event) -> Result<(), JournalError> {
        self.seen = Some(tree(&self.root));
        Ok(())
    }
}

#[test]
fn nothing_happens_before_the_journal_confirms_the_start() {
    let sandbox = Sandbox::new("journal");
    sandbox.file("work/build/out.txt", "text");
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");
    let before = tree(&sandbox.root);

    // The journal refuses: nothing runs.
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let failure = executor
        .execute(flow.permit, &flow.started, &mut Refusing)
        .unwrap_err();
    assert_eq!(failure.reason, Reason::JournalRefused);
    assert_eq!(tree(&sandbox.root), before);

    // The start of another action: the journal is not even asked.
    let other = through_gate(&policy, delete(&sandbox, "act-2", &["work/build"]));
    let flow = through_gate(&policy, delete(&sandbox, "act-3", &["work/build"]));
    let mut journal = MemoryJournal::new();
    let failure = executor
        .execute(flow.permit, &other.started, &mut journal)
        .unwrap_err();
    assert_eq!(failure.reason, Reason::StartedMismatch);
    assert!(journal.events().is_empty());
    assert_eq!(tree(&sandbox.root), before);

    // When it confirms, the disk is still untouched at that instant, and changed afterwards.
    let flow = through_gate(&policy, delete(&sandbox, "act-4", &["work/build"]));
    let mut journal = Watching {
        root: sandbox.root.clone(),
        seen: None,
    };
    executor
        .execute(flow.permit, &flow.started, &mut journal)
        .unwrap();
    assert_eq!(journal.seen.unwrap(), before);
    assert_ne!(tree(&sandbox.root), before);
}

#[test]
fn the_memory_journal_takes_one_start_per_action() {
    let sandbox = Sandbox::new("memjournal");
    sandbox.file("work/build/out.txt", "text");
    let policy = sandbox.policy("");
    let first = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let again = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let mut journal = MemoryJournal::new();
    sandbox
        .executor("")
        .execute(first.permit, &first.started, &mut journal)
        .unwrap();
    sandbox.file("work/build/out.txt", "again");
    let before = tree(&sandbox.root);
    // Another executor, as after a restart: the journal is what remembers.
    let failure = sandbox
        .executor("")
        .execute(again.permit, &again.started, &mut journal)
        .unwrap_err();
    assert_eq!(failure.reason, Reason::JournalRefused);
    assert_eq!(journal.events().len(), 1);
    assert_eq!(tree(&sandbox.root), before);
}

// ---- what this adapter does not execute ---------------------------------------------------------

#[test]
fn what_is_not_defined_yet_is_refused_whole() {
    let sandbox = Sandbox::new("unsupported");
    sandbox.file("work/a/b/c.txt", "c");
    sandbox.file("work/sub/x.o", "x");
    sandbox.dir("work/elsewhere");
    let policy = sandbox.policy("");
    let mut executor = sandbox.executor("");
    let before = tree(&sandbox.root);
    let cases = [
        (
            delete(&sandbox, "act-1", &["work/sub/*.o"]),
            Reason::Unsupported,
        ),
        (
            delete(&sandbox, "act-2", &["work/a", "work/a/b"]),
            Reason::OverlappingTargets,
        ),
        (
            request(
                &sandbox,
                "act-3",
                OperationRequest::Overwrite,
                &["work/a/b/c.txt"],
            ),
            Reason::Unsupported,
        ),
        (
            request(
                &sandbox,
                "act-4",
                OperationRequest::CleanUntracked,
                &["work/a"],
            ),
            Reason::Unsupported,
        ),
        (
            request(
                &sandbox,
                "act-5",
                OperationRequest::Move {
                    destination: sandbox.text("work/elsewhere"),
                },
                &["work/a"],
            ),
            Reason::Unsupported,
        ),
    ];
    for (asked, reason) in cases {
        let flow = through_gate(&policy, asked);
        let (outcome, _) = execute(&mut executor, flow);
        assert_eq!(outcome.unwrap_err().reason, reason);
        assert_eq!(tree(&sandbox.root), before);
    }
}

// ---- where a move really lands -----------------------------------------------------------------

#[test]
fn a_destination_that_is_a_link_is_refused() {
    let sandbox = Sandbox::new("destlink");
    sandbox.file("work/build/out.txt", "text");
    sandbox.dir("work/.ads/quarantine");
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-1", &["work/build"]),
    );
    // The folder the gate chose, by text inside the work root, leads out of the fence.
    let (batch, _) = batch_of(&sandbox, &flow, "act-1");
    if !folder_link(batch.parent().unwrap(), &sandbox.outside) {
        return;
    }
    let before = tree(&sandbox.root);
    let mut executor = sandbox.executor("");
    assert_eq!(
        executor.rehearse(&flow.permit).unwrap_err().reason,
        Reason::LinkInPath
    );
    let (outcome, _) = execute(&mut executor, flow);
    assert_eq!(outcome.unwrap_err().reason, Reason::LinkInPath);
    assert_eq!(tree(&sandbox.root), before);
    assert_eq!(tree(&sandbox.outside).len(), 1);
}

#[test]
fn a_link_swapped_into_the_destination_is_caught_before_the_move() {
    let sandbox = Sandbox::new("swapdest");
    sandbox.file("work/a/data/file.txt", "inside");
    if !folder_link(&sandbox.path("work/probe"), &sandbox.outside) {
        return;
    }
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-1", &["work/a/data"]),
    );
    // By the time of the move the quarantine exists; it is then replaced by a link.
    let mut executor = sandbox.executor("").with_probe(Box::new(SwapForLink {
        folder: sandbox.path("work/.ads"),
        aside: sandbox.path("work/.ads-real"),
        points_to: sandbox.outside.clone(),
        done: false,
    }));
    let (outcome, _) = execute(&mut executor, flow);
    let failure = outcome.unwrap_err();
    assert_eq!(failure.reason, Reason::LinkInPath);
    assert!(matches!(
        failure.report.steps[0].status,
        Status::Failed { .. }
    ));
    // The target never left, and nothing was written through the link.
    assert_eq!(
        fs::read_to_string(sandbox.path("work/a/data/file.txt")).unwrap(),
        "inside"
    );
    assert_eq!(tree(&sandbox.outside), outside_before);
}

/// A policy can name a protected folder inside the work root only in the POSIX lists.
#[cfg(not(windows))]
#[test]
fn a_quarantine_inside_a_protected_folder_is_refused() {
    use ads_core::gate::{GatePolicy, HostFacts, PolicyError};

    let sandbox = Sandbox::new("protecteddest");
    sandbox.file("work/build/out.txt", "text");
    let overrides = format!(
        "quarantine_dir = \"trash\"\nposix_system_dirs = [\"{}\"]",
        sandbox.text("work/trash")
    );
    let before = tree(&sandbox.root);
    // The policy itself refuses this configuration when it is loaded: with no policy there is
    // no gate and no executor, so it never gets as far as a plan.
    let home = sandbox.text("fake-home");
    let loaded = GatePolicy::from_toml_str(&overrides, &HostFacts { user_home: &home });
    assert_eq!(
        loaded,
        Err(PolicyError::Invalid {
            key: "quarantine_dir",
            value: "trash".to_owned()
        })
    );
    assert_eq!(tree(&sandbox.root), before);
}
