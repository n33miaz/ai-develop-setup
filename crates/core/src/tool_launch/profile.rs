use std::collections::BTreeMap;
use std::path::Path;

use ads_contracts::{CommonIdV1, ContractPtyHostRequestV1SessionOpenParamsCwd as Folder};
use serde::Deserialize;

use crate::child_env::EnvPolicy;
use crate::gate::AbsPath;

use super::ConfigError;

/// Account selection policy for a future session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwitchMode {
    Manual,
    Propose,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchPolicy {
    pub mode: SwitchMode,
    pub switch_at_pct: Option<u8>,
}

impl SwitchPolicy {
    pub const MANUAL: Self = Self {
        mode: SwitchMode::Manual,
        switch_at_pct: None,
    };

    pub(super) fn from_config(
        profile_env: Option<&'static str>,
        mode: Option<SwitchMode>,
        threshold: Option<u8>,
    ) -> Result<Self, ConfigError> {
        if profile_env.is_none() {
            return if mode.is_none() && threshold.is_none() {
                Ok(Self::MANUAL)
            } else {
                Err(ConfigError::SwitchPolicy)
            };
        }

        let mode = mode.ok_or(ConfigError::SwitchPolicy)?;
        let threshold = threshold.filter(|pct| (1..=100).contains(pct));
        Ok(Self {
            mode,
            switch_at_pct: Some(threshold.ok_or(ConfigError::SwitchPolicy)?),
        })
    }
}

/// A configured account folder and the adapter's reserved environment variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBinding {
    alias: CommonIdV1,
    directory: Folder,
    variable: &'static str,
}

impl ProfileBinding {
    pub(super) fn from_config(
        alias: String,
        directory: &str,
        variable: &'static str,
        index: usize,
    ) -> Result<Self, ConfigError> {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .and_then(Path::to_str)
            .and_then(|root| AbsPath::parse(root).ok());
        Self::validate(alias, directory, variable, index, source.as_ref())
    }

    fn validate(
        alias: String,
        directory: &str,
        variable: &'static str,
        index: usize,
        repository: Option<&AbsPath>,
    ) -> Result<Self, ConfigError> {
        let alias = CommonIdV1::new(alias).map_err(|_| ConfigError::ProfileAlias(index))?;
        let path =
            AbsPath::parse(directory).map_err(|error| ConfigError::ProfileDir { index, error })?;
        if path.is_root() {
            return Err(ConfigError::ProfileDirIsRoot(index));
        }
        if repository.is_some_and(|root| path.is_loosely_within(root)) {
            return Err(ConfigError::ProfileDirInRepository(index));
        }
        let directory =
            Folder::new(path.to_string()).map_err(|_| ConfigError::ProfileDirTooLong(index))?;
        Ok(Self {
            alias,
            directory,
            variable,
        })
    }

    pub fn alias(&self) -> &CommonIdV1 {
        &self.alias
    }

    pub fn directory(&self) -> &Folder {
        &self.directory
    }

    /// Filters the parent environment before applying the chosen account folder.
    pub fn child_env<I, K, V>(&self, policy: &EnvPolicy, parent: I) -> BTreeMap<String, String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut child = policy.child_env(parent);
        child.retain(|name, _| !name.eq_ignore_ascii_case(self.variable));
        child.insert(self.variable.to_owned(), self.directory.to_string());
        child
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::child_env::NameCase;
    use crate::tool_launch::ToolConfig;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    crate::tool_harness!(ProfileTool = "claude", profile_env = "CLAUDE_CONFIG_DIR");
    type Config = ToolConfig<ProfileTool>;

    #[test]
    fn configured_profiles_are_validated_and_policy_is_layered() {
        assert_eq!(
            Config::defaults(NameCase::host()).switch_policy(),
            SwitchPolicy {
                mode: SwitchMode::Propose,
                switch_at_pct: Some(90)
            },
        );
        let config = Config::from_toml_str(
            "[accounts.claude.secondary]\nprofile_dir = '/profiles/secondary'\n\
             [tools.claude]\nswitch = 'auto'\nswitch_at_pct = 75",
            NameCase::host(),
        )
        .unwrap();
        assert_eq!(
            config.switch_policy(),
            SwitchPolicy {
                mode: SwitchMode::Auto,
                switch_at_pct: Some(75)
            }
        );
        let profile = config
            .profile(&CommonIdV1::new("secondary").unwrap())
            .unwrap();
        assert_eq!(profile.directory().as_str(), "/profiles/secondary");

        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let nested = repo.join("nested-profile");
        for (alias, path, error) in [
            (
                "'profile@example.invalid'",
                "/profiles/secondary",
                ConfigError::ProfileAlias(0),
            ),
            ("secondary", "/", ConfigError::ProfileDirIsRoot(0)),
            (
                "secondary",
                nested.to_str().unwrap(),
                ConfigError::ProfileDirInRepository(0),
            ),
        ] {
            let overrides = format!("[accounts.claude.{alias}]\nprofile_dir = '{path}'");
            assert_eq!(
                Config::from_toml_str(&overrides, NameCase::host()),
                Err(error)
            );
        }
        for path in ["relative", "/profiles/../escape", "/profiles/$USER"] {
            let overrides = format!("[accounts.claude.secondary]\nprofile_dir = '{path}'");
            assert!(matches!(
                Config::from_toml_str(&overrides, NameCase::host()),
                Err(ConfigError::ProfileDir { .. })
            ));
        }
        for threshold in [0, 101] {
            let overrides = format!("[tools.claude]\nswitch_at_pct = {threshold}");
            assert_eq!(
                Config::from_toml_str(&overrides, NameCase::host()),
                Err(ConfigError::SwitchPolicy)
            );
        }
    }

    #[test]
    fn a_tool_without_profile_support_has_no_switch_override() {
        assert_eq!(
            SwitchPolicy::from_config(None, None, None),
            Ok(SwitchPolicy::MANUAL)
        );
        for mode in [SwitchMode::Manual, SwitchMode::Propose, SwitchMode::Auto] {
            assert_eq!(
                SwitchPolicy::from_config(None, Some(mode), None),
                Err(ConfigError::SwitchPolicy)
            );
        }
        assert_eq!(
            SwitchPolicy::from_config(None, None, Some(90)),
            Err(ConfigError::SwitchPolicy)
        );
    }

    #[test]
    fn selected_profile_reaches_the_child_without_inherited_credentials() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("target")
            .join(format!("profile-test-{}-{stamp}", std::process::id()));
        let repository = fixture.join("repository");
        let repository = AbsPath::parse(repository.to_str().unwrap()).unwrap();
        let mut bindings = Vec::new();

        for alias in ["primary", "secondary"] {
            let folder = fixture.join(alias);
            fs::create_dir_all(&folder).unwrap();
            let sentinel = folder.join("sentinel");
            fs::write(&sentinel, "profile data").unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(&sentinel)
                .unwrap()
                .set_times(fs::FileTimes::new().set_accessed(UNIX_EPOCH))
                .unwrap();
            let before = fs::metadata(&sentinel).unwrap();
            let binding = ProfileBinding::validate(
                alias.to_owned(),
                folder.to_str().unwrap(),
                "CLAUDE_CONFIG_DIR",
                0,
                Some(&repository),
            )
            .unwrap();
            bindings.push((binding, sentinel, before));
        }

        let config = Config::defaults(NameCase::host());
        let parent = [
            ("ANTHROPIC_API_KEY", "poison"),
            ("CLAUDE_CONFIG_DIR", "wrong"),
            ("CODEX_HOME", "wrong"),
        ];
        let child = bindings[1].0.child_env(config.env(), parent);
        assert_eq!(
            child.get("CLAUDE_CONFIG_DIR").map(String::as_str),
            Some(bindings[1].0.directory().as_str())
        );
        assert!(!child.contains_key("CODEX_HOME"));
        assert!(!child.contains_key("ANTHROPIC_API_KEY"));

        let output = if cfg!(windows) {
            let system = std::env::var_os("SystemRoot").unwrap();
            Command::new(Path::new(&system).join("System32/cmd.exe"))
                .args(["/D", "/C", "set"])
                .env_clear()
                .envs(&child)
                .output()
        } else {
            Command::new("/bin/sh")
                .args(["-c", "printf '%s\\n' \"$CLAUDE_CONFIG_DIR\" \"${CODEX_HOME-unset}\" \"${ANTHROPIC_API_KEY-unset}\""])
                .env_clear()
                .envs(&child)
                .output()
        }.unwrap();
        assert!(output.status.success());
        let output = String::from_utf8_lossy(&output.stdout);
        if cfg!(windows) {
            assert!(
                output
                    .lines()
                    .any(|line| line == format!("CLAUDE_CONFIG_DIR={}", bindings[1].0.directory()))
            );
            assert!(!output.contains("CODEX_HOME="));
            assert!(!output.contains("ANTHROPIC_API_KEY="));
        } else {
            assert_eq!(
                output.lines().collect::<Vec<_>>(),
                [bindings[1].0.directory().as_str(), "unset", "unset"]
            );
        }
        assert!(!output.contains(bindings[0].0.directory().as_str()));

        for (_, sentinel, before) in bindings {
            let after = fs::metadata(&sentinel).unwrap();
            assert_eq!(after.modified().unwrap(), before.modified().unwrap());
            assert_eq!(after.accessed().ok(), before.accessed().ok());
            assert_eq!(fs::read_to_string(sentinel).unwrap(), "profile data");
        }
    }
}
