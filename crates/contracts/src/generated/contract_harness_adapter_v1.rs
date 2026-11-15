// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.harness-adapter.v1.schema.json. Não edite à mão.

use std::ops::Deref;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use crate::support;
use crate::support::{ContractError, JsonType, Literal, Rule};

static CONTRACT_HARNESS_ADAPTER_V1_RULES: &[(&str, Rule)] = &[(
    "#/if",
    Rule::If {
        when: &Rule::AllOf(&[
            Rule::Properties(&[("kind", Rule::Const(Literal::Str("api")))]),
            Rule::Required(&["kind"]),
        ]),
        then: Some(&Rule::Properties(&[
            ("credential", Rule::Const(Literal::Str("vault"))),
            (
                "modes",
                Rule::AllOf(&[
                    Rule::Type(&[JsonType::Array]),
                    Rule::Items(&Rule::Const(Literal::Str("headless"))),
                ]),
            ),
        ])),
        otherwise: Some(&Rule::Properties(&[(
            "credential",
            Rule::Const(Literal::Str("cli_managed")),
        )])),
    },
)];

/// Descritor do adaptador de IA. O que um adaptador declara ao núcleo no contrato harness-adapter/1: qual ferramenta adapta, se é linha de comando ou API, em que modos e papéis atende e quem guarda a credencial. Adaptador de linha de comando nunca toca na credencial que a ferramenta administra; adaptador de API usa a chave do cofre do produto e não tem modo interativo.
///
/// `urn:ads:contract:harness-adapter:1`.
///
/// Checked: the fields obey the rules the Schema sets between them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "ContractHarnessAdapterV1Fields")]
pub struct ContractHarnessAdapterV1(ContractHarnessAdapterV1Fields);

/// The fields of [`ContractHarnessAdapterV1`], before the rules between them are checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractHarnessAdapterV1Fields {
    pub contract: ContractHarnessAdapterV1Contract,
    pub harness: CommonIdV1,
    pub kind: ContractHarnessAdapterV1Kind,
    pub modes: ContractHarnessAdapterV1Modes,
    pub roles: ContractHarnessAdapterV1Roles,
    pub credential: ContractHarnessAdapterV1Credential,
}

impl ContractHarnessAdapterV1 {
    pub fn new(fields: ContractHarnessAdapterV1Fields) -> Result<Self, ContractError> {
        support::check_rules(
            "ContractHarnessAdapterV1",
            &fields,
            CONTRACT_HARNESS_ADAPTER_V1_RULES,
        )?;
        Ok(Self(fields))
    }

    pub fn into_fields(self) -> ContractHarnessAdapterV1Fields {
        self.0
    }
}

impl Deref for ContractHarnessAdapterV1 {
    type Target = ContractHarnessAdapterV1Fields;

    fn deref(&self) -> &ContractHarnessAdapterV1Fields {
        &self.0
    }
}

impl TryFrom<ContractHarnessAdapterV1Fields> for ContractHarnessAdapterV1 {
    type Error = ContractError;

    fn try_from(fields: ContractHarnessAdapterV1Fields) -> Result<Self, ContractError> {
        Self::new(fields)
    }
}

impl From<ContractHarnessAdapterV1> for ContractHarnessAdapterV1Fields {
    fn from(value: ContractHarnessAdapterV1) -> ContractHarnessAdapterV1Fields {
        value.0
    }
}

impl Serialize for ContractHarnessAdapterV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `contract` in `urn:ads:contract:harness-adapter:1`.
///
/// Always `"harness-adapter/1"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractHarnessAdapterV1Contract;

impl ContractHarnessAdapterV1Contract {
    pub const VALUE: &str = "harness-adapter/1";
}

impl Serialize for ContractHarnessAdapterV1Contract {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractHarnessAdapterV1Contract {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `kind` in `urn:ads:contract:harness-adapter:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractHarnessAdapterV1Kind {
    #[serde(rename = "cli")]
    Cli,
    #[serde(rename = "api")]
    Api,
}

impl ContractHarnessAdapterV1Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Api => "api",
        }
    }
}

/// `modes` in `urn:ads:contract:harness-adapter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<ContractHarnessAdapterV1ModesItem>")]
pub struct ContractHarnessAdapterV1Modes(Vec<ContractHarnessAdapterV1ModesItem>);

impl ContractHarnessAdapterV1Modes {
    pub fn new(items: Vec<ContractHarnessAdapterV1ModesItem>) -> Result<Self, ContractError> {
        if !items.is_empty() && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "ContractHarnessAdapterV1Modes",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[ContractHarnessAdapterV1ModesItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<ContractHarnessAdapterV1ModesItem> {
        self.0
    }
}

impl TryFrom<Vec<ContractHarnessAdapterV1ModesItem>> for ContractHarnessAdapterV1Modes {
    type Error = ContractError;

    fn try_from(items: Vec<ContractHarnessAdapterV1ModesItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<ContractHarnessAdapterV1Modes> for Vec<ContractHarnessAdapterV1ModesItem> {
    fn from(value: ContractHarnessAdapterV1Modes) -> Vec<ContractHarnessAdapterV1ModesItem> {
        value.0
    }
}

impl Serialize for ContractHarnessAdapterV1Modes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `modes[]` in `urn:ads:contract:harness-adapter:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractHarnessAdapterV1ModesItem {
    #[serde(rename = "interactive")]
    Interactive,
    #[serde(rename = "headless")]
    Headless,
}

impl ContractHarnessAdapterV1ModesItem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Headless => "headless",
        }
    }
}

/// `roles` in `urn:ads:contract:harness-adapter:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<ContractHarnessAdapterV1RolesItem>")]
pub struct ContractHarnessAdapterV1Roles(Vec<ContractHarnessAdapterV1RolesItem>);

impl ContractHarnessAdapterV1Roles {
    pub fn new(items: Vec<ContractHarnessAdapterV1RolesItem>) -> Result<Self, ContractError> {
        if !items.is_empty() && support::all_distinct(&items) {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "ContractHarnessAdapterV1Roles",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[ContractHarnessAdapterV1RolesItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<ContractHarnessAdapterV1RolesItem> {
        self.0
    }
}

impl TryFrom<Vec<ContractHarnessAdapterV1RolesItem>> for ContractHarnessAdapterV1Roles {
    type Error = ContractError;

    fn try_from(items: Vec<ContractHarnessAdapterV1RolesItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<ContractHarnessAdapterV1Roles> for Vec<ContractHarnessAdapterV1RolesItem> {
    fn from(value: ContractHarnessAdapterV1Roles) -> Vec<ContractHarnessAdapterV1RolesItem> {
        value.0
    }
}

impl Serialize for ContractHarnessAdapterV1Roles {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `roles[]` in `urn:ads:contract:harness-adapter:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractHarnessAdapterV1RolesItem {
    #[serde(rename = "executor")]
    Executor,
    #[serde(rename = "reviewer")]
    Reviewer,
    #[serde(rename = "router")]
    Router,
    #[serde(rename = "advisor")]
    Advisor,
    #[serde(rename = "orchestrator")]
    Orchestrator,
}

impl ContractHarnessAdapterV1RolesItem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
            Self::Router => "router",
            Self::Advisor => "advisor",
            Self::Orchestrator => "orchestrator",
        }
    }
}

/// `credential` in `urn:ads:contract:harness-adapter:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractHarnessAdapterV1Credential {
    #[serde(rename = "cli_managed")]
    CliManaged,
    #[serde(rename = "vault")]
    Vault,
}

impl ContractHarnessAdapterV1Credential {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CliManaged => "cli_managed",
            Self::Vault => "vault",
        }
    }
}
