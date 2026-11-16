//! What an independent review found, each with the test that keeps it closed: swaps after the
//! last check, a destination that appears at the last instant, entries that were not approved,
//! repositories deeper in a target, a stop at every step, a permit used twice. Always inside
//! the folder of the test (see `common`).

mod common;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ads_adapter_fs::{
    Completed, Executor, Failure, ITEMS_DIR, Laid, Limits, MANIFEST_NAME, Manifest, Mutation,
    Probe, Reason, Scaffold, Stage, Status,
};
use ads_core::gate::OperationRequest;

use common::{
    Flow, MemoryJournal, Request, SEPARATOR, Sandbox, estimate, meta, through_gate, tree,
};

const QUARANTINE_OFF: &str = "quarantine_enabled = false";

fn delete(sandbox: &Sandbox, action_id: &'static str, targets: &[&str]) -> Request<'static> {
    Request {
        action_id,
        operation: OperationRequest::DeleteRecursive,
        work_root: sandbox.work_root(),
        targets: targets.iter().map(|rel| sandbox.text(rel)).collect(),
        estimate: estimate(50, 4096),
    }
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

fn execute(executor: &mut Executor, flow: Flow) -> Result<Completed, Box<Failure>> {
    executor.execute(flow.permit, &flow.started, &mut MemoryJournal::new())
}

/// The folder of the batch of a quarantine flow, below the fence with `/`, and as a plan
/// writes it.
fn batch_of(sandbox: &Sandbox, flow: &Flow, action_id: &str) -> (String, String) {
    let destination = flow.permit.plan().destination().unwrap().to_string();
    let text = format!("{destination}{SEPARATOR}{action_id}");
    let below = text
        .strip_prefix(&sandbox.fence_text)
        .unwrap()
        .trim_start_matches(SEPARATOR)
        .replace(SEPARATOR, "/");
    (below, text)
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

/// The work root without the quarantine.
fn work_tree(sandbox: &Sandbox) -> BTreeMap<String, String> {
    let mut entries = tree(&sandbox.path("work"));
    entries.retain(|rel, _| !rel.starts_with(".ads"));
    entries
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Moment {
    Before,
    Opened,
}

/// A probe made of a closure.
struct Hook<F>(F);

impl<F: FnMut(Moment, &Mutation) -> io::Result<()>> Probe for Hook<F> {
    fn before(&mut self, mutation: &Mutation) -> io::Result<()> {
        (self.0)(Moment::Before, mutation)
    }

    fn opened(&mut self, mutation: &Mutation) -> io::Result<()> {
        (self.0)(Moment::Opened, mutation)
    }
}

fn hook(
    executor: Executor,
    probe: impl FnMut(Moment, &Mutation) -> io::Result<()> + 'static,
) -> Executor {
    executor.with_probe(Box::new(Hook(probe)))
}

fn touches_a_target(mutation: &Mutation) -> bool {
    matches!(mutation, Mutation::Rename { .. } | Mutation::Remove(_))
}

// ---- a swap after the last check ---------------------------------------------------------------

/// Between the opening of the handles and the system call, a folder on the way to the target
/// (or the fence itself) is replaced by a link to the sibling folder, which holds the same
/// names. The system call acts on what is open: the approved entry is the one changed, or the
/// call fails; the sibling is never touched. Where the system refuses the swap itself (a
/// folder with open handles below it, on Windows), there is nothing to redirect.
fn swap_after_the_last_check(name: &str, overrides: &'static str, swapped_folder: &str) {
    let sandbox = Sandbox::new(name);
    sandbox.file("work/a/data/file.txt", "inside");
    for under in ["data", "a/data", "work/a/data"] {
        let twin = sandbox.outside.join(under);
        fs::create_dir_all(&twin).unwrap();
        fs::write(twin.join("file.txt"), "outside").unwrap();
    }
    if !folder_link(&sandbox.path("work/probe"), &sandbox.outside) {
        return;
    }
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(overrides),
        delete(&sandbox, "act-1", &["work/a/data"]),
    );
    let batch = match overrides {
        "" => batch_of(&sandbox, &flow, "act-1").0,
        _ => String::new(),
    };

    let (folder, aside) = if swapped_folder.is_empty() {
        (sandbox.fence.clone(), sandbox.root.join("fence-real"))
    } else {
        (
            sandbox.path(swapped_folder),
            sandbox.path(&format!("{swapped_folder}-real")),
        )
    };
    let swapped = Rc::new(Cell::new(false));
    let tried = Rc::new(Cell::new(false));
    let executor = hook(sandbox.executor(overrides), {
        let (swapped, tried) = (swapped.clone(), tried.clone());
        let (folder, aside, outside) = (folder.clone(), aside.clone(), sandbox.outside.clone());
        move |moment, mutation| {
            if moment == Moment::Opened && touches_a_target(mutation) && !tried.replace(true) {
                // The system may refuse to rename a folder that has open handles below it.
                if fs::rename(&folder, &aside).is_ok() {
                    assert!(folder_link(&folder, &outside));
                    swapped.set(true);
                }
            }
            Ok(())
        }
    });
    let mut executor = executor;
    let outcome = execute(&mut executor, flow);
    assert!(tried.get(), "the hook ran between the opening and the call");
    println!("{name}: swapped = {}", swapped.get());
    // Linux lets a folder with open handles be renamed: there the swap is real, and the
    // call has to land on the approved entry all the same.
    if cfg!(target_os = "linux") {
        assert!(swapped.get(), "{name}: the swap did not happen");
        assert!(outcome.is_ok(), "{name}: {:?}", outcome.as_ref().err());
    }

    // Never the sibling, whatever happened.
    assert_eq!(tree(&sandbox.outside), outside_before);
    assert!(sandbox.sentinel_is_intact());

    // Where the approved entry really is now, by a path that does not go through the link.
    let below_swapped = |rel: &str| -> PathBuf {
        if !swapped.get() {
            return sandbox.path(rel);
        }
        if swapped_folder.is_empty() {
            let mut path = aside.clone();
            path.extend(rel.split('/'));
            path
        } else {
            let rest = rel.strip_prefix(swapped_folder).unwrap_or_else(|| {
                panic!("{rel} is not below {swapped_folder}");
            });
            PathBuf::from(format!("{}{rest}", aside.display()))
        }
    };
    let real = if swapped_folder.is_empty() || "work/a/data".starts_with(swapped_folder) {
        below_swapped("work/a/data/file.txt")
    } else {
        sandbox.path("work/a/data/file.txt")
    };
    match outcome {
        Ok(done) => {
            assert_eq!(done.report.steps[0].status, Status::Done);
            assert!(!real.exists(), "the approved entry is the one that went");
            if overrides.is_empty() {
                let stored = if swapped_folder.is_empty() {
                    below_swapped(&format!("{batch}/{ITEMS_DIR}/a/data/file.txt"))
                } else {
                    sandbox.path(&format!("{batch}/{ITEMS_DIR}/a/data/file.txt"))
                };
                assert_eq!(fs::read_to_string(stored).unwrap(), "inside");
            }
        }
        Err(failure) => {
            // The call failed: the approved entry is where it was, or (a move that was not
            // confirmed) whole in its batch.
            println!("{name}: {failure}");
            let kept = fs::read_to_string(&real).is_ok_and(|text| text == "inside");
            let unconfirmed = matches!(failure.report.steps[0].status, Status::Unconfirmed { .. });
            assert!(kept || unconfirmed, "{failure}");
        }
    }
}

#[test]
fn a_link_swapped_in_after_the_last_check_cannot_redirect_a_delete() {
    swap_after_the_last_check("lateswap-delete", QUARANTINE_OFF, "work/a");
}

#[test]
fn a_link_swapped_in_after_the_last_check_cannot_redirect_a_move() {
    swap_after_the_last_check("lateswap-move", "", "work/a");
}

#[test]
fn the_work_root_swapped_after_the_last_check_cannot_redirect_a_delete() {
    swap_after_the_last_check("lateswap-root", QUARANTINE_OFF, "work");
}

#[test]
fn the_fence_swapped_after_the_last_check_cannot_redirect_a_delete() {
    swap_after_the_last_check("lateswap-fence", QUARANTINE_OFF, "");
}

#[test]
fn the_fence_swapped_after_the_last_check_cannot_redirect_a_move() {
    swap_after_the_last_check("lateswap-fence-move", "", "");
}

// ---- a destination that appears at the last instant --------------------------------------------

#[test]
fn a_destination_created_at_the_last_instant_is_not_replaced() {
    for (at, as_folder) in [false, true].into_iter().enumerate() {
        let sandbox = Sandbox::new("latecollision");
        let policy = sandbox.policy("");
        sandbox.file("work/notes.txt", "old");
        let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/notes.txt"]));
        let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
        execute(&mut sandbox.executor(""), flow).unwrap();
        assert!(!sandbox.path("work/notes.txt").exists());

        // Another process takes the place after the last check found it vacant.
        let place = sandbox.path("work/notes.txt");
        let created = Rc::new(Cell::new(false));
        let mut executor = hook(sandbox.executor(""), {
            let (place, created) = (place.clone(), created.clone());
            move |moment, mutation| {
                if moment == Moment::Opened && matches!(mutation, Mutation::Rename { .. }) {
                    if as_folder {
                        fs::create_dir(&place)?;
                    } else {
                        fs::write(&place, "new")?;
                    }
                    created.set(true);
                }
                Ok(())
            }
        });
        let flow = through_gate(&policy, restore(&sandbox, "act-2", &batch_text));
        let failure = execute(&mut executor, flow).unwrap_err();
        assert!(created.get(), "{at}");
        assert_eq!(failure.reason, Reason::RestorePlaceTaken, "{at}: {failure}");
        assert!(matches!(
            failure.report.steps[0].status,
            Status::Failed {
                reason: Reason::RestorePlaceTaken,
                ..
            }
        ));
        // What took the place is intact, and the item is still whole in quarantine.
        if as_folder {
            assert!(place.is_dir());
        } else {
            assert_eq!(fs::read_to_string(&place).unwrap(), "new");
        }
        let stored = sandbox.path(&format!("{batch}/{ITEMS_DIR}/notes.txt"));
        assert_eq!(fs::read_to_string(stored).unwrap(), "old");
    }
}

#[test]
fn a_quarantine_place_created_at_the_last_instant_is_not_replaced() {
    let sandbox = Sandbox::new("latecollision-in");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-1", &["work/build"]),
    );
    let (batch, _) = batch_of(&sandbox, &flow, "act-1");
    let place = sandbox.path(&format!("{batch}/{ITEMS_DIR}/build"));
    let mut executor = hook(sandbox.executor(""), {
        let place = place.clone();
        move |moment, mutation| {
            if moment == Moment::Opened && matches!(mutation, Mutation::Rename { .. }) {
                fs::create_dir(&place)?;
            }
            Ok(())
        }
    });
    let failure = execute(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::DestinationTaken, "{failure}");
    assert_eq!(
        fs::read_to_string(sandbox.path("work/build/out.txt")).unwrap(),
        "text"
    );
    assert!(fs::read_dir(&place).unwrap().next().is_none());
}

// ---- only what was approved --------------------------------------------------------------------

/// The approved file gives its name to another file after the survey: same name, same kind,
/// another object.
#[test]
fn another_file_under_the_approved_name_is_not_touched() {
    for overrides in ["", QUARANTINE_OFF] {
        let sandbox = Sandbox::new("substituted");
        sandbox.file("work/a/report.txt", "approved");
        let flow = through_gate(
            &sandbox.policy(overrides),
            delete(&sandbox, "act-1", &["work/a/report.txt"]),
        );
        let done = Rc::new(Cell::new(false));
        let mut executor = hook(sandbox.executor(overrides), {
            let done = done.clone();
            let name = sandbox.path("work/a/report.txt");
            let aside = sandbox.path("work/a/kept.txt");
            move |moment, mutation| {
                let first = matches!(
                    mutation,
                    Mutation::Rename { .. } | Mutation::Remove(_) | Mutation::RemoveTree(_)
                );
                if moment == Moment::Before && first && !done.replace(true) {
                    fs::rename(&name, &aside)?;
                    fs::write(&name, "another")?;
                }
                Ok(())
            }
        });
        let failure = execute(&mut executor, flow).unwrap_err();
        assert!(done.get());
        assert_eq!(failure.reason, Reason::Changed, "{overrides}: {failure}");
        assert!(matches!(
            failure.report.steps[0].status,
            Status::Failed { removed: 0, .. }
        ));
        assert_eq!(
            fs::read_to_string(sandbox.path("work/a/report.txt")).unwrap(),
            "another"
        );
        assert_eq!(
            fs::read_to_string(sandbox.path("work/a/kept.txt")).unwrap(),
            "approved"
        );
    }
}

/// Files and a `.git` are put into the approved folder after the survey. The approval was
/// given for what was there: nothing is deleted on top of it, and nothing new goes along.
#[test]
fn entries_added_after_the_survey_are_not_deleted_nor_moved() {
    for overrides in ["", QUARANTINE_OFF] {
        let sandbox = Sandbox::new("added");
        sandbox.file("work/tree/a.txt", "a");
        sandbox.file("work/tree/sub/b.txt", "b");
        let flow = through_gate(
            &sandbox.policy(overrides),
            delete(&sandbox, "act-1", &["work/tree"]),
        );
        let done = Rc::new(Cell::new(false));
        let mut executor = hook(sandbox.executor(overrides), {
            let done = done.clone();
            let folder = sandbox.path("work/tree");
            move |moment, mutation| {
                let first = matches!(mutation, Mutation::Rename { .. } | Mutation::RemoveTree(_));
                if moment == Moment::Before && first && !done.replace(true) {
                    for n in 0..20 {
                        fs::write(folder.join(format!("new-{n}.txt")), "new")?;
                    }
                    fs::write(folder.join("sub").join("late.txt"), "late")?;
                    fs::create_dir(folder.join(".git"))?;
                    fs::write(folder.join(".git").join("HEAD"), "ref: refs/heads/main")?;
                }
                Ok(())
            }
        });
        let failure = execute(&mut executor, flow).unwrap_err();
        assert!(done.get());
        assert_eq!(failure.reason, Reason::Changed, "{overrides}: {failure}");
        assert!(matches!(
            failure.report.steps[0].status,
            Status::Failed { removed: 0, .. }
        ));
        // Everything is where it was: the approved entries and the new ones.
        let left = tree(&sandbox.path("work/tree"));
        assert_eq!(left.len(), 3 + 20 + 1 + 2, "{left:?}");
        assert_eq!(left.get("a.txt").unwrap(), "file:a");
        assert_eq!(left.get("sub/b.txt").unwrap(), "file:b");
        assert_eq!(left.get("sub/late.txt").unwrap(), "file:late");
    }
}

/// An entry put deep into the tree while the delete is already running: the folder it is in
/// stays, with the new entry, and the delete stops there.
#[test]
fn an_entry_added_while_a_tree_is_being_deleted_survives() {
    let sandbox = Sandbox::new("addedlate");
    sandbox.file("work/tree/a/one.txt", "1");
    sandbox.file("work/tree/z/two.txt", "2");
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/tree"]),
    );
    let done = Rc::new(Cell::new(false));
    let mut executor = hook(sandbox.executor(QUARANTINE_OFF), {
        let done = done.clone();
        let late = sandbox.path("work/tree/z/late.txt");
        move |moment, mutation| {
            if moment == Moment::Opened
                && matches!(mutation, Mutation::Remove(_))
                && !done.replace(true)
            {
                fs::write(&late, "late")?;
            }
            Ok(())
        }
    });
    let failure = execute(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::Changed, "{failure}");
    assert_eq!(
        fs::read_to_string(sandbox.path("work/tree/z/late.txt")).unwrap(),
        "late"
    );
    assert_eq!(
        fs::read_to_string(sandbox.path("work/tree/z/two.txt")).unwrap(),
        "2"
    );
}

#[test]
fn a_repository_deeper_in_the_target_refuses() {
    let sandbox = Sandbox::new("nestedrepo");
    sandbox.file("work/pack/project/.git/HEAD", "ref: refs/heads/main");
    sandbox.file("work/pack/project/src/main.rs", "fn main() {}");
    // A worktree, deeper, has `.git` as a file.
    sandbox.file("work/other/a/b/tree/.git", "gitdir: elsewhere");
    // On a file system that ignores case this is a repository too.
    sandbox.file("work/upper/project/.GIT/HEAD", "ref: refs/heads/main");
    sandbox.file("work/plain/a/b/file.txt", "x");
    for (at, target) in ["work/pack", "work/other", "work/upper"]
        .into_iter()
        .enumerate()
    {
        for (n, overrides) in ["", QUARANTINE_OFF].into_iter().enumerate() {
            let action_id = [["act-a", "act-b"], ["act-c", "act-d"], ["act-f", "act-g"]][at][n];
            let flow = through_gate(
                &sandbox.policy(overrides),
                delete(&sandbox, action_id, &[target]),
            );
            let before = tree(&sandbox.root);
            let mut executor = sandbox.executor(overrides);
            assert_eq!(
                executor.rehearse(&flow.permit).unwrap_err().reason,
                Reason::NestedRepository
            );
            let failure = execute(&mut executor, flow).unwrap_err();
            assert_eq!(failure.reason, Reason::NestedRepository, "{target}");
            assert_eq!(failure.stage, Stage::Checks);
            assert!(failure.report.untouched());
            assert_eq!(tree(&sandbox.root), before);
        }
    }
    // The fact is told to whoever measures, and a tree without a repository still goes.
    let path = |rel: &str| ads_core::gate::AbsPath::parse(&sandbox.text(rel)).unwrap();
    let executor = sandbox.executor("");
    let facts = executor.measure(&path("work"), &path("work/pack")).unwrap();
    assert!(facts.nested_repo && !facts.repo_root);
    let facts = executor
        .measure(&path("work"), &path("work/plain"))
        .unwrap();
    assert!(!facts.nested_repo && !facts.repo_root);
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-e", &["work/plain"]),
    );
    execute(&mut sandbox.executor(""), flow).unwrap();
}

// ---- the manifest ------------------------------------------------------------------------------

#[test]
fn a_manifest_that_would_not_be_read_back_refuses_before_any_change() {
    let sandbox = Sandbox::new("manifestsize");
    let names: Vec<String> = (0..12).map(|n| format!("work/target-{n}.txt")).collect();
    for name in &names {
        sandbox.file(name, "x");
    }
    let targets: Vec<&str> = names.iter().map(String::as_str).collect();
    let small = Limits::from_toml_str("max_manifest_bytes = 1024").unwrap();
    let before = tree(&sandbox.root);
    let before_work = work_tree(&sandbox);

    let flow = through_gate(&sandbox.policy(""), delete(&sandbox, "act-1", &targets));
    let mut executor = sandbox.executor("").with_limits(small);
    assert_eq!(
        executor.rehearse(&flow.permit).unwrap_err().reason,
        Reason::ManifestTooLarge
    );
    let failure = execute(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::ManifestTooLarge);
    assert_eq!(failure.stage, Stage::Checks);
    assert!(failure.report.untouched());
    assert_eq!(tree(&sandbox.root), before);

    // With the shipped limit the same batch is made, and its manifest is read back.
    let flow = through_gate(&sandbox.policy(""), delete(&sandbox, "act-2", &targets));
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-2");
    execute(&mut sandbox.executor(""), flow).unwrap();
    let written = fs::metadata(sandbox.path(&format!("{batch}/{MANIFEST_NAME}")))
        .unwrap()
        .len();
    assert!(written > 1024);

    // An executor that reads less refuses the restore, reading no more than its limit.
    let flow = through_gate(&sandbox.policy(""), restore(&sandbox, "act-3", &batch_text));
    let failure = execute(&mut sandbox.executor("").with_limits(small), flow).unwrap_err();
    assert_eq!(failure.reason, Reason::ManifestInvalid);
    assert!(failure.detail.contains("too large"), "{failure}");
    let flow = through_gate(&sandbox.policy(""), restore(&sandbox, "act-4", &batch_text));
    execute(&mut sandbox.executor(""), flow).unwrap();
    assert_eq!(work_tree(&sandbox), before_work);
}

/// The manifest of a batch replaced by a link to a file outside that reads as a good one.
#[test]
fn a_manifest_that_is_a_link_is_not_read() {
    let sandbox = Sandbox::new("manifestlink");
    let policy = sandbox.policy("");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/build"]));
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    execute(&mut sandbox.executor(""), flow).unwrap();

    let manifest = sandbox.path(&format!("{batch}/{MANIFEST_NAME}"));
    let copy = sandbox.outside.join("manifest-copy.json");
    fs::rename(&manifest, &copy).unwrap();
    if !common::symlink(&manifest, &copy) {
        return;
    }
    assert!(serde_json::from_slice::<Manifest>(&fs::read(&manifest).unwrap()).is_ok());
    let before = tree(&sandbox.root);
    let flow = through_gate(&policy, restore(&sandbox, "act-2", &batch_text));
    let mut executor = sandbox.executor("");
    assert_eq!(
        executor.rehearse(&flow.permit).unwrap_err().reason,
        Reason::ManifestInvalid
    );
    let failure = execute(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::ManifestInvalid, "{failure}");
    assert_eq!(tree(&sandbox.root), before);
}

// ---- a stop at every step ----------------------------------------------------------------------

/// Counts every call of the probe and fails the `nth`: the execution stops there, as if the
/// process had died.
fn dying_at(executor: Executor, nth: u32) -> Executor {
    let mut seen = 0;
    hook(executor, move |_, _| {
        seen += 1;
        if seen == nth {
            return Err(io::Error::other("the process dies here"));
        }
        Ok(())
    })
}

/// Whatever is in the batch comes back, by a new executor, as after a restart.
fn recover(sandbox: &Sandbox, batch: &str, batch_text: &str, action_id: &'static str) {
    let manifest = sandbox.path(&format!("{batch}/{MANIFEST_NAME}"));
    let items = sandbox.path(&format!("{batch}/{ITEMS_DIR}"));
    let stored = if items.is_dir() {
        fs::read_dir(&items).unwrap().count()
    } else {
        0
    };
    let readable = fs::read(&manifest)
        .ok()
        .is_some_and(|bytes| serde_json::from_slice::<Manifest>(&bytes).is_ok());
    if !readable {
        // No manifest yet: the order guarantees nothing was moved.
        assert_eq!(stored, 0, "items without a manifest");
        return;
    }
    if !sandbox.path(batch).is_dir() {
        return;
    }
    let flow = through_gate(&sandbox.policy(""), restore(sandbox, action_id, batch_text));
    match execute(&mut sandbox.executor(""), flow) {
        Ok(done) => assert!(
            done.report
                .steps
                .iter()
                .all(|step| matches!(step.status, Status::Done | Status::Absent))
        ),
        // Only the folders made on the way are left in the batch: no item.
        Err(failure) => assert_eq!(failure.reason, Reason::NothingToRestore, "{failure}"),
    }
}

#[test]
fn a_stop_at_any_step_of_a_quarantine_loses_nothing_and_is_told_as_it_is() {
    let mut stages = Vec::new();
    for nth in 1.. {
        let sandbox = Sandbox::new("crash-quarantine");
        sandbox.file("work/a/build/out.txt", "a");
        sandbox.file("work/a/build/deep/x.txt", "deep");
        sandbox.file("work/b/notes.txt", "b");
        sandbox.file("work/keep.txt", "kept");
        let before_work = work_tree(&sandbox);
        let before_all = tree(&sandbox.root);
        let flow = through_gate(
            &sandbox.policy(""),
            delete(&sandbox, "act-1", &["work/a/build", "work/b/notes.txt"]),
        );
        let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
        let mut executor = dying_at(sandbox.executor(""), nth);
        let failure = match execute(&mut executor, flow) {
            Ok(done) => {
                assert!(done.report.not_done().next().is_none());
                assert!(nth > 12, "the plan has more steps than {nth}");
                break;
            }
            Err(failure) => failure,
        };
        stages.push(failure.stage);

        // The report tells "nothing was changed" from "stopped while laying the batch".
        let changed = tree(&sandbox.root) != before_all;
        assert_eq!(failure.report.untouched(), !changed, "{nth}: {failure}");
        assert_ne!(failure.stage, Stage::Checks, "{nth}");
        if failure.stage == Stage::Staging {
            assert!(
                failure
                    .report
                    .steps
                    .iter()
                    .all(|step| step.status == Status::Pending)
            );
            assert_eq!(work_tree(&sandbox), before_work, "{nth}");
        }

        // Each target is whole in exactly one place: where it was, or in the batch.
        for target in ["a/build", "b/notes.txt"] {
            let at_home = sandbox.path(&format!("work/{target}")).exists();
            let stored = sandbox
                .path(&format!("{batch}/{ITEMS_DIR}/{target}"))
                .exists();
            assert!(at_home != stored, "{nth}: {target}: {at_home} {stored}");
        }

        // A step counts as done only with the folders involved confirmed.
        for step in &failure.report.steps {
            if step.status == Status::Done {
                let rel = step.target.strip_prefix(&sandbox.fence_text).unwrap();
                let rel = rel.trim_start_matches(SEPARATOR).replace(SEPARATOR, "/");
                assert!(!sandbox.path(&rel).exists(), "{nth}: {rel}");
            }
        }

        // The same action is never run again, not even by an executor and a journal that
        // forgot it: its folder is taken (or its targets are no longer where they were).
        if sandbox.path(&batch).is_dir() {
            let stopped = tree(&sandbox.root);
            let again = through_gate(
                &sandbox.policy(""),
                delete(&sandbox, "act-1", &["work/a/build", "work/b/notes.txt"]),
            );
            let refused = execute(&mut sandbox.executor(""), again).unwrap_err();
            assert!(
                matches!(refused.reason, Reason::DestinationTaken | Reason::Missing),
                "{nth}: {refused}"
            );
            assert_eq!(tree(&sandbox.root), stopped, "{nth}");
        }

        recover(&sandbox, &batch, &batch_text, "act-2");
        assert_eq!(work_tree(&sandbox), before_work, "{nth}: {failure}");
    }
    assert!(stages.contains(&Stage::Staging) && stages.contains(&Stage::Steps));
}

#[test]
fn a_stop_at_any_step_of_a_restore_loses_nothing() {
    for nth in 1.. {
        let sandbox = Sandbox::new("crash-restore");
        sandbox.file("work/a/b/notes.txt", "notes");
        sandbox.file("work/c/build/out.txt", "out");
        let before_work = work_tree(&sandbox);
        let policy = sandbox.policy("");
        let flow = through_gate(
            &policy,
            delete(&sandbox, "act-1", &["work/a", "work/c/build"]),
        );
        let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
        execute(&mut sandbox.executor(""), flow).unwrap();

        let flow = through_gate(&policy, restore(&sandbox, "act-2", &batch_text));
        let mut executor = dying_at(sandbox.executor(""), nth);
        let failure = match execute(&mut executor, flow) {
            Ok(_) => {
                assert_eq!(work_tree(&sandbox), before_work);
                assert!(nth > 4);
                break;
            }
            Err(failure) => failure,
        };
        assert_eq!(failure.stage, Stage::Steps, "{nth}");
        for target in ["a", "c/build"] {
            let at_home = sandbox.path(&format!("work/{target}")).exists();
            let stored = sandbox
                .path(&format!("{batch}/{ITEMS_DIR}/{target}"))
                .exists();
            assert!(at_home != stored, "{nth}: {target}");
        }
        // A second restore finds what is left and finishes.
        recover(&sandbox, &batch, &batch_text, "act-3");
        assert_eq!(work_tree(&sandbox), before_work, "{nth}: {failure}");
    }
}

#[test]
fn a_failure_while_laying_the_batch_is_reported_with_what_was_made() {
    let sandbox = Sandbox::new("staging");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-1", &["work/build"]),
    );
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");

    // The rehearsal lists what would be laid, all of it pending.
    let rehearsed = sandbox.executor("").rehearse(&flow.permit).unwrap();
    assert!(rehearsed.untouched());
    assert!(
        rehearsed
            .staging
            .iter()
            .all(|staged| staged.status == Laid::Pending)
    );
    assert_eq!(rehearsed.staging.last().unwrap().what, Scaffold::Manifest);

    let mut executor = hook(sandbox.executor(""), |moment, mutation| {
        if moment == Moment::Opened && matches!(mutation, Mutation::WriteFile(_)) {
            return Err(io::Error::other("the disk is full"));
        }
        Ok(())
    });
    let failure = execute(&mut executor, flow).unwrap_err();
    assert_eq!(failure.stage, Stage::Staging);
    assert!(!failure.report.untouched());
    assert_eq!(failure.report.steps[0].status, Status::Pending);
    let (manifest, folders) = failure.report.staging.split_last().unwrap();
    assert_eq!(manifest.what, Scaffold::Manifest);
    assert!(matches!(manifest.status, Laid::Failed { .. }));
    assert!(
        folders
            .iter()
            .all(|staged| staged.what == Scaffold::Folder && staged.status == Laid::Made)
    );
    // Exactly the folders the report lists are on disk, and the target did not move.
    for staged in folders {
        assert!(Path::new(&staged.path).is_dir(), "{}", staged.path);
    }
    assert!(sandbox.path(&batch).is_dir());
    assert!(!sandbox.path(&format!("{batch}/{MANIFEST_NAME}")).exists());
    assert!(sandbox.path("work/build/out.txt").exists());

    // What a new execution finds: the folder of that action is taken and holds nothing to
    // restore; another action does the job in its own folder.
    let flow = through_gate(&sandbox.policy(""), restore(&sandbox, "act-2", &batch_text));
    let refused = execute(&mut sandbox.executor(""), flow).unwrap_err();
    assert_eq!(refused.reason, Reason::ManifestInvalid);
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-3", &["work/build"]),
    );
    let done = execute(&mut sandbox.executor(""), flow).unwrap();
    assert!(
        done.report
            .staging
            .iter()
            .all(|staged| staged.status == Laid::Made)
    );
    assert!(!sandbox.path("work/build").exists());
}

#[test]
fn a_move_that_the_system_does_not_confirm_is_not_reported_as_done() {
    let sandbox = Sandbox::new("unconfirmed");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(
        &sandbox.policy(""),
        delete(&sandbox, "act-1", &["work/build"]),
    );
    let (batch, _) = batch_of(&sandbox, &flow, "act-1");
    let moved = Rc::new(Cell::new(false));
    let mut executor = hook(sandbox.executor(""), {
        let moved = moved.clone();
        move |moment, mutation| match mutation {
            Mutation::Rename { .. } if moment == Moment::Opened => {
                moved.set(true);
                Ok(())
            }
            Mutation::Sync(_) if moved.get() => Err(io::Error::other("the flush failed")),
            _ => Ok(()),
        }
    });
    let failure = execute(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::Unconfirmed);
    assert!(matches!(
        failure.report.steps[0].status,
        Status::Unconfirmed { .. }
    ));
    assert_eq!(failure.report.done().count(), 0);
    assert!(!failure.report.untouched());
    assert!(
        sandbox
            .path(&format!("{batch}/{ITEMS_DIR}/build/out.txt"))
            .exists()
    );
}

// ---- one permit, one execution -----------------------------------------------------------------

/// With the quarantine off nothing on disk remembers an action. The caller puts the path back
/// and comes again with a permit of the same action and a journal that never saw it.
#[test]
fn a_second_permit_of_an_action_is_refused_even_with_another_journal() {
    let sandbox = Sandbox::new("reuse");
    let policy = sandbox.policy(QUARANTINE_OFF);
    let mut executor = sandbox.executor(QUARANTINE_OFF);
    sandbox.file("work/a/data.txt", "first");
    sandbox.file("work/keep.txt", "kept");
    let flow = through_gate(&policy, delete(&sandbox, "act-1", &["work/a"]));
    let done = executor
        .execute(flow.permit, &flow.started, &mut MemoryJournal::new())
        .unwrap();
    assert!(!sandbox.path("work/a").exists());
    // What comes back holds no permit to run again: it can only close an action, and the
    // action of another permit refuses it and gives nothing back.
    let other = through_gate(&policy, delete(&sandbox, "act-9", &["work/keep.txt"]));
    let stray = execute(&mut executor, other).unwrap();
    let refused = stray
        .close(flow.action, meta("2026-11-15T12:00:09Z", 3))
        .unwrap_err();
    let (action, event, report) = done
        .close(refused.action, meta("2026-11-15T12:00:09Z", 3))
        .unwrap();
    assert!(matches!(event, ads_core::gate::Event::ActionExecuted(_)));
    assert!(action.state().is_terminal());
    assert_eq!(report.steps[0].status, Status::Done);

    sandbox.file("work/a/data.txt", "second");
    let before = tree(&sandbox.root);
    let again = through_gate(&policy, delete(&sandbox, "act-1", &["work/a"]));
    let mut other_journal = MemoryJournal::new();
    let failure = executor
        .execute(again.permit, &again.started, &mut other_journal)
        .unwrap_err();
    assert_eq!(failure.reason, Reason::PermitReused);
    assert_eq!(failure.stage, Stage::Checks);
    // The other journal was not even asked, and nothing was touched.
    assert!(other_journal.events().is_empty());
    assert_eq!(tree(&sandbox.root), before);

    // Another action is not affected.
    let flow = through_gate(&policy, delete(&sandbox, "act-2", &["work/a"]));
    execute(&mut executor, flow).unwrap();
    assert!(!sandbox.path("work/a").exists());
}

// ---- names -------------------------------------------------------------------------------------

#[test]
fn names_outside_ascii_and_long_names_go_and_come_back() {
    let sandbox = Sandbox::new("names");
    let long = "n".repeat(30);
    sandbox.file("work/ação é/файл 日本.txt", "unicode");
    sandbox.file(&format!("work/{long}/{long}.txt"), "long");
    sandbox.file("work/with space/a.b.c", "dots");
    let before = work_tree(&sandbox);
    let policy = sandbox.policy("");
    let long_target = format!("work/{long}");
    let flow = through_gate(
        &policy,
        delete(
            &sandbox,
            "act-1",
            &["work/ação é", &long_target, "work/with space/a.b.c"],
        ),
    );
    let (_, batch_text) = batch_of(&sandbox, &flow, "act-1");
    execute(&mut sandbox.executor(""), flow).unwrap();
    assert_eq!(work_tree(&sandbox).len(), 1);
    let flow = through_gate(&policy, restore(&sandbox, "act-2", &batch_text));
    execute(&mut sandbox.executor(""), flow).unwrap();
    assert_eq!(work_tree(&sandbox), before);
}

#[test]
fn a_read_only_file_goes_like_any_other() {
    let sandbox = Sandbox::new("readonly");
    let file = sandbox.file("work/tree/locked.txt", "x");
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&file, permissions).unwrap();
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/tree"]),
    );
    execute(&mut sandbox.executor(QUARANTINE_OFF), flow).unwrap();
    assert!(!sandbox.path("work/tree").exists());
}

/// Windows reaches one entry by more than one spelling. Only the spelling the folder lists is
/// accepted: another case is refused, so the text the gate judged is the entry that is opened.
#[cfg(windows)]
#[test]
fn another_case_of_a_name_is_refused() {
    let sandbox = Sandbox::new("case");
    sandbox.file("work/Build/out.txt", "text");
    let before = tree(&sandbox.root);
    for (action_id, target) in [("act-1", "work/build"), ("act-2", "work/BUILD/out.txt")] {
        let flow = through_gate(&sandbox.policy(""), delete(&sandbox, action_id, &[target]));
        let failure = execute(&mut sandbox.executor(""), flow).unwrap_err();
        assert_eq!(failure.reason, Reason::NotPhysical, "{target}: {failure}");
        assert_eq!(tree(&sandbox.root), before);
    }
    // A work root in another case is another folder as far as the executor goes.
    let flow = through_gate(
        &sandbox.policy(""),
        Request {
            action_id: "act-3",
            operation: OperationRequest::DeleteRecursive,
            work_root: sandbox.text("WORK"),
            targets: vec![sandbox.text("WORK/Build")],
            estimate: estimate(50, 4096),
        },
    );
    let failure = execute(&mut sandbox.executor(""), flow).unwrap_err();
    assert_eq!(failure.reason, Reason::NotPhysical, "{failure}");
    assert_eq!(tree(&sandbox.root), before);
}

/// A folder that tells `Data` from `data` (case sensitivity turned on for it): the plan names
/// one of the two, and only that one goes.
#[cfg(windows)]
#[test]
fn in_a_case_sensitive_folder_only_the_named_entry_goes() {
    let sandbox = Sandbox::new("casesensitive");
    let folder = sandbox.dir("work/cs");
    let enabled = std::process::Command::new("fsutil.exe")
        .args(["file", "setCaseSensitiveInfo"])
        .arg(&folder)
        .arg("enable")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    let both = enabled
        && fs::create_dir(folder.join("Data")).is_ok()
        && fs::create_dir(folder.join("data")).is_ok();
    if !both {
        eprintln!("SKIPPED: this system does not make a case sensitive folder");
        return;
    }
    fs::write(folder.join("Data").join("file.txt"), "upper").unwrap();
    fs::write(folder.join("data").join("file.txt"), "lower").unwrap();
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete(&sandbox, "act-1", &["work/cs/data"]),
    );
    execute(&mut sandbox.executor(QUARANTINE_OFF), flow).unwrap();
    assert!(!folder.join("data").exists());
    assert_eq!(
        fs::read_to_string(folder.join("Data").join("file.txt")).unwrap(),
        "upper"
    );
}
