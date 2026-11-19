use std::path::{Path, PathBuf};
use std::process::Command;

fn user_file(name: &str, contents: &str) -> PathBuf {
    let path =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}.toml", std::process::id()));
    std::fs::write(&path, contents).unwrap();
    path
}

/// Runs the daemon with a user file it must refuse before contacting the terminal host.
fn refused(name: &str, contents: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
        .arg("--config")
        .arg(user_file(name, contents))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(!error.contains("sentinel"), "{error}");
    error
}

#[test]
fn invalid_user_configuration_stops_the_daemon_without_quoting_it() {
    let error = refused(
        "daemon-relative-program",
        "[tools.codex]\nprogram = 'relative-sentinel.exe'\n",
    );
    assert!(error.contains("invalid codex configuration: program path refused"));

    let error = refused(
        "daemon-denied-variable",
        "[env]\nallow = ['PATH', 'SENTINEL_TOKEN']\n",
    );
    assert!(
        error.contains("invalid claude configuration: denied variable name at `env.allow` entry 1")
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
}

#[test]
fn relative_configuration_path_is_refused() {
    let output = Command::new(env!("CARGO_BIN_EXE_ads-daemon"))
        .args(["--config", "sentinel.toml"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("--config requires an absolute path"));
    assert!(!error.contains("sentinel"));
}
