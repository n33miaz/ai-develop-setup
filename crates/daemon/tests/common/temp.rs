use ads_private_dir::FixtureDir;
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
    original: Option<FixtureDir>,
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
        fs::create_dir(&cleanup_path).unwrap();
        let original = FixtureDir::open(&cleanup_path).unwrap();
        Self {
            path,
            cleanup_path,
            original: Some(original),
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
            !name.is_empty()
                && !name.contains(['/', '\\', ':', '\0'])
                && name != "."
                && name != "..",
            "{name}"
        );
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn cleanup(&mut self) -> Result<(), String> {
        let path = &self.cleanup_path;
        if !check_folder(path)? {
            return Ok(());
        }
        let parent = FixtureDir::open(path.parent().unwrap())
            .map_err(|error| format!("cannot open parent of scratch folder {path:?}: {error}"))?;
        let root = open_child(&parent, path.file_name().unwrap().to_str().unwrap(), path)?;
        let original = self.original.take().unwrap();
        if !root
            .same_directory(&original)
            .map_err(|error| format!("cannot identify scratch folder {path:?}: {error}"))?
        {
            return Err(format!("scratch folder {path:?} identity changed"));
        }
        drop(original);

        for &(relative, files) in self.subdirs {
            let subdir_path = path.join(relative);
            if !check_folder(&subdir_path)? {
                continue;
            }
            let mut components = relative.split('/');
            let name = components.next().unwrap();
            let mut child_path = path.join(name);
            let mut child = open_child(&root, name, &child_path)?;
            for name in components {
                child_path.push(name);
                child = open_child(&child, name, &child_path)?;
            }
            remove_folder(child, &subdir_path, files)?;
        }
        remove_folder(root, path, self.files)
    }
}

fn check_folder(path: &Path) -> Result<bool, String> {
    if let Err(error) = fs::symlink_metadata(path) {
        return match error.kind() {
            ErrorKind::NotFound => Ok(false),
            _ => Err(format!("cannot inspect scratch folder {path:?}: {error}")),
        };
    }
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("cannot resolve scratch folder {path:?}: {error}"))?;
    if resolved != path {
        return Err(format!("scratch folder {path:?} resolves to {resolved:?}"));
    }
    Ok(true)
}

fn open_child(parent: &FixtureDir, name: &str, path: &Path) -> Result<FixtureDir, String> {
    parent
        .child(name)
        .map_err(|error| format!("cannot open scratch folder {path:?}: {error}"))
}

fn remove_folder(folder: FixtureDir, path: &Path, files: &[&str]) -> Result<(), String> {
    for name in files {
        if let Err(error) = folder.remove_file(name)
            && error.kind() != ErrorKind::NotFound
        {
            let file = path.join(name);
            return Err(format!("cannot remove scratch file {file:?}: {error}"));
        }
    }
    folder
        .remove()
        .map_err(|error| format!("cannot remove scratch folder {path:?}: {error}"))
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
