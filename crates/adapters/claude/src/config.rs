pub use ads_core::tool_launch::{ConfigError, Mode};
use ads_core::tool_launch::{Harness, ToolConfig};

/// Identifies the Claude configuration table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claude;

impl Harness for Claude {
    const ID: &'static str = crate::HARNESS_ID;
    const TABLE_KEY: &'static str = concat!("tools.", "claude");
    const PROGRAM_KEY: &'static str = concat!("tools.", "claude", ".program");
    const ARGS_KEY: &'static str = concat!("tools.", "claude", ".args");
    const MODE_KEY: &'static str = concat!("tools.", "claude", ".mode");
}

pub type ClaudeConfig = ToolConfig<Claude>;
