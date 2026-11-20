use ads_core::tool_launch::ToolConfig;
pub use ads_core::tool_launch::{ConfigError, Mode};

ads_core::tool_harness!(
    /// Identifies the Codex configuration table.
    Codex = "codex"
);

pub type CodexConfig = ToolConfig<Codex>;
