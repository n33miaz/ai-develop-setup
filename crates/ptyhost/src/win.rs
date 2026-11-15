//! Windows system calls: the Job Object that ends a process tree, the named pipe only the current
//! user can open, and the probes that prove both.
//! Every `unsafe` block of the crate lives here and is listed in `docs/unsafe.md`.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
    SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicProcessIdList,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessHandleCount, OpenProcessToken,
};
use windows::core::{PCWSTR, PWSTR};

/// Exit code given to the processes of a terminated job.
const TERMINATED_EXIT_CODE: u32 = 1;

/// How many process ids one query of a job returns at most.
const LISTED_PROCESSES_MAX: usize = 256;

/// Same layout as `JOBOBJECT_BASIC_PROCESS_ID_LIST`, with room for the ids.
#[repr(C)]
struct ProcessIdList {
    assigned: u32,
    in_list: u32,
    ids: [usize; LISTED_PROCESSES_MAX],
}

/// A job whose processes all die when it is terminated or when its handle closes.
#[derive(Debug)]
pub struct JobObject {
    handle: OwnedHandle,
}

impl JobObject {
    pub fn kill_on_close() -> io::Result<Self> {
        // SAFETY: no security attributes and no name; the call only returns a new handle.
        let raw = unsafe { CreateJobObjectW(None, PCWSTR::null()) }?;
        // SAFETY: `raw` was just created, is valid and has no other owner.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw.0) };
        let job = Self { handle };

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `info` outlives the call and the size passed is its own size.
        unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }?;
        Ok(job)
    }

    /// Adds a process, and with it every process it starts from now on.
    pub fn assign(&self, process: RawHandle) -> io::Result<()> {
        // SAFETY: the job handle is owned by `self`; the caller's process handle is only read.
        unsafe { AssignProcessToJobObject(self.raw(), HANDLE(process)) }?;
        Ok(())
    }

    /// Ends every process in the job. Safe to call more than once.
    pub fn terminate(&self) {
        // SAFETY: the job handle is owned by `self` and stays open during the call.
        let _ = unsafe { TerminateJobObject(self.raw(), TERMINATED_EXIT_CODE) };
    }

    /// Ids of the processes still alive in the job.
    pub fn process_ids(&self) -> Vec<u32> {
        let mut list = ProcessIdList {
            assigned: 0,
            in_list: 0,
            ids: [0; LISTED_PROCESSES_MAX],
        };
        // SAFETY: `list` has the layout the call fills and the size passed is its own size.
        // A failure leaves `in_list` at zero or at the number of ids that did fit.
        let _ = unsafe {
            QueryInformationJobObject(
                Some(self.raw()),
                JobObjectBasicProcessIdList,
                (&raw mut list).cast(),
                size_of::<ProcessIdList>() as u32,
                None,
            )
        };
        let listed = (list.in_list as usize).min(LISTED_PROCESSES_MAX);
        list.ids[..listed].iter().map(|id| *id as u32).collect()
    }

    fn raw(&self) -> HANDLE {
        HANDLE(self.handle.as_raw_handle())
    }
}

/// How many handles this process holds open right now.
pub fn own_handle_count() -> io::Result<u32> {
    let mut count = 0_u32;
    // SAFETY: the pseudo handle of the current process is always valid; `count` outlives the call.
    unsafe { GetProcessHandleCount(GetCurrentProcess(), &raw mut count) }?;
    Ok(count)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessEntry {
    pub process_id: u32,
    pub parent_process_id: u32,
}

/// Every process alive in the system right now, with its parent.
pub fn process_table() -> io::Result<Vec<ProcessEntry>> {
    // SAFETY: the call only returns a new snapshot handle.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?;
    // SAFETY: `raw` was just created, is valid and has no other owner.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw.0) };
    let snapshot = HANDLE(snapshot.as_raw_handle());

    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut table = Vec::new();
    // SAFETY: the snapshot stays open for the whole loop; `entry` carries its own size.
    let mut more = unsafe { Process32FirstW(snapshot, &raw mut entry) }.is_ok();
    while more {
        table.push(ProcessEntry {
            process_id: entry.th32ProcessID,
            parent_process_id: entry.th32ParentProcessID,
        });
        // SAFETY: same as above.
        more = unsafe { Process32NextW(snapshot, &raw mut entry) }.is_ok();
    }
    Ok(table)
}

/// Memory the system allocated for the caller, released with `LocalFree` when dropped.
struct LocalMemory(*mut core::ffi::c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a call documented to be released with `LocalFree`
        // and is released only here.
        let _ = unsafe { LocalFree(Some(HLOCAL(self.0))) };
    }
}

/// Copies a system-allocated wide string and releases it.
fn take_text(text: PWSTR) -> io::Result<String> {
    let _memory = LocalMemory(text.0.cast());
    // SAFETY: the call that produced `text` succeeded, so it is a valid, terminated wide string.
    unsafe { text.to_string() }.map_err(io::Error::other)
}

/// The user this process runs as, as a security identifier in text form (`S-1-...`).
pub fn current_user_sid() -> io::Result<String> {
    let mut token = HANDLE::default();
    // SAFETY: the pseudo handle of the current process is always valid; `token` outlives the call.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) }?;
    // SAFETY: `token` was just opened, is valid and has no other owner.
    let token = unsafe { OwnedHandle::from_raw_handle(token.0) };

    // `TOKEN_USER` followed by the identifier it points to; `u64` keeps the alignment.
    let mut buffer = [0_u64; 32];
    let mut written = 0_u32;
    // SAFETY: the buffer is writable for the size passed and `written` outlives the call.
    unsafe {
        GetTokenInformation(
            HANDLE(token.as_raw_handle()),
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            size_of_val(&buffer) as u32,
            &raw mut written,
        )
    }?;
    // SAFETY: the call succeeded, so the buffer starts with an aligned, initialized `TOKEN_USER`.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text = PWSTR::null();
    // SAFETY: `sid` points into `buffer`, alive during the call; `text` receives a new string.
    unsafe { ConvertSidToStringSidW(sid, &raw mut text) }?;
    take_text(text)
}

/// A new instance of the named pipe `name` that only `user_sid` can open: the access list has
/// that single entry, so no other account of the machine reads, writes or even sees its state.
/// `first` refuses to start if someone else already holds a pipe with this name.
/// Must be called inside a Tokio runtime with I/O enabled.
pub fn create_user_pipe(name: &str, user_sid: &str, first: bool) -> io::Result<NamedPipeServer> {
    let rule: Vec<u16> = format!("D:P(A;;GA;;;{user_sid})")
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
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    // SAFETY: `attributes` is a valid `SECURITY_ATTRIBUTES` whose descriptor stays allocated
    // until after the call, which copies what it needs.
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(name, (&raw mut attributes).cast())
    }
}

/// The access list of a kernel object, in the system's text form (proof of who can open it).
pub fn access_list_text(handle: RawHandle) -> io::Result<String> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the caller's handle is only read; `descriptor` receives a new allocation.
    let status = unsafe {
        GetSecurityInfo(
            HANDLE(handle),
            SE_KERNEL_OBJECT,
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
    // SAFETY: `descriptor` was just returned and is still allocated; `text` receives a new string.
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
