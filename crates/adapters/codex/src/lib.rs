//! Adapter for the Codex command-line tool: builds the terminal host's open parameters from the
//! user's configuration, never from the request.

pub mod config;
pub mod launch;

pub use config::{Codex, CodexConfig, ConfigError, Mode};
pub use launch::{DiskProbe, LaunchError, Probe, open_params, process_env};

/// The tool identifier a session request uses to pick this adapter.
pub const HARNESS_ID: &str = <Codex as ads_core::tool_launch::Harness>::ID;

/// Package name as declared in the manifest.
pub const fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(crate_name(), "ads-adapter-codex");
    }
}
