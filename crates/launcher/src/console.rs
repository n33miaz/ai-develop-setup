use std::io;
use std::time::SystemTime;

#[cfg(windows)]
#[allow(unsafe_code)]
mod win;

#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process};
#[cfg(unix)]
use std::process::Child;

/// Sends SIGTERM to a child the caller still holds unreaped.
#[cfg(unix)]
pub(crate) fn request_close(child: &Child) -> io::Result<()> {
    Ok(kill_process(Pid::from_child(child), Signal::TERM)?)
}

pub(crate) fn install_console_handler() -> io::Result<()> {
    #[cfg(windows)]
    win::install_console_handler()?;
    Ok(())
}

#[cfg(all(test, windows))]
pub(crate) use win::generate_console_interrupt;

pub(crate) fn console_stop_requested() -> bool {
    #[cfg(windows)]
    return win::console_stop_requested();
    #[cfg(unix)]
    false
}

/// When the launcher process started: the creation time on Windows, the first reading in
/// `main` elsewhere.
pub(crate) fn process_started() -> io::Result<SystemTime> {
    #[cfg(windows)]
    return win::process_started();
    #[cfg(unix)]
    Ok(SystemTime::now())
}
