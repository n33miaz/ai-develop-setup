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
mod tests;
