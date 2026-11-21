use std::io;

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
