//! Map a request to the Chat Completions wire format and a response back. No network, no key.

use std::fmt;
use std::io::{self, Write};

use serde::{Deserialize, Serialize};

use crate::config::ProviderConfig;

/// Largest response body the mapping reads.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Largest token count taken from a provider, 2^53 - 1: not every reader of the usage record
/// holds a larger integer exactly.
pub const MAX_TOKEN_COUNT: u64 = (1 << 53) - 1;

/// Errors never carry request or response text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    EmptyMessages,
    MaxTokens,
    Temperature,
    RequestTooLarge,
    MalformedRequest,
    MalformedResponse,
    ResponseTooLarge,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyMessages => "request has no messages",
            Self::MaxTokens => "max_tokens is zero or above the provider limit",
            Self::Temperature => "temperature is not between 0 and 2",
            Self::RequestTooLarge => "request body is too large",
            Self::MalformedRequest => "request body is not a valid chat request",
            Self::MalformedResponse => "response body is not a valid chat response",
            Self::ResponseTooLarge => "response body is too large",
        })
    }
}

impl std::error::Error for MapError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

// Redacted on purpose: prompt text must not reach a log through `{:?}`.
impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Message({:?}, {} bytes)", self.role, self.content.len())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    /// Defaults to the provider limit; must not exceed it.
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest<M, S> {
    model: M,
    messages: S,
    max_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    stream: bool,
}

struct RequestBody {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for RequestBody {
    fn write(&mut self, chunk: &[u8]) -> io::Result<usize> {
        if chunk.len() > self.limit - self.bytes.len() {
            return Err(io::Error::from(io::ErrorKind::FileTooLarge));
        }
        self.bytes.extend_from_slice(chunk);
        Ok(chunk.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Reference to a key kept in the product vault, taken from the provider's active key alias.
/// The adapter never sees the key.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyHandle(String);

impl KeyHandle {
    pub(crate) fn new(reference: String) -> Self {
        Self(reference)
    }

    /// For the transport that resolves the handle at the last moment.
    pub fn reference(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for KeyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyHandle(<redacted>)")
    }
}

/// Where the transport puts the key: header name and prefix come from the config, the value
/// from resolving the handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyAuth {
    pub header: String,
    pub prefix: String,
    pub handle: KeyHandle,
}

/// A POST ready for a transport. Everything except the key value is already decided.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub key: KeyAuth,
    pub body: String,
    /// Upper bound of the cost: the body size counts as input tokens, `max_tokens` as output.
    pub worst_case_micro_usd: u64,
}

impl fmt::Debug for PreparedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedRequest")
            .field("url", &self.url)
            .field("body_bytes", &self.body.len())
            .field("worst_case_micro_usd", &self.worst_case_micro_usd)
            .finish_non_exhaustive()
    }
}

pub fn build_request(
    provider: &ProviderConfig,
    request: &ChatRequest,
) -> Result<PreparedRequest, MapError> {
    if request.messages.is_empty() {
        return Err(MapError::EmptyMessages);
    }
    let max_tokens = request.max_tokens.unwrap_or(provider.max_output_tokens);
    if max_tokens == 0 || max_tokens > provider.max_output_tokens {
        return Err(MapError::MaxTokens);
    }
    if let Some(t) = request.temperature
        && !(0.0..=2.0).contains(&t)
    {
        return Err(MapError::Temperature);
    }
    let limit = provider.max_request_bytes;
    request
        .messages
        .iter()
        .try_fold(limit, |remaining, message| {
            remaining.checked_sub(message.content.len())
        })
        .ok_or(MapError::RequestTooLarge)?;
    let wire = WireRequest {
        model: provider.model.as_str(),
        messages: request.messages.as_slice(),
        max_tokens,
        temperature: request.temperature,
        stream: false,
    };
    let mut output = RequestBody {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut output, &wire).map_err(|error| match error.io_error_kind() {
        Some(io::ErrorKind::FileTooLarge) => MapError::RequestTooLarge,
        _ => MapError::MalformedRequest,
    })?;
    let body = String::from_utf8(output.bytes).map_err(|_| MapError::MalformedRequest)?;
    let worst_case_micro_usd = provider.cost_micro_usd(body.len() as u64, u64::from(max_tokens));
    Ok(PreparedRequest {
        url: provider.url.clone(),
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        key: KeyAuth {
            header: provider.key_header.clone(),
            prefix: provider.key_prefix.clone(),
            handle: provider.key_handle(),
        },
        body,
        worst_case_micro_usd,
    })
}

/// Read a request body back: the model it names and the request that produced it.
pub fn parse_request_body(body: &str) -> Result<(String, ChatRequest), MapError> {
    let wire: WireRequest<String, Vec<Message>> =
        serde_json::from_str(body).map_err(|_| MapError::MalformedRequest)?;
    Ok((
        wire.model,
        ChatRequest {
            messages: wire.messages,
            max_tokens: Some(wire.max_tokens),
            temperature: wire.temperature,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsage {
    pub tokens_in: u64,
    pub tokens_out: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChatResponse {
    pub model: Option<String>,
    pub text: String,
    pub finish_reason: Option<String>,
    /// `None` unless the provider reported both counts, neither above [`MAX_TOKEN_COUNT`]; the
    /// caller then charges the reserve.
    pub usage: Option<TokenUsage>,
}

impl fmt::Debug for ChatResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatResponse")
            .field("text_bytes", &self.text.len())
            // Provider text like any other part of the body: only its presence is shown.
            .field("has_finish_reason", &self.finish_reason.is_some())
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct WireResponse {
    model: Option<String>,
    choices: Vec<WireChoice>,
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct WireChoice {
    message: WireMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct WireMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct WireUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

pub fn parse_response(body: &str) -> Result<ChatResponse, MapError> {
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(MapError::ResponseTooLarge);
    }
    let wire: WireResponse = serde_json::from_str(body).map_err(|_| MapError::MalformedResponse)?;
    let choice = wire
        .choices
        .into_iter()
        .next()
        .ok_or(MapError::MalformedResponse)?;
    let usage = wire
        .usage
        .and_then(|u| match (u.prompt_tokens, u.completion_tokens) {
            (Some(tokens_in), Some(tokens_out)) if tokens_in.max(tokens_out) <= MAX_TOKEN_COUNT => {
                Some(TokenUsage {
                    tokens_in,
                    tokens_out,
                })
            }
            _ => None,
        });
    Ok(ChatResponse {
        model: wire.model,
        text: choice.message.content.unwrap_or_default(),
        finish_reason: choice.finish_reason,
        usage,
    })
}
