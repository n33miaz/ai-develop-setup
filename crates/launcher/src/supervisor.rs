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
pub(crate) mod tests {
    use super::*;
    use ads_private_dir as private;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;

    fn run(
        config: &Config,
        control: &Path,
        daemon: Command,
        host: Command,
        shell: Option<Command>,
    ) -> io::Result<()> {
        super::run(config, control, daemon, host, shell, SystemTime::now())
    }

    pub(crate) struct TestDir(pub(crate) PathBuf);

    impl TestDir {
        pub(crate) fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target")
                .join(format!("launcher-test-{}-{stamp}", std::process::id()));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            private::create_dir(&path, &Identity::current().unwrap()).unwrap();
            Self(path)
        }
    }

    fn config() -> Config {
        let mut config = Config::load(None).unwrap();
        config.poll_ms = 5;
        config.restart_initial_ms = 20;
        config.restart_max_ms = 40;
        config.crash_window_ms = 600_000;
        config.crash_window_limit = 3;
        config.shell_close_timeout_ms = 200;
        config
    }

    fn fixture(stop: Option<&Path>) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "supervisor::tests::child_fixture"]);
        if let Some(stop) = stop {
            command.env("ADS_LAUNCHER_TEST_RELEASE", stop);
        }
        command.stdout(Stdio::null()).stderr(Stdio::null());
        command
    }

    #[test]
    fn child_fixture() {
        if let Some(path) = std::env::var_os("ADS_LAUNCHER_TEST_RELEASE") {
            wait_for_release(Path::new(&path));
        }
    }

    const BOOTSTRAP_SENTINEL: &str = "desktop-pipe-sentinel";

    #[test]
    fn shell_pipe_fixture() {
        if std::env::var_os("ADS_TEST_SHELL_PIPE").is_some() {
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            assert!(input.trim_end() == format!("12345 {BOOTSTRAP_SENTINEL}"));
            println!("bootstrap received");
        }
    }

    #[test]
    fn passes_bootstrap_only_through_inherited_pipe() {
        use io::Write;
        let (read, mut write) = io::pipe().unwrap();
        writeln!(write, "12345 {BOOTSTRAP_SENTINEL}").unwrap();
        drop(write);
        let mut shell = Command::new(std::env::current_exe().unwrap());
        shell
            .args(["shell_pipe_fixture", "--nocapture"])
            .env("ADS_TEST_SHELL_PIPE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = start_shell(&mut shell, read)
            .unwrap()
            .wait_with_output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("bootstrap received"));
        for bytes in [output.stdout, output.stderr] {
            assert!(!String::from_utf8_lossy(&bytes).contains(BOOTSTRAP_SENTINEL));
        }
    }

    fn until<T>(mut condition: impl FnMut() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(result) = condition() {
                return result;
            }
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "condition timed out"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    const ALONE_TEST: &str = "ADS_LAUNCHER_TEST_ALONE";
    const ALONE_DIR: &str = "ADS_LAUNCHER_TEST_ALONE_DIR";

    /// Runs the test `name` again in a process of its own and returns its folder there. A child
    /// started by another test thread holds a copy of a released lock until it execs, so a test
    /// that locks the same folder twice must not share its process. `verify` rechecks the
    /// folder from here, which also proves the body ran.
    fn alone(name: &str, verify: impl FnOnce(&Path)) -> Option<TestDir> {
        if std::env::var_os(ALONE_TEST).is_some_and(|value| value == name) {
            let dir = std::env::var_os(ALONE_DIR).expect("folder of the test");
            return Some(TestDir(PathBuf::from(dir)));
        }
        let dir = TestDir::new();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--test-threads", "1"])
            .env(ALONE_TEST, name)
            .env(ALONE_DIR, &dir.0)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name} failed alone:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        verify(&dir.0);
        None
    }

    #[test]
    fn a_session_that_creates_as_administrators_starts_nothing() {
        let dir = TestDir::new();
        let control = dir.0.join("control");
        let user = Identity::current().unwrap().user;
        SESSION.with(|session| {
            *session.borrow_mut() = Some(Identity {
                user,
                default_owner: "S-1-5-32-544".into(),
            });
        });
        let refused = run_with_interrupt(
            &config(),
            &control,
            fixture(None),
            fixture(None),
            None,
            SystemTime::now(),
            || panic!("nothing may run in this session"),
        );
        SESSION.with(|session| session.borrow_mut().take());
        let error = refused.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), ads_private_dir::ELEVATED_SESSION);
        assert!(!control.exists());
    }

    #[test]
    fn separate_installations_never_share_the_lock() {
        let settings = config();
        let local = Identity::current().unwrap();
        let [first, second] = [TestDir::new(), TestDir::new()].map(|dir| {
            let (control, _, _) = crate::installation_commands(&settings, &dir.0, &dir.0);
            ControlDir::open(&control, &local).unwrap()
        });
        assert_ne!(first.path(), second.path());
        let _held = open_lock(&first, &settings.lock_file).unwrap();
        let again = open_lock(&first, &settings.lock_file).unwrap_err();
        assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
        assert!(open_lock(&second, &settings.lock_file).is_ok());
    }

    #[test]
    fn increases_retry_delay_caps_it_and_stops_after_exits() {
        let config = config();
        let mut process = Process::new(fixture(None), true);
        for (index, delay) in [20, 40, 40].into_iter().enumerate() {
            assert!(process.tick(&config).unwrap());
            let child = process.child.as_mut().unwrap();
            child.wait().unwrap();
            assert!(process.tick(&config).unwrap());
            let (at, wait) = process.retry.unwrap();
            assert_eq!(wait, Duration::from_millis(delay));
            // Move the retry clock directly so assertions never depend on machine load.
            process.retry = Some((at + wait, wait));
            assert!(!process.tick(&config).unwrap());
            process.retry = Some((at, Duration::ZERO));
            assert_eq!(process.state.restarts, index as u64);
        }
        assert!(process.state.gave_up);
        assert_eq!(process.state.last_exit.as_ref().unwrap().code, Some(0));
        assert!(!process.tick(&config).unwrap());
        process.restart().unwrap();
        assert!(process.tick(&config).unwrap());
        process.stop().unwrap();
        assert!(process.failures.is_empty());
    }

    #[test]
    fn counts_slow_failures_until_the_window_expires() {
        let config = config();
        let start = Instant::now();
        let mut process = Process::new(fixture(None), true);
        for seconds in [0, 31, 62] {
            process.schedule_retry(&config, start + Duration::from_secs(seconds));
        }
        assert!(process.state.gave_up);
        process.restart().unwrap();
        process.schedule_retry(&config, start);
        process.schedule_retry(
            &config,
            start + Duration::from_millis(config.crash_window_ms),
        );
        assert_eq!(process.state.failures_in_window, 1);
        assert!(!process.state.gave_up);
        assert_eq!(process.retry.unwrap().1, Duration::from_millis(20));
    }

    #[test]
    fn a_later_error_replaces_the_discard_message_but_not_its_record() {
        let mut publication = Publication {
            stop_pending_at_startup: true,
            ..Publication::default()
        };
        publication.report("discard stale stop signal: removed stop".to_owned());
        publication.dirty = false;
        let failed: io::Result<()> = Err(io::Error::other("busy"));
        assert!(
            publication
                .attempt("consume restart signal", failed)
                .is_none()
        );
        assert_eq!(
            publication.last_error.as_deref(),
            Some("consume restart signal: busy")
        );
        assert!(publication.dirty);
        assert!(
            publication.stop_pending_at_startup,
            "the discard stays recorded after the next error"
        );
    }

    #[test]
    fn records_spawn_error_and_honors_retry_limit() {
        let mut config = config();
        config.crash_window_limit = 1;
        let dir = TestDir::new();
        let mut process = Process::new(Command::new(dir.0.join("missing")), true);
        assert!(process.tick(&config).unwrap());
        assert!(process.state.last_error.is_some());
        assert!(process.state.pid.is_none());
        assert!(process.state.gave_up);
        assert!(!process.tick(&config).unwrap());
    }

    #[test]
    fn consumes_stop_when_console_interrupt_arrives() {
        let dir = TestDir::new();
        let settings = config();
        let stop = dir.0.join(&settings.stop_file);
        let state = dir.0.join(&settings.state_file);
        run_with_interrupt(
            &settings,
            &dir.0,
            fixture(None),
            fixture(None),
            None,
            SystemTime::now(),
            || {
                fs::write(&stop, []).unwrap();
                true
            },
        )
        .unwrap();
        assert_eq!(state_value(&state).unwrap()["stopped"], true);
        assert!(!stop.exists());
    }

    #[test]
    fn discards_abandoned_stop_before_starting_children() {
        let dir = TestDir::new();
        let settings = config();
        let stop = dir.0.join(&settings.stop_file);
        let state = dir.0.join(&settings.state_file);
        fs::write(&stop, []).unwrap();
        let modified = fs::metadata(&stop).unwrap().modified().unwrap();
        let started = modified + STALE_SIGNAL_MARGIN + Duration::from_secs(1);
        let host_release = dir.0.join("release-host");
        let daemon_release = dir.0.join("release-daemon");
        let daemon = fixture(Some(&daemon_release));
        let host = fixture(Some(&host_release));
        let (interrupt, interrupted) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            run_with_interrupt(&settings, &dir.0, daemon, host, None, started, || {
                interrupted.try_recv().is_ok()
            })
        });
        let active = until(|| {
            state_value(&state)
                .filter(|state| state["daemon"]["pid"].is_u64() && state["ptyhost"]["pid"].is_u64())
        });
        assert_eq!(active["stop_pending_at_startup"], true);
        assert!(!stop.exists());
        interrupt.send(()).unwrap();
        worker.join().unwrap().unwrap();
        fs::write(host_release, []).unwrap();
    }

    #[test]
    fn honors_stop_created_after_instance_start() {
        let dir = TestDir::new();
        let settings = config();
        let started = SystemTime::now() - Duration::from_secs(1);
        let stop = dir.0.join(&settings.stop_file);
        fs::write(&stop, []).unwrap();
        let mut checked = false;
        run_with_interrupt(
            &settings,
            &dir.0,
            fixture(None),
            fixture(None),
            None,
            started,
            || {
                assert!(!checked, "the first poll must honor the startup stop");
                checked = true;
                false
            },
        )
        .unwrap();
        let state = state_value(&dir.0.join(&settings.state_file)).unwrap();
        assert_eq!(state["stopped"], true);
        assert_eq!(state["stop_pending_at_startup"], false);
        assert_eq!(state["ptyhost"]["pid"], Value::Null);
        assert!(!stop.exists());
    }

    #[test]
    fn honors_stop_dated_just_before_instance_start() {
        // File times may lag the start time (coarse clock or rounding to the second).
        let dir = TestDir::new();
        let settings = config();
        let stop = dir.0.join(&settings.stop_file);
        fs::write(&stop, []).unwrap();
        let metadata = fs::metadata(&stop).unwrap();
        let modified = metadata.modified().unwrap();
        let dated = modified.max(metadata.created().unwrap_or(modified));
        let started = dated + Duration::from_millis(500);
        let mut checked = false;
        run_with_interrupt(
            &settings,
            &dir.0,
            fixture(None),
            fixture(None),
            None,
            started,
            || {
                assert!(!checked, "the first poll must honor the recent stop");
                checked = true;
                false
            },
        )
        .unwrap();
        let state = state_value(&dir.0.join(&settings.state_file)).unwrap();
        assert_eq!(state["stopped"], true);
        assert_eq!(state["stop_pending_at_startup"], false);
        assert!(!stop.exists());
    }

    #[test]
    fn honors_stop_dated_before_start_found_while_running() {
        // A clock stepped back after the start, or a file moved into the folder, dates a new
        // request before this run.
        let dir = TestDir::new();
        let settings = config();
        let stop = dir.0.join(&settings.stop_file);
        let state = dir.0.join(&settings.state_file);
        let started = SystemTime::now() + STALE_SIGNAL_MARGIN + Duration::from_secs(60);
        let host_release = dir.0.join("release-host");
        let daemon = fixture(Some(&dir.0.join("release-daemon")));
        let host = fixture(Some(&host_release));
        let (interrupt, interrupted) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            run_with_interrupt(&settings, &dir.0, daemon, host, None, started, || {
                interrupted.try_recv().is_ok()
            })
        });
        until(|| {
            state_value(&state)
                .filter(|state| state["daemon"]["pid"].is_u64() && state["ptyhost"]["pid"].is_u64())
        });
        fs::write(&stop, []).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !worker.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let honored = worker.is_finished();
        // Release everything before asserting so a failure leaves no process behind.
        let _ = interrupt.send(());
        worker.join().unwrap().unwrap();
        fs::write(host_release, []).unwrap();
        assert!(honored, "a stop found while running must stop the launcher");
        assert_eq!(state_value(&state).unwrap()["stopped"], true);
        assert!(!stop.exists());
    }

    /// Stops the launcher of a test and releases its children when the test ends, also when an
    /// assertion fails midway: the fixtures would otherwise wait for their release forever.
    struct Release {
        interrupt: std::sync::mpsc::Sender<()>,
        files: Vec<PathBuf>,
    }

    impl Drop for Release {
        fn drop(&mut self) {
            let _ = self.interrupt.send(());
            for file in &self.files {
                let _ = fs::write(file, []);
            }
        }
    }

    /// A leftover `stop` that startup could not remove: `prepare` runs on the launcher thread
    /// before it starts, and `unblock` once both children run. The leftover must be discarded,
    /// with a record the next error cannot replace, and a new `stop` must still be honored.
    fn leftover_stop_is_discarded_then_new_one_honored(
        dir: &Path,
        prepare: impl FnOnce() + Send + 'static,
        unblock: impl FnOnce(),
    ) {
        let settings = config();
        let stop = dir.join(&settings.stop_file);
        let state = dir.join(&settings.state_file);
        let started = SystemTime::now() + STALE_SIGNAL_MARGIN + Duration::from_secs(60);
        let (daemon_release, host_release) = (dir.join("release-daemon"), dir.join("release-host"));
        let daemon = fixture(Some(&daemon_release));
        let host = fixture(Some(&host_release));
        let (interrupt, interrupted) = std::sync::mpsc::channel();
        let release = Release {
            interrupt,
            files: vec![daemon_release, host_release],
        };
        let control = dir.to_path_buf();
        let worker = thread::spawn(move || {
            prepare();
            run_with_interrupt(&settings, &control, daemon, host, None, started, || {
                interrupted.try_recv().is_ok()
            })
        });
        until(|| {
            state_value(&state)
                .filter(|state| state["daemon"]["pid"].is_u64() && state["ptyhost"]["pid"].is_u64())
        });
        unblock();
        let discarded = until(|| {
            state_value(&state).filter(|state| {
                state["last_error"]
                    .as_str()
                    .is_some_and(|error| error.starts_with("discard stale stop signal: removed"))
            })
        });
        assert_eq!(
            discarded["stop_pending_at_startup"], true,
            "the discard must stay recorded after the next error: {discarded}"
        );
        let still_running = !worker.is_finished();
        fs::write(&stop, []).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !worker.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let honored = worker.is_finished();
        drop(release);
        worker.join().unwrap().unwrap();
        assert!(
            still_running,
            "the leftover stop must not stop the launcher"
        );
        assert_eq!(discarded["stopped"], false);
        assert!(honored, "a new stop must stop the launcher");
        let stopped = state_value(&state).unwrap();
        assert_eq!(stopped["stopped"], true);
        assert_eq!(stopped["stop_pending_at_startup"], true);
        assert!(!stop.exists());
    }

    #[cfg(windows)]
    #[test]
    fn reports_leftover_stop_removed_while_running_then_honors_new_one() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = TestDir::new();
        // Without delete sharing the leftover cannot be removed at startup.
        let blocked_signal = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(3)
            .open(dir.0.join(config().stop_file))
            .unwrap();
        leftover_stop_is_discarded_then_new_one_honored(&dir.0, || {}, || drop(blocked_signal));
    }

    #[cfg(unix)]
    #[test]
    fn reports_leftover_stop_removed_while_running_then_honors_new_one() {
        let dir = TestDir::new();
        let stop = dir.0.join(config().stop_file);
        fs::write(&stop, []).unwrap();
        // Swapping the file between its check and its removal makes the startup removal fail;
        // the replacement is as old as the leftover for this run.
        let moved = dir.0.join("moved-stop");
        let swap = move || {
            fs::rename(&stop, &moved).unwrap();
            fs::write(&stop, []).unwrap();
        };
        let prepare = move || {
            private::BEFORE_REMOVE.with(|hook| *hook.borrow_mut() = Some(Box::new(swap)));
        };
        leftover_stop_is_discarded_then_new_one_honored(&dir.0, prepare, || {});
    }

    #[test]
    fn consumes_stop_arriving_during_shutdown() {
        let dir = TestDir::new();
        let settings = config();
        let stop = dir.0.join(&settings.stop_file);
        fs::write(dir.0.join(&settings.restart_file), []).unwrap();
        let late_stop = stop.clone();
        private::BEFORE_REMOVE.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || fs::write(late_stop, []).unwrap()));
        });
        run_with_interrupt(
            &settings,
            &dir.0,
            fixture(None),
            fixture(None),
            None,
            SystemTime::now(),
            || true,
        )
        .unwrap();
        assert!(!stop.exists());
    }

    #[test]
    fn initial_state_write_failure_is_fatal_and_clears_stop() {
        let dir = TestDir::new();
        let settings = config();
        let started = SystemTime::now() - Duration::from_secs(1);
        let stop = dir.0.join(&settings.stop_file);
        fs::write(&stop, []).unwrap();
        fs::create_dir(dir.0.join(&settings.state_file)).unwrap();
        assert!(
            run_with_interrupt(
                &settings,
                &dir.0,
                fixture(None),
                fixture(None),
                None,
                started,
                || panic!("initial publication must fail before polling"),
            )
            .is_err()
        );
        assert!(!stop.exists());
    }

    #[cfg(windows)]
    #[test]
    fn locked_stop_allows_startup_and_console_shutdown() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = TestDir::new();
        let settings = config();
        let stop = dir.0.join(&settings.stop_file);
        let state = dir.0.join(&settings.state_file);
        let blocked_signal = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(3)
            .open(&stop)
            .unwrap();
        let started = SystemTime::now();
        let host_release = dir.0.join("release-host");
        let daemon = fixture(Some(&dir.0.join("release-daemon")));
        let host = fixture(Some(&host_release));
        let (interrupt, interrupted) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            run_with_interrupt(&settings, &dir.0, daemon, host, None, started, || {
                interrupted.try_recv().is_ok()
            })
        });
        until(|| {
            assert!(
                !worker.is_finished(),
                "launcher exited before starting its children"
            );
            state_value(&state)
                .filter(|state| state["daemon"]["pid"].is_u64() && state["ptyhost"]["pid"].is_u64())
        });
        interrupt.send(()).unwrap();
        until(|| worker.is_finished().then_some(()));
        worker.join().unwrap().unwrap();
        let stopped = state_value(&state).unwrap();
        assert_eq!(stopped["stopped"], true);
        assert!(
            stopped["last_error"]
                .as_str()
                .unwrap()
                .contains(&*stop.to_string_lossy())
        );
        assert!(stop.exists());
        drop(blocked_signal);
        fs::write(host_release, []).unwrap();
    }

    #[test]
    fn retries_publication_after_the_state_path_becomes_writable() {
        let dir = TestDir::new();
        let settings = config();
        let state = dir.0.join(&settings.state_file);
        let restart = dir.0.join(&settings.restart_file);
        let stop = dir.0.join(&settings.stop_file);
        let daemon = fixture(Some(&dir.0.join("release-daemon")));
        let host_release = dir.0.join("release-host");
        let host = fixture(Some(&host_release));
        let control = dir.0.clone();
        let worker = thread::spawn(move || run(&settings, &control, daemon, host, None).unwrap());
        let read = || state_value(&state);
        let initial = until(|| read().filter(|value| value["daemon"]["pid"].is_u64()));
        fs::rename(&state, dir.0.join("parked-state")).unwrap();
        fs::create_dir(&state).unwrap();
        for _ in 0..2 {
            fs::write(&restart, []).unwrap();
            until(|| (!restart.exists()).then_some(()));
        }
        // The second signal can only be consumed after the first failed publication.
        fs::rename(&state, dir.0.join("parked-directory")).unwrap();
        let recovered = until(|| read().filter(|value| value["daemon"]["requested_restarts"] == 2));
        assert!(recovered["state_write_error"].is_string());
        assert!(recovered["daemon"]["pid"].is_u64());
        assert_eq!(recovered["ptyhost"]["pid"], initial["ptyhost"]["pid"]);
        fs::write(stop, []).unwrap();
        until(|| read().filter(|value| value["stopped"] == true));
        worker.join().unwrap();
        fs::write(host_release, []).unwrap();
    }

    /// Grants every account of the machine full access to `path`; on a folder, also to what
    /// is created inside, as an inherited open list would.
    fn open_to_everyone(path: &Path) {
        #[cfg(windows)]
        {
            let rule = if path.is_dir() {
                "*S-1-1-0:(OI)(CI)F"
            } else {
                "*S-1-1-0:F"
            };
            icacls(path, &["/grant", rule]);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o777)).unwrap();
        }
    }

    #[cfg(windows)]
    fn icacls(path: &Path, args: &[&str]) {
        let status = Command::new("icacls")
            .arg(path)
            .args(args)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// Asserts the exact access list of the folder and of the lock and state files: protected,
    /// with the user's entry alone, inherited by new files only from the folder.
    fn assert_private(control: &Path, settings: &Config, local: &Identity) {
        let files = [
            settings.lock_file.as_str(),
            settings.state_file.as_str(),
            crate::DAEMON_LOCK_FILE,
        ]
        .map(|name| control.join(name));
        #[cfg(windows)]
        {
            let folder =
                private::canonical_access_list(&format!("D:P(A;OICI;FA;;;{})", local.user))
                    .unwrap();
            assert_eq!(private::access_list_text(control).unwrap(), folder);
            let file =
                private::canonical_access_list(&format!("D:P(A;;FA;;;{})", local.user)).unwrap();
            for path in files {
                assert_eq!(private::access_list_text(&path).unwrap(), file, "{path:?}");
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let _ = local;
            assert_eq!(fs::metadata(control).unwrap().mode() & 0o777, 0o700);
            for path in files {
                assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
            }
        }
    }

    #[test]
    fn creates_a_private_control_folder() {
        let Some(dir) = alone(
            "supervisor::tests::creates_a_private_control_folder",
            |dir| {
                let local = Identity::current().unwrap();
                assert_private(&dir.join("control"), &config(), &local);
            },
        ) else {
            return;
        };
        let control = dir.0.join("control");
        let settings = config();
        let local = Identity::current().unwrap();
        fs::create_dir_all(&dir.0).unwrap();
        ControlDir::open(&control, &local).unwrap();
        run_with_interrupt(
            &settings,
            &control,
            fixture(None),
            fixture(None),
            None,
            SystemTime::now(),
            || true,
        )
        .unwrap();
        assert_private(&control, &settings, &local);
        // The folder, lock and state now exist: a second run reuses and keeps them private.
        run_with_interrupt(
            &settings,
            &control,
            fixture(None),
            fixture(None),
            None,
            SystemTime::now(),
            || true,
        )
        .unwrap();
        assert_private(&control, &settings, &local);
    }

    /// A folder whose single entry is not inherited: the files still get the private list,
    /// set when they are created rather than taken from the folder.
    #[cfg(windows)]
    #[test]
    fn creates_private_files_in_a_folder_without_inherited_entries() {
        let dir = TestDir::new();
        let settings = config();
        let local = Identity::current().unwrap();
        let user = format!("*{}", local.user);
        icacls(&dir.0, &["/remove:g", &user]);
        icacls(&dir.0, &["/grant", &format!("{user}:F")]);
        let folder = private::access_list_text(&dir.0).unwrap();
        assert!(!folder.contains("OI") && !folder.contains("CI"), "{folder}");
        run_with_interrupt(
            &settings,
            &dir.0,
            fixture(None),
            fixture(None),
            None,
            SystemTime::now(),
            || true,
        )
        .unwrap();
        let file = private::canonical_access_list(&format!("D:P(A;;FA;;;{})", local.user)).unwrap();
        for name in [&settings.lock_file, &settings.state_file] {
            assert_eq!(private::access_list_text(&dir.0.join(name)).unwrap(), file);
        }
    }

    #[test]
    fn refuses_a_control_folder_other_accounts_can_use() {
        let dir = TestDir::new();
        let settings = config();
        open_to_everyone(&dir.0);
        let stop = dir.0.join(&settings.stop_file);
        fs::write(&stop, []).unwrap();
        let daemon = Command::new(dir.0.join("missing-daemon"));
        let host = Command::new(dir.0.join("missing-host"));
        let error = run(&settings, &dir.0, daemon, host, None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("another account"), "{error}");
        assert!(stop.exists());
        assert!(!dir.0.join(&settings.lock_file).exists());
        assert!(!dir.0.join(&settings.state_file).exists());
    }

    #[test]
    fn refuses_a_lock_file_other_accounts_can_use() {
        let dir = TestDir::new();
        let settings = config();
        let lock = dir.0.join(&settings.lock_file);
        fs::write(&lock, []).unwrap();
        open_to_everyone(&lock);
        let stop = dir.0.join(&settings.stop_file);
        fs::write(&stop, []).unwrap();
        let daemon = Command::new(dir.0.join("missing-daemon"));
        let host = Command::new(dir.0.join("missing-host"));
        let error = run(&settings, &dir.0, daemon, host, None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("another account"), "{error}");
        assert!(stop.exists());
        assert!(!dir.0.join(&settings.state_file).exists());
    }

    #[test]
    fn refuses_a_daemon_lock_other_accounts_can_use() {
        let dir = TestDir::new();
        let settings = config();
        let lock = dir.0.join(crate::DAEMON_LOCK_FILE);
        fs::write(&lock, []).unwrap();
        open_to_everyone(&lock);
        let stop = dir.0.join(&settings.stop_file);
        fs::write(&stop, []).unwrap();
        let daemon = Command::new(dir.0.join("missing-daemon"));
        let host = Command::new(dir.0.join("missing-host"));
        let error = run(&settings, &dir.0, daemon, host, None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("another account"), "{error}");
        assert!(stop.exists());
        assert!(!dir.0.join(&settings.state_file).exists());
    }

    /// A lock the daemon created by path, still held by a daemon that outlived its launcher,
    /// does not keep a new launcher from starting. The options mirror the daemon's.
    #[test]
    fn accepts_a_daemon_lock_held_by_a_running_daemon() {
        let dir = TestDir::new();
        let settings = config();
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let held = options.open(dir.0.join(crate::DAEMON_LOCK_FILE)).unwrap();
        held.try_lock().unwrap();
        fs::write(dir.0.join(&settings.stop_file), []).unwrap();
        let daemon = Command::new(dir.0.join("missing-daemon"));
        let host = Command::new(dir.0.join("missing-host"));
        run(&settings, &dir.0, daemon, host, None).unwrap();
        drop(held);
    }

    /// A link or junction in place of the folder is refused, never followed to its target.
    #[test]
    fn refuses_a_link_in_place_of_the_control_folder() {
        let dir = TestDir::new();
        let target = TestDir::new();
        let link = dir.0.join("control");
        #[cfg(windows)]
        {
            // `mklink` reads `/` as an option: hand it backslashes only.
            let native = |path: &Path| path.to_string_lossy().replace('/', "\\");
            let status = Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(native(&link))
                .arg(native(&target.0))
                .stdout(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success());
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target.0, &link).unwrap();
        let error = ControlDir::open(&link, &Identity::current().unwrap())
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("not a plain folder"), "{error}");
    }

    /// Once open, the folder checked is the one used: replacing it by name changes nothing.
    #[test]
    fn keeps_using_the_folder_it_checked() {
        let dir = TestDir::new();
        let path = dir.0.join("control");
        let local = Identity::current().unwrap();
        let control = ControlDir::open(&path, &local).unwrap();
        let moved = dir.0.join("moved");
        #[cfg(windows)]
        {
            // The handle holds the folder and the folders above it where they are.
            assert!(fs::rename(&path, &moved).is_err());
            let parent_moved = dir.0.with_extension("moved");
            assert!(fs::rename(&dir.0, &parent_moved).is_err());
            assert!(!parent_moved.exists());
            drop(control);
            fs::rename(&path, &moved).unwrap();
        }
        #[cfg(unix)]
        {
            fs::rename(&path, &moved).unwrap();
            private::create_dir(&path, &local).unwrap();
            let planted = path.join("restart");
            fs::write(&planted, []).unwrap();
            assert_eq!(control.signal("restart", true).unwrap(), Signal::Absent);
            assert!(planted.exists());
            fs::write(moved.join("restart"), []).unwrap();
            assert_eq!(control.signal("restart", true).unwrap(), Signal::Accepted);
            assert!(!moved.join("restart").exists());
        }
    }

    /// Another account, derived from the current one and different from it: a second account
    /// cannot be created here, so the files are checked against this identity instead.
    fn another_account(local: &Identity) -> Identity {
        let shift = |id: &str, by: u32| -> String {
            #[cfg(windows)]
            {
                let (domain, rid) = id.rsplit_once('-').unwrap();
                format!("{domain}-{}", rid.parse::<u32>().unwrap().wrapping_add(by))
            }
            #[cfg(unix)]
            {
                id.parse::<u32>().unwrap().wrapping_add(by).to_string()
            }
        };
        let user = (1..)
            .map(|by| shift(&local.user, by))
            .find(|id| !local.owns(id))
            .unwrap();
        let other = Identity {
            user: user.clone(),
            default_owner: user,
        };
        assert!(!local.owns(&other.user));
        assert!(!other.owns(&local.user) && !other.owns(&local.default_owner));
        other
    }

    #[test]
    fn ignores_a_signal_file_owned_by_another_account() {
        let dir = TestDir::new();
        let signal = dir.0.join("restart");
        fs::write(&signal, []).unwrap();
        let local = Identity::current().unwrap();
        let control = ControlDir::open(&dir.0, &local).unwrap();
        let as_other = ControlDir::open(&dir.0, &local)
            .unwrap()
            .judged_as(another_account(&local));
        assert_eq!(as_other.signal("restart", true).unwrap(), Signal::Foreign);
        assert!(signal.exists());
        assert_eq!(control.signal("restart", false).unwrap(), Signal::Accepted);
        assert!(signal.exists());
        assert_eq!(control.signal("restart", true).unwrap(), Signal::Accepted);
        assert!(!signal.exists());
        assert_eq!(control.signal("restart", true).unwrap(), Signal::Absent);
    }

    #[cfg(windows)]
    #[test]
    fn ignores_a_signal_file_other_accounts_can_use() {
        let dir = TestDir::new();
        let signal = dir.0.join("restart");
        fs::write(&signal, []).unwrap();
        open_to_everyone(&signal);
        let control = ControlDir::open(&dir.0, &Identity::current().unwrap()).unwrap();
        assert_eq!(control.signal("restart", true).unwrap(), Signal::Foreign);
        assert!(signal.exists());
    }

    /// The entry removed is the one checked, even if the name is reused in between.
    #[test]
    fn removes_only_the_signal_it_checked() {
        let dir = TestDir::new();
        let signal = dir.0.join("restart");
        fs::write(&signal, []).unwrap();
        let control = ControlDir::open(&dir.0, &Identity::current().unwrap()).unwrap();
        let moved = dir.0.join("moved");
        let swap = {
            let (signal, moved) = (signal.clone(), moved.clone());
            move || {
                if fs::rename(&signal, &moved).is_ok() {
                    fs::write(&signal, b"replacement").unwrap();
                }
            }
        };
        private::BEFORE_REMOVE.with(|hook| *hook.borrow_mut() = Some(Box::new(swap)));
        let outcome = control.signal("restart", true);
        #[cfg(windows)]
        {
            // The signal is open without sharing the right to delete: the swap is refused.
            assert_eq!(outcome.unwrap(), Signal::Accepted);
            assert!(!moved.exists());
            assert!(!signal.exists());
        }
        #[cfg(unix)]
        {
            // The swap goes through; the name now points elsewhere, so nothing is removed.
            assert!(outcome.is_err());
            assert!(moved.exists());
            assert_eq!(fs::read(&signal).unwrap(), b"replacement");
        }
    }

    fn state_value(path: &Path) -> Option<Value> {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    }

    #[cfg(windows)]
    #[test]
    fn retries_locked_signal_without_restarting_the_daemon() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ_WRITE: u32 = 3;
        let dir = TestDir::new();
        let settings = config();
        let state = dir.0.join(&settings.state_file);
        let restart = dir.0.join(&settings.restart_file);
        let stop = dir.0.join(&settings.stop_file);
        let daemon_release = dir.0.join("release-daemon");
        let host_release = dir.0.join("release-host");
        let daemon = fixture(Some(&daemon_release));
        let host = fixture(Some(&host_release));
        let control = dir.0.clone();
        let worker = thread::spawn(move || run(&settings, &control, daemon, host, None).unwrap());
        let read = || state_value(&state);
        let initial = until(|| read().filter(|value| value["daemon"]["pid"].is_u64()));
        let blocked_signal = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ_WRITE)
            .open(&restart)
            .unwrap();
        let recovered = until(|| {
            read().filter(|value| {
                value["last_error"]
                    .as_str()
                    .is_some_and(|error| error.starts_with("consume restart signal"))
            })
        });
        assert_eq!(recovered["daemon"]["pid"], initial["daemon"]["pid"]);
        drop(blocked_signal);
        until(|| read().filter(|value| value["daemon"]["requested_restarts"] == 1));
        fs::write(stop, []).unwrap();
        until(|| read().filter(|value| value["stopped"] == true));
        worker.join().unwrap();
        fs::write(host_release, []).unwrap();
    }

    #[test]
    fn desktop_daemon_fixture() {
        if let Some(release) = std::env::var_os("ADS_TEST_DESKTOP_RELEASE") {
            println!("{} {BOOTSTRAP_SENTINEL}", std::process::id());
            wait_for_release(Path::new(&release));
        }
    }

    #[test]
    fn desktop_window_fixture() {
        if let Some(directory) = std::env::var_os("ADS_TEST_DESKTOP_DIRECTORY") {
            use io::BufRead;
            let daemon = io::stdin()
                .lock()
                .lines()
                .map_while(Result::ok)
                .find_map(|line| {
                    let (daemon, token) = line.split_once(' ')?;
                    (token == BOOTSTRAP_SENTINEL).then(|| daemon.parse::<u32>().unwrap())
                })
                .unwrap();
            let directory = Path::new(&directory);
            let lock = window_lock(&directory.join(format!("window-lock-{daemon}")));
            lock.lock().unwrap();
            fs::write(directory.join(format!("window-{daemon}")), []).unwrap();
            wait_for_release(&directory.join("release-window"));
        }
    }

    fn wait_for_release(path: &Path) {
        let start = Instant::now();
        while !path.is_file() && start.elapsed() < Duration::from_secs(120) {
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn run_reconnects_desktop_to_restarted_daemon() {
        let dir = TestDir::new();
        let host_release = dir.0.join("release-host");
        let mut daemon = Command::new(std::env::current_exe().unwrap());
        daemon
            .args([
                "desktop_daemon_fixture",
                "--quiet",
                "--nocapture",
                "--format",
                "terse",
            ])
            .env("ADS_TEST_DESKTOP_RELEASE", dir.0.join("release-daemon"))
            .stderr(Stdio::null());
        let mut shell = Command::new(std::env::current_exe().unwrap());
        shell
            .args(["desktop_window_fixture", "--quiet"])
            .env("ADS_TEST_DESKTOP_DIRECTORY", &dir.0)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let control = dir.0.clone();
        let host = fixture(Some(&host_release));
        let worker = thread::spawn(move || run(&config(), &control, daemon, host, Some(shell)));
        let connected = |restarts| {
            until(|| {
                let state: Value =
                    serde_json::from_slice(&fs::read(dir.0.join("state.json")).ok()?).unwrap();
                let daemon = state["daemon"]["pid"].as_u64()?;
                let host = state["ptyhost"]["pid"].as_u64()?;
                (state["daemon"]["requested_restarts"] == restarts
                    && dir.0.join(format!("window-{daemon}")).is_file())
                .then_some((daemon, host))
            })
        };
        let (first, host) = connected(0);
        fs::write(dir.0.join("restart"), []).unwrap();
        let (second, same_host) = connected(1);
        let previous_closed = window_lock(&dir.0.join(format!("window-lock-{first}")))
            .try_lock()
            .is_ok();
        fs::write(dir.0.join("stop"), []).unwrap();
        fs::write(dir.0.join("release-window"), []).unwrap();
        fs::write(host_release, []).unwrap();
        worker.join().unwrap().unwrap();
        assert_ne!(first, second);
        assert_eq!(host, same_host);
        assert!(
            previous_closed,
            "the replacement started before the previous shell exited"
        );
    }

    /// Runs a desktop installation whose window is `shell` until `state.json` satisfies
    /// `published`, then stops it and returns that state.
    fn shell_state(shell: Command, published: impl Fn(&Value) -> bool) -> Value {
        let dir = TestDir::new();
        // These shells never read the bootstrap pipe. A test harness writing to it can exit
        // on a broken pipe and trigger a daemon restart before the state is observed.
        #[cfg(windows)]
        let (program, args) = (
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 120",
            ][..],
        );
        #[cfg(unix)]
        let (program, args) = ("sleep", &["120"][..]);
        let mut daemon = Command::new(program);
        daemon.args(args);
        let settings = config();
        let state_file = dir.0.join(&settings.state_file);
        let mut observed = None;
        let start = Instant::now();
        run_with_interrupt(
            &settings,
            &dir.0,
            daemon,
            fixture(None),
            Some(shell),
            SystemTime::now(),
            || {
                let state = state_value(&state_file).expect("published supervisor state");
                if published(&state) {
                    observed = Some(state);
                    return true;
                }
                assert!(start.elapsed() < Duration::from_secs(30), "{state}");
                false
            },
        )
        .unwrap();
        observed.expect("state before shutdown")
    }

    #[test]
    fn publishes_a_shell_that_fails_to_start() {
        let missing = Command::new(TestDir::new().0.join("missing-shell"));
        let state = shell_state(missing, |state| state["shell"]["starts"] == 1);
        assert!(state["shell"]["pid"].is_null(), "{state}");
        assert!(state["shell"]["last_error"].is_string(), "{state}");
        assert!(state["daemon"]["pid"].is_u64(), "{state}");
    }

    #[test]
    fn publishes_the_exit_of_the_shell() {
        let state = shell_state(fixture(None), |state| {
            state["shell"]["last_exit"].is_object()
        });
        assert_eq!(state["shell"]["starts"], 1, "{state}");
        assert!(state["shell"]["pid"].is_null(), "{state}");
        assert_eq!(state["shell"]["last_exit"]["code"], 0, "{state}");
    }

    #[test]
    fn a_successful_start_clears_the_previous_shell_error() {
        let mut window = Window::new(&config());
        let mut missing = Command::new(TestDir::new().0.join("missing-shell"));
        assert!(window.start(&mut missing, Stdio::null()).is_err());
        assert!(window.state.last_error.is_some());
        window.start(&mut fixture(None), Stdio::null()).unwrap();
        assert!(window.state.pid.is_some());
        assert!(window.state.last_error.is_none());
        assert_eq!(window.state.starts, 2);
    }

    #[test]
    fn publishes_no_shell_in_browser_mode() {
        let dir = TestDir::new();
        let settings = config();
        fs::write(dir.0.join(&settings.stop_file), []).unwrap();
        let daemon = Command::new(dir.0.join("missing-daemon"));
        let host = Command::new(dir.0.join("missing-host"));
        run(&settings, &dir.0, daemon, host, None).unwrap();
        let state = state_value(&dir.0.join(&settings.state_file)).unwrap();
        assert_eq!(state.get("shell"), Some(&Value::Null), "{state}");
    }

    fn desktop_daemon(dir: &Path) -> Command {
        let mut daemon = Command::new(std::env::current_exe().unwrap());
        daemon
            .args([
                "desktop_daemon_fixture",
                "--quiet",
                "--nocapture",
                "--format",
                "terse",
            ])
            .env("ADS_TEST_DESKTOP_RELEASE", dir.join("release-daemon"))
            .stderr(Stdio::null());
        daemon
    }

    /// A window that reads only its first line, so the daemon's end alone never closes it,
    /// and holds a lock on `window.lock` while it runs.
    #[test]
    fn held_window_fixture() {
        if let Some(directory) = std::env::var_os("ADS_TEST_HELD_WINDOW") {
            let directory = Path::new(&directory);
            let mut line = String::new();
            io::stdin().read_line(&mut line).unwrap();
            let lock = window_lock(&directory.join("window.lock"));
            lock.lock().unwrap();
            #[cfg(windows)]
            {
                use io::Read;
                use std::os::windows::io::AsHandle;
                let mut pipe = File::from(io::stderr().as_handle().try_clone_to_owned().unwrap());
                let mut command = [0];
                if pipe.read_exact(&mut command).is_ok() {
                    assert_eq!(command, *b"Q");
                    fs::write(directory.join("close-request"), []).unwrap();
                    if !directory.join("ignore-close").exists() {
                        return;
                    }
                }
            }
            wait_for_release(&directory.join("release-window"));
        }
    }

    fn held_window(dir: &Path) -> Command {
        let mut shell = Command::new(std::env::current_exe().unwrap());
        shell
            .args(["--exact", "supervisor::tests::held_window_fixture"])
            .env("ADS_TEST_HELD_WINDOW", dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        shell
    }

    fn window_lock(path: &Path) -> File {
        fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .unwrap()
    }

    fn window_running(dir: &Path) -> bool {
        match window_lock(&dir.join("window.lock")).try_lock() {
            Ok(()) => false,
            Err(TryLockError::WouldBlock) => true,
            Err(TryLockError::Error(error)) => panic!("check window lock: {error}"),
        }
    }

    fn wait_for_window(dir: &Path, running: bool) -> bool {
        let start = Instant::now();
        while window_running(dir) != running {
            if start.elapsed() > Duration::from_secs(2) {
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
        true
    }

    #[test]
    fn closes_the_window_on_stop() {
        let dir = TestDir::new();
        let host_release = dir.0.join("release-host");
        let (control, daemon, shell) = (dir.0.clone(), desktop_daemon(&dir.0), held_window(&dir.0));
        let host = fixture(Some(&host_release));
        let worker = thread::spawn(move || run(&config(), &control, daemon, host, Some(shell)));
        until(|| window_running(&dir.0).then_some(()));
        fs::write(dir.0.join("stop"), []).unwrap();
        worker.join().unwrap().unwrap();
        let ended = wait_for_window(&dir.0, false);
        fs::write(dir.0.join("release-window"), []).unwrap();
        fs::write(host_release, []).unwrap();
        assert!(ended, "the window outlived the stop signal");
        #[cfg(windows)]
        assert!(dir.0.join("close-request").is_file());
    }

    #[test]
    fn closes_the_window_when_giving_up_on_the_daemon() {
        let dir = TestDir::new();
        let host_release = dir.0.join("release-host");
        let (control, shell) = (dir.0.clone(), held_window(&dir.0));
        let daemon = desktop_daemon(&dir.0);
        let host = fixture(Some(&host_release));
        let worker = thread::spawn(move || {
            let mut config = config();
            config.crash_window_limit = 1;
            run(&config, &control, daemon, host, Some(shell))
        });
        if !wait_for_window(&dir.0, true) {
            fs::write(dir.0.join("stop"), []).unwrap();
            fs::write(host_release, []).unwrap();
            worker.join().unwrap().unwrap();
            panic!("the window never became ready");
        }
        fs::write(dir.0.join("release-daemon"), []).unwrap();
        until(|| {
            state_value(&dir.0.join("state.json"))
                .filter(|state| state["daemon"]["gave_up"] == true)
        });
        let ended = wait_for_window(&dir.0, false);
        fs::write(dir.0.join("stop"), []).unwrap();
        worker.join().unwrap().unwrap();
        fs::write(dir.0.join("release-window"), []).unwrap();
        fs::write(host_release, []).unwrap();
        assert!(ended, "the window outlived the daemon it was opened for");
    }

    #[cfg(windows)]
    #[test]
    fn console_interrupt_closes_a_running_window() {
        const CHILD: &str = "ADS_TEST_OWN_CONSOLE";
        const NAME: &str = "supervisor::tests::console_interrupt_closes_a_running_window";
        if std::env::var_os(CHILD).is_none() {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_CONSOLE: u32 = 0x10;
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME])
                .env(CHILD, "1")
                .creation_flags(CREATE_NEW_CONSOLE)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            return;
        }

        console::install_console_handler().unwrap();
        let dir = TestDir::new();
        let control = dir.0.clone();
        let host_release = dir.0.join("release-host");
        let host = fixture(Some(&host_release));
        let daemon = desktop_daemon(&dir.0);
        let shell = held_window(&dir.0);
        let worker = thread::spawn(move || run(&config(), &control, daemon, host, Some(shell)));
        until(|| window_running(&dir.0).then_some(()));
        console::generate_console_interrupt().unwrap();
        let now = Instant::now();
        while !worker.is_finished() && now.elapsed() < Duration::from_secs(5) {
            thread::sleep(Duration::from_millis(5));
        }
        let stopped = worker.is_finished();
        if !stopped {
            fs::write(dir.0.join("stop"), []).unwrap();
        }
        worker.join().unwrap().unwrap();
        fs::write(host_release, []).unwrap();
        assert!(stopped, "Ctrl+C did not stop the supervisor");
        assert!(dir.0.join("close-request").is_file());
        assert!(!window_running(&dir.0));
    }

    #[cfg(windows)]
    fn start_held_window(dir: &Path, timeout_ms: u64) -> Window {
        use io::Write;
        let mut config = config();
        config.shell_close_timeout_ms = timeout_ms;
        let (reader, mut writer) = io::pipe().unwrap();
        writer.write_all(b"bootstrap\n").unwrap();
        let mut window = Window::new(&config);
        window.start(&mut held_window(dir), reader).unwrap();
        until(|| window_running(dir).then_some(()));
        window
    }

    #[cfg(windows)]
    #[test]
    fn window_drop_only_requests_its_own_child() {
        let owned = TestDir::new();
        let independent = TestDir::new();
        let window = start_held_window(&owned.0, 2000);
        let mut other = start_held_window(&independent.0, 2000);
        drop(window);
        assert!(owned.0.join("close-request").is_file());
        assert!(!window_running(&owned.0));
        assert!(other.child.as_mut().unwrap().try_wait().unwrap().is_none());
        assert!(!independent.0.join("close-request").exists());
    }

    #[cfg(windows)]
    #[test]
    fn forces_unresponsive_window_only_after_the_deadline() {
        let dir = TestDir::new();
        fs::write(dir.0.join("ignore-close"), []).unwrap();
        let mut window = start_held_window(&dir.0, 1000);
        let start = Instant::now();
        window.close(|error| panic!("{error}")).unwrap();
        assert!(start.elapsed() >= Duration::from_secs(1));
        assert!(dir.0.join("close-request").is_file());
        assert!(!window_running(&dir.0));
    }

    #[cfg(windows)]
    #[test]
    fn zero_deadline_skips_the_close_request() {
        let dir = TestDir::new();
        let mut window = start_held_window(&dir.0, 0);
        window.close(|error| panic!("{error}")).unwrap();
        assert!(!dir.0.join("close-request").exists());
        assert!(!window_running(&dir.0));
    }

    #[cfg(windows)]
    #[test]
    fn failed_request_is_reported_even_when_forced_close_succeeds() {
        let dir = TestDir::new();
        let mut window = start_held_window(&dir.0, 2000);
        let (reader, writer) = io::pipe().unwrap();
        drop(reader);
        window.close_pipe = Some(writer);
        let mut publication = Publication::default();
        assert!(publication.close_shell(&mut window));
        assert!(
            publication
                .last_error
                .unwrap()
                .starts_with("request shell close: ")
        );
        assert!(!window_running(&dir.0));
    }

    #[test]
    fn publishes_state_consumes_signals_and_keeps_the_same_host() {
        let name = "supervisor::tests::publishes_state_consumes_signals_and_keeps_the_same_host";
        let Some(dir) = alone(name, |dir| {
            let state = state_value(&dir.join("state.json")).unwrap();
            assert_eq!(state["stopped"], true);
        }) else {
            return;
        };
        let daemon_release = dir.0.join("release-daemon");
        let host_release = dir.0.join("release-host");
        let state_path = dir.0.join("state.json");
        let read_state = || {
            fs::read(&state_path)
                .ok()
                .map(|bytes| serde_json::from_slice::<Value>(&bytes).unwrap())
        };
        let control = dir.0.clone();
        let daemon = fixture(Some(&daemon_release));
        let host = fixture(Some(&host_release));
        let worker = thread::spawn(move || run(&config(), &control, daemon, host, None).unwrap());
        let initial = until(|| {
            read_state()
                .filter(|value| value["daemon"]["pid"].is_u64() && value["ptyhost"]["pid"].is_u64())
        });
        let lock = run(&config(), &dir.0, fixture(None), fixture(None), None).unwrap_err();
        assert_eq!(lock.kind(), io::ErrorKind::AlreadyExists);
        assert!(lock.to_string().contains("another launcher"));
        let restart = dir.0.join("restart");
        fs::write(&restart, []).unwrap();
        let restarted = until(|| {
            read_state().filter(|value| {
                value["daemon"]["requested_restarts"] == 1
                    && value["daemon"]["restarts"] == 1
                    && value["daemon"]["pid"].is_u64()
            })
        });
        assert!(!restart.exists());
        assert_eq!(restarted["ptyhost"]["pid"], initial["ptyhost"]["pid"]);
        assert!(restarted["daemon"]["last_exit"].is_object());
        assert_ne!(restarted["daemon"]["last_exit"]["code"], 0);

        let stop = dir.0.join("stop");
        fs::write(&stop, []).unwrap();
        let stopped = until(|| read_state().filter(|value| value["stopped"] == true));
        worker.join().unwrap();
        assert!(!stop.exists());
        assert!(stopped["daemon"]["pid"].is_null());
        assert_eq!(stopped["ptyhost"]["pid"], initial["ptyhost"]["pid"]);
        let control = ControlDir::open(&dir.0, &Identity::current().unwrap()).unwrap();
        assert!(open_lock(&control, &config().lock_file).is_ok());
        fs::write(host_release, []).unwrap();
    }
}
