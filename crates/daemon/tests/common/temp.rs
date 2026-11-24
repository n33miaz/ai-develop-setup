use std::fs;
use std::io::ErrorKind;
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
/// Declare it before the processes that use it, so they stop first.
#[allow(
    dead_code,
    reason = "not every test target that shares this module uses it"
)]
pub struct Scratch {
    path: PathBuf,
    cleanup_path: PathBuf,
    files: &'static [&'static str],
    subdirs: &'static [(&'static str, &'static [&'static str])],
}

#[allow(
    dead_code,
    reason = "not every test target that shares this module uses it"
)]
impl Scratch {
    /// `name` must be unique to the run: one plain folder name, no separators.
    pub fn with_files(name: &str, files: &'static [&'static str]) -> Self {
        Self::assert_plain_name(name);
        for file in files {
            Self::assert_plain_name(file);
        }
        let root = directory();
        let cleanup_path = root.canonicalize().unwrap().join(name);
        let path = root.join(name);
        fs::create_dir(&path).unwrap();
        Self {
            path,
            cleanup_path,
            files,
            subdirs: &[],
        }
    }

    /// List child folders before their parents so each is empty when removed.
    pub fn with_subdirs(mut self, subdirs: &'static [(&str, &[&str])]) -> Self {
        for &(relative, files) in subdirs {
            for name in relative.split('/') {
                Self::assert_plain_name(name);
            }
            for file in files {
                Self::assert_plain_name(file);
            }
        }
        self.subdirs = subdirs;
        self
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

    fn cleanup(&self) -> Result<(), String> {
        for &(relative, files) in self.subdirs {
            remove_folder(&self.cleanup_path.join(relative), files)?;
        }
        remove_folder(&self.cleanup_path, self.files)
    }
}

fn remove_folder(path: &Path, files: &[&str]) -> Result<(), String> {
    if let Err(error) = fs::symlink_metadata(path) {
        return match error.kind() {
            ErrorKind::NotFound => Ok(()),
            _ => Err(format!("cannot inspect scratch folder {path:?}: {error}")),
        };
    }
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("cannot resolve scratch folder {path:?}: {error}"))?;
    if resolved != path {
        return Err(format!("scratch folder {path:?} resolves to {resolved:?}"));
    }
    for name in files {
        let file = path.join(name);
        if let Err(error) = fs::remove_file(&file)
            && error.kind() != ErrorKind::NotFound
        {
            return Err(format!("cannot remove scratch file {file:?}: {error}"));
        }
    }
    fs::remove_dir(path).map_err(|error| format!("cannot remove scratch folder {path:?}: {error}"))
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            if std::thread::panicking() {
                eprintln!("{error}");
            } else {
                panic!("{error}");
            }
        }
    }
}
