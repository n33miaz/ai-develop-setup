//! Adaptação de session.open para os parâmetros do host.

use crate::api::{ApiError, Launcher};
use ads_adapter_claude as claude;
use ads_adapter_codex as codex;
use ads_contracts::CommandSessionOpenV1Params;
use ads_core::child_env::NameCase;
use ads_ptyhost::wire::OpenParams;

pub struct ToolLauncher<C, X> {
    claude: claude::ClaudeConfig,
    codex: codex::CodexConfig,
    parent_env: Vec<(String, String)>,
    claude_probe: C,
    codex_probe: X,
}

impl<C, X> ToolLauncher<C, X> {
    pub fn from_toml_str(
        overrides: &str,
        parent_env: Vec<(String, String)>,
        claude_probe: C,
        codex_probe: X,
    ) -> Result<Self, String> {
        let case = NameCase::host();
        Ok(Self {
            claude: claude::ClaudeConfig::from_toml_str(overrides, case)
                .map_err(|error| format!("invalid claude configuration: {error}"))?,
            codex: codex::CodexConfig::from_toml_str(overrides, case)
                .map_err(|error| format!("invalid codex configuration: {error}"))?,
            parent_env,
            claude_probe,
            codex_probe,
        })
    }
}

impl<C, X> Launcher for ToolLauncher<C, X>
where
    C: claude::Probe + Send + Sync + 'static,
    X: codex::Probe + Send + Sync + 'static,
{
    fn prepare(&self, request: &CommandSessionOpenV1Params) -> Result<OpenParams, ApiError> {
        let parent_env = self
            .parent_env
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()));
        match request.harness.as_str() {
            claude::HARNESS_ID => {
                claude::open_params(&self.claude, request, parent_env, &self.claude_probe).map_err(
                    |error| match error {
                        claude::LaunchError::Disabled => ApiError::HarnessNotConfigured,
                        _ => ApiError::Invalid,
                    },
                )
            }
            codex::HARNESS_ID => {
                codex::open_params(&self.codex, request, parent_env, &self.codex_probe).map_err(
                    |error| match error {
                        codex::LaunchError::Disabled => ApiError::HarnessNotConfigured,
                        _ => ApiError::Invalid,
                    },
                )
            }
            _ => Err(ApiError::Invalid),
        }
    }
}
