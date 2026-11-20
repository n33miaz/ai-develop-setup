//! Tool launch parameters: the shipped `defaults/harnesses.toml` with the user's file over
//! it. Only the user's file holds the real program path and folders.

use std::collections::BTreeMap;
use std::fmt;

use crate::child_env::{EnvLayer, EnvPolicy, EnvPolicyError, NameCase};
use crate::gate::{AbsPath, PathError};
use ads_contracts::{
    CommonIdV1, ContractPtyHostRequestV1SessionOpenParamsArgsItem as ArgItem,
    ContractPtyHostRequestV1SessionOpenParamsCwd as Cwd,
    ContractPtyHostRequestV1SessionOpenParamsProgram as Program,
};
use serde::Deserialize;

use super::{Harness, ProfileBinding, SwitchPolicy};

pub(super) const DEFAULTS_TOML: &str = include_str!("../../../../defaults/harnesses.toml");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLayer {
    env: Option<EnvLayer>,
    accounts: Option<BTreeMap<String, BTreeMap<String, ProfileLayer>>>,
    workdirs: Option<BTreeMap<String, String>>,
    /// Other tools' tables belong to other adapters and are not read here.
    tools: Option<BTreeMap<String, toml::Table>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolLayer {
    program: Option<String>,
    args: Option<Vec<String>>,
    mode: Option<Mode>,
    switch: Option<super::SwitchMode>,
    switch_at_pct: Option<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileLayer {
    profile_dir: String,
}

/// How the tool is used. Headless use is not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Interactive,
}

/// Why the parameters were refused. Carries a key, a position or a reason code, never the
/// rejected text nor the raw parser message, which could quote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Not TOML, or a key or type the file does not allow. `line` is 1-based when known.
    Toml {
        line: Option<usize>,
    },
    MissingKey(&'static str),
    Env(EnvPolicyError),
    /// The program path is not absolute and literal.
    Program(PathError),
    ProgramTooLong,
    /// Too long, or with NUL or another control character.
    Argument(usize),
    /// Entries of `[workdirs]` count the shipped ones first, each layer in name order.
    Alias(usize),
    Workdir {
        index: usize,
        error: PathError,
    },
    WorkdirTooLong(usize),
    /// A drive root or `/`: the tool would work on the whole disk.
    WorkdirIsRoot(usize),
    ProfileAlias(usize),
    ProfileDir {
        index: usize,
        error: PathError,
    },
    ProfileDirTooLong(usize),
    ProfileDirIsRoot(usize),
    ProfileDirForeignStyle(usize),
    ProfileDirOverlapsWorkdir(usize),
    ProfileDirsOverlap,
    ProfileUnsupported,
    SwitchPolicy,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml { line: Some(line) } => {
                write!(f, "invalid harness parameters at line {line}")
            }
            Self::Toml { line: None } => f.write_str("invalid harness parameters"),
            Self::MissingKey(key) => write!(f, "missing key `{key}`"),
            Self::Env(error) => error.fmt(f),
            Self::Program(error) => write!(f, "program path refused: {error}"),
            Self::ProgramTooLong => f.write_str("program path too long"),
            Self::Argument(index) => write!(f, "invalid argument {index}"),
            Self::Alias(index) => write!(f, "invalid folder alias at entry {index}"),
            Self::Workdir { index, error } => {
                write!(f, "folder at entry {index} refused: {error}")
            }
            Self::WorkdirTooLong(index) => write!(f, "folder at entry {index} too long"),
            Self::WorkdirIsRoot(index) => write!(f, "folder at entry {index} is a drive root"),
            Self::ProfileAlias(index) => write!(f, "invalid profile alias at entry {index}"),
            Self::ProfileDir { index, error } => {
                write!(f, "profile folder at entry {index} refused: {error}")
            }
            Self::ProfileDirTooLong(index) => write!(f, "profile folder at entry {index} too long"),
            Self::ProfileDirIsRoot(index) => {
                write!(f, "profile folder at entry {index} is a drive root")
            }
            Self::ProfileDirForeignStyle(index) => {
                write!(
                    f,
                    "profile folder at entry {index} is not absolute on this system"
                )
            }
            Self::ProfileDirOverlapsWorkdir(index) => {
                write!(
                    f,
                    "profile folder at entry {index} overlaps a working folder"
                )
            }
            Self::ProfileDirsOverlap => f.write_str("configured account profile folders overlap"),
            Self::ProfileUnsupported => {
                f.write_str("account profiles are unavailable for this tool")
            }
            Self::SwitchPolicy => f.write_str("invalid account switch policy"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// What the adapter may launch. Only [`ToolConfig::from_toml_str`] builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolConfig<H: Harness> {
    harness: std::marker::PhantomData<H>,
    /// `None`: the tool is off.
    program: Option<Program>,
    args: Vec<ArgItem>,
    mode: Mode,
    workdirs: BTreeMap<String, Cwd>,
    env: EnvPolicy,
    profiles: BTreeMap<String, ProfileBinding>,
    switch_policy: SwitchPolicy,
}

impl<H: Harness> ToolConfig<H> {
    /// The shipped defaults: the tool is off and no folder is known.
    pub fn defaults(case: NameCase) -> Self {
        Self::from_toml_str("", case).expect("the shipped defaults are complete")
    }

    /// The shipped defaults with the user's file `overrides` over them.
    pub fn from_toml_str(overrides: &str, case: NameCase) -> Result<Self, ConfigError> {
        #[cfg(test)]
        let defaults = H::DEFAULTS;
        #[cfg(not(test))]
        let defaults = DEFAULTS_TOML;
        let base = parse(defaults)?;
        let over = parse(overrides)?;
        let env = EnvPolicy::from_layers(
            base.env.as_ref().ok_or(ConfigError::MissingKey("env"))?,
            over.env.as_ref(),
            case,
        )
        .map_err(ConfigError::Env)?;

        let base_tool = tool_layer::<H>(&base)?.ok_or(ConfigError::MissingKey(H::TABLE_KEY))?;
        let over_tool = tool_layer::<H>(&over)?;
        let over_tool = over_tool.as_ref();
        let program = over_tool
            .and_then(|tool| tool.program.clone())
            .or(base_tool.program)
            .ok_or(ConfigError::MissingKey(H::PROGRAM_KEY))?;
        let args = over_tool
            .and_then(|tool| tool.args.clone())
            .or(base_tool.args)
            .ok_or(ConfigError::MissingKey(H::ARGS_KEY))?;
        let mode = over_tool
            .and_then(|tool| tool.mode)
            .or(base_tool.mode)
            .ok_or(ConfigError::MissingKey(H::MODE_KEY))?;

        let program = if program.is_empty() {
            None
        } else {
            let path = AbsPath::parse(&program).map_err(ConfigError::Program)?;
            Some(Program::new(path.to_string()).map_err(|_| ConfigError::ProgramTooLong)?)
        };
        let args = args
            .into_iter()
            .enumerate()
            .map(|(index, arg)| {
                if arg.chars().any(char::is_control) {
                    return Err(ConfigError::Argument(index));
                }
                ArgItem::new(arg).map_err(|_| ConfigError::Argument(index))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut workdirs = BTreeMap::new();
        for (index, (alias, path)) in base
            .workdirs
            .into_iter()
            .flatten()
            .chain(over.workdirs.into_iter().flatten())
            .enumerate()
        {
            let id = CommonIdV1::new(alias).map_err(|_| ConfigError::Alias(index))?;
            let path =
                AbsPath::parse(&path).map_err(|error| ConfigError::Workdir { index, error })?;
            if path.is_root() {
                return Err(ConfigError::WorkdirIsRoot(index));
            }
            let cwd = Cwd::new(path.to_string()).map_err(|_| ConfigError::WorkdirTooLong(index))?;
            workdirs.insert(String::from(id), (path, cwd));
        }

        let switch = over_tool.and_then(|tool| tool.switch).or(base_tool.switch);
        let switch_at_pct = over_tool
            .and_then(|tool| tool.switch_at_pct)
            .or(base_tool.switch_at_pct);
        let switch_policy = SwitchPolicy::from_config(H::PROFILE_ENV, switch, switch_at_pct)?;

        let mut profiles = BTreeMap::new();
        for (index, (alias, profile)) in [base.accounts, over.accounts]
            .into_iter()
            .flatten()
            .filter_map(|mut accounts| accounts.remove(H::ID))
            .flatten()
            .enumerate()
        {
            let variable = H::PROFILE_ENV.ok_or(ConfigError::ProfileUnsupported)?;
            let binding =
                ProfileBinding::from_config(alias, &profile.profile_dir, variable, index)?;
            profiles.insert(binding.alias().as_str().to_owned(), (index, binding));
        }
        let mut validated_profiles = BTreeMap::<String, ProfileBinding>::new();
        for (alias, (index, binding)) in profiles {
            if workdirs
                .values()
                .any(|(path, _)| binding.overlaps_path(path))
            {
                return Err(ConfigError::ProfileDirOverlapsWorkdir(index));
            }
            let path = binding.path();
            if validated_profiles
                .values()
                .any(|other| other.overlaps_path(&path))
            {
                return Err(ConfigError::ProfileDirsOverlap);
            }
            validated_profiles.insert(alias, binding);
        }

        Ok(Self {
            harness: std::marker::PhantomData,
            program,
            args,
            mode,
            workdirs: workdirs
                .into_iter()
                .map(|(alias, (_, cwd))| (alias, cwd))
                .collect(),
            env,
            profiles: validated_profiles,
            switch_policy,
        })
    }

    pub(super) fn harness_id(&self) -> &'static str {
        H::ID
    }

    /// The program to run, or `None` when the tool is off.
    pub fn program(&self) -> Option<&Program> {
        self.program.as_ref()
    }

    pub fn args(&self) -> &[ArgItem] {
        &self.args
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The folder configured for `alias`, if any.
    pub fn workdir(&self, alias: &CommonIdV1) -> Option<&Cwd> {
        self.workdirs.get(alias.as_str())
    }

    /// The configured folder aliases in name order; never the folders themselves.
    pub fn workdir_aliases(&self) -> impl Iterator<Item = &str> {
        self.workdirs.keys().map(String::as_str)
    }

    pub fn profile(&self, alias: &CommonIdV1) -> Option<&ProfileBinding> {
        self.profiles.get(alias.as_str())
    }

    /// Checks both directions without consulting the filesystem.
    pub fn has_overlapping_profiles(&self, other: &ToolConfig<impl Harness>) -> bool {
        self.profiles.values().any(|profile| {
            let path = profile.path();
            other
                .profiles
                .values()
                .any(|other| other.overlaps_path(&path))
        })
    }

    pub fn switch_policy(&self) -> SwitchPolicy {
        self.switch_policy
    }

    pub fn env(&self) -> &EnvPolicy {
        &self.env
    }
}

fn parse(text: &str) -> Result<FileLayer, ConfigError> {
    toml::from_str(text).map_err(|error: toml::de::Error| ConfigError::Toml {
        line: error.span().map(|span| {
            text.as_bytes()[..span.start.min(text.len())]
                .iter()
                .filter(|b| **b == b'\n')
                .count()
                + 1
        }),
    })
}

fn tool_layer<H: Harness>(file: &FileLayer) -> Result<Option<ToolLayer>, ConfigError> {
    let Some(table) = file.tools.as_ref().and_then(|tools| tools.get(H::ID)) else {
        return Ok(None);
    };
    table
        .clone()
        .try_into()
        .map(Some)
        .map_err(|_: toml::de::Error| ConfigError::Toml { line: None })
}
