#[cfg(all(test, windows))]
#[path = "../../src/main.rs"]
mod daemon;
#[cfg(all(test, windows))]
#[path = "../common/host_binary.rs"]
mod host_binary;
#[cfg(all(test, windows))]
#[path = "../common/temp.rs"]
mod temp;
#[cfg(all(test, windows))]
mod windows;

#[cfg(all(test, windows))]
fn main() -> std::process::ExitCode {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--instance-lock")) {
        return daemon::run(windows::MemoryVault::default());
    }
    windows::daemon_serves_queue_and_approvals();
    std::process::ExitCode::SUCCESS
}

#[cfg(not(all(test, windows)))]
fn main() {}
