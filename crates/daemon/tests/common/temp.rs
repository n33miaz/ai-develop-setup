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

/// A folder of one test inside [`directory`], removed with everything in it when dropped,
/// also on a panic. Declare it before the processes that use it, so they stop first.
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
            return;
        };
        if path.parent() != Some(root.as_path()) {
            return;
        }
        if let Some(files) = self.files {
            for file in files {
                let _ = std::fs::remove_file(path.join(file));
            }
            let _ = std::fs::remove_dir(&path);
        } else {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}
