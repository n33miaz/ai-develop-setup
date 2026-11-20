use ads_core::tool_launch::ToolConfig;
pub use ads_core::tool_launch::{ConfigError, Mode};

ads_core::tool_harness!(
    /// Identifies the Claude configuration table.
    Claude = "claude"
);

pub type ClaudeConfig = ToolConfig<Claude>;
