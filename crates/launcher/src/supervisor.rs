use std::{
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
    rapid_failures: u32,
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
    child: Option<(Child, Instant)>,
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
            state: ProcessState::default(),
            attempted: false,
            retry: None,
            terminate_on_drop,
        }
    }

    fn tick(&mut self, config: &Config) -> io::Result<bool> {
        if let Some((child, started)) = &mut self.child {
            if let Some(status) = child.try_wait()? {
                let rapid = started.elapsed() < Duration::from_millis(config.rapid_exit_ms);
                self.record_exit(status);
                self.schedule_retry(config, rapid);
                return Ok(true);
            }
            return Ok(false);
        }
        if self.state.gave_up || self.retry.is_some_and(|(at, delay)| at.elapsed() < delay) {
            return Ok(false);
        }
        if self.attempted {
            self.state.restarts = self.state.restarts.saturating_add(1);
        }
        self.attempted = true;
        match self.command.spawn() {
            Ok(child) => {
                self.state.pid = Some(child.id());
                self.child = Some((child, Instant::now()));
            }
            Err(error) => {
                self.state.last_error = Some(error.to_string());
                self.schedule_retry(config, true);
            }
        }
        Ok(true)
    }

    fn schedule_retry(&mut self, config: &Config, rapid: bool) {
        if rapid {
            self.state.rapid_failures += 1;
        } else {
            self.state.rapid_failures = 0;
        }
        self.state.gave_up = self.state.rapid_failures >= config.rapid_exit_limit;
        let exponent = self.state.rapid_failures.saturating_sub(1);
        let factor = 2_u64.saturating_pow(exponent);
        let delay = config
            .restart_initial_ms
            .saturating_mul(factor)
            .min(config.restart_max_ms);
        self.retry = Some((Instant::now(), Duration::from_millis(delay)));
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
        if let Some((child, _)) = &mut self.child {
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
        self.state.rapid_failures = 0;
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

fn write_state(path: &Path, daemon: &Process, host: &Process, stopped: bool) -> io::Result<()> {
    #[derive(Serialize)]
    struct State<'a> {
        launcher_pid: u32,
        stopped: bool,
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
    write_state(&state, &daemon, &host, false)?;
    loop {
        if consume_signal(&stop)? {
            consume_signal(&restart)?;
            host.tick_exit_only()?;
            daemon.stop()?;
            return write_state(&state, &daemon, &host, true);
        }
        let requested_restart = consume_signal(&restart)?;
        if requested_restart {
            daemon.restart()?;
        }
        let host_changed = host.tick(config)?;
        let daemon_changed = daemon.tick(config)?;
        if host_changed || daemon_changed || requested_restart {
            write_state(&state, &daemon, &host, false)?;
        }
        thread::sleep(Duration::from_millis(config.poll_ms));
    }
}

impl Process {
    fn tick_exit_only(&mut self) -> io::Result<()> {
        if let Some((child, _)) = &mut self.child
            && let Some(status) = child.try_wait()?
        {
            self.record_exit(status);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::PathBuf;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target")
                .join(format!("launcher-test-{}-{stamp}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    fn config() -> Config {
        let mut config = Config::load(None).unwrap();
        config.poll_ms = 5;
        config.restart_initial_ms = 20;
        config.restart_max_ms = 40;
        config.rapid_exit_ms = 10_000;
        config.rapid_exit_limit = 3;
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
            while !Path::new(&path).is_file() && start.elapsed() < Duration::from_secs(5) {
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
                start.elapsed() < Duration::from_secs(3),
                "condition timed out"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn increases_retry_delay_caps_it_and_stops_after_rapid_exits() {
        let config = config();
        let mut process = Process::new(fixture(None), true);
        for (index, delay) in [20, 40, 40].into_iter().enumerate() {
            assert!(process.tick(&config).unwrap());
            let (child, _) = process.child.as_mut().unwrap();
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
        process.state.rapid_failures = 2;
        process.schedule_retry(&config, false);
        assert_eq!(process.state.rapid_failures, 0);
        assert_eq!(process.retry.unwrap().1, Duration::from_millis(20));
    }

    #[test]
    fn records_spawn_error_and_honors_retry_limit() {
        let mut config = config();
        config.rapid_exit_limit = 1;
        let dir = TestDir::new();
        let mut process = Process::new(Command::new(dir.0.join("missing")), true);
        assert!(process.tick(&config).unwrap());
        assert!(process.state.last_error.is_some());
        assert!(process.state.pid.is_none());
        assert!(process.state.gave_up);
        assert!(!process.tick(&config).unwrap());
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
            read_state()
                .filter(|value| value["daemon"]["restarts"] == 1 && value["daemon"]["pid"].is_u64())
        });
        assert!(!restart.exists());
        assert_eq!(restarted["ptyhost"]["pid"], initial["ptyhost"]["pid"]);
        assert!(restarted["daemon"]["last_exit"].is_object());

        let stop = dir.0.join("stop");
        fs::write(&stop, []).unwrap();
        let stopped = until(|| read_state().filter(|value| value["stopped"] == true));
        worker.join().unwrap();
        assert!(!stop.exists());
        assert!(stopped["daemon"]["pid"].is_null());
        assert_eq!(stopped["ptyhost"]["pid"], initial["ptyhost"]["pid"]);
        assert!(open_lock(&dir.0.join("supervisor.lock")).is_ok());
        fs::write(host_release, []).unwrap();
    }
}
