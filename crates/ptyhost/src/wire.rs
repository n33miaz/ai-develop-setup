//! The `pty-host/1` contract: frame codec and message types.
//!
//! Written by hand from `contracts/schemas/contract.pty-host.*` and checked against the golden
//! files in `contracts/golden/` (`tests/wire_golden.rs`). Nothing else in the crate knows the
//! byte layout or the JSON field names.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt};

/// The contract version this module speaks.
pub const CONTRACT: &str = "pty-host/1";

pub const KIND_JSON: u8 = 0x01;
pub const KIND_OUTPUT: u8 = 0x02;

const PREFIX_LEN: usize = 5;
const HEADER_LEN_LEN: usize = 2;

const SESSION_ID_MAX: usize = 64;
const PROGRAM_MAX: usize = 1024;
const ARG_MAX: usize = 8192;
const CWD_MAX: usize = 1024;
const DIMENSION_MAX: u16 = 1000;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputHeader {
    pub session_id: SessionId,
    pub seq: NonZeroU64,
    pub length: NonZeroU32,
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

// ---- small validated values ----------------------------------------------------------------

/// The literal `"2.0"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JsonRpc;

impl Serialize for JsonRpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for JsonRpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "2.0" => Ok(Self),
            other => Err(D::Error::custom(format!(
                "jsonrpc must be \"2.0\", got {other:?}"
            ))),
        }
    }
}

/// Request number, per connection, starting at 1.
pub type RequestId = NonZeroU64;

/// Opaque session name chosen by the client (`urn:ads:common:id:1`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SessionId(Arc<str>);

impl SessionId {
    pub fn new(text: &str) -> Result<Self, String> {
        let bytes = text.as_bytes();
        let first_ok = bytes.first().is_some_and(u8::is_ascii_alphanumeric);
        let rest_ok = bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'));
        if first_ok && rest_ok && bytes.len() <= SESSION_ID_MAX {
            Ok(Self(Arc::from(text)))
        } else {
            Err("session_id must be 1 to 64 of letters, digits and . _ : -".to_owned())
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SessionId {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        Self::new(&text)
    }
}

impl From<SessionId> for String {
    fn from(id: SessionId) -> Self {
        id.0.as_ref().to_owned()
    }
}

/// Columns or rows of a terminal: 1 to 1000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct Dimension(u16);

impl Dimension {
    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for Dimension {
    type Error = String;
    fn try_from(value: u16) -> Result<Self, String> {
        if (1..=DIMENSION_MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!("terminal size must be 1 to {DIMENSION_MAX}"))
        }
    }
}

impl From<Dimension> for u16 {
    fn from(value: Dimension) -> Self {
        value.0
    }
}

/// Bytes that travel as padded standard base64 (`urn:ads:common:base64:1`).
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

impl Serialize for Base64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64_encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Base64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        base64_decode(&String::deserialize(deserializer)?)
            .map(Self)
            .ok_or_else(|| D::Error::custom("not padded standard base64"))
    }
}

/// A field that must be present and may be `null`.
fn nullable<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// A field that may be absent (with `default`) but, when present, is never `null`.
fn present<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn bounded<E: serde::de::Error>(what: &str, text: &str, min: usize, max: usize) -> Result<(), E> {
    let len = text.chars().count();
    if (min..=max).contains(&len) {
        Ok(())
    } else {
        Err(E::custom(format!(
            "{what} must have {min} to {max} characters"
        )))
    }
}

fn program<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let text = String::deserialize(deserializer)?;
    bounded("program", &text, 1, PROGRAM_MAX)?;
    Ok(text)
}

fn args<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let args = Vec::<String>::deserialize(deserializer)?;
    for arg in &args {
        bounded("each argument", arg, 0, ARG_MAX)?;
    }
    Ok(args)
}

fn cwd<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    let cwd = Option::<String>::deserialize(deserializer)?;
    if let Some(cwd) = &cwd {
        bounded("cwd", cwd, 1, CWD_MAX)?;
    }
    Ok(cwd)
}

fn to_value<T: Serialize>(value: &T) -> Value {
    // The message types hold only strings, numbers and string-keyed maps: this cannot fail.
    serde_json::to_value(value).unwrap_or(Value::Null)
}

// ---- request -------------------------------------------------------------------------------

/// A request from the client (`urn:ads:contract:pty-host.request:1`).
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloParams {
    /// Any text here, so the host can answer an unknown version with its own error code.
    pub contract: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenParams {
    pub session_id: SessionId,
    #[serde(deserialize_with = "program")]
    pub program: String,
    #[serde(deserialize_with = "args")]
    pub args: Vec<String>,
    #[serde(deserialize_with = "cwd")]
    pub cwd: Option<String>,
    pub env: BTreeMap<String, String>,
    pub cols: Dimension,
    pub rows: Dimension,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteParams {
    pub session_id: SessionId,
    pub data: Base64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResizeParams {
    pub session_id: SessionId,
    pub cols: Dimension,
    pub rows: Dimension,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribeParams {
    pub session_id: SessionId,
    /// `None` asks for everything the replay ring still holds.
    #[serde(deserialize_with = "nullable")]
    pub from_seq: Option<NonZeroU64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionParams {
    pub session_id: SessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    jsonrpc: JsonRpc,
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
                .map_err(|err| (ErrorCode::InvalidParams, err.to_string()))
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
            jsonrpc: JsonRpc,
            id: request.id,
            method: request.call.method().to_owned(),
            params: request.call.params(),
        }
    }
}

// ---- response ------------------------------------------------------------------------------

/// A response from the host (`urn:ads:contract:pty-host.response:1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawResponse", into = "RawResponse")]
pub struct Response {
    /// `None` only when the request was too broken to carry a number.
    pub id: Option<RequestId>,
    pub outcome: Result<ResultBody, ErrorBody>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResponse {
    jsonrpc: JsonRpc,
    #[serde(deserialize_with = "nullable")]
    id: Option<RequestId>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    result: Option<ResultBody>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    error: Option<ErrorBody>,
}

impl TryFrom<RawResponse> for Response {
    type Error = String;
    fn try_from(raw: RawResponse) -> Result<Self, String> {
        let outcome = match (raw.result, raw.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error),
            _ => return Err("a response carries either result or error".to_owned()),
        };
        Ok(Self {
            id: raw.id,
            outcome,
        })
    }
}

impl From<Response> for RawResponse {
    fn from(response: Response) -> Self {
        let (result, error) = match response.outcome {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            jsonrpc: JsonRpc,
            id: response.id,
            result,
            error,
        }
    }
}

/// The shapes a `result` can have. `Empty` is last: it is what is left when nothing else fits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResultBody {
    Hello(HelloResult),
    ReplayBounds(ReplayBounds),
    Closed(Closed),
    SessionList(SessionList),
    Empty(NoParams),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloResult {
    pub contract: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayBounds {
    pub oldest_seq: NonZeroU64,
    pub next_seq: NonZeroU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Closed {
    #[serde(deserialize_with = "nullable")]
    pub exit_code: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionList {
    pub sessions: Vec<SessionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionEntry {
    pub session_id: SessionId,
    pub running: bool,
    #[serde(deserialize_with = "nullable")]
    pub exit_code: Option<i64>,
    pub oldest_seq: NonZeroU64,
    pub next_seq: NonZeroU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
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
    const ALL: [Self; 14] = [
        Self::ParseError,
        Self::InvalidRequest,
        Self::MethodNotFound,
        Self::InvalidParams,
        Self::InternalError,
        Self::UnknownSession,
        Self::SessionExists,
        Self::TooManySessions,
        Self::ReplayEvicted,
        Self::ReplayAhead,
        Self::SessionEnded,
        Self::SpawnFailed,
        Self::TerminalIo,
        Self::UnsupportedContract,
    ];

    pub fn number(self) -> i64 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
            Self::UnknownSession => -32001,
            Self::SessionExists => -32002,
            Self::TooManySessions => -32003,
            Self::ReplayEvicted => -32004,
            Self::ReplayAhead => -32005,
            Self::SessionEnded => -32006,
            Self::SpawnFailed => -32007,
            Self::TerminalIo => -32008,
            Self::UnsupportedContract => -32009,
        }
    }
}

impl TryFrom<i64> for ErrorCode {
    type Error = String;
    fn try_from(number: i64) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|code| code.number() == number)
            .ok_or_else(|| format!("{number} is not an error code of the contract"))
    }
}

impl From<ErrorCode> for i64 {
    fn from(code: ErrorCode) -> Self {
        code.number()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawError")]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ErrorData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorData {
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub oldest_seq: Option<NonZeroU64>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub next_seq: Option<NonZeroU64>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub limit: Option<NonZeroU64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawError {
    code: ErrorCode,
    message: String,
    #[serde(default, deserialize_with = "present")]
    data: Option<ErrorData>,
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

impl TryFrom<RawError> for ErrorBody {
    type Error = String;
    fn try_from(raw: RawError) -> Result<Self, String> {
        let len = raw.message.chars().count();
        if !(1..=ERROR_MESSAGE_MAX).contains(&len) {
            return Err(format!(
                "error message must have 1 to {ERROR_MESSAGE_MAX} characters"
            ));
        }
        let data = raw.data.unwrap_or_default();
        match raw.code {
            ErrorCode::ReplayEvicted if data.oldest_seq.is_none() => {
                return Err("a replay refusal must carry data.oldest_seq".to_owned());
            }
            ErrorCode::ReplayAhead if data.next_seq.is_none() => {
                return Err("a replay refusal must carry data.next_seq".to_owned());
            }
            _ => {}
        }
        Ok(Self {
            code: raw.code,
            message: raw.message,
            data: raw.data,
        })
    }
}

// ---- notification --------------------------------------------------------------------------

/// A notification from the host (`urn:ads:contract:pty-host.notification:1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawNotification", into = "RawNotification")]
pub enum Notification {
    SessionExited(SessionExited),
    SubscriptionClosed(SubscriptionClosed),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionExited {
    pub session_id: SessionId,
    #[serde(deserialize_with = "nullable")]
    pub exit_code: Option<i64>,
    /// Last numbered piece of output; 0 when there was none.
    pub last_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriptionClosed {
    pub session_id: SessionId,
    pub reason: CloseReason,
    /// Last piece this subscription delivered; subscribing again from `last_seq + 1` loses nothing.
    pub last_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    SlowSubscriber,
    SessionClosed,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNotification {
    jsonrpc: JsonRpc,
    method: String,
    params: Value,
}

impl TryFrom<RawNotification> for Notification {
    type Error = String;
    fn try_from(raw: RawNotification) -> Result<Self, String> {
        let typed = match raw.method.as_str() {
            "session.exited" => serde_json::from_value(raw.params).map(Self::SessionExited),
            "subscription.closed" => {
                serde_json::from_value(raw.params).map(Self::SubscriptionClosed)
            }
            other => return Err(format!("unknown notification {other:?}")),
        };
        typed.map_err(|err| err.to_string())
    }
}

impl From<Notification> for RawNotification {
    fn from(notification: Notification) -> Self {
        let (method, params) = match &notification {
            Notification::SessionExited(params) => ("session.exited", to_value(params)),
            Notification::SubscriptionClosed(params) => ("subscription.closed", to_value(params)),
        };
        Self {
            jsonrpc: JsonRpc,
            method: method.to_owned(),
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
}
