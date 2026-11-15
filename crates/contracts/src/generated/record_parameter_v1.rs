// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.parameter.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;

use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static RECORD_PARAMETER_V1_RULES: &[(&str, Rule)] = &[
    (
        "#/allOf/0",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("type", Rule::Const(Literal::Str("string")))]),
                Rule::Required(&["type"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("default", Rule::Type(&[JsonType::String])),
                ("value", Rule::Type(&[JsonType::String])),
            ])),
            otherwise: Some(&Rule::Not(&Rule::Required(&["choices"]))),
        },
    ),
    (
        "#/allOf/1",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("type", Rule::Const(Literal::Str("integer")))]),
                Rule::Required(&["type"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("default", Rule::Type(&[JsonType::Integer])),
                ("value", Rule::Type(&[JsonType::Integer])),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/allOf/2",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("type", Rule::Const(Literal::Str("number")))]),
                Rule::Required(&["type"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("default", Rule::Type(&[JsonType::Number])),
                ("value", Rule::Type(&[JsonType::Number])),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/allOf/3",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("type", Rule::Const(Literal::Str("boolean")))]),
                Rule::Required(&["type"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("default", Rule::Type(&[JsonType::Boolean])),
                ("value", Rule::Type(&[JsonType::Boolean])),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/allOf/4",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("type", Rule::Const(Literal::Str("string_list")))]),
                Rule::Required(&["type"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("default", Rule::Type(&[JsonType::Array])),
                ("value", Rule::Type(&[JsonType::Array])),
            ])),
            otherwise: None,
        },
    ),
];

/// Parâmetro. Um parâmetro do produto como o núcleo o apresenta: a chave, o tipo, o padrão recomendado, o valor em vigor e a explicação. Todo comportamento com mais de uma escolha razoável é um registro destes. Parâmetro nunca guarda segredo: segredo fica no cofre.
///
/// `urn:ads:record:parameter:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordParameterV1Fields")]
pub struct RecordParameterV1(RecordParameterV1Fields);

/// The fields of [`RecordParameterV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordParameterV1Fields {
    pub version: RecordParameterV1Version,
    pub key: RecordParameterV1Key,
    pub r#type: RecordParameterV1Type,
    pub default: RecordParameterV1Value,
    pub value: RecordParameterV1Value,
    pub description: RecordParameterV1Description,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub choices: Option<RecordParameterV1Choices>,
}

impl RecordParameterV1 {
    pub fn new(fields: RecordParameterV1Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordParameterV1", &fields, RECORD_PARAMETER_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordParameterV1Fields {
        self.0
    }
}

impl Deref for RecordParameterV1 {
    type Target = RecordParameterV1Fields;

    fn deref(&self) -> &RecordParameterV1Fields {
        &self.0
    }
}

impl TryFrom<RecordParameterV1Fields> for RecordParameterV1 {
    type Error = ContractError;

    fn try_from(fields: RecordParameterV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordParameterV1> for RecordParameterV1Fields {
    fn from(value: RecordParameterV1) -> RecordParameterV1Fields {
        value.0
    }
}

impl Serialize for RecordParameterV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:parameter:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordParameterV1Version;

impl RecordParameterV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordParameterV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordParameterV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

static RECORD_PARAMETER_V1_KEY_PATTERN: Pattern =
    Pattern::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$");

/// `key` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordParameterV1Key(String);

impl RecordParameterV1Key {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordParameterV1Key", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 96 && RECORD_PARAMETER_V1_KEY_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordParameterV1Key {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordParameterV1Key> for String {
    fn from(value: RecordParameterV1Key) -> String {
        value.0
    }
}

impl fmt::Display for RecordParameterV1Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordParameterV1Key {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `type` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordParameterV1Type {
    #[serde(rename = "string")]
    String,
    #[serde(rename = "integer")]
    Integer,
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "boolean")]
    Boolean,
    #[serde(rename = "string_list")]
    StringList,
}

impl RecordParameterV1Type {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::StringList => "string_list",
        }
    }
}

/// `$defs.value` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RecordParameterV1Value {
    String(RecordParameterV1ValueString),
    Number(Number),
    Boolean(bool),
    Array(Vec<RecordParameterV1ValueArrayItem>),
}

/// `$defs.value|string` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordParameterV1ValueString(String);

impl RecordParameterV1ValueString {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordParameterV1ValueString", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 500
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordParameterV1ValueString {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordParameterV1ValueString> for String {
    fn from(value: RecordParameterV1ValueString) -> String {
        value.0
    }
}

impl fmt::Display for RecordParameterV1ValueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordParameterV1ValueString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `$defs.value|array[]` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordParameterV1ValueArrayItem(String);

impl RecordParameterV1ValueArrayItem {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordParameterV1ValueArrayItem", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 500
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordParameterV1ValueArrayItem {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordParameterV1ValueArrayItem> for String {
    fn from(value: RecordParameterV1ValueArrayItem) -> String {
        value.0
    }
}

impl fmt::Display for RecordParameterV1ValueArrayItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordParameterV1ValueArrayItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `description` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordParameterV1Description(String);

impl RecordParameterV1Description {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordParameterV1Description", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=500).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordParameterV1Description {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordParameterV1Description> for String {
    fn from(value: RecordParameterV1Description) -> String {
        value.0
    }
}

impl fmt::Display for RecordParameterV1Description {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordParameterV1Description {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `choices` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordParameterV1ChoicesItem>")]
pub struct RecordParameterV1Choices(Vec<RecordParameterV1ChoicesItem>);

impl RecordParameterV1Choices {
    pub fn new(items: Vec<RecordParameterV1ChoicesItem>) -> Result<Self, ContractError> {
        if items.len() >= 2 && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordParameterV1Choices",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordParameterV1ChoicesItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordParameterV1ChoicesItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordParameterV1ChoicesItem>> for RecordParameterV1Choices {
    type Error = ContractError;

    fn try_from(items: Vec<RecordParameterV1ChoicesItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordParameterV1Choices> for Vec<RecordParameterV1ChoicesItem> {
    fn from(value: RecordParameterV1Choices) -> Vec<RecordParameterV1ChoicesItem> {
        value.0
    }
}

impl Serialize for RecordParameterV1Choices {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `choices[]` in `urn:ads:record:parameter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordParameterV1ChoicesItem(String);

impl RecordParameterV1ChoicesItem {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordParameterV1ChoicesItem", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=64).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordParameterV1ChoicesItem {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordParameterV1ChoicesItem> for String {
    fn from(value: RecordParameterV1ChoicesItem) -> String {
        value.0
    }
}

impl fmt::Display for RecordParameterV1ChoicesItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordParameterV1ChoicesItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
