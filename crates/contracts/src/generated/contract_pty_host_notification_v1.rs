// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.pty-host.notification.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::common_id_v1::CommonIdV1;
use crate::support;

/// Aviso do host de PTY. Notificação JSON-RPC 2.0 (sem id, sem resposta) do host de PTY ao núcleo: o processo de uma sessão terminou, ou uma assinatura foi encerrada. last_seq é o último pedaço numerado (0 se não houve saída); o núcleo reassina a partir de last_seq + 1 sem perder nada.
///
/// `urn:ads:contract:pty-host.notification:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum ContractPtyHostNotificationV1 {
    #[serde(rename = "session.exited")]
    SessionExited(ContractPtyHostNotificationV1SessionExited),
    #[serde(rename = "subscription.closed")]
    SubscriptionClosed(ContractPtyHostNotificationV1SubscriptionClosed),
}

/// `session.exited` in `urn:ads:contract:pty-host.notification:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostNotificationV1SessionExited {
    pub jsonrpc: ContractPtyHostNotificationV1SessionExitedJsonrpc,
    pub params: ContractPtyHostNotificationV1SessionExitedParams,
}

/// `session.exited.jsonrpc` in `urn:ads:contract:pty-host.notification:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostNotificationV1SessionExitedJsonrpc;

impl ContractPtyHostNotificationV1SessionExitedJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostNotificationV1SessionExitedJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostNotificationV1SessionExitedJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `session.exited.params` in `urn:ads:contract:pty-host.notification:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostNotificationV1SessionExitedParams {
    pub session_id: CommonIdV1,
    #[serde(deserialize_with = "support::nullable")]
    pub exit_code: Option<i64>,
    pub last_seq: u64,
}

/// `subscription.closed` in `urn:ads:contract:pty-host.notification:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostNotificationV1SubscriptionClosed {
    pub jsonrpc: ContractPtyHostNotificationV1SubscriptionClosedJsonrpc,
    pub params: ContractPtyHostNotificationV1SubscriptionClosedParams,
}

/// `subscription.closed.jsonrpc` in `urn:ads:contract:pty-host.notification:1`.
///
/// Always `"2.0"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractPtyHostNotificationV1SubscriptionClosedJsonrpc;

impl ContractPtyHostNotificationV1SubscriptionClosedJsonrpc {
    pub const VALUE: &str = "2.0";
}

impl Serialize for ContractPtyHostNotificationV1SubscriptionClosedJsonrpc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractPtyHostNotificationV1SubscriptionClosedJsonrpc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_str(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// `subscription.closed.params` in `urn:ads:contract:pty-host.notification:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPtyHostNotificationV1SubscriptionClosedParams {
    pub session_id: CommonIdV1,
    pub reason: ContractPtyHostNotificationV1SubscriptionClosedParamsReason,
    pub last_seq: u64,
}

/// `subscription.closed.params.reason` in `urn:ads:contract:pty-host.notification:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractPtyHostNotificationV1SubscriptionClosedParamsReason {
    #[serde(rename = "slow_subscriber")]
    SlowSubscriber,
    #[serde(rename = "session_closed")]
    SessionClosed,
}

impl ContractPtyHostNotificationV1SubscriptionClosedParamsReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SlowSubscriber => "slow_subscriber",
            Self::SessionClosed => "session_closed",
        }
    }
}
