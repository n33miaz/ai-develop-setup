//! Message and event types generated from the schemas, valid by construction.
//!
//! `generated/` comes from `contracts/schemas` (`npm run gen:rust` in `contracts/`) and is never
//! edited by hand; `support` provides the validators the generated types lean on.

#[cfg(feature = "desktop-proof")]
pub mod desktop_proof;
pub mod generated;
pub mod support;

pub use generated::*;
pub use support::ContractError;
