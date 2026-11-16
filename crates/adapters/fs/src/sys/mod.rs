//! The system calls, by open handle. Nothing above this module gives a path to the operating
//! system: a [`Node`] is an entry that is already open, a child is opened by one plain name
//! relative to its open folder, and no link is ever followed. What an operation reaches is
//! decided by what is open, not by how a path is spelled.
//!
//! The only path taken is the one of [`Node::root`], once, for the fence.

use std::io;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Claim, Node};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{Claim, Node};

#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", windows)))]
pub use unsupported::{Claim, Node};

/// What an entry is, as seen on its own handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
    /// A symbolic link, a junction or any other reparse point; never entered.
    Link,
}

/// Which object an entry is: the device and inode, or the volume and file id on Windows. Two
/// spellings of one object have one identity; another object under the same name has another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Identity {
    pub volume: u64,
    pub file: u128,
}

/// What the system says about the mount an entry is on. `None` is "it did not say".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mount {
    pub id: Option<u64>,
    /// The entry is the root of a mount.
    pub root: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meta {
    /// `None` for a device, a socket or a pipe.
    pub kind: Option<Kind>,
    pub id: Identity,
    /// Bytes of a regular file.
    pub size: u64,
    pub mount: Mount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crossing {
    Same,
    Crossed,
    /// The system gave no mount identification: nothing is proved.
    Unknown,
}

/// Whether an entry below the fence is on another mount than the fence. A bind mount of the
/// same device has its own mount id and is the root of its mount.
pub fn crossing(fence: &Mount, entry: &Mount) -> Crossing {
    match (fence.id, entry.id, entry.root) {
        (Some(_), Some(_), Some(true)) => Crossing::Crossed,
        (Some(home), Some(id), Some(false)) if home == id => Crossing::Same,
        (Some(_), Some(_), Some(false)) => Crossing::Crossed,
        _ => Crossing::Unknown,
    }
}

#[derive(Debug)]
pub enum Error {
    Missing,
    /// The name is taken.
    Exists,
    /// The entry under the name is not the object that was checked.
    Changed,
    /// The name reaches an entry the folder lists under another spelling.
    #[cfg_attr(not(windows), allow(dead_code))]
    Alias,
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => Self::Missing,
            io::ErrorKind::AlreadyExists => Self::Exists,
            _ => Self::Io(error),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => f.write_str("it does not exist"),
            Self::Exists => f.write_str("the name is taken"),
            Self::Changed => f.write_str("it is not the entry that was checked"),
            Self::Alias => f.write_str("the folder lists it under another spelling"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

/// One name of one folder: never a path, a stream or a relative step.
pub fn plain(name: &str) -> io::Result<()> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
        || (cfg!(windows) && name.contains(':'));
    if bad {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("`{name}` is not a plain name"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure: the decision about mounts, with made-up answers of the system.

    const fn mount(id: Option<u64>, root: Option<bool>) -> Mount {
        Mount { id, root }
    }

    #[test]
    fn a_mount_inside_the_fence_is_a_crossing_even_on_the_same_device() {
        let fence = mount(Some(40), Some(true));
        // Plain entries of the mount of the fence.
        assert_eq!(
            crossing(&fence, &mount(Some(40), Some(false))),
            Crossing::Same
        );
        // Another file system mounted below.
        assert_eq!(
            crossing(&fence, &mount(Some(41), Some(true))),
            Crossing::Crossed
        );
        // A bind mount of a folder of the same device: new mount id, root of its mount.
        assert_eq!(
            crossing(&fence, &mount(Some(77), Some(true))),
            Crossing::Crossed
        );
        // Below that bind mount: not a root any more, still another mount.
        assert_eq!(
            crossing(&fence, &mount(Some(77), Some(false))),
            Crossing::Crossed
        );
        // A folder bound onto itself keeps nothing but the root mark.
        assert_eq!(
            crossing(&fence, &mount(Some(40), Some(true))),
            Crossing::Crossed
        );
    }

    #[test]
    fn a_system_that_does_not_identify_mounts_proves_nothing() {
        let known = mount(Some(40), Some(false));
        for unknown in [
            mount(None, Some(false)),
            mount(Some(40), None),
            mount(None, None),
        ] {
            assert_eq!(crossing(&known, &unknown), Crossing::Unknown);
        }
        assert_eq!(
            crossing(&mount(None, Some(true)), &known),
            Crossing::Unknown
        );
        // The fence not saying whether it is a root is fine: only its id is compared.
        assert_eq!(
            crossing(&mount(Some(40), None), &mount(Some(40), Some(false))),
            Crossing::Same
        );
    }

    #[test]
    fn only_one_plain_name_is_a_name() {
        for name in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
            assert!(plain(name).is_err(), "{name:?}");
        }
        assert!(plain("a..b").is_ok());
        assert_eq!(plain("file:stream").is_err(), cfg!(windows));
    }
}
