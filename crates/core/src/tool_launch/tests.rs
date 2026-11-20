//! The shared launch suite with a tool that exists only in these tests, so that
//! `cargo test -p ads-core` covers the module without going through an adapter.

use super::Harness;

use super::{ConfigError, DiskProbe, LaunchError, Probe, open_params};
use crate::child_env::{EnvList, EnvPolicyError, NameCase};
use crate::gate::PathError;

crate::tool_harness!(
    /// A tool with its own table, appended to the shipped defaults.
    TestTool = "harness-test",
    defaults = concat!(
        include_str!("../../../../defaults/harnesses.toml"),
        "\n[tools.harness-test]\nprogram = ''\nargs = []\nmode = 'interactive'\n"
    )
);

type ToolConfig = super::ToolConfig<TestTool>;

const HARNESS_ID: &str = TestTool::ID;
const OTHER_HARNESS: &str = "claude";
const PROGRAM: &str = r"C:\Tools\harness-test\tool.exe";
const POISON_EXTRA: &[&str] = &[];
const DENIED_EXTRA: &[&str] = &[];
const WRONG_HARNESSES: &[&str] = &["claude", "codex", "Harness-Test", "harness-a"];

mod suite;

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
        super::ToolConfig::<Absent>::from_toml_str("", NameCase::Insensitive),
        Err(ConfigError::MissingKey("tools.harness-absent"))
    );
    assert!(!super::config::DEFAULTS_TOML.contains("[tools.harness-test]"));
    assert!(TestTool::DEFAULTS.contains("[tools.harness-test]"));
}
