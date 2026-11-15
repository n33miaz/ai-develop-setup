// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.update-manifest.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_relpath_v1::CommonRelpathV1;
use super::common_sha256_v1::CommonSha256V1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Manifesto de atualização. O que o CI publica a cada junção: qual build é, de qual commit saiu e o SHA-256 de cada arquivo. O lançador confere a assinatura do manifesto (arquivo à parte, sobre os bytes exatos deste JSON) com a chave pública embutida e depois o digest de cada arquivo, antes de trocar qualquer coisa.
///
/// `urn:ads:record:update-manifest:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordUpdateManifestV1 {
    pub version: RecordUpdateManifestV1Version,
    pub build: RecordUpdateManifestV1Build,
    pub commit: RecordUpdateManifestV1Commit,
    pub created_at: CommonTimestampV1,
    pub files: RecordUpdateManifestV1Files,
}

/// `version` in `urn:ads:record:update-manifest:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordUpdateManifestV1Version;

impl RecordUpdateManifestV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordUpdateManifestV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordUpdateManifestV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

static RECORD_UPDATE_MANIFEST_V1_BUILD_PATTERN: Pattern =
    Pattern::new(r"^dev-[0-9]{8}-[0-9a-f]{7,12}$");

/// `build` in `urn:ads:record:update-manifest:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUpdateManifestV1Build(String);

impl RecordUpdateManifestV1Build {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUpdateManifestV1Build", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_UPDATE_MANIFEST_V1_BUILD_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUpdateManifestV1Build {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUpdateManifestV1Build> for String {
    fn from(value: RecordUpdateManifestV1Build) -> String {
        value.0
    }
}

impl fmt::Display for RecordUpdateManifestV1Build {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUpdateManifestV1Build {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static RECORD_UPDATE_MANIFEST_V1_COMMIT_PATTERN: Pattern = Pattern::new(r"^[0-9a-f]{40}$");

/// `commit` in `urn:ads:record:update-manifest:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordUpdateManifestV1Commit(String);

impl RecordUpdateManifestV1Commit {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordUpdateManifestV1Commit", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_UPDATE_MANIFEST_V1_COMMIT_PATTERN.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordUpdateManifestV1Commit {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordUpdateManifestV1Commit> for String {
    fn from(value: RecordUpdateManifestV1Commit) -> String {
        value.0
    }
}

impl fmt::Display for RecordUpdateManifestV1Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordUpdateManifestV1Commit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `files` in `urn:ads:record:update-manifest:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<RecordUpdateManifestV1FilesItem>")]
pub struct RecordUpdateManifestV1Files(Vec<RecordUpdateManifestV1FilesItem>);

impl RecordUpdateManifestV1Files {
    pub fn new(items: Vec<RecordUpdateManifestV1FilesItem>) -> Result<Self, ContractError> {
        if !items.is_empty() {
            Ok(Self(items))
        } else {
            Err(ContractError::new(
                "RecordUpdateManifestV1Files",
                format!("{} items", items.len()),
            ))
        }
    }

    pub fn as_slice(&self) -> &[RecordUpdateManifestV1FilesItem] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<RecordUpdateManifestV1FilesItem> {
        self.0
    }
}

impl TryFrom<Vec<RecordUpdateManifestV1FilesItem>> for RecordUpdateManifestV1Files {
    type Error = ContractError;

    fn try_from(items: Vec<RecordUpdateManifestV1FilesItem>) -> Result<Self, ContractError> {
        Self::new(items)
    }
}

impl From<RecordUpdateManifestV1Files> for Vec<RecordUpdateManifestV1FilesItem> {
    fn from(value: RecordUpdateManifestV1Files) -> Vec<RecordUpdateManifestV1FilesItem> {
        value.0
    }
}

impl Serialize for RecordUpdateManifestV1Files {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `files[]` in `urn:ads:record:update-manifest:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordUpdateManifestV1FilesItem {
    pub path: CommonRelpathV1,
    pub sha256: CommonSha256V1,
}
