//! Gate domain values and explicit conversions to generated wire contracts.

use std::fmt;
use std::num::NonZeroU64;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use super::path::{AbsPath, TargetPath};
use crate::digest::sha256_text;

#[path = "conversion.rs"]
mod conversion;

use ads_contracts as wire;
pub use ads_contracts::ContractError;

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
                    Err(ContractError::new($what, text))
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

pub use wire::CommonIdV1 as Id;
pub use wire::CommonUlidV1 as Ulid;
text_type!(
    /// `urn:ads:common:timestamp:1`: UTC, `Z` suffix, up to 6 fraction digits.
    Timestamp,
    "timestamp",
    |text| wire::CommonTimestampV1::new(text).is_ok()
);
text_type!(
    /// `urn:ads:common:sha256:1`, used as the hash of a plan.
    PlanHash,
    "sha256 digest",
    |text| wire::CommonSha256V1::new(text).is_ok()
);
pub use wire::CommonActionRuleV1 as RuleCode;
pub use wire::EventActionFailedV1DataErrorCode as ErrorCode;
pub use wire::EventActionPlannedV1DataVerb as Verb;
pub use wire::EventApprovalRequestedV2DataSummary as ShortText;
text_type!(
    /// `urn:ads:common:abspath:2`: an absolute path exactly as the gate normalizes it. A path
    /// the normalization would refuse or rewrite is not one. Version 1 of the Schema is wider
    /// (Windows device names and short names pass it); this type refuses them all the same.
    PathText,
    "normalized absolute path",
    |text| {
        AbsPath::parse(text).is_ok_and(|path| path.to_string() == text)
    }
);
pub use wire::RecordActionPlanV2TargetsItemPattern as PatternText;
pub use wire::RecordActionPlanV2VerdictReasonsItemSubject as SubjectText;

impl PlanHash {
    /// `sha256:` and the SHA-256 of the UTF-8 bytes of `text`, in lower-case hex.
    pub(crate) fn of(text: &str) -> Self {
        Self(sha256_text(text.as_bytes()))
    }
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
    pub(super) fn precedes_by_more_than(&self, other: &Self, seconds: u64) -> bool {
        let (earlier, earlier_micros) = timestamp_key(&self.0).expect("validated timestamp");
        let (later, later_micros) = timestamp_key(&other.0).expect("validated timestamp");
        if (earlier, earlier_micros) >= (later, later_micros) {
            return false;
        }
        if seconds == 0 {
            return true;
        }

        let epoch_seconds =
            |text| crate::timestamp::epoch_seconds(text).expect("validated timestamp");
        let elapsed = epoch_seconds(later) - epoch_seconds(earlier);
        // The civil conversion maps 23:59:60 to midnight. Include that second when
        // measuring from an explicit leap second to a stamp on a subsequent day.
        let leap_second = i64::from(earlier.ends_with(":60") && earlier[..10] != later[..10]);
        let elapsed_micros = (elapsed + leap_second) * 1_000_000 + i64::from(later_micros)
            - i64::from(earlier_micros);
        i128::from(elapsed_micros) > i128::from(seconds) * 1_000_000
    }

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
pub use wire::CommonDataClassV1 as DataClass;

pub use wire::CommonActorV1Kind as ActorKind;

/// `urn:ads:common:actor:1`.
pub use wire::CommonActorV1 as Actor;

/// The path an approval was granted through.
pub use wire::EventApprovalGrantedV2DataVia as Via;

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
pub trait EventData {
    type Wire: Serialize + DeserializeOwned;
    fn from_wire(wire: Self::Wire) -> Result<Envelope<Self>, ContractError>
    where
        Self: Sized;
    fn to_wire(envelope: &Envelope<Self>) -> Self::Wire
    where
        Self: Sized;
    /// The URN of the event.
    const TYPE: &'static str;
    /// The stream family, before the slash.
    const STREAM: &'static str;
    /// The entity the stream belongs to.
    fn stream_id(&self) -> &Id;
}

/// `urn:ads:event:envelope:1` fixed to one event type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope<D: EventData> {
    id: Ulid,
    kind: String,
    ts: Timestamp,
    stream: String,
    seq: NonZeroU64,
    actor: Actor,
    cause: Option<Ulid>,
    data_class: DataClass,
    data: D,
}

impl<'de, D: EventData> Deserialize<'de> for Envelope<D> {
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        D::from_wire(D::Wire::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl<D: EventData> Serialize for Envelope<D> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        D::to_wire(self).serialize(serializer)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlanned {
    pub action_id: Id,
    pub verb: Verb,
    pub risk: Risk,
    pub plan_hash: PlanHash,
    pub needs_elevation: bool,
}

/// The `rules` of `action.denied`: 1 to 100 codes, each one once.
pub use wire::EventActionDeniedV1DataRules as RuleList;

/// The only event of a denied action: it comes in the place of `action.planned`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDenied {
    pub action_id: Id,
    pub verb: Verb,
    pub plan_hash: PlanHash,
    pub rules: RuleList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionStarted {
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub approval_id: Option<Id>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionExecuted {
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub approval_id: Option<Id>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFailed {
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub error_code: ErrorCode,
}

/// `approval.requested:2`, the one the gate writes: it names the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequested {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub summary: ShortText,
    pub expires_at: Timestamp,
    pub needs_elevation: bool,
}

/// `approval.requested:1`, which does not say which action it belongs to. Read only: the gate
/// never writes it and never rebuilds a state from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequestedV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub summary: ShortText,
    pub expires_at: Timestamp,
    pub needs_elevation: bool,
}

/// `approval.granted:2`, the one the gate writes: it names the action the decision was taken
/// for, so the log keeps what the command checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalGranted {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub via: Via,
}

/// `approval.denied:2`, the one the gate writes: it names the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDenied {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub reason: Option<ShortText>,
}

/// `approval.expired:2`, the one the gate writes: it names the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalExpired {
    pub approval_id: Id,
    pub action_id: Id,
    pub plan_hash: PlanHash,
}

/// `approval.granted:1`, which does not say which action the decision was taken for. Read
/// only, like the two below: the gate never writes them and never rebuilds a state from them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalGrantedV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub via: Via,
}

/// `approval.denied:1`. Read only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDeniedV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub reason: Option<ShortText>,
}

/// `approval.expired:1`. Read only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalExpiredV1 {
    pub approval_id: Id,
    pub plan_hash: PlanHash,
}

/// The one event a transition of the action state machine produces.
/// Reading a document cannot create an action or an execution permit.
///
/// ```compile_fail,E0277
/// let _: ads_core::gate::Action = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail,E0277
/// let _: ads_core::gate::ExecutionPermit = serde_json::from_str("{}").unwrap();
/// ```
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
            serde_json::from_value(json).map_err(|err| ContractError::new("event", err.to_string()))
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
            _ => Err(ContractError::new("event type of the gate", kind)),
        }
    }

    pub(super) fn meta(&self) -> EventMeta {
        each_event!(self, event => EventMeta {
            id: event.id.clone(),
            ts: event.ts.clone(),
            seq: event.seq,
            actor: event.actor.clone(),
            cause: event.cause.clone(),
            data_class: event.data_class,
        })
    }

    pub(super) fn id(&self) -> &Ulid {
        each_event!(self, event => &event.id)
    }

    pub(super) fn seq(&self) -> NonZeroU64 {
        each_event!(self, event => event.seq)
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
pub use wire::RecordActionPlanV2Operation as PlanOperation;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanTarget {
    pub path: PathText,
    pub pattern: Option<PatternText>,
    pub repo_root: bool,
}

impl PlanTarget {
    /// The target as one text: the path, or the path, a separator and the wildcard.
    pub fn text(&self) -> String {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlanEstimate {
    pub items: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanReason {
    pub rule: RuleCode,
    pub subject: SubjectText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanVerdict {
    pub decision: Decision,
    pub risk: Risk,
    pub reasons: Vec<PlanReason>,
}

/// The fields of `urn:ads:record:action-plan`, versions 1 and 2 (the same fields; version 2
/// narrows the paths and bounds the estimate), before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlanFields {
    pub version: u64,
    pub action_id: Id,
    pub plan_hash: PlanHash,
    pub operation: PlanOperation,
    pub work_root: PathText,
    pub destination: Option<PathText>,
    pub targets: Vec<PlanTarget>,
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
        let broken = |what: &'static str, value: String| Err(ContractError::new(what, value));
        if self.version != version {
            return broken("action plan version", self.version.to_string());
        }
        let count = self.targets.len();
        if !(1..=wire::RecordActionPlanV2Targets::MAX_ITEMS).contains(&count) {
            return broken("count of targets the record holds", count.to_string());
        }
        let mut previous: Option<TargetPath> = None;
        for target in &self.targets {
            let text = target.text();
            let Ok(key) = TargetPath::parse_record_target(&text) else {
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
            && verdict.reasons.len() <= wire::RecordActionPlanV2VerdictReasons::MAX_ITEMS
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
#[serde(
    try_from = "wire::RecordActionPlanV2",
    into = "wire::RecordActionPlanV2"
)]
pub struct ActionPlanRecord(ActionPlanFields);

impl ActionPlanRecord {
    /// The `version` field of the record this gate writes.
    pub const VERSION: u64 = wire::RecordActionPlanV2Version::VALUE as u64;
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
#[serde(
    try_from = "wire::RecordActionPlanV1",
    into = "wire::RecordActionPlanV1"
)]
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

pub use wire::RecordApprovalV1Status as ApprovalStatus;

/// The fields of `urn:ads:record:approval:1`, before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalFields {
    pub version: u64,
    pub approval_id: Id,
    pub plan_hash: PlanHash,
    pub risk: Risk,
    pub summary: ShortText,
    pub status: ApprovalStatus,
    pub requested_at: Timestamp,
    pub expires_at: Timestamp,
    pub decided_at: Option<Timestamp>,
    pub decided_by: Option<Actor>,
    pub via: Option<Via>,
    pub reason: Option<ShortText>,
}

/// `urn:ads:record:approval:1`: the state of an approval. Read only: the gate writes the
/// approval events, and whoever persists them keeps this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "wire::RecordApprovalV1", into = "wire::RecordApprovalV1")]
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
            return Err(ContractError::new(
                "approval with the decision its status asks for",
                format!("version {}, {:?}", fields.version, fields.status),
            ));
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
mod tests;
