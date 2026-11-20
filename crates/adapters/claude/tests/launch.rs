use ads_adapter_claude::{
    ClaudeConfig as ToolConfig, ConfigError, DiskProbe, HARNESS_ID, LaunchError, Probe, open_params,
};
use ads_core::child_env::{EnvList, EnvPolicyError, NameCase};
use ads_core::gate::PathError;

const OTHER_HARNESS: &str = "codex";
const PROGRAM: &str = r"C:\Tools\claude\claude.exe";
const POISON_EXTRA: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "anthropic_api_key",
    "OPENAI_API_KEY",
    "Openai_Api_Key",
    "CLAUDE_CONFIG_DIR",
    "claude_config_dir",
    "CLAUDE_CODE_USE_BEDROCK",
    "Claude_Code_Entrypoint",
    "ANTHROPIC_BASE_URL",
    "Anthropic_Base_Url",
];
const DENIED_EXTRA: &[&str] = &["Claude_Config_Dir"];
const WRONG_HARNESSES: &[&str] = &["codex", "Claude", "harness-a"];

#[path = "../../../core/src/tool_launch/tests/suite.rs"]
mod tool_launch_tests;
