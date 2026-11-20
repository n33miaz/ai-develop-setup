//! Adaptação de session.open para os parâmetros do host.

use crate::api::{ApiError, HarnessCatalog, Launcher};
use ads_adapter_claude as claude;
use ads_adapter_codex as codex;
use ads_contracts::CommandSessionOpenV1Params;
use ads_core::child_env::NameCase;
use ads_ptyhost::wire::OpenParams;
use std::collections::BTreeMap;

pub struct ToolLauncher<C, X> {
    claude: claude::ClaudeConfig,
    codex: codex::CodexConfig,
    /// The parent environment after the `[env]` policy: credentials are never kept.
    child_env: BTreeMap<String, String>,
    claude_probe: C,
    codex_probe: X,
}

/// `[env]` and `[workdirs]` are shared by every tool, so their errors name no tool.
fn describe(tool: &str, shared: bool, error: impl std::fmt::Display) -> String {
    if shared {
        format!("invalid user configuration: {error}")
    } else {
        format!("invalid {tool} configuration: {error}")
    }
}

impl<C, X> ToolLauncher<C, X> {
    /// Filters `parent_env` once; only the result is kept.
    pub fn from_toml_str<I, K, V>(
        overrides: &str,
        parent_env: I,
        claude_probe: C,
        codex_probe: X,
    ) -> Result<Self, String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        use claude::ConfigError as A;
        use codex::ConfigError as B;
        let case = NameCase::host();
        // The daemon passes a rebuilt subset of the user's file: line numbers would point to
        // the wrong place, so they are dropped.
        let claude = claude::ClaudeConfig::from_toml_str(overrides, case).map_err(|error| {
            let shared = matches!(
                error,
                A::Env(_) | A::Alias(_) | A::Workdir { .. } | A::WorkdirTooLong(_)
            );
            let error = match error {
                A::Toml { .. } => A::Toml { line: None },
                other => other,
            };
            describe(claude::HARNESS_ID, shared, error)
        })?;
        let codex = codex::CodexConfig::from_toml_str(overrides, case).map_err(|error| {
            let shared = matches!(
                error,
                B::Env(_) | B::Alias(_) | B::Workdir { .. } | B::WorkdirTooLong(_)
            );
            let error = match error {
                B::Toml { .. } => B::Toml { line: None },
                other => other,
            };
            describe(codex::HARNESS_ID, shared, error)
        })?;
        if claude.env() != codex.env() {
            return Err("invalid user configuration: tools disagree on `[env]`".into());
        }
        let child_env = claude.env().child_env(parent_env);
        Ok(Self {
            claude,
            codex,
            child_env,
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
        // Already filtered; the adapters apply the same policy again, which keeps it unchanged.
        let env = self
            .child_env
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()));
        match request.harness.as_str() {
            claude::HARNESS_ID => {
                claude::open_params(&self.claude, request, env, &self.claude_probe).map_err(
                    |error| match error {
                        claude::LaunchError::Disabled => ApiError::HarnessNotConfigured,
                        _ => ApiError::Invalid,
                    },
                )
            }
            codex::HARNESS_ID => codex::open_params(&self.codex, request, env, &self.codex_probe)
                .map_err(|error| match error {
                    codex::LaunchError::Disabled => ApiError::HarnessNotConfigured,
                    _ => ApiError::Invalid,
                }),
            _ => Err(ApiError::Invalid),
        }
    }

    fn catalog(&self) -> HarnessCatalog {
        let harnesses = [
            (claude::HARNESS_ID, self.claude.program().is_some()),
            (codex::HARNESS_ID, self.codex.program().is_some()),
        ]
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(id, _)| id.to_owned())
        .collect();
        // `[workdirs]` is shared, so both adapters hold the same aliases.
        HarnessCatalog {
            harnesses,
            workdir_aliases: self.claude.workdir_aliases().map(str::to_owned).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_filtered_environment_is_kept() {
        let launcher = ToolLauncher::from_toml_str(
            "",
            [
                ("PATH", "p"),
                ("ANTHROPIC_API_KEY", "sentinel"),
                ("GH_TOKEN", "sentinel"),
                ("UNLISTED", "sentinel"),
            ],
            claude::DiskProbe,
            codex::DiskProbe,
        )
        .unwrap();
        assert_eq!(
            launcher.child_env,
            BTreeMap::from(
                [
                    ("PATH", "p"),
                    ("TERM", "xterm-256color"),
                    ("COLORTERM", "truecolor"),
                ]
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
            )
        );
    }
}
