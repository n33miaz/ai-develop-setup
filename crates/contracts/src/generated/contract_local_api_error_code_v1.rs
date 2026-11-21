// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.error-code.v1.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::support;

/// Código de erro da API local. Códigos que a API local atribui às próprias respostas JSON-RPC.
///
/// `urn:ads:contract:local-api.error-code:1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiErrorCodeV1 {
    MethodNotFound(ContractLocalApiErrorCodeV1MethodNotFound),
    InvalidParams(ContractLocalApiErrorCodeV1InvalidParams),
    InternalError(ContractLocalApiErrorCodeV1InternalError),
    CapacityExceeded(ContractLocalApiErrorCodeV1CapacityExceeded),
    SubscriptionReplayUnavailable(ContractLocalApiErrorCodeV1SubscriptionReplayUnavailable),
    RemoteOriginRefused(ContractLocalApiErrorCodeV1RemoteOriginRefused),
    HarnessNotConfigured(ContractLocalApiErrorCodeV1HarnessNotConfigured),
    Conflict(ContractLocalApiErrorCodeV1Conflict),
    SessionClosePending(ContractLocalApiErrorCodeV1SessionClosePending),
    SessionSetupFailed(ContractLocalApiErrorCodeV1SessionSetupFailed),
    QueueNotConfigured(ContractLocalApiErrorCodeV1QueueNotConfigured),
    QueueAlreadyEnabled(ContractLocalApiErrorCodeV1QueueAlreadyEnabled),
}

/// O método solicitado é desconhecido.
///
/// `$defs.method_not_found` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32601`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1MethodNotFound;

impl ContractLocalApiErrorCodeV1MethodNotFound {
    pub const VALUE: i64 = -32601;
}

impl Serialize for ContractLocalApiErrorCodeV1MethodNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1MethodNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O comando ou seus parâmetros são inválidos para a API local.
///
/// `$defs.invalid_params` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32602`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1InvalidParams;

impl ContractLocalApiErrorCodeV1InvalidParams {
    pub const VALUE: i64 = -32602;
}

impl Serialize for ContractLocalApiErrorCodeV1InvalidParams {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1InvalidParams {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O serviço está indisponível, inclusive quando o prazo do host se esgota.
///
/// `$defs.internal_error` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32603`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1InternalError;

impl ContractLocalApiErrorCodeV1InternalError {
    pub const VALUE: i64 = -32603;
}

impl Serialize for ContractLocalApiErrorCodeV1InternalError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1InternalError {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Um limite de capacidade foi atingido.
///
/// `$defs.capacity_exceeded` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32003`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1CapacityExceeded;

impl ContractLocalApiErrorCodeV1CapacityExceeded {
    pub const VALUE: i64 = -32003;
}

impl Serialize for ContractLocalApiErrorCodeV1CapacityExceeded {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1CapacityExceeded {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O replay solicitado está indisponível ou o stream foi invalidado ou descartado.
///
/// `$defs.subscription_replay_unavailable` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32004`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1SubscriptionReplayUnavailable;

impl ContractLocalApiErrorCodeV1SubscriptionReplayUnavailable {
    pub const VALUE: i64 = -32004;
}

impl Serialize for ContractLocalApiErrorCodeV1SubscriptionReplayUnavailable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1SubscriptionReplayUnavailable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A API local recusou um comando de origem remota.
///
/// `$defs.remote_origin_refused` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32010`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1RemoteOriginRefused;

impl ContractLocalApiErrorCodeV1RemoteOriginRefused {
    pub const VALUE: i64 = -32010;
}

impl Serialize for ContractLocalApiErrorCodeV1RemoteOriginRefused {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1RemoteOriginRefused {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ferramenta solicitada não tem programa configurado.
///
/// `$defs.harness_not_configured` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32011`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1HarnessNotConfigured;

impl ContractLocalApiErrorCodeV1HarnessNotConfigured {
    pub const VALUE: i64 = -32011;
}

impl Serialize for ContractLocalApiErrorCodeV1HarnessNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1HarnessNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A chave de idempotência conflita com outro pedido ou o id da sessão está reservado.
///
/// `$defs.conflict` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32012`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1Conflict;

impl ContractLocalApiErrorCodeV1Conflict {
    pub const VALUE: i64 = -32012;
}

impl Serialize for ContractLocalApiErrorCodeV1Conflict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1Conflict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O fechamento automático esgotou as tentativas e ainda depende de um session.close.
///
/// `$defs.session_close_pending` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32013`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1SessionClosePending;

impl ContractLocalApiErrorCodeV1SessionClosePending {
    pub const VALUE: i64 = -32013;
}

impl Serialize for ContractLocalApiErrorCodeV1SessionClosePending {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1SessionClosePending {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A preparação falhou após a abertura do terminal e o fechamento foi agendado.
///
/// `$defs.session_setup_failed` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32014`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1SessionSetupFailed;

impl ContractLocalApiErrorCodeV1SessionSetupFailed {
    pub const VALUE: i64 = -32014;
}

impl Serialize for ContractLocalApiErrorCodeV1SessionSetupFailed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1SessionSetupFailed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve a fila pedida: nenhuma fila ligada ao log de auditoria ou id de fila desconhecido.
///
/// `$defs.queue_not_configured` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32015`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1QueueNotConfigured;

impl ContractLocalApiErrorCodeV1QueueNotConfigured {
    pub const VALUE: i64 = -32015;
}

impl Serialize for ContractLocalApiErrorCodeV1QueueNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1QueueNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila já está ligada; ligar de novo não registra nada.
///
/// `$defs.queue_already_enabled` in `urn:ads:contract:local-api.error-code:1`.
///
/// Always `-32016`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV1QueueAlreadyEnabled;

impl ContractLocalApiErrorCodeV1QueueAlreadyEnabled {
    pub const VALUE: i64 = -32016;
}

impl Serialize for ContractLocalApiErrorCodeV1QueueAlreadyEnabled {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV1QueueAlreadyEnabled {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
