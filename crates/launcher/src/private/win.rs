//! Windows security calls for the control folder. Each block is a single call or the read of
//! data the previous call filled, with a `SAFETY` comment; see docs/unsafe.md.

use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree};
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
use windows::Win32::Storage::FileSystem::CreateDirectoryW;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCWSTR, PWSTR};

use super::{Entry, Identity, Security};

const READ_CONTROL: u32 = 0x0002_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;

/// Memory the system allocated for the caller, released with `LocalFree` when dropped.
struct LocalMemory(*mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a call documented to be released with `LocalFree`
        // and is released only here.
        let _ = unsafe { LocalFree(Some(HLOCAL(self.0))) };
    }
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

/// Creates the folder owned by the user, with a protected access list whose single entry, the
/// user's, is inherited by every file created inside.
pub(super) fn create_dir(path: &Path, local: &Identity) -> io::Result<()> {
    let rule: Vec<u16> = format!("O:{0}D:P(A;OICI;FA;;;{0})", local.user)
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
    let _memory = LocalMemory(descriptor.0);
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

/// The owner and access list of a file or folder, read from a handle that does not follow links.
pub(super) fn security(path: &Path) -> io::Result<Security> {
    let file = OpenOptions::new()
        .access_mode(READ_CONTROL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut owner = PSID::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the handle stays open during the call; `descriptor` receives a new allocation and
    // `owner` and `dacl` pointers into it.
    let status = unsafe {
        GetSecurityInfo(
            HANDLE(file.as_raw_handle()),
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
