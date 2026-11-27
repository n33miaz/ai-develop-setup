use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::FixtureDir;

/// A folder of one test inside the repository's `.test-tmp`, removed entry by entry on drop.
struct Temp {
    root: PathBuf,
    entries: &'static [&'static str],
}

impl Temp {
    /// `entries` are every relative path the test may leave, children before parents.
    fn new(label: &str, entries: &'static [&'static str]) -> Self {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join(".test-tmp")
            .join("private-dir");
        fs::create_dir_all(&base).unwrap();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = base.join(format!("{label}-{}-{stamp}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self { root, entries }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn dir(&self, relative: &str) {
        fs::create_dir(self.path(relative)).unwrap();
    }

    fn file(&self, relative: &str, contents: &str) {
        fs::write(self.path(relative), contents).unwrap();
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let paths = self.entries.iter().map(|relative| self.path(relative));
        for path in paths.chain([self.root.clone()]) {
            let result = fs::symlink_metadata(&path).and_then(|metadata| {
                if metadata.is_dir() {
                    fs::remove_dir(&path)
                } else {
                    fs::remove_file(&path)
                }
            });
            if let Err(error) = result
                && error.kind() != ErrorKind::NotFound
                && !std::thread::panicking()
            {
                panic!("cannot remove test entry {path:?}: {error}");
            }
        }
    }
}

#[cfg(unix)]
fn assert_identity_changed(result: std::io::Result<()>) {
    let error = result.expect_err("a replaced folder should be refused");
    assert_eq!(error.to_string(), "scratch folder identity changed");
}

#[test]
fn starting_directory_is_not_removed() {
    let temp = Temp::new("starting", &[]);

    let error = FixtureDir::open(&temp.root).unwrap().remove().unwrap_err();

    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(error.to_string(), "cannot remove the starting directory");
    assert!(temp.root.is_dir());
}

/// Identity is bound when a folder is opened: an earlier replacement counts as the declared one.
#[test]
fn subfolder_replaced_before_opening_loses_only_the_named_file() {
    let temp = Temp::new(
        "replaced-before",
        &[
            "nested/listed",
            "nested/unlisted",
            "nested",
            "original/listed",
            "original",
        ],
    );
    temp.dir("nested");
    temp.file("nested/listed", "original");
    fs::rename(temp.path("nested"), temp.path("original")).unwrap();
    temp.dir("nested");
    temp.file("nested/listed", "replacement");
    temp.file("nested/unlisted", "unlisted");
    let root = FixtureDir::open(&temp.root).unwrap();
    let nested = root.child("nested").unwrap();

    nested.remove_file("listed").unwrap();
    let error = nested.remove().unwrap_err();

    #[cfg(unix)]
    assert_eq!(error.kind(), ErrorKind::DirectoryNotEmpty);
    // STATUS_DIRECTORY_NOT_EMPTY, which has no error kind of its own here.
    #[cfg(windows)]
    assert_eq!(error.to_string(), "the system answered 0xc0000101");
    assert!(!temp.path("nested/listed").exists());
    assert_eq!(temp.read("nested/unlisted"), "unlisted");
    assert_eq!(temp.read("original/listed"), "original");
}

#[cfg(windows)]
#[test]
fn open_subfolder_cannot_be_replaced() {
    const SHARING_VIOLATION: i32 = 32;
    const ACCESS_DENIED: i32 = 5;
    let temp = Temp::new(
        "held",
        &["nested/listed", "nested", "original/listed", "original"],
    );
    temp.dir("nested");
    temp.file("nested/listed", "original");
    let root = FixtureDir::open(&temp.root).unwrap();
    let nested = root.child("nested").unwrap();

    let error = fs::rename(temp.path("nested"), temp.path("original")).unwrap_err();

    assert!(
        matches!(
            error.raw_os_error(),
            Some(SHARING_VIOLATION | ACCESS_DENIED)
        ),
        "{error}"
    );
    assert_eq!(temp.read("nested/listed"), "original");
    assert!(!temp.path("original").exists());
    nested.remove_file("listed").unwrap();
    nested.remove().unwrap();
    assert!(!temp.path("nested").exists());
}

#[cfg(unix)]
#[test]
fn subfolder_replaced_while_open_is_rejected_and_preserved() {
    let temp = Temp::new(
        "replaced-open",
        &["nested/listed", "nested", "original/listed", "original"],
    );
    temp.dir("nested");
    temp.file("nested/listed", "original");
    let root = FixtureDir::open(&temp.root).unwrap();
    let nested = root.child("nested").unwrap();
    fs::rename(temp.path("nested"), temp.path("original")).unwrap();
    temp.dir("nested");
    temp.file("nested/listed", "replacement");

    assert_identity_changed(nested.remove_file("listed"));
    assert_eq!(temp.read("nested/listed"), "replacement");
    assert_eq!(temp.read("original/listed"), "original");

    // Emptied so that only the identity check keeps the replacement.
    fs::remove_file(temp.path("nested/listed")).unwrap();
    assert_identity_changed(nested.remove());
    assert!(temp.path("nested").is_dir());
    assert_eq!(temp.read("original/listed"), "original");
}

#[cfg(unix)]
#[test]
fn renamed_ancestor_refuses_removal() {
    let temp = Temp::new(
        "ancestor",
        &[
            "outer/inner",
            "outer",
            "moved/inner/listed",
            "moved/inner",
            "moved",
        ],
    );
    temp.dir("outer");
    temp.dir("outer/inner");
    temp.file("outer/inner/listed", "original");
    let root = FixtureDir::open(&temp.root).unwrap();
    let inner = root.child("outer").unwrap().child("inner").unwrap();
    fs::rename(temp.path("outer"), temp.path("moved")).unwrap();

    // The old name is first missing, then taken by another folder.
    assert_identity_changed(inner.remove_file("listed"));
    temp.dir("outer");
    temp.dir("outer/inner");
    assert_identity_changed(inner.remove_file("listed"));
    assert_eq!(temp.read("moved/inner/listed"), "original");

    // Emptied so that only the ancestor check keeps the moved folder.
    fs::remove_file(temp.path("moved/inner/listed")).unwrap();
    assert_identity_changed(inner.remove());
    assert!(temp.path("moved/inner").is_dir());
    assert!(temp.path("outer/inner").is_dir());
}

/// Calls the platform step directly: the swap happens after the location check.
#[cfg(unix)]
#[test]
fn link_count_detects_that_another_folder_was_removed() {
    use super::platform;

    let temp = Temp::new("link-count", &["child", "moved"]);
    temp.dir("child");
    let parent = platform::open_dir(&temp.root).unwrap();
    let child = platform::open_child_dir(&parent, "child").unwrap();
    fs::rename(temp.path("child"), temp.path("moved")).unwrap();
    temp.dir("child");

    assert_identity_changed(platform::remove_child_dir(&parent, "child", &child));

    // Detection, not prevention: the empty folder that took the name is gone.
    assert!(!temp.path("child").exists());
    assert!(temp.path("moved").is_dir());

    platform::remove_child_dir(&parent, "moved", &child).unwrap();
    assert!(!temp.path("moved").exists());
}
