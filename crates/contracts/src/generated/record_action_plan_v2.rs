// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.action-plan.v2.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_abspath_v2::CommonAbspathV2;
use super::common_action_rule_v1::CommonActionRuleV1;
use super::common_id_v1::CommonIdV1;
use super::common_risk_v1::CommonRiskV1;
use super::common_sha256_v1::CommonSha256V1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static RECORD_ACTION_PLAN_V2_RULES: &[(&str, Rule)] = &[
    (
        "#/allOf/0",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("quarantined", Rule::Const(Literal::Bool(true)))]),
                Rule::Required(&["quarantined"]),
            ]),
            then: Some(&Rule::Properties(&[
                (
                    "operation",
                    Rule::Enum(&[
                        Literal::Str("fs.delete"),
                        Literal::Str("fs.delete_recursive"),
                    ]),
                ),
                ("destination", Rule::Type(&[JsonType::String])),
                (
                    "verdict",
                    Rule::AllOf(&[
                        Rule::Type(&[JsonType::Object]),
                        Rule::Properties(&[(
                            "decision",
                            Rule::Enum(&[
                                Literal::Str("allowed"),
                                Literal::Str("requires_approval"),
                            ]),
                        )]),
                    ]),
                ),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/allOf/1",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("operation", Rule::Const(Literal::Str("fs.move")))]),
                Rule::Required(&["operation"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("destination", Rule::Type(&[JsonType::String])),
                ("quarantined", Rule::Const(Literal::Bool(false))),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/allOf/2",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[
                    (
                        "operation",
                        Rule::Enum(&[
                            Literal::Str("fs.delete"),
                            Literal::Str("fs.delete_recursive"),
                            Literal::Str("fs.overwrite"),
                            Literal::Str("fs.clean_untracked"),
                        ]),
                    ),
                    ("quarantined", Rule::Const(Literal::Bool(false))),
                ]),
                Rule::Required(&["operation", "quarantined"]),
            ]),
            then: Some(&Rule::Properties(&[(
                "destination",
                Rule::Type(&[JsonType::Null]),
            )])),
            otherwise: None,
        },
    ),
];

/// Plano de ação. O plano de uma ação destrutiva sobre arquivos, como o portão o julgou: a operação pedida, os alvos, a raiz de trabalho, a estimativa do ensaio, o veredito com as regras que dispararam, se o apagar virou mover para a quarentena e o destino. O plan_hash cobre o plano em vigor (o reescrito, quando há quarentena) e pode ser reconferido a partir deste registro pela serialização canônica descrita no README. A versão 2 aperta a versão 1 em dois pontos: os caminhos são urn:ads:common:abspath:2 (sem nome de dispositivo nem nome curto do Windows) e a estimativa vai de 0 a 9007199254740991 (2^53 - 1), o maior inteiro que todo leitor do contrato representa sem perda.
///
/// `urn:ads:record:action-plan:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordActionPlanV2Fields")]
pub struct RecordActionPlanV2(RecordActionPlanV2Fields);

/// The fields of [`RecordActionPlanV2`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV2Fields {
    pub version: RecordActionPlanV2Version,
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub operation: RecordActionPlanV2Operation,
    pub work_root: CommonAbspathV2,
    #[serde(deserialize_with = "support::nullable")]
    pub destination: Option<CommonAbspathV2>,
    pub targets: RecordActionPlanV2Targets,
    #[serde(deserialize_with = "support::nullable")]
    pub estimate: Option<RecordActionPlanV2Estimate>,
    pub verdict: RecordActionPlanV2Verdict,
    pub quarantined: bool,
}

impl RecordActionPlanV2 {
    pub fn new(fields: RecordActionPlanV2Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordActionPlanV2", &fields, RECORD_ACTION_PLAN_V2_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordActionPlanV2Fields {
        self.0
    }
}

impl Deref for RecordActionPlanV2 {
    type Target = RecordActionPlanV2Fields;

    fn deref(&self) -> &RecordActionPlanV2Fields {
        &self.0
    }
}

impl TryFrom<RecordActionPlanV2Fields> for RecordActionPlanV2 {
    type Error = ContractError;

    fn try_from(fields: RecordActionPlanV2Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordActionPlanV2> for RecordActionPlanV2Fields {
    fn from(value: RecordActionPlanV2) -> RecordActionPlanV2Fields {
        value.0
    }
}

impl Serialize for RecordActionPlanV2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:action-plan:2`.
///
/// Always `2`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordActionPlanV2Version;

impl RecordActionPlanV2Version {
    pub const VALUE: i64 = 2;
}

impl Serialize for RecordActionPlanV2Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordActionPlanV2Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `operation` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordActionPlanV2Operation {
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

impl RecordActionPlanV2Operation {
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

/// `targets` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordActionPlanV2TargetsItem>")]
pub struct RecordActionPlanV2Targets(Vec<RecordActionPlanV2TargetsItem>);

impl RecordActionPlanV2Targets {
    pub fn new(items: Vec<RecordActionPlanV2TargetsItem>) -> Result<Self, ContractError> {
        if (1..=10000).contains(&items.len()) && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV2Targets",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordActionPlanV2TargetsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordActionPlanV2TargetsItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordActionPlanV2TargetsItem>> for RecordActionPlanV2Targets {
    type Error = ContractError;

    fn try_from(items: Vec<RecordActionPlanV2TargetsItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordActionPlanV2Targets> for Vec<RecordActionPlanV2TargetsItem> {
    fn from(value: RecordActionPlanV2Targets) -> Vec<RecordActionPlanV2TargetsItem> {
        value.0
    }
}

impl Serialize for RecordActionPlanV2Targets {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `targets[]` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV2TargetsItem {
    pub path: CommonAbspathV2,
    #[serde(deserialize_with = "support::nullable")]
    pub pattern: Option<RecordActionPlanV2TargetsItemPattern>,
    pub repo_root: bool,
}

static RECORD_ACTION_PLAN_V2_TARGETS_ITEM_PATTERN_PATTERN: Pattern =
    Pattern::new(r"^[^\\/$%]*[*?][^\\/$%]*$");

/// `targets[].pattern` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordActionPlanV2TargetsItemPattern(String);

impl RecordActionPlanV2TargetsItemPattern {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV2TargetsItemPattern",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 255 && RECORD_ACTION_PLAN_V2_TARGETS_ITEM_PATTERN_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordActionPlanV2TargetsItemPattern {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordActionPlanV2TargetsItemPattern> for String {
    fn from(value: RecordActionPlanV2TargetsItemPattern) -> String {
        value.0
    }
}

impl fmt::Display for RecordActionPlanV2TargetsItemPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordActionPlanV2TargetsItemPattern {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `estimate` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV2Estimate {
    pub items: RecordActionPlanV2EstimateItems,
    pub bytes: RecordActionPlanV2EstimateBytes,
}

/// `estimate.items` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordActionPlanV2EstimateItems(u64);

impl RecordActionPlanV2EstimateItems {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV2EstimateItems",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordActionPlanV2EstimateItems {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordActionPlanV2EstimateItems> for u64 {
    fn from(value: RecordActionPlanV2EstimateItems) -> u64 {
        value.0
    }
}

impl Serialize for RecordActionPlanV2EstimateItems {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `estimate.bytes` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordActionPlanV2EstimateBytes(u64);

impl RecordActionPlanV2EstimateBytes {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV2EstimateBytes",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordActionPlanV2EstimateBytes {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordActionPlanV2EstimateBytes> for u64 {
    fn from(value: RecordActionPlanV2EstimateBytes) -> u64 {
        value.0
    }
}

impl Serialize for RecordActionPlanV2EstimateBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static RECORD_ACTION_PLAN_V2_VERDICT_RULES: &[(&str, Rule)] = &[
    (
        "#/properties/verdict/allOf/0",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("decision", Rule::Const(Literal::Str("allowed")))]),
                Rule::Required(&["decision"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("risk", Rule::Const(Literal::Str("low"))),
                (
                    "reasons",
                    Rule::AllOf(&[Rule::Type(&[JsonType::Array]), Rule::MaxItems(0)]),
                ),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/properties/verdict/allOf/1",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("decision", Rule::Const(Literal::Str("requires_approval")))]),
                Rule::Required(&["decision"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("risk", Rule::Const(Literal::Str("medium"))),
                (
                    "reasons",
                    Rule::AllOf(&[Rule::Type(&[JsonType::Array]), Rule::MinItems(1)]),
                ),
            ])),
            otherwise: None,
        },
    ),
    (
        "#/properties/verdict/allOf/2",
        Rule::If {
            when: &Rule::AllOf(&[
                Rule::Properties(&[("decision", Rule::Const(Literal::Str("denied")))]),
                Rule::Required(&["decision"]),
            ]),
            then: Some(&Rule::Properties(&[
                ("risk", Rule::Const(Literal::Str("high"))),
                (
                    "reasons",
                    Rule::AllOf(&[Rule::Type(&[JsonType::Array]), Rule::MinItems(1)]),
                ),
            ])),
            otherwise: None,
        },
    ),
];

/// `verdict` in `urn:ads:record:action-plan:2`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordActionPlanV2VerdictFields")]
pub struct RecordActionPlanV2Verdict(RecordActionPlanV2VerdictFields);

/// The fields of [`RecordActionPlanV2Verdict`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV2VerdictFields {
    pub decision: RecordActionPlanV2VerdictDecision,
    pub risk: CommonRiskV1,
    pub reasons: RecordActionPlanV2VerdictReasons,
}

impl RecordActionPlanV2Verdict {
    pub fn new(fields: RecordActionPlanV2VerdictFields) -> Result<Self, ContractError> {
        support::check_rules(
            "RecordActionPlanV2Verdict",
            &fields,
            RECORD_ACTION_PLAN_V2_VERDICT_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordActionPlanV2VerdictFields {
        self.0
    }
}

impl Deref for RecordActionPlanV2Verdict {
    type Target = RecordActionPlanV2VerdictFields;

    fn deref(&self) -> &RecordActionPlanV2VerdictFields {
        &self.0
    }
}

impl TryFrom<RecordActionPlanV2VerdictFields> for RecordActionPlanV2Verdict {
    type Error = ContractError;

    fn try_from(fields: RecordActionPlanV2VerdictFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordActionPlanV2Verdict> for RecordActionPlanV2VerdictFields {
    fn from(value: RecordActionPlanV2Verdict) -> RecordActionPlanV2VerdictFields {
        value.0
    }
}

impl Serialize for RecordActionPlanV2Verdict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `verdict.decision` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordActionPlanV2VerdictDecision {
    #[serde(rename = "allowed")]
    Allowed,
    #[serde(rename = "requires_approval")]
    RequiresApproval,
    #[serde(rename = "denied")]
    Denied,
}

impl RecordActionPlanV2VerdictDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::RequiresApproval => "requires_approval",
            Self::Denied => "denied",
        }
    }
}

/// `verdict.reasons` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordActionPlanV2VerdictReasonsItem>")]
pub struct RecordActionPlanV2VerdictReasons(Vec<RecordActionPlanV2VerdictReasonsItem>);

impl RecordActionPlanV2VerdictReasons {
    pub fn new(items: Vec<RecordActionPlanV2VerdictReasonsItem>) -> Result<Self, ContractError> {
        if items.len() <= 1000 {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV2VerdictReasons",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordActionPlanV2VerdictReasonsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordActionPlanV2VerdictReasonsItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordActionPlanV2VerdictReasonsItem>> for RecordActionPlanV2VerdictReasons {
    type Error = ContractError;

    fn try_from(items: Vec<RecordActionPlanV2VerdictReasonsItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordActionPlanV2VerdictReasons> for Vec<RecordActionPlanV2VerdictReasonsItem> {
    fn from(value: RecordActionPlanV2VerdictReasons) -> Vec<RecordActionPlanV2VerdictReasonsItem> {
        value.0
    }
}

impl Serialize for RecordActionPlanV2VerdictReasons {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `verdict.reasons[]` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV2VerdictReasonsItem {
    pub rule: CommonActionRuleV1,
    pub subject: RecordActionPlanV2VerdictReasonsItemSubject,
}

/// `verdict.reasons[].subject` in `urn:ads:record:action-plan:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordActionPlanV2VerdictReasonsItemSubject(String);

impl RecordActionPlanV2VerdictReasonsItemSubject {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV2VerdictReasonsItemSubject",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=4096).contains(&length)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordActionPlanV2VerdictReasonsItemSubject {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordActionPlanV2VerdictReasonsItemSubject> for String {
    fn from(value: RecordActionPlanV2VerdictReasonsItemSubject) -> String {
        value.0
    }
}

impl fmt::Display for RecordActionPlanV2VerdictReasonsItemSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordActionPlanV2VerdictReasonsItemSubject {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
