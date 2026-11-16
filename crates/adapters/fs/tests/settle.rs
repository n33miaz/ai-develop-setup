//! The third independent review, each finding with the test that keeps it closed: an entry
//! claimed to be removed cannot leave the fence before it goes, a move to the holding folder
//! that is not verified promises no place, the end of an execution reports the removal of
//! its holding folder as it went, and a folder that was created is told as created. Always
//! inside the folder of the test (see `common`).

mod common;

use std::cell::RefCell;
use std::fs;
use std::io;
use std::rc::Rc;

use ads_adapter_fs::{
    Executor, FenceError, Laid, MANIFEST_NAME, Midway, Mutation, Reason, Scaffold, Stage, Status,
};

use common::{Moment, SEPARATOR, Sandbox, batch_of, delete_of, hooked, run, through_gate, tree};

const QUARANTINE_OFF: &str = "quarantine_enabled = false";

/// The sharing violation of Windows: someone has the entry open in a way that forbids this.
const SHARING_VIOLATION: i32 = 32;

// ---- an entry claimed to be removed stays where it was claimed ---------------------------------

/// Between the last check of an entry about to be removed and the removal, another process
/// moves the entry itself (not a folder above it) out of the fence. On Windows the claim
/// does not let anyone rename the entry: the move is refused and the removal happens inside.
/// On Linux nothing can refuse it: the entry is looked for again under its name before the
/// removal, which stops. Either way nothing outside the fence goes away.
#[test]
fn an_entry_claimed_to_be_removed_cannot_be_taken_out_of_the_fence() {
    // (the target of the delete, the entry taken at the last instant, the entries left)
    let cases = [
        ("work/one.txt", "one.txt", 1),
        ("work/tree", "a.txt", 1),
        ("work/tree", "empty", 1),
    ];
    for (target, taken, left) in cases {
        let sandbox = Sandbox::new("claimed");
        sandbox.file("work/one.txt", "1");
        sandbox.file("work/tree/a.txt", "a");
        sandbox.dir("work/tree/empty");
        sandbox.file("work/keep.txt", "kept");
        let outside_before = tree(&sandbox.outside);
        let flow = through_gate(
            &sandbox.policy(QUARANTINE_OFF),
            delete_of(&sandbox, "act-1", &[target]),
        );
        // (it was there when the probe looked, the answer of the system to the move)
        let tried = Rc::new(RefCell::new(None));
        let away = sandbox.outside.join(format!("taken-{taken}"));
        let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), {
            let (tried, away) = (tried.clone(), away.clone());
            move |moment, mutation| {
                let Mutation::Remove(path) = mutation else {
                    return Ok(());
                };
                if moment == Moment::Opened && path.ends_with(taken) && tried.borrow().is_none() {
                    let there = fs::symlink_metadata(path).is_ok();
                    let moved = fs::rename(path, &away).map_err(|error| error.raw_os_error());
                    *tried.borrow_mut() = Some((there, moved));
                }
                Ok(())
            }
        });
        let outcome = run(&mut executor, flow);
        let (there, moved) = (*tried.borrow()).expect("the probe ran");
        assert!(
            there,
            "{target}/{taken}: the entry was not where it was claimed"
        );
        if cfg!(windows) {
            // Refused by the system itself, and then removed inside the fence.
            assert_eq!(moved, Err(Some(SHARING_VIOLATION)), "{target}/{taken}");
            outcome.unwrap();
            assert_eq!(tree(&sandbox.outside), outside_before, "{target}/{taken}");
        } else {
            // Taken: the removal finds it gone from its folder and stops.
            assert_eq!(moved, Ok(()), "{target}/{taken}");
            let failure = outcome.unwrap_err();
            assert_eq!(
                failure.reason,
                Reason::Displaced,
                "{target}/{taken}: {failure}"
            );
            assert!(fs::symlink_metadata(&away).is_ok(), "{target}/{taken}");
        }
        // What was outside is outside, whatever happened.
        for (rel, what) in &outside_before {
            assert_eq!(
                tree(&sandbox.outside).get(rel),
                Some(what),
                "{target}/{taken}: {rel}"
            );
        }
        assert!(sandbox.sentinel_is_intact());
        assert!(sandbox.work_tree().len() >= left, "{target}/{taken}");
        assert!(sandbox.path("work/keep.txt").exists());
    }
}

/// The removal of one file: the reason the system gives when the entry is open by another
/// claim, so that the case above cannot pass because the move failed for another reason.
#[test]
fn a_file_open_to_be_deleted_by_someone_else_is_not_claimed() {
    if !cfg!(windows) {
        return;
    }
    let sandbox = Sandbox::new("claimed-busy");
    let path = sandbox.file("work/one.txt", "1");
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete_of(&sandbox, "act-1", &["work/one.txt"]),
    );
    let busy: Rc<RefCell<Option<fs::File>>> = Rc::new(RefCell::new(None));
    let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), {
        let (busy, path) = (busy.clone(), path.clone());
        move |moment, mutation| {
            if moment == Moment::Before && matches!(mutation, Mutation::Remove(_)) {
                *busy.borrow_mut() = Some(open_to_delete(&path)?);
            }
            Ok(())
        }
    });
    let failure = run(&mut executor, flow).unwrap_err();
    assert!(busy.borrow().is_some());
    assert_eq!(failure.reason, Reason::Io, "{failure}");
    assert!(matches!(
        failure.report.steps[0].status,
        Status::Failed { .. }
    ));
    drop(busy.borrow_mut().take());
    assert_eq!(fs::read_to_string(&path).unwrap(), "1");
}

#[cfg(windows)]
fn open_to_delete(path: &std::path::Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const DELETE: u32 = 0x0001_0000;
    const SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
    fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(SHARE_ALL)
        .open(path)
}

#[cfg(not(windows))]
fn open_to_delete(_path: &std::path::Path) -> io::Result<fs::File> {
    Err(io::Error::other("Windows only"))
}

// ---- a move to the holding folder that is not verified promises no place -----------------------

/// The system moved the tree to its holding folder and the check right after the move failed
/// (on Linux, another process took it away from under the name). The step says the target
/// left its place and does not say it is held there: `Unverified`, never `Held`.
#[test]
fn a_tree_not_verified_in_its_holding_folder_is_not_reported_as_held() {
    let sandbox = Sandbox::new("unverified-hold");
    sandbox.file("work/tree/a.txt", "a");
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete_of(&sandbox, "act-1", &["work/tree"]),
    );
    let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), |moment, mutation| match (
        moment, mutation,
    ) {
        (Moment::Midway(Midway::Moved), Mutation::Rename { .. }) => {
            Err(io::Error::other("the check after the move failed"))
        }
        _ => Ok(()),
    });
    let failure = run(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::MoveUnverified, "{failure}");
    let step = &failure.report.steps[0];
    assert!(
        matches!(step.status, Status::Unverified { .. }),
        "{:?}",
        step.status
    );
    assert!(!failure.report.untouched());
    // The place it was sent to is still named, as a place tried, and the detail says so.
    let tried = step.held.as_ref().unwrap();
    match &step.status {
        Status::Unverified { detail } => assert!(detail.contains(tried.as_str()), "{detail}"),
        other => panic!("{other:?}"),
    }
    assert!(!sandbox.path("work/tree").exists());
}

// ---- the holding folder goes at the end like any other removal ---------------------------------

fn holding_of<'a>(report: &'a ads_adapter_fs::Report, sandbox: &Sandbox) -> &'a Laid {
    let holding = sandbox.path(&format!("{}/act-1", sandbox.private_dir()));
    &report
        .staging
        .iter()
        .find(|staged| std::path::Path::new(&staged.path) == holding)
        .unwrap()
        .status
}

/// The holding folder of the execution is removed at the end. When the system removed it and
/// does not confirm its folder on disk, the execution does not end as a success: every step
/// is done, and the report says the removal went and was not confirmed.
#[test]
fn a_holding_folder_removed_and_not_confirmed_is_a_failure() {
    let sandbox = Sandbox::new("clear-sync");
    sandbox.file("work/tree/a.txt", "a");
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete_of(&sandbox, "act-1", &["work/tree"]),
    );
    let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), {
        let mut cleared = false;
        move |moment, mutation| match (moment, mutation) {
            (Moment::Opened, Mutation::Remove(path)) if path.ends_with("act-1") => {
                cleared = true;
                Ok(())
            }
            (Moment::Before, Mutation::Sync(path)) if cleared && path.ends_with("tmp") => {
                Err(io::Error::other("the flush failed"))
            }
            _ => Ok(()),
        }
    });
    let failure = run(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::Unconfirmed, "{failure}");
    assert_eq!(failure.stage, Stage::Clearing);
    assert_eq!(failure.report.steps[0].status, Status::Done);
    assert!(
        matches!(
            holding_of(&failure.report, &sandbox),
            Laid::ClearedUnconfirmed { .. }
        ),
        "{:?}",
        failure.report.staging
    );
    assert!(tree(&sandbox.path(sandbox.private_dir())).is_empty());
}

/// While the holding folder is removed, it and the folder it is in stay where they are: on
/// Windows the system refuses to move either out of the fence. On Linux the folder that left
/// is not found again under its name, and is not removed; the execution still ends, its
/// steps done, with the folder reported as made.
#[test]
fn a_holding_folder_cannot_be_taken_out_of_the_fence_while_it_is_removed() {
    let sandbox = Sandbox::new("clear-hold");
    sandbox.file("work/tree/a.txt", "a");
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete_of(&sandbox, "act-1", &["work/tree"]),
    );
    let attempts = Rc::new(RefCell::new(Vec::new()));
    let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), {
        let (attempts, outside) = (attempts.clone(), sandbox.outside.clone());
        move |moment, mutation| {
            let Mutation::Remove(path) = mutation else {
                return Ok(());
            };
            if moment != Moment::Opened || !path.ends_with("act-1") {
                return Ok(());
            }
            // The holding folder, then the private folder it is in.
            for folder in path.ancestors().take(2) {
                let n = attempts.borrow().len();
                let moved = fs::rename(folder, outside.join(format!("taken-{n}"))).is_ok();
                attempts.borrow_mut().push(moved);
                if moved {
                    break;
                }
            }
            Ok(())
        }
    });
    let done = run(&mut executor, flow).unwrap();
    assert_eq!(done.report.steps[0].status, Status::Done);
    let attempts = attempts.borrow().clone();
    if cfg!(windows) {
        assert_eq!(attempts, [false, false]);
        assert_eq!(holding_of(&done.report, &sandbox), &Laid::Cleared);
        assert_eq!(tree(&sandbox.outside), outside_before);
    } else {
        assert_eq!(attempts, [true]);
        assert_eq!(holding_of(&done.report, &sandbox), &Laid::Made);
        assert!(sandbox.outside.join("taken-0").is_dir());
    }
    assert!(sandbox.sentinel_is_intact());
}

// ---- what was created is told as created, and its folder is confirmed --------------------------

/// The folder a path ends in, with `/`, as the last two names.
fn last_two(path: &std::path::Path) -> String {
    let names: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    names[names.len() - 2..].join("/")
}

/// A folder of the batch is made and the execution stops right after (the folder could not be
/// opened, the resources ran out). It exists: the report says "created, not confirmed", never
/// "not created", and the folder it is in is confirmed on disk on the way out.
#[test]
fn a_folder_that_stops_after_it_was_made_is_told_as_made_and_confirmed() {
    let sandbox = Sandbox::new("made-midway");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(
        &sandbox.policy(""),
        delete_of(&sandbox, "act-1", &["work/build"]),
    );
    let (batch, _) = batch_of(&sandbox, &flow, "act-1");
    let after = Rc::new(RefCell::new(Vec::new()));
    let mut executor = hooked(sandbox.executor(""), {
        let after = after.clone();
        let mut stopped: Option<std::path::PathBuf> = None;
        move |moment, mutation| match (moment, mutation) {
            (Moment::Midway(Midway::Created), Mutation::CreateDir(path))
                if path.ends_with("act-1") =>
            {
                stopped = Some(path.clone());
                Err(io::Error::other("out of resources"))
            }
            (Moment::Before, Mutation::Sync(folder)) if stopped.is_some() => {
                after.borrow_mut().push(last_two(folder));
                Ok(())
            }
            _ => Ok(()),
        }
    });
    let failure = run(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::Unconfirmed, "{failure}");
    assert_eq!(failure.stage, Stage::Staging);
    let made = failure
        .report
        .staging
        .iter()
        .find(|staged| staged.path.ends_with("act-1"))
        .unwrap();
    assert_eq!(made.what, Scaffold::Folder);
    match &made.status {
        Laid::Unconfirmed { detail } => {
            assert!(detail.contains("its folder was confirmed"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
    assert!(sandbox.path(&batch).is_dir());
    // The folder that holds the new one, and only it, was confirmed after the stop.
    let holder = last_two(sandbox.path(&batch).parent().unwrap());
    assert_eq!(*after.borrow(), [holder]);
    assert!(!failure.report.untouched());
    assert_eq!(failure.report.steps[0].status, Status::Pending);
}

/// The manifest is created and its writing fails, at either point: the folder of the batch,
/// where the manifest now is, is still confirmed on disk before the execution stops.
#[test]
fn a_manifest_that_fails_after_it_was_created_has_its_folder_confirmed() {
    for at in [Midway::Created, Midway::Written] {
        let sandbox = Sandbox::new("manifest-folder");
        sandbox.file("work/build/out.txt", "text");
        let flow = through_gate(
            &sandbox.policy(""),
            delete_of(&sandbox, "act-1", &["work/build"]),
        );
        let (batch, _) = batch_of(&sandbox, &flow, "act-1");
        let after = Rc::new(RefCell::new(Vec::new()));
        let mut executor = hooked(sandbox.executor(""), {
            let after = after.clone();
            let mut stopped = false;
            move |moment, mutation| match (moment, mutation) {
                (Moment::Midway(point), Mutation::WriteFile(_)) if point == at => {
                    stopped = true;
                    Err(io::Error::other("no space left on the device"))
                }
                (Moment::Before, Mutation::Sync(folder)) if stopped => {
                    after.borrow_mut().push(last_two(folder));
                    Ok(())
                }
                _ => Ok(()),
            }
        });
        let failure = run(&mut executor, flow).unwrap_err();
        assert_eq!(failure.reason, Reason::Unconfirmed, "{at:?}: {failure}");
        let manifest = failure.report.staging.last().unwrap();
        assert_eq!(manifest.what, Scaffold::Manifest);
        assert!(
            matches!(manifest.status, Laid::Unconfirmed { .. }),
            "{at:?}: {:?}",
            manifest.status
        );
        assert!(sandbox.path(&format!("{batch}/{MANIFEST_NAME}")).is_file());
        assert_eq!(*after.borrow(), [last_two(&sandbox.path(&batch))], "{at:?}");
    }
}

// ---- the text of the fence names what the walk opened ------------------------------------------

/// While the fence is opened name by name, a folder already opened on the way is moved aside
/// and a new one, with the same names inside, takes its place. On Windows the folder is held
/// from the instant it is open and the move is refused. On Linux nothing refuses it, and the
/// second walk from the root of the volume finds another folder under the name. Either way
/// the fence that is built is the folder its text names, or there is no fence.
#[test]
fn a_folder_moved_while_the_fence_is_walked_does_not_give_the_fence_another_name() {
    let sandbox = Sandbox::new("build-walk");
    let base = sandbox.root.join("base");
    let aside = sandbox.root.join("base-first");
    fs::create_dir_all(base.join("fence").join("work")).unwrap();
    fs::write(base.join("fence").join("work").join("file.txt"), "first").unwrap();
    let moved = Rc::new(RefCell::new(None));
    let built = Executor::new_walked(&base.join("fence"), sandbox.policy(""), || {}, {
        let (base, aside, moved) = (base.clone(), aside.clone(), moved.clone());
        move |name| {
            if name != "base" || moved.borrow().is_some() {
                return;
            }
            let outcome = fs::rename(&base, &aside);
            if outcome.is_ok() {
                fs::create_dir_all(base.join("fence").join("work")).unwrap();
                fs::write(
                    base.join("fence").join("work").join("file.txt"),
                    "second one",
                )
                .unwrap();
            }
            *moved.borrow_mut() = Some(outcome.is_ok());
        }
    });
    let moved = (*moved.borrow()).expect("the walk went through `base`");
    if cfg!(windows) {
        assert!(!moved, "a folder on the way was moved while it was held");
    }
    let executor = match built {
        Ok(executor) => executor,
        Err(FenceError::NotPlain(name)) => {
            assert!(moved);
            assert_eq!(name, "base");
            return;
        }
        Err(other) => panic!("{other:?}"),
    };
    // Built: what is open under the text is what the text names on disk now.
    let text = executor.fence_text();
    let path = |rel: &str| {
        let native = rel.replace('/', &SEPARATOR.to_string());
        ads_core::gate::AbsPath::parse(&format!("{text}{SEPARATOR}{native}")).unwrap()
    };
    let facts = executor
        .measure(&path("work"), &path("work/file.txt"))
        .unwrap();
    let on_disk = fs::read(base.join("fence").join("work").join("file.txt")).unwrap();
    assert_eq!(facts.bytes, on_disk.len() as u64);
}
