//! Supervises the daemon and terminal host for one installation.

mod supervisor;

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    daemon_program: PathBuf,
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
    rapid_exit_ms: u64,
    rapid_exit_limit: u32,
}

impl Config {
    fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut defaults: toml::Table =
            toml::from_str(include_str!("../../../defaults/launcher.toml"))?;
        for key in ["daemon_program", "ptyhost_program"] {
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
        Ok(config)
    }

    fn validate(&self) -> io::Result<()> {
        if self.poll_ms == 0
            || self.restart_initial_ms == 0
            || self.restart_max_ms < self.restart_initial_ms
            || self.rapid_exit_ms == 0
            || self.rapid_exit_limit == 0
        {
            return Err(io::Error::other("invalid launcher interval or retry limit"));
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
}

fn child_command(base: &Path, program: &Path, config: Option<&Path>) -> Command {
    let mut command = Command::new(base.join(program));
    command.env_clear();
    // Read only system and channel settings needed by the local product processes.
    for name in [
        "SystemRoot",
        "windir",
        "ComSpec",
        "PATH",
        "PATHEXT",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "HOMEDRIVE",
        "HOMEPATH",
        "TEMP",
        "TMP",
        "HOME",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "XDG_RUNTIME_DIR",
    ] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    add_config(&mut command, "--config", base, config);
    command
}

fn add_config(command: &mut Command, flag: &str, base: &Path, path: Option<&Path>) {
    if let Some(path) = path {
        command.arg(flag).arg(base.join(path));
    }
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
    let base = config_path.as_deref().unwrap_or(&exe).parent().unwrap();
    let config = Config::load(config_path.as_deref())?;
    let mut daemon = child_command(
        base,
        &config.daemon_program,
        config.daemon_config.as_deref(),
    );
    add_config(
        &mut daemon,
        "--host-config",
        base,
        config.ptyhost_config.as_deref(),
    );
    let host = child_command(
        base,
        &config.ptyhost_program,
        config.ptyhost_config.as_deref(),
    );
    supervisor::run(&config, &base.join(&config.control_dir), daemon, host)?;
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    run().unwrap_or_else(|error| {
        eprintln!("ads-launcher: {error}");
        ExitCode::FAILURE
    })
}
