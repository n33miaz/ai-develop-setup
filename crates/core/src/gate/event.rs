//! Every contract type of the gate (`action.*` and `approval.*` events, the action plan record
//! and the common types they use), checked against the golden files. Hand-written and kept in
//! this one module on purpose: generating Rust types from the Schemas is a separate delivery,
//! and these leave when `ads-contracts` ships the generated ones.

use std::fmt;
use std::num::NonZeroU64;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use super::path::{AbsPath, TargetPath};

/// A value that does not fit the contract type it was meant for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    pub what: &'static str,
    pub value: String,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a valid {}: {:?}", self.what, self.value)
    }
}

macro_rules! text_type {
    ($(#[$doc:meta])* $name:ident, $what:literal, $check:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
                let text = text.into();
                let check: fn(&str) -> bool = $check;
                if check(&text) {
                    Ok(Self(text))
                } else {
                    Err(ContractError { what: $what, value: text })
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = ContractError;
            fn try_from(text: String) -> Result<Self, ContractError> {
                Self::new(text)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> String {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

text_type!(
    /// `urn:ads:common:id:1`.
    Id,
    "id",
    is_id
);
text_type!(
    /// `urn:ads:common:ulid:1`.
    Ulid,
    "ulid",
    |text| {
        let bytes = text.as_bytes();
        bytes.len() == 26
            && matches!(bytes[0], b'0'..=b'7')
            && bytes.iter().all(|b| {
                b.is_ascii_digit() || (b.is_ascii_uppercase() && !matches!(b, b'I' | b'L' | b'O' | b'U'))
            })
    }
);
text_type!(
    /// `urn:ads:common:timestamp:1`: UTC, `Z` suffix, up to 6 fraction digits.
    Timestamp,
    "timestamp",
    |text| timestamp_key(text).is_some()
);
text_type!(
    /// `urn:ads:common:sha256:1`, used as the hash of a plan.
    PlanHash,
    "sha256 digest",
    |text| {
        text.strip_prefix("sha256:").is_some_and(|hex| {
            hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
    }
);
text_type!(
    /// The `verb` of `action.planned`: dotted lower-case words.
    Verb,
    "verb",
    |text| {
        text.len() <= 64
            && text.split('.').all(|word| {
                word.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                    && word
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            })
    }
);
text_type!(
    /// The `error_code` of `action.failed`: `E_` and up to 60 of `A-Z 0-9 _`.
    ErrorCode,
    "error code",
    |text| {
        text.strip_prefix("E_").is_some_and(|rest| {
            (1..=60).contains(&rest.len())
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        })
    }
);
text_type!(
    /// Free text of 1 to 500 characters (`summary` and `reason`).
    ShortText,
    "text of 1 to 500 characters",
    |text| (1..=500).contains(&text.chars().count())
);
text_type!(
    /// `urn:ads:common:action-rule:1`: the stable code of a gate rule, with at most one detail
    /// after a dot. Open text, not a closed list: a new code does not change the contract.
    RuleCode,
    "action rule code",
    |text| {
        let word = |word: &str| {
            word.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        };
        text.len() <= 64 && text.split('.').count() <= 2 && text.split('.').all(word)
    }
);
text_type!(
    /// `urn:ads:common:abspath:2`: an absolute path exactly as the gate normalizes it. A path
    /// the normalization would refuse or rewrite is not one. Version 1 of the Schema is wider
    /// (Windows device names and short names pass it); this type refuses them all the same.
    PathText,
    "normalized absolute path",
    |text| {
        text.chars().count() <= 4096
            && AbsPath::parse(text).is_ok_and(|path| path.to_string() == text)
    }
);
text_type!(
    /// The `pattern` of a target in the action plan record: one name with a wildcard.
    PatternText,
    "wildcard pattern",
    |text| {
        text.chars().count() <= 255
            && text.contains(['*', '?'])
            && !text.contains(['\\', '/', '$', '%'])
    }
);
text_type!(
    /// The `subject` of a reason in the action plan record: the path a rule fired on.
    SubjectText,
    "text of 1 to 4096 characters",
    |text| (1..=4096).contains(&text.chars().count())
);

impl PlanHash {
    /// `sha256:` and the SHA-256 of the UTF-8 bytes of `text`, in lower-case hex.
    pub(crate) fn of(text: &str) -> Self {
        let digest = Sha256::digest(text.as_bytes());
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        Self(format!("sha256:{hex}"))
    }
}

fn is_id(text: &str) -> bool {
    let bytes = text.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

/// `(date and time to the second, microseconds)`; `None` when the text is not a timestamp.
/// Needed because the optional fraction makes plain text comparison wrong.
fn timestamp_key(text: &str) -> Option<(&str, u32)> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || !text.is_ascii() {
        return None;
    }
    let shape = b"dddd-dd-ddTdd:dd:dd";
    let shaped = bytes[..19].iter().zip(shape).all(|(b, s)| match s {
        b'd' => b.is_ascii_digit(),
        other => b == other,
    });
    if !shaped {
        return None;
    }
    let number = |from: usize, to: usize| text[from..to].parse::<u32>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    // A leap second exists only as the last second of a day.
    let real = (1..=days).contains(&day)
        && hour < 24
        && minute < 60
        && (second < 60 || (second == 60 && hour == 23 && minute == 59));
    if !real {
        return None;
    }
    let micros = match &text[19..] {
        "Z" => 0,
        tail => {
            let digits = tail.strip_prefix('.')?.strip_suffix('Z')?;
            if !(1..=6).contains(&digits.len()) || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            format!("{digits:0<6}").parse().ok()?
        }
    };
    Some((&text[..19], micros))
}

impl Timestamp {
    /// Compared as instants: `05Z` and `05.000Z` are the same one, and neither is before.
    pub fn is_before(&self, other: &Timestamp) -> bool {
        timestamp_key(&self.0) < timestamp_key(&other.0)
    }
}

/// `urn:ads:common:risk:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

/// The three levels of a verdict, from the mildest to the strictest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allowed,
    RequiresApproval,
    Denied,
}

impl Decision {
    /// One level of `urn:ads:common:risk:1` for each level of the verdict.
    pub fn risk(self) -> Risk {
        match self {
            Self::Allowed => Risk::Low,
            Self::RequiresApproval => Risk::Medium,
            Self::Denied => Risk::High,
        }
    }
}

/// `urn:ads:common:data-class:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataClass {
    Public,
    Internal,
    Private,
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorKind {
    Device,
    Agent,
    Daemon,
}

/// `urn:ads:common:actor:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub kind: ActorKind,
    pub id: Id,
}

/// The path an approval was granted through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    LocalUi,
    Tray,
    RemoteDevice,
}

/// What the core cannot produce by itself (no clock, no counter, no random source): the caller
/// supplies it for each event. `seq` counts within the stream of that event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMeta {
    pub id: Ulid,
    pub ts: Timestamp,
    pub seq: NonZeroU64,
    pub actor: Actor,
    pub cause: Option<Ulid>,
    pub data_class: DataClass,
}

/// The `data` of one event type.
pub trait EventData: Serialize + DeserializeOwned {
    /// The URN of the event.
    const TYPE: &'static str;
    /// The stream family, before the slash.
    const STREAM: &'static str;
    /// The entity the stream belongs to.
    fn stream_id(&self) -> &Id;
}

/// `urn:ads:event:envelope:1` fixed to one event type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// The bounds come from `EventData`; the ones serde would add repeat them and do not compile.
#[serde(try_from = "RawEnvelope<D>", bound = "")]
pub struct Envelope<D: EventData> {
    id: Ulid,
    #[serde(rename = "type")]
    kind: String,
    ts: Timestamp,
    stream: String,
    seq: NonZeroU64,
    actor: Actor,
    cause: Option<Ulid>,
    data_class: DataClass,
    data: D,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvelope<D> {
    id: Ulid,
    #[serde(rename = "type")]
    kind: String,
    ts: Timestamp,
    stream: String,
    seq: NonZeroU64,
    actor: Actor,
    #[serde(deserialize_with = "nullable")]
    cause: Option<Ulid>,
    data_class: DataClass,
    data: D,
}

/// A field that must be present and may be `null`.
fn nullable<'de, T, De>(deserializer: De) -> Result<Option<T>, De::Error>
where
    T: Deserialize<'de>,
    De: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer)
}

impl<D: EventData> TryFrom<RawEnvelope<D>> for Envelope<D> {
    type Error = ContractError;

    fn try_from(raw: RawEnvelope<D>) -> Result<Self, ContractError> {
        if raw.kind != D::TYPE {
            return Err(ContractError {
                what: "event type",
                value: raw.kind,
            });
        }
        // The stream names the entity `data` names: a rule of the contract, not of the Schema.
        if raw.stream != format!("{}/{}", D::STREAM, raw.data.stream_id()) {
            return Err(ContractError {
                what: "event stream",
                value: raw.stream,
            });
        }
        Ok(Self {
            id: raw.id,
            kind: raw.kind,
            ts: raw.ts,
            stream: raw.stream,
            seq: raw.seq,
            actor: raw.actor,
            cause: raw.cause,
            data_class: raw.data_class,
            data: raw.data,
        })
    }
}

impl<D: EventData> Envelope<D> {
    pub(crate) fn new(meta: EventMeta, data: D) -> Self {
        Self {
            id: meta.id,
            kind: D::TYPE.to_owned(),
            ts: meta.ts,
            stream: format!("{}/{}", D::STREAM, data.stream_id()),
            seq: meta.seq,
            actor: meta.actor,
            cause: meta.cause,
            data_class: meta.data_class,
            data,
        }
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    pub fn ts(&self) -> &Timestamp {
        &self.ts
    }

    pub fn data(&self) -> &D {
        &self.data
    }
}

macro_rules! event_data {
    ($name:ident, $urn:literal, $stream:literal, $id:ident) => {
        impl EventData for $name {
            const TYPE: &'static str = $urn;
            const STREAM: &'static str = $stream;
            fn stream_id(&self) -> &Id {
                &self.$id
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionPlanned {
    pub action_id: Id,
    pub verb: Verb,
    pub risk: Risk,
    pub plan_hash: PlanHash,
    pub needs_elevation: bool,
}
event_data!(
    ActionPlanned,
    "urn:ads:event:action.planned:1",
    "action",
    action_id
);

/// The `rules` of `action.denied`: 1 to 100 codes, each one once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<RuleCode>")]
pub struct RuleList(Vec<RuleCode>);

impl RuleList {
    pub fn new(rules: Vec<RuleCode>) -> Result<Self, ContractError> {
        let repeated = (1..rules.len()).any(|at| rules[..at].contains(&rules[at]));
        if rules.is_empty() || rules.len() > 100 || repeated {
            let codes: Vec<&str> = rules.iter().map(RuleCode::as_str).collect();
            return Err(ContractError {
                what: "list of 1 to 100 distinct rule codes",
                value: codes.join(","),
            });
        }
        Ok(Self(rules))
    }

    pub fn as_slice(&self) -> &[RuleCode] {
        &self.0
    }
}

impl TryFrom<Vec<RuleCode>> for RuleList {
    type Error = ContractError;
    fn try_from(rules: Vec<RuleCode>) -> Result<Self, ContractError> {
        Self::new(rules)
    }
}

/// The only event of a denied action: it comes in the place of `action.planned`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDenied {
    pub action_id: Id,
    pub verb: Verb,
    pub plan_hash: PlanHash,
    pub rules: RuleList,
}
event_data!(
    ActionDenied,
    "urn:ads:event:action.denied:1",
    "action",
    action_id
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionStarted {
    pub action_id: Id,
    pub plan_hash: PlanHash,
    #[serde(deserialize_with = "nullable")]
    pub approval_id: Option<Id>,
}
event_data!(
    ActionStarted,
    "urn:ads:event:action.started:1",
    "action",
    action_id
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionExecuted {
    pub action_id: Id,
    pub plan_hash: PlanHash,
    #[serde(deserialize_with = "nullable")]
    pub approval_id: Option<Id>,
}
event_data!(
    ActionExecuted,
    "urn:ads:event:action.executed:1",
    "action",
    action_id
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionFailed {
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub error_code: ErrorCode,
}
event_data!(
    ActionFailed,
    "urn:ads:event:action.failed:1",
    "action",
    action_id
);

/// `approval.requested:2`, the one the gate writes: it names the action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequested {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub summary: ShortText,
    pub expires_at: Timestamp,
    pub needs_elevation: bool,
}
event_data!(
    ApprovalRequested,
    "urn:ads:event:approval.requested:2",
    "approval",
    approval_id
);

/// `approval.requested:1`, which does not say which action it belongs to. Read only: the gate
/// never writes it and never rebuilds a state from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequestedV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub summary: ShortText,
    pub expires_at: Timestamp,
    pub needs_elevation: bool,
}
event_data!(
    ApprovalRequestedV1,
    "urn:ads:event:approval.requested:1",
    "approval",
    approval_id
);

/// `approval.granted:2`, the one the gate writes: it names the action the decision was taken
/// for, so the log keeps what the command checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalGranted {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub via: Via,
}
event_data!(
    ApprovalGranted,
    "urn:ads:event:approval.granted:2",
    "approval",
    approval_id
);

/// `approval.denied:2`, the one the gate writes: it names the action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDenied {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<ShortText>,
}
event_data!(
    ApprovalDenied,
    "urn:ads:event:approval.denied:2",
    "approval",
    approval_id
);

/// `approval.expired:2`, the one the gate writes: it names the action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalExpired {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
}
event_data!(
    ApprovalExpired,
    "urn:ads:event:approval.expired:2",
    "approval",
    approval_id
);

/// `approval.granted:1`, which does not say which action the decision was taken for. Read
/// only, like the two below: the gate never writes them and never rebuilds a state from them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalGrantedV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub via: Via,
}
event_data!(
    ApprovalGrantedV1,
    "urn:ads:event:approval.granted:1",
    "approval",
    approval_id
);

/// `approval.denied:1`. Read only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDeniedV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<ShortText>,
}
event_data!(
    ApprovalDeniedV1,
    "urn:ads:event:approval.denied:1",
    "approval",
    approval_id
);

/// `approval.expired:1`. Read only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalExpiredV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
}
event_data!(
    ApprovalExpiredV1,
    "urn:ads:event:approval.expired:1",
    "approval",
    approval_id
);

/// The one event a transition of the action state machine produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ActionPlanned(Envelope<ActionPlanned>),
    ActionDenied(Envelope<ActionDenied>),
    ApprovalRequested(Envelope<ApprovalRequested>),
    ApprovalGranted(Envelope<ApprovalGranted>),
    ApprovalDenied(Envelope<ApprovalDenied>),
    ApprovalExpired(Envelope<ApprovalExpired>),
    ActionStarted(Envelope<ActionStarted>),
    ActionExecuted(Envelope<ActionExecuted>),
    ActionFailed(Envelope<ActionFailed>),
}

macro_rules! each_event {
    ($event:expr, $envelope:ident => $body:expr) => {
        match $event {
            Event::ActionPlanned($envelope) => $body,
            Event::ActionDenied($envelope) => $body,
            Event::ApprovalRequested($envelope) => $body,
            Event::ApprovalGranted($envelope) => $body,
            Event::ApprovalDenied($envelope) => $body,
            Event::ApprovalExpired($envelope) => $body,
            Event::ActionStarted($envelope) => $body,
            Event::ActionExecuted($envelope) => $body,
            Event::ActionFailed($envelope) => $body,
        }
    };
}

impl Event {
    /// The event as the contract writes it.
    pub fn to_json(&self) -> serde_json::Value {
        each_event!(self, event => serde_json::to_value(event))
            .expect("event types hold only text, numbers and booleans")
    }

    /// Reads one event of the gate from a log, by its `type`. Anything else is refused,
    /// version 1 of the four approval events included: they do not name their action.
    pub fn from_json(json: serde_json::Value) -> Result<Self, ContractError> {
        fn read<D: EventData>(json: serde_json::Value) -> Result<Envelope<D>, ContractError> {
            serde_json::from_value(json).map_err(|err| ContractError {
                what: "event",
                value: err.to_string(),
            })
        }
        let kind = json["type"].as_str().unwrap_or_default().to_owned();
        match kind.as_str() {
            ActionPlanned::TYPE => read(json).map(Self::ActionPlanned),
            ActionDenied::TYPE => read(json).map(Self::ActionDenied),
            ApprovalRequested::TYPE => read(json).map(Self::ApprovalRequested),
            ApprovalGranted::TYPE => read(json).map(Self::ApprovalGranted),
            ApprovalDenied::TYPE => read(json).map(Self::ApprovalDenied),
            ApprovalExpired::TYPE => read(json).map(Self::ApprovalExpired),
            ActionStarted::TYPE => read(json).map(Self::ActionStarted),
            ActionExecuted::TYPE => read(json).map(Self::ActionExecuted),
            ActionFailed::TYPE => read(json).map(Self::ActionFailed),
            _ => Err(ContractError {
                what: "event type of the gate",
                value: kind,
            }),
        }
    }

    /// The URN of the event.
    pub fn kind(&self) -> &str {
        each_event!(self, event => &event.kind)
    }

    pub fn stream(&self) -> &str {
        each_event!(self, event => event.stream())
    }

    pub fn ts(&self) -> &Timestamp {
        each_event!(self, event => event.ts())
    }
}

/// The `operation` of the action plan record: what was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanOperation {
    #[serde(rename = "fs.delete")]
    Delete,
    #[serde(rename = "fs.delete_recursive")]
    DeleteRecursive,
    #[serde(rename = "fs.move")]
    Move,
    #[serde(rename = "fs.overwrite")]
    Overwrite,
    #[serde(rename = "fs.clean_untracked")]
    CleanUntracked,
}

impl PlanOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "fs.delete",
            Self::DeleteRecursive => "fs.delete_recursive",
            Self::Move => "fs.move",
            Self::Overwrite => "fs.overwrite",
            Self::CleanUntracked => "fs.clean_untracked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanTarget {
    pub path: PathText,
    #[serde(deserialize_with = "nullable")]
    pub pattern: Option<PatternText>,
    pub repo_root: bool,
}

impl PlanTarget {
    /// The target as one text: the path, or the path, a separator and the wildcard.
    fn text(&self) -> String {
        let path = self.path.as_str();
        let Some(pattern) = &self.pattern else {
            return path.to_owned();
        };
        let windows = !path.starts_with('/');
        let separator = match (path == "/" || (windows && path.len() == 3), windows) {
            (true, _) => "",
            (false, true) => "\\",
            (false, false) => "/",
        };
        format!("{path}{separator}{pattern}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanEstimate {
    pub items: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanReason {
    pub rule: RuleCode,
    pub subject: SubjectText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanVerdict {
    pub decision: Decision,
    pub risk: Risk,
    pub reasons: Vec<PlanReason>,
}

/// The fields of `urn:ads:record:action-plan`, versions 1 and 2 (the same fields; version 2
/// narrows the paths and bounds the estimate), before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionPlanFields {
    pub version: u64,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub operation: PlanOperation,
    pub work_root: PathText,
    #[serde(deserialize_with = "nullable")]
    pub destination: Option<PathText>,
    pub targets: Vec<PlanTarget>,
    #[serde(deserialize_with = "nullable")]
    pub estimate: Option<PlanEstimate>,
    pub verdict: PlanVerdict,
    pub quarantined: bool,
}

/// The largest count an estimate of the record holds from version 2 on: 2^53 - 1, the
/// largest integer every reader of the contract represents exactly (a JSON number in
/// JavaScript is a double). Version 1 has no such bound.
pub const ESTIMATE_MAX: u64 = (1 << 53) - 1;

impl ActionPlanFields {
    /// The rules between the fields that the Schema cannot express, for the record of
    /// `version`.
    fn checked(self, version: u64) -> Result<Self, ContractError> {
        let broken = |what: &'static str, value: String| Err(ContractError { what, value });
        if self.version != version {
            return broken("action plan version", self.version.to_string());
        }
        let count = self.targets.len();
        if !(1..=10_000).contains(&count) {
            return broken("count of 1 to 10000 targets", count.to_string());
        }
        let mut previous: Option<TargetPath> = None;
        for target in &self.targets {
            let text = target.text();
            let Ok(key) = TargetPath::parse(&text) else {
                return broken("target", text);
            };
            if previous.is_some_and(|previous| previous >= key) {
                return broken("target in canonical order, not repeated", text);
            }
            previous = Some(key);
        }
        // Version 1 put no bound on the estimate: what it wrote is still read, up to 64 bits.
        if version != 1
            && let Some(estimate) = self.estimate
            && (estimate.items > ESTIMATE_MAX || estimate.bytes > ESTIMATE_MAX)
        {
            return broken(
                "estimate of at most 2^53 - 1 items and bytes",
                format!("{}:{}", estimate.items, estimate.bytes),
            );
        }
        let verdict = &self.verdict;
        let coherent = verdict.decision.risk() == verdict.risk
            && verdict.reasons.len() <= 1000
            && (verdict.decision == Decision::Allowed) == verdict.reasons.is_empty();
        if !coherent {
            let reasons = verdict.reasons.len();
            return broken(
                "verdict with its own risk and reasons",
                format!(
                    "{:?}, {:?}, {reasons} reason(s)",
                    verdict.decision, verdict.risk
                ),
            );
        }
        let operation = self.operation.as_str().to_owned();
        let deletes = matches!(
            self.operation,
            PlanOperation::Delete | PlanOperation::DeleteRecursive
        );
        if self.quarantined && !(deletes && verdict.decision != Decision::Denied) {
            return broken("quarantine of a delete that was not denied", operation);
        }
        let needs_destination = self.quarantined || self.operation == PlanOperation::Move;
        if needs_destination != self.destination.is_some() {
            return broken("destination of a move or of a quarantine only", operation);
        }
        if self.plan_hash != PlanHash::of(&self.canonical()) {
            return broken("hash of the canonical plan", self.plan_hash.0);
        }
        Ok(self)
    }

    /// The text `plan_hash` is taken over, rebuilt from the fields alone. The same for every
    /// version of the record: the version is not part of the plan.
    fn canonical(&self) -> String {
        canonical_plan(
            if self.quarantined {
                PlanOperation::Move
            } else {
                self.operation
            },
            self.work_root.as_str(),
            self.destination.as_ref().map(PathText::as_str),
            self.estimate,
            self.targets
                .iter()
                .map(|target| (target.repo_root, target.text())),
        )
    }
}

/// `urn:ads:record:action-plan:2`, the one the gate writes: the plan as the gate judged it.
/// One exists only when it keeps every rule of the contract, the order of the targets, the
/// range of the estimate and the `plan_hash` included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ActionPlanFields", into = "ActionPlanFields")]
pub struct ActionPlanRecord(ActionPlanFields);

impl ActionPlanRecord {
    /// The `version` field of the record this gate writes.
    pub const VERSION: u64 = 2;
}

impl From<ActionPlanRecord> for ActionPlanFields {
    fn from(record: ActionPlanRecord) -> Self {
        record.0
    }
}

impl TryFrom<ActionPlanFields> for ActionPlanRecord {
    type Error = ContractError;

    fn try_from(fields: ActionPlanFields) -> Result<Self, ContractError> {
        fields.checked(Self::VERSION).map(Self)
    }
}

impl ActionPlanRecord {
    pub fn fields(&self) -> &ActionPlanFields {
        &self.0
    }

    /// The record as the contract writes it.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("record types hold only text, numbers and booleans")
    }

    /// The text `plan_hash` is taken over, rebuilt from the record alone.
    pub fn canonical(&self) -> String {
        self.0.canonical()
    }
}

/// `urn:ads:record:action-plan:1`. Read only: the gate writes version 2. It keeps reading
/// what version 1 could hold: an estimate up to 64 bits, with no bound of version 2. Its
/// Schema is still wider than this type in the paths (`abspath:1`) and above 64 bits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ActionPlanFields", into = "ActionPlanFields")]
pub struct ActionPlanRecordV1(ActionPlanFields);

impl From<ActionPlanRecordV1> for ActionPlanFields {
    fn from(record: ActionPlanRecordV1) -> Self {
        record.0
    }
}

impl TryFrom<ActionPlanFields> for ActionPlanRecordV1 {
    type Error = ContractError;

    fn try_from(fields: ActionPlanFields) -> Result<Self, ContractError> {
        fields.checked(1).map(Self)
    }
}

impl ActionPlanRecordV1 {
    pub fn fields(&self) -> &ActionPlanFields {
        &self.0
    }

    /// The text `plan_hash` is taken over, rebuilt from the record alone.
    pub fn canonical(&self) -> String {
        self.0.canonical()
    }
}

// The canonical serialization of a plan (`contracts/README.md`): one field per line, in a
/// fixed order, every path prefixed by its length in bytes so no path can pass for two fields.
pub(crate) fn canonical_plan(
    operation: PlanOperation,
    work_root: &str,
    destination: Option<&str>,
    estimate: Option<PlanEstimate>,
    targets: impl ExactSizeIterator<Item = (bool, String)>,
) -> String {
    let field = |text: &str| format!("{}:{text}", text.len());
    let mut out = String::from("ads.action-plan/1\n");
    out.push_str(&format!("op:{}\n", operation.as_str()));
    out.push_str(&format!("work_root:{}\n", field(work_root)));
    match destination {
        Some(destination) => out.push_str(&format!("destination:{}\n", field(destination))),
        None => out.push_str("destination:-\n"),
    }
    match estimate {
        Some(estimate) => {
            out.push_str(&format!("estimate:{}:{}\n", estimate.items, estimate.bytes));
        }
        None => out.push_str("estimate:unknown\n"),
    }
    out.push_str(&format!("targets:{}\n", targets.len()));
    for (repo_root, text) in targets {
        out.push_str(&format!(
            "target:{}:{}\n",
            u8::from(repo_root),
            field(&text)
        ));
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Granted,
    Denied,
    Expired,
}

/// The fields of `urn:ads:record:approval:1`, before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalFields {
    pub version: u64,
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub summary: ShortText,
    pub status: ApprovalStatus,
    pub requested_at: Timestamp,
    pub expires_at: Timestamp,
    #[serde(deserialize_with = "nullable")]
    pub decided_at: Option<Timestamp>,
    #[serde(deserialize_with = "nullable")]
    pub decided_by: Option<Actor>,
    #[serde(deserialize_with = "nullable")]
    pub via: Option<Via>,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<ShortText>,
}

/// `urn:ads:record:approval:1`: the state of an approval. Read only: the gate writes the
/// approval events, and whoever persists them keeps this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ApprovalFields", into = "ApprovalFields")]
pub struct ApprovalRecord(ApprovalFields);

impl From<ApprovalRecord> for ApprovalFields {
    fn from(record: ApprovalRecord) -> Self {
        record.0
    }
}

impl TryFrom<ApprovalFields> for ApprovalRecord {
    type Error = ContractError;

    fn try_from(fields: ApprovalFields) -> Result<Self, ContractError> {
        let decided = matches!(
            fields.status,
            ApprovalStatus::Granted | ApprovalStatus::Denied
        );
        let coherent = if decided {
            fields.decided_at.is_some() && fields.decided_by.is_some()
        } else {
            fields.decided_at.is_none() && fields.decided_by.is_none() && fields.via.is_none()
        };
        if fields.version != 1 || !coherent {
            return Err(ContractError {
                what: "approval with the decision its status asks for",
                value: format!("version {}, {:?}", fields.version, fields.status),
            });
        }
        Ok(Self(fields))
    }
}

impl ApprovalRecord {
    pub fn fields(&self) -> &ApprovalFields {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_types_follow_the_contract_patterns() {
        assert!(Id::new("act-01").is_ok());
        assert!(Id::new("a".repeat(64)).is_ok());
        for bad in ["", "-a", "a b", "a@b", &"a".repeat(65)] {
            assert!(Id::new(bad).is_err(), "{bad}");
        }
        assert!(Ulid::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").is_ok());
        for bad in [
            "81ARZ3NDEKTSV4RRFFQ69G5FAB",
            "01ARZ3NDEKTSV4RRFFQ69G5FAI",
            "01ARZ3",
        ] {
            assert!(Ulid::new(bad).is_err(), "{bad}");
        }
        assert!(PlanHash::new(format!("sha256:{}", "ab".repeat(32))).is_ok());
        for bad in [
            format!("sha256:{}", "AB".repeat(32)),
            format!("sha256:{}", "ab".repeat(31)),
            "ab".repeat(32),
        ] {
            assert!(PlanHash::new(bad.clone()).is_err(), "{bad}");
        }
        assert!(Verb::new("fs.delete_recursive").is_ok());
        for bad in ["", "Fs.delete", "fs..delete", "fs.", "apagar tudo", "fs.1x"] {
            assert!(Verb::new(bad).is_err(), "{bad}");
        }
        assert!(ErrorCode::new("E_PLAN_CHANGED").is_ok());
        for bad in ["E_", "PLAN_CHANGED", "E_plan", "plan changed"] {
            assert!(ErrorCode::new(bad).is_err(), "{bad}");
        }
        assert!(ShortText::new("á".repeat(500)).is_ok());
        assert!(ShortText::new("a".repeat(501)).is_err());
        assert!(ShortText::new("").is_err());
    }

    #[test]
    fn sha256_matches_a_known_vector() {
        assert_eq!(
            PlanHash::of("abc").as_str(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn rule_codes_paths_and_patterns_follow_the_contract_patterns() {
        for good in ["repository_root", "protected.user_home", "a1_b.c2"] {
            assert!(RuleCode::new(good).is_ok(), "{good}");
        }
        let long = "a".repeat(65);
        for bad in ["", "Protected", "a.b.c", "a.", ".a", "1a", "a b", &long] {
            assert!(RuleCode::new(bad).is_err(), "{bad}");
        }
        for good in ["C:\\", "/", "C:\\work\\My Files\\a.b", "/work/proj"] {
            assert!(PathText::new(good).is_ok(), "{good}");
        }
        let long = format!("/{}", "a".repeat(4096));
        for bad in [
            "c:\\work",
            "C:/work",
            "C:\\work\\",
            "/work/",
            "/work//proj",
            "/work/./proj",
            "/work/../proj",
            "/work/*",
            "work",
            "\\\\server\\share",
            "C:\\PROGRA~1",
            long.as_str(),
        ] {
            assert!(PathText::new(bad).is_err(), "{bad}");
        }
        assert!(PathText::new(&long[..4096]).is_ok());
        for good in ["*", "*.tmp", "a?c"] {
            assert!(PatternText::new(good).is_ok(), "{good}");
        }
        let long = "*".repeat(256);
        for bad in ["", "name", "a/*", "a\\*", "$*", "%*", &long] {
            assert!(PatternText::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rule_list_has_one_to_a_hundred_distinct_codes() {
        let code = |text: &str| RuleCode::new(text).unwrap();
        assert!(RuleList::new(vec![code("a"), code("b")]).is_ok());
        assert!(RuleList::new(vec![]).is_err());
        assert!(RuleList::new(vec![code("a"), code("b"), code("a")]).is_err());
        let many = |count: usize| (0..count).map(|n| code(&format!("r{n}"))).collect();
        assert!(RuleList::new(many(100)).is_ok());
        assert!(RuleList::new(many(101)).is_err());
    }

    #[test]
    fn timestamps_are_validated_and_compared_as_instants() {
        let at = |text: &str| Timestamp::new(text).unwrap();
        for bad in [
            "2030-02-31T03:04:05Z",
            "2031-02-29T03:04:05Z",
            "2030-01-02T03:04:60Z",
            "2030-01-02T03:04:05",
            "2030-01-02 03:04:05Z",
            "2030-13-02T03:04:05Z",
            "2030-01-02T24:04:05Z",
            "2030-01-02T03:04:05.Z",
            "2030-01-02T03:04:05.1234567Z",
            "2030-01-02T03:04:05+00:00",
        ] {
            assert!(Timestamp::new(bad).is_err(), "{bad}");
        }
        // As plain text "05Z" sorts after "05.5Z"; as instants it comes first.
        assert!(at("2030-01-02T03:04:05Z").is_before(&at("2030-01-02T03:04:05.5Z")));
        assert!(!at("2030-01-02T03:04:05.5Z").is_before(&at("2030-01-02T03:04:05Z")));
        assert!(at("2030-01-02T03:04:05.05Z").is_before(&at("2030-01-02T03:04:05.5Z")));
        assert!(!at("2030-01-02T03:04:05Z").is_before(&at("2030-01-02T03:04:05.000Z")));
        assert!(at("2030-01-02T03:04:05.999999Z").is_before(&at("2030-01-02T03:04:06Z")));
    }
}
