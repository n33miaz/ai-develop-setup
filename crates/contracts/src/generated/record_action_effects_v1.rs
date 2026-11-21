// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.action-effects.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_abspath_v2::CommonAbspathV2;
use super::common_id_v1::CommonIdV1;
use super::common_sha256_v1::CommonSha256V1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Efeitos concretos de uma ação. O que o ensaio de uma ação achou no disco e o que a execução vai fazer, alvo por alvo: o caminho, o efeito, onde a quarentena guarda o alvo ou de onde a restauração o tira, e cada entrada da árvore (nomes abaixo do alvo, tipo, tamanho de arquivo e identidade no volume). Na restauração, manifest é o resumo dos bytes do manifesto de onde vieram os pares; um item do manifesto que não está mais na quarentena aparece sem entradas. É o que a aprovação cobre: a execução refaz o ensaio e recusa antes de tocar o disco se achar outro registro. O tamanho é limitado pelo parâmetro max_effects_bytes do executor.
///
/// `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionEffectsV1 {
    pub version: RecordActionEffectsV1Version,
    pub action_id: CommonIdV1,
    pub plan_hash: CommonSha256V1,
    #[serde(deserialize_with = "support::nullable")]
    pub manifest: Option<CommonSha256V1>,
    pub steps: RecordActionEffectsV1Steps,
}

/// `version` in `urn:ads:record:action-effects:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordActionEffectsV1Version;

impl RecordActionEffectsV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordActionEffectsV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordActionEffectsV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `steps` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordActionEffectsV1StepsItem>")]
pub struct RecordActionEffectsV1Steps(Vec<RecordActionEffectsV1StepsItem>);

impl RecordActionEffectsV1Steps {
    pub fn new(items: Vec<RecordActionEffectsV1StepsItem>) -> Result<Self, ContractError> {
        if !items.is_empty() {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordActionEffectsV1Steps",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordActionEffectsV1StepsItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordActionEffectsV1StepsItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordActionEffectsV1StepsItem>> for RecordActionEffectsV1Steps {
    type Error = ContractError;

    fn try_from(items: Vec<RecordActionEffectsV1StepsItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordActionEffectsV1Steps> for Vec<RecordActionEffectsV1StepsItem> {
    fn from(value: RecordActionEffectsV1Steps) -> Vec<RecordActionEffectsV1StepsItem> {
        value.0
    }
}

impl Serialize for RecordActionEffectsV1Steps {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `steps[]` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionEffectsV1StepsItem {
    pub target: CommonAbspathV2,
    pub effect: RecordActionEffectsV1StepsItemEffect,
    #[serde(deserialize_with = "support::nullable")]
    pub stored: Option<CommonAbspathV2>,
    pub entries: Vec<RecordActionEffectsV1StepsItemEntriesItem>,
}

/// `steps[].effect` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordActionEffectsV1StepsItemEffect {
    #[serde(rename = "quarantine")]
    Quarantine,
    #[serde(rename = "restore")]
    Restore,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "delete_recursive")]
    DeleteRecursive,
}

impl RecordActionEffectsV1StepsItemEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quarantine => "quarantine",
            Self::Restore => "restore",
            Self::Delete => "delete",
            Self::DeleteRecursive => "delete_recursive",
        }
    }
}

/// `steps[].entries[]` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionEffectsV1StepsItemEntriesItem {
    pub path: RecordActionEffectsV1StepsItemEntriesItemPath,
    pub kind: RecordActionEffectsV1StepsItemEntriesItemKind,
    pub bytes: RecordActionEffectsV1StepsItemEntriesItemBytes,
    pub id: RecordActionEffectsV1StepsItemEntriesItemId,
}

/// `steps[].entries[].path` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem>")]
pub struct RecordActionEffectsV1StepsItemEntriesItemPath(
    Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem>,
);

impl RecordActionEffectsV1StepsItemEntriesItemPath {
    pub const MAX_ITEMS: usize = 128;

    pub fn new(
        items: Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem>,
    ) -> Result<Self, ContractError> {
        if items.len() <= Self::MAX_ITEMS {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordActionEffectsV1StepsItemEntriesItemPath",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordActionEffectsV1StepsItemEntriesItemPathItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem>>
    for RecordActionEffectsV1StepsItemEntriesItemPath
{
    type Error = ContractError;

    fn try_from(
        items: Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem>,
    ) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordActionEffectsV1StepsItemEntriesItemPath>
    for Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem>
{
    fn from(
        value: RecordActionEffectsV1StepsItemEntriesItemPath,
    ) -> Vec<RecordActionEffectsV1StepsItemEntriesItemPathItem> {
        value.0
    }
}

impl Serialize for RecordActionEffectsV1StepsItemEntriesItemPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static RECORD_ACTION_EFFECTS_V1_STEPS_ITEM_ENTRIES_ITEM_PATH_ITEM_PATTERN: Pattern =
    Pattern::new(r"^[^\\/]+$");

/// `steps[].entries[].path[]` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordActionEffectsV1StepsItemEntriesItemPathItem(String);

impl RecordActionEffectsV1StepsItemEntriesItemPathItem {
    pub const MAX_CHARS: usize = 255;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordActionEffectsV1StepsItemEntriesItemPathItem",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=Self::MAX_CHARS).contains(&length)
            && RECORD_ACTION_EFFECTS_V1_STEPS_ITEM_ENTRIES_ITEM_PATH_ITEM_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordActionEffectsV1StepsItemEntriesItemPathItem {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordActionEffectsV1StepsItemEntriesItemPathItem> for String {
    fn from(value: RecordActionEffectsV1StepsItemEntriesItemPathItem) -> String {
        value.0
    }
}

impl fmt::Display for RecordActionEffectsV1StepsItemEntriesItemPathItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordActionEffectsV1StepsItemEntriesItemPathItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `steps[].entries[].kind` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordActionEffectsV1StepsItemEntriesItemKind {
    #[serde(rename = "dir")]
    Dir,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "link")]
    Link,
}

impl RecordActionEffectsV1StepsItemEntriesItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dir => "dir",
            Self::File => "file",
            Self::Link => "link",
        }
    }
}

/// `steps[].entries[].bytes` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordActionEffectsV1StepsItemEntriesItemBytes(u64);

impl RecordActionEffectsV1StepsItemEntriesItemBytes {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordActionEffectsV1StepsItemEntriesItemBytes",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordActionEffectsV1StepsItemEntriesItemBytes {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordActionEffectsV1StepsItemEntriesItemBytes> for u64 {
    fn from(value: RecordActionEffectsV1StepsItemEntriesItemBytes) -> u64 {
        value.0
    }
}

impl Serialize for RecordActionEffectsV1StepsItemEntriesItemBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static RECORD_ACTION_EFFECTS_V1_STEPS_ITEM_ENTRIES_ITEM_ID_PATTERN: Pattern =
    Pattern::new(r"^[0-9]{1,20}:[0-9]{1,39}$");

/// `steps[].entries[].id` in `urn:ads:record:action-effects:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordActionEffectsV1StepsItemEntriesItemId(String);

impl RecordActionEffectsV1StepsItemEntriesItemId {
    pub const MAX_CHARS: usize = 64;

    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new(
                "RecordActionEffectsV1StepsItemEntriesItemId",
                text,
            ))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        length <= Self::MAX_CHARS
            && RECORD_ACTION_EFFECTS_V1_STEPS_ITEM_ENTRIES_ITEM_ID_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordActionEffectsV1StepsItemEntriesItemId {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordActionEffectsV1StepsItemEntriesItemId> for String {
    fn from(value: RecordActionEffectsV1StepsItemEntriesItemId) -> String {
        value.0
    }
}

impl fmt::Display for RecordActionEffectsV1StepsItemEntriesItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordActionEffectsV1StepsItemEntriesItemId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
