//! The `pty-host/1` contract: frame codec and message types.
//!
//! The JSON shapes are the types `ads-contracts` generates from
//! `contracts/schemas/contract.pty-host.*`, re-exported here under the names the host uses.
//! What stays written here is what the Schemas do not say: the binary frame, which JSON-RPC
//! error answers a malformed request, and the few places where the host holds a value in
//! another form than the wire does (see `crates/contracts/swap-map.md`). Each of those reads
//! the generated type first and converts, so a message is never accepted past its Schema.
//! Nothing else in the crate knows the byte layout or the JSON field names.

use std::fmt;
use std::io;
use std::num::{NonZeroU32, NonZeroU64};

use ads_contracts::{
    CommonBase64V1, ContractPtyHostFrameV1, ContractPtyHostNotificationV1,
    ContractPtyHostNotificationV1SessionExited, ContractPtyHostNotificationV1SessionExitedJsonrpc,
    ContractPtyHostNotificationV1SubscriptionClosed, ContractPtyHostRequestV1HostHelloJsonrpc,
    ContractPtyHostResponseV1, ContractPtyHostResponseV1Error, ContractPtyHostResponseV1ErrorCode,
    ContractPtyHostResponseV1ErrorFields, ContractPtyHostResponseV1ErrorMessage,
    ContractPtyHostResponseV1Fields, ContractPtyHostResponseV1Jsonrpc,
};
pub use ads_contracts::{
    CommonIdV1 as SessionId, ContractPtyHostNotificationV1SessionExitedParams as SessionExited,
    ContractPtyHostNotificationV1SubscriptionClosedParams as SubscriptionClosed,
    ContractPtyHostNotificationV1SubscriptionClosedParamsReason as CloseReason,
    ContractPtyHostRequestV1SessionCloseParams as SessionParams,
    ContractPtyHostRequestV1SessionListParams as NoParams,
    ContractPtyHostRequestV1SessionOpenParams as OpenParams,
    ContractPtyHostRequestV1SessionResizeParams as ResizeParams,
    ContractPtyHostRequestV1SessionSubscribeParams as SubscribeParams,
    ContractPtyHostRequestV1SessionWriteParams as WriteParams,
    ContractPtyHostResponseV1Closed as Closed, ContractPtyHostResponseV1Empty as Empty,
    ContractPtyHostResponseV1ErrorData as ErrorData, ContractPtyHostResponseV1Hello as HelloResult,
    ContractPtyHostResponseV1ReplayBounds as ReplayBounds,
    ContractPtyHostResponseV1Result as ResultBody,
    ContractPtyHostResponseV1SessionList as SessionList,
    ContractPtyHostResponseV1SessionListSessionsItem as SessionEntry,
};
use serde::de::DeserializeOwned;
use serde::ser::Error as _;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt};

/// The contract version this module speaks.
pub const CONTRACT: &str = "pty-host/1";

pub const KIND_JSON: u8 = 0x01;
pub const KIND_OUTPUT: u8 = 0x02;

const PREFIX_LEN: usize = 5;
const HEADER_LEN_LEN: usize = 2;

const ERROR_MESSAGE_MAX: usize = 500;

// ---- frame ---------------------------------------------------------------------------------

#[derive(Debug)]
pub enum WireError {
    Io(io::Error),
    UnknownKind(u8),
    /// The payload does not fit the limit of this side (or the `u32`/`u16` of the layout).
    TooLarge {
        len: usize,
        max: usize,
    },
    /// An output frame whose sizes do not add up or whose header is not valid.
    BadOutputFrame(String),
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "channel i/o failed: {err}"),
            Self::UnknownKind(kind) => write!(f, "unknown frame kind 0x{kind:02x}"),
            Self::TooLarge { len, max } => write!(f, "frame of {len} bytes exceeds {max}"),
            Self::BadOutputFrame(why) => write!(f, "malformed output frame: {why}"),
        }
    }
}

impl std::error::Error for WireError {}

impl From<io::Error> for WireError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// One JSON message, still as bytes.
    Json(Vec<u8>),
    /// Terminal output: the header and the raw bytes that followed it.
    Output {
        header: OutputHeader,
        bytes: Vec<u8>,
    },
}

/// Header of an output frame (`urn:ads:contract:pty-host.frame:1`).
///
/// The Schema gives `length` no ceiling; the frame prefix counts the payload in 32 bits, so a
/// header that passes the Schema with a larger `length` is refused here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ContractPtyHostFrameV1", into = "ContractPtyHostFrameV1")]
pub struct OutputHeader {
    pub session_id: SessionId,
    pub seq: NonZeroU64,
    pub length: NonZeroU32,
}

impl TryFrom<ContractPtyHostFrameV1> for OutputHeader {
    type Error = String;
    fn try_from(header: ContractPtyHostFrameV1) -> Result<Self, String> {
        let length = NonZeroU32::try_from(header.length)
            .map_err(|_| format!("length {} does not fit a frame", header.length))?;
        Ok(Self {
            session_id: header.session_id,
            seq: header.seq,
            length,
        })
    }
}

impl From<OutputHeader> for ContractPtyHostFrameV1 {
    fn from(header: OutputHeader) -> Self {
        Self {
            session_id: header.session_id,
            seq: header.seq,
            length: header.length.into(),
        }
    }
}

fn frame(kind: u8, parts: &[&[u8]]) -> Result<Vec<u8>, WireError> {
    let len: usize = parts.iter().map(|part| part.len()).sum();
    let payload_len = u32::try_from(len).map_err(|_| WireError::TooLarge {
        len,
        max: u32::MAX as usize,
    })?;
    let mut out = Vec::with_capacity(PREFIX_LEN + len);
    out.push(kind);
    out.extend_from_slice(&payload_len.to_be_bytes());
    for part in parts {
        out.extend_from_slice(part);
    }
    Ok(out)
}

/// A whole `0x01` frame carrying `message`.
pub fn encode_json<T: Serialize>(message: &T) -> Result<Vec<u8>, WireError> {
    let json = serde_json::to_vec(message).map_err(|err| WireError::Io(err.into()))?;
    frame(KIND_JSON, &[&json])
}

/// A whole `0x02` frame carrying one numbered piece of terminal output.
pub fn encode_output(session_id: &SessionId, seq: u64, bytes: &[u8]) -> Result<Vec<u8>, WireError> {
    let bad = |why: &str| WireError::BadOutputFrame(why.to_owned());
    let header = OutputHeader {
        session_id: session_id.clone(),
        seq: NonZeroU64::new(seq).ok_or_else(|| bad("seq starts at 1"))?,
        length: u32::try_from(bytes.len())
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| bad("output must have between 1 and u32::MAX bytes"))?,
    };
    let header = serde_json::to_vec(&header).map_err(|err| WireError::Io(err.into()))?;
    let header_len = u16::try_from(header.len()).map_err(|_| WireError::TooLarge {
        len: header.len(),
        max: u16::MAX as usize,
    })?;
    frame(KIND_OUTPUT, &[&header_len.to_be_bytes(), &header, bytes])
}

/// Turns the payload of a frame of the given kind into a [`Frame`].
pub fn decode_payload(kind: u8, payload: Vec<u8>) -> Result<Frame, WireError> {
    match kind {
        KIND_JSON => Ok(Frame::Json(payload)),
        KIND_OUTPUT => decode_output(&payload),
        other => Err(WireError::UnknownKind(other)),
    }
}

fn decode_output(payload: &[u8]) -> Result<Frame, WireError> {
    let bad = |why: &str| WireError::BadOutputFrame(why.to_owned());
    let (len, rest) = payload
        .split_first_chunk::<HEADER_LEN_LEN>()
        .ok_or_else(|| bad("shorter than the header length field"))?;
    let header_len = usize::from(u16::from_be_bytes(*len));
    let (header, bytes) = rest
        .split_at_checked(header_len)
        .ok_or_else(|| bad("header runs past the payload"))?;
    let header: OutputHeader =
        serde_json::from_slice(header).map_err(|err| WireError::BadOutputFrame(err.to_string()))?;
    if u32::try_from(bytes.len()).ok() != Some(header.length.get()) {
        return Err(bad("payload_len is not 2 + header_len + length"));
    }
    Ok(Frame::Output {
        header,
        bytes: bytes.to_vec(),
    })
}

/// Reads one frame. `None` means the peer closed the channel between two frames.
pub async fn read_frame<R>(reader: &mut R, max_payload: usize) -> Result<Option<Frame>, WireError>
where
    R: AsyncRead + Unpin,
{
    let mut kind = [0_u8; 1];
    if reader.read(&mut kind).await? == 0 {
        return Ok(None);
    }
    let kind = kind[0];
    if kind != KIND_JSON && kind != KIND_OUTPUT {
        return Err(WireError::UnknownKind(kind));
    }
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > max_payload {
        return Err(WireError::TooLarge {
            len,
            max: max_payload,
        });
    }
    let mut payload = vec![0_u8; len];
    reader.read_exact(&mut payload).await?;
    decode_payload(kind, payload).map(Some)
}

// ---- small values ---------------------------------------------------------------------------

/// Request number, per connection, starting at 1.
pub type RequestId = NonZeroU64;

/// The bytes a `urn:ads:common:base64:1` text stands for. The generated type is the text and
/// checks its shape; turning it into bytes and back is done here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Base64(pub Vec<u8>);

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let word = group
            .iter()
            .fold(0_u32, |word, byte| (word << 8) | u32::from(*byte))
            << (8 * (3 - group.len()));
        for index in 0..4 {
            if index <= group.len() {
                let sextet = (word >> (18 - 6 * index)) & 0x3f;
                out.push(char::from(BASE64_ALPHABET[sextet as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.as_bytes();
    if !text.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let groups = text.len() / 4;
    for (index, group) in text.chunks(4).enumerate() {
        let padding = group.iter().rev().take_while(|byte| **byte == b'=').count();
        if padding > 2 || (padding > 0 && index + 1 != groups) {
            return None;
        }
        let mut word = 0_u32;
        for byte in &group[..4 - padding] {
            let sextet = BASE64_ALPHABET.iter().position(|letter| letter == byte)?;
            word = (word << 6) | sextet as u32;
        }
        word <<= 6 * padding;
        out.extend_from_slice(&word.to_be_bytes()[1..4 - padding]);
    }
    Some(out)
}

impl From<&Base64> for CommonBase64V1 {
    fn from(bytes: &Base64) -> Self {
        Self::new(base64_encode(&bytes.0)).expect("the encoder writes padded standard base64")
    }
}

impl From<Base64> for CommonBase64V1 {
    fn from(bytes: Base64) -> Self {
        Self::from(&bytes)
    }
}

impl TryFrom<&CommonBase64V1> for Base64 {
    type Error = &'static str;
    fn try_from(text: &CommonBase64V1) -> Result<Self, Self::Error> {
        base64_decode(text.as_str())
            .map(Self)
            .ok_or("not padded standard base64")
    }
}

fn to_value<T: Serialize>(value: &T) -> Value {
    // The message types hold only strings, numbers and string-keyed maps: this cannot fail.
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// What a refused `params` says. The generated types put the refused value in their error;
/// an answer names only the type, so it never carries back what the client sent.
fn params_error(err: &serde_json::Error) -> String {
    let text = err.to_string();
    match text.strip_prefix("not a valid ") {
        Some(rest) => format!("not a valid {}", rest.split(':').next().unwrap_or(rest)),
        None => text,
    }
}

// ---- request -------------------------------------------------------------------------------

/// A request from the client (`urn:ads:contract:pty-host.request:1`).
///
/// The envelope is read in steps and not through the generated `ContractPtyHostRequestV1`,
/// because each step that fails is a different JSON-RPC error (see [`Request::parse`]) and
/// because the host has to read a `host.hello` of a version it does not speak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawRequest", into = "RawRequest")]
pub struct Request {
    pub id: RequestId,
    pub call: Call,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Hello(HelloParams),
    Open(OpenParams),
    Write(WriteParams),
    Resize(ResizeParams),
    Subscribe(SubscribeParams),
    Close(SessionParams),
    List(NoParams),
}

/// `params` of `host.hello`. The generated type only reads `pty-host/1`; this one reads any
/// version, so the host can answer an unknown one with its own error code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloParams {
    pub contract: String,
}

impl HelloParams {
    /// Whether the client asked for the version this host speaks.
    pub fn is_supported(&self) -> bool {
        self.contract == CONTRACT
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    jsonrpc: ContractPtyHostRequestV1HostHelloJsonrpc,
    id: RequestId,
    method: String,
    params: Value,
}

/// Why a request was refused before reaching a session, ready to become an error response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    /// The request number, when it could be read.
    pub id: Option<RequestId>,
    pub code: ErrorCode,
    pub message: String,
}

impl Call {
    pub fn method(&self) -> &'static str {
        match self {
            Self::Hello(_) => "host.hello",
            Self::Open(_) => "session.open",
            Self::Write(_) => "session.write",
            Self::Resize(_) => "session.resize",
            Self::Subscribe(_) => "session.subscribe",
            Self::Close(_) => "session.close",
            Self::List(_) => "session.list",
        }
    }

    fn params(&self) -> Value {
        match self {
            Self::Hello(params) => to_value(params),
            Self::Open(params) => to_value(params),
            Self::Write(params) => to_value(params),
            Self::Resize(params) => to_value(params),
            Self::Subscribe(params) => to_value(params),
            Self::Close(params) => to_value(params),
            Self::List(params) => to_value(params),
        }
    }

    fn from_parts(method: &str, params: Value) -> Result<Self, (ErrorCode, String)> {
        fn typed<T: DeserializeOwned>(params: Value) -> Result<T, (ErrorCode, String)> {
            serde_json::from_value(params)
                .map_err(|err| (ErrorCode::InvalidParams, params_error(&err)))
        }
        Ok(match method {
            "host.hello" => Self::Hello(typed(params)?),
            "session.open" => Self::Open(typed(params)?),
            "session.write" => Self::Write(typed(params)?),
            "session.resize" => Self::Resize(typed(params)?),
            "session.subscribe" => Self::Subscribe(typed(params)?),
            "session.close" => Self::Close(typed(params)?),
            "session.list" => Self::List(typed(params)?),
            other => {
                return Err((
                    ErrorCode::MethodNotFound,
                    format!("unknown method {other:?}"),
                ));
            }
        })
    }
}

impl Request {
    /// Parses the payload of a JSON frame, telling apart the JSON-RPC failure classes.
    pub fn parse(payload: &[u8]) -> Result<Self, Rejection> {
        let value: Value = serde_json::from_slice(payload).map_err(|err| Rejection {
            id: None,
            code: ErrorCode::ParseError,
            message: err.to_string(),
        })?;
        let id = value
            .get("id")
            .and_then(Value::as_u64)
            .and_then(RequestId::new);
        let raw: RawRequest = serde_json::from_value(value).map_err(|err| Rejection {
            id,
            code: ErrorCode::InvalidRequest,
            message: err.to_string(),
        })?;
        match Call::from_parts(&raw.method, raw.params) {
            Ok(call) => Ok(Self { id: raw.id, call }),
            Err((code, message)) => Err(Rejection {
                id: Some(raw.id),
                code,
                message,
            }),
        }
    }
}

impl TryFrom<RawRequest> for Request {
    type Error = String;
    fn try_from(raw: RawRequest) -> Result<Self, String> {
        let call = Call::from_parts(&raw.method, raw.params).map_err(|(_, message)| message)?;
        Ok(Self { id: raw.id, call })
    }
}

impl From<Request> for RawRequest {
    fn from(request: Request) -> Self {
        Self {
            jsonrpc: ContractPtyHostRequestV1HostHelloJsonrpc,
            id: request.id,
            method: request.call.method().to_owned(),
            params: request.call.params(),
        }
    }
}

// ---- response ------------------------------------------------------------------------------

/// A response from the host (`urn:ads:contract:pty-host.response:1`).
///
/// On the wire `result` and `error` are two optional fields and the Schema demands exactly
/// one; here they are one `Result`. Reading goes through the generated type, which checks it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "ContractPtyHostResponseV1")]
pub struct Response {
    /// `None` only when the request was too broken to carry a number.
    pub id: Option<RequestId>,
    pub outcome: Result<ResultBody, ErrorBody>,
}

impl TryFrom<ContractPtyHostResponseV1> for Response {
    type Error = String;
    fn try_from(wire: ContractPtyHostResponseV1) -> Result<Self, String> {
        let wire = wire.into_fields();
        let outcome = match (wire.result, wire.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error.into()),
            _ => return Err("a response carries either result or error".to_owned()),
        };
        Ok(Self {
            id: wire.id,
            outcome,
        })
    }
}

impl Serialize for Response {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (result, error) = match &self.outcome {
            Ok(result) => (Some(result.clone()), None),
            Err(error) => (
                None,
                Some(ContractPtyHostResponseV1Error::try_from(error).map_err(S::Error::custom)?),
            ),
        };
        ContractPtyHostResponseV1Fields {
            jsonrpc: ContractPtyHostResponseV1Jsonrpc,
            id: self.id,
            result,
            error,
        }
        .serialize(serializer)
    }
}

/// The error codes of the contract by what they mean. The generated type names them by number
/// (`Neg32004`); the two matches below are the whole map, and the compiler keeps it complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
    UnknownSession,
    SessionExists,
    TooManySessions,
    ReplayEvicted,
    ReplayAhead,
    SessionEnded,
    SpawnFailed,
    TerminalIo,
    UnsupportedContract,
}

impl ErrorCode {
    pub fn number(self) -> i64 {
        ContractPtyHostResponseV1ErrorCode::from(self).number()
    }
}

impl From<ErrorCode> for ContractPtyHostResponseV1ErrorCode {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::ParseError => Self::Neg32700,
            ErrorCode::InvalidRequest => Self::Neg32600,
            ErrorCode::MethodNotFound => Self::Neg32601,
            ErrorCode::InvalidParams => Self::Neg32602,
            ErrorCode::InternalError => Self::Neg32603,
            ErrorCode::UnknownSession => Self::Neg32001,
            ErrorCode::SessionExists => Self::Neg32002,
            ErrorCode::TooManySessions => Self::Neg32003,
            ErrorCode::ReplayEvicted => Self::Neg32004,
            ErrorCode::ReplayAhead => Self::Neg32005,
            ErrorCode::SessionEnded => Self::Neg32006,
            ErrorCode::SpawnFailed => Self::Neg32007,
            ErrorCode::TerminalIo => Self::Neg32008,
            ErrorCode::UnsupportedContract => Self::Neg32009,
        }
    }
}

impl From<ContractPtyHostResponseV1ErrorCode> for ErrorCode {
    fn from(code: ContractPtyHostResponseV1ErrorCode) -> Self {
        use ContractPtyHostResponseV1ErrorCode as Wire;
        match code {
            Wire::Neg32700 => Self::ParseError,
            Wire::Neg32600 => Self::InvalidRequest,
            Wire::Neg32601 => Self::MethodNotFound,
            Wire::Neg32602 => Self::InvalidParams,
            Wire::Neg32603 => Self::InternalError,
            Wire::Neg32001 => Self::UnknownSession,
            Wire::Neg32002 => Self::SessionExists,
            Wire::Neg32003 => Self::TooManySessions,
            Wire::Neg32004 => Self::ReplayEvicted,
            Wire::Neg32005 => Self::ReplayAhead,
            Wire::Neg32006 => Self::SessionEnded,
            Wire::Neg32007 => Self::SpawnFailed,
            Wire::Neg32008 => Self::TerminalIo,
            Wire::Neg32009 => Self::UnsupportedContract,
        }
    }
}

/// The `error` of a response, with the code by its meaning. Read through the generated type,
/// which checks the message length and that a replay refusal carries its bound; written
/// through it too, so an error outside the contract fails to encode instead of going out.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "ContractPtyHostResponseV1Error")]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    pub data: Option<ErrorData>,
}

impl ErrorBody {
    /// Builds an error whose message always fits the contract (1 to 500 characters).
    pub fn new(code: ErrorCode, message: &str, data: Option<ErrorData>) -> Self {
        let mut message: String = message.chars().take(ERROR_MESSAGE_MAX).collect();
        if message.is_empty() {
            message.push_str("error");
        }
        Self {
            code,
            message,
            data,
        }
    }
}

impl From<ContractPtyHostResponseV1Error> for ErrorBody {
    fn from(wire: ContractPtyHostResponseV1Error) -> Self {
        let wire = wire.into_fields();
        Self {
            code: wire.code.into(),
            message: wire.message.into(),
            data: wire.data,
        }
    }
}

impl TryFrom<&ErrorBody> for ContractPtyHostResponseV1Error {
    type Error = ads_contracts::ContractError;
    fn try_from(error: &ErrorBody) -> Result<Self, Self::Error> {
        Self::new(ContractPtyHostResponseV1ErrorFields {
            code: error.code.into(),
            message: ContractPtyHostResponseV1ErrorMessage::new(error.message.as_str())?,
            data: error.data.clone(),
        })
    }
}

impl Serialize for ErrorBody {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ContractPtyHostResponseV1Error::try_from(self)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

// ---- notification --------------------------------------------------------------------------

/// A notification from the host (`urn:ads:contract:pty-host.notification:1`).
///
/// Read through the generated type. Written by the envelope below, which keeps the order of
/// the keys the host has always sent (`jsonrpc`, `method`, `params`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "ContractPtyHostNotificationV1", into = "RawNotification")]
pub enum Notification {
    SessionExited(SessionExited),
    SubscriptionClosed(SubscriptionClosed),
}

impl From<ContractPtyHostNotificationV1> for Notification {
    fn from(wire: ContractPtyHostNotificationV1) -> Self {
        match wire {
            ContractPtyHostNotificationV1::SessionExited(
                ContractPtyHostNotificationV1SessionExited { params, .. },
            ) => Self::SessionExited(params),
            ContractPtyHostNotificationV1::SubscriptionClosed(
                ContractPtyHostNotificationV1SubscriptionClosed { params, .. },
            ) => Self::SubscriptionClosed(params),
        }
    }
}

#[derive(Serialize)]
struct RawNotification {
    jsonrpc: ContractPtyHostNotificationV1SessionExitedJsonrpc,
    method: &'static str,
    params: Value,
}

impl From<Notification> for RawNotification {
    fn from(notification: Notification) -> Self {
        let (method, params) = match &notification {
            Notification::SessionExited(params) => ("session.exited", to_value(params)),
            Notification::SubscriptionClosed(params) => ("subscription.closed", to_value(params)),
        };
        Self {
            jsonrpc: ContractPtyHostNotificationV1SessionExitedJsonrpc,
            method,
            params,
        }
    }
}

/// What a client can find in a JSON frame coming from the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostMessage {
    Response(Response),
    Notification(Notification),
}

impl HostMessage {
    /// A message with `method` is a notification; anything else has to be a response.
    pub fn parse(payload: &[u8]) -> Result<Self, serde_json::Error> {
        let value: Value = serde_json::from_slice(payload)?;
        if value.get("method").is_some() {
            serde_json::from_value(value).map(Self::Notification)
        } else {
            serde_json::from_value(value).map(Self::Response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> SessionId {
        SessionId::new(text).unwrap()
    }

    #[test]
    fn output_frame_has_the_documented_layout() {
        let frame = encode_output(&id("sess-01"), 7, b"hello").unwrap();
        let header = br#"{"session_id":"sess-01","seq":7,"length":5}"#;

        assert_eq!(frame[0], 0x02);
        let payload_len = u32::from_be_bytes(frame[1..5].try_into().unwrap()) as usize;
        assert_eq!(payload_len, frame.len() - 5);
        assert_eq!(payload_len, 2 + header.len() + 5);
        let header_len = usize::from(u16::from_be_bytes(frame[5..7].try_into().unwrap()));
        assert_eq!(header_len, header.len());
        assert_eq!(&frame[7..7 + header_len], header);
        assert_eq!(&frame[7 + header_len..], b"hello");
    }

    #[test]
    fn json_frame_is_kind_length_and_the_message() {
        let frame = encode_json(&serde_json::json!({"a": 1})).unwrap();
        assert_eq!(frame, [&[0x01, 0, 0, 0, 7][..], br#"{"a":1}"#].concat());
    }

    #[tokio::test]
    async fn frames_round_trip_through_the_reader() {
        let mut stream = encode_json(&serde_json::json!({"a": 1})).unwrap();
        stream.extend(encode_output(&id("s"), 3, &[0, 255, 10, 13]).unwrap());
        let mut reader = stream.as_slice();

        assert_eq!(
            read_frame(&mut reader, 1024).await.unwrap(),
            Some(Frame::Json(br#"{"a":1}"#.to_vec()))
        );
        let Some(Frame::Output { header, bytes }) = read_frame(&mut reader, 1024).await.unwrap()
        else {
            panic!("expected an output frame");
        };
        assert_eq!((header.session_id, header.seq.get()), (id("s"), 3));
        assert_eq!(bytes, [0, 255, 10, 13]);
        assert_eq!(read_frame(&mut reader, 1024).await.unwrap(), None);
    }

    #[tokio::test]
    async fn unknown_kind_and_oversized_payload_are_refused() {
        let mut unknown = [0x03_u8, 0, 0, 0, 0].as_slice();
        assert!(matches!(
            read_frame(&mut unknown, 1024).await,
            Err(WireError::UnknownKind(0x03))
        ));
        let mut big = [0x01_u8, 0, 0, 4, 1].as_slice();
        assert!(matches!(
            read_frame(&mut big, 1024).await,
            Err(WireError::TooLarge {
                len: 1025,
                max: 1024
            })
        ));
        let mut cut = [0x01_u8, 0, 0, 0, 9, b'{'].as_slice();
        assert!(matches!(
            read_frame(&mut cut, 1024).await,
            Err(WireError::Io(_))
        ));
    }

    #[test]
    fn output_frame_whose_sizes_do_not_add_up_is_refused() {
        let good = encode_output(&id("s"), 1, b"abc").unwrap();
        let payload = &good[5..];
        assert!(decode_payload(KIND_OUTPUT, payload.to_vec()).is_ok());

        // One byte more or less than `length` says.
        let mut longer = payload.to_vec();
        longer.push(b'd');
        assert!(matches!(
            decode_payload(KIND_OUTPUT, longer),
            Err(WireError::BadOutputFrame(_))
        ));
        let shorter = payload[..payload.len() - 1].to_vec();
        assert!(decode_payload(KIND_OUTPUT, shorter).is_err());
        // A header length that runs past the payload, and a payload with no room for it.
        let mut overrun = payload.to_vec();
        overrun[..2].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(decode_payload(KIND_OUTPUT, overrun).is_err());
        assert!(decode_payload(KIND_OUTPUT, vec![0]).is_err());
    }

    #[test]
    fn empty_output_and_seq_zero_cannot_be_framed() {
        assert!(encode_output(&id("s"), 1, b"").is_err());
        assert!(encode_output(&id("s"), 0, b"a").is_err());
    }

    #[test]
    fn base64_matches_the_rfc_vectors_and_refuses_loose_text() {
        let vectors: [(&[u8], &str); 7] = [
            (b"", ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (bytes, text) in vectors {
            assert_eq!(base64_encode(bytes), text);
            assert_eq!(base64_decode(text).as_deref(), Some(bytes));
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&base64_encode(&all)), Some(all));
        for loose in ["Zg", "Zg=", "Z===", "Zm9v\n", "Zg==Zg==", "Zm 9", "=Zg="] {
            assert_eq!(base64_decode(loose), None, "{loose:?}");
        }
    }

    #[test]
    fn session_id_follows_the_common_id_rule() {
        for good in ["a", "sess-01", "A.b_c:d-9", &"x".repeat(64)] {
            assert!(SessionId::new(good).is_ok(), "{good:?}");
        }
        for bad in ["", "-a", "a b", "a@b", "é", &"x".repeat(65)] {
            assert!(SessionId::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn request_parse_tells_the_failure_classes_apart() {
        let code = |payload: &str| Request::parse(payload.as_bytes()).unwrap_err();
        assert_eq!(code("{nope").code, ErrorCode::ParseError);
        assert_eq!(code("{nope").id, None);

        let invalid = code(r#"{"jsonrpc":"1.0","id":4,"method":"session.list","params":{}}"#);
        assert_eq!(
            (invalid.code, invalid.id),
            (ErrorCode::InvalidRequest, RequestId::new(4))
        );

        let unknown = code(r#"{"jsonrpc":"2.0","id":5,"method":"session.kill","params":{}}"#);
        assert_eq!(
            (unknown.code, unknown.id),
            (ErrorCode::MethodNotFound, RequestId::new(5))
        );

        let params = code(r#"{"jsonrpc":"2.0","id":6,"method":"session.close","params":{}}"#);
        assert_eq!(
            (params.code, params.id),
            (ErrorCode::InvalidParams, RequestId::new(6))
        );
    }

    #[test]
    fn built_errors_always_fit_the_contract() {
        let long = ErrorBody::new(ErrorCode::InternalError, &"x".repeat(900), None);
        assert_eq!(long.message.chars().count(), 500);
        let empty = ErrorBody::new(ErrorCode::InternalError, "", None);
        assert!(!empty.message.is_empty());
    }

    // ---- where the host and the generated types differ: reading the generated type with
    // success never stands in for the check the host makes on top of it.

    fn output_payload(header: &str, bytes: &[u8]) -> Vec<u8> {
        [
            &(header.len() as u16).to_be_bytes()[..],
            header.as_bytes(),
            bytes,
        ]
        .concat()
    }

    #[test]
    fn header_the_schema_accepts_with_a_length_over_32_bits_is_refused_by_the_codec() {
        for length in [u64::from(u32::MAX) + 1, u64::from(u32::MAX) + 4, u64::MAX] {
            let header = format!(r#"{{"session_id":"s","seq":1,"length":{length}}}"#);
            let generated: ContractPtyHostFrameV1 = serde_json::from_str(&header).unwrap();
            assert_eq!(generated.length.get(), length);

            assert!(OutputHeader::try_from(generated).is_err(), "{length}");
            assert!(serde_json::from_str::<OutputHeader>(&header).is_err());
            assert!(matches!(
                decode_payload(KIND_OUTPUT, output_payload(&header, b"abc")),
                Err(WireError::BadOutputFrame(_))
            ));
        }
        // The largest length the layout can say still converts both ways.
        let header = format!(r#"{{"session_id":"s","seq":1,"length":{}}}"#, u32::MAX);
        let read: OutputHeader = serde_json::from_str(&header).unwrap();
        assert_eq!(read.length.get(), u32::MAX);
        assert_eq!(serde_json::to_string(&read).unwrap(), header);
    }

    #[test]
    fn hello_of_another_version_is_read_by_the_host_and_refused_by_the_generated_type() {
        let hello = |contract: &str| {
            format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"host.hello","params":{{"contract":"{contract}"}}}}"#
            )
        };
        type Generated = ads_contracts::ContractPtyHostRequestV1;

        assert!(serde_json::from_str::<Generated>(&hello(CONTRACT)).is_ok());
        let Call::Hello(params) = Request::parse(hello(CONTRACT).as_bytes()).unwrap().call else {
            panic!("not a hello");
        };
        assert!(params.is_supported());

        for other in ["pty-host/2", "pty-host/10", "PTY-HOST/1", " pty-host/1", ""] {
            assert!(serde_json::from_str::<Generated>(&hello(other)).is_err());
            // Read, so that the host can answer with its own code; never taken for supported.
            let Call::Hello(params) = Request::parse(hello(other).as_bytes()).unwrap().call else {
                panic!("not a hello");
            };
            assert_eq!(params.contract, other);
            assert!(!params.is_supported(), "{other:?}");
        }
    }

    #[test]
    fn requests_the_host_writes_are_the_ones_the_generated_type_reads() {
        let session_id = id("sess-01");
        let calls = [
            Call::Hello(HelloParams {
                contract: CONTRACT.to_owned(),
            }),
            Call::Open(OpenParams {
                session_id: session_id.clone(),
                program: "sh".to_owned().try_into().unwrap(),
                args: vec!["-c".to_owned().try_into().unwrap()],
                cwd: Some("/work".to_owned().try_into().unwrap()),
                env: [("LANG".to_owned(), "C".to_owned())].into(),
                cols: 120.try_into().unwrap(),
                rows: 30.try_into().unwrap(),
            }),
            Call::Write(WriteParams {
                session_id: session_id.clone(),
                data: Base64(b"dir\r".to_vec()).into(),
            }),
            Call::Resize(ResizeParams {
                session_id: session_id.clone(),
                cols: 1.try_into().unwrap(),
                rows: 1000.try_into().unwrap(),
            }),
            Call::Subscribe(SubscribeParams {
                session_id: session_id.clone(),
                from_seq: None,
            }),
            Call::Close(SessionParams { session_id }),
            Call::List(NoParams {}),
        ];
        for call in calls {
            let request = Request {
                id: RequestId::MIN,
                call,
            };
            let written = serde_json::to_value(&request).unwrap();
            let generated: ads_contracts::ContractPtyHostRequestV1 =
                serde_json::from_value(written.clone()).unwrap();
            assert_eq!(serde_json::to_value(&generated).unwrap(), written);
            let payload = serde_json::to_vec(&generated).unwrap();
            assert_eq!(Request::parse(&payload).unwrap(), request);
        }
    }

    #[test]
    fn response_with_result_and_error_or_with_neither_is_refused_past_the_field_types() {
        let both = r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{"code":-32603,"message":"m"}}"#;
        let neither = r#"{"jsonrpc":"2.0","id":1}"#;
        for payload in [both, neither] {
            // Field by field the message is fine: each field has its type.
            assert!(serde_json::from_str::<ContractPtyHostResponseV1Fields>(payload).is_ok());
            assert!(serde_json::from_str::<Response>(payload).is_err());
            assert!(HostMessage::parse(payload.as_bytes()).is_err());
        }
        let one = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let Ok(HostMessage::Response(response)) = HostMessage::parse(one.as_bytes()) else {
            panic!("refused");
        };
        assert_eq!(response.outcome, Ok(ResultBody::Empty(Empty {})));
        assert_eq!(serde_json::to_string(&response).unwrap(), one);
    }

    #[test]
    fn error_codes_map_one_to_one_onto_the_numbers_of_the_contract() {
        let table = [
            (ErrorCode::ParseError, -32700),
            (ErrorCode::InvalidRequest, -32600),
            (ErrorCode::MethodNotFound, -32601),
            (ErrorCode::InvalidParams, -32602),
            (ErrorCode::InternalError, -32603),
            (ErrorCode::UnknownSession, -32001),
            (ErrorCode::SessionExists, -32002),
            (ErrorCode::TooManySessions, -32003),
            (ErrorCode::ReplayEvicted, -32004),
            (ErrorCode::ReplayAhead, -32005),
            (ErrorCode::SessionEnded, -32006),
            (ErrorCode::SpawnFailed, -32007),
            (ErrorCode::TerminalIo, -32008),
            (ErrorCode::UnsupportedContract, -32009),
        ];
        for (code, number) in table {
            assert_eq!(code.number(), number);
            let wire = ContractPtyHostResponseV1ErrorCode::try_from(number).unwrap();
            assert_eq!(ContractPtyHostResponseV1ErrorCode::from(code), wire);
            assert_eq!(ErrorCode::from(wire), code);
            // And through a whole message, in and out.
            let payload = format!(
                r#"{{"jsonrpc":"2.0","id":1,"error":{{"code":{number},"message":"m","data":{{"oldest_seq":1,"next_seq":2}}}}}}"#
            );
            let response: Response = serde_json::from_str(&payload).unwrap();
            assert_eq!(response.outcome.as_ref().unwrap_err().code, code);
            assert_eq!(serde_json::to_string(&response).unwrap(), payload);
        }
        let numbers: std::collections::HashSet<i64> = table.iter().map(|(_, n)| *n).collect();
        assert_eq!(numbers.len(), table.len());
    }

    #[test]
    fn error_outside_the_contract_fails_to_encode_instead_of_going_out() {
        let respond = |error: ErrorBody| Response {
            id: None,
            outcome: Err(error),
        };
        let no_data = || ErrorData {
            oldest_seq: None,
            next_seq: None,
            limit: None,
        };
        let bound = NonZeroU64::new(9);
        let broken = [
            ErrorBody::new(ErrorCode::ReplayEvicted, "m", None),
            ErrorBody::new(ErrorCode::ReplayEvicted, "m", Some(no_data())),
            ErrorBody::new(
                ErrorCode::ReplayEvicted,
                "m",
                Some(ErrorData {
                    next_seq: bound,
                    ..no_data()
                }),
            ),
            ErrorBody::new(ErrorCode::ReplayAhead, "m", None),
            ErrorBody {
                code: ErrorCode::InternalError,
                message: String::new(),
                data: None,
            },
            ErrorBody {
                code: ErrorCode::InternalError,
                message: "x".repeat(501),
                data: None,
            },
        ];
        for error in broken {
            assert!(serde_json::to_string(&error).is_err(), "{error:?}");
            assert!(encode_json(&respond(error)).is_err());
        }
        let fine = [
            ErrorBody::new(
                ErrorCode::ReplayEvicted,
                "m",
                Some(ErrorData {
                    oldest_seq: bound,
                    ..no_data()
                }),
            ),
            ErrorBody::new(
                ErrorCode::ReplayAhead,
                "m",
                Some(ErrorData {
                    next_seq: bound,
                    ..no_data()
                }),
            ),
            ErrorBody::new(ErrorCode::InternalError, &"x".repeat(900), None),
            ErrorBody::new(ErrorCode::InternalError, "", None),
        ];
        for error in fine {
            let text = serde_json::to_string(&respond(error.clone())).unwrap();
            let read: Response = serde_json::from_str(&text).unwrap();
            assert_eq!(read.outcome, Err(error));
        }
    }

    #[test]
    fn base64_text_the_schema_accepts_always_becomes_bytes_and_loose_text_never_gets_there() {
        for len in 0..=255_usize {
            let bytes: Vec<u8> = (0..len).map(|n| (n * 7) as u8).collect();
            let text = CommonBase64V1::from(Base64(bytes.clone()));
            assert_eq!(Base64::try_from(&text), Ok(Base64(bytes)));
        }
        // Padding bits that are not zero pass the pattern; they still decode, to the same bytes.
        for (text, bytes) in [("Zh==", b"f".as_slice()), ("Zm9=", b"fo".as_slice())] {
            let text = CommonBase64V1::new(text).unwrap();
            assert_eq!(Base64::try_from(&text), Ok(Base64(bytes.to_vec())));
        }
        for loose in [
            "Zg", "Zg=", "Z===", "Zm9v\n", "Zg==Zg==", "Zm 9", "=Zg=", "Zm9-",
        ] {
            assert!(CommonBase64V1::new(loose).is_err(), "{loose:?}");
            let payload = serde_json::json!({
                "jsonrpc": "2.0", "id": 3, "method": "session.write",
                "params": {"session_id": "a", "data": loose}
            });
            let rejection = Request::parse(payload.to_string().as_bytes()).unwrap_err();
            assert_eq!(rejection.code, ErrorCode::InvalidParams, "{loose:?}");
        }
    }

    #[test]
    fn refused_params_are_not_echoed_in_the_answer() {
        let marker = "s3cr3t-marker";
        let request = |method: &str, params: Value| {
            serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": method, "params": params})
                .to_string()
        };
        let open = |field: &str, value: Value| {
            let mut params = serde_json::json!({
                "session_id": "a", "program": "sh", "args": [], "cwd": null,
                "env": {}, "cols": 80, "rows": 24
            });
            params[field] = value;
            request("session.open", params)
        };
        let refused = [
            request(
                "session.close",
                serde_json::json!({"session_id": format!("a b {marker}")}),
            ),
            request(
                "session.write",
                serde_json::json!({"session_id": "a", "data": format!("{marker}!")}),
            ),
            open("program", format!("{marker}{}", "x".repeat(1100)).into()),
            open("cwd", format!("{marker}{}", "x".repeat(1100)).into()),
            open(
                "args",
                serde_json::json!([format!("{marker}{}", "x".repeat(8200))]),
            ),
        ];
        for payload in refused {
            let rejection = Request::parse(payload.as_bytes()).unwrap_err();
            assert_eq!(rejection.code, ErrorCode::InvalidParams, "{payload}");
            assert!(rejection.message.starts_with("not a valid "), "{payload}");
            assert!(!rejection.message.contains(marker), "{}", rejection.message);
        }
    }
}
