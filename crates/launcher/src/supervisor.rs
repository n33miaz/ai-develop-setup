use std::{
    collections::VecDeque,
    fs::{File, TryLockError},
    io,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime},
};

use serde::Serialize;

use ads_private_dir::{ControlDir, Identity, Signal};

use crate::{Config, console};

/// File times can come from a coarser clock than the start time, or be rounded to the second,
/// so only a stop request older than this margin is taken as left over from an earlier run.
const STALE_SIGNAL_MARGIN: Duration = Duration::from_secs(2);

#[derive(Default, Serialize)]
struct ProcessState {
    pid: Option<u32>,
    restarts: u64,
    failures_in_window: usize,
    requested_restarts: u64,
    gave_up: bool,
    last_exit: Option<Exit>,
    last_error: Option<String>,
}

#[derive(Serialize)]
struct Exit {
    code: Option<i32>,
    status: String,
}

impl From<ExitStatus> for Exit {
    fn from(status: ExitStatus) -> Self {
        Self {
            code: status.code(),
            status: status.to_string(),
        }
    }
}

/// The desktop window as published: it is not restarted on its own, only replaced for a new
/// daemon, so it has no retry counters.
#[derive(Default, Serialize)]
struct ShellState {
    pid: Option<u32>,
    starts: u64,
    last_exit: Option<Exit>,
    last_error: Option<String>,
}

struct Process {
    command: Command,
    child: Option<Child>,
    failures: VecDeque<Instant>,
    state: ProcessState,
    attempted: bool,
    retry: Option<(Instant, Duration)>,
    terminate_on_drop: bool,
}

impl Process {
    fn new(command: Command, terminate_on_drop: bool) -> Self {
        Self {
            command,
            child: None,
            failures: VecDeque::new(),
            state: ProcessState::default(),
            attempted: false,
            retry: None,
            terminate_on_drop,
        }
    }

    fn tick(&mut self, config: &Config) -> io::Result<bool> {
        let expired = self.expire_failures(config, Instant::now());
        if let Some(child) = &mut self.child {
            if let Some(status) = child.try_wait()? {
                self.record_exit(status);
                self.schedule_retry(config, Instant::now());
                return Ok(true);
            }
            return Ok(expired);
        }
        if self.state.gave_up || self.retry.is_some_and(|(at, delay)| at.elapsed() < delay) {
            return Ok(expired);
        }
        if self.attempted {
            self.state.restarts = self.state.restarts.saturating_add(1);
        }
        self.attempted = true;
        #[cfg(test)]
        tests::CHILD_SPAWN_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
        match self.command.spawn() {
            Ok(child) => {
                self.state.pid = Some(child.id());
                self.child = Some(child);
            }
            Err(error) => {
                self.state.last_error = Some(error.to_string());
                self.schedule_retry(config, Instant::now());
            }
        }
        Ok(true)
    }

    fn schedule_retry(&mut self, config: &Config, now: Instant) {
        self.expire_failures(config, now);
        self.failures.push_back(now);
        self.state.failures_in_window = self.failures.len();
        self.state.gave_up = self.failures.len() >= config.crash_window_limit;
        let exponent = u32::try_from(self.failures.len() - 1).unwrap_or(u32::MAX);
        let factor = 2_u64.saturating_pow(exponent);
        let delay = config
            .restart_initial_ms
            .saturating_mul(factor)
            .min(config.restart_max_ms);
        self.retry = Some((now, Duration::from_millis(delay)));
    }

    fn expire_failures(&mut self, config: &Config, now: Instant) -> bool {
        let window = Duration::from_millis(config.crash_window_ms);
        while self
            .failures
            .front()
            .is_some_and(|at| now.duration_since(*at) >= window)
        {
            self.failures.pop_front();
        }
        let changed = self.state.failures_in_window != self.failures.len();
        self.state.failures_in_window = self.failures.len();
        changed
    }

    fn record_exit(&mut self, status: ExitStatus) {
        self.child = None;
        self.state.pid = None;
        self.state.last_exit = Some(status.into());
    }

    fn stop(&mut self) -> io::Result<()> {
        if let Some(child) = &mut self.child {
            let status = stop_child(child)?;
            self.record_exit(status);
        }
        Ok(())
    }

    fn restart(&mut self) -> io::Result<()> {
        self.stop()?;
        self.failures.clear();
        self.state.failures_in_window = 0;
        self.state.requested_restarts = self.state.requested_restarts.saturating_add(1);
        self.state.gave_up = false;
        self.retry = None;
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if self.terminate_on_drop {
            let _ = self.stop();
        }
    }
}

fn stop_child(child: &mut Child) -> io::Result<ExitStatus> {
    if let Some(status) = child.try_wait()? {
        return Ok(status);
    }
    if let Err(error) = child.kill()
        && child.try_wait()?.is_none()
    {
        return Err(error);
    }
    child.wait()
}

/// Asks the child to close, waits up to `timeout`, then forces it. Only a child this launcher
/// started and has not reaped gets here, so neither step can reach another process.
fn close_child(
    child: &mut Child,
    timeout: Duration,
    poll: Duration,
    request: impl FnOnce(&Child) -> io::Result<()>,
    report: impl FnOnce(String),
) -> io::Result<ExitStatus> {
    if let Some(status) = child.try_wait()? {
        return Ok(status);
    }
    let delivered = !timeout.is_zero()
        && match request(child) {
            Ok(()) => true,
            Err(error) => {
                report(format!("request shell close: {error}"));
                false
            }
        };
    if delivered {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            thread::sleep(poll.min(left));
        }
    }
    stop_child(child)
}

/// The desktop process opened for the current daemon, with cleanup on scope exit.
struct Window {
    child: Option<Child>,
    #[cfg(windows)]
    close_pipe: Option<io::PipeWriter>,
    timeout: Duration,
    poll: Duration,
    state: ShellState,
}

impl Window {
    fn new(config: &Config) -> Self {
        Self {
            child: None,
            #[cfg(windows)]
            close_pipe: None,
            timeout: Duration::from_millis(config.shell_close_timeout_ms),
            poll: Duration::from_millis(config.poll_ms),
            state: ShellState::default(),
        }
    }

    fn start(&mut self, command: &mut Command, bootstrap: impl Into<Stdio>) -> io::Result<()> {
        self.state.starts = self.state.starts.saturating_add(1);
        let result = self.spawn(command, bootstrap);
        match &result {
            Ok(()) => {
                self.state.pid = self.child.as_ref().map(Child::id);
                self.state.last_error = None;
            }
            Err(error) => self.state.last_error = Some(error.to_string()),
        }
        result
    }

    fn spawn(&mut self, command: &mut Command, bootstrap: impl Into<Stdio>) -> io::Result<()> {
        #[cfg(windows)]
        {
            let (reader, writer) = io::pipe()?;
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command
                .creation_flags(CREATE_NO_WINDOW)
                .stderr(reader)
                .env("ADS_SHELL_CLOSE_PIPE", "1");
            let child = start_shell(command, bootstrap);
            command.stderr(Stdio::null());
            self.child = Some(child?);
            self.close_pipe = Some(writer);
        }
        #[cfg(unix)]
        {
            self.child = Some(start_shell(command, bootstrap)?);
        }
        Ok(())
    }

    fn close(&mut self, report: impl FnOnce(String)) -> io::Result<()> {
        if let Some(child) = &mut self.child {
            #[cfg(windows)]
            let request = |_: &Child| {
                use io::Write;
                self.close_pipe
                    .take()
                    .ok_or_else(|| io::Error::other("missing shell close pipe"))?
                    .write_all(b"Q")
            };
            #[cfg(unix)]
            let request = console::request_close;
            let status = close_child(child, self.timeout, self.poll, request, report)?;
            self.exited(status);
        }
        Ok(())
    }

    /// Records the exit of the window, whether it closed by itself or on request.
    fn exited(&mut self, status: ExitStatus) {
        self.child = None;
        #[cfg(windows)]
        {
            self.close_pipe = None;
        }
        self.state.pid = None;
        self.state.last_exit = Some(status.into());
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Err(error) = self.close(|message| eprintln!("ads-launcher: {message}")) {
            eprintln!("ads-launcher: close shell: {error}");
        }
    }
}

fn detach(host: &mut Command) {
    host.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        host.creation_flags(DETACHED_PROCESS);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        host.process_group(0);
    }
}

fn open_lock(control: &ControlDir, name: &str) -> io::Result<File> {
    let lock = control.lock_file(name)?;
    lock.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::AlreadyExists,
            "another launcher is already running for this installation",
        ),
        TryLockError::Error(error) => error,
    })?;
    Ok(lock)
}

fn write_state(
    control: &ControlDir,
    name: &str,
    daemon: &Process,
    host: &Process,
    shell: Option<&ShellState>,
    stopped: bool,
    publication: &Publication,
) -> io::Result<()> {
    #[derive(Serialize)]
    struct State<'a> {
        launcher_pid: u32,
        stopped: bool,
        stop_pending_at_startup: bool,
        last_error: Option<&'a str>,
        state_write_error: Option<&'a str>,
        daemon: &'a ProcessState,
        ptyhost: &'a ProcessState,
        shell: Option<&'a ShellState>,
    }
    let bytes = serde_json::to_vec(&State {
        launcher_pid: std::process::id(),
        stopped,
        stop_pending_at_startup: publication.stop_pending_at_startup,
        last_error: publication.last_error.as_deref(),
        state_write_error: publication.state_write_error.as_deref(),
        daemon: &daemon.state,
        ptyhost: &host.state,
        shell,
    })
    .map_err(io::Error::other)?;
    control.publish(name, &bytes)
}

#[derive(Default)]
struct Publication {
    dirty: bool,
    stop_pending_at_startup: bool,
    last_error: Option<String>,
    state_write_error: Option<String>,
}

impl Publication {
    fn attempt<T>(&mut self, operation: &str, result: io::Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.report(format!("{operation}: {error}"));
                None
            }
        }
    }

    fn report(&mut self, message: String) {
        if self.last_error.as_deref() != Some(&message) {
            eprintln!("ads-launcher: {message}");
            self.last_error = Some(message);
            self.dirty = true;
        }
    }

    fn close_shell(&mut self, window: &mut Window) -> bool {
        self.dirty |= window.child.is_some();
        let result = window.close(|message| self.report(message));
        self.attempt("close shell", result).is_some()
    }

    /// `Some(true)` when the signal was accepted; a foreign signal file is reported and ignored.
    fn signal(&mut self, operation: &str, control: &ControlDir, name: &str) -> Option<bool> {
        self.record_signal(operation, control, name, control.signal(name, true))
    }

    fn record_signal(
        &mut self,
        operation: &str,
        control: &ControlDir,
        name: &str,
        result: io::Result<Signal>,
    ) -> Option<bool> {
        match self.attempt(
            &format!("{operation} {}", control.path().join(name).display()),
            result,
        )? {
            Signal::Accepted | Signal::Stale => Some(true),
            Signal::Absent => Some(false),
            Signal::Foreign => {
                self.report(format!(
                    "{operation}: ignored {}, owned by or open to another account",
                    control.path().join(name).display()
                ));
                Some(false)
            }
        }
    }
}

pub(crate) fn run(
    config: &Config,
    control: &Path,
    daemon: Command,
    host: Command,
    shell: Option<Command>,
    started: SystemTime,
) -> io::Result<()> {
    run_with_interrupt(
        config,
        control,
        daemon,
        host,
        shell,
        started,
        console::console_stop_requested,
    )
}

#[cfg(test)]
thread_local! {
    /// Stands in for the identity of the session, which a test cannot elevate.
    static SESSION: std::cell::RefCell<Option<Identity>> = const { std::cell::RefCell::new(None) };
}

fn session_identity() -> io::Result<Identity> {
    #[cfg(test)]
    if let Some(local) = SESSION.with(|session| session.borrow().clone()) {
        return Ok(local);
    }
    Identity::current()
}

fn run_with_interrupt(
    config: &Config,
    control: &Path,
    mut daemon: Command,
    mut host: Command,
    mut shell: Option<Command>,
    started: SystemTime,
    mut interrupted: impl FnMut() -> bool,
) -> io::Result<()> {
    let local = session_identity()?;
    // The daemon and the host would create files the next run refuses; stop before any of
    // them, or the control folder, exists.
    if !local.creates_as_user() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            ads_private_dir::ELEVATED_SESSION,
        ));
    }
    let control = ControlDir::open(control, &local)?;
    let _lock = open_lock(&control, &config.lock_file)?;
    // The daemon opens its lock by path; checking it here refuses one left open to other
    // accounts, which could hold it and keep every daemon of this installation from starting.
    drop(control.lock_file(crate::DAEMON_LOCK_FILE)?);
    let state = config.state_file.as_str();
    let stop = config.stop_file.as_str();
    let restart = config.restart_file.as_str();
    detach(&mut host);
    daemon.stdin(Stdio::null());
    if shell.is_some() {
        daemon.stdout(Stdio::piped());
    }
    let mut daemon = Process::new(daemon, true);
    let mut host = Process::new(host, false);
    // Declared after the daemon so that, on any exit, the window closes before it.
    let mut window = Window::new(config);

    // Only a request clearly older than this run is left over from an earlier one.
    let stale_before = started
        .checked_sub(STALE_SIGNAL_MARGIN)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut publication = Publication::default();
    let startup_stop = control.dated_signal(stop, stale_before, true);
    // Only a leftover that could not be handled here keeps the date rule while running;
    // any other request that appears later is honored, whatever its date.
    let mut leftover_stop = startup_stop.is_err();
    publication.stop_pending_at_startup =
        publication.record_signal("discard stale stop signal", &control, stop, startup_stop)
            == Some(true);
    // Publish before starting children so an unusable state path fails early.
    let desktop = shell.is_some();
    if let Err(error) = write_state(
        &control,
        state,
        &daemon,
        &host,
        desktop.then_some(&window.state),
        false,
        &publication,
    ) {
        publication.signal("consume stop signal", &control, stop);
        return Err(error);
    }
    let mut stopping = false;
    let mut restarting = false;
    loop {
        if !stopping {
            let console_stop = interrupted();
            let file_stop = if leftover_stop {
                control.dated_signal(stop, stale_before, false)
            } else {
                control.signal(stop, true)
            };
            leftover_stop &= file_stop.is_err();
            let stale = matches!(file_stop, Ok(Signal::Stale));
            let consumed =
                publication.record_signal("consume stop signal", &control, stop, file_stop);
            if stale {
                // Kept apart from `last_error`, which the next error replaces.
                publication.stop_pending_at_startup = true;
                publication.report(format!(
                    "discard stale stop signal: removed {}, left from before this run",
                    control.path().join(stop).display()
                ));
            }
            stopping = console_stop || (!stale && consumed == Some(true));
        }
        let stopped = if stopping {
            let daemon_stopped = publication.attempt("stop daemon", daemon.stop()).is_some();
            let window_closed = publication.close_shell(&mut window);
            let restart_cleared = publication
                .signal("clear restart signal", &control, restart)
                .is_some();
            publication.attempt("check host exit", host.tick_exit_only());
            publication.dirty = true;
            publication.signal("consume stop signal", &control, stop);
            daemon_stopped && window_closed && restart_cleared
        } else {
            restarting |=
                publication.signal("consume restart signal", &control, restart) == Some(true);
            if restarting
                && publication
                    .attempt("restart daemon", daemon.restart())
                    .is_some()
            {
                restarting = false;
                publication.dirty = true;
            }
            let host_changed = publication.attempt("check host", host.tick(config)) == Some(true);
            let daemon_changed = !restarting
                && publication.attempt("check daemon", daemon.tick(config)) == Some(true);
            if let Some(window_process) = &mut window.child
                && let Some(status) = publication
                    .attempt("check shell", Child::try_wait(window_process))
                    .flatten()
            {
                window.exited(status);
                publication.dirty = true;
            }
            if daemon.state.gave_up {
                // No daemon will follow to replace the window: it would stay disconnected.
                publication.close_shell(&mut window);
            }
            if let Some(shell) = &mut shell
                && let Some(daemon_output) =
                    daemon.child.as_mut().and_then(|child| child.stdout.take())
                && publication.close_shell(&mut window)
            {
                publication.attempt("start shell", window.start(shell, daemon_output));
                publication.dirty = true;
            }
            publication.dirty |= host_changed || daemon_changed;
            false
        };
        if publication.dirty {
            match write_state(
                &control,
                state,
                &daemon,
                &host,
                desktop.then_some(&window.state),
                stopped,
                &publication,
            ) {
                Ok(()) => {
                    publication.dirty = false;
                    if stopped {
                        return Ok(());
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    if publication.state_write_error.as_deref() != Some(&message) {
                        eprintln!("ads-launcher: publish state: {message}");
                        publication.state_write_error = Some(message);
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(config.poll_ms));
    }
}

fn start_shell(command: &mut Command, bootstrap: impl Into<Stdio>) -> io::Result<Child> {
    let child = command.stdin(bootstrap).spawn();
    command.stdin(Stdio::null());
    child
}

impl Process {
    fn tick_exit_only(&mut self) -> io::Result<()> {
        if let Some(child) = &mut self.child
            && let Some(status) = child.try_wait()?
        {
            self.record_exit(status);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
pub(crate) mod tests;
