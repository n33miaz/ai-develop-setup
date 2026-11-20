// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.approval.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_actor_v1::CommonActorV1;
use super::common_id_v1::CommonIdV1;
use super::common_risk_v1::CommonRiskV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Rule};

static RECORD_APPROVAL_V1_RULES: &[(&str, Rule)] = &[(
    "#/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[(
                "status",
                Rule::Enum(&[Literal::Str("granted"), Literal::Str("denied")]),
            )]),
            Rule::Required(&["status"]),
        ]),
        then: Some(&Rule::Properties(&[
            ("decided_at", Rule::Type(&[JsonType::String])),
            ("decided_by", Rule::Type(&[JsonType::Object])),
        ])),
        otherwise: Some(&Rule::Properties(&[
            ("decided_at", Rule::Type(&[JsonType::Null])),
            ("decided_by", Rule::Type(&[JsonType::Null])),
            ("via", Rule::Type(&[JsonType::Null])),
        ])),
    },
)];

/// Aprovação. Estado de uma aprovação: vale para o hash de um plano e expira. Pendente e expirada não têm decisor; concedida e negada têm autor e instante da decisão.
///
/// `urn:ads:record:approval:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordApprovalV1Fields")]
pub struct RecordApprovalV1(RecordApprovalV1Fields);

/// The fields of [`RecordApprovalV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordApprovalV1Fields {
    pub version: RecordApprovalV1Version,
    pub approval_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub risk: CommonRiskV1,
    pub summary: RecordApprovalV1Summary,
    pub status: RecordApprovalV1Status,
    pub requested_at: CommonTimestampV1,
    pub expires_at: CommonTimestampV1,
    #[serde(deserialize_with = "support::nullable")]
    pub decided_at: Option<CommonTimestampV1>,
    #[serde(deserialize_with = "support::nullable")]
    pub decided_by: Option<CommonActorV1>,
    #[serde(deserialize_with = "support::nullable")]
    pub via: Option<RecordApprovalV1Via>,
    #[serde(deserialize_with = "support::nullable")]
    pub reason: Option<RecordApprovalV1Reason>,
}

impl RecordApprovalV1 {
    pub fn new(fields: RecordApprovalV1Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordApprovalV1", &fields, RECORD_APPROVAL_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordApprovalV1Fields {
        self.0
    }
}

impl Deref for RecordApprovalV1 {
    type Target = RecordApprovalV1Fields;

    fn deref(&self) -> &RecordApprovalV1Fields {
        &self.0
    }
}

impl TryFrom<RecordApprovalV1Fields> for RecordApprovalV1 {
    type Error = ContractError;

    fn try_from(fields: RecordApprovalV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordApprovalV1> for RecordApprovalV1Fields {
    fn from(value: RecordApprovalV1) -> RecordApprovalV1Fields {
        value.0
    }
}

impl Serialize for RecordApprovalV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:approval:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordApprovalV1Version;

impl RecordApprovalV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordApprovalV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordApprovalV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `summary` in `urn:ads:record:approval:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApprovalV1Summary(String);

impl RecordApprovalV1Summary {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApprovalV1Summary", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordApprovalV1Summary {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApprovalV1Summary> for String {
    fn from(value: RecordApprovalV1Summary) -> String {
        value.0
    }
}

impl fmt::Display for RecordApprovalV1Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApprovalV1Summary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `status` in `urn:ads:record:approval:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordApprovalV1Status {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "granted")]
    Granted,
    #[serde(rename = "denied")]
    Denied,
    #[serde(rename = "expired")]
    Expired,
}

impl RecordApprovalV1Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Expired => "expired",
        }
    }
}

/// `via` in `urn:ads:record:approval:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordApprovalV1Via {
    #[serde(rename = "local_ui")]
    LocalUi,
    #[serde(rename = "tray")]
    Tray,
    #[serde(rename = "remote_device")]
    RemoteDevice,
}

impl RecordApprovalV1Via {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalUi => "local_ui",
            Self::Tray => "tray",
            Self::RemoteDevice => "remote_device",
        }
    }
}

/// `reason` in `urn:ads:record:approval:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordApprovalV1Reason(String);

impl RecordApprovalV1Reason {
    pub const MAX_CHARS: usize = 500;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordApprovalV1Reason", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordApprovalV1Reason {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordApprovalV1Reason> for String {
    fn from(value: RecordApprovalV1Reason) -> String {
        value.0
    }
}

impl fmt::Display for RecordApprovalV1Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordApprovalV1Reason {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
