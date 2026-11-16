//! The system layer by itself: what is opened is the entry, never what a link points to, and
//! an entry is known by its identity, not by its name. Always inside the folder of the test
//! (see `common`).

mod common;

#[allow(dead_code, unused_imports)]
#[path = "../src/sys/mod.rs"]
mod sys;

use std::fs;
use std::path::Path;

use common::Sandbox;
use sys::{Error, Kind, Node};

/// Opens `path` the way the fence is opened: from the root of its volume, name by name.
fn open(path: &Path) -> Node {
    let resolved = fs::canonicalize(path).unwrap();
    assert!(resolved.starts_with(fs::canonicalize(env!("CARGO_TARGET_TMPDIR")).unwrap()));
    let mut parts = resolved.components();
    let mut anchor = std::path::PathBuf::new();
    for part in parts.by_ref() {
        anchor.push(part);
        if matches!(part, std::path::Component::RootDir) {
            break;
        }
    }
    let mut node = Node::anchor(&anchor).unwrap();
    for part in parts {
        node = node.child(part.as_os_str().to_str().unwrap()).unwrap();
    }
    node
}

fn fence(sandbox: &Sandbox) -> Node {
    open(&sandbox.fence)
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

fn kind(node: &Node) -> Kind {
    node.meta().unwrap().kind.unwrap()
}

// ---- opening, with no mutation -----------------------------------------------------------------

#[test]
fn an_entry_is_opened_by_one_name_of_an_open_folder() {
    let sandbox = Sandbox::new("sys-open");
    sandbox.file("work/a/file.txt", "12345");
    sandbox.dir("work/a/empty");
    let root = fence(&sandbox);
    assert_eq!(kind(&root), Kind::Dir);
    let a = root.child("work").unwrap().child("a").unwrap();
    assert_eq!(kind(&a), Kind::Dir);
    assert_eq!(a.names().unwrap(), ["empty", "file.txt"]);
    let file = a.child("file.txt").unwrap();
    let meta = file.meta().unwrap();
    assert_eq!(meta.kind, Some(Kind::File));
    assert_eq!(meta.size, 5);
    assert!(a.child("empty").unwrap().names().unwrap().is_empty());
    assert!(matches!(a.child("gone"), Err(Error::Missing)));
    assert_eq!(a.read_file("file.txt", 5).unwrap(), b"12345");

    // A name is one name: no path, no step up, nothing through the open folder.
    for name in ["", ".", "..", "a/file.txt", "a\\file.txt", "../outside"] {
        assert!(matches!(root.child(name), Err(Error::Io(_))), "{name:?}");
    }
}

#[test]
fn identity_tells_one_object_from_another_under_the_same_name() {
    let sandbox = Sandbox::new("sys-identity");
    sandbox.file("work/one.txt", "same");
    sandbox.file("work/two.txt", "same");
    let work = fence(&sandbox).child("work").unwrap();
    let one = work.child("one.txt").unwrap().meta().unwrap();
    let again = work.child("one.txt").unwrap().meta().unwrap();
    let two = work.child("two.txt").unwrap().meta().unwrap();
    assert_eq!(one.id, again.id);
    assert_ne!(one.id, two.id);
    assert_eq!(one.id.volume, two.id.volume);
    assert_eq!(
        work.again().unwrap().meta().unwrap().id,
        work.meta().unwrap().id
    );
    // The same file system all the way, and the system says so.
    let home = fence(&sandbox).meta().unwrap().mount;
    assert_eq!(sys::crossing(&home, &one.mount), sys::Crossing::Same);
}

#[test]
fn a_link_is_opened_as_a_link_and_never_followed() {
    let sandbox = Sandbox::new("sys-link");
    if !folder_link(&sandbox.path("work/link"), &sandbox.outside) {
        return;
    }
    // By path the sentinel is reachable through the link.
    assert!(sandbox.path("work/link").join(common::SENTINEL).exists());
    let work = fence(&sandbox).child("work").unwrap();
    let link = work.child("link").unwrap();
    assert_eq!(kind(&link), Kind::Link);
    // What is open is the link: it is not the folder outside, and holds nothing of it.
    let outside = open(&sandbox.outside);
    assert_ne!(link.meta().unwrap().id, outside.meta().unwrap().id);
    assert!(link.child(common::SENTINEL).is_err());
    assert!(link.names().is_err() || link.names().unwrap().is_empty());
    assert!(work.read_file("link", 64).is_err());
    assert!(sandbox.sentinel_is_intact());
}

#[test]
fn a_file_link_is_not_read_through() {
    let sandbox = Sandbox::new("sys-filelink");
    if !common::symlink(
        &sandbox.path("work/manifest.json"),
        &sandbox.outside.join(common::SENTINEL),
    ) {
        return;
    }
    let work = fence(&sandbox).child("work").unwrap();
    assert_eq!(kind(&work.child("manifest.json").unwrap()), Kind::Link);
    assert!(work.read_file("manifest.json", 1024).is_err());
}

#[test]
fn a_read_stops_at_the_limit() {
    let sandbox = Sandbox::new("sys-limit");
    sandbox.file("work/big.txt", "123456789");
    let work = fence(&sandbox).child("work").unwrap();
    assert_eq!(work.read_file("big.txt", 9).unwrap().len(), 9);
    match work.read_file("big.txt", 8) {
        Err(Error::Io(error)) => assert_eq!(error.kind(), std::io::ErrorKind::FileTooLarge),
        other => panic!("{other:?}"),
    }
    assert!(work.read_file("gone.txt", 8).is_err());
    assert!(fence(&sandbox).read_file("work", 8).is_err());
}

/// Windows reaches one entry by several spellings (another case, the short name). Only the
/// spelling the folder lists is the entry.
#[cfg(windows)]
#[test]
fn another_spelling_of_a_name_is_an_alias_not_the_entry() {
    let sandbox = Sandbox::new("sys-alias");
    sandbox.file("work/A Long Folder Name/file.txt", "x");
    let work = fence(&sandbox).child("work").unwrap();
    assert!(work.child("A Long Folder Name").is_ok());
    assert!(matches!(
        work.child("a long folder name"),
        Err(Error::Alias)
    ));
    assert!(matches!(fence(&sandbox).child("WORK"), Err(Error::Alias)));
    // The short name, when the volume keeps one.
    if sandbox.path("work/ALONGF~1").exists() {
        assert!(matches!(work.child("ALONGF~1"), Err(Error::Alias)));
    }
    // A stream of a file is not a name.
    assert!(matches!(
        work.child("A Long Folder Name:stream"),
        Err(Error::Io(_))
    ));
}

// ---- mutations, by handle -----------------------------------------------------------------------

#[test]
fn creation_is_exclusive_and_relative_to_the_open_folder() {
    let sandbox = Sandbox::new("sys-create");
    let work = fence(&sandbox).child("work").unwrap();
    let made = work.make_dir("new").unwrap();
    assert_eq!(kind(&made), Kind::Dir);
    assert!(matches!(work.make_dir("new"), Err(Error::Exists)));
    {
        use std::io::Write;
        let mut file = made.create_file("file.txt").unwrap();
        // It exists from this instant, empty.
        assert_eq!(fs::read(sandbox.path("work/new/file.txt")).unwrap(), b"");
        file.write_all(b"text").unwrap();
        file.sync_all().unwrap();
    }
    assert!(matches!(made.create_file("file.txt"), Err(Error::Exists)));
    assert!(work.create_file("new").is_err());
    assert_eq!(
        fs::read_to_string(sandbox.path("work/new/file.txt")).unwrap(),
        "text"
    );
    made.sync().unwrap();
    work.sync().unwrap();
}

#[test]
fn a_move_refuses_a_destination_that_exists() {
    let sandbox = Sandbox::new("sys-move");
    sandbox.file("work/from/file.txt", "moved");
    sandbox.file("work/from/tree/deep.txt", "deep");
    sandbox.file("work/to/taken.txt", "kept");
    sandbox.dir("work/to/empty");
    let work = fence(&sandbox).child("work").unwrap();
    let from = work.child("from").unwrap();
    let to = work.child("to").unwrap();
    let id = |name: &str| from.child(name).unwrap().meta().unwrap().id;

    // Over a file and over an empty folder: refused by the system, nothing replaced.
    for taken in ["taken.txt", "empty"] {
        let claim = from.claim("file.txt", id("file.txt")).unwrap();
        let refused = claim.move_to(&to, taken);
        assert!(
            matches!(refused, Err(Error::Exists)),
            "{taken}: {refused:?}"
        );
        let claim = from.claim("tree", id("tree")).unwrap();
        let refused = claim.move_to(&to, taken);
        assert!(
            matches!(refused, Err(Error::Exists)),
            "{taken}: {refused:?}"
        );
    }
    assert_eq!(
        fs::read_to_string(sandbox.path("work/to/taken.txt")).unwrap(),
        "kept"
    );
    assert!(sandbox.path("work/from/file.txt").exists());

    // To a free name of another open folder: a file and a whole tree.
    let file_id = id("file.txt");
    from.claim("file.txt", file_id)
        .unwrap()
        .move_to(&to, "file.txt")
        .unwrap();
    assert_eq!(to.child("file.txt").unwrap().meta().unwrap().id, file_id);
    from.claim("tree", id("tree"))
        .unwrap()
        .move_to(&to, "tree")
        .unwrap();
    assert_eq!(
        fs::read_to_string(sandbox.path("work/to/tree/deep.txt")).unwrap(),
        "deep"
    );
    assert!(from.names().unwrap().is_empty());
    from.sync().unwrap();
    to.sync().unwrap();
}

#[test]
fn only_the_object_that_was_checked_is_claimed() {
    let sandbox = Sandbox::new("sys-claim");
    sandbox.file("work/a.txt", "approved");
    let work = fence(&sandbox).child("work").unwrap();
    let approved = work.child("a.txt").unwrap().meta().unwrap().id;
    // Another file takes the name.
    fs::rename(sandbox.path("work/a.txt"), sandbox.path("work/kept.txt")).unwrap();
    sandbox.file("work/a.txt", "another");
    assert!(matches!(work.claim("a.txt", approved), Err(Error::Changed)));
    assert!(matches!(
        work.claim("gone.txt", approved),
        Err(Error::Missing)
    ));
    assert_eq!(
        fs::read_to_string(sandbox.path("work/a.txt")).unwrap(),
        "another"
    );
    // Under its new name it is still the approved object.
    work.claim("kept.txt", approved).unwrap().remove().unwrap();
    assert_eq!(work.names().unwrap(), ["a.txt"]);
}

#[test]
fn removal_takes_one_entry_and_a_link_goes_as_a_link() {
    let sandbox = Sandbox::new("sys-remove");
    sandbox.file("work/full/file.txt", "x");
    sandbox.dir("work/empty");
    let read_only = sandbox.file("work/locked.txt", "x");
    let mut permissions = fs::metadata(&read_only).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&read_only, permissions).unwrap();
    let linked = folder_link(&sandbox.path("work/link"), &sandbox.outside);
    let work = fence(&sandbox).child("work").unwrap();
    let id = |name: &str| work.child(name).unwrap().meta().unwrap().id;

    // A folder with something in it stays.
    assert!(work.claim("full", id("full")).unwrap().remove().is_err());
    assert!(sandbox.path("work/full/file.txt").exists());

    work.claim("empty", id("empty")).unwrap().remove().unwrap();
    work.claim("locked.txt", id("locked.txt"))
        .unwrap()
        .remove()
        .unwrap();
    if linked {
        work.claim("link", id("link")).unwrap().remove().unwrap();
    }
    assert_eq!(work.names().unwrap(), ["full"]);
    assert!(sandbox.sentinel_is_intact());
    assert_eq!(common::tree(&sandbox.outside).len(), 1);
}

// ---- holding a folder in place ------------------------------------------------------------------

/// What the containment of a definitive delete stands on. An open folder is not kept where
/// it is by being open: only a handle that denies the right to delete does that, and only
/// Windows has one.
#[test]
fn a_folder_that_is_held_cannot_be_moved_away() {
    let sandbox = Sandbox::new("sys-hold");
    sandbox.file("work/a/sub/file.txt", "x");
    let away = sandbox.outside.join("taken");
    let work = fence(&sandbox).child("work").unwrap();
    let a = work.child("a").unwrap();
    let id = a.meta().unwrap().id;
    assert_eq!(sys::HOLDS, cfg!(windows));

    // Just open, with nothing open below it: another process takes it out of the fence.
    fs::rename(sandbox.path("work/a"), &away).unwrap();
    assert!(!work.holds("a", id).unwrap());
    fs::rename(&away, sandbox.path("work/a")).unwrap();
    assert!(work.holds("a", id).unwrap());

    let held = a.hold().unwrap();
    assert_eq!(held.meta().unwrap().id, id);
    let moved = fs::rename(sandbox.path("work/a"), &away).is_ok();
    if sys::HOLDS {
        // Held: not by a path, not by its own handle, and not by moving a folder above it.
        assert!(!moved, "a held folder was moved");
        assert!(work.claim("a", id).is_err());
        assert!(fs::rename(sandbox.path("work"), sandbox.outside.join("work")).is_err());
        // What is in it still comes and goes.
        let sub = held.child("sub").unwrap();
        let file = sub.child("file.txt").unwrap().meta().unwrap().id;
        sub.claim("file.txt", file).unwrap().remove().unwrap();
        assert!(work.holds("a", id).unwrap());
        // Let go, it can be moved again.
        drop(sub);
        drop(held);
        fs::rename(sandbox.path("work/a"), &away).unwrap();
    } else {
        // Here nothing holds it: the handle follows the folder out of the fence.
        assert!(moved);
        assert!(!work.holds("a", id).unwrap());
        assert_eq!(held.meta().unwrap().id, id);
        assert_eq!(held.names().unwrap(), ["sub"]);
    }
    assert!(sandbox.sentinel_is_intact());
}
