// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.update-status.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static RECORD_UPDATE_STATUS_V1_RULES: &[(&str, Rule)] = &[
    (
        "#/allOf/0",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[(
                    "state",
                    Rule::Enum(&[
                        Literal::Str("downloading"),
                        Literal::Str("verifying"),
                        Literal::Str("switching"),
                    ]),
                )]),
                Rule::Required(&["state"]),
            ]),
            then: Some(&Rule::Properties(&[(
                "candidate",
                Rule::Type(&[JsonType::Object]),
            )])),
            otherwise: None,
        },
    ),
    (
        "#/allOf/1",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("state", Rule::Const(Literal::Str("probation")))]),
                Rule::Required(&["state"]),
            ]),
            then: Some(&Rule::Properties(&[(
                "probation_until",
                Rule::Type(&[JsonType::String]),
            )])),
            otherwise: Some(&Rule::Properties(&[(
                "probation_until",
                Rule::Type(&[JsonType::Null]),
            )])),
        },
    ),
    (
        "#/allOf/2",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("state", Rule::Const(Literal::Str("failed")))]),
                Rule::Required(&["state"]),
            ]),
            then: Some(&Rule::Properties(&[(
                "last_error",
                Rule::Type(&[JsonType::String]),
            )])),
            otherwise: None,
        },
    ),
];

/// Estado da atualização. Estado visível da auto-atualização, escrito pelo lançador: o que está rodando, a versão anterior guardada, a candidata em andamento, a prova depois da troca, as quedas contadas e as versões bloqueadas. Baixando, conferindo e trocando têm candidata; em prova há prazo; falha traz código de erro.
///
/// `urn:ads:record:update-status:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordUpdateStatusV1Fields")]
pub struct RecordUpdateStatusV1(RecordUpdateStatusV1Fields);

/// The fields of [`RecordUpdateStatusV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordUpdateStatusV1Fields {
    pub version: RecordUpdateStatusV1Version,
    pub state: RecordUpdateStatusV1State,
    pub current: RecordUpdateStatusV1BuildRef,
    #[serde(deserialize_with = "support::nullable")]
    pub previous: Option<RecordUpdateStatusV1BuildRef>,
    #[serde(deserialize_with = "support::nullable")]
    pub candidate: Option<RecordUpdateStatusV1BuildRef>,
    #[serde(deserialize_with = "support::nullable")]
    pub probation_until: Option<CommonTimestampV1>,
    pub crashes: u64,
    pub blocked: RecordUpdateStatusV1Blocked,
    #[serde(deserialize_with = "support::nullable")]
    pub last_error: Option<RecordUpdateStatusV1LastError>,
    pub updated_at: CommonTimestampV1,
}

impl RecordUpdateStatusV1 {
    pub fn new(fields: RecordUpdateStatusV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "RecordUpdateStatusV1",
            &fields,
            RECORD_UPDATE_STATUS_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordUpdateStatusV1Fields {
        self.0
    }
}

impl Deref for RecordUpdateStatusV1 {
    type Target = RecordUpdateStatusV1Fields;

    fn deref(&self) -> &RecordUpdateStatusV1Fields {
        &self.0
    }
}

impl TryFrom<RecordUpdateStatusV1Fields> for RecordUpdateStatusV1 {
    type Error = ContractError;

    fn try_from(fields: RecordUpdateStatusV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordUpdateStatusV1> for RecordUpdateStatusV1Fields {
    fn from(value: RecordUpdateStatusV1) -> RecordUpdateStatusV1Fields {
        value.0
    }
}

impl Serialize for RecordUpdateStatusV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:update-status:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordUpdateStatusV1Version;

impl RecordUpdateStatusV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordUpdateStatusV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordUpdateStatusV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `state` in `urn:ads:record:update-status:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordUpdateStatusV1State {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "downloading")]
    Downloading,
    #[serde(rename = "verifying")]
    Verifying,
    #[serde(rename = "switching")]
    Switching,
    #[serde(rename = "probation")]
    Probation,
    #[serde(rename = "rolled_back")]
    RolledBack,
    #[serde(rename = "failed")]
    Failed,
}

impl RecordUpdateStatusV1State {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Switching => "switching",
            Self::Probation => "probation",
            Self::RolledBack => "rolled_back",
            Self::Failed => "failed",
        }
    }
}

/// `$defs.build_ref` in `urn:ads:record:update-status:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordUpdateStatusV1BuildRef {
    pub build: RecordUpdateStatusV1BuildRefBuild,
    pub commit: RecordUpdateStatusV1BuildRefCommit,
}

static RECORD_UPDATE_STATUS_V1_BUILD_REF_BUILD_PATTERN: Pattern =
    Pattern::new(r"^dev-[0-9]{8}-[0-9a-f]{7,12}$");

/// `$defs.build_ref.build` in `urn:ads:record:update-status:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUpdateStatusV1BuildRefBuild(String);

impl RecordUpdateStatusV1BuildRefBuild {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordUpdateStatusV1BuildRefBuild",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_UPDATE_STATUS_V1_BUILD_REF_BUILD_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUpdateStatusV1BuildRefBuild {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUpdateStatusV1BuildRefBuild> for String {
    fn from(value: RecordUpdateStatusV1BuildRefBuild) -> String {
        value.0
    }
}

impl fmt::Display for RecordUpdateStatusV1BuildRefBuild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUpdateStatusV1BuildRefBuild {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static RECORD_UPDATE_STATUS_V1_BUILD_REF_COMMIT_PATTERN: Pattern = Pattern::new(r"^[0-9a-f]{40}$");

/// `$defs.build_ref.commit` in `urn:ads:record:update-status:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUpdateStatusV1BuildRefCommit(String);

impl RecordUpdateStatusV1BuildRefCommit {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordUpdateStatusV1BuildRefCommit",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_UPDATE_STATUS_V1_BUILD_REF_COMMIT_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUpdateStatusV1BuildRefCommit {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUpdateStatusV1BuildRefCommit> for String {
    fn from(value: RecordUpdateStatusV1BuildRefCommit) -> String {
        value.0
    }
}

impl fmt::Display for RecordUpdateStatusV1BuildRefCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUpdateStatusV1BuildRefCommit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `blocked` in `urn:ads:record:update-status:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordUpdateStatusV1BlockedItem>")]
pub struct RecordUpdateStatusV1Blocked(Vec<RecordUpdateStatusV1BlockedItem>);

impl RecordUpdateStatusV1Blocked {
    pub fn new(items: Vec<RecordUpdateStatusV1BlockedItem>) -> Result<Self, ContractError> {
        if support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordUpdateStatusV1Blocked",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordUpdateStatusV1BlockedItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordUpdateStatusV1BlockedItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordUpdateStatusV1BlockedItem>> for RecordUpdateStatusV1Blocked {
    type Error = ContractError;

    fn try_from(items: Vec<RecordUpdateStatusV1BlockedItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordUpdateStatusV1Blocked> for Vec<RecordUpdateStatusV1BlockedItem> {
    fn from(value: RecordUpdateStatusV1Blocked) -> Vec<RecordUpdateStatusV1BlockedItem> {
        value.0
    }
}

impl Serialize for RecordUpdateStatusV1Blocked {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static RECORD_UPDATE_STATUS_V1_BLOCKED_ITEM_PATTERN: Pattern =
    Pattern::new(r"^dev-[0-9]{8}-[0-9a-f]{7,12}$");

/// `blocked[]` in `urn:ads:record:update-status:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUpdateStatusV1BlockedItem(String);

impl RecordUpdateStatusV1BlockedItem {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUpdateStatusV1BlockedItem", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_UPDATE_STATUS_V1_BLOCKED_ITEM_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUpdateStatusV1BlockedItem {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUpdateStatusV1BlockedItem> for String {
    fn from(value: RecordUpdateStatusV1BlockedItem) -> String {
        value.0
    }
}

impl fmt::Display for RecordUpdateStatusV1BlockedItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUpdateStatusV1BlockedItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static RECORD_UPDATE_STATUS_V1_LAST_ERROR_PATTERN: Pattern = Pattern::new(r"^E_[A-Z0-9_]{1,60}$");

/// `last_error` in `urn:ads:record:update-status:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUpdateStatusV1LastError(String);

impl RecordUpdateStatusV1LastError {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUpdateStatusV1LastError", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_UPDATE_STATUS_V1_LAST_ERROR_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUpdateStatusV1LastError {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUpdateStatusV1LastError> for String {
    fn from(value: RecordUpdateStatusV1LastError) -> String {
        value.0
    }
}

impl fmt::Display for RecordUpdateStatusV1LastError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUpdateStatusV1LastError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
