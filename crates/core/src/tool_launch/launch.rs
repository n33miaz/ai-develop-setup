//! Turns a session request into the terminal host's open parameters. The requester picks only
//! the tool, the folder alias and the size; everything that runs comes from [`ToolConfig`].

use std::fmt;
use std::path::Path;

use ads_contracts::{
    CommandSessionOpenV1Params, ContractPtyHostRequestV1SessionOpenParams as OpenParams,
    ContractPtyHostRequestV1SessionOpenParamsCols as Cols,
    ContractPtyHostRequestV1SessionOpenParamsRows as Rows,
};

use super::{Harness, Mode, ToolConfig};
use crate::gate::{AbsPath, PathStyle};

/// Checks whether the configured program and working folder exist.
pub trait Probe {
    fn is_file(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;
}

/// The real file system. A path absolute only in the other system's style is never found: on
/// Windows `/x` is relative to the current drive, and elsewhere `C:\x` is a relative name.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskProbe;

impl Probe for DiskProbe {
    fn is_file(&self, path: &str) -> bool {
        absolute_on(path, host_style()) && Path::new(path).is_file()
    }

    fn is_dir(&self, path: &str) -> bool {
        absolute_on(path, host_style()) && Path::new(path).is_dir()
    }
}

const fn host_style() -> PathStyle {
    if cfg!(windows) {
        PathStyle::Windows
    } else {
        PathStyle::Posix
    }
}

/// Whether `path` is a literal absolute path in `style`, written as the configuration keeps it.
fn absolute_on(path: &str, style: PathStyle) -> bool {
    AbsPath::parse(path).is_ok_and(|parsed| parsed.style() == style && parsed.to_string() == path)
}

/// Why no session is opened. Carries no path, value or variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    /// The request names another tool.
    UnknownHarness,
    /// The program path is empty in the configuration.
    Disabled,
    /// The configured program is not an existing file.
    ProgramMissing,
    /// The alias is not in the configuration.
    UnknownWorkdir,
    /// The configured folder is not an existing folder.
    WorkdirMissing,
    /// A size the terminal host does not accept.
    Size,
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownHarness => "unknown tool",
            Self::Disabled => "tool is off",
            Self::ProgramMissing => "configured program not found",
            Self::UnknownWorkdir => "unknown folder alias",
            Self::WorkdirMissing => "configured folder not found",
            Self::Size => "invalid terminal size",
        })
    }
}

impl std::error::Error for LaunchError {}

/// The open parameters for `request`, or why there are none. Pure apart from `probe`.
///
/// `parent_env` is the environment of the calling process; only what the policy allows reaches
/// the child.
pub fn open_params<I, K, V>(
    config: &ToolConfig<impl Harness>,
    request: &CommandSessionOpenV1Params,
    parent_env: I,
    probe: &impl Probe,
) -> Result<OpenParams, LaunchError>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    if request.harness.as_str() != config.harness_id() {
        return Err(LaunchError::UnknownHarness);
    }
    match config.mode() {
        Mode::Interactive => {}
    }
    let program = config.program().ok_or(LaunchError::Disabled)?;
    if !probe.is_file(program.as_str()) {
        return Err(LaunchError::ProgramMissing);
    }
    let cwd = config
        .workdir(&request.workdir_alias)
        .ok_or(LaunchError::UnknownWorkdir)?;
    if !probe.is_dir(cwd.as_str()) {
        return Err(LaunchError::WorkdirMissing);
    }
    Ok(OpenParams {
        session_id: request.session_id.clone(),
        program: program.clone(),
        args: config.args().to_vec(),
        // Always set: without it the host would use its own current folder.
        cwd: Some(cwd.clone()),
        env: config.env().child_env(parent_env),
        cols: Cols::new(request.cols.get()).map_err(|_| LaunchError::Size)?,
        rows: Rows::new(request.rows.get()).map_err(|_| LaunchError::Size)?,
    })
}

/// The current process environment, skipping names or values that are not Unicode.
pub fn process_env() -> Vec<(String, String)> {
    std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_path_in_the_style_of_the_system_is_looked_up() {
        for (path, style, other) in [
            (r"C:\Tools\claude.exe", PathStyle::Windows, PathStyle::Posix),
            (
                r"C:\Program Files\work dir",
                PathStyle::Windows,
                PathStyle::Posix,
            ),
            ("/usr/local/bin/codex", PathStyle::Posix, PathStyle::Windows),
            (
                "/home/someone/work dir",
                PathStyle::Posix,
                PathStyle::Windows,
            ),
        ] {
            assert!(absolute_on(path, style), "{path}");
            assert!(!absolute_on(path, other), "{path}");
        }
        // Not written as the configuration keeps it: never looked up.
        for path in [
            "c:/Tools/claude.exe",
            "/usr//bin",
            r"\\?\C:\x",
            "tool.exe",
            "",
        ] {
            assert!(!absolute_on(path, PathStyle::Windows), "{path}");
            assert!(!absolute_on(path, PathStyle::Posix), "{path}");
        }
    }

    #[test]
    fn the_disk_probe_does_not_find_a_folder_written_for_the_other_system() {
        if cfg!(windows) {
            // `\Windows` written as `/Windows` resolves against the current drive.
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
            let posix = root.get(2..).unwrap_or_default().replace('\\', "/");
            assert!(DiskProbe.is_dir(&root), "{root}");
            assert!(Path::new(&posix).is_dir(), "{posix}");
            assert!(!DiskProbe.is_dir(&posix), "{posix}");
        } else {
            assert!(DiskProbe.is_dir("/"));
            assert!(!DiskProbe.is_dir(r"C:\"));
        }
    }
}
