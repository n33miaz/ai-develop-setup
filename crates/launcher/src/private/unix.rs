//! Unix: every entry is opened, renamed and removed relative to the folder held open
//! (`openat`, `renameat`, `unlinkat`), never following a link. No `unsafe`: the calls come
//! from `rustix`, as does the `SIGTERM` that asks a started child to close.

use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::process::Child;

use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, Stat, fstat, open, openat, renameat, statat, unlinkat,
};
use rustix::process::{Pid, Signal, kill_process};

use super::Kind;

const ENTRY: OFlags = OFlags::NOFOLLOW.union(OFlags::CLOEXEC);

pub(super) fn open_dir(path: &Path) -> io::Result<OwnedFd> {
    let flags = ENTRY | OFlags::RDONLY | OFlags::DIRECTORY;
    Ok(open(path, flags, Mode::empty())?)
}

pub(super) fn stat(fd: &OwnedFd) -> io::Result<Stat> {
    Ok(fstat(fd)?)
}

pub(super) fn stat_file(file: &File) -> io::Result<Stat> {
    Ok(fstat(file)?)
}

pub(super) fn kind(stat: &Stat) -> Kind {
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::Directory => Kind::Dir,
        FileType::RegularFile => Kind::File,
        _ => Kind::Other,
    }
}

/// Opens the file `name` of the folder for reading and writing, created with mode 0600 when
/// missing. An existing file keeps its own owner and mode: the caller checks them.
pub(super) fn open_or_create(dir: &OwnedFd, name: &str) -> io::Result<File> {
    let flags = ENTRY | OFlags::RDWR | OFlags::CREATE;
    Ok(openat(dir, name, flags, Mode::from_raw_mode(0o600))?.into())
}

/// Creates the new file `name` of the folder with mode 0600, open for writing.
pub(super) fn create_new(dir: &OwnedFd, name: &str) -> io::Result<File> {
    let flags = ENTRY | OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL;
    Ok(openat(dir, name, flags, Mode::from_raw_mode(0o600))?.into())
}

/// Opens the entry `name` of the folder itself to read what it is; a link is not followed
/// (`ELOOP`) and a FIFO does not block.
pub(super) fn open_entry(dir: &OwnedFd, name: &str) -> io::Result<OwnedFd> {
    let flags = ENTRY | OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOCTTY;
    Ok(openat(dir, name, flags, Mode::empty())?)
}

/// What `name` of the folder is right now, without following a link.
pub(super) fn stat_at(dir: &OwnedFd, name: &str) -> io::Result<Stat> {
    Ok(statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)?)
}

pub(super) fn same(left: &Stat, right: &Stat) -> bool {
    left.st_dev == right.st_dev && left.st_ino == right.st_ino
}

pub(super) fn remove(dir: &OwnedFd, name: &str) -> io::Result<()> {
    Ok(unlinkat(dir, name, AtFlags::empty())?)
}

pub(super) fn rename(dir: &OwnedFd, from: &str, to: &str) -> io::Result<()> {
    Ok(renameat(dir, from, dir, to)?)
}

/// Sends `SIGTERM` to a child the caller still holds unreaped, so its pid cannot be reused.
pub(crate) fn request_close(child: &Child) -> io::Result<()> {
    Ok(kill_process(Pid::from_child(child), Signal::TERM)?)
}
