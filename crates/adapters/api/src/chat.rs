//! Map a request to the Chat Completions wire format and a response back. No network, no key.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::config::ProviderConfig;

/// Largest response body the mapping reads.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Errors never carry request or response text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    EmptyMessages,
    MaxTokens,
    Temperature,
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
struct WireRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    stream: bool,
}

/// Reference to a key the caller keeps in the product vault. The adapter never sees the key.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyHandle(String);

impl KeyHandle {
    pub fn new(reference: impl Into<String>) -> Self {
        Self(reference.into())
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
    key: KeyHandle,
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
    let wire = WireRequest {
        model: provider.model.clone(),
        messages: request.messages.clone(),
        max_tokens,
        temperature: request.temperature,
        stream: false,
    };
    let body = serde_json::to_string(&wire).map_err(|_| MapError::MalformedRequest)?;
    let worst_case_micro_usd = provider.cost_micro_usd(body.len() as u64, u64::from(max_tokens));
    Ok(PreparedRequest {
        url: provider.url.clone(),
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        key: KeyAuth {
            header: provider.key_header.clone(),
            prefix: provider.key_prefix.clone(),
            handle: key,
        },
        body,
        worst_case_micro_usd,
    })
}

/// Read a request body back: the model it names and the request that produced it.
pub fn parse_request_body(body: &str) -> Result<(String, ChatRequest), MapError> {
    let wire: WireRequest = serde_json::from_str(body).map_err(|_| MapError::MalformedRequest)?;
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
    /// `None` when the provider did not report both counts; the caller then charges the reserve.
    pub usage: Option<TokenUsage>,
}

impl fmt::Debug for ChatResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatResponse")
            .field("text_bytes", &self.text.len())
            .field("finish_reason", &self.finish_reason)
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
            (Some(tokens_in), Some(tokens_out)) => Some(TokenUsage {
                tokens_in,
                tokens_out,
            }),
            _ => None,
        });
    Ok(ChatResponse {
        model: wire.model,
        text: choice.message.content.unwrap_or_default(),
        finish_reason: choice.finish_reason,
        usage,
    })
}
