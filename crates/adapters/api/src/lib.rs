//! Adapter for direct provider API calls in the Chat Completions format.
//!
//! This slice is pure logic: no network and no key. A call names its key only by a [`KeyHandle`],
//! built from the active key alias and resolved inside a future [`Transport`].

mod call;
mod chat;
mod config;
mod keys;
mod spend;
mod transport;
mod usage;

pub use call::{CallError, CallOutcome, run_call};
pub use chat::{
    ChatRequest, ChatResponse, KeyAuth, KeyHandle, MAX_RESPONSE_BYTES, MapError, Message,
    PreparedRequest, Role, TokenUsage, build_request, parse_request_body, parse_response,
};
pub use config::{ApiConfig, ConfigError, DEFAULT_POLICY, ProviderConfig};
pub use keys::{KeyAliases, MAX_KEY_ALIAS_CHARS, MAX_KEY_ALIASES};
pub use spend::{ForeignReservation, Month, Refusal, Reservation, SpendGuard};
pub use transport::{Transport, TransportError, TransportResponse};
pub use usage::usage_record;

/// Package name as declared in the manifest.
pub const fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}
