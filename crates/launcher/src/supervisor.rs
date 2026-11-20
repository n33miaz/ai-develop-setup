use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions, TryLockError},
    io,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::Config;

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
        self.state.last_exit = Some(Exit {
            code: status.code(),
            status: status.to_string(),
        });
    }

    fn stop(&mut self) -> io::Result<()> {
        if let Some(child) = &mut self.child {
            let status = match child.try_wait()? {
                Some(status) => status,
                None => {
                    if let Err(error) = child.kill()
                        && child.try_wait()?.is_none()
                    {
                        return Err(error);
                    }
                    child.wait()?
                }
            };
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

fn open_lock(path: &Path) -> io::Result<File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    lock.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::AlreadyExists,
            "another launcher is already running for this installation",
        ),
        TryLockError::Error(error) => error,
    })?;
    Ok(lock)
}

fn consume_signal(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        },
        Ok(_) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn write_state(
    path: &Path,
    daemon: &Process,
    host: &Process,
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
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = path.with_extension(format!("{}-{stamp}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = serde_json::to_writer(
        &mut file,
        &State {
            launcher_pid: std::process::id(),
            stopped,
            stop_pending_at_startup: publication.stop_pending_at_startup,
            last_error: publication.last_error.as_deref(),
            state_write_error: publication.state_write_error.as_deref(),
            daemon: &daemon.state,
            ptyhost: &host.state,
        },
    )
    .map_err(io::Error::other)
    .and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
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
                let message = format!("{operation}: {error}");
                if self.last_error.as_deref() != Some(&message) {
                    eprintln!("ads-launcher: {message}");
                    self.last_error = Some(message);
                    self.dirty = true;
                }
                None
            }
        }
    }
}

pub(crate) fn run(
    config: &Config,
    control: &Path,
    mut daemon: Command,
    mut host: Command,
) -> io::Result<()> {
    fs::create_dir_all(control)?;
    let _lock = open_lock(&control.join(&config.lock_file))?;
    let state = control.join(&config.state_file);
    let stop = control.join(&config.stop_file);
    let restart = control.join(&config.restart_file);
    detach(&mut host);
    daemon.stdin(Stdio::null());
    let mut daemon = Process::new(daemon, true);
    let mut host = Process::new(host, false);

    // Publish before starting children so an unusable state path fails early.
    let mut publication = Publication {
        stop_pending_at_startup: fs::symlink_metadata(&stop)
            .is_ok_and(|metadata| metadata.is_file()),
        ..Publication::default()
    };
    write_state(&state, &daemon, &host, false, &publication)?;
    let mut stopping = false;
    let mut restarting = false;
    loop {
        if !stopping {
            stopping =
                publication.attempt("consume stop signal", consume_signal(&stop)) == Some(true);
        }
        let stopped = if stopping {
            let daemon_stopped = publication.attempt("stop daemon", daemon.stop()).is_some();
            let signal_cleared = publication
                .attempt("clear restart signal", consume_signal(&restart))
                .is_some();
            publication.attempt("check host exit", host.tick_exit_only());
            publication.dirty = true;
            daemon_stopped && signal_cleared
        } else {
            restarting |= publication.attempt("consume restart signal", consume_signal(&restart))
                == Some(true);
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
            publication.dirty |= host_changed || daemon_changed;
            false
        };
        if publication.dirty {
            match write_state(&state, &daemon, &host, stopped, &publication) {
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
    use serde_json::Value;
    use std::path::PathBuf;

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
            fs::create_dir(&path).unwrap();
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
            let start = Instant::now();
            while !Path::new(&path).is_file() && start.elapsed() < Duration::from_secs(120) {
                thread::sleep(Duration::from_millis(5));
            }
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
    fn reports_stop_pending_at_startup_without_starting_children() {
        let dir = TestDir::new();
        let settings = config();
        fs::write(dir.0.join(&settings.stop_file), []).unwrap();
        let daemon = Command::new(dir.0.join("missing-daemon"));
        let host = Command::new(dir.0.join("missing-host"));
        run(&settings, &dir.0, daemon, host).unwrap();
        let state = state_value(&dir.0.join(&settings.state_file)).unwrap();
        assert_eq!(state["stopped"], true);
        assert_eq!(state["stop_pending_at_startup"], true);
        assert!(state["daemon"]["last_error"].is_null());
        assert!(state["ptyhost"]["last_error"].is_null());
    }

    #[test]
    fn initial_state_write_failure_is_fatal() {
        let dir = TestDir::new();
        let settings = config();
        fs::create_dir(dir.0.join(&settings.state_file)).unwrap();
        assert!(run(&settings, &dir.0, fixture(None), fixture(None)).is_err());
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
        let worker = thread::spawn(move || run(&settings, &control, daemon, host).unwrap());
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
        let worker = thread::spawn(move || run(&settings, &control, daemon, host).unwrap());
        let read = || state_value(&state);
        let initial = until(|| read().filter(|value| value["daemon"]["pid"].is_u64()));
        let blocked_signal = OpenOptions::new()
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
    fn publishes_state_consumes_signals_and_keeps_the_same_host() {
        let dir = TestDir::new();
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
        let worker = thread::spawn(move || run(&config(), &control, daemon, host).unwrap());
        let initial = until(|| {
            read_state()
                .filter(|value| value["daemon"]["pid"].is_u64() && value["ptyhost"]["pid"].is_u64())
        });
        let lock = run(&config(), &dir.0, fixture(None), fixture(None)).unwrap_err();
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
        assert!(open_lock(&dir.0.join(config().lock_file)).is_ok());
        fs::write(host_release, []).unwrap();
    }
}
