use ads_contracts::{
    CommandSessionOpenV1Params, ContractPtyHostRequestV1SessionOpenParams as OpenParams,
};
pub use ads_core::tool_launch::{DiskProbe, LaunchError, Probe, process_env};

use crate::CodexConfig;

/// The open parameters for a Codex session, or why there are none. Takes only the Codex
/// configuration; a request for another tool is still refused with
/// [`LaunchError::UnknownHarness`].
///
/// ```
/// # use ads_adapter_codex::{CodexConfig, DiskProbe, open_params};
/// # use ads_contracts::CommandSessionOpenV1Params;
/// # use ads_core::child_env::NameCase;
/// fn open(request: &CommandSessionOpenV1Params) {
///     let config = CodexConfig::defaults(NameCase::Sensitive);
///     let _ = open_params(&config, request, Vec::<(String, String)>::new(), &DiskProbe);
/// }
/// ```
///
/// ```compile_fail,E0308
/// # use ads_adapter_codex::{DiskProbe, open_params};
/// # use ads_contracts::CommandSessionOpenV1Params;
/// # use ads_core::child_env::NameCase;
/// # use ads_core::tool_launch::{Claude, ToolConfig};
/// fn open(request: &CommandSessionOpenV1Params) {
///     let config = ToolConfig::<Claude>::defaults(NameCase::Sensitive);
///     let _ = open_params(&config, request, Vec::<(String, String)>::new(), &DiskProbe);
/// }
/// ```
pub fn open_params<I, K, V>(
    config: &CodexConfig,
    request: &CommandSessionOpenV1Params,
    parent_env: I,
    probe: &impl Probe,
) -> Result<OpenParams, LaunchError>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    ads_core::tool_launch::open_params(config, request, parent_env, probe)
}
