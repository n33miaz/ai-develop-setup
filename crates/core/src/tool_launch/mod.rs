//! Shared configuration and preparation of terminal sessions for command-line tools.

mod config;
mod launch;

pub use config::{ConfigError, Mode, ToolConfig};
pub use launch::{DiskProbe, LaunchError, Probe, open_params, process_env};

/// The adapter's identifier and its static configuration keys for diagnostics.
pub trait Harness {
    const ID: &'static str;
    const TABLE_KEY: &'static str;
    const PROGRAM_KEY: &'static str;
    const ARGS_KEY: &'static str;
    const MODE_KEY: &'static str;
}
