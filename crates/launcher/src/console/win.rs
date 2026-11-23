use std::io;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static STOP_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "system" fn console_control(event: u32) -> windows::core::BOOL {
    use windows::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};
    let stopping = matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT);
    if stopping {
        STOP_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    windows::core::BOOL::from(stopping)
}

pub(super) fn install_console_handler() -> io::Result<()> {
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    // SAFETY: the callback has static lifetime and touches only an atomic flag.
    unsafe { SetConsoleCtrlHandler(Some(console_control), true) }.map_err(io::Error::other)
}

pub(super) fn console_stop_requested() -> bool {
    STOP_REQUESTED.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
pub(crate) fn generate_console_interrupt() -> io::Result<()> {
    use windows::Win32::System::Console::{
        CTRL_C_EVENT, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
    };
    // SAFETY: this resets the inherited ignore flag only in the isolated test process.
    unsafe { SetConsoleCtrlHandler(None, false) }.map_err(io::Error::other)?;
    // SAFETY: the test starts this process in a new console with no unrelated clients.
    unsafe { GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) }.map_err(io::Error::other)
}

/// When this process was created, read from the system rather than from the first line of `main`.
pub(super) fn process_started() -> io::Result<SystemTime> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: the current process pseudo-handle is valid and each output is writable.
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    }
    .map_err(io::Error::other)?;
    let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    let unix_ticks = ticks
        .checked_sub(116_444_736_000_000_000)
        .ok_or_else(|| io::Error::other("invalid launcher creation time"))?;
    Ok(UNIX_EPOCH
        + Duration::new(
            unix_ticks / 10_000_000,
            (unix_ticks % 10_000_000) as u32 * 100,
        ))
}
