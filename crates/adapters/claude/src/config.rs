use ads_core::tool_launch::ToolConfig;
pub use ads_core::tool_launch::{ConfigError, Mode};

ads_core::tool_harness!(
    /// Identifies the Claude configuration table.
    Claude = "claude", profile_env = "CLAUDE_CONFIG_DIR"
);

pub type ClaudeConfig = ToolConfig<Claude>;
