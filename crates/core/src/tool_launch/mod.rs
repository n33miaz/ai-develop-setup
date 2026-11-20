//! Shared configuration and preparation of terminal sessions for command-line tools.

mod config;
mod launch;

pub use config::{ConfigError, Mode, ToolConfig};
pub use launch::{DiskProbe, LaunchError, Probe, open_params, process_env};

/// A tool the product can launch: its identifier and its static configuration keys for
/// diagnostics, all taken from the identifier.
///
/// Sealed: only the tools listed in this module exist, each with a `[tools.<id>]` table in the
/// shipped defaults. Another crate cannot add one:
///
/// ```compile_fail,E0277
/// struct Mine;
///
/// impl ads_core::tool_launch::Harness for Mine {
///     const ID: &'static str = "mine";
///     const TABLE_KEY: &'static str = "tools.mine";
///     const PROGRAM_KEY: &'static str = "tools.mine.program";
///     const ARGS_KEY: &'static str = "tools.mine.args";
///     const MODE_KEY: &'static str = "tools.mine.mode";
/// }
/// ```
pub trait Harness: sealed::Sealed {
    const ID: &'static str;
    const TABLE_KEY: &'static str;
    const PROGRAM_KEY: &'static str;
    const ARGS_KEY: &'static str;
    const MODE_KEY: &'static str;
}

mod sealed {
    pub trait Sealed {
        /// The shipped parameters the user's file goes over.
        const DEFAULTS: &'static str = super::config::DEFAULTS_TOML;
    }
}

/// Declares a tool with every key derived from its identifier.
macro_rules! harness {
    ($(#[$attr:meta])* $name:ident = $id:literal $(, defaults = $defaults:expr)?) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl sealed::Sealed for $name {
            $(const DEFAULTS: &'static str = $defaults;)?
        }

        impl Harness for $name {
            const ID: &'static str = $id;
            const TABLE_KEY: &'static str = concat!("tools.", $id);
            const PROGRAM_KEY: &'static str = concat!("tools.", $id, ".program");
            const ARGS_KEY: &'static str = concat!("tools.", $id, ".args");
            const MODE_KEY: &'static str = concat!("tools.", $id, ".mode");
        }
    };
}

harness!(
    /// The Claude command-line tool, `[tools.claude]`.
    Claude = "claude"
);
harness!(
    /// The Codex command-line tool, `[tools.codex]`.
    Codex = "codex"
);

#[cfg(test)]
mod tests;
