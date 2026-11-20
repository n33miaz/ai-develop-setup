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

use super::Harness;

const DEFAULTS_TOML: &str = include_str!("../../../../defaults/harnesses.toml");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLayer {
    env: Option<EnvLayer>,
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
}

impl<H: Harness> ToolConfig<H> {
    /// The shipped defaults: the tool is off and no folder is known.
    pub fn defaults(case: NameCase) -> Self {
        Self::from_toml_str("", case).expect("the shipped defaults are complete")
    }

    /// The shipped defaults with the user's file `overrides` over them.
    pub fn from_toml_str(overrides: &str, case: NameCase) -> Result<Self, ConfigError> {
        let base = parse(DEFAULTS_TOML)?;
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
            workdirs.insert(String::from(id), cwd);
        }

        Ok(Self {
            harness: std::marker::PhantomData,
            program,
            args,
            mode,
            workdirs,
            env,
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
