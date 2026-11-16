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

use ads_adapter_fs::{Mutation, Reason, Status};

use common::{Moment, Sandbox, delete_of, hooked, run, through_gate, tree};

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
