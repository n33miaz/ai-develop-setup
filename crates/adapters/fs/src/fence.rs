//! The fence: the one folder everything the executor touches has to be physically inside.
//!
//! A path of a plan is text. Here it becomes a list of names below the fence, and every name is
//! looked at on disk, one by one, without following a link. The answer is never trusted for
//! longer than one operation: whoever mutates asks again right before.

use std::fs::{self, Metadata};
use std::path::{Component, Path, PathBuf};

use ads_core::gate::{AbsPath, PathStyle};

use crate::reason::{Reason, Stop};

/// What a name is on disk, as seen without following it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
    /// A symbolic link, a junction or any other reparse point. `dir` says which system call
    /// removes the link itself (on Windows a folder link goes away as a folder).
    Link {
        dir: bool,
    },
}

/// A path split into the volume it starts at and its names.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Parts {
    /// `C:` on Windows, empty elsewhere.
    anchor: String,
    names: Vec<String>,
}

fn same_name(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

pub(crate) fn same_names(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_name(a, b))
}

/// A name that can only mean one entry of one folder.
pub(crate) fn check_name(name: &str) -> Result<(), Stop> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
        || (cfg!(windows) && name.contains(':'));
    if bad {
        return Err(Stop::new(
            Reason::NotPhysical,
            format!("`{name}` is not a plain name"),
        ));
    }
    Ok(())
}

/// The parts of a path the operating system resolved (the output of `canonicalize`).
fn parts_of_resolved(path: &Path) -> Result<Parts, Stop> {
    let odd = || {
        Stop::new(
            Reason::NotPhysical,
            format!("{} is not a local absolute path", path.display()),
        )
    };
    let mut anchor = String::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                use std::path::Prefix;
                match prefix.kind() {
                    Prefix::VerbatimDisk(letter) | Prefix::Disk(letter) => {
                        anchor = format!("{}:", char::from(letter.to_ascii_uppercase()));
                    }
                    _ => return Err(odd()),
                }
            }
            Component::RootDir => {}
            Component::Normal(name) => names.push(name.to_str().ok_or_else(odd)?.to_owned()),
            Component::CurDir | Component::ParentDir => return Err(odd()),
        }
    }
    if cfg!(windows) == anchor.is_empty() {
        return Err(odd());
    }
    Ok(Parts { anchor, names })
}

/// The parts of a path of a plan. The core already normalized it; nothing is fixed here.
fn parts_of_text(path: &AbsPath) -> Result<Parts, Stop> {
    let text = path.to_string();
    let native = match path.style() {
        PathStyle::Windows => cfg!(windows),
        PathStyle::Posix => !cfg!(windows),
    };
    if !native {
        return Err(Stop::new(
            Reason::PathStyle,
            format!("{text} is not a path of this system"),
        ));
    }
    let (anchor, rest, separator) = match path.style() {
        PathStyle::Windows => {
            let (drive, rest) = text.split_at_checked(2).unwrap_or((&text, ""));
            (drive.to_ascii_uppercase(), rest, '\\')
        }
        PathStyle::Posix => (String::new(), text.as_str(), '/'),
    };
    let names: Vec<String> = rest
        .split(separator)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    for name in &names {
        check_name(name)?;
    }
    Ok(Parts { anchor, names })
}

#[cfg(windows)]
pub(crate) fn kind_of(metadata: &Metadata) -> Option<Kind> {
    use std::os::windows::fs::MetadataExt;
    const DIRECTORY: u32 = 0x10;
    const REPARSE_POINT: u32 = 0x400;
    let attributes = metadata.file_attributes();
    if metadata.file_type().is_symlink() || attributes & REPARSE_POINT != 0 {
        Some(Kind::Link {
            dir: attributes & DIRECTORY != 0,
        })
    } else if metadata.is_dir() {
        Some(Kind::Dir)
    } else if metadata.is_file() {
        Some(Kind::File)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub(crate) fn kind_of(metadata: &Metadata) -> Option<Kind> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        Some(Kind::Link { dir: false })
    } else if file_type.is_dir() {
        Some(Kind::Dir)
    } else if file_type.is_file() {
        Some(Kind::File)
    } else {
        None
    }
}

#[cfg(unix)]
fn volume_of(metadata: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.dev()
}

// On Windows another volume mounted in a folder is a reparse point, already seen as a link.
#[cfg(not(unix))]
fn volume_of(_metadata: &Metadata) -> u64 {
    0
}

/// Why a folder cannot be a fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceError {
    /// The folder does not exist or cannot be resolved.
    Unresolved(String),
    NotADirectory,
    /// A drive root or `/`: a fence that fences nothing.
    FilesystemRoot,
    /// A network path, a device path or a name that is not text.
    NotLocal,
}

impl std::fmt::Display for FenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unresolved(error) => write!(f, "the fence cannot be resolved: {error}"),
            Self::NotADirectory => f.write_str("the fence is not a folder"),
            Self::FilesystemRoot => f.write_str("the fence is the root of a file system"),
            Self::NotLocal => f.write_str("the fence is not a local path"),
        }
    }
}

/// A resolved base folder. Paths are named by their names below it (`rel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fence {
    /// As the operating system resolved it; every path used for I/O grows from here.
    root: PathBuf,
    parts: Parts,
    volume: u64,
}

impl Fence {
    /// Resolves `base` once, links included: this is the only path taken on trust.
    pub fn new(base: &Path) -> Result<Self, FenceError> {
        let root =
            fs::canonicalize(base).map_err(|error| FenceError::Unresolved(error.to_string()))?;
        let metadata = fs::symlink_metadata(&root)
            .map_err(|error| FenceError::Unresolved(error.to_string()))?;
        if kind_of(&metadata) != Some(Kind::Dir) {
            return Err(FenceError::NotADirectory);
        }
        let volume = volume_of(&metadata);
        Self::from_resolved(root, volume)
    }

    fn from_resolved(root: PathBuf, volume: u64) -> Result<Self, FenceError> {
        let parts = parts_of_resolved(&root).map_err(|_| FenceError::NotLocal)?;
        if parts.names.is_empty() {
            return Err(FenceError::FilesystemRoot);
        }
        Ok(Self {
            root,
            parts,
            volume,
        })
    }

    /// The fence as a plan writes it (`C:\work`, `/work`).
    pub fn text(&self) -> String {
        self.text_of(&[])
    }

    /// The path of `rel` as a plan writes it.
    pub(crate) fn text_of(&self, rel: &[String]) -> String {
        let separator = if cfg!(windows) { "\\" } else { "/" };
        let mut text = self.parts.anchor.clone();
        for name in self.parts.names.iter().chain(rel) {
            text.push_str(separator);
            text.push_str(name);
        }
        text
    }

    /// Where `rel` would be, to name it to a probe. Checks nothing: never used for I/O.
    pub(crate) fn path_of(&self, rel: &[String]) -> PathBuf {
        let mut path = self.root.clone();
        path.extend(rel);
        path
    }

    /// The names of `path` below the fence; empty when it is the fence itself. By text only.
    pub(crate) fn relative(&self, path: &AbsPath) -> Result<Vec<String>, Stop> {
        let parts = parts_of_text(path)?;
        let depth = self.parts.names.len();
        let inside = parts.anchor == self.parts.anchor
            && parts.names.len() >= depth
            && same_names(&parts.names[..depth], &self.parts.names);
        if !inside {
            return Err(Stop::new(
                Reason::OutsideFence,
                format!("{path} is not inside the fence"),
            ));
        }
        Ok(parts.names[depth..].to_vec())
    }

    /// Looks at `rel` on disk, name by name from the fence, following nothing. `None` when the
    /// last name does not exist (everything before it has to). A link is accepted only as the
    /// last name, and is reported as a link: what it points to is never looked at.
    pub(crate) fn inspect(&self, rel: &[String]) -> Result<(PathBuf, Option<Kind>), Stop> {
        let mut path = self.root.clone();
        let mut found = Some(Kind::Dir);
        for (at, name) in rel.iter().enumerate() {
            check_name(name)?;
            path.push(name);
            let last = at + 1 == rel.len();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if last {
                        found = None;
                        break;
                    }
                    return Err(Stop::new(
                        Reason::Missing,
                        format!("{} does not exist", path.display()),
                    ));
                }
                Err(error) => return Err(Stop::io("looking at", &path, &error)),
            };
            let Some(kind) = kind_of(&metadata) else {
                return Err(Stop::new(
                    Reason::SpecialFile,
                    format!("{} is not a file, a folder or a link", path.display()),
                ));
            };
            match kind {
                Kind::Link { .. } if !last => {
                    return Err(Stop::new(
                        Reason::LinkInPath,
                        format!("{} is a link", path.display()),
                    ));
                }
                Kind::File if !last => {
                    return Err(Stop::new(
                        Reason::NotADirectory,
                        format!("{} is not a folder", path.display()),
                    ));
                }
                Kind::Link { .. } => {}
                Kind::Dir | Kind::File => {
                    if volume_of(&metadata) != self.volume {
                        return Err(Stop::new(
                            Reason::OtherVolume,
                            format!("{} is on another volume", path.display()),
                        ));
                    }
                }
            }
            found = Some(kind);
        }

        // The deepest part that is neither missing nor a link has to resolve to itself.
        let solid = match found {
            Some(Kind::Dir | Kind::File) => rel.len(),
            Some(Kind::Link { .. }) | None => rel.len().saturating_sub(1),
        };
        let mut solid_path = self.root.clone();
        solid_path.extend(&rel[..solid]);
        let resolved = fs::canonicalize(&solid_path)
            .map_err(|error| Stop::io("resolving", &solid_path, &error))?;
        let resolved = parts_of_resolved(&resolved)?;
        let depth = self.parts.names.len();
        let same = resolved.anchor == self.parts.anchor
            && resolved.names.len() == depth + solid
            && same_names(&resolved.names[..depth], &self.parts.names)
            && same_names(&resolved.names[depth..], &rel[..solid]);
        if !same {
            return Err(Stop::new(
                Reason::NotPhysical,
                format!("{} resolves to another path", solid_path.display()),
            ));
        }
        Ok((path, found))
    }

    /// Like [`Self::inspect`], for a path that has to exist.
    pub(crate) fn existing(&self, rel: &[String]) -> Result<(PathBuf, Kind), Stop> {
        match self.inspect(rel)? {
            (path, Some(kind)) => Ok((path, kind)),
            (path, None) => Err(Stop::new(
                Reason::Missing,
                format!("{} does not exist", path.display()),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure: nothing here looks at the disk.

    #[cfg(windows)]
    const ROOT: &str = "\\\\?\\C:\\";
    #[cfg(windows)]
    const INSIDE: &str = "\\\\?\\C:\\base\\fence";
    #[cfg(not(windows))]
    const ROOT: &str = "/";
    #[cfg(not(windows))]
    const INSIDE: &str = "/base/fence";

    #[test]
    fn a_file_system_root_is_not_a_fence() {
        assert_eq!(
            Fence::from_resolved(PathBuf::from(ROOT), 0),
            Err(FenceError::FilesystemRoot)
        );
        assert!(Fence::from_resolved(PathBuf::from(INSIDE), 0).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn a_network_path_is_not_a_fence() {
        let unc = PathBuf::from("\\\\?\\UNC\\server\\share\\folder");
        assert_eq!(Fence::from_resolved(unc, 0), Err(FenceError::NotLocal));
    }

    #[test]
    fn containment_is_by_name_not_by_prefix_of_text() {
        let fence = Fence::from_resolved(PathBuf::from(INSIDE), 0).unwrap();
        let text = fence.text();
        let path = |text: &str| AbsPath::parse(text).unwrap();
        assert_eq!(fence.relative(&path(&text)).unwrap(), Vec::<String>::new());
        let separator = if cfg!(windows) { "\\" } else { "/" };
        let child = format!("{text}{separator}a{separator}b");
        assert_eq!(fence.relative(&path(&child)).unwrap(), ["a", "b"]);
        let sibling = format!("{text}2{separator}a");
        assert_eq!(
            fence.relative(&path(&sibling)).unwrap_err().reason,
            Reason::OutsideFence
        );
        let parent = text.rsplit_once(separator).unwrap().0.to_owned();
        assert_eq!(
            fence.relative(&path(&parent)).unwrap_err().reason,
            Reason::OutsideFence
        );
    }

    #[test]
    fn a_path_of_the_other_system_is_refused() {
        let fence = Fence::from_resolved(PathBuf::from(INSIDE), 0).unwrap();
        let other = if cfg!(windows) {
            "/base/fence/a"
        } else {
            "C:\\base\\fence\\a"
        };
        let error = fence.relative(&AbsPath::parse(other).unwrap()).unwrap_err();
        assert_eq!(error.reason, Reason::PathStyle);
    }

    #[test]
    fn names_that_are_not_plain_names_are_refused() {
        for name in ["", ".", "..", "a/b", "a\\b"] {
            assert!(check_name(name).is_err(), "{name}");
        }
        assert!(check_name("a..b").is_ok());
    }
}
