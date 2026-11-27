//! Policy: tasks, approvals, scope, routing and budget.

pub mod budget;
pub use ads_child_env as child_env;
mod device;
pub mod gate;
pub mod handoff;
pub mod handoff_document;
pub mod queue;
pub mod refusal;
pub mod routing;
pub mod spend;
mod timestamp;
pub mod tool_launch;
pub mod vault;

/// Package name as declared in the manifest.
pub const fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_manifest() {
        assert_eq!(crate_name(), "ads-core");
    }
}
