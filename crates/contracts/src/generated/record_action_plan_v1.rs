// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.action-plan.v1.schema.json. Não edite à mão.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_abspath_v1::CommonAbspathV1;
use super::common_action_rule_v1::CommonActionRuleV1;
use super::common_id_v1::CommonIdV1;
use super::common_risk_v1::CommonRiskV1;
use super::common_sha256_v1::CommonSha256V1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Pattern, Rule};

static RECORD_ACTION_PLAN_V1_RULES: &[(&str, Rule)] = &[
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

/// Plano de ação. O plano de uma ação destrutiva sobre arquivos, como o portão o julgou: a operação pedida, os alvos, a raiz de trabalho, a estimativa do ensaio, o veredito com as regras que dispararam, se o apagar virou mover para a quarentena e o destino. O plan_hash cobre o plano em vigor (o reescrito, quando há quarentena) e pode ser reconferido a partir deste registro pela serialização canônica descrita no README.
///
/// `urn:ads:record:action-plan:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordActionPlanV1Fields")]
pub struct RecordActionPlanV1(RecordActionPlanV1Fields);

/// The fields of [`RecordActionPlanV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV1Fields {
    pub version: RecordActionPlanV1Version,
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    pub operation: RecordActionPlanV1Operation,
    pub work_root: CommonAbspathV1,
    #[serde(deserialize_with = "support::nullable")]
    pub destination: Option<CommonAbspathV1>,
    pub targets: RecordActionPlanV1Targets,
    #[serde(deserialize_with = "support::nullable")]
    pub estimate: Option<RecordActionPlanV1Estimate>,
    pub verdict: RecordActionPlanV1Verdict,
    pub quarantined: bool,
}

impl RecordActionPlanV1 {
    pub fn new(fields: RecordActionPlanV1Fields) -> Result<Self, ContractError> {
        support::check_rules("RecordActionPlanV1", &fields, RECORD_ACTION_PLAN_V1_RULES)?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordActionPlanV1Fields {
        self.0
    }
}

impl Deref for RecordActionPlanV1 {
    type Target = RecordActionPlanV1Fields;

    fn deref(&self) -> &RecordActionPlanV1Fields {
        &self.0
    }
}

impl TryFrom<RecordActionPlanV1Fields> for RecordActionPlanV1 {
    type Error = ContractError;

    fn try_from(fields: RecordActionPlanV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordActionPlanV1> for RecordActionPlanV1Fields {
    fn from(value: RecordActionPlanV1) -> RecordActionPlanV1Fields {
        value.0
    }
}

impl Serialize for RecordActionPlanV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `version` in `urn:ads:record:action-plan:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordActionPlanV1Version;

impl RecordActionPlanV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordActionPlanV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordActionPlanV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `operation` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordActionPlanV1Operation {
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

impl RecordActionPlanV1Operation {
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

/// `targets` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordActionPlanV1TargetsItem>")]
pub struct RecordActionPlanV1Targets(Vec<RecordActionPlanV1TargetsItem>);

impl RecordActionPlanV1Targets {
    pub fn new(items: Vec<RecordActionPlanV1TargetsItem>) -> Result<Self, ContractError> {
        if (1..=10000).contains(&items.len()) && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV1Targets",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordActionPlanV1TargetsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordActionPlanV1TargetsItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordActionPlanV1TargetsItem>> for RecordActionPlanV1Targets {
    type Error = ContractError;

    fn try_from(items: Vec<RecordActionPlanV1TargetsItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordActionPlanV1Targets> for Vec<RecordActionPlanV1TargetsItem> {
    fn from(value: RecordActionPlanV1Targets) -> Vec<RecordActionPlanV1TargetsItem> {
        value.0
    }
}

impl Serialize for RecordActionPlanV1Targets {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `targets[]` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV1TargetsItem {
    pub path: CommonAbspathV1,
    #[serde(deserialize_with = "support::nullable")]
    pub pattern: Option<RecordActionPlanV1TargetsItemPattern>,
    pub repo_root: bool,
}

static RECORD_ACTION_PLAN_V1_TARGETS_ITEM_PATTERN_PATTERN: Pattern =
    Pattern::new(r"^[^\\/$%]*[*?][^\\/$%]*$");

/// `targets[].pattern` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordActionPlanV1TargetsItemPattern(String);

impl RecordActionPlanV1TargetsItemPattern {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV1TargetsItemPattern",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= 255 && RECORD_ACTION_PLAN_V1_TARGETS_ITEM_PATTERN_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordActionPlanV1TargetsItemPattern {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordActionPlanV1TargetsItemPattern> for String {
    fn from(value: RecordActionPlanV1TargetsItemPattern) -> String {
        value.0
    }
}

impl fmt::Display for RecordActionPlanV1TargetsItemPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordActionPlanV1TargetsItemPattern {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `estimate` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV1Estimate {
    pub items: u64,
    pub bytes: u64,
}

static RECORD_ACTION_PLAN_V1_VERDICT_RULES: &[(&str, Rule)] = &[
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

/// `verdict` in `urn:ads:record:action-plan:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RecordActionPlanV1VerdictFields")]
pub struct RecordActionPlanV1Verdict(RecordActionPlanV1VerdictFields);

/// The fields of [`RecordActionPlanV1Verdict`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV1VerdictFields {
    pub decision: RecordActionPlanV1VerdictDecision,
    pub risk: CommonRiskV1,
    pub reasons: RecordActionPlanV1VerdictReasons,
}

impl RecordActionPlanV1Verdict {
    pub fn new(fields: RecordActionPlanV1VerdictFields) -> Result<Self, ContractError> {
        support::check_rules(
            "RecordActionPlanV1Verdict",
            &fields,
            RECORD_ACTION_PLAN_V1_VERDICT_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> RecordActionPlanV1VerdictFields {
        self.0
    }
}

impl Deref for RecordActionPlanV1Verdict {
    type Target = RecordActionPlanV1VerdictFields;

    fn deref(&self) -> &RecordActionPlanV1VerdictFields {
        &self.0
    }
}

impl TryFrom<RecordActionPlanV1VerdictFields> for RecordActionPlanV1Verdict {
    type Error = ContractError;

    fn try_from(fields: RecordActionPlanV1VerdictFields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<RecordActionPlanV1Verdict> for RecordActionPlanV1VerdictFields {
    fn from(value: RecordActionPlanV1Verdict) -> RecordActionPlanV1VerdictFields {
        value.0
    }
}

impl Serialize for RecordActionPlanV1Verdict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `verdict.decision` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordActionPlanV1VerdictDecision {
    #[serde(rename = "allowed")]
    Allowed,
    #[serde(rename = "requires_approval")]
    RequiresApproval,
    #[serde(rename = "denied")]
    Denied,
}

impl RecordActionPlanV1VerdictDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::RequiresApproval => "requires_approval",
            Self::Denied => "denied",
        }
    }
}

/// `verdict.reasons` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordActionPlanV1VerdictReasonsItem>")]
pub struct RecordActionPlanV1VerdictReasons(Vec<RecordActionPlanV1VerdictReasonsItem>);

impl RecordActionPlanV1VerdictReasons {
    pub fn new(items: Vec<RecordActionPlanV1VerdictReasonsItem>) -> Result<Self, ContractError> {
        if items.len() <= 1000 {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV1VerdictReasons",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordActionPlanV1VerdictReasonsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordActionPlanV1VerdictReasonsItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordActionPlanV1VerdictReasonsItem>> for RecordActionPlanV1VerdictReasons {
    type Error = ContractError;

    fn try_from(items: Vec<RecordActionPlanV1VerdictReasonsItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordActionPlanV1VerdictReasons> for Vec<RecordActionPlanV1VerdictReasonsItem> {
    fn from(value: RecordActionPlanV1VerdictReasons) -> Vec<RecordActionPlanV1VerdictReasonsItem> {
        value.0
    }
}

impl Serialize for RecordActionPlanV1VerdictReasons {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `verdict.reasons[]` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionPlanV1VerdictReasonsItem {
    pub rule: CommonActionRuleV1,
    pub subject: RecordActionPlanV1VerdictReasonsItemSubject,
}

/// `verdict.reasons[].subject` in `urn:ads:record:action-plan:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordActionPlanV1VerdictReasonsItemSubject(String);

impl RecordActionPlanV1VerdictReasonsItemSubject {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordActionPlanV1VerdictReasonsItemSubject",
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

impl TryFrom<String> for RecordActionPlanV1VerdictReasonsItemSubject {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordActionPlanV1VerdictReasonsItemSubject> for String {
    fn from(value: RecordActionPlanV1VerdictReasonsItemSubject) -> String {
        value.0
    }
}

impl fmt::Display for RecordActionPlanV1VerdictReasonsItemSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordActionPlanV1VerdictReasonsItemSubject {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
