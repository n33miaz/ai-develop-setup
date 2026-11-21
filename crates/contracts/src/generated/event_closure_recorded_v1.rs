// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/event.closure.recorded.v1.schema.json. Não edite à mão.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_data_class_v1::CommonDataClassV1;
use super::common_id_v1::CommonIdV1;
use super::common_timestamp_v1::CommonTimestampV1;
use super::common_ulid_v1::CommonUlidV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static EVENT_CLOSURE_RECORDED_V1_RULES: &[(&str, Rule)] = &[(
    "#/allOf/1/properties/actor",
    Rule::Properties(&[(
        "actor",
        Rule::AllOf(&[
            Rule::Type(&[JsonType::Object]),
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("device")))]),
        ]),
    )]),
)];

/// Fechamento de pedido ilegível registrado. O dono fechou, pelo caminho de recuperação (approval.close), os pedidos de aprovação pendentes de uma ação cujo histórico o núcleo não lê: o aparelho (actor), o dono, o instante (ts), a decisão (deny ou expire), os pedidos fechados (approval_ids) e a chave de idempotência do comando, que deduplica o fechamento no log. Gravado no stream closure/<action_id> antes de qualquer efeito; cada pedido ganha depois approval.denied:2 ou approval.expired:2 com cause apontando para este registro. Nunca aprova e nunca emite licença.
///
/// `urn:ads:event:closure.recorded:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "EventClosureRecordedV1Fields")]
pub struct EventClosureRecordedV1(EventClosureRecordedV1Fields);

/// The fields of [`EventClosureRecordedV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventClosureRecordedV1Fields {
    pub id: CommonUlidV1,
    pub r#type: EventClosureRecordedV1Type,
    pub ts: CommonTimestampV1,
    pub stream: EventClosureRecordedV1Stream,
    pub seq: NonZeroU64,
    pub actor: CommonActorV1,
    #[serde(deserialize_with = "support::nullable")]
    pub cause: Option<CommonUlidV1>,
    pub data_class: CommonDataClassV1,
    pub data: EventClosureRecordedV1Data,
}

impl EventClosureRecordedV1 {
    pub fn new(fields: EventClosureRecordedV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "EventClosureRecordedV1",
            &fields,
            EVENT_CLOSURE_RECORDED_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> EventClosureRecordedV1Fields {
        self.0
    }
}

impl Deref for EventClosureRecordedV1 {
    type Target = EventClosureRecordedV1Fields;

    fn deref(&self) -> &EventClosureRecordedV1Fields {
        &self.0
    }
}

impl TryFrom<EventClosureRecordedV1Fields> for EventClosureRecordedV1 {
    type Error = ContractError;

    fn try_from(fields: EventClosureRecordedV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<EventClosureRecordedV1> for EventClosureRecordedV1Fields {
    fn from(value: EventClosureRecordedV1) -> EventClosureRecordedV1Fields {
        value.0
    }
}

impl Serialize for EventClosureRecordedV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `type` in `urn:ads:event:closure.recorded:1`.
///
/// Always `"urn:ads:event:closure.recorded:1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EventClosureRecordedV1Type;

impl EventClosureRecordedV1Type {
    pub const VALUE: &str = "urn:ads:event:closure.recorded:1";
}

impl Serialize for EventClosureRecordedV1Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for EventClosureRecordedV1Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

static EVENT_CLOSURE_RECORDED_V1_STREAM_PATTERN_1: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$");
static EVENT_CLOSURE_RECORDED_V1_STREAM_PATTERN_2: Pattern = Pattern::new(r"^closure/");

/// `stream` in `urn:ads:event:closure.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct EventClosureRecordedV1Stream(String);

impl EventClosureRecordedV1Stream {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("EventClosureRecordedV1Stream", text))
        }
    }

    fn accepts(text: &str) -> bool {
        EVENT_CLOSURE_RECORDED_V1_STREAM_PATTERN_1.matches(text)
            && EVENT_CLOSURE_RECORDED_V1_STREAM_PATTERN_2.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EventClosureRecordedV1Stream {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<EventClosureRecordedV1Stream> for String {
    fn from(value: EventClosureRecordedV1Stream) -> String {
        value.0
    }
}

impl fmt::Display for EventClosureRecordedV1Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for EventClosureRecordedV1Stream {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `data` in `urn:ads:event:closure.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventClosureRecordedV1Data {
    pub action_id: CommonIdV1,
    pub approval_ids: EventClosureRecordedV1DataApprovalIds,
    pub decision: EventClosureRecordedV1DataDecision,
    pub owner_id: CommonIdV1,
    pub via: EventClosureRecordedV1DataVia,
    pub idempotency_key: CommonUlidV1,
}

/// `data.approval_ids` in `urn:ads:event:closure.recorded:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<CommonIdV1>")]
pub struct EventClosureRecordedV1DataApprovalIds(Vec<CommonIdV1>);

impl EventClosureRecordedV1DataApprovalIds {
    pub const MAX_ITEMS: usize = 64;

    pub fn new(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        if (1..=Self::MAX_ITEMS).contains(&items.len()) && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "EventClosureRecordedV1DataApprovalIds",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[CommonIdV1] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<CommonIdV1> {
        self.0
    }
}

impl TryFrom<Vec<CommonIdV1>> for EventClosureRecordedV1DataApprovalIds {
    type Error = ContractError;

    fn try_from(items: Vec<CommonIdV1>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<EventClosureRecordedV1DataApprovalIds> for Vec<CommonIdV1> {
    fn from(value: EventClosureRecordedV1DataApprovalIds) -> Vec<CommonIdV1> {
        value.0
    }
}

impl Serialize for EventClosureRecordedV1DataApprovalIds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `data.decision` in `urn:ads:event:closure.recorded:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventClosureRecordedV1DataDecision {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "expire")]
    Expire,
}

impl EventClosureRecordedV1DataDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Expire => "expire",
        }
    }
}

/// `data.via` in `urn:ads:event:closure.recorded:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventClosureRecordedV1DataVia {
    #[serde(rename = "local_ui")]
    LocalUi,
}

impl EventClosureRecordedV1DataVia {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
        }
    }
}
