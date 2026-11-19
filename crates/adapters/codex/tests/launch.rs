use ads_adapter_codex::{
    CodexConfig as ToolConfig, ConfigError, DiskProbe, HARNESS_ID, LaunchError, Probe, open_params,
};

const OTHER_HARNESS: &str = "claude";
const PROGRAM: &str = r"C:\Tools\codex\codex.exe";
const POISON_EXTRA: &[&str] = &[
    "CODEX_HOME",
    "codex_home",
    "CODEX_API_KEY",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "OPENAI_BASE_URL",
];
const DENIED_EXTRA: &[&str] = &["Codex_Home", "OPENAI_ORG_ID"];
const WRONG_HARNESSES: &[&str] = &["claude", "Codex", "CODEX", "harness-a"];

#[path = "../../tool_launch_tests.rs"]
mod tool_launch_tests;
