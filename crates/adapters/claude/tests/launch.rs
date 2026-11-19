use ads_adapter_claude::{
    ClaudeConfig as ToolConfig, ConfigError, DiskProbe, HARNESS_ID, LaunchError, Probe, open_params,
};

const OTHER_HARNESS: &str = "codex";
const PROGRAM: &str = r"C:\Tools\claude\claude.exe";
const POISON_EXTRA: &[&str] = &[];
const DENIED_EXTRA: &[&str] = &["Claude_Config_Dir"];
const WRONG_HARNESSES: &[&str] = &["codex", "Claude", "harness-a"];

#[path = "../../tool_launch_tests.rs"]
mod tool_launch_tests;
