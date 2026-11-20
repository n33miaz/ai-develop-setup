use ads_adapter_codex::{
    CodexConfig as ToolConfig, ConfigError, DiskProbe, HARNESS_ID, LaunchError, Probe, open_params,
};
use ads_core::child_env::{EnvList, EnvPolicyError, NameCase};
use ads_core::gate::{PathError, PathStyle};
use ads_core::tool_launch::HostRules;

const OTHER_HARNESS: &str = "claude";
const PROGRAM: &str = r"C:\Tools\codex\codex.exe";
const POISON_EXTRA: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "anthropic_api_key",
    "OPENAI_API_KEY",
    "Openai_Api_Key",
    "CODEX_HOME",
    "codex_home",
    "CODEX_API_KEY",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "OPENAI_BASE_URL",
];
const DENIED_EXTRA: &[&str] = &["Codex_Home", "OPENAI_ORG_ID"];
const WRONG_HARNESSES: &[&str] = &["claude", "Codex", "CODEX", "harness-a"];

#[path = "../../../core/src/tool_launch/tests/suite.rs"]
mod tool_launch_tests;
