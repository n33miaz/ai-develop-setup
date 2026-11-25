//! The port a network transport implements. Nothing here opens a connection or reads a key.

use std::fmt;

use crate::chat::PreparedRequest;

/// Sends a prepared request to a provider.
///
/// An implementation must:
///
/// - resolve `request.key.handle` itself, only to send, and keep the key and the reference out of
///   its errors and logs;
/// - not follow redirects: a 3xx answer is returned as it came;
/// - not retry, whatever the failure: one `send` is at most one request;
/// - read no further into the body than it takes to see that it passes
///   [`MAX_RESPONSE_BYTES`](crate::MAX_RESPONSE_BYTES) (one byte more), and then return that
///   oversized body or [`TransportError::MaybeSent`], never a body cut to the limit.
pub trait Transport {
    fn send(&self, request: &PreparedRequest) -> Result<TransportResponse, TransportError>;
}

/// What the provider answered, whatever the status.
#[derive(Clone, PartialEq, Eq)]
pub struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

// Redacted on purpose: the body may hold conversation text.
impl fmt::Debug for TransportResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportResponse")
            .field("status", &self.status)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

/// No answer to read. Carries no detail, so no URL, key or body can travel in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    /// Certainly no part of the request left. The spend held for it is released.
    NotSent,
    /// The provider may have received the request. Any doubt belongs here.
    MaybeSent,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotSent => "the request was not sent",
            Self::MaybeSent => "the request may have reached the provider",
        })
    }
}

impl std::error::Error for TransportError {}
