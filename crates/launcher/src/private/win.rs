//! Windows calls for the control folder: identity and access lists, and every entry opened
//! relative to the folder held open (`NtCreateFile` with a root handle), as itself and never
//! through a reparse point, then renamed or removed by its own handle
//! (`NtSetInformationFile`). Each block is a single call or the read of data the previous call
//! filled, with a `SAFETY` comment; see docs/unsafe.md.

// The rename structure below is laid out by hand for 64-bit Windows.
#[cfg(not(target_pointer_width = "64"))]
compile_error!("ads-launcher supports Windows on 64-bit targets only");

use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

use windows::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_INFORMATION_CLASS, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF,
    FILE_OPEN_REPARSE_POINT, FILE_RENAME_INFORMATION, FILE_SYNCHRONOUS_IO_NONALERT,
    FileDispositionInformationEx, FileRenameInformationEx, NTCREATEFILE_CREATE_DISPOSITION,
    NTCREATEFILE_CREATE_OPTIONS, NtCreateFile, NtSetInformationFile,
};
use windows::Win32::Foundation::{
    ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree, NTSTATUS, OBJECT_ATTRIBUTE_FLAGS,
    STATUS_ACCESS_DENIED, STATUS_OBJECT_NAME_COLLISION, STATUS_OBJECT_NAME_NOT_FOUND,
    STATUS_OBJECT_PATH_NOT_FOUND, UNICODE_STRING,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION, AclSizeInformation,
    DACL_SECURITY_INFORMATION, GetAce, GetAclInformation, GetTokenInformation,
    OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES,
    TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TokenOwner, TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    CreateDirectoryW, DELETE, FILE_ACCESS_RIGHTS, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FileAttributeTagInfo,
    GetFileInformationByHandleEx, READ_CONTROL, SYNCHRONIZE,
};
use windows::Win32::System::IO::IO_STATUS_BLOCK;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCWSTR, PWSTR};

use super::{Entry, Identity, Kind, Security};

const FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const ATTRIBUTE_DIRECTORY: u32 = 0x10;
const ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;

/// Delete now (not when the last handle closes), read-only or not.
const DISPOSITION_DELETE: u32 = 0x1 | 0x2 | 0x10;
/// Replace the destination, even while others have it open for reading.
const RENAME_REPLACE: u32 = 0x1 | 0x2;

/// Others may read and write what is open, but not rename or delete it: both ask for the right
/// to delete, which this way of sharing denies for as long as the handle lives. Held on the
/// folder, it also keeps every folder above it from being renamed.
const HELD: FILE_SHARE_MODE = FILE_SHARE_MODE(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0);
const SHARED: FILE_SHARE_MODE = FILE_SHARE_MODE(HELD.0 | FILE_SHARE_DELETE.0);
const FILE: NTCREATEFILE_CREATE_OPTIONS = NTCREATEFILE_CREATE_OPTIONS(
    FILE_NON_DIRECTORY_FILE.0 | FILE_OPEN_REPARSE_POINT.0 | FILE_SYNCHRONOUS_IO_NONALERT.0,
);
/// The entry itself, whatever it is: a reparse point is opened, not followed. No data is read
/// or written through it, so it asks for no right to wait on it, which an access list that
/// grants nothing would refuse to the owner.
const ITSELF: NTCREATEFILE_CREATE_OPTIONS = FILE_OPEN_REPARSE_POINT;

// The layout `rename` writes by hand, against the structure of the system.
const _: () = {
    assert!(size_of::<HANDLE>() == 8);
    assert!(offset_of!(FILE_RENAME_INFORMATION, RootDirectory) == RENAME_ROOT);
    assert!(offset_of!(FILE_RENAME_INFORMATION, FileNameLength) == RENAME_LENGTH);
    assert!(offset_of!(FILE_RENAME_INFORMATION, FileName) == RENAME_NAME);
    assert!(size_of::<FILE_RENAME_INFORMATION>() == RENAME_SIZE);
};
const RENAME_ROOT: usize = 8;
const RENAME_LENGTH: usize = 16;
const RENAME_NAME: usize = 20;
const RENAME_SIZE: usize = 24;

/// Memory the system allocated for the caller, released with `LocalFree` when dropped.
struct LocalMemory(*mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a call documented to be released with `LocalFree`
        // and is released only here.
        let _ = unsafe { LocalFree(Some(HLOCAL(self.0))) };
    }
}

fn raw(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}

fn take_text(text: PWSTR) -> io::Result<String> {
    let _memory = LocalMemory(text.0.cast());
    // SAFETY: the call that produced `text` succeeded, so it is a valid, terminated wide string.
    unsafe { text.to_string() }.map_err(io::Error::other)
}

fn sid_text(sid: PSID) -> io::Result<String> {
    let mut text = PWSTR::null();
    // SAFETY: the caller keeps the memory `sid` points to alive during the call; `text`
    // receives a new string.
    unsafe { ConvertSidToStringSidW(sid, &raw mut text) }?;
    take_text(text)
}

fn token_sid(token: &OwnedHandle, class: TOKEN_INFORMATION_CLASS) -> io::Result<String> {
    // `TOKEN_USER` or `TOKEN_OWNER` followed by the identifier it points to; `u64` keeps the
    // alignment.
    let mut buffer = [0_u64; 32];
    let mut written = 0_u32;
    // SAFETY: the buffer is writable for the size passed and `written` outlives the call.
    unsafe {
        GetTokenInformation(
            HANDLE(token.as_raw_handle()),
            class,
            Some(buffer.as_mut_ptr().cast()),
            size_of_val(&buffer) as u32,
            &raw mut written,
        )
    }?;
    // SAFETY: the call succeeded, so the buffer starts with an aligned, initialized
    // `TOKEN_USER` or `TOKEN_OWNER`; both begin with the pointer to the identifier, which
    // points into `buffer`.
    let sid = unsafe { *buffer.as_ptr().cast::<PSID>() };
    sid_text(sid)
}

pub(super) fn current_identity() -> io::Result<Identity> {
    let mut raw = HANDLE::default();
    // SAFETY: the pseudo handle of the current process is always valid; `raw` outlives the call.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut raw) }?;
    // SAFETY: `raw` was just opened, is valid and has no other owner.
    let token = unsafe { OwnedHandle::from_raw_handle(raw.0) };
    Ok(Identity {
        user: token_sid(&token, TokenUser)?,
        default_owner: token_sid(&token, TokenOwner)?,
    })
}

/// A security descriptor owned by the user, with a protected access list whose single entry,
/// the user's, has the inheritance flags `inherit` (`OICI` for the folder, empty for a file).
fn private_descriptor(local: &Identity, inherit: &str) -> io::Result<LocalMemory> {
    let rule: Vec<u16> = format!("O:{0}D:P(A;{inherit};FA;;;{0})", local.user)
        .encode_utf16()
        .chain([0])
        .collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: `rule` is a terminated wide string alive during the call; `descriptor` receives
    // a new allocation.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(rule.as_ptr()),
            SDDL_REVISION_1,
            &raw mut descriptor,
            None,
        )
    }?;
    Ok(LocalMemory(descriptor.0))
}

/// Creates the folder owned by the user, with a protected access list whose single entry, the
/// user's, is inherited by every file created inside.
pub(super) fn create_dir(path: &Path, local: &Identity) -> io::Result<()> {
    let descriptor = private_descriptor(local, "OICI")?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: `wide` is a terminated wide string and `attributes` a valid structure whose
    // descriptor stays allocated until after the call.
    unsafe { CreateDirectoryW(PCWSTR(wide.as_ptr()), Some(&raw const attributes)) }
        .map_err(os_error)
}

/// The system error behind a failed call, so `io::ErrorKind` (such as `AlreadyExists`) holds.
fn os_error(error: windows::core::Error) -> io::Error {
    let code = error.code().0 as u32;
    if code & 0xFFFF_0000 == 0x8007_0000 {
        io::Error::from_raw_os_error((code & 0xFFFF) as i32)
    } else {
        io::Error::other(error)
    }
}

fn status_error(status: NTSTATUS) -> io::Error {
    let kind = match status {
        STATUS_OBJECT_NAME_NOT_FOUND | STATUS_OBJECT_PATH_NOT_FOUND => io::ErrorKind::NotFound,
        STATUS_OBJECT_NAME_COLLISION => io::ErrorKind::AlreadyExists,
        STATUS_ACCESS_DENIED => io::ErrorKind::PermissionDenied,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(
        kind,
        format!("the system answered {:#010x}", status.0 as u32),
    )
}

/// Opens the folder `path` itself (a reparse point is opened, not followed) and holds it: until
/// the handle is dropped, nobody can rename or delete it, nor rename a folder above it.
pub(super) fn open_dir(path: &Path) -> io::Result<File> {
    let access = FILE_READ_ATTRIBUTES.0
        | FILE_LIST_DIRECTORY.0
        | FILE_TRAVERSE.0
        | READ_CONTROL.0
        | SYNCHRONIZE.0;
    OpenOptions::new()
        .access_mode(access)
        .share_mode(HELD.0)
        .custom_flags(FLAG_BACKUP_SEMANTICS | FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

/// What the open entry is, read from its handle.
pub(super) fn kind(file: &File) -> io::Result<Kind> {
    // FILE_ATTRIBUTE_TAG_INFO: attributes, reparse tag.
    let mut tag = [0_u64; 1];
    // SAFETY: the buffer is writable for the size passed and aligned for the structure.
    unsafe {
        GetFileInformationByHandleEx(
            raw(file),
            FileAttributeTagInfo,
            tag.as_mut_ptr().cast(),
            size_of_val(&tag) as u32,
        )
    }
    .map_err(os_error)?;
    let attributes = tag[0] as u32;
    Ok(if attributes & ATTRIBUTE_REPARSE_POINT != 0 {
        Kind::Other
    } else if attributes & ATTRIBUTE_DIRECTORY != 0 {
        Kind::Dir
    } else {
        Kind::File
    })
}

/// Opens or creates `name` relative to the open folder `root`; a new entry gets `descriptor`.
fn open_at(
    root: &File,
    name: &str,
    access: FILE_ACCESS_RIGHTS,
    share: FILE_SHARE_MODE,
    disposition: NTCREATEFILE_CREATE_DISPOSITION,
    options: NTCREATEFILE_CREATE_OPTIONS,
    descriptor: Option<&LocalMemory>,
) -> io::Result<File> {
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    let bytes = u16::try_from(wide.len() * 2)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the name is too long"))?;
    let text = UNICODE_STRING {
        Length: bytes,
        MaximumLength: bytes,
        Buffer: PWSTR(wide.as_mut_ptr()),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: raw(root),
        ObjectName: &raw const text,
        Attributes: OBJECT_ATTRIBUTE_FLAGS(0),
        SecurityDescriptor: descriptor
            .map_or(std::ptr::null(), |memory| memory.0.cast_const().cast()),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle = HANDLE::default();
    let mut block = IO_STATUS_BLOCK::default();
    // SAFETY: `attributes` points to `text`, which points to `wide`, and to the descriptor;
    // all live until the call returns, and `handle` and `block` are valid places to write to.
    let status = unsafe {
        NtCreateFile(
            &raw mut handle,
            access,
            &raw const attributes,
            &raw mut block,
            None,
            FILE_ATTRIBUTE_NORMAL,
            share,
            disposition,
            options,
            None,
            0,
        )
    };
    if status.is_err() {
        return Err(status_error(status));
    }
    // SAFETY: the call succeeded, so `handle` is a new handle nobody else owns.
    Ok(File::from(unsafe {
        OwnedHandle::from_raw_handle(handle.0)
    }))
}

/// Applies the first `size` bytes of `words` to the open `file`.
fn apply(file: &File, class: FILE_INFORMATION_CLASS, words: &[u64], size: usize) -> io::Result<()> {
    let size = u32::try_from(size.min(size_of_val(words))).map_err(io::Error::other)?;
    let mut block = IO_STATUS_BLOCK::default();
    // SAFETY: the buffer is at least `size` readable bytes, aligned for the structures used,
    // and `block` is a valid place to write to.
    let status = unsafe {
        NtSetInformationFile(
            raw(file),
            &raw mut block,
            words.as_ptr().cast::<c_void>(),
            size,
            class,
        )
    };
    if status.is_err() {
        return Err(status_error(status));
    }
    Ok(())
}

/// Opens the regular file `name` of the folder for reading and writing, creating it with the
/// private access list when missing. An existing file keeps its own list: the caller checks it.
pub(super) fn open_or_create(root: &File, name: &str, local: &Identity) -> io::Result<File> {
    let descriptor = private_descriptor(local, "")?;
    let access = FILE_ACCESS_RIGHTS(FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0);
    open_at(
        root,
        name,
        access,
        SHARED,
        FILE_OPEN_IF,
        FILE,
        Some(&descriptor),
    )
}

/// Creates the new file `name` of the folder with the private access list, open for writing
/// and later for its own rename or removal.
pub(super) fn create_new(root: &File, name: &str, local: &Identity) -> io::Result<File> {
    let descriptor = private_descriptor(local, "")?;
    let access = FILE_ACCESS_RIGHTS(FILE_GENERIC_WRITE.0 | DELETE.0);
    open_at(
        root,
        name,
        access,
        HELD,
        FILE_CREATE,
        FILE,
        Some(&descriptor),
    )
}

/// Opens the entry `name` of the folder itself, to read its owner and access list and then
/// remove it. Until the handle is dropped nobody can rename, replace or delete that entry.
pub(super) fn open_to_remove(root: &File, name: &str) -> io::Result<File> {
    let access = FILE_ACCESS_RIGHTS(FILE_READ_ATTRIBUTES.0 | READ_CONTROL.0 | DELETE.0);
    open_at(root, name, access, HELD, FILE_OPEN, ITSELF, None)
}

/// Removes the open entry, the one the handle points to, whatever its name is now.
pub(super) fn remove(file: &File) -> io::Result<()> {
    // FILE_DISPOSITION_INFO_EX: flags.
    apply(
        file,
        FileDispositionInformationEx,
        &[u64::from(DISPOSITION_DELETE)],
        4,
    )
}

/// Moves the open entry to `name` in the open folder `root`, replacing what is there.
pub(super) fn rename(file: &File, root: &File, name: &str) -> io::Result<()> {
    // FILE_RENAME_INFORMATION: the flags, the root folder, the length of the name and the
    // name. The offsets are checked against the structure of the system at compile time.
    let units: Vec<u16> = name.encode_utf16().collect();
    let length = u32::try_from(units.len() * 2).map_err(io::Error::other)?;
    let mut bytes = vec![0_u8; RENAME_NAME];
    bytes[..4].copy_from_slice(&RENAME_REPLACE.to_le_bytes());
    bytes[RENAME_ROOT..RENAME_LENGTH].copy_from_slice(&(raw(root).0 as u64).to_le_bytes());
    bytes[RENAME_LENGTH..RENAME_NAME].copy_from_slice(&length.to_le_bytes());
    bytes.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    let size = bytes.len().max(RENAME_SIZE);
    bytes.resize(size.div_ceil(8) * 8, 0);
    let words: Vec<u64> = bytes
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("chunks of eight")))
        .collect();
    apply(file, FileRenameInformationEx, &words, size)
}

/// The owner and access list of an open file or folder.
pub(super) fn security(file: &File) -> io::Result<Security> {
    let mut owner = PSID::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the handle stays open during the call; `descriptor` receives a new allocation and
    // `owner` and `dacl` pointers into it.
    let status = unsafe {
        GetSecurityInfo(
            raw(file),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&raw mut owner),
            None,
            Some(&raw mut dacl),
            None,
            Some(&raw mut descriptor),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status.0 as i32));
    }
    let _memory = LocalMemory(descriptor.0);
    if owner.is_invalid() {
        return Err(io::Error::other("the object has no owner"));
    }
    let owner = sid_text(owner)?;
    if dacl.is_null() {
        return Ok(Security { owner, dacl: None });
    }
    let mut size = ACL_SIZE_INFORMATION::default();
    // SAFETY: `dacl` points into the descriptor, still allocated; `size` is writable for the
    // size passed.
    unsafe {
        GetAclInformation(
            dacl,
            (&raw mut size).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    }?;
    let mut entries = Vec::with_capacity(size.AceCount as usize);
    for index in 0..size.AceCount {
        let mut ace: *mut c_void = std::ptr::null_mut();
        // SAFETY: `index` is below the entry count of the list, still allocated.
        unsafe { GetAce(dacl, index, &raw mut ace) }?;
        // SAFETY: the call succeeded, so `ace` points to an entry that starts with its header.
        let header = unsafe { *ace.cast::<ACE_HEADER>() };
        entries.push(match header.AceType {
            ACCESS_ALLOWED_ACE_TYPE => {
                let sid = ace
                    .cast::<u8>()
                    .wrapping_add(offset_of!(ACCESS_ALLOWED_ACE, SidStart));
                Entry::Allow(sid_text(PSID(sid.cast()))?)
            }
            ACCESS_DENIED_ACE_TYPE => Entry::Deny,
            _ => Entry::Other,
        });
    }
    Ok(Security {
        owner,
        dacl: Some(entries),
    })
}

/// The access list of `path` in text form (`D:P(A;OICI;FA;;;S-...)`), protection and
/// inheritance flags included.
#[cfg(test)]
pub(super) fn access_list_text(path: &Path) -> io::Result<String> {
    use windows::Win32::Security::Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW;
    let file = OpenOptions::new()
        .access_mode(READ_CONTROL.0)
        .custom_flags(FLAG_BACKUP_SEMANTICS | FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the handle stays open during the call; `descriptor` receives a new allocation.
    let status = unsafe {
        GetSecurityInfo(
            raw(&file),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            Some(&raw mut descriptor),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status.0 as i32));
    }
    let _memory = LocalMemory(descriptor.0);
    let mut text = PWSTR::null();
    // SAFETY: `descriptor` is the allocation the previous call returned, still alive; `text`
    // receives a new string.
    unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &raw mut text,
            None,
        )
    }?;
    take_text(text)
}
