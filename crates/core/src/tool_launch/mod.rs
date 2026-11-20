//! Shared configuration and preparation of terminal sessions for command-line tools.

mod config;
mod launch;

pub use config::{ConfigError, Mode, ToolConfig};
pub use launch::{DiskProbe, LaunchError, Probe, open_params, process_env};

/// A tool an adapter launches: its identifier and its static configuration keys for
/// diagnostics. Declare one with [`tool_harness!`](crate::tool_harness), which takes every key
/// from the identifier.
///
/// Not sealed, since sealing would list the tools in the core. A tool without a `[tools.<id>]`
/// table in the shipped defaults is refused with [`ConfigError::MissingKey`], and
/// [`open_params`] refuses a request for another tool.
pub trait Harness {
    const ID: &'static str;
    const TABLE_KEY: &'static str;
    const PROGRAM_KEY: &'static str;
    const ARGS_KEY: &'static str;
    const MODE_KEY: &'static str;
    /// The shipped parameters, which the core's test tool extends with its own table.
    #[cfg(test)]
    const DEFAULTS: &'static str = config::DEFAULTS_TOML;
}

/// Declares a tool marker type with every [`Harness`] key derived from the identifier.
///
/// ```
/// ads_core::tool_harness!(
///     /// Identifies the example tool.
///     Example = "example"
/// );
/// use ads_core::tool_launch::Harness;
/// assert_eq!(Example::PROGRAM_KEY, "tools.example.program");
/// ```
#[macro_export]
macro_rules! tool_harness {
    ($(#[$attr:meta])* $name:ident = $id:literal $(, defaults = $defaults:expr)?) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl $crate::tool_launch::Harness for $name {
            const ID: &'static str = $id;
            const TABLE_KEY: &'static str = concat!("tools.", $id);
            const PROGRAM_KEY: &'static str = concat!("tools.", $id, ".program");
            const ARGS_KEY: &'static str = concat!("tools.", $id, ".args");
            const MODE_KEY: &'static str = concat!("tools.", $id, ".mode");
            $(const DEFAULTS: &'static str = $defaults;)?
        }
    };
}

#[cfg(test)]
mod tests;
