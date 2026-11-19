pub use ads_core::tool_launch::{ConfigError, Mode};
use ads_core::tool_launch::{Harness, ToolConfig};

/// Identifies the Codex configuration table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codex;

impl Harness for Codex {
    const ID: &'static str = crate::HARNESS_ID;
    const TABLE_KEY: &'static str = concat!("tools.", "codex");
    const PROGRAM_KEY: &'static str = concat!("tools.", "codex", ".program");
    const ARGS_KEY: &'static str = concat!("tools.", "codex", ".args");
    const MODE_KEY: &'static str = concat!("tools.", "codex", ".mode");
}

pub type CodexConfig = ToolConfig<Codex>;
