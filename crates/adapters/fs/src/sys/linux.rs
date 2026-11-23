//! Linux: `openat`, `statx`, `mkdirat`, `unlinkat` and `renameat2`, always relative to an open
//! folder and never following a link. No `unsafe`: the calls come from `rustix`.
//!
//! `unlinkat` and `renameat2` act on a name inside the open folder, not on an open object: the
//! folder cannot be swapped, the entry under the name is compared right before the call.

use std::fs::File;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, RawMode, RenameFlags, StatxAttributes, StatxFlags,
    fstatfs, fsync, mkdirat, open, openat, renameat_with, statx, unlinkat,
};

use super::{Error, Identity, Kind, Meta, Mount, VolumeFacts, plain};

impl From<rustix::io::Errno> for Error {
    fn from(errno: rustix::io::Errno) -> Self {
        Self::from(io::Error::from(errno))
    }
}

/// A descriptor names an object, not a place: a folder that is open can still be renamed,
/// moved out of the fence or removed by another process. Nothing here holds it in place.
pub const HOLDS: bool = false;

/// An open entry: a folder, a file or a link itself (`O_PATH`, `O_NOFOLLOW`).
#[derive(Debug)]
pub struct Node {
    fd: OwnedFd,
}

const ENTRY: OFlags = OFlags::PATH.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);
const WANTED: StatxFlags = StatxFlags::BASIC_STATS
    .union(StatxFlags::BTIME)
    .union(StatxFlags::MNT_ID);

/// The inode with, above it, the birth time where the system reports it: a freed inode handed
/// at once to a new object (ext4, xfs, tmpfs) comes back with another birth time. The birth
/// time is never changed by a rename, a write or a change of links, so the executor's own moves
/// keep the identity. Seconds take 34 bits up to the year 2514, nanoseconds 30.
fn file_of(inode: u64, birth: Option<(i64, u32)>) -> u128 {
    let birth = birth.map_or(0, |(seconds, nanos)| {
        ((seconds as u64) << 30) | u64::from(nanos)
    });
    (u128::from(birth) << 64) | u128::from(inode)
}

#[cfg(test)]
mod tests {
    use super::file_of;

    #[test]
    fn birth_time_distinguishes_objects_with_the_same_inode() {
        let inode = 42;
        let file = file_of(inode, Some((1, 7)));
        assert_ne!(file, file_of(inode, Some((1, 8))));
        assert_ne!(file, file_of(inode, Some((2, 7))));
        assert_ne!(file, file_of(inode, None));
        assert_eq!(file_of(inode, None), u128::from(inode));
    }
}

fn meta_of(stat: &rustix::fs::Statx) -> Meta {
    let kind = match FileType::from_raw_mode(RawMode::from(stat.stx_mode)) {
        FileType::Directory => Some(Kind::Dir),
        FileType::RegularFile => Some(Kind::File),
        FileType::Symlink => Some(Kind::Link),
        _ => None,
    };
    let told = StatxFlags::from_bits_retain(stat.stx_mask);
    Meta {
        kind,
        id: Identity {
            volume: (u64::from(stat.stx_dev_major) << 32) | u64::from(stat.stx_dev_minor),
            file: file_of(
                stat.stx_ino,
                told.contains(StatxFlags::BTIME)
                    .then_some((stat.stx_btime.tv_sec, stat.stx_btime.tv_nsec)),
            ),
        },
        size: stat.stx_size,
        mount: Mount {
            id: told.contains(StatxFlags::MNT_ID).then_some(stat.stx_mnt_id),
            root: stat
                .stx_attributes_mask
                .contains(StatxAttributes::MOUNT_ROOT)
                .then(|| stat.stx_attributes.contains(StatxAttributes::MOUNT_ROOT)),
        },
    }
}

impl Node {
    /// Opens the root of the file system by its path (`/`): where the walk to the fence
    /// starts. The only path this crate gives to the system.
    pub fn anchor(path: &Path) -> io::Result<Self> {
        let fd = open(path, ENTRY | OFlags::DIRECTORY, Mode::empty())?;
        Ok(Self { fd })
    }

    pub fn again(&self) -> io::Result<Self> {
        Ok(Self {
            fd: self.fd.try_clone()?,
        })
    }

    pub fn meta(&self) -> io::Result<Meta> {
        let flags = AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW;
        Ok(meta_of(&statx(&self.fd, "", flags, WANTED)?))
    }

    /// What the file system of this entry says about itself.
    pub fn volume(&self) -> io::Result<VolumeFacts> {
        let flags = AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW;
        let stat = statx(&self.fd, "", flags, WANTED)?;
        // Magic numbers are 32 bits; `f_type` is a signed word on some targets.
        let magic = fstatfs(&self.fd)?.f_type as u64 & 0xFFFF_FFFF;
        Ok(VolumeFacts::Linux {
            magic,
            birth: StatxFlags::from_bits_retain(stat.stx_mask).contains(StatxFlags::BTIME),
        })
    }

    /// There is no holding a folder in place here (see [`HOLDS`]): this is one more handle.
    pub fn hold(&self) -> io::Result<Self> {
        self.again()
    }

    /// Whether the entry under `name` of this folder is the object `expected`, right now.
    pub fn holds(&self, name: &str, expected: Identity) -> Result<bool, Error> {
        plain(name)?;
        match statx(&self.fd, name, AtFlags::SYMLINK_NOFOLLOW, WANTED) {
            Ok(stat) => Ok(meta_of(&stat).id == expected),
            Err(rustix::io::Errno::NOENT) => Ok(false),
            Err(errno) => Err(errno.into()),
        }
    }

    /// Whether a move that the system accepted took `expected` to `name` of this folder.
    /// The move acted on a name, so what arrived is looked at again, by name: this says what
    /// is under the name at this instant and nothing more.
    pub fn arrived(&self, name: &str, expected: Identity) -> Result<bool, Error> {
        self.holds(name, expected)
    }

    /// Opens the entry `name` of this folder, itself: a link is opened, not followed.
    pub fn child(&self, name: &str) -> Result<Self, Error> {
        plain(name)?;
        let fd = openat(&self.fd, name, ENTRY, Mode::empty())?;
        Ok(Self { fd })
    }

    /// Like [`Self::child`], for a name this folder listed. Names are exact here.
    pub fn listed(&self, name: &str) -> Result<Self, Error> {
        self.child(name)
    }

    fn readable(&self) -> io::Result<OwnedFd> {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        Ok(openat(&self.fd, ".", flags, Mode::empty())?)
    }

    /// The names in this folder, sorted.
    pub fn names(&self) -> io::Result<Vec<String>> {
        self.names_limited(usize::MAX)
    }

    pub fn names_limited(&self, limit: usize) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in Dir::new(self.readable()?)? {
            let entry = entry?;
            let name = entry.file_name().to_str().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "a name that is not text")
            })?;
            if name != "." && name != ".." {
                names.push(name.to_owned());
                if names.len() > limit {
                    break;
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Makes the folder `name`, which must not exist. `Ok` is "made", and nothing else: the
    /// folder is not opened here, so no later failure can be mistaken for "not made".
    pub fn make_dir(&self, name: &str) -> Result<(), Error> {
        plain(name)?;
        Ok(mkdirat(&self.fd, name, Mode::from_raw_mode(0o777))?)
    }

    /// Like [`Self::make_dir`], for a folder only this executor is meant to use: no other
    /// user gets in. A process of the same user still does.
    ///
    /// Always `None` for the identity: `mkdirat` gives no handle of what it made, and the
    /// name, looked at afterwards, may already be another folder.
    pub fn make_private_dir(&self, name: &str) -> Result<Option<Identity>, Error> {
        plain(name)?;
        mkdirat(&self.fd, name, Mode::from_raw_mode(0o700))?;
        Ok(None)
    }

    /// Creates the new, empty file `name`, which must not exist, open for writing. Writing
    /// it and having it on disk are the caller's next two steps.
    pub fn create_file(&self, name: &str) -> Result<File, Error> {
        plain(name)?;
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = openat(&self.fd, name, flags, Mode::from_raw_mode(0o666))?;
        Ok(File::from(fd))
    }

    /// Reads the regular file `name`, at most `limit` bytes. What is checked is the file that
    /// was opened; a link is not followed and a larger file is refused.
    pub fn read_file(&self, name: &str, limit: u64) -> Result<Vec<u8>, Error> {
        plain(name)?;
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let fd = openat(&self.fd, name, flags, Mode::empty())?;
        let stat = statx(&fd, "", AtFlags::EMPTY_PATH, WANTED)?;
        if meta_of(&stat).kind != Some(Kind::File) {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            )));
        }
        let mut bytes = Vec::new();
        File::from(fd)
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "larger than the limit",
            )));
        }
        Ok(bytes)
    }

    /// Has the entries of this folder (names created, moved, removed) on disk.
    pub fn sync(&self) -> io::Result<()> {
        Ok(fsync(self.readable()?)?)
    }

    /// Takes hold of the entry `name` of this folder for one mutation, after comparing it
    /// with `expected`.
    pub fn claim(&self, name: &str, expected: Identity) -> Result<Claim<'_>, Error> {
        self.claim_named(name, expected)
    }

    /// Like [`Self::claim`]: nothing here keeps the entry under its name (see [`HOLDS`]).
    /// The removal acts on the name, and whoever renames the entry in between makes it fail.
    pub fn claim_to_remove(&self, name: &str, expected: Identity) -> Result<Claim<'_>, Error> {
        self.claim_named(name, expected)
    }

    fn claim_named(&self, name: &str, expected: Identity) -> Result<Claim<'_>, Error> {
        plain(name)?;
        let found = meta_of(&statx(&self.fd, name, AtFlags::SYMLINK_NOFOLLOW, WANTED)?);
        if found.id != expected {
            return Err(Error::Changed);
        }
        Ok(Claim {
            parent: self,
            name: name.to_owned(),
            meta: found,
        })
    }
}

/// An entry held for one mutation: the open folder and the name that was just compared.
#[derive(Debug)]
pub struct Claim<'a> {
    parent: &'a Node,
    name: String,
    meta: Meta,
}

impl Claim<'_> {
    /// What the entry is, read when it was claimed.
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Removes the entry: a file, a link itself or an empty folder.
    pub fn remove(self) -> Result<(), Error> {
        let flags = if self.meta.kind == Some(Kind::Dir) {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        Ok(unlinkat(&self.parent.fd, &self.name, flags)?)
    }

    /// Moves the entry to `name` in the open folder `to`. An existing destination is refused
    /// by the system itself (`RENAME_NOREPLACE`); a file system without it refuses the move.
    /// `Ok` means the system made the move; what arrived is for [`Node::arrived`] to say.
    pub fn move_to(self, to: &Node, name: &str) -> Result<(), Error> {
        plain(name)?;
        Ok(renameat_with(
            &self.parent.fd,
            &self.name,
            &to.fd,
            name,
            RenameFlags::NOREPLACE,
        )?)
    }
}
