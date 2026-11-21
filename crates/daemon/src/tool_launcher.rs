//! Adaptação de session.open para os parâmetros do host.

use crate::api::{ApiError, HarnessCatalog, Launcher};
use ads_adapter_claude as claude;
use ads_adapter_codex as codex;
use ads_contracts::CommandSessionOpenV1Params;
use ads_core::child_env::EnvPolicy;
use ads_core::gate::{AbsPath, GatePolicy};
use ads_core::tool_launch::{HostRules, process_env};
use ads_ptyhost::wire::OpenParams;
use std::collections::BTreeMap;
use std::path::Path;

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
    pub fn from_process_env(
        overrides: &str,
        host: HostRules,
        claude_probe: C,
        codex_probe: X,
    ) -> Result<Self, String> {
        Self::with_env(overrides, host, claude_probe, codex_probe, |policy| {
            policy.child_env(process_env(policy))
        })
    }

    /// Filters `parent_env` once; only the result is kept. Paths and names are judged by the
    /// rules of `host`, [`HostRules::host`] outside tests.
    pub fn from_toml_str<I, K, V>(
        overrides: &str,
        host: HostRules,
        parent_env: I,
        claude_probe: C,
        codex_probe: X,
    ) -> Result<Self, String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self::with_env(overrides, host, claude_probe, codex_probe, |policy| {
            policy.child_env(parent_env)
        })
    }

    fn with_env(
        overrides: &str,
        host: HostRules,
        claude_probe: C,
        codex_probe: X,
        child_env: impl FnOnce(&EnvPolicy) -> BTreeMap<String, String>,
    ) -> Result<Self, String> {
        use claude::ConfigError as A;
        use codex::ConfigError as B;
        // The daemon passes a rebuilt subset of the user's file: line numbers would point to
        // the wrong place, so they are dropped.
        let claude = claude::ClaudeConfig::from_toml_str(overrides, host).map_err(|error| {
            let shared = matches!(
                error,
                A::Env(_)
                    | A::Alias(_)
                    | A::Workdir { .. }
                    | A::WorkdirTooLong(_)
                    | A::WorkdirIsRoot(_)
                    | A::WorkdirOtherSystem(_)
            );
            let error = match error {
                A::Toml { .. } => A::Toml { line: None },
                other => other,
            };
            describe(claude::HARNESS_ID, shared, error)
        })?;
        let codex = codex::CodexConfig::from_toml_str(overrides, host).map_err(|error| {
            let shared = matches!(
                error,
                B::Env(_)
                    | B::Alias(_)
                    | B::Workdir { .. }
                    | B::WorkdirTooLong(_)
                    | B::WorkdirIsRoot(_)
                    | B::WorkdirOtherSystem(_)
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
        if claude.has_overlapping_profiles(&codex) {
            return Err(
                "invalid user configuration: account profile folders overlap across tools".into(),
            );
        }
        let child_env = child_env(claude.env());
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

impl<C, X> ToolLauncher<C, X>
where
    C: claude::Probe,
    X: codex::Probe,
{
    /// Binds actions to the working folder used to open this session on the host. The state of
    /// the product is denied as a target whatever `policy` says.
    pub fn session_gate(
        &self,
        opened: &OpenParams,
        policy: GatePolicy,
        product: &crate::audit::ProductLog,
        fence: &Path,
    ) -> Result<crate::gate::SessionGate, String> {
        let cwd = opened
            .cwd
            .as_ref()
            .ok_or("session has no configured working folder")?;
        let root = AbsPath::parse(cwd.as_str()).map_err(|error| error.to_string())?;
        // A configured path may be an alias on disk as well. Protect both spellings.
        let mut roots = Vec::new();
        for dir in self.claude.workdirs() {
            for path in [
                Some(dir.as_str().to_owned()),
                self.claude_probe.resolve_workdir(dir.as_str()),
                self.codex_probe.resolve_workdir(dir.as_str()),
            ]
            .into_iter()
            .flatten()
            {
                let path = AbsPath::parse(&path).map_err(|error| error.to_string())?;
                if !roots.contains(&path) {
                    roots.push(path);
                }
            }
        }
        crate::gate::SessionGate::new(root, roots, policy, product, fence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_configuration_refuses_credentials_before_a_session_exists() {
        let drive = if cfg!(windows) { "C:" } else { "" };
        for (setting, key) in [
            ("allow = ['AWS_PROFILE']", "env.allow"),
            ("set = { AWS_PROFILE = 'fixture' }", "env.set"),
        ] {
            let error = ToolLauncher::from_toml_str(
                &format!("[env]\n{setting}\n[accounts.claude.primary]\nprofile_dir = '{drive}/profiles/primary'"),
                HostRules::host(),
                [("AWS_PROFILE", "fixture")],
                claude::DiskProbe,
                codex::DiskProbe,
            )
            .err();
            assert_eq!(
                error.as_deref(),
                Some(
                    format!("invalid user configuration: denied variable name at `{key}` entry 0")
                        .as_str()
                )
            );
        }
    }

    #[test]
    fn tools_reject_overlapping_profile_folders() {
        let drive = if cfg!(windows) { "C:" } else { "" };
        for second in ["profiles/./main", "profiles/main/nested"] {
            let overrides = format!(
                "[accounts.claude.a]\nprofile_dir = '{drive}/profiles/main'\n[accounts.codex.b]\nprofile_dir = '{drive}/{second}'"
            );
            let error = ToolLauncher::from_toml_str(
                &overrides,
                HostRules::host(),
                Vec::<(String, String)>::new(),
                claude::DiskProbe,
                codex::DiskProbe,
            )
            .err()
            .unwrap();
            assert_eq!(
                error, "invalid user configuration: account profile folders overlap across tools",
                "{second}"
            );
        }
    }

    #[test]
    fn a_root_folder_is_reported_as_a_shared_error() {
        let error = ToolLauncher::from_toml_str(
            "[workdirs]\nscratch = '/'\n",
            HostRules::POSIX,
            Vec::<(String, String)>::new(),
            claude::DiskProbe,
            codex::DiskProbe,
        )
        .err()
        .unwrap();
        assert_eq!(
            error,
            "invalid user configuration: folder at entry 0 is a drive root"
        );
    }

    #[test]
    fn a_path_written_for_another_system_is_refused_at_load() {
        for (host, profile, folder, program) in [
            (
                HostRules::WINDOWS,
                "C:/profiles/primary",
                "/work",
                "/usr/bin/tool",
            ),
            (
                HostRules::POSIX,
                "/profiles/primary",
                r"C:\work",
                r"C:\tools\tool.exe",
            ),
        ] {
            let error = |text: String| {
                ToolLauncher::from_toml_str(
                    &format!("{text}[accounts.codex.primary]\nprofile_dir = '{profile}'"),
                    host,
                    Vec::<(String, String)>::new(),
                    claude::DiskProbe,
                    codex::DiskProbe,
                )
                .err()
                .unwrap()
            };
            assert_eq!(
                error(format!("[workdirs]\nscratch = '{folder}'\n")),
                "invalid user configuration: folder at entry 0 is written for another operating system"
            );
            assert_eq!(
                error(format!("[tools.codex]\nprogram = '{program}'\n")),
                "invalid codex configuration: program path is written for another operating system"
            );
        }
    }

    #[test]
    fn only_the_filtered_environment_is_kept() {
        let launcher = ToolLauncher::from_toml_str(
            "",
            HostRules::host(),
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
