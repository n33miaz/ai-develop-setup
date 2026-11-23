//! The system calls, by open handle. Nothing above this module gives a path to the operating
//! system: a [`Node`] is an entry that is already open, a child is opened by one plain name
//! relative to its open folder, and no link is ever followed. What an operation reaches is
//! decided by what is open, not by how a path is spelled.
//!
//! The only path taken is the one of [`Node::anchor`]: the root of the volume, from which the
//! fence itself is reached name by name.

use std::io;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Claim, HOLDS, Node};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{Claim, HOLDS, Node, NtStatusError};

#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", windows)))]
pub use unsupported::{Claim, HOLDS, Node};

/// What an entry is, as seen on its own handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
    /// A symbolic link, a junction or any other reparse point; never entered.
    Link,
}

/// Which object an entry is: the device and inode (with its birth time, where the system
/// gives it), or the volume and file id on Windows. Two spellings of one object have one
/// identity; another object under the same name has another only on a volume whose identity
/// is stable (see [`VolumeFacts::stable`]).
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

/// What the file system of a volume says about itself, read on an open entry of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeFacts {
    #[cfg_attr(not(windows), allow(dead_code))]
    Windows {
        /// `NTFS`, `ReFS`, `FAT32`, `exFAT`...
        file_system: String,
        /// Reached through a network redirector.
        remote: bool,
        /// The file system can open a file by its id: its ids name objects, not places.
        open_by_id: bool,
    },
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Linux {
        /// `f_type` of `statfs`.
        magic: u64,
        /// `statx` reports the birth time, which goes into the identity.
        birth: bool,
    },
}

// Linux file systems whose inode lives as long as its object; with the birth time in the
// identity, an inode handed again to a new object gets a new identity.
const EXT: u64 = 0xEF53;
const XFS: u64 = 0x5846_5342;
const BTRFS: u64 = 0x9123_683E;
const TMPFS: u64 = 0x0102_1994;
const F2FS: u64 = 0xF2F5_2010;
const ZFS: u64 = 0x2FC1_2FC1;

impl VolumeFacts {
    /// Whether an object deleted and created again under the same name gets another identity.
    /// NTFS and ReFS, local and able to open by id: yes. A known local Linux file system that
    /// reports the birth time: yes, to the granularity of its clock. FAT, exFAT, a network
    /// share and anything not listed here: no, or not known, which counts as no.
    pub fn stable(&self) -> bool {
        match self {
            Self::Windows {
                file_system,
                remote,
                open_by_id,
            } => {
                let local_ids = !remote && *open_by_id;
                local_ids && matches!(file_system.as_str(), "NTFS" | "ReFS")
            }
            Self::Linux { magic, birth } => {
                *birth && matches!(*magic, EXT | XFS | BTRFS | TMPFS | F2FS | ZFS)
            }
        }
    }
}

impl std::fmt::Display for VolumeFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Windows {
                file_system,
                remote,
                open_by_id,
            } => write!(
                f,
                "{file_system}{}{}",
                if *remote { ", over the network" } else { "" },
                if *open_by_id { "" } else { ", no open by id" }
            ),
            Self::Linux { magic, birth } => write!(
                f,
                "file system {magic:#x}{}",
                if *birth { "" } else { ", no birth time" }
            ),
        }
    }
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
    Missing(io::Error),
    /// The name is taken.
    Exists(io::Error),
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
            io::ErrorKind::NotFound => Self::Missing(error),
            io::ErrorKind::AlreadyExists => Self::Exists(error),
            _ => Self::Io(error),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Missing(error) | Self::Exists(error) | Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(_) => f.write_str("it does not exist"),
            Self::Exists(_) => f.write_str("the name is taken"),
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

    // Pure: the decision about volumes, with made-up answers of the system.

    fn windows(file_system: &str, remote: bool, open_by_id: bool) -> VolumeFacts {
        VolumeFacts::Windows {
            file_system: file_system.to_owned(),
            remote,
            open_by_id,
        }
    }

    #[test]
    fn only_local_ntfs_and_refs_keep_identities_on_windows() {
        assert!(windows("NTFS", false, true).stable());
        assert!(windows("ReFS", false, true).stable());
        for unstable in [
            windows("FAT32", false, true),
            windows("FAT", false, false),
            windows("exFAT", false, true),
            windows("UDF", false, false),
            windows("", false, true),
            // A share served by NTFS is still a share.
            windows("NTFS", true, true),
            windows("NTFS", false, false),
            windows("ntfs", false, true),
        ] {
            assert!(!unstable.stable(), "{unstable}");
        }
    }

    #[test]
    fn only_known_local_linux_file_systems_with_a_birth_time_keep_identities() {
        let linux = |magic, birth| VolumeFacts::Linux { magic, birth };
        for magic in [EXT, XFS, BTRFS, TMPFS, F2FS, ZFS] {
            assert!(linux(magic, true).stable(), "{magic:#x}");
            // Without the birth time a freed inode handed again is the same identity.
            assert!(!linux(magic, false).stable(), "{magic:#x}");
        }
        // vfat, exfat, nfs, smb, cifs, smb2, fuse, overlay, 9p, ntfs3, and an unknown one.
        for magic in [
            0x4d44,
            0x2011_BAB0,
            0x6969,
            0x517B,
            0xFF53_4D42,
            0xFE53_4D42,
            0x6573_5546,
            0x794C_7630,
            0x0102_1997,
            0x7366_746E,
            0,
        ] {
            assert!(!linux(magic, true).stable(), "{magic:#x}");
        }
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
