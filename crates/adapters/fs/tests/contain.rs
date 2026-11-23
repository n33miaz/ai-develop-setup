//! Containment, each case with the test that keeps it closed: the fence
//! is opened name by name, a file that grew is not what was approved, a move that happened is
//! told as one, a manifest that exists is told as one, a tree is removed for good only where
//! it is contained, and a restore puts nothing on a reserved folder. Always inside the folder
//! of the test (see `common`).

mod common;

use common::shared::{Cell, RefCell};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ads_adapter_fs::{
    Executor, FenceError, ITEMS_DIR, Laid, MANIFEST_NAME, Manifest, Midway, Mutation, Reason,
    Scaffold, Stage, Status,
};
use ads_core::gate::AbsPath;

use common::{
    Moment, Request, Sandbox, batch_of, delete_of, folder_link, hooked, limits, restore_of, run,
    through_gate, tree,
};

const QUARANTINE_OFF: &str = "quarantine_enabled = false";

fn sys_moves_by_handle() -> bool {
    cfg!(windows)
}

/// Where the creation of a folder gives the identity of what it made (Windows), the holding
/// folder of an execution is removed at its end. Elsewhere it is left, and reported as made.
fn clears_holding() -> bool {
    cfg!(windows)
}

fn append(path: &Path, text: &str) -> io::Result<()> {
    fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(text.as_bytes())
}

/// The last names of a path, with `/`.
fn tail(path: &Path, names: usize) -> String {
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts[parts.len() - names..].join("/")
}

// ---- the fence is opened name by name ----------------------------------------------------------

/// Between the resolution of the path and the opening of the fence, a folder above it is
/// replaced by a link to the sibling, which holds a folder of the same name. The fence is
/// walked from the root of the volume without following a link: it does not open.
#[test]
fn an_ancestor_swapped_for_a_link_while_the_fence_is_built_is_refused() {
    let sandbox = Sandbox::new("build-link");
    let base = sandbox.root.join("base");
    fs::create_dir_all(base.join("fence").join("work")).unwrap();
    fs::create_dir_all(sandbox.outside.join("fence").join("work")).unwrap();
    if !folder_link(&sandbox.root.join("probe"), &sandbox.outside) {
        return;
    }
    let aside = sandbox.root.join("base-real");
    let swapped = Arc::new(Cell::new(false));
    let built = Executor::new_watched(&base.join("fence"), sandbox.policy(""), {
        let (base, outside, swapped) = (base.clone(), sandbox.outside.clone(), swapped.clone());
        move || {
            fs::rename(&base, &aside).unwrap();
            assert!(folder_link(&base, &outside));
            swapped.set(true);
        }
    });
    assert!(swapped.get());
    match built.err() {
        Some(FenceError::NotPlain(name)) => assert_eq!(name, "base"),
        other => panic!("the fence was built through a link: {other:?}"),
    }
    assert!(sandbox.sentinel_is_intact());

    // A fence asked for through a link, with nothing swapped: the resolution gives the real
    // names, and those are the ones walked and published.
    let built = Executor::new(&base.join("fence"), sandbox.policy("")).unwrap();
    assert!(
        built
            .fence_text()
            .ends_with(&format!("outside{0}fence", common::SEPARATOR))
    );
}

/// The same instant, with a real folder put in the place: what is open is what the published
/// text names now, not what was there when the path was resolved.
#[test]
fn the_text_of_the_fence_names_the_folder_that_was_opened() {
    let sandbox = Sandbox::new("build-real");
    let base = sandbox.root.join("base");
    fs::create_dir_all(base.join("fence").join("work")).unwrap();
    fs::write(base.join("fence").join("work").join("file.txt"), "first").unwrap();
    let executor = Executor::new_watched(&base.join("fence"), sandbox.policy(""), {
        let (base, aside) = (base.clone(), sandbox.root.join("base-first"));
        move || {
            fs::rename(&base, &aside).unwrap();
            fs::create_dir_all(base.join("fence").join("work")).unwrap();
            fs::write(
                base.join("fence").join("work").join("file.txt"),
                "second one",
            )
            .unwrap();
        }
    })
    .unwrap();
    let text = executor.fence_text();
    let path = |rel: &str| {
        let native = rel.replace('/', &common::SEPARATOR.to_string());
        AbsPath::parse(&format!("{text}{}{native}", common::SEPARATOR)).unwrap()
    };
    let facts = executor
        .measure(&path("work"), &path("work/file.txt"))
        .unwrap();
    assert_eq!(facts.bytes, "second one".len() as u64);
}

// ---- a file that grew is not what was approved -------------------------------------------------

/// The approval was given for the bytes that were counted. A file that keeps its identity
/// and grows afterwards (written to directly, or through another hard link) is refused.
#[test]
fn a_file_that_grew_after_the_survey_is_not_removed_nor_moved() {
    // (policy, target, the file that grows, written through)
    let cases = [
        (
            QUARANTINE_OFF,
            "work/one.txt",
            "work/one.txt",
            "work/one.txt",
        ),
        (
            QUARANTINE_OFF,
            "work/tree",
            "work/tree/a.txt",
            "work/tree/a.txt",
        ),
        (
            QUARANTINE_OFF,
            "work/tree",
            "work/tree/a.txt",
            "work/alias.txt",
        ),
        ("", "work/tree", "work/tree/a.txt", "work/alias.txt"),
    ];
    for (at, (overrides, target, file, through)) in cases.into_iter().enumerate() {
        let sandbox = Sandbox::new("grew");
        sandbox.file("work/one.txt", "1");
        sandbox.file("work/tree/a.txt", "1");
        sandbox.file("work/tree/sub/b.txt", "22");
        fs::hard_link(
            sandbox.path("work/tree/a.txt"),
            sandbox.path("work/alias.txt"),
        )
        .unwrap();
        let flow = through_gate(
            &sandbox.policy(overrides),
            delete_of(&sandbox, "act-1", &[target]),
        );
        let grew = Arc::new(Cell::new(false));
        let mut executor = hooked(sandbox.executor(overrides), {
            let (grew, through) = (grew.clone(), sandbox.path(through));
            move |moment, mutation| {
                let first = matches!(
                    mutation,
                    Mutation::Rename { .. } | Mutation::Remove(_) | Mutation::RemoveTree(_)
                );
                if moment == Moment::Before && first && !grew.replace(true) {
                    append(&through, &"x".repeat(100_000))?;
                }
                Ok(())
            }
        });
        let failure = run(&mut executor, flow).unwrap_err();
        assert!(grew.get(), "{at}");
        assert_eq!(failure.reason, Reason::Changed, "{at}: {failure}");
        assert!(failure.detail.contains("bytes"), "{at}: {failure}");
        assert!(
            matches!(failure.report.steps[0].status, Status::Failed { .. }),
            "{at}"
        );
        // It is where it was, with everything that was written to it.
        assert_eq!(
            fs::metadata(sandbox.path(file)).unwrap().len(),
            100_001,
            "{at}"
        );
        assert_eq!(sandbox.work_tree().len(), 6, "{at}");
    }
}

#[test]
fn a_file_that_grew_in_quarantine_is_not_purged() {
    let sandbox = Sandbox::new("grew-purge");
    let policy = sandbox.policy("");
    sandbox.file("work/build/out.txt", "1");
    let flow = through_gate(&policy, delete_of(&sandbox, "act-1", &["work/build"]));
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    run(&mut sandbox.executor(""), flow).unwrap();

    let stored = sandbox.path(&format!("{batch}/{ITEMS_DIR}/build/out.txt"));
    let purge = Request {
        targets: vec![batch_text],
        ..delete_of(&sandbox, "act-2", &[])
    };
    let flow = through_gate(&policy, purge);
    assert!(flow.permit.approval_id().is_some());
    let mut executor = hooked(sandbox.executor(""), {
        let stored = stored.clone();
        move |moment, mutation| {
            if moment == Moment::Before && matches!(mutation, Mutation::RemoveTree(_)) {
                append(&stored, &"x".repeat(100_000))?;
            }
            Ok(())
        }
    });
    let failure = run(&mut executor, flow).unwrap_err();
    assert_eq!(failure.reason, Reason::Changed, "{failure}");
    assert_eq!(fs::metadata(&stored).unwrap().len(), 100_001);
}

// ---- a move that happened is told as one -------------------------------------------------------

/// After the system made a move, whatever fails is not "nothing was changed".
#[test]
fn a_move_that_is_not_verified_is_told_as_a_move_that_happened() {
    let sandbox = Sandbox::new("unverified");
    let policy = sandbox.policy("");
    sandbox.file("work/notes.txt", "notes");
    let flow = through_gate(&policy, delete_of(&sandbox, "act-1", &["work/notes.txt"]));
    let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
    run(&mut sandbox.executor(""), flow).unwrap();
    let stored = sandbox.path(&format!("{batch}/{ITEMS_DIR}/notes.txt"));
    assert!(stored.exists());

    // The restore moves the item back; the check right after the move fails.
    let synced = Arc::new(RefCell::new(Vec::new()));
    let moved = Arc::new(Cell::new(false));
    let mut executor = hooked(sandbox.executor(""), {
        let (synced, moved) = (synced.clone(), moved.clone());
        move |moment, mutation| match (moment, mutation) {
            (Moment::Midway(Midway::Moved), Mutation::Rename { .. }) => {
                moved.set(true);
                Err(io::Error::other("the check after the move failed"))
            }
            (Moment::Before, Mutation::Sync(folder)) if moved.get() => {
                synced.borrow_mut().push(tail(folder, 1));
                Ok(())
            }
            _ => Ok(()),
        }
    });
    let flow = through_gate(&policy, restore_of(&sandbox, "act-2", &batch_text));
    let failure = run(&mut executor, flow).unwrap_err();
    assert!(moved.get());
    assert_eq!(failure.reason, Reason::MoveUnverified, "{failure}");
    assert_eq!(failure.stage, Stage::Steps);
    assert!(matches!(
        failure.report.steps[0].status,
        Status::Unverified { .. }
    ));
    // Not "nothing was changed": the item left the quarantine.
    assert!(!failure.report.untouched());
    assert_eq!(failure.report.done().count(), 0);
    assert!(!stored.exists());
    assert_eq!(
        fs::read_to_string(sandbox.path("work/notes.txt")).unwrap(),
        "notes"
    );
    // The two folders of the move were confirmed all the same.
    assert_eq!(*synced.borrow(), ["work", ITEMS_DIR]);
}

/// Where a move acts on a name (Linux), what arrived is looked at again; another process
/// that takes it away in between leaves a move that happened and cannot be vouched for.
/// Where a move acts on the open entry itself (Windows), the handle is the proof.
#[test]
fn an_entry_taken_away_right_after_its_move_is_not_reported_as_untouched() {
    let sandbox = Sandbox::new("takenaway");
    let policy = sandbox.policy("");
    sandbox.file("work/notes.txt", "notes");
    let flow = through_gate(&policy, delete_of(&sandbox, "act-1", &["work/notes.txt"]));
    let (_, batch_text) = batch_of(&sandbox, &flow, "act-1");
    run(&mut sandbox.executor(""), flow).unwrap();

    let mut executor = hooked(sandbox.executor(""), {
        let (place, away) = (
            sandbox.path("work/notes.txt"),
            sandbox.path("work/elsewhere.txt"),
        );
        move |moment, mutation| {
            if moment == Moment::Midway(Midway::Moved)
                && matches!(mutation, Mutation::Rename { .. })
            {
                fs::rename(&place, &away)?;
            }
            Ok(())
        }
    });
    let flow = through_gate(&policy, restore_of(&sandbox, "act-2", &batch_text));
    let outcome = run(&mut executor, flow);
    // Vouched for only where the handle of the entry is the proof.
    assert_eq!(
        outcome.is_ok(),
        sys_moves_by_handle(),
        "{:?}",
        outcome.as_ref().err()
    );
    let report = match outcome {
        Ok(done) => done.report,
        Err(failure) => {
            assert_eq!(failure.reason, Reason::MoveUnverified, "{failure}");
            assert!(matches!(
                failure.report.steps[0].status,
                Status::Unverified { .. }
            ));
            failure.report
        }
    };
    assert!(!report.untouched());
    assert_eq!(
        fs::read_to_string(sandbox.path("work/elsewhere.txt")).unwrap(),
        "notes"
    );
}

// ---- a manifest that exists is told as one -----------------------------------------------------

/// The manifest is created, then written, then confirmed. Stopping after the first step
/// leaves a file on disk: the report says so, and says that no target was moved.
#[test]
fn a_manifest_that_stops_after_it_was_created_is_told_as_created() {
    for at in [Midway::Created, Midway::Written] {
        let sandbox = Sandbox::new("manifest-midway");
        let policy = sandbox.policy("");
        sandbox.file("work/build/out.txt", "text");
        let flow = through_gate(&policy, delete_of(&sandbox, "act-1", &["work/build"]));
        let (batch, batch_text) = batch_of(&sandbox, &flow, "act-1");
        let mut executor = hooked(sandbox.executor(""), move |moment, mutation| {
            match (moment, mutation) {
                (Moment::Midway(point), Mutation::WriteFile(_)) if point == at => {
                    Err(io::Error::other("no space left on the device"))
                }
                _ => Ok(()),
            }
        });
        let failure = run(&mut executor, flow).unwrap_err();
        assert_eq!(failure.reason, Reason::Unconfirmed, "{at:?}: {failure}");
        assert_eq!(failure.stage, Stage::Staging, "{at:?}");
        assert!(!failure.report.untouched(), "{at:?}");
        let staged = failure.report.staging.last().unwrap();
        assert_eq!(staged.what, Scaffold::Manifest);
        assert!(
            matches!(staged.status, Laid::Unconfirmed { .. }),
            "{at:?}: {:?}",
            staged.status
        );
        // No target was moved, and the file the report speaks of is on disk.
        assert!(
            failure
                .report
                .steps
                .iter()
                .all(|step| step.status == Status::Pending)
        );
        assert_eq!(
            fs::read_to_string(sandbox.path("work/build/out.txt")).unwrap(),
            "text"
        );
        let written = fs::read(sandbox.path(&format!("{batch}/{MANIFEST_NAME}"))).unwrap();
        let whole = serde_json::from_slice::<Manifest>(&written).is_ok();
        assert_eq!(whole, at == Midway::Written, "{at:?}");
        assert_eq!(written.is_empty(), at == Midway::Created, "{at:?}");

        // What a restore of that batch finds: a manifest that does not read, or one that
        // reads and lists items that never left.
        let flow = through_gate(&policy, restore_of(&sandbox, "act-2", &batch_text));
        let refused = run(&mut sandbox.executor(""), flow).unwrap_err();
        let expected = match at {
            Midway::Written => Reason::NothingToRestore,
            _ => Reason::ManifestInvalid,
        };
        assert_eq!(refused.reason, expected, "{at:?}: {refused}");
        assert!(sandbox.path("work/build/out.txt").exists());
    }
}

// ---- a tree is removed for good only where it is contained -------------------------------------

/// What a probe is told from the first removal on: `("remove" | "sync", last two names)`.
fn recorded(log: &Arc<RefCell<Vec<(String, String)>>>, mutation: &Mutation) {
    let mut log = log.borrow_mut();
    match mutation {
        Mutation::Remove(path) => log.push(("remove".to_owned(), tail(path, 2))),
        Mutation::Sync(path) if !log.is_empty() => log.push(("sync".to_owned(), tail(path, 2))),
        _ => {}
    }
}

fn pairs(log: &Arc<RefCell<Vec<(String, String)>>>) -> Vec<(String, String)> {
    log.borrow().clone()
}

fn pair(kind: &str, names: &str) -> (String, String) {
    (kind.to_owned(), names.to_owned())
}

/// A tree is not emptied where it is: it is moved, whole, to a folder made for the
/// execution, and every removal happens there. Every folder that lost an entry is confirmed
/// on disk before it is left.
#[test]
fn a_tree_is_removed_in_its_holding_folder_and_every_folder_is_confirmed() {
    let sandbox = Sandbox::new("holding");
    sandbox.file("work/tree/a.txt", "a");
    sandbox.file("work/tree/sub/b.txt", "b");
    sandbox.file("work/tree/sub/c.txt", "c");
    sandbox.file("work/keep.txt", "kept");
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete_of(&sandbox, "act-1", &["work/tree"]),
    );
    let private = sandbox.private_dir();
    let holding = sandbox.path(&format!("{private}/act-1"));

    // The rehearsal says where the tree would be held, and changes nothing.
    let rehearsed = sandbox
        .executor(QUARANTINE_OFF)
        .rehearse(&flow.permit)
        .unwrap();
    let held = PathBuf::from(rehearsed.steps[0].held.as_ref().unwrap());
    assert_eq!(held, holding.join("0"));
    assert!(rehearsed.untouched() && !sandbox.path("work/.ads").exists());

    let log = Arc::new(RefCell::new(Vec::new()));
    let moves = Arc::new(RefCell::new(Vec::new()));
    let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), {
        let (log, moves) = (log.clone(), moves.clone());
        let holding = fs::canonicalize(&sandbox.fence)
            .unwrap()
            .join("work")
            .join(".ads")
            .join("tmp")
            .join("act-1");
        move |moment, mutation| {
            if moment != Moment::Before {
                return Ok(());
            }
            match mutation {
                Mutation::Rename { from, to } => {
                    assert!(log.borrow().is_empty(), "moved after a removal");
                    moves.borrow_mut().push((tail(from, 2), tail(to, 2)));
                }
                Mutation::Remove(path) => {
                    assert!(
                        path.starts_with(&holding),
                        "{} removed in place",
                        path.display()
                    );
                }
                _ => {}
            }
            recorded(&log, mutation);
            Ok(())
        }
    });
    let done = run(&mut executor, flow).unwrap();
    assert_eq!(done.report.steps[0].status, Status::Done);
    assert_eq!(*moves.borrow(), [pair("work/tree", "act-1/0")]);
    let mut expected = vec![
        pair("remove", "0/a.txt"),
        pair("remove", "sub/b.txt"),
        pair("remove", "sub/c.txt"),
        pair("sync", "0/sub"),
        pair("remove", "0/sub"),
        pair("sync", "act-1/0"),
        pair("remove", "act-1/0"),
        pair("sync", "tmp/act-1"),
    ];
    if clears_holding() {
        // The holding folder itself, made for this execution, goes at the end.
        expected.extend([pair("remove", "tmp/act-1"), pair("sync", ".ads/tmp")]);
    }
    assert_eq!(pairs(&log), expected);
    let cleared = done
        .report
        .staging
        .iter()
        .find(|staged| Path::new(&staged.path) == holding)
        .unwrap();
    if clears_holding() {
        assert_eq!(cleared.status, Laid::Cleared);
        assert!(tree(&sandbox.path(private)).is_empty());
    } else {
        assert_eq!(cleared.status, Laid::Made);
        assert_eq!(
            tree(&sandbox.path(private)).keys().collect::<Vec<_>>(),
            ["act-1"]
        );
    }
    assert_eq!(sandbox.work_tree().keys().collect::<Vec<_>>(), ["keep.txt"]);
}

/// A removal that stops in the middle confirms what it changed before it returns, and the
/// report tells entries seen to go from entries confirmed.
#[test]
fn a_tree_removal_that_stops_confirms_the_folders_it_changed() {
    for unconfirmed in [false, true] {
        let sandbox = Sandbox::new("holding-stop");
        sandbox.file("work/tree/a.txt", "a");
        sandbox.file("work/tree/sub/b.txt", "b");
        sandbox.file("work/tree/sub/c.txt", "c");
        let flow = through_gate(
            &sandbox.policy(QUARANTINE_OFF),
            delete_of(&sandbox, "act-1", &["work/tree"]),
        );
        let log = Arc::new(RefCell::new(Vec::new()));
        let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), {
            let log = log.clone();
            move |moment, mutation| {
                if moment != Moment::Before {
                    return Ok(());
                }
                recorded(&log, mutation);
                match mutation {
                    Mutation::Remove(path) if path.ends_with("c.txt") => {
                        Err(io::Error::other("the file is in use"))
                    }
                    Mutation::Sync(path) if unconfirmed && path.ends_with("sub") => {
                        Err(io::Error::other("the flush failed"))
                    }
                    _ => Ok(()),
                }
            }
        });
        let failure = run(&mut executor, flow).unwrap_err();
        assert_eq!(failure.reason, Reason::Io, "{failure}");
        // `a.txt` and `sub/b.txt` went. Both folders are confirmed on the way out, the
        // inner one first.
        assert_eq!(
            pairs(&log),
            [
                pair("remove", "0/a.txt"),
                pair("remove", "sub/b.txt"),
                pair("remove", "sub/c.txt"),
                pair("sync", "0/sub"),
                pair("sync", "act-1/0"),
            ]
        );
        match &failure.report.steps[0].status {
            Status::Held {
                removed, confirmed, ..
            } => {
                assert_eq!(*removed, 2);
                assert_eq!(*confirmed, if unconfirmed { 1 } else { 2 });
            }
            other => panic!("{other:?}"),
        }
        assert!(!failure.report.untouched());
        let held = PathBuf::from(failure.report.steps[0].held.as_ref().unwrap());
        assert_eq!(tree(&held).keys().collect::<Vec<_>>(), ["sub", "sub/c.txt"]);
        assert!(sandbox.work_tree().is_empty());
    }
}

/// While a tree is emptied, the folder being emptied and every folder above it stay where
/// they are. On Windows the system refuses to move them, between two entries and with an
/// entry open. On Linux nothing can hold them: with the parameter that lets a delete run
/// there, a folder taken out of the holding folder is noticed before the next removal, and
/// the removal stops (it is not noticed if it is taken after that look).
#[test]
fn a_folder_being_emptied_stays_where_it_is() {
    let sandbox = Sandbox::new("holdfast");
    sandbox.file("work/tree/a/1.txt", "1");
    sandbox.file("work/tree/a/2.txt", "2");
    sandbox.file("work/tree/b.txt", "b");
    let outside_before = tree(&sandbox.outside);
    let flow = through_gate(
        &sandbox.policy(QUARANTINE_OFF),
        delete_of(&sandbox, "act-1", &["work/tree"]),
    );
    // (the instant, the folder, whether it was taken)
    let attempts = Arc::new(RefCell::new(Vec::new()));
    let mut executor = hooked(sandbox.executor(QUARANTINE_OFF), {
        let (attempts, outside) = (attempts.clone(), sandbox.outside.clone());
        let mut taken_one = false;
        move |moment, mutation| {
            let Mutation::Remove(path) = mutation else {
                return Ok(());
            };
            if !path.ends_with("2.txt") || matches!(moment, Moment::Midway(_)) || taken_one {
                return Ok(());
            }
            // The six folders from the one being emptied up to the work root, and no
            // further: `a`, the tree, the holding folder, the private folder, `.ads`, `work`.
            assert_eq!(tail(path, 7), "work/.ads/tmp/act-1/0/a/2.txt");
            for folder in path.ancestors().skip(1).take(6) {
                let n = attempts.borrow().len();
                let moved = fs::rename(folder, outside.join(format!("taken-{n}"))).is_ok();
                attempts.borrow_mut().push((moment, tail(folder, 1), moved));
                if moved {
                    taken_one = true;
                    break;
                }
            }
            Ok(())
        }
    });
    let outcome = run(&mut executor, flow);
    let attempts = attempts.borrow().clone();
    assert!(!attempts.is_empty());
    println!("holdfast: {attempts:?}");

    if cfg!(windows) {
        // Nothing moved: six folders, at each of the two instants.
        assert_eq!(attempts.len(), 12, "{attempts:?}");
        assert!(attempts.iter().all(|(_, _, moved)| !moved), "{attempts:?}");
        assert!(
            attempts
                .iter()
                .any(|(moment, ..)| *moment == Moment::Before)
        );
        assert!(
            attempts
                .iter()
                .any(|(moment, ..)| *moment == Moment::Opened)
        );
        outcome.unwrap();
        assert!(sandbox.work_tree().is_empty());
        assert_eq!(tree(&sandbox.outside), outside_before);
    } else {
        // Taken at the first try, between two entries. The removal stops before it touches
        // anything in the folder that left.
        assert_eq!(attempts, [(Moment::Before, "a".to_owned(), true)]);
        let failure = outcome.unwrap_err();
        assert_eq!(failure.reason, Reason::Displaced, "{failure}");
        assert!(matches!(
            failure.report.steps[0].status,
            Status::Held { removed: 1, .. }
        ));
        assert_eq!(
            fs::read_to_string(sandbox.outside.join("taken-0").join("2.txt")).unwrap(),
            "2"
        );
    }
    assert!(sandbox.sentinel_is_intact());
}

/// The shipped parameter: an entry is removed for good only where the system can hold its
/// folder in place. On Linux no path leads to a removal: not a tree, not one file, not the
/// purge of the quarantine.
#[test]
fn by_default_nothing_is_removed_for_good_where_folders_cannot_be_held() {
    let sandbox = Sandbox::new("default");
    let policy = sandbox.policy("");
    sandbox.file("work/build/out.txt", "text");
    let flow = through_gate(&policy, delete_of(&sandbox, "act-0", &["work/build"]));
    let (_, batch_text) = batch_of(&sandbox, &flow, "act-0");
    run(&mut sandbox.shipped_executor(""), flow).unwrap();
    sandbox.file("work/tree/a.txt", "a");
    sandbox.file("work/one.txt", "1");

    let purge = Request {
        targets: vec![batch_text],
        ..delete_of(&sandbox, "act-3", &[])
    };
    let cases = [
        (QUARANTINE_OFF, delete_of(&sandbox, "act-1", &["work/tree"])),
        (
            QUARANTINE_OFF,
            delete_of(&sandbox, "act-2", &["work/one.txt"]),
        ),
        ("", purge),
    ];
    for (at, (overrides, request)) in cases.into_iter().enumerate() {
        let flow = through_gate(&sandbox.policy(overrides), request);
        let before = tree(&sandbox.root);
        let told = Arc::new(Cell::new(0));
        let mut executor = hooked(sandbox.shipped_executor(overrides), {
            let told = told.clone();
            move |_, _| {
                told.set(told.get() + 1);
                Ok(())
            }
        });
        let rehearsed = executor.rehearse(&flow.permit);
        let outcome = run(&mut executor, flow);
        if cfg!(windows) {
            rehearsed.unwrap();
            outcome.unwrap();
            assert_ne!(tree(&sandbox.root), before, "{at}");
        } else {
            assert_eq!(rehearsed.unwrap_err().reason, Reason::DeleteUncontained);
            let failure = outcome.unwrap_err();
            assert_eq!(failure.reason, Reason::DeleteUncontained, "{at}: {failure}");
            assert_eq!(failure.stage, Stage::Checks);
            assert!(failure.report.untouched());
            // The probe was told of no mutation at all: nothing reached a system call.
            assert_eq!(told.get(), 0, "{at}");
            assert_eq!(tree(&sandbox.root), before, "{at}");
        }
    }
}

#[test]
fn with_definitive_deletes_turned_off_nothing_is_removed_anywhere() {
    let sandbox = Sandbox::new("refuse");
    sandbox.file("work/tree/a.txt", "a");
    sandbox.file("work/one.txt", "1");
    let off = limits("definitive_delete = \"refuse\"");
    for (action_id, target) in [("act-1", "work/tree"), ("act-2", "work/one.txt")] {
        let flow = through_gate(
            &sandbox.policy(QUARANTINE_OFF),
            delete_of(&sandbox, action_id, &[target]),
        );
        let before = tree(&sandbox.root);
        let mut executor = sandbox.executor(QUARANTINE_OFF).with_limits(off.clone());
        let failure = run(&mut executor, flow).unwrap_err();
        assert_eq!(failure.reason, Reason::DeleteUncontained, "{failure}");
        assert_eq!(tree(&sandbox.root), before);
    }
    // The quarantine still runs.
    let flow = through_gate(
        &sandbox.policy(""),
        delete_of(&sandbox, "act-3", &["work/tree"]),
    );
    run(&mut sandbox.executor("").with_limits(off), flow).unwrap();
    assert!(!sandbox.path("work/tree").exists());
}

/// What a delete that stopped leaves is in its holding folder. The same action does not run
/// again; the private folder itself is never a target; another action removes what was left.
#[test]
fn what_a_stopped_delete_left_is_removed_by_another_action() {
    let sandbox = Sandbox::new("leftover");
    let policy = sandbox.policy(QUARANTINE_OFF);
    sandbox.file("work/tree/a.txt", "a");
    sandbox.file("work/tree/b.txt", "b");
    let flow = through_gate(&policy, delete_of(&sandbox, "act-1", &["work/tree"]));
    let mut executor = hooked(
        sandbox.executor(QUARANTINE_OFF),
        |moment, mutation| match mutation {
            Mutation::Remove(path) if moment == Moment::Opened && path.ends_with("b.txt") => {
                Err(io::Error::other("the file is in use"))
            }
            _ => Ok(()),
        },
    );
    let failure = run(&mut executor, flow).unwrap_err();
    let held = failure.report.steps[0].held.clone().unwrap();
    assert!(matches!(
        failure.report.steps[0].status,
        Status::Held { removed: 1, .. }
    ));
    let private = sandbox.private_dir();
    let holding = format!("{private}/act-1");
    assert_eq!(sandbox.below(&held), format!("{holding}/0"));
    assert_eq!(
        tree(&sandbox.path(&holding)).keys().collect::<Vec<_>>(),
        ["0", "0/b.txt"]
    );

    // The same action again: its target is gone and its holding folder is taken.
    sandbox.file("work/tree/c.txt", "c");
    let again = through_gate(&policy, delete_of(&sandbox, "act-1", &["work/tree"]));
    let refused = run(&mut sandbox.executor(QUARANTINE_OFF), again).unwrap_err();
    assert_eq!(refused.reason, Reason::DestinationTaken, "{refused}");
    assert!(sandbox.path("work/tree/c.txt").exists());

    // The private folder, or a folder that holds it, is not a target.
    for (action_id, target) in [("act-2", private), ("act-3", "work/.ads")] {
        let flow = through_gate(&policy, delete_of(&sandbox, action_id, &[target]));
        let before = tree(&sandbox.root);
        let refused = run(&mut sandbox.executor(QUARANTINE_OFF), flow).unwrap_err();
        assert_eq!(refused.reason, Reason::Unsupported, "{target}: {refused}");
        assert_eq!(tree(&sandbox.root), before);
    }

    // What the first action left is a target like any other.
    let flow = through_gate(&policy, delete_of(&sandbox, "act-4", &[&holding]));
    run(&mut sandbox.executor(QUARANTINE_OFF), flow).unwrap();
    let left: Vec<String> = tree(&sandbox.path(private)).into_keys().collect();
    match clears_holding() {
        true => assert!(left.is_empty(), "{left:?}"),
        // The empty holding folder of the action that finished the job.
        false => assert_eq!(left, ["act-4"]),
    }
}

// ---- a restore puts nothing on a reserved folder -----------------------------------------------

/// The gate judged the work root as the destination of a restore; the manifest says where
/// each item really goes. An item that would be, or would hold, a reserved folder (here an
/// allowed root that is missing on disk) is refused by the rules of the gate itself, before
/// any item is moved.
#[test]
fn a_restore_does_not_recreate_a_reserved_folder() {
    // (the item that was quarantined, the allowed root of the policy that restores)
    for (at, (item, root)) in [
        ("work/shared", "work/shared"),
        ("work/outer", "work/outer/inner"),
    ]
    .into_iter()
    .enumerate()
    {
        let sandbox = Sandbox::new("reserved");
        sandbox.file(&format!("{root}/data.txt"), "data");
        sandbox.file("work/plain/file.txt", "plain");
        let flow = through_gate(
            &sandbox.policy(""),
            delete_of(&sandbox, "act-1", &["work/plain", item]),
        );
        let (_, batch_text) = batch_of(&sandbox, &flow, "act-1");
        run(&mut sandbox.executor(""), flow).unwrap();
        assert!(!sandbox.path(item).exists());

        // A policy that names the root. The permit is one of a gate that does not.
        let strict = format!("allowed_roots = ['{}']", sandbox.text(root));
        let flow = through_gate(
            &sandbox.policy(""),
            restore_of(&sandbox, "act-2", &batch_text),
        );
        let before = tree(&sandbox.root);
        let mut executor = sandbox.executor(&strict);
        let rehearsed = executor.rehearse(&flow.permit).unwrap_err();
        assert_eq!(rehearsed.reason, Reason::PolicyDenies, "{at}: {rehearsed}");
        let failure = run(&mut executor, flow).unwrap_err();
        assert_eq!(failure.reason, Reason::PolicyDenies, "{at}: {failure}");
        assert!(
            failure.detail.contains("destination_entry_reserved"),
            "{at}: {failure}"
        );
        assert_eq!(failure.stage, Stage::Checks);
        // Not even the item that could go back was moved.
        assert_eq!(tree(&sandbox.root), before, "{at}");

        // Under the policy of the gate that planned it, the same batch goes back whole.
        let flow = through_gate(
            &sandbox.policy(""),
            restore_of(&sandbox, "act-3", &batch_text),
        );
        run(&mut sandbox.executor(""), flow).unwrap();
        assert!(sandbox.path(&format!("{root}/data.txt")).exists());
        assert!(sandbox.path("work/plain/file.txt").exists());
    }
}
