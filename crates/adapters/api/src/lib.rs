//! Adapter for direct provider API calls in the Chat Completions format.
//!
//! This slice is pure logic: no network and no key. The key arrives as a [`KeyHandle`] from the
//! caller, which resolves it only inside a future transport.

mod chat;
mod config;
mod spend;
mod usage;

pub use chat::{
    ChatRequest, ChatResponse, KeyAuth, KeyHandle, MAX_RESPONSE_BYTES, MapError, Message,
    PreparedRequest, Role, TokenUsage, build_request, parse_request_body, parse_response,
};
pub use config::{ApiConfig, ConfigError, DEFAULT_POLICY, ProviderConfig};
pub use spend::{Month, Refusal, Reservation, SpendGuard};
pub use usage::usage_record;

/// Package name as declared in the manifest.
pub const fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}
