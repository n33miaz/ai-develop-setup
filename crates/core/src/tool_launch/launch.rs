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

/// Checks whether the configured program and working folder exist.
pub trait Probe {
    fn is_file(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;
}

/// The real file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskProbe;

impl Probe for DiskProbe {
    fn is_file(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn is_dir(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }
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
