use ads_contracts::{
    CommandSessionOpenV1Params, ContractPtyHostRequestV1SessionOpenParams as OpenParams,
};
pub use ads_core::tool_launch::{DiskProbe, LaunchError, Probe, process_env};

use crate::CodexConfig;

/// The open parameters for a Codex session, or why there are none. Takes only the Codex
/// configuration; a request for another tool is still refused with
/// [`LaunchError::UnknownHarness`].
///
/// The two examples differ only in the configuration's tool, which a unit test checks: the
/// first compiles, so the second fails only because the configuration is another tool's.
///
/// ```
/// use ads_adapter_codex::{Codex, DiskProbe, open_params};
/// use ads_contracts::CommandSessionOpenV1Params;
/// use ads_core::tool_launch::HostRules;
/// use ads_core::tool_launch::ToolConfig;
///
/// ads_core::tool_harness!(Other = "other");
///
/// fn open(request: &CommandSessionOpenV1Params) {
///     let config = ToolConfig::<Codex>::defaults(HostRules::POSIX);
///     let _ = open_params(&config, request, Vec::<(String, String)>::new(), &DiskProbe);
/// }
/// ```
///
/// ```compile_fail
/// use ads_adapter_codex::{Codex, DiskProbe, open_params};
/// use ads_contracts::CommandSessionOpenV1Params;
/// use ads_core::tool_launch::HostRules;
/// use ads_core::tool_launch::ToolConfig;
///
/// ads_core::tool_harness!(Other = "other");
///
/// fn open(request: &CommandSessionOpenV1Params) {
///     let config = ToolConfig::<Other>::defaults(HostRules::POSIX);
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

#[cfg(test)]
mod tests {
    /// The doc examples of [`super::open_params`] in order, each as its fence and its lines.
    fn examples() -> Vec<(&'static str, Vec<&'static str>)> {
        let mut blocks = Vec::new();
        let mut current: Option<(&str, Vec<&str>)> = None;
        for line in include_str!("launch.rs").lines() {
            let Some(text) = line.strip_prefix("///") else {
                continue;
            };
            let text = text.strip_prefix(' ').unwrap_or(text);
            match current.take() {
                None if text.starts_with("```") => current = Some((text, Vec::new())),
                None => {}
                Some(block) if text == "```" => blocks.push(block),
                Some((fence, mut lines)) => {
                    lines.push(text);
                    current = Some((fence, lines));
                }
            }
        }
        blocks
    }

    #[test]
    fn the_failing_example_differs_from_the_passing_one_only_in_the_tool() {
        let blocks = examples();
        assert_eq!(blocks.len(), 2);
        let (passing_fence, passing) = &blocks[0];
        let (failing_fence, failing) = &blocks[1];
        assert_eq!((*passing_fence, *failing_fence), ("```", "```compile_fail"));
        let renamed: Vec<String> = passing
            .iter()
            .map(|line| line.replace("ToolConfig::<Codex>", "ToolConfig::<Other>"))
            .collect();
        assert_eq!(&renamed, failing);
        assert!(
            passing
                .iter()
                .any(|line| line.contains("ToolConfig::<Codex>"))
        );
    }
}
