//! Windows system calls: the Job Object that ends a process tree, and the probes that prove it.
//! Every `unsafe` block of the crate lives here and is listed in `docs/unsafe.md`.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicProcessIdList,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
use windows::core::PCWSTR;

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
