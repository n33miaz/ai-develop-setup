//! The Claude tool's parameters: the shipped `defaults/harnesses.toml` with the user's file over
//! it. Only the user's file holds the real program path and folders.

use std::collections::BTreeMap;
use std::fmt;

use ads_contracts::{
    CommonIdV1, ContractPtyHostRequestV1SessionOpenParamsArgsItem as ArgItem,
    ContractPtyHostRequestV1SessionOpenParamsCwd as Cwd,
    ContractPtyHostRequestV1SessionOpenParamsProgram as Program,
};
use ads_core::child_env::{EnvLayer, EnvPolicy, EnvPolicyError, NameCase};
use ads_core::gate::{AbsPath, PathError};
use serde::Deserialize;

use crate::HARNESS_ID;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Toml(String),
    MissingKey(&'static str),
    Env(EnvPolicyError),
    /// The program path is not absolute and literal, or too long.
    Program(PathError),
    ProgramTooLong,
    Argument(usize),
    Alias(String),
    Workdir {
        alias: String,
        error: PathError,
    },
    WorkdirTooLong(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(error) => write!(f, "invalid harness parameters: {error}"),
            Self::MissingKey(key) => write!(f, "missing key `{key}`"),
            Self::Env(error) => error.fmt(f),
            Self::Program(error) => write!(f, "program path refused: {error}"),
            Self::ProgramTooLong => f.write_str("program path too long"),
            Self::Argument(index) => write!(f, "argument {index} too long"),
            Self::Alias(alias) => write!(f, "invalid folder alias `{alias}`"),
            Self::Workdir { alias, error } => write!(f, "folder of `{alias}` refused: {error}"),
            Self::WorkdirTooLong(alias) => write!(f, "folder of `{alias}` too long"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// What the adapter may launch. Only [`ClaudeConfig::from_toml_str`] builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeConfig {
    /// `None`: the tool is off.
    program: Option<Program>,
    args: Vec<ArgItem>,
    mode: Mode,
    workdirs: BTreeMap<String, Cwd>,
    env: EnvPolicy,
}

impl ClaudeConfig {
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

        let base_tool = tool_layer(&base)?.ok_or(ConfigError::MissingKey("tools.claude"))?;
        let over_tool = tool_layer(&over)?;
        let over_tool = over_tool.as_ref();
        let program = over_tool
            .and_then(|tool| tool.program.clone())
            .or(base_tool.program)
            .ok_or(ConfigError::MissingKey("tools.claude.program"))?;
        let args = over_tool
            .and_then(|tool| tool.args.clone())
            .or(base_tool.args)
            .ok_or(ConfigError::MissingKey("tools.claude.args"))?;
        let mode = over_tool
            .and_then(|tool| tool.mode)
            .or(base_tool.mode)
            .ok_or(ConfigError::MissingKey("tools.claude.mode"))?;

        let program = if program.is_empty() {
            None
        } else {
            let path = AbsPath::parse(&program).map_err(ConfigError::Program)?;
            Some(Program::new(path.to_string()).map_err(|_| ConfigError::ProgramTooLong)?)
        };
        let args = args
            .into_iter()
            .enumerate()
            .map(|(index, arg)| ArgItem::new(arg).map_err(|_| ConfigError::Argument(index)))
            .collect::<Result<Vec<_>, _>>()?;

        let mut workdirs = BTreeMap::new();
        for (alias, path) in base
            .workdirs
            .into_iter()
            .flatten()
            .chain(over.workdirs.into_iter().flatten())
        {
            let id =
                CommonIdV1::new(alias.clone()).map_err(|_| ConfigError::Alias(alias.clone()))?;
            let path = AbsPath::parse(&path).map_err(|error| ConfigError::Workdir {
                alias: alias.clone(),
                error,
            })?;
            let cwd = Cwd::new(path.to_string()).map_err(|_| ConfigError::WorkdirTooLong(alias))?;
            workdirs.insert(String::from(id), cwd);
        }

        Ok(Self {
            program,
            args,
            mode,
            workdirs,
            env,
        })
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

    pub fn env(&self) -> &EnvPolicy {
        &self.env
    }
}

fn parse(text: &str) -> Result<FileLayer, ConfigError> {
    toml::from_str(text).map_err(|error| ConfigError::Toml(error.to_string()))
}

fn tool_layer(file: &FileLayer) -> Result<Option<ToolLayer>, ConfigError> {
    let Some(table) = file.tools.as_ref().and_then(|tools| tools.get(HARNESS_ID)) else {
        return Ok(None);
    };
    table
        .clone()
        .try_into()
        .map(Some)
        .map_err(|error: toml::de::Error| ConfigError::Toml(error.to_string()))
}
