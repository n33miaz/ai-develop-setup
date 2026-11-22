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
pub struct Scratch(PathBuf);

#[allow(
    dead_code,
    reason = "not every test target that shares this module uses it"
)]
impl Scratch {
    /// `name` must be unique to the run: one plain folder name, no separators.
    pub fn new(name: &str) -> Self {
        assert!(
            !name.is_empty() && !name.contains(['/', '\\', ':']) && name != "." && name != "..",
            "{name}"
        );
        let path = directory().join(name);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Only ever a folder strictly inside the test directory, resolved on disk.
        let (Ok(root), Ok(path)) = (directory().canonicalize(), self.0.canonicalize()) else {
            return;
        };
        if path != root && path.parent() == Some(root.as_path()) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}
