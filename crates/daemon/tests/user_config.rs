use ads_daemon::startup::{MAX_CONFIG_BYTES, load_config};
use std::path::{Path, PathBuf};
use std::process::Command;

fn user_file(name: &str, contents: &[u8]) -> PathBuf {
    let path = temp::directory().join(format!("{name}-{}.toml", std::process::id()));
    std::fs::write(&path, contents).unwrap();
    path
}

fn load(flag: &str, path: impl AsRef<Path>) -> Result<ads_daemon::startup::StartupConfig, String> {
    let path = path.as_ref().to_str().unwrap().to_owned();
    load_config([flag.to_owned(), path].into_iter())
}

/// Runs the daemon with a user file it must refuse before contacting the terminal host.
fn refused(name: &str, contents: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
        .arg("--config")
        .arg(user_file(name, contents.as_bytes()))
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
fn user_file_is_split_between_api_and_tools() {
    let path = user_file(
        "daemon-split",
        b"max_sessions = 3\n[workdirs]\ndefault = 'C:/w'\n[tools.codex]\nargs = ['x']\n[storage]\nbusy_timeout_ms = 42\n",
    );
    let (api, _, tools, storage) = load("--config", path).unwrap();
    assert_eq!(api.max_sessions, 3);
    assert_eq!(storage.unwrap().busy_timeout_ms(), 42);
    let tools: toml::Table = toml::from_str(&tools).unwrap();
    assert_eq!(
        tools.keys().collect::<Vec<_>>(),
        ["tools", "workdirs"].iter().collect::<Vec<_>>()
    );
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
            load(flag, large).unwrap_err(),
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
        load("--config", path).unwrap_err(),
        "unknown tool in user configuration"
    );
    let path = user_file("daemon-bad", b"max_sessions = 1\nsentinel = = 'x'\n");
    assert_eq!(
        load("--config", path).unwrap_err(),
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
#[path = "common/temp.rs"]
mod temp;
