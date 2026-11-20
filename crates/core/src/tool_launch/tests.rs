//! The shared launch suite with a tool that exists only in these tests, so that
//! `cargo test -p ads-core` covers the module without going through an adapter.

use super::{Claude, Codex, Harness, sealed};

use super::{ConfigError, DiskProbe, LaunchError, Probe, open_params};
use crate::child_env::{EnvList, EnvPolicyError, NameCase};
use crate::gate::PathError;

harness!(
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

fn assert_keys_follow_the_id<H: Harness>() {
    assert_eq!(H::TABLE_KEY, format!("tools.{}", H::ID));
    assert_eq!(H::PROGRAM_KEY, format!("tools.{}.program", H::ID));
    assert_eq!(H::ARGS_KEY, format!("tools.{}.args", H::ID));
    assert_eq!(H::MODE_KEY, format!("tools.{}.mode", H::ID));
}

#[test]
fn every_diagnostic_key_is_derived_from_the_tool_id() {
    assert_keys_follow_the_id::<Claude>();
    assert_keys_follow_the_id::<Codex>();
    assert_keys_follow_the_id::<TestTool>();
    assert_eq!((Claude::ID, Codex::ID), ("claude", "codex"));
}

#[test]
fn every_shipped_tool_has_a_table_in_the_defaults() {
    for config in [
        super::ToolConfig::<Claude>::from_toml_str("", NameCase::Insensitive).map(|_| ()),
        super::ToolConfig::<Codex>::from_toml_str("", NameCase::Insensitive).map(|_| ()),
    ] {
        assert_eq!(config, Ok(()));
    }
    // The test tool's table exists only in its own defaults.
    assert!(!<Claude as sealed::Sealed>::DEFAULTS.contains("[tools.harness-test]"));
    assert_eq!(
        super::ToolConfig::<TestTool>::from_toml_str("", NameCase::Insensitive).map(|_| ()),
        Ok(())
    );
}
