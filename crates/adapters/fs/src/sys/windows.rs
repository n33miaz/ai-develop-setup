//! Windows: every entry is opened relative to an open folder (`NtCreateFile` with a root
//! handle), as itself and never through a reparse point, and is moved or removed by its own
//! handle (`NtSetInformationFile`).
//! Every `unsafe` block of the crate lives here and is listed in `docs/unsafe.md`.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

use windows::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_DIRECTORY_FILE, FILE_INFORMATION_CLASS, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
    FILE_OPEN_FOR_BACKUP_INTENT, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
    FileDispositionInformationEx, FileRenameInformation, NTCREATEFILE_CREATE_DISPOSITION,
    NTCREATEFILE_CREATE_OPTIONS, NtCreateFile, NtSetInformationFile,
};
use windows::Win32::Foundation::{
    HANDLE, NTSTATUS, OBJECT_ATTRIBUTE_FLAGS, STATUS_DIRECTORY_NOT_EMPTY, STATUS_NOT_A_DIRECTORY,
    STATUS_OBJECT_NAME_COLLISION, STATUS_OBJECT_NAME_NOT_FOUND, STATUS_OBJECT_PATH_NOT_FOUND,
    UNICODE_STRING,
};
use windows::Win32::Storage::FileSystem::{
    DELETE, FILE_ACCESS_RIGHTS, FILE_ADD_FILE, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_INFO_BY_HANDLE_CLASS, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FileAttributeTagInfo,
    FileFullDirectoryInfo, FileFullDirectoryRestartInfo, FileIdInfo, FileStandardInfo,
    GetFileInformationByHandleEx, SYNCHRONIZE,
};
use windows::Win32::System::IO::IO_STATUS_BLOCK;
use windows::core::PWSTR;

use super::{Error, Identity, Kind, Meta, Mount, plain};

const ATTRIBUTE_DIRECTORY: u32 = 0x10;
const ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const ERROR_NO_MORE_FILES: i32 = 18;

/// Delete now (not when the last handle closes), read-only or not.
const DISPOSITION_DELETE: u32 = 0x1 | 0x2 | 0x10;

const LOOK: FILE_ACCESS_RIGHTS = FILE_ACCESS_RIGHTS(FILE_READ_ATTRIBUTES.0 | SYNCHRONIZE.0);
const BROWSE: FILE_ACCESS_RIGHTS =
    FILE_ACCESS_RIGHTS(LOOK.0 | FILE_LIST_DIRECTORY.0 | FILE_TRAVERSE.0);
/// Always the entry itself (a reparse point is opened, not followed), with blocking calls.
const ITSELF: NTCREATEFILE_CREATE_OPTIONS = NTCREATEFILE_CREATE_OPTIONS(
    FILE_OPEN_REPARSE_POINT.0 | FILE_SYNCHRONOUS_IO_NONALERT.0 | FILE_OPEN_FOR_BACKUP_INTENT.0,
);

/// An open entry: a folder, a file or a reparse point itself.
#[derive(Debug)]
pub struct Node {
    file: File,
}

fn raw(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}

enum Refusal {
    Missing,
    Exists,
    NotADirectory,
    Other(io::Error),
}

impl From<Refusal> for Error {
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::Missing => Self::Missing,
            Refusal::Exists => Self::Exists,
            Refusal::NotADirectory => {
                Self::Io(io::Error::new(io::ErrorKind::NotADirectory, "not a folder"))
            }
            Refusal::Other(error) => Self::Io(error),
        }
    }
}

/// Opens or creates `name` relative to the open folder `root`. An empty name opens `root`
/// itself again, with another access.
fn open_at(
    root: &File,
    name: &str,
    access: FILE_ACCESS_RIGHTS,
    disposition: NTCREATEFILE_CREATE_DISPOSITION,
    options: NTCREATEFILE_CREATE_OPTIONS,
) -> Result<File, Refusal> {
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    let Ok(bytes) = u16::try_from(wide.len() * 2) else {
        return Err(Refusal::Other(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the name is too long",
        )));
    };
    let text = UNICODE_STRING {
        Length: bytes,
        MaximumLength: bytes,
        Buffer: PWSTR(wide.as_mut_ptr()),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: raw(root),
        ObjectName: &text,
        Attributes: OBJECT_ATTRIBUTE_FLAGS(0),
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle = HANDLE::default();
    let mut block = IO_STATUS_BLOCK::default();
    // SAFETY: `attributes` points to `text`, which points to `wide`; the three live until the
    // call returns, and `handle` and `block` are valid places to write to.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &attributes,
            &mut block,
            None,
            FILE_ATTRIBUTE_NORMAL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            disposition,
            options,
            None,
            0,
        )
    };
    if status.is_ok() {
        // SAFETY: the call succeeded, so `handle` is a new handle nobody else owns.
        return Ok(File::from(unsafe {
            OwnedHandle::from_raw_handle(handle.0)
        }));
    }
    Err(match status {
        STATUS_OBJECT_NAME_NOT_FOUND | STATUS_OBJECT_PATH_NOT_FOUND => Refusal::Missing,
        STATUS_OBJECT_NAME_COLLISION => Refusal::Exists,
        STATUS_NOT_A_DIRECTORY => Refusal::NotADirectory,
        other => Refusal::Other(refused(other)),
    })
}

/// The error of a Win32 call with its system code, so that its kind can be told.
fn system_error(error: windows::core::Error) -> io::Error {
    let code = error.code().0;
    if (code as u32) >> 16 == 0x8007 {
        io::Error::from_raw_os_error(code & 0xFFFF)
    } else {
        io::Error::from(error)
    }
}

/// Fills `words` with the answer of the system about the open `file`.
fn query(file: &File, class: FILE_INFO_BY_HANDLE_CLASS, words: &mut [u64]) -> io::Result<()> {
    let size = u32::try_from(size_of_val(words)).map_err(io::Error::other)?;
    // SAFETY: the buffer is `size` writable bytes, aligned for every structure asked for here.
    unsafe { GetFileInformationByHandleEx(raw(file), class, words.as_mut_ptr().cast(), size) }
        .map_err(system_error)
}

/// Applies `words` (the first `size` bytes) to the open `file`.
fn apply(
    file: &File,
    class: FILE_INFORMATION_CLASS,
    words: &[u64],
    size: usize,
) -> Result<(), Error> {
    let size = u32::try_from(size.min(size_of_val(words))).map_err(io::Error::other)?;
    let mut block = IO_STATUS_BLOCK::default();
    // SAFETY: the buffer is at least `size` readable bytes, aligned for the structures used,
    // and `block` is a valid place to write to.
    let status = unsafe {
        NtSetInformationFile(
            raw(file),
            &mut block,
            words.as_ptr().cast::<c_void>(),
            size,
            class,
        )
    };
    match status {
        status if status.is_ok() => Ok(()),
        STATUS_OBJECT_NAME_COLLISION => Err(Error::Exists),
        STATUS_DIRECTORY_NOT_EMPTY => Err(Error::Io(io::Error::new(
            io::ErrorKind::DirectoryNotEmpty,
            "the folder is not empty",
        ))),
        other => Err(Error::Io(refused(other))),
    }
}

fn refused(status: NTSTATUS) -> io::Error {
    io::Error::other(format!("the system answered {:#010x}", status.0))
}

// The answers are little-endian structures read out of 64-bit words.
fn u32_at(words: &[u64], offset: usize) -> Option<u32> {
    let word = words.get(offset / 8)?;
    Some((word >> ((offset % 8) * 8)) as u32)
}

fn u16_at(words: &[u64], offset: usize) -> Option<u16> {
    let word = words.get(offset / 8)?;
    Some((word >> ((offset % 8) * 8)) as u16)
}

fn meta_of(file: &File) -> io::Result<Meta> {
    // FILE_ATTRIBUTE_TAG_INFO: attributes, reparse tag.
    let mut tag = [0u64; 1];
    query(file, FileAttributeTagInfo, &mut tag)?;
    let attributes = tag[0] as u32;
    // FILE_ID_INFO: volume serial number, 128-bit file id.
    let mut id = [0u64; 3];
    query(file, FileIdInfo, &mut id)?;
    // FILE_STANDARD_INFO: allocation size, end of file, ...
    let mut standard = [0u64; 3];
    query(file, FileStandardInfo, &mut standard)?;
    let kind = if attributes & ATTRIBUTE_REPARSE_POINT != 0 {
        Kind::Link
    } else if attributes & ATTRIBUTE_DIRECTORY != 0 {
        Kind::Dir
    } else {
        Kind::File
    };
    Ok(Meta {
        kind: Some(kind),
        id: Identity {
            volume: id[0],
            file: (u128::from(id[2]) << 64) | u128::from(id[1]),
        },
        size: standard[1],
        // A volume mounted in a folder is a reparse point here: seen as a link, never entered.
        mount: Mount {
            id: Some(id[0]),
            root: Some(false),
        },
    })
}

impl Node {
    /// Opens the fence by its path. The only path this crate gives to the system.
    pub fn root(path: &Path) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .access_mode(BROWSE.0)
            .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
            .custom_flags(FLAG_BACKUP_SEMANTICS | FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        Ok(Self { file })
    }

    pub fn again(&self) -> io::Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
        })
    }

    pub fn meta(&self) -> io::Result<Meta> {
        meta_of(&self.file)
    }

    /// Opens the entry `name` of this folder, itself: a reparse point is opened, not
    /// followed. The name has to be the one this folder lists, letter by letter: a short
    /// name or another case is an alias, not the entry.
    pub fn child(&self, name: &str) -> Result<Self, Error> {
        let node = self.listed(name)?;
        if !self.names()?.iter().any(|listed| listed == name) {
            return Err(Error::Alias);
        }
        Ok(node)
    }

    /// Like [`Self::child`], for a name this folder just listed.
    pub fn listed(&self, name: &str) -> Result<Self, Error> {
        plain(name)?;
        let folder = NTCREATEFILE_CREATE_OPTIONS(ITSELF.0 | FILE_DIRECTORY_FILE.0);
        let file = match open_at(&self.file, name, BROWSE, FILE_OPEN, folder) {
            Err(Refusal::NotADirectory) => open_at(&self.file, name, LOOK, FILE_OPEN, ITSELF),
            other => other,
        }?;
        Ok(Self { file })
    }

    /// The names in this folder, sorted.
    pub fn names(&self) -> io::Result<Vec<String>> {
        let broken = || io::Error::new(io::ErrorKind::InvalidData, "a listing that does not fit");
        let mut names = Vec::new();
        let mut words = vec![0u64; 8192];
        let mut class = FileFullDirectoryRestartInfo;
        loop {
            match query(&self.file, class, &mut words) {
                Ok(()) => {}
                Err(error) if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) => break,
                Err(error) => return Err(error),
            }
            class = FileFullDirectoryInfo;
            // FILE_FULL_DIR_INFO: next entry at 0, name length at 60, name from 68.
            let mut at = 0;
            loop {
                let next = u32_at(&words, at).ok_or_else(broken)? as usize;
                let length = u32_at(&words, at + 60).ok_or_else(broken)? as usize;
                let units = (0..length / 2)
                    .map(|unit| u16_at(&words, at + 68 + unit * 2))
                    .collect::<Option<Vec<u16>>>()
                    .ok_or_else(broken)?;
                let name = String::from_utf16(&units).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "a name that is not text")
                })?;
                if name != "." && name != ".." {
                    names.push(name);
                }
                if next == 0 {
                    break;
                }
                at += next;
            }
        }
        names.sort();
        Ok(names)
    }

    /// Makes the folder `name`, which must not exist, and opens it.
    pub fn make_dir(&self, name: &str) -> Result<Self, Error> {
        plain(name)?;
        let options = NTCREATEFILE_CREATE_OPTIONS(ITSELF.0 | FILE_DIRECTORY_FILE.0);
        let file = open_at(&self.file, name, BROWSE, FILE_CREATE, options)?;
        Ok(Self { file })
    }

    /// Writes the new file `name`, which must not exist, and has it on disk before returning.
    pub fn write_new(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
        plain(name)?;
        let options =
            NTCREATEFILE_CREATE_OPTIONS(FILE_NON_DIRECTORY_FILE.0 | FILE_SYNCHRONOUS_IO_NONALERT.0);
        let mut file = open_at(&self.file, name, FILE_GENERIC_WRITE, FILE_CREATE, options)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }

    /// Reads the regular file `name`, at most `limit` bytes. What is checked is the file that
    /// was opened; a reparse point is not followed and a larger file is refused.
    pub fn read_file(&self, name: &str, limit: u64) -> Result<Vec<u8>, Error> {
        plain(name)?;
        let options = NTCREATEFILE_CREATE_OPTIONS(
            FILE_NON_DIRECTORY_FILE.0 | FILE_OPEN_REPARSE_POINT.0 | FILE_SYNCHRONOUS_IO_NONALERT.0,
        );
        let file = open_at(&self.file, name, FILE_GENERIC_READ, FILE_OPEN, options)?;
        if meta_of(&file)?.kind != Some(Kind::File) {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            )));
        }
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
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
        let access = FILE_ACCESS_RIGHTS(FILE_ADD_FILE.0 | SYNCHRONIZE.0);
        let options = NTCREATEFILE_CREATE_OPTIONS(ITSELF.0 | FILE_DIRECTORY_FILE.0);
        let writable =
            open_at(&self.file, "", access, FILE_OPEN, options).map_err(
                |refusal| match Error::from(refusal) {
                    Error::Io(error) => error,
                    other => io::Error::other(other.to_string()),
                },
            )?;
        writable.sync_all()
    }

    /// Takes hold of the entry `name` of this folder for one mutation: opens it with the
    /// right to move and delete it, and compares what was opened with `expected`.
    pub fn claim(&self, name: &str, expected: Identity) -> Result<Claim<'_>, Error> {
        plain(name)?;
        let access = FILE_ACCESS_RIGHTS(LOOK.0 | DELETE.0);
        let file = open_at(&self.file, name, access, FILE_OPEN, ITSELF)?;
        if meta_of(&file)?.id != expected {
            return Err(Error::Changed);
        }
        Ok(Claim {
            file,
            parent: std::marker::PhantomData,
        })
    }
}

/// An entry held for one mutation by its own open handle.
#[derive(Debug)]
pub struct Claim<'a> {
    file: File,
    parent: std::marker::PhantomData<&'a Node>,
}

impl Claim<'_> {
    /// Removes the entry that is open: a file, a reparse point itself or an empty folder.
    pub fn remove(self) -> Result<(), Error> {
        // FILE_DISPOSITION_INFO_EX: flags.
        let words = [u64::from(DISPOSITION_DELETE)];
        apply(&self.file, FileDispositionInformationEx, &words, 4)
    }

    /// Moves the entry that is open to `name` in the open folder `to`. An existing
    /// destination is refused by the system itself.
    pub fn move_to(self, to: &Node, name: &str) -> Result<(), Error> {
        plain(name)?;
        // FILE_RENAME_INFORMATION: replace flag at 0, root folder at 8, name length at 16, name
        // from 20.
        let units: Vec<u16> = name.encode_utf16().collect();
        let length = u32::try_from(units.len() * 2).map_err(io::Error::other)?;
        let mut bytes = vec![0u8; 20];
        bytes[8..16].copy_from_slice(&(raw(&to.file).0 as u64).to_le_bytes());
        bytes[16..20].copy_from_slice(&length.to_le_bytes());
        bytes.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
        let size = bytes.len().max(24);
        bytes.resize(size.div_ceil(8) * 8, 0);
        let words: Vec<u64> = bytes
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("chunks of eight")))
            .collect();
        apply(&self.file, FileRenameInformation, &words, size)
    }
}
