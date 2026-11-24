use ads_daemon::startup::{MAX_CONFIG_BYTES, load_config};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A temporary configuration file created in [`temp::directory`], removed on drop.
struct UserFile {
    path: PathBuf,
}

impl UserFile {
    fn new(name: &str, contents: &[u8]) -> Self {
        let path = temp::directory().join(format!("{name}-{}.toml", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for UserFile {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<std::ffi::OsStr> for UserFile {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.path.as_ref()
    }
}

impl std::ops::Deref for UserFile {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for UserFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
            && !std::thread::panicking()
        {
            panic!(
                "failed to remove temporary user file {:?}: {error}",
                self.path
            );
        }
    }
}

fn user_file(name: &str, contents: &[u8]) -> UserFile {
    UserFile::new(name, contents)
}

fn load(flag: &str, path: impl AsRef<Path>) -> Result<ads_daemon::startup::StartupConfig, String> {
    let path = path.as_ref().to_str().unwrap().to_owned();
    load_config([flag.to_owned(), path].into_iter())
}

/// Runs the daemon with a user file it must refuse before contacting the terminal host.
fn refused(name: &str, contents: &str) -> String {
    let file = user_file(name, contents.as_bytes());
    let output = Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
        .arg("--config")
        .arg(file.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(!error.contains("sentinel"), "{error}");
    error
}

#[test]
fn arguments_are_closed() {
    assert!(load_config(std::iter::empty()).is_ok());
    for arg in ["--unknown", "--config", "--host-config"] {
        assert!(load_config([arg.to_owned()].into_iter()).is_err());
    }
}

#[test]
fn empty_configuration_files_preserve_defaults() {
    let paths = {
        let (_scratch, paths) = empty_configuration_files();
        let [user_path, host_path] = &paths;
        assert_empty_configuration_defaults(user_path, host_path);
        paths
    };
    assert_empty_configuration_files_removed(&paths);
}

#[test]
fn empty_configuration_files_are_removed_on_panic() {
    let (scratch, paths) = empty_configuration_files();
    let result = std::panic::catch_unwind(move || {
        let _scratch = scratch;
        panic!("unwind with empty configuration files");
    });
    assert!(result.is_err());
    assert_empty_configuration_files_removed(&paths);
}

#[test]
fn scratch_reports_and_preserves_unexpected_files_in_subdirectories() {
    let run = ads_daemon::events::new_id().unwrap();
    let scratch = temp::Scratch::with_files(&format!("daemon-cleanup-{run}"), &[])
        .with_subdirs(&[("nested", &["expected"])]);
    let root = scratch.path().to_owned();
    let nested = root.join("nested");
    std::fs::create_dir(&nested).unwrap();
    let expected = nested.join("expected");
    let unexpected = nested.join("unexpected");
    std::fs::write(&expected, []).unwrap();
    std::fs::write(&unexpected, b"keep").unwrap();

    let error = std::panic::catch_unwind(|| drop(scratch)).expect_err("cleanup should fail");
    let message = error.downcast_ref::<String>().unwrap();
    assert!(message.contains("cannot remove scratch folder") && message.contains("nested"));
    assert!(!expected.try_exists().unwrap());
    assert_eq!(std::fs::read(&unexpected).unwrap(), b"keep");

    std::fs::remove_file(unexpected).unwrap();
    std::fs::remove_dir(nested).unwrap();
    std::fs::remove_dir(root).unwrap();
}

fn empty_configuration_files() -> (temp::Scratch, [PathBuf; 2]) {
    const FILES: [&str; 2] = ["user.toml", "host.toml"];
    let run = ads_daemon::events::new_id().unwrap();
    let scratch = temp::Scratch::with_files(&format!("daemon-empty-config-{run}"), &FILES);
    let paths = FILES.map(|name| {
        let path = scratch.path().join(name);
        std::fs::write(&path, b"").unwrap();
        path
    });
    (scratch, paths)
}

fn assert_empty_configuration_files_removed(paths: &[PathBuf; 2]) {
    let directory = paths[0].parent().unwrap();
    for path in paths.iter().map(PathBuf::as_path).chain([directory]) {
        assert!(!path.try_exists().unwrap(), "fixture remains: {path:?}");
    }
}

fn assert_empty_configuration_defaults(user_path: &Path, host_path: &Path) {
    let (default_api, default_host, default_tools, default_storage) =
        load_config(std::iter::empty()).unwrap();
    let default_storage = default_storage.unwrap();

    let user = ["--config", user_path.to_str().unwrap()];
    let host = ["--host-config", host_path.to_str().unwrap()];
    let cases = [
        ("--config", load("--config", user_path)),
        ("--host-config", load("--host-config", host_path)),
        (
            "--config and --host-config",
            load_config(user.into_iter().chain(host).map(str::to_owned)),
        ),
    ];

    for (flags, result) in cases {
        let (api, host, tools, storage) = result.expect(flags);
        assert_eq!(api, default_api, "{flags}");
        assert_eq!(host, default_host, "{flags}");
        assert_eq!(tools, default_tools, "{flags}");
        assert_eq!(storage.expect(flags), default_storage, "{flags}");
    }
}

#[test]
fn user_file_is_split_between_api_and_tools() {
    let path = user_file(
        "daemon-split",
        b"max_sessions = 3\n[workdirs]\ndefault = 'C:/w'\n[tools.codex]\nargs = ['x']\n[storage]\nbusy_timeout_ms = 42\n",
    );
    let (api, _, tools, storage) = load("--config", &path).unwrap();
    assert_eq!(api.max_sessions, 3);
    assert_eq!(storage.unwrap().busy_timeout_ms(), 42);
    let tools: toml::Table = toml::from_str(&tools).unwrap();
    assert_eq!(
        tools.keys().collect::<Vec<_>>(),
        ["tools", "workdirs"].iter().collect::<Vec<_>>()
    );
}

#[test]
fn loads_user_and_host_configuration_in_either_order() {
    let user_path = user_file("daemon-both-user", b"max_sessions = 7\n");
    let host_path = user_file("daemon-both-host", b"replay_capacity_bytes = 16384\n");
    let user = ["--config", user_path.to_str().unwrap()];
    let host = ["--host-config", host_path.to_str().unwrap()];
    let (default_api, default_host, ..) = load_config(std::iter::empty()).unwrap();
    assert_ne!(default_api.max_sessions, 7);
    assert_ne!(default_host.replay_capacity_bytes, 16_384);

    for [first, second] in [[user, host], [host, user]] {
        let args = first.into_iter().chain(second).map(str::to_owned);
        let (api, host, ..) = load_config(args).expect(first[0]);
        assert_eq!(api.max_sessions, 7, "{} first", first[0]);
        assert_eq!(host.replay_capacity_bytes, 16_384, "{} first", first[0]);
    }
}

#[test]
fn configuration_paths_must_be_literal_absolute_regular_small_files() {
    for flag in ["--config", "--host-config"] {
        for path in [
            "sentinel.toml",
            r"\\server\share\sentinel.toml",
            r"\\.\pipe\sentinel",
            r"\\?\C:\sentinel.toml",
            "/tmp/%SENTINEL%/x.toml",
            "/tmp/../sentinel.toml",
        ] {
            let error = load(flag, path).unwrap_err();
            assert!(
                error.starts_with(&format!("{flag} path refused: ")),
                "{error}"
            );
            assert!(!error.contains("sentinel"), "{error}");
        }
        assert_eq!(
            load(flag, temp::directory()).unwrap_err(),
            format!("{flag} must name a regular file")
        );
        let large = user_file("daemon-large", &vec![b'#'; MAX_CONFIG_BYTES as usize + 1]);
        assert_eq!(
            load(flag, &large).unwrap_err(),
            format!("{flag} file exceeds {MAX_CONFIG_BYTES} bytes")
        );
        let missing = temp::directory().join("daemon-missing.toml");
        assert_eq!(
            load(flag, missing).unwrap_err(),
            format!("cannot read {flag} file")
        );
    }
    let limit = user_file("daemon-limit", &vec![b'#'; MAX_CONFIG_BYTES as usize]);
    for flag in ["--config", "--host-config"] {
        let _ = load(flag, &limit).unwrap();
    }
}

#[test]
fn invalid_utf8_is_refused_without_quoting_the_file() {
    let path = user_file("daemon-invalid-utf8", b"# sentinel \xff\n");
    let results = ["--config", "--host-config"].map(|flag| load(flag, &path).map(|_| ()));
    assert_eq!(
        results,
        [
            Err("--config file is not UTF-8".into()),
            Err("--host-config file is not UTF-8".into()),
        ]
    );
}

#[test]
fn unknown_tool_and_bad_toml_are_refused_without_quoting_the_file() {
    let path = user_file("daemon-unknown", b"[tools.sentinel-tool]\nprogram = ''\n");
    assert_eq!(
        load("--config", &path).unwrap_err(),
        "unknown tool in user configuration"
    );
    let path = user_file("daemon-bad", b"max_sessions = 1\nsentinel = = 'x'\n");
    assert_eq!(
        load("--config", &path).unwrap_err(),
        "invalid user configuration TOML at line 2"
    );
}

#[test]
fn invalid_user_configuration_stops_the_daemon_without_quoting_it() {
    let error = refused(
        "daemon-relative-program",
        "[tools.codex]\nprogram = 'relative-sentinel.exe'\n",
    );
    assert!(error.contains("invalid codex configuration: program path refused"));

    // `[env]` is shared by both tools: its error names neither.
    let error = refused(
        "daemon-denied-variable",
        "[env]\nallow = ['PATH', 'SENTINEL_TOKEN']\n",
    );
    assert!(
        error.contains("invalid user configuration: denied variable name at `env.allow` entry 1")
    );

    let error = refused("daemon-unknown-tool", "[tools.sentinel]\nprogram = ''\n");
    assert!(error.contains("unknown tool in user configuration"));

    let error = refused("daemon-bad-toml", "[tools.claude]\nprogram = sentinel\n");
    assert!(error.contains("invalid user configuration TOML at line 2"));

    let error = refused("daemon-unknown-key", "[tools.claude]\nsentinel = 'x'\n");
    assert!(error.contains("invalid claude configuration: invalid harness parameters"));

    // The tools see a rebuilt part of the file: a line number would point elsewhere.
    let error = refused(
        "daemon-bad-folder",
        "max_sessions = 3\n\n[workdirs]\nsentinel = 5\n",
    );
    assert!(error.ends_with("invalid claude configuration: invalid harness parameters\n"));

    let error = refused("daemon-tools-type", "tools = 5\n");
    assert!(error.contains("invalid claude configuration"), "{error}");
}

#[test]
fn relative_configuration_path_is_refused_by_the_binary() {
    let output = Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
        .args(["--config", "sentinel.toml"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("--config path refused"), "{error}");
    assert!(!error.contains("sentinel"));
}

#[test]
fn refuses_an_installation_with_a_live_daemon_lock() {
    let path = user_file("daemon-instance", b"");
    let lock = std::fs::File::options()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    lock.try_lock().unwrap();
    let start = || {
        Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
            .arg("--instance-lock")
            .arg(&path)
            .arg("--config")
            .output()
            .unwrap()
    };
    let blocked = start();
    assert!(!blocked.status.success());
    assert!(blocked.stdout.is_empty());
    let error = String::from_utf8(blocked.stderr).unwrap();
    assert!(error.contains("another daemon"), "{error}");
    // A child spawned meanwhile by another test keeps a copy of the descriptor until it
    // execs, so closing ours does not free the lock; the copy here stands for that child.
    let copy_held_by_a_child = lock.try_clone().unwrap();
    lock.unlock().unwrap();
    drop(lock);
    let available = start();
    drop(copy_held_by_a_child);
    assert!(!available.status.success());
    let error = String::from_utf8(available.stderr).unwrap();
    assert!(!error.contains("another daemon"), "{error}");
    assert!(error.contains("invalid daemon arguments"), "{error}");
}

#[test]
fn user_file_is_removed_on_drop() {
    let path = {
        let file = user_file("daemon-drop-test", b"# drop test\n");
        let path = file.path().to_path_buf();
        assert!(
            path.try_exists().unwrap(),
            "file should exist while guard is held"
        );
        path
    };
    assert!(
        !path.try_exists().unwrap(),
        "file should be removed after guard is dropped: {path:?}"
    );
}

#[test]
fn user_file_is_removed_on_panic() {
    let mut file_path = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let file = user_file("daemon-drop-panic-test", b"# drop panic\n");
        assert!(file.path().try_exists().unwrap());
        file_path = Some(file.path().to_path_buf());
        panic!("unwind with user file guard");
    }));
    assert!(result.is_err());
    let path = file_path.expect("path should be captured");
    assert!(
        !path.try_exists().unwrap(),
        "file should be removed even on panic: {path:?}"
    );
}

#[path = "common/temp.rs"]
mod temp;
