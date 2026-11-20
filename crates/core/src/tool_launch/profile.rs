use std::collections::BTreeMap;

use ads_contracts::{CommonIdV1, ContractPtyHostRequestV1SessionOpenParamsCwd as Folder};
use serde::Deserialize;

use crate::child_env::EnvPolicy;
use crate::gate::{AbsPath, PathStyle};

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
        style: PathStyle,
        index: usize,
    ) -> Result<Self, ConfigError> {
        let alias = CommonIdV1::new(alias).map_err(|_| ConfigError::ProfileAlias(index))?;
        let path =
            AbsPath::parse(directory).map_err(|error| ConfigError::ProfileDir { index, error })?;
        if path.style() != style {
            return Err(ConfigError::ProfileDirForeignStyle(index));
        }
        if path.is_root() {
            return Err(ConfigError::ProfileDirIsRoot(index));
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

    pub(super) fn path(&self) -> AbsPath {
        AbsPath::parse(self.directory.as_str())
            .expect("profile paths are normalized at construction")
    }

    pub(super) fn overlaps_path(&self, other: &AbsPath) -> bool {
        let path = self.path();
        path.is_loosely_within(other) || other.is_loosely_within(&path)
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
    use crate::tool_launch::{HostRules, ToolConfig};

    #[test]
    fn profiles_reject_foreign_absolute_paths() {
        let path = if cfg!(windows) {
            "/profiles/x"
        } else {
            "C:/profiles/x"
        };
        let overrides = format!("[accounts.claude.primary]\nprofile_dir = '{path}'");
        assert_eq!(
            Config::from_toml_str(&overrides, HostRules::host()),
            Err(ConfigError::ProfileDirForeignStyle(0))
        );
    }

    #[test]
    fn profiles_reject_overlapping_configured_folders() {
        let drive = if cfg!(windows) { "C:" } else { "" };
        for (workdir, profile) in [("work", "work/profile"), ("work/project", "work")] {
            let overrides = format!(
                "[workdirs]\nw = '{drive}/{workdir}'\n[accounts.claude.a]\nprofile_dir = '{drive}/{profile}'"
            );
            assert_eq!(
                Config::from_toml_str(&overrides, HostRules::host()),
                Err(ConfigError::ProfileDirOverlapsWorkdir(0)),
                "{workdir} / {profile}"
            );
        }
    }

    #[test]
    fn profiles_reject_duplicate_and_nested_folders() {
        let drive = if cfg!(windows) { "C:" } else { "" };
        for second in ["profiles/./MAIN", "profiles/main/nested"] {
            let overrides = format!(
                "[accounts.claude.a]\nprofile_dir = '{drive}/profiles/main'\n[accounts.claude.b]\nprofile_dir = '{drive}/{second}'"
            );
            assert_eq!(
                Config::from_toml_str(&overrides, HostRules::host()),
                Err(ConfigError::ProfileDirsOverlap),
                "{second}"
            );
        }
    }

    crate::tool_harness!(ProfileTool = "claude", profile_env = "CLAUDE_CONFIG_DIR");
    type Config = ToolConfig<ProfileTool>;

    #[test]
    fn configured_profiles_are_validated_and_policy_is_layered() {
        assert_eq!(
            Config::defaults(HostRules::host()).switch_policy(),
            SwitchPolicy {
                mode: SwitchMode::Propose,
                switch_at_pct: Some(90)
            },
        );
        let drive = if cfg!(windows) { "C:" } else { "" };
        let path = format!("{drive}/profiles/secondary");
        let config = Config::from_toml_str(
            &format!(
                "[accounts.claude.secondary]\nprofile_dir = '{path}'\n\
             [tools.claude]\nswitch = 'auto'\nswitch_at_pct = 75"
            ),
            HostRules::host(),
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
        assert_eq!(
            profile.directory().as_str(),
            AbsPath::parse(&path).unwrap().to_string()
        );

        for (alias, path, error) in [
            (
                "'profile@example.invalid'",
                path,
                ConfigError::ProfileAlias(0),
            ),
            (
                "secondary",
                format!("{drive}/"),
                ConfigError::ProfileDirIsRoot(0),
            ),
        ] {
            let overrides = format!("[accounts.claude.{alias}]\nprofile_dir = '{path}'");
            assert_eq!(
                Config::from_toml_str(&overrides, HostRules::host()),
                Err(error)
            );
        }
        for path in ["relative", "/profiles/../escape", "/profiles/$USER"] {
            let overrides = format!("[accounts.claude.secondary]\nprofile_dir = '{path}'");
            assert!(matches!(
                Config::from_toml_str(&overrides, HostRules::host()),
                Err(ConfigError::ProfileDir { .. })
            ));
        }
        for threshold in [0, 101] {
            let overrides = format!("[tools.claude]\nswitch_at_pct = {threshold}");
            assert_eq!(
                Config::from_toml_str(&overrides, HostRules::host()),
                Err(ConfigError::SwitchPolicy)
            );
        }
    }

    #[test]
    fn paths_with_shared_text_prefixes_can_be_distinct_folders() {
        let drive = if cfg!(windows) { "C:" } else { "" };
        let overrides = format!(
            "[workdirs]\nw = '{drive}/work'\n\
             [accounts.claude.a]\nprofile_dir = '{drive}/work-profile'\n\
             [accounts.claude.b]\nprofile_dir = '{drive}/work-profile2'"
        );
        Config::from_toml_str(&overrides, HostRules::host()).unwrap();
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
    fn selected_profile_replaces_profile_environment_and_filters_credentials() {
        let drive = if cfg!(windows) { "C:" } else { "" };
        let config = Config::from_toml_str(
            &format!(
                "[accounts.claude.primary]\nprofile_dir = '{drive}/profiles/primary'\n\
                      [accounts.claude.secondary]\nprofile_dir = '{drive}/profiles/secondary'"
            ),
            HostRules::host(),
        )
        .unwrap();
        let binding = config
            .profile(&CommonIdV1::new("secondary").unwrap())
            .unwrap();
        let child = binding.child_env(
            config.env(),
            [
                ("ANTHROPIC_API_KEY", "poison"),
                ("CLAUDE_CONFIG_DIR", "wrong"),
                ("CODEX_HOME", "wrong"),
            ],
        );
        assert_eq!(
            child,
            BTreeMap::from([
                (
                    "CLAUDE_CONFIG_DIR".to_owned(),
                    binding.directory().to_string()
                ),
                ("TERM".to_owned(), "xterm-256color".to_owned()),
                ("COLORTERM".to_owned(), "truecolor".to_owned()),
            ])
        );
    }
}
