//! Test-only helpers, used as a dev-dependency and never by a product binary: a child process
//! of the test binary that plays a server, started and always reaped by `HelperProcess`.

use std::io::{self, BufRead};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::{Restricted, restrict_own_process};

/// This test binary started again to run one ignored helper test, with one variable telling it
/// what to do. Its output is read line by line on a thread of its own, so waits have a deadline
/// and the child never blocks on a full pipe. Dropping it kills the child and waits for it.
pub struct HelperProcess {
    child: Child,
    lines: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}

impl HelperProcess {
    /// Runs `test` alone (`--exact`, `--ignored`) in a new process with `variable=value`.
    pub fn start(test: &str, variable: &str, value: &str) -> io::Result<Self> {
        let mut child = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                test,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(variable, value)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let (sender, lines) = channel();
        let reader = std::thread::spawn(move || {
            for line in io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            lines,
            reader: Some(reader),
        })
    }

    /// Waits up to `timeout` for a line holding `marker` and returns what follows it on that
    /// line, up to the next `@@`. `None` when the time runs out or the output ends first.
    pub fn wait_for(&mut self, marker: &str, timeout: Duration) -> Option<String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    if let Some(at) = line.find(marker) {
                        let rest = &line[at + marker.len()..];
                        return Some(rest.split("@@").next().unwrap_or("").to_owned());
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    /// Waits up to `timeout` for the child to exit on its own. `None` means it had to be killed.
    pub fn finish(mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        // Killing an exited child fails harmlessly; the wait reaps it either way, and the end of
        // its output lets the reader thread finish.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARIABLE: &str = "ADS_TESTKIT_HELPER";

    /// Not a test: the child of the tests below. Prints a marker, then sleeps or exits.
    #[test]
    #[ignore = "helper run in a child process by the tests of this file"]
    fn helper() {
        let what = std::env::var(VARIABLE).expect("helper only");
        println!("@@ready=yes@@");
        if what == "hang" {
            std::thread::sleep(Duration::from_secs(600));
        }
    }

    #[test]
    fn a_marker_is_read_and_a_finished_child_is_reaped() {
        let mut child = HelperProcess::start("tests::helper", VARIABLE, "exit").unwrap();
        assert_eq!(
            child.wait_for("@@ready=", Duration::from_secs(20)),
            Some("yes".to_owned())
        );
        assert!(child.finish(Duration::from_secs(20)).unwrap().success());
    }

    #[test]
    fn a_missing_marker_times_out_and_the_child_is_killed() {
        let mut child = HelperProcess::start("tests::helper", VARIABLE, "hang").unwrap();
        let started = Instant::now();
        assert_eq!(
            child.wait_for("@@never@@", Duration::from_millis(500)),
            None
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        let id = child.child.id();
        drop(child);
        // The process is gone from the system's list.
        if cfg!(windows) {
            let listed = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {id}"), "/NH"])
                .output()
                .unwrap();
            let listed = String::from_utf8_lossy(&listed.stdout);
            assert!(!listed.contains(&format!(" {id} ")), "{listed}");
        }
    }
}
