// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/record.queue-task.v1.schema.json. Não edite à mão.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use super::common_queue_task_state_v1::CommonQueueTaskStateV1;
use super::common_timestamp_v1::CommonTimestampV1;
use crate::support;
use crate::support::{ContractError, Pattern};

/// Tarefa da fila. Retrato versionado de uma tarefa. A pasta é referida por apelido; o teto de tentativas inclui a primeira execução.
///
/// `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1 {
    pub version: RecordQueueTaskV1Version,
    pub id: CommonIdV1,
    pub queue_id: CommonIdV1,
    pub origin: RecordQueueTaskV1Origin,
    pub title: RecordQueueTaskV1Title,
    pub workspace_alias: RecordQueueTaskV1WorkspaceAlias,
    pub tool: CommonIdV1,
    pub state: CommonQueueTaskStateV1,
    pub attempts: RecordQueueTaskV1Attempts,
    pub attempt_limit: RecordQueueTaskV1AttemptLimit,
    pub created_at: CommonTimestampV1,
    pub updated_at: CommonTimestampV1,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub branch: Option<RecordQueueTaskV1Branch>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "support::present"
    )]
    pub pull_request: Option<RecordQueueTaskV1PullRequest>,
}

/// `version` in `urn:ads:record:queue-task:1`.
///
/// Always `1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordQueueTaskV1Version;

impl RecordQueueTaskV1Version {
    pub const VALUE: i64 = 1;
}

impl Serialize for RecordQueueTaskV1Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for RecordQueueTaskV1Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `origin` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RecordQueueTaskV1Origin {
    #[serde(rename = "manual")]
    Manual(RecordQueueTaskV1OriginManual),
    #[serde(rename = "issue")]
    Issue(RecordQueueTaskV1OriginIssue),
    #[serde(rename = "bug_rule")]
    BugRule(RecordQueueTaskV1OriginBugRule),
}

/// `origin|manual` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1OriginManual {
    pub request_id: CommonIdV1,
}

/// `origin|issue` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1OriginIssue {
    pub r#ref: RecordQueueTaskV1OriginIssueRef,
}

/// `origin|issue.ref` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV1OriginIssueRef(u64);

impl RecordQueueTaskV1OriginIssueRef {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1OriginIssueRef",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV1OriginIssueRef {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1OriginIssueRef> for u64 {
    fn from(value: RecordQueueTaskV1OriginIssueRef) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1OriginIssueRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `origin|bug_rule` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordQueueTaskV1OriginBugRule {
    pub rule_id: CommonIdV1,
    pub target_id: CommonIdV1,
}

static RECORD_QUEUE_TASK_V1_TITLE_PATTERN: Pattern =
    Pattern::new(r"^[\p{L}\p{M}\p{N}\p{P}\p{S} \x{200d}]*$");
static RECORD_QUEUE_TASK_V1_TITLE_EXCLUDED: Pattern = Pattern::new(
    r"[\x{034f}\x{115f}-\x{1160}\x{17b4}-\x{17b5}\x{180b}-\x{180f}\x{2800}\x{3164}\x{fe00}-\x{fe0e}\x{ffa0}\x{fffc}\x{16FE4}\x{1D159}\x{E0100}-\x{E01EF}\x{0378}-\x{0379}\x{0380}-\x{0383}\x{038b}\x{038d}\x{03a2}\x{0530}\x{0557}-\x{0558}\x{058b}-\x{058c}\x{0590}\x{05c8}-\x{05cf}\x{05eb}-\x{05ee}\x{05f5}-\x{05ff}\x{070e}\x{074b}-\x{074c}\x{07b2}-\x{07bf}\x{07fb}-\x{07fc}\x{082e}-\x{082f}\x{083f}\x{085c}-\x{085d}\x{085f}\x{086b}-\x{086f}\x{088f}\x{0892}-\x{0896}\x{0984}\x{098d}-\x{098e}\x{0991}-\x{0992}\x{09a9}\x{09b1}\x{09b3}-\x{09b5}\x{09ba}-\x{09bb}\x{09c5}-\x{09c6}\x{09c9}-\x{09ca}\x{09cf}-\x{09d6}\x{09d8}-\x{09db}\x{09de}\x{09e4}-\x{09e5}\x{09ff}-\x{0a00}\x{0a04}\x{0a0b}-\x{0a0e}\x{0a11}-\x{0a12}\x{0a29}\x{0a31}\x{0a34}\x{0a37}\x{0a3a}-\x{0a3b}\x{0a3d}\x{0a43}-\x{0a46}\x{0a49}-\x{0a4a}\x{0a4e}-\x{0a50}\x{0a52}-\x{0a58}\x{0a5d}\x{0a5f}-\x{0a65}\x{0a77}-\x{0a80}\x{0a84}\x{0a8e}\x{0a92}\x{0aa9}\x{0ab1}\x{0ab4}\x{0aba}-\x{0abb}\x{0ac6}\x{0aca}\x{0ace}-\x{0acf}\x{0ad1}-\x{0adf}\x{0ae4}-\x{0ae5}\x{0af2}-\x{0af8}\x{0b00}\x{0b04}\x{0b0d}-\x{0b0e}\x{0b11}-\x{0b12}\x{0b29}\x{0b31}\x{0b34}\x{0b3a}-\x{0b3b}\x{0b45}-\x{0b46}\x{0b49}-\x{0b4a}\x{0b4e}-\x{0b54}\x{0b58}-\x{0b5b}\x{0b5e}\x{0b64}-\x{0b65}\x{0b78}-\x{0b81}\x{0b84}\x{0b8b}-\x{0b8d}\x{0b91}\x{0b96}-\x{0b98}\x{0b9b}\x{0b9d}\x{0ba0}-\x{0ba2}\x{0ba5}-\x{0ba7}\x{0bab}-\x{0bad}\x{0bba}-\x{0bbd}\x{0bc3}-\x{0bc5}\x{0bc9}\x{0bce}-\x{0bcf}\x{0bd1}-\x{0bd6}\x{0bd8}-\x{0be5}\x{0bfb}-\x{0bff}\x{0c0d}\x{0c11}\x{0c29}\x{0c3a}-\x{0c3b}\x{0c45}\x{0c49}\x{0c4e}-\x{0c54}\x{0c57}\x{0c5b}-\x{0c5c}\x{0c5e}-\x{0c5f}\x{0c64}-\x{0c65}\x{0c70}-\x{0c76}\x{0c8d}\x{0c91}\x{0ca9}\x{0cb4}\x{0cba}-\x{0cbb}\x{0cc5}\x{0cc9}\x{0cce}-\x{0cd4}\x{0cd7}-\x{0cdc}\x{0cdf}\x{0ce4}-\x{0ce5}\x{0cf0}\x{0cf4}-\x{0cff}\x{0d0d}\x{0d11}\x{0d45}\x{0d49}\x{0d50}-\x{0d53}\x{0d64}-\x{0d65}\x{0d80}\x{0d84}\x{0d97}-\x{0d99}\x{0db2}\x{0dbc}\x{0dbe}-\x{0dbf}\x{0dc7}-\x{0dc9}\x{0dcb}-\x{0dce}\x{0dd5}\x{0dd7}\x{0de0}-\x{0de5}\x{0df0}-\x{0df1}\x{0df5}-\x{0e00}\x{0e3b}-\x{0e3e}\x{0e5c}-\x{0e80}\x{0e83}\x{0e85}\x{0e8b}\x{0ea4}\x{0ea6}\x{0ebe}-\x{0ebf}\x{0ec5}\x{0ec7}\x{0ecf}\x{0eda}-\x{0edb}\x{0ee0}-\x{0eff}\x{0f48}\x{0f6d}-\x{0f70}\x{0f98}\x{0fbd}\x{0fcd}\x{0fdb}-\x{0fff}\x{10c6}\x{10c8}-\x{10cc}\x{10ce}-\x{10cf}\x{1249}\x{124e}-\x{124f}\x{1257}\x{1259}\x{125e}-\x{125f}\x{1289}\x{128e}-\x{128f}\x{12b1}\x{12b6}-\x{12b7}\x{12bf}\x{12c1}\x{12c6}-\x{12c7}\x{12d7}\x{1311}\x{1316}-\x{1317}\x{135b}-\x{135c}\x{137d}-\x{137f}\x{139a}-\x{139f}\x{13f6}-\x{13f7}\x{13fe}-\x{13ff}\x{169d}-\x{169f}\x{16f9}-\x{16ff}\x{1716}-\x{171e}\x{1737}-\x{173f}\x{1754}-\x{175f}\x{176d}\x{1771}\x{1774}-\x{177f}\x{17de}-\x{17df}\x{17ea}-\x{17ef}\x{17fa}-\x{17ff}\x{181a}-\x{181f}\x{1879}-\x{187f}\x{18ab}-\x{18af}\x{18f6}-\x{18ff}\x{191f}\x{192c}-\x{192f}\x{193c}-\x{193f}\x{1941}-\x{1943}\x{196e}-\x{196f}\x{1975}-\x{197f}\x{19ac}-\x{19af}\x{19ca}-\x{19cf}\x{19db}-\x{19dd}\x{1a1c}-\x{1a1d}\x{1a5f}\x{1a7d}-\x{1a7e}\x{1a8a}-\x{1a8f}\x{1a9a}-\x{1a9f}\x{1aae}-\x{1aaf}\x{1acf}-\x{1aff}\x{1b4d}\x{1bf4}-\x{1bfb}\x{1c38}-\x{1c3a}\x{1c4a}-\x{1c4c}\x{1c8b}-\x{1c8f}\x{1cbb}-\x{1cbc}\x{1cc8}-\x{1ccf}\x{1cfb}-\x{1cff}\x{1f16}-\x{1f17}\x{1f1e}-\x{1f1f}\x{1f46}-\x{1f47}\x{1f4e}-\x{1f4f}\x{1f58}\x{1f5a}\x{1f5c}\x{1f5e}\x{1f7e}-\x{1f7f}\x{1fb5}\x{1fc5}\x{1fd4}-\x{1fd5}\x{1fdc}\x{1ff0}-\x{1ff1}\x{1ff5}\x{1fff}\x{2065}\x{2072}-\x{2073}\x{208f}\x{209d}-\x{209f}\x{20c1}-\x{20cf}\x{20f1}-\x{20ff}\x{218c}-\x{218f}\x{242a}-\x{243f}\x{244b}-\x{245f}\x{2b74}-\x{2b75}\x{2b96}\x{2cf4}-\x{2cf8}\x{2d26}\x{2d28}-\x{2d2c}\x{2d2e}-\x{2d2f}\x{2d68}-\x{2d6e}\x{2d71}-\x{2d7e}\x{2d97}-\x{2d9f}\x{2da7}\x{2daf}\x{2db7}\x{2dbf}\x{2dc7}\x{2dcf}\x{2dd7}\x{2ddf}\x{2e5e}-\x{2e7f}\x{2e9a}\x{2ef4}-\x{2eff}\x{2fd6}-\x{2fef}\x{3040}\x{3097}-\x{3098}\x{3100}-\x{3104}\x{3130}\x{318f}\x{31e6}-\x{31ee}\x{321f}\x{a48d}-\x{a48f}\x{a4c7}-\x{a4cf}\x{a62c}-\x{a63f}\x{a6f8}-\x{a6ff}\x{a7ce}-\x{a7cf}\x{a7d2}\x{a7d4}\x{a7dd}-\x{a7f1}\x{a82d}-\x{a82f}\x{a83a}-\x{a83f}\x{a878}-\x{a87f}\x{a8c6}-\x{a8cd}\x{a8da}-\x{a8df}\x{a954}-\x{a95e}\x{a97d}-\x{a97f}\x{a9ce}\x{a9da}-\x{a9dd}\x{a9ff}\x{aa37}-\x{aa3f}\x{aa4e}-\x{aa4f}\x{aa5a}-\x{aa5b}\x{aac3}-\x{aada}\x{aaf7}-\x{ab00}\x{ab07}-\x{ab08}\x{ab0f}-\x{ab10}\x{ab17}-\x{ab1f}\x{ab27}\x{ab2f}\x{ab6c}-\x{ab6f}\x{abee}-\x{abef}\x{abfa}-\x{abff}\x{d7a4}-\x{d7af}\x{d7c7}-\x{d7ca}\x{d7fc}-\x{d7ff}\x{fa6e}-\x{fa6f}\x{fada}-\x{faff}\x{fb07}-\x{fb12}\x{fb18}-\x{fb1c}\x{fb37}\x{fb3d}\x{fb3f}\x{fb42}\x{fb45}\x{fbc3}-\x{fbd2}\x{fd90}-\x{fd91}\x{fdc8}-\x{fdce}\x{fdd0}-\x{fdef}\x{fe1a}-\x{fe1f}\x{fe53}\x{fe67}\x{fe6c}-\x{fe6f}\x{fe75}\x{fefd}-\x{fefe}\x{ff00}\x{ffbf}-\x{ffc1}\x{ffc8}-\x{ffc9}\x{ffd0}-\x{ffd1}\x{ffd8}-\x{ffd9}\x{ffdd}-\x{ffdf}\x{ffe7}\x{ffef}-\x{fff8}\x{fffe}-\x{ffff}\x{1000C}\x{10027}\x{1003B}\x{1003E}\x{1004E}-\x{1004F}\x{1005E}-\x{1007F}\x{100FB}-\x{100FF}\x{10103}-\x{10106}\x{10134}-\x{10136}\x{1018F}\x{1019D}-\x{1019F}\x{101A1}-\x{101CF}\x{101FE}-\x{1027F}\x{1029D}-\x{1029F}\x{102D1}-\x{102DF}\x{102FC}-\x{102FF}\x{10324}-\x{1032C}\x{1034B}-\x{1034F}\x{1037B}-\x{1037F}\x{1039E}\x{103C4}-\x{103C7}\x{103D6}-\x{103FF}\x{1049E}-\x{1049F}\x{104AA}-\x{104AF}\x{104D4}-\x{104D7}\x{104FC}-\x{104FF}\x{10528}-\x{1052F}\x{10564}-\x{1056E}\x{1057B}\x{1058B}\x{10593}\x{10596}\x{105A2}\x{105B2}\x{105BA}\x{105BD}-\x{105BF}\x{105F4}-\x{105FF}\x{10737}-\x{1073F}\x{10756}-\x{1075F}\x{10768}-\x{1077F}\x{10786}\x{107B1}\x{107BB}-\x{107FF}\x{10806}-\x{10807}\x{10809}\x{10836}\x{10839}-\x{1083B}\x{1083D}-\x{1083E}\x{10856}\x{1089F}-\x{108A6}\x{108B0}-\x{108DF}\x{108F3}\x{108F6}-\x{108FA}\x{1091C}-\x{1091E}\x{1093A}-\x{1093E}\x{10940}-\x{1097F}\x{109B8}-\x{109BB}\x{109D0}-\x{109D1}\x{10A04}\x{10A07}-\x{10A0B}\x{10A14}\x{10A18}\x{10A36}-\x{10A37}\x{10A3B}-\x{10A3E}\x{10A49}-\x{10A4F}\x{10A59}-\x{10A5F}\x{10AA0}-\x{10ABF}\x{10AE7}-\x{10AEA}\x{10AF7}-\x{10AFF}\x{10B36}-\x{10B38}\x{10B56}-\x{10B57}\x{10B73}-\x{10B77}\x{10B92}-\x{10B98}\x{10B9D}-\x{10BA8}\x{10BB0}-\x{10BFF}\x{10C49}-\x{10C7F}\x{10CB3}-\x{10CBF}\x{10CF3}-\x{10CF9}\x{10D28}-\x{10D2F}\x{10D3A}-\x{10D3F}\x{10D66}-\x{10D68}\x{10D86}-\x{10D8D}\x{10D90}-\x{10E5F}\x{10E7F}\x{10EAA}\x{10EAE}-\x{10EAF}\x{10EB2}-\x{10EC1}\x{10EC5}-\x{10EFB}\x{10F28}-\x{10F2F}\x{10F5A}-\x{10F6F}\x{10F8A}-\x{10FAF}\x{10FCC}-\x{10FDF}\x{10FF7}-\x{10FFF}\x{1104E}-\x{11051}\x{11076}-\x{1107E}\x{110C3}-\x{110CC}\x{110CE}-\x{110CF}\x{110E9}-\x{110EF}\x{110FA}-\x{110FF}\x{11135}\x{11148}-\x{1114F}\x{11177}-\x{1117F}\x{111E0}\x{111F5}-\x{111FF}\x{11212}\x{11242}-\x{1127F}\x{11287}\x{11289}\x{1128E}\x{1129E}\x{112AA}-\x{112AF}\x{112EB}-\x{112EF}\x{112FA}-\x{112FF}\x{11304}\x{1130D}-\x{1130E}\x{11311}-\x{11312}\x{11329}\x{11331}\x{11334}\x{1133A}\x{11345}-\x{11346}\x{11349}-\x{1134A}\x{1134E}-\x{1134F}\x{11351}-\x{11356}\x{11358}-\x{1135C}\x{11364}-\x{11365}\x{1136D}-\x{1136F}\x{11375}-\x{1137F}\x{1138A}\x{1138C}-\x{1138D}\x{1138F}\x{113B6}\x{113C1}\x{113C3}-\x{113C4}\x{113C6}\x{113CB}\x{113D6}\x{113D9}-\x{113E0}\x{113E3}-\x{113FF}\x{1145C}\x{11462}-\x{1147F}\x{114C8}-\x{114CF}\x{114DA}-\x{1157F}\x{115B6}-\x{115B7}\x{115DE}-\x{115FF}\x{11645}-\x{1164F}\x{1165A}-\x{1165F}\x{1166D}-\x{1167F}\x{116BA}-\x{116BF}\x{116CA}-\x{116CF}\x{116E4}-\x{116FF}\x{1171B}-\x{1171C}\x{1172C}-\x{1172F}\x{11747}-\x{117FF}\x{1183C}-\x{1189F}\x{118F3}-\x{118FE}\x{11907}-\x{11908}\x{1190A}-\x{1190B}\x{11914}\x{11917}\x{11936}\x{11939}-\x{1193A}\x{11947}-\x{1194F}\x{1195A}-\x{1199F}\x{119A8}-\x{119A9}\x{119D8}-\x{119D9}\x{119E5}-\x{119FF}\x{11A48}-\x{11A4F}\x{11AA3}-\x{11AAF}\x{11AF9}-\x{11AFF}\x{11B0A}-\x{11BBF}\x{11BE2}-\x{11BEF}\x{11BFA}-\x{11BFF}\x{11C09}\x{11C37}\x{11C46}-\x{11C4F}\x{11C6D}-\x{11C6F}\x{11C90}-\x{11C91}\x{11CA8}\x{11CB7}-\x{11CFF}\x{11D07}\x{11D0A}\x{11D37}-\x{11D39}\x{11D3B}\x{11D3E}\x{11D48}-\x{11D4F}\x{11D5A}-\x{11D5F}\x{11D66}\x{11D69}\x{11D8F}\x{11D92}\x{11D99}-\x{11D9F}\x{11DAA}-\x{11EDF}\x{11EF9}-\x{11EFF}\x{11F11}\x{11F3B}-\x{11F3D}\x{11F5B}-\x{11FAF}\x{11FB1}-\x{11FBF}\x{11FF2}-\x{11FFE}\x{1239A}-\x{123FF}\x{1246F}\x{12475}-\x{1247F}\x{12544}-\x{12F8F}\x{12FF3}-\x{12FFF}\x{13456}-\x{1345F}\x{143FB}-\x{143FF}\x{14647}-\x{160FF}\x{1613A}-\x{167FF}\x{16A39}-\x{16A3F}\x{16A5F}\x{16A6A}-\x{16A6D}\x{16ABF}\x{16ACA}-\x{16ACF}\x{16AEE}-\x{16AEF}\x{16AF6}-\x{16AFF}\x{16B46}-\x{16B4F}\x{16B5A}\x{16B62}\x{16B78}-\x{16B7C}\x{16B90}-\x{16D3F}\x{16D7A}-\x{16E3F}\x{16E9B}-\x{16EFF}\x{16F4B}-\x{16F4E}\x{16F88}-\x{16F8E}\x{16FA0}-\x{16FDF}\x{16FE5}-\x{16FEF}\x{16FF2}-\x{16FFF}\x{187F8}-\x{187FF}\x{18CD6}-\x{18CFE}\x{18D09}-\x{1AFEF}\x{1AFF4}\x{1AFFC}\x{1AFFF}\x{1B123}-\x{1B131}\x{1B133}-\x{1B14F}\x{1B153}-\x{1B154}\x{1B156}-\x{1B163}\x{1B168}-\x{1B16F}\x{1B2FC}-\x{1BBFF}\x{1BC6B}-\x{1BC6F}\x{1BC7D}-\x{1BC7F}\x{1BC89}-\x{1BC8F}\x{1BC9A}-\x{1BC9B}\x{1BCA4}-\x{1CBFF}\x{1CCFA}-\x{1CCFF}\x{1CEB4}-\x{1CEFF}\x{1CF2E}-\x{1CF2F}\x{1CF47}-\x{1CF4F}\x{1CFC4}-\x{1CFFF}\x{1D0F6}-\x{1D0FF}\x{1D127}-\x{1D128}\x{1D1EB}-\x{1D1FF}\x{1D246}-\x{1D2BF}\x{1D2D4}-\x{1D2DF}\x{1D2F4}-\x{1D2FF}\x{1D357}-\x{1D35F}\x{1D379}-\x{1D3FF}\x{1D455}\x{1D49D}\x{1D4A0}-\x{1D4A1}\x{1D4A3}-\x{1D4A4}\x{1D4A7}-\x{1D4A8}\x{1D4AD}\x{1D4BA}\x{1D4BC}\x{1D4C4}\x{1D506}\x{1D50B}-\x{1D50C}\x{1D515}\x{1D51D}\x{1D53A}\x{1D53F}\x{1D545}\x{1D547}-\x{1D549}\x{1D551}\x{1D6A6}-\x{1D6A7}\x{1D7CC}-\x{1D7CD}\x{1DA8C}-\x{1DA9A}\x{1DAA0}\x{1DAB0}-\x{1DEFF}\x{1DF1F}-\x{1DF24}\x{1DF2B}-\x{1DFFF}\x{1E007}\x{1E019}-\x{1E01A}\x{1E022}\x{1E025}\x{1E02B}-\x{1E02F}\x{1E06E}-\x{1E08E}\x{1E090}-\x{1E0FF}\x{1E12D}-\x{1E12F}\x{1E13E}-\x{1E13F}\x{1E14A}-\x{1E14D}\x{1E150}-\x{1E28F}\x{1E2AF}-\x{1E2BF}\x{1E2FA}-\x{1E2FE}\x{1E300}-\x{1E4CF}\x{1E4FA}-\x{1E5CF}\x{1E5FB}-\x{1E5FE}\x{1E600}-\x{1E7DF}\x{1E7E7}\x{1E7EC}\x{1E7EF}\x{1E7FF}\x{1E8C5}-\x{1E8C6}\x{1E8D7}-\x{1E8FF}\x{1E94C}-\x{1E94F}\x{1E95A}-\x{1E95D}\x{1E960}-\x{1EC70}\x{1ECB5}-\x{1ED00}\x{1ED3E}-\x{1EDFF}\x{1EE04}\x{1EE20}\x{1EE23}\x{1EE25}-\x{1EE26}\x{1EE28}\x{1EE33}\x{1EE38}\x{1EE3A}\x{1EE3C}-\x{1EE41}\x{1EE43}-\x{1EE46}\x{1EE48}\x{1EE4A}\x{1EE4C}\x{1EE50}\x{1EE53}\x{1EE55}-\x{1EE56}\x{1EE58}\x{1EE5A}\x{1EE5C}\x{1EE5E}\x{1EE60}\x{1EE63}\x{1EE65}-\x{1EE66}\x{1EE6B}\x{1EE73}\x{1EE78}\x{1EE7D}\x{1EE7F}\x{1EE8A}\x{1EE9C}-\x{1EEA0}\x{1EEA4}\x{1EEAA}\x{1EEBC}-\x{1EEEF}\x{1EEF2}-\x{1EFFF}\x{1F02C}-\x{1F02F}\x{1F094}-\x{1F09F}\x{1F0AF}-\x{1F0B0}\x{1F0C0}\x{1F0D0}\x{1F0F6}-\x{1F0FF}\x{1F1AE}-\x{1F1E5}\x{1F203}-\x{1F20F}\x{1F23C}-\x{1F23F}\x{1F249}-\x{1F24F}\x{1F252}-\x{1F25F}\x{1F266}-\x{1F2FF}\x{1F6D8}-\x{1F6DB}\x{1F6ED}-\x{1F6EF}\x{1F6FD}-\x{1F6FF}\x{1F777}-\x{1F77A}\x{1F7DA}-\x{1F7DF}\x{1F7EC}-\x{1F7EF}\x{1F7F1}-\x{1F7FF}\x{1F80C}-\x{1F80F}\x{1F848}-\x{1F84F}\x{1F85A}-\x{1F85F}\x{1F888}-\x{1F88F}\x{1F8AE}-\x{1F8AF}\x{1F8BC}-\x{1F8BF}\x{1F8C2}-\x{1F8FF}\x{1FA54}-\x{1FA5F}\x{1FA6E}-\x{1FA6F}\x{1FA7D}-\x{1FA7F}\x{1FA8A}-\x{1FA8E}\x{1FAC7}-\x{1FACD}\x{1FADD}-\x{1FADE}\x{1FAEA}-\x{1FAEF}\x{1FAF9}-\x{1FAFF}\x{1FB93}\x{1FBFA}-\x{1FFFF}\x{2A6E0}-\x{2A6FF}\x{2B73A}-\x{2B73F}\x{2B81E}-\x{2B81F}\x{2CEA2}-\x{2CEAF}\x{2EBE1}-\x{2EBEF}\x{2EE5E}-\x{2F7FF}\x{2FA1E}-\x{2FFFF}\x{3134B}-\x{3134F}\x{323B0}-\x{E0000}\x{E0002}-\x{E001F}\x{E0080}-\x{E00FF}\x{E01F0}-\x{EFFFF}\x{FFFFE}-\x{FFFFF}\x{10FFFE}-\x{10FFFF}]|^[^\p{L}\p{N}]*$|\p{M}{4}|(^|[^\p{S}0-9#*])\x{fe0f}|(^|[^\p{S}\x{fe0f}])\x{200d}|\x{200d}($|[^\p{S}])",
);

/// `title` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordQueueTaskV1Title(String);

impl RecordQueueTaskV1Title {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordQueueTaskV1Title", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=120).contains(&length)
            && RECORD_QUEUE_TASK_V1_TITLE_PATTERN.matches(text)
            && !RECORD_QUEUE_TASK_V1_TITLE_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordQueueTaskV1Title {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordQueueTaskV1Title> for String {
    fn from(value: RecordQueueTaskV1Title) -> String {
        value.0
    }
}

impl fmt::Display for RecordQueueTaskV1Title {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordQueueTaskV1Title {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

static RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_PATTERN: Pattern =
    Pattern::new(r"^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9_-])?$");
static RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_EXCLUDED: Pattern =
    Pattern::new(r"(^((con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|$)))|\.\.");

/// `workspace_alias` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordQueueTaskV1WorkspaceAlias(String);

impl RecordQueueTaskV1WorkspaceAlias {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordQueueTaskV1WorkspaceAlias", text))
        }
    }

    fn accepts(text: &str) -> bool {
        RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_PATTERN.matches(text)
            && !RECORD_QUEUE_TASK_V1_WORKSPACE_ALIAS_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordQueueTaskV1WorkspaceAlias {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordQueueTaskV1WorkspaceAlias> for String {
    fn from(value: RecordQueueTaskV1WorkspaceAlias) -> String {
        value.0
    }
}

impl fmt::Display for RecordQueueTaskV1WorkspaceAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordQueueTaskV1WorkspaceAlias {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `attempts` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV1Attempts(u64);

impl RecordQueueTaskV1Attempts {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (0..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1Attempts",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV1Attempts {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1Attempts> for u64 {
    fn from(value: RecordQueueTaskV1Attempts) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1Attempts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// `attempt_limit` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u16")]
pub struct RecordQueueTaskV1AttemptLimit(u16);

impl RecordQueueTaskV1AttemptLimit {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1AttemptLimit",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for RecordQueueTaskV1AttemptLimit {
    type Error = ContractError;

    fn try_from(value: u16) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1AttemptLimit> for u16 {
    fn from(value: RecordQueueTaskV1AttemptLimit) -> u16 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1AttemptLimit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

static RECORD_QUEUE_TASK_V1_BRANCH_PATTERN: Pattern =
    Pattern::new(r"^(feature|fix)/[a-z0-9][a-z0-9._-]*(/[a-z0-9][a-z0-9._-]*)*$");
static RECORD_QUEUE_TASK_V1_BRANCH_EXCLUDED: Pattern =
    Pattern::new(r"\.\.|\.lock(/|$)|\.(/|$)|(^|/)(con|prn|aux|nul|com[0-9]|lpt[0-9])(\.|/|$)");

/// `branch` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordQueueTaskV1Branch(String);

impl RecordQueueTaskV1Branch {
    pub fn new(text: impl Into<String>) -> Result<Self, ContractError> {
        let text = text.into();
        if Self::accepts(&text) {
            Ok(Self(text))
        } else {
            Err(ContractError::new("RecordQueueTaskV1Branch", text))
        }
    }

    fn accepts(text: &str) -> bool {
        let length = text.chars().count();
        (1..=255).contains(&length)
            && RECORD_QUEUE_TASK_V1_BRANCH_PATTERN.matches(text)
            && !RECORD_QUEUE_TASK_V1_BRANCH_EXCLUDED.matches(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordQueueTaskV1Branch {
    type Error = ContractError;

    fn try_from(text: String) -> Result<Self, ContractError> {
        Self::new(text)
    }
}

impl From<RecordQueueTaskV1Branch> for String {
    fn from(value: RecordQueueTaskV1Branch) -> String {
        value.0
    }
}

impl fmt::Display for RecordQueueTaskV1Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RecordQueueTaskV1Branch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// `pull_request` in `urn:ads:record:queue-task:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct RecordQueueTaskV1PullRequest(u64);

impl RecordQueueTaskV1PullRequest {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if (1..=9007199254740991).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::new(
                "RecordQueueTaskV1PullRequest",
                value.to_string(),
            ))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for RecordQueueTaskV1PullRequest {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, ContractError> {
        Self::new(value)
    }
}

impl From<RecordQueueTaskV1PullRequest> for u64 {
    fn from(value: RecordQueueTaskV1PullRequest) -> u64 {
        value.0
    }
}

impl Serialize for RecordQueueTaskV1PullRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
