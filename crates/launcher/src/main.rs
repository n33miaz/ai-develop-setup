//! Supervises the daemon and terminal host for one installation.

use ads_private_dir as private;
mod supervisor;

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use ads_core::child_env::{EnvLayer, EnvPolicy, NameCase};
use serde::Deserialize;

const DAEMON_LOCK_FILE: &str = "daemon.lock";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    daemon_program: PathBuf,
    shell_program: PathBuf,
    shell_autostart: bool,
    shell_width: u16,
    shell_height: u16,
    shell_start_minimized: bool,
    ptyhost_program: PathBuf,
    daemon_config: Option<PathBuf>,
    ptyhost_config: Option<PathBuf>,
    control_dir: PathBuf,
    lock_file: String,
    state_file: String,
    restart_file: String,
    stop_file: String,
    poll_ms: u64,
    restart_initial_ms: u64,
    restart_max_ms: u64,
    crash_window_ms: u64,
    crash_window_limit: usize,
    child_env_allow: Vec<String>,
}

impl Config {
    fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut defaults: toml::Table =
            toml::from_str(include_str!("../../../defaults/launcher.toml"))?;
        for key in ["daemon_program", "ptyhost_program", "shell_program"] {
            let program = defaults[key].as_str().expect("default program name");
            defaults.insert(
                key.into(),
                format!("{program}{}", env::consts::EXE_SUFFIX).into(),
            );
        }
        if let Some(path) = path {
            let overrides: toml::Table = toml::from_str(&fs::read_to_string(path)?)
                .map_err(|_| io::Error::other("invalid launcher configuration TOML"))?;
            defaults.extend(overrides);
        }
        let config: Self = defaults
            .try_into()
            .map_err(|_| io::Error::other("unknown launcher setting or invalid setting type"))?;
        config.validate()?;
        config.validate_child_env()?;
        Ok(config)
    }

    fn validate(&self) -> io::Result<()> {
        if self.poll_ms == 0
            || self.restart_initial_ms == 0
            || self.restart_max_ms < self.restart_initial_ms
            || self.crash_window_ms == 0
            || self.crash_window_limit == 0
        {
            return Err(io::Error::other("invalid launcher interval or retry limit"));
        }
        if self.shell_width == 0 || self.shell_height == 0 {
            return Err(io::Error::other("invalid shell window dimensions"));
        }
        let names = [
            &self.lock_file,
            &self.state_file,
            &self.restart_file,
            &self.stop_file,
        ];
        for (i, name) in names.iter().enumerate() {
            if name.is_empty()
                || name.starts_with('.')
                || name.ends_with('.')
                || name.contains("..")
                || name.as_str() == DAEMON_LOCK_FILE
                || reserved_control_name(name)
                || !name.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
                })
                || names[..i].contains(name)
            {
                return Err(io::Error::other("invalid or repeated control filename"));
            }
        }
        if [
            &self.daemon_program,
            &self.shell_program,
            &self.ptyhost_program,
            &self.control_dir,
        ]
        .iter()
        .any(|path| path.as_os_str().is_empty())
        {
            return Err(io::Error::other("launcher paths must not be empty"));
        }
        Ok(())
    }

    fn desktop_enabled(&self) -> bool {
        cfg!(windows) && self.shell_autostart
    }

    fn validate_child_env(&self) -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Deserialize)]
        struct HarnessDefaults {
            env: EnvLayer,
        }

        let defaults: HarnessDefaults =
            toml::from_str(include_str!("../../../defaults/harnesses.toml"))?;
        let layer = EnvLayer {
            allow: Some(self.child_env_allow.clone()),
            set: Some(Default::default()),
            ..EnvLayer::default()
        };
        EnvPolicy::from_layers(&defaults.env, Some(&layer), NameCase::host())?;
        Ok(())
    }
}

fn reserved_control_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default();
    matches!(stem, "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|suffix| matches!(suffix.as_bytes(), [b'0'..=b'9']))
        })
}

fn child_command(program: &Path, allowed: &[String]) -> Command {
    let mut command = Command::new(program);
    command.env_clear().envs(
        allowed
            .iter()
            .filter_map(|name| env::var_os(name).map(|value| (name, value))),
    );
    command
}

fn add_config(command: &mut Command, flag: &str, base: &Path, path: Option<&Path>) {
    if let Some(path) = path {
        command.arg(flag).arg(base.join(path));
    }
}

fn installation_commands(
    config: &Config,
    install: &Path,
    config_base: &Path,
) -> (PathBuf, Command, Command) {
    let control = install.join(&config.control_dir);
    let mut daemon = child_command(
        &install.join(&config.daemon_program),
        &config.child_env_allow,
    );
    daemon
        .arg("--instance-lock")
        .arg(control.join(DAEMON_LOCK_FILE));
    if config.desktop_enabled() {
        daemon.arg("--desktop");
    }
    add_config(
        &mut daemon,
        "--config",
        config_base,
        config.daemon_config.as_deref(),
    );
    add_config(
        &mut daemon,
        "--host-config",
        config_base,
        config.ptyhost_config.as_deref(),
    );
    let mut host = child_command(
        &install.join(&config.ptyhost_program),
        &config.child_env_allow,
    );
    add_config(
        &mut host,
        "--config",
        config_base,
        config.ptyhost_config.as_deref(),
    );
    (control, daemon, host)
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let config_path = match args.as_slice() {
        [] => None,
        [flag, path] if flag == "--config" => Some(std::path::absolute(path)?),
        [flag] if flag == "--version" => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return Ok(ExitCode::SUCCESS);
        }
        _ => return Err("usage: ads-launcher [--config <file> | --version]".into()),
    };
    let exe = env::current_exe()?;
    let install = exe.parent().unwrap();
    let config_base = config_path.as_deref().unwrap_or(&exe).parent().unwrap();
    let config = Config::load(config_path.as_deref())?;
    let (control, daemon, host) = installation_commands(&config, install, config_base);
    let shell = config.desktop_enabled().then(|| {
        let mut shell = child_command(
            &install.join(&config.shell_program),
            &config.child_env_allow,
        );
        for name in &config.child_env_allow {
            if name.to_ascii_uppercase().starts_with("WEBVIEW2_") {
                shell.env_remove(name);
            }
        }
        shell.args([
            config.shell_width.to_string(),
            config.shell_height.to_string(),
        ]);
        if config.shell_start_minimized {
            shell.arg("--minimized");
        }
        shell
    });
    supervisor::run(&config, &control, daemon, host, shell)?;
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    run().unwrap_or_else(|error| {
        eprintln!("ads-launcher: {error}");
        ExitCode::FAILURE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_and_repeated_control_names() {
        for name in [
            "",
            "..",
            "../stop",
            "a..b",
            "a/b",
            r"a\b",
            "/stop",
            r"C:\stop",
            "Stop",
            ".x",
            "state.json",
            "daemon.lock",
            "con",
            "nul.txt",
            "lpt1",
        ] {
            let mut config = Config::load(None).unwrap();
            config.stop_file = name.into();
            assert!(config.validate().is_err(), "{name:?}");
        }
        let mut config = Config::load(None).unwrap();
        config.restart_max_ms = config.restart_initial_ms - 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn loads_partial_configuration_and_preserves_defaults() {
        let dir = supervisor::tests::TestDir::new();
        let path = dir.0.join("launcher.toml");
        fs::write(
            &path,
            "poll_ms = 12\nchild_env_allow = ['USER']\ndaemon_config = 'daemon.toml'",
        )
        .unwrap();
        let config = Config::load(Some(&path)).unwrap();
        let defaults = Config::load(None).unwrap();
        assert_eq!(config.poll_ms, 12);
        assert_eq!(config.child_env_allow, ["USER"]);
        assert_eq!(config.daemon_program, defaults.daemon_program);
        assert_eq!(config.control_dir, defaults.control_dir);
        let install = dir.0.join("install");
        let (control, daemon, host) = installation_commands(&config, &install, &dir.0);
        assert_eq!(control, install.join(defaults.control_dir));
        assert_eq!(daemon.get_program(), install.join(defaults.daemon_program));
        assert_eq!(host.get_program(), install.join(defaults.ptyhost_program));
        let args: Vec<_> = daemon.get_args().collect();
        assert_eq!(args[0], "--instance-lock");
        assert_eq!(args[1], control.join(DAEMON_LOCK_FILE));
        let config_index = if config.desktop_enabled() {
            assert_eq!(args[2], "--desktop");
            3
        } else {
            2
        };
        assert_eq!(args[config_index], "--config");
        assert_eq!(args[config_index + 1], dir.0.join("daemon.toml"));
    }

    // The parent fixture receives synthetic values without changing this test runner's environment.
    const AUTH_NAMES: [&str; 3] = ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GITHUB_TOKEN"];
    const ENV_PROBE: &str = "ADS_LAUNCHER_ENV_PROBE";

    #[test]
    fn child_environment_fixture() {
        let probe = env::var(ENV_PROBE);
        if probe.as_deref() == Ok("filtered") {
            assert_eq!(env::var("ADS_LAUNCHER_INCLUDED").unwrap(), "synthetic");
            for name in AUTH_NAMES.into_iter().chain(["ADS_LAUNCHER_EXCLUDED"]) {
                assert!(env::var_os(name).is_none(), "{name}");
            }
        } else if probe.as_deref() == Ok("parent") {
            let mut config = Config::load(None).unwrap();
            config.child_env_allow.push("ADS_LAUNCHER_INCLUDED".into());
            config.validate_child_env().unwrap();
            for name in AUTH_NAMES {
                assert_eq!(env::var(name).unwrap(), "synthetic");
                config.child_env_allow.push(name.into());
                assert!(config.validate_child_env().is_err());
                config.child_env_allow.pop();
            }
            let mut child = child_command(&env::current_exe().unwrap(), &config.child_env_allow);
            let status = child
                .args(["--exact", "tests::child_environment_fixture"])
                .env(ENV_PROBE, "filtered")
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    #[test]
    fn filters_credentials_from_the_spawned_child() {
        let status = Command::new(env::current_exe().unwrap())
            .args(["--exact", "tests::child_environment_fixture"])
            .env(ENV_PROBE, "parent")
            .env("ADS_LAUNCHER_INCLUDED", "synthetic")
            .env("ADS_LAUNCHER_EXCLUDED", "synthetic")
            .envs(AUTH_NAMES.map(|name| (name, "synthetic")))
            .status()
            .unwrap();
        assert!(status.success());
    }
}
