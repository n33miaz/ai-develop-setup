//! Windows system calls: the Job Object that ends a process tree, the named pipe only the current
//! user can open, the check a client makes of who serves a pipe, and the probes that prove them.
//! Every `unsafe` block of the crate lives here and is listed in `docs/unsafe.md`.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows::Win32::Foundation::{
    ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
    SE_KERNEL_OBJECT,
};
use windows::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetTokenInformation, OWNER_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_INFORMATION_CLASS, TOKEN_QUERY,
    TokenOwner, TokenUser,
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
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessHandleCount, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
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

/// Keeps the identity of a process across exit and PID reuse.
#[derive(Debug)]
pub struct ProcessProbe {
    id: u32,
    handle: OwnedHandle,
}

impl ProcessProbe {
    pub fn open(id: u32) -> io::Result<Self> {
        // SAFETY: the call opens a non-inheritable handle with wait access only.
        let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, id) }?;
        // SAFETY: `raw` was just opened, is valid and has no other owner.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw.0) };
        Ok(Self { id, handle })
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn is_alive(&self) -> io::Result<bool> {
        // SAFETY: the owned handle stays valid during the zero-timeout wait.
        match unsafe { WaitForSingleObject(HANDLE(self.handle.as_raw_handle()), 0) } {
            WAIT_TIMEOUT => Ok(true),
            WAIT_OBJECT_0 => Ok(false),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            status => Err(io::Error::other(format!(
                "unexpected process wait status: {status:?}"
            ))),
        }
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

/// A security identifier in text form (`S-1-...`).
fn sid_text(sid: PSID) -> io::Result<String> {
    let mut text = PWSTR::null();
    // SAFETY: the caller keeps the memory `sid` points to alive during the call; `text`
    // receives a new string.
    unsafe { ConvertSidToStringSidW(sid, &raw mut text) }?;
    take_text(text)
}

/// The token of a process, opened only to be read.
fn process_token(process: HANDLE) -> io::Result<OwnedHandle> {
    let mut token = HANDLE::default();
    // SAFETY: the caller's process handle is only read; `token` outlives the call.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) }?;
    // SAFETY: `token` was just opened, is valid and has no other owner.
    Ok(unsafe { OwnedHandle::from_raw_handle(token.0) })
}

/// The identifier a token names in `class`: its user (`TokenUser`) or the owner it gives the
/// objects it creates (`TokenOwner`).
fn token_sid(token: &OwnedHandle, class: TOKEN_INFORMATION_CLASS) -> io::Result<String> {
    if class != TokenUser && class != TokenOwner {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
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

fn own_token() -> io::Result<OwnedHandle> {
    // SAFETY: the call only returns the pseudo handle of the current process, always valid.
    process_token(unsafe { GetCurrentProcess() })
}

/// The user this process runs as, as a security identifier in text form (`S-1-...`).
pub fn current_user_sid() -> io::Result<String> {
    token_sid(&own_token()?, TokenUser)
}

/// Who this process is, to compare with whoever answers on the other end of a pipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalUser {
    /// The user of the token.
    pub user: String,
    /// The owner the token gives the objects it creates: the user, or the administrators group
    /// for an elevated process under that system policy.
    pub default_owner: String,
}

pub fn local_user() -> io::Result<LocalUser> {
    let token = own_token()?;
    Ok(LocalUser {
        user: token_sid(&token, TokenUser)?,
        default_owner: token_sid(&token, TokenOwner)?,
    })
}

/// Who answers on a pipe this process opened as a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeServer {
    /// The user of the process that holds the server end.
    pub process_user: String,
    /// The owner of the pipe object, set by whoever created the pipe's name.
    pub object_owner: String,
}

/// The queries that read who serves a pipe, in the order they run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeQuery {
    /// The id of the process that holds the server end.
    ServerProcess,
    /// Opening that process.
    OpenProcess,
    /// Opening its token.
    OpenToken,
    /// Reading the user of the token.
    ProcessUser,
    /// Reading the owner of the pipe object.
    ObjectOwner,
}

/// Which query failed, and why. Carried inside the `io::Error` that `pipe_server` returns.
#[derive(Debug)]
pub struct PipeQueryFailed {
    pub query: PipeQuery,
    pub source: io::Error,
}

impl std::fmt::Display for PipeQueryFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.query, self.source)
    }
}

impl std::error::Error for PipeQueryFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl PipeQueryFailed {
    /// The failed query, when `error` came from `pipe_server`.
    pub fn of(error: &io::Error) -> Option<PipeQuery> {
        error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<Self>())
            .map(|failed| failed.query)
    }
}

/// Reads who serves the pipe behind `pipe`, a client handle opened with read access.
/// A normal account is denied another user's process or token, so a stranger's server comes
/// back as an error, which names the query that failed (`PipeQueryFailed::of`).
pub fn pipe_server(pipe: RawHandle) -> io::Result<PipeServer> {
    pipe_server_after(pipe, |_| Ok(()))
}

/// `pipe_server` calling `after` once each query has succeeded; an error from `after` stops it
/// there, dropping what the query returned, as if it had failed. Only tests pass anything but `Ok`.
fn pipe_server_after(
    pipe: RawHandle,
    mut after: impl FnMut(PipeQuery) -> io::Result<()>,
) -> io::Result<PipeServer> {
    let mut process_id = 0_u32;
    // SAFETY: the caller's handle is only read; `process_id` outlives the call.
    let found = unsafe { GetNamedPipeServerProcessId(HANDLE(pipe), &raw mut process_id) };
    checked(
        &mut after,
        PipeQuery::ServerProcess,
        found.map_err(io::Error::from),
    )?;
    // SAFETY: the call only returns a new handle, with the least access that reads a token.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
        .map(|raw| {
            // SAFETY: `raw` was just opened, is valid and has no other owner.
            unsafe { OwnedHandle::from_raw_handle(raw.0) }
        })
        .map_err(io::Error::from);
    let process = checked(&mut after, PipeQuery::OpenProcess, process)?;
    let token = process_token(HANDLE(process.as_raw_handle()));
    let token = checked(&mut after, PipeQuery::OpenToken, token)?;
    let process_user = token_sid(&token, TokenUser);
    let process_user = checked(&mut after, PipeQuery::ProcessUser, process_user)?;
    let object_owner = checked(&mut after, PipeQuery::ObjectOwner, object_owner_sid(pipe))?;
    Ok(PipeServer {
        process_user,
        object_owner,
    })
}

/// The result of one query of `pipe_server_after`, with the query named in its error.
fn checked<T>(
    after: &mut impl FnMut(PipeQuery) -> io::Result<()>,
    query: PipeQuery,
    result: io::Result<T>,
) -> io::Result<T> {
    let wrap = |source: io::Error| io::Error::new(source.kind(), PipeQueryFailed { query, source });
    let value = result.map_err(wrap)?;
    after(query).map_err(wrap)?;
    Ok(value)
}

/// The owner of a kernel object, as a security identifier in text form.
fn object_owner_sid(handle: RawHandle) -> io::Result<String> {
    let mut owner = PSID::default();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the caller's handle is only read; `descriptor` receives a new allocation and
    // `owner` a pointer into it.
    let status = unsafe {
        GetSecurityInfo(
            HANDLE(handle),
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&raw mut owner),
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
    if owner.is_invalid() {
        return Err(io::Error::other("the object has no owner"));
    }
    sid_text(owner)
}

/// The decision: the server process runs as this user and the pipe's name was created by this
/// user (owned by the user, or by the owner this user's objects get). Either part alone leaves
/// a gap: a process id can be reused after the server exits, and an object owner says nothing
/// about who holds the server end now.
pub fn is_own_server(local: &LocalUser, server: &PipeServer) -> bool {
    !local.user.is_empty()
        && server.process_user == local.user
        && (server.object_owner == local.user || server.object_owner == local.default_owner)
}

/// Confirms that the pipe behind the client handle `pipe` is served by the current user.
/// A failure to read who serves it is a refusal too.
pub fn check_pipe_server(pipe: RawHandle) -> io::Result<()> {
    check_pipe_server_as(&local_user()?, pipe)
}

/// `check_pipe_server` against a given identity, with the real queries of the pipe. Only tests
/// pass another identity, to have a real pipe refused without a second account.
pub(crate) fn check_pipe_server_as(local: &LocalUser, pipe: RawHandle) -> io::Result<()> {
    if is_own_server(local, &pipe_server(pipe)?) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the pipe is not served by the current user",
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "S-1-5-21-1000-2000-3000-1001";
    const OTHER: &str = "S-1-5-21-1000-2000-3000-1002";
    const ADMINISTRATORS: &str = "S-1-5-32-544";

    fn me(default_owner: &str) -> LocalUser {
        LocalUser {
            user: ME.to_owned(),
            default_owner: default_owner.to_owned(),
        }
    }

    fn server(process_user: &str, object_owner: &str) -> PipeServer {
        PipeServer {
            process_user: process_user.to_owned(),
            object_owner: object_owner.to_owned(),
        }
    }

    #[test]
    fn a_pipe_served_and_owned_by_the_user_is_accepted() {
        assert!(is_own_server(&me(ME), &server(ME, ME)));
        // An elevated process names the administrators group as the owner of what it creates.
        assert!(is_own_server(
            &me(ADMINISTRATORS),
            &server(ME, ADMINISTRATORS)
        ));
        assert!(is_own_server(&me(ADMINISTRATORS), &server(ME, ME)));
    }

    #[test]
    fn a_pipe_served_by_another_user_is_refused() {
        assert!(!is_own_server(&me(ME), &server(OTHER, ME)));
        assert!(!is_own_server(&me(ME), &server(OTHER, OTHER)));
    }

    #[test]
    fn another_administrators_pipe_is_refused_by_an_elevated_client() {
        // Both elevated: the pipe is owned by the administrators group, which is also the
        // client's default owner; the process user still differs.
        assert!(!is_own_server(
            &me(ADMINISTRATORS),
            &server(OTHER, ADMINISTRATORS)
        ));
    }

    #[test]
    fn a_pipe_whose_name_another_user_created_is_refused() {
        assert!(!is_own_server(&me(ME), &server(ME, OTHER)));
        assert!(!is_own_server(&me(ME), &server(ME, ADMINISTRATORS)));
        assert!(!is_own_server(&me(ADMINISTRATORS), &server(ME, OTHER)));
    }

    #[test]
    fn identifiers_are_compared_whole() {
        assert!(!is_own_server(
            &me(ME),
            &server("S-1-5-21-1000-2000-3000-100", ME)
        ));
        assert!(!is_own_server(&me(ME), &server(&ME.to_lowercase(), ME)));
        let nobody = LocalUser {
            user: String::new(),
            default_owner: String::new(),
        };
        assert!(!is_own_server(&nobody, &server("", "")));
    }

    #[test]
    fn a_handle_that_is_not_a_pipe_is_refused() {
        use std::os::windows::io::AsRawHandle;

        let file = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
        assert!(pipe_server(file.as_raw_handle()).is_err());
        assert!(check_pipe_server(file.as_raw_handle()).is_err());
    }

    #[test]
    fn the_local_user_reads_the_token() {
        let local = local_user().unwrap();
        assert_eq!(local.user, current_user_sid().unwrap());
        assert!(local.default_owner.starts_with("S-1-"), "{local:?}");
    }

    const QUERIES: [PipeQuery; 5] = [
        PipeQuery::ServerProcess,
        PipeQuery::OpenProcess,
        PipeQuery::OpenToken,
        PipeQuery::ProcessUser,
        PipeQuery::ObjectOwner,
    ];
    /// Failures per query; a handle kept by each one would raise the count by this much.
    const FAILURES: u32 = 200;

    /// A failure right after each query refuses the pipe, names that query, and releases what
    /// the queries before it opened. Other tests of this binary run alongside and open handles
    /// of their own, so the margin is half of what one leaked handle per failure would add.
    #[tokio::test]
    async fn a_failure_after_each_query_refuses_and_releases() {
        use std::os::windows::io::AsRawHandle;

        let endpoint = format!(r"\\.\pipe\ads-ptyhost-win-unit-{}", std::process::id());
        let _server = create_user_pipe(&endpoint, &current_user_sid().unwrap(), true).unwrap();
        let client = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&endpoint)
            .unwrap();
        let pipe = client.as_raw_handle();
        let fail_after = |failing: PipeQuery| {
            let error = pipe_server_after(pipe, |done| {
                if done == failing {
                    Err(io::Error::other("injected"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(PipeQueryFailed::of(&error), Some(failing), "{error}");
        };
        QUERIES.into_iter().for_each(fail_after);

        let before = own_handle_count().unwrap();
        for _ in 0..FAILURES {
            QUERIES.into_iter().for_each(fail_after);
        }
        let after = own_handle_count().unwrap();
        assert!(
            after < before + FAILURES / 2,
            "handle count grew from {before} to {after}"
        );
    }

    /// Every query succeeds on the host's own pipe and reads this user; the refusal for another
    /// identity comes from the comparison, not from a failed query.
    #[tokio::test]
    async fn a_refusal_for_another_identity_comes_from_the_comparison() {
        use std::os::windows::io::AsRawHandle;

        let endpoint = format!(r"\\.\pipe\ads-ptyhost-win-whole-{}", std::process::id());
        let _server = create_user_pipe(&endpoint, &current_user_sid().unwrap(), true).unwrap();
        let client = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&endpoint)
            .unwrap();
        let local = local_user().unwrap();
        let read = pipe_server(client.as_raw_handle()).unwrap();
        assert_eq!(read.process_user, local.user);
        assert!(
            read.object_owner == local.user || read.object_owner == local.default_owner,
            "{read:?} {local:?}"
        );
        assert!(check_pipe_server(client.as_raw_handle()).is_ok());

        let error = check_pipe_server_as(&me(OTHER), client.as_raw_handle()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(PipeQueryFailed::of(&error), None, "{error}");
    }
}
