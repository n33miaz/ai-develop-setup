//! The shared launch suite with a tool that exists only in these tests, so that
//! `cargo test -p ads-core` covers the module without going through an adapter.

use super::Harness;

use super::{ConfigError, DiskProbe, HostRules, LaunchError, Probe, open_params};
use crate::child_env::{EnvList, EnvPolicyError, NameCase};
use crate::gate::{PathError, PathStyle};

/// A tool with its own table, appended to the shipped defaults. Written by hand: only this
/// test tool replaces the defaults, so the public macro does not offer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestTool;

impl Harness for TestTool {
    const ID: &'static str = "harness-test";
    const TABLE_KEY: &'static str = "tools.harness-test";
    const PROGRAM_KEY: &'static str = "tools.harness-test.program";
    const ARGS_KEY: &'static str = "tools.harness-test.args";
    const MODE_KEY: &'static str = "tools.harness-test.mode";
    const DEFAULTS: &'static str = concat!(
        include_str!("../../../../defaults/harnesses.toml"),
        "\n[tools.harness-test]\nprogram = ''\nargs = []\nmode = 'interactive'\n"
    );
}

type ToolConfig = super::ToolConfig<TestTool>;

const HARNESS_ID: &str = TestTool::ID;
const OTHER_HARNESS: &str = "harness-other";
const PROGRAM: &str = r"C:\Tools\harness-test\tool.exe";
const POISON_EXTRA: &[&str] = &[];
const DENIED_EXTRA: &[&str] = &[];
const WRONG_HARNESSES: &[&str] = &["harness-other", "Harness-Test", "harness-a"];

mod real_process;
mod suite;

/// `path` as the configuration keeps it: canonical, so that a short name or a lowercase drive
/// letter on the machine running the tests does not change the text, and without `\\?\`.
fn literal(path: &std::path::Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap().display().to_string();
    let plain = canonical.strip_prefix(r"\\?\").unwrap_or(&canonical);
    crate::gate::AbsPath::parse(plain).unwrap().to_string()
}

crate::tool_harness!(
    /// A tool declared like an adapter declares one, with no table in the shipped defaults.
    Absent = "harness-absent"
);

#[test]
fn every_diagnostic_key_is_derived_from_the_tool_id() {
    for (id, keys) in [
        (
            TestTool::ID,
            [
                TestTool::TABLE_KEY,
                TestTool::PROGRAM_KEY,
                TestTool::ARGS_KEY,
                TestTool::MODE_KEY,
            ],
        ),
        (
            Absent::ID,
            [
                Absent::TABLE_KEY,
                Absent::PROGRAM_KEY,
                Absent::ARGS_KEY,
                Absent::MODE_KEY,
            ],
        ),
    ] {
        assert_eq!(
            keys.map(String::from),
            [
                format!("tools.{id}"),
                format!("tools.{id}.program"),
                format!("tools.{id}.args"),
                format!("tools.{id}.mode"),
            ]
        );
    }
}

#[test]
fn a_tool_without_a_shipped_table_is_refused() {
    assert_eq!(
        super::ToolConfig::<Absent>::from_toml_str("", HostRules::WINDOWS),
        Err(ConfigError::MissingKey("tools.harness-absent"))
    );
    // The user's file cannot add a tool: the table has to come from the shipped defaults.
    let complete = r"
[workdirs]
scratch = 'C:\work\scratch'

[tools.harness-absent]
program = 'C:\Tools\absent\tool.exe'
args = []
mode = 'interactive'
";
    assert_eq!(
        super::ToolConfig::<Absent>::from_toml_str(complete, HostRules::WINDOWS),
        Err(ConfigError::MissingKey("tools.harness-absent"))
    );
    assert!(!super::config::DEFAULTS_TOML.contains("[tools.harness-test]"));
    assert!(TestTool::DEFAULTS.contains("[tools.harness-test]"));
}
