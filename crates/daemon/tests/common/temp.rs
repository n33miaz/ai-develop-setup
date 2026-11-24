use std::path::{Path, PathBuf};

pub fn directory() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join(".test-tmp")
        .join("daemon");
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// A folder of one test inside [`directory`], removed when dropped, also on a panic.
///
/// In [`Scratch::new`], the folder and its entire contents are removed recursively on drop.
/// In [`Scratch::with_files`], only the explicitly named files are removed individually
/// before removing the empty folder, enforcing non-recursive cleanup.
///
/// Declare it before the processes that use it, so they stop first.
#[allow(
    dead_code,
    reason = "not every test target that shares this module uses it"
)]
pub struct Scratch {
    path: PathBuf,
    files: Option<&'static [&'static str]>,
}

#[allow(
    dead_code,
    reason = "not every test target that shares this module uses it"
)]
impl Scratch {
    /// `name` must be unique to the run: one plain folder name, no separators.
    pub fn new(name: &str) -> Self {
        Self::create(name, None)
    }

    /// Remove only the named files and the empty folder on drop.
    pub fn with_files(name: &str, files: &'static [&'static str]) -> Self {
        for file in files {
            Self::assert_plain_name(file);
        }
        Self::create(name, Some(files))
    }

    fn create(name: &str, files: Option<&'static [&'static str]>) -> Self {
        Self::assert_plain_name(name);
        let path = directory().join(name);
        std::fs::create_dir(&path).unwrap();
        Self { path, files }
    }

    fn assert_plain_name(name: &str) {
        assert!(
            !name.is_empty() && !name.contains(['/', '\\', ':']) && name != "." && name != "..",
            "{name}"
        );
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Only ever a folder strictly inside the test directory, resolved on disk.
        let (Ok(root), Ok(path)) = (directory().canonicalize(), self.path.canonicalize()) else {
            if !std::thread::panicking() && self.path.try_exists().unwrap_or(false) {
                panic!(
                    "scratch folder {:?} exists but could not be canonicalized for cleanup",
                    self.path
                );
            }
            return;
        };
        if path.parent() != Some(root.as_path()) {
            if !std::thread::panicking() {
                panic!("scratch folder {:?} is outside {:?}", path, root);
            }
            return;
        }
        let mut cleanup_error = None;
        if let Some(files) = self.files {
            for file in files {
                let file_path = path.join(file);
                if let Err(error) = std::fs::remove_file(&file_path) {
                    if error.kind() != std::io::ErrorKind::NotFound && cleanup_error.is_none() {
                        cleanup_error = Some(error);
                    }
                }
            }
            if let Err(error) = std::fs::remove_dir(&path) {
                if error.kind() != std::io::ErrorKind::NotFound && cleanup_error.is_none() {
                    cleanup_error = Some(error);
                }
            }
        } else if let Err(error) = std::fs::remove_dir_all(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                cleanup_error = Some(error);
            }
        }

        if !std::thread::panicking() {
            if let Some(error) = cleanup_error {
                panic!("failed to remove scratch folder {:?}: {error}", self.path);
            }
            if self.path.try_exists().unwrap_or(false) {
                panic!("scratch folder unexpectedly remains: {:?}", self.path);
            }
        }
    }
}
