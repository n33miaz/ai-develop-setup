//! Hand-written stand-ins for the `action.*` and `approval.*` event contracts, checked against
//! the golden files. Provisional: they leave when `ads-contracts` ships types generated from
//! the Schemas.

use std::fmt;
use std::num::NonZeroU64;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

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
    let in_range = (1..=12).contains(&number(5, 7)?)
        && (1..=31).contains(&number(8, 10)?)
        && number(11, 13)? < 24
        && number(14, 16)? < 60
        && number(17, 19)? <= 60;
    if !in_range {
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
        let entity = raw
            .stream
            .strip_prefix(D::STREAM)
            .and_then(|rest| rest.strip_prefix('/'));
        if !entity.is_some_and(is_id) {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequested {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub summary: ShortText,
    pub expires_at: Timestamp,
    pub needs_elevation: bool,
}
event_data!(
    ApprovalRequested,
    "urn:ads:event:approval.requested:1",
    "approval",
    approval_id
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalGranted {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub via: Via,
}
event_data!(
    ApprovalGranted,
    "urn:ads:event:approval.granted:1",
    "approval",
    approval_id
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDenied {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<ShortText>,
}
event_data!(
    ApprovalDenied,
    "urn:ads:event:approval.denied:1",
    "approval",
    approval_id
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalExpired {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
}
event_data!(
    ApprovalExpired,
    "urn:ads:event:approval.expired:1",
    "approval",
    approval_id
);

/// The one event a transition of the action state machine produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ActionPlanned(Envelope<ActionPlanned>),
    ApprovalRequested(Envelope<ApprovalRequested>),
    ApprovalGranted(Envelope<ApprovalGranted>),
    ApprovalDenied(Envelope<ApprovalDenied>),
    ApprovalExpired(Envelope<ApprovalExpired>),
    ActionExecuted(Envelope<ActionExecuted>),
    ActionFailed(Envelope<ActionFailed>),
}

impl Event {
    /// The event as the contract writes it.
    pub fn to_json(&self) -> serde_json::Value {
        let json = match self {
            Self::ActionPlanned(event) => serde_json::to_value(event),
            Self::ApprovalRequested(event) => serde_json::to_value(event),
            Self::ApprovalGranted(event) => serde_json::to_value(event),
            Self::ApprovalDenied(event) => serde_json::to_value(event),
            Self::ApprovalExpired(event) => serde_json::to_value(event),
            Self::ActionExecuted(event) => serde_json::to_value(event),
            Self::ActionFailed(event) => serde_json::to_value(event),
        };
        json.expect("event types hold only text, numbers and booleans")
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
    fn timestamps_are_validated_and_compared_as_instants() {
        let at = |text: &str| Timestamp::new(text).unwrap();
        for bad in [
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
