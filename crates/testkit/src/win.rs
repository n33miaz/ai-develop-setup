//! The one system call of the kit: closing this process, or its token, to every other process.
//! Every `unsafe` block of the crate lives here and is listed in `docs/unsafe.md`.
#![allow(unsafe_code)]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_KERNEL_OBJECT,
    SetSecurityInfo,
};
use windows::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, TOKEN_ACCESS_MASK,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PCWSTR;

/// Memory the system allocated for the caller, released with `LocalFree` when dropped.
struct LocalMemory(*mut core::ffi::c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a call documented to be released with `LocalFree`
        // and is released only here.
        let _ = unsafe { LocalFree(Some(HLOCAL(self.0))) };
    }
}

/// What `restrict_own_process` closes to every other process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restricted {
    /// The process object: opening it to query fails.
    Process,
    /// Its token: opening it to read the user fails.
    Token,
}

/// Replaces the access list of this process, or of its token, with one that lets in only the
/// system account. Other processes of the same user can then no longer open it, as with a
/// server of another account; handles opened before keep working. Run it only in a child
/// process made for that (`HelperProcess`), never in the test process itself.
pub fn restrict_own_process(part: Restricted) -> io::Result<()> {
    /// `WRITE_DAC`: the only access needed to replace the list.
    const WRITE_DAC: TOKEN_ACCESS_MASK = TOKEN_ACCESS_MASK(0x0004_0000);

    let rule: Vec<u16> = "D:P(A;;GA;;;SY)".encode_utf16().chain([0]).collect();
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
    let mut present = windows::core::BOOL::default();
    let mut defaulted = windows::core::BOOL::default();
    let mut list: *mut ACL = std::ptr::null_mut();
    // SAFETY: `descriptor` is allocated until `_memory` drops; `list` receives a pointer into it.
    unsafe {
        GetSecurityDescriptorDacl(
            descriptor,
            &raw mut present,
            &raw mut list,
            &raw mut defaulted,
        )
    }?;

    // SAFETY: the call only returns the pseudo handle of the current process, always valid.
    let current = unsafe { GetCurrentProcess() };
    let token;
    let target = match part {
        Restricted::Process => current,
        Restricted::Token => {
            let mut raw = HANDLE::default();
            // SAFETY: the pseudo handle is always valid; `raw` outlives the call.
            unsafe { OpenProcessToken(current, WRITE_DAC, &raw mut raw) }?;
            // SAFETY: `raw` was just opened, is valid and has no other owner.
            token = unsafe { OwnedHandle::from_raw_handle(raw.0) };
            HANDLE(token.as_raw_handle())
        }
    };
    // SAFETY: `target` stays open during the call; `list` points into `descriptor`, still
    // allocated, and the call copies it.
    let status = unsafe {
        SetSecurityInfo(
            target,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(list.cast_const()),
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status.0 as i32));
    }
    Ok(())
}
