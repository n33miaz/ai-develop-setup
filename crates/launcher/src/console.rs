use std::io;

#[cfg(windows)]
#[allow(unsafe_code)]
mod win;

#[cfg(unix)]
use std::process::Child;

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
