//! The fence: the one folder everything the executor touches has to be physically inside.
//!
//! The fence is opened once and stays open. A path of a plan is text: here it becomes a list
//! of names below the fence, and each name is opened relative to the folder opened before it,
//! never following a link. What comes back is the open entry and its identity, not a path:
//! whoever mutates does it through what is open (see `sys`).

use std::fs;
use std::path::{Component, Path, PathBuf};

use ads_core::gate::{AbsPath, PathStyle};

use crate::reason::{Reason, Stop};
pub use crate::sys::Kind;
use crate::sys::{self, Crossing, Meta, Node};

/// A path split into the volume it starts at and its names.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Parts {
    /// `C:` on Windows, empty elsewhere.
    anchor: String,
    names: Vec<String>,
}

/// A name that can only mean one entry of one folder.
pub(crate) fn check_name(name: &str) -> Result<(), Stop> {
    sys::plain(name).map_err(|error| Stop::new(Reason::NotPhysical, error))
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
    /// This system lacks the calls the executor needs; nothing runs on it.
    UnsupportedSystem,
}

impl std::fmt::Display for FenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unresolved(error) => write!(f, "the fence cannot be resolved: {error}"),
            Self::NotADirectory => f.write_str("the fence is not a folder"),
            Self::FilesystemRoot => f.write_str("the fence is the root of a file system"),
            Self::NotLocal => f.write_str("the fence is not a local path"),
            Self::UnsupportedSystem => f.write_str("this system is not supported"),
        }
    }
}

/// The fence as text: how a path of a plan is told to be inside it, and written back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    /// As the operating system resolved it. Opened once; afterwards only shown to people.
    root: PathBuf,
    parts: Parts,
}

impl Frame {
    fn from_resolved(root: PathBuf) -> Result<Self, FenceError> {
        let parts = parts_of_resolved(&root).map_err(|_| FenceError::NotLocal)?;
        if parts.names.is_empty() {
            return Err(FenceError::FilesystemRoot);
        }
        Ok(Self { root, parts })
    }

    /// The path of `rel` as a plan writes it.
    fn text_of(&self, rel: &[String]) -> String {
        let separator = if cfg!(windows) { "\\" } else { "/" };
        let mut text = self.parts.anchor.clone();
        for name in self.parts.names.iter().chain(rel) {
            text.push_str(separator);
            text.push_str(name);
        }
        text
    }

    /// The names of `path` below the fence; empty when it is the fence itself. By text, and
    /// letter by letter: another case of a name of the fence may be another folder.
    fn relative(&self, path: &AbsPath) -> Result<Vec<String>, Stop> {
        let parts = parts_of_text(path)?;
        let depth = self.parts.names.len();
        let inside = parts.anchor == self.parts.anchor
            && parts.names.len() >= depth
            && parts.names[..depth] == self.parts.names[..];
        if !inside {
            return Err(Stop::new(
                Reason::OutsideFence,
                format!("{path} is not inside the fence"),
            ));
        }
        Ok(parts.names[depth..].to_vec())
    }
}

/// An entry that exists, open, with what its own handle says about it.
#[derive(Debug)]
pub(crate) struct Entry {
    pub node: Node,
    pub meta: Meta,
    pub kind: Kind,
}

/// Where a path ends: its folder, open, and the entry under the last name, if there is one.
#[derive(Debug)]
pub(crate) struct Spot {
    pub parent: Node,
    pub name: String,
    pub entry: Option<Entry>,
}

/// The base folder, open. Paths are named by their names below it (`rel`).
#[derive(Debug)]
pub struct Fence {
    frame: Frame,
    root: Node,
    home: Meta,
}

impl Fence {
    /// Resolves `base` once, links included, and opens it: this is the only path taken on
    /// trust, and the only one ever given to the operating system.
    pub fn new(base: &Path) -> Result<Self, FenceError> {
        let unresolved = |error: std::io::Error| match error.kind() {
            std::io::ErrorKind::Unsupported => FenceError::UnsupportedSystem,
            _ => FenceError::Unresolved(error.to_string()),
        };
        let resolved = fs::canonicalize(base).map_err(unresolved)?;
        let frame = Frame::from_resolved(resolved)?;
        let root = Node::root(&frame.root).map_err(unresolved)?;
        let home = root.meta().map_err(unresolved)?;
        if home.kind != Some(Kind::Dir) {
            return Err(FenceError::NotADirectory);
        }
        Ok(Self { frame, root, home })
    }

    /// The fence as a plan writes it (`C:\work`, `/work`).
    pub fn text(&self) -> String {
        self.frame.text_of(&[])
    }

    pub(crate) fn text_of(&self, rel: &[String]) -> String {
        self.frame.text_of(rel)
    }

    /// Where `rel` would be, to name it to a probe. Never used for I/O.
    pub(crate) fn path_of(&self, rel: &[String]) -> PathBuf {
        let mut path = self.frame.root.clone();
        path.extend(rel);
        path
    }

    pub(crate) fn relative(&self, path: &AbsPath) -> Result<Vec<String>, Stop> {
        self.frame.relative(path)
    }

    /// What an open entry below the fence is. Refuses what is not a file, a folder or a link,
    /// what is on another volume, and what is on another mount than the fence (a bind mount
    /// of the same device too); a system that does not identify mounts proves nothing.
    pub(crate) fn admit(&self, rel: &[String], meta: &Meta) -> Result<Kind, Stop> {
        let Some(kind) = meta.kind else {
            return Err(Stop::new(
                Reason::SpecialFile,
                format!("{} is not a file, a folder or a link", self.text_of(rel)),
            ));
        };
        if meta.id.volume != self.home.id.volume {
            return Err(Stop::new(
                Reason::OtherVolume,
                format!("{} is on another volume", self.text_of(rel)),
            ));
        }
        match sys::crossing(&self.home.mount, &meta.mount) {
            Crossing::Same => Ok(kind),
            Crossing::Crossed => Err(Stop::new(
                Reason::MountPoint,
                format!("{} is on another mount", self.text_of(rel)),
            )),
            Crossing::Unknown => Err(Stop::new(
                Reason::MountPoint,
                format!(
                    "the system does not say which mount {} is on",
                    self.text_of(rel)
                ),
            )),
        }
    }

    /// Opens the last name of `rel` in its open folder. `None` when it does not exist.
    fn step(&self, parent: &Node, rel: &[String]) -> Result<Option<Entry>, Stop> {
        let name = rel.last().expect("a step has a name");
        check_name(name)?;
        let node = match parent.child(name) {
            Ok(node) => node,
            Err(sys::Error::Missing) => return Ok(None),
            Err(sys::Error::Alias) => {
                return Err(Stop::new(
                    Reason::NotPhysical,
                    format!("{} is another spelling of an entry", self.text_of(rel)),
                ));
            }
            Err(error) => return Err(Stop::io("opening", self.text_of(rel), error)),
        };
        let meta = node
            .meta()
            .map_err(|error| Stop::io("looking at", self.text_of(rel), error))?;
        let kind = self.admit(rel, &meta)?;
        Ok(Some(Entry { node, meta, kind }))
    }

    /// Opens the folder `rel`, name by name from the fence. Every name has to be a folder:
    /// a link on the way is refused, never followed.
    pub(crate) fn open_dir(&self, rel: &[String]) -> Result<Node, Stop> {
        let mut node = self
            .root
            .again()
            .map_err(|error| Stop::io("opening", self.text(), error))?;
        for depth in 1..=rel.len() {
            let upto = &rel[..depth];
            let Some(entry) = self.step(&node, upto)? else {
                return Err(Stop::new(
                    Reason::Missing,
                    format!("{} does not exist", self.text_of(upto)),
                ));
            };
            match entry.kind {
                Kind::Dir => node = entry.node,
                Kind::Link => {
                    return Err(Stop::new(
                        Reason::LinkInPath,
                        format!("{} is a link", self.text_of(upto)),
                    ));
                }
                Kind::File => {
                    return Err(Stop::new(
                        Reason::NotADirectory,
                        format!("{} is not a folder", self.text_of(upto)),
                    ));
                }
            }
        }
        Ok(node)
    }

    /// Opens the folder of `rel` and looks at its last name there, following nothing. A link
    /// is accepted only as the last name, and is opened as a link: what it points to is never
    /// looked at.
    pub(crate) fn locate(&self, rel: &[String]) -> Result<Spot, Stop> {
        let Some((name, folder)) = rel.split_last() else {
            return Err(Stop::new(
                Reason::NotPhysical,
                "the fence itself is not an entry",
            ));
        };
        let parent = self.open_dir(folder)?;
        let entry = self.step(&parent, rel)?;
        Ok(Spot {
            parent,
            name: name.clone(),
            entry,
        })
    }

    /// Like [`Self::locate`], for an entry that has to exist.
    pub(crate) fn existing(&self, rel: &[String]) -> Result<(Spot, Entry), Stop> {
        let mut spot = self.locate(rel)?;
        match spot.entry.take() {
            Some(entry) => Ok((spot, entry)),
            None => Err(Stop::new(
                Reason::Missing,
                format!("{} does not exist", self.text_of(rel)),
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
    const INSIDE: &str = "\\\\?\\C:\\base\\Fence";
    #[cfg(not(windows))]
    const ROOT: &str = "/";
    #[cfg(not(windows))]
    const INSIDE: &str = "/base/Fence";

    fn frame() -> Frame {
        Frame::from_resolved(PathBuf::from(INSIDE)).unwrap()
    }

    #[test]
    fn a_file_system_root_is_not_a_fence() {
        assert_eq!(
            Frame::from_resolved(PathBuf::from(ROOT)),
            Err(FenceError::FilesystemRoot)
        );
        assert!(Frame::from_resolved(PathBuf::from(INSIDE)).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn a_network_path_is_not_a_fence() {
        let unc = PathBuf::from("\\\\?\\UNC\\server\\share\\folder");
        assert_eq!(Frame::from_resolved(unc), Err(FenceError::NotLocal));
    }

    #[test]
    fn containment_is_by_name_not_by_prefix_of_text() {
        let fence = frame();
        let text = fence.text_of(&[]);
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

    /// On a folder that tells `Fence` from `fence`, the other case is another tree. The
    /// spelling of the fence is the only one that is the fence, on every system.
    #[test]
    fn another_case_of_the_fence_is_not_the_fence() {
        let fence = frame();
        let text = fence.text_of(&[]);
        let separator = if cfg!(windows) { "\\" } else { "/" };
        let other = text.replace("Fence", "fence");
        assert_ne!(other, text);
        let inside = format!("{other}{separator}work{separator}file");
        assert_eq!(
            fence
                .relative(&AbsPath::parse(&inside).unwrap())
                .unwrap_err()
                .reason,
            Reason::OutsideFence
        );
        let exact = format!("{text}{separator}work{separator}file");
        assert!(fence.relative(&AbsPath::parse(&exact).unwrap()).is_ok());
    }

    #[test]
    fn a_path_of_the_other_system_is_refused() {
        let fence = frame();
        let other = if cfg!(windows) {
            "/base/Fence/a"
        } else {
            "C:\\base\\Fence\\a"
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
