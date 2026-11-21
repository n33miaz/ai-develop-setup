// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.error-code.v2.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::support;

/// Código de erro da API local. Códigos que a API local atribui às próprias respostas JSON-RPC. A versão 2 acrescenta à :1 os códigos da fila de tarefas; a :1 continua valendo para quem só conhece os códigos anteriores.
///
/// `urn:ads:contract:local-api.error-code:2`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiErrorCodeV2 {
    MethodNotFound(ContractLocalApiErrorCodeV2MethodNotFound),
    InvalidParams(ContractLocalApiErrorCodeV2InvalidParams),
    InternalError(ContractLocalApiErrorCodeV2InternalError),
    CapacityExceeded(ContractLocalApiErrorCodeV2CapacityExceeded),
    SubscriptionReplayUnavailable(ContractLocalApiErrorCodeV2SubscriptionReplayUnavailable),
    RemoteOriginRefused(ContractLocalApiErrorCodeV2RemoteOriginRefused),
    HarnessNotConfigured(ContractLocalApiErrorCodeV2HarnessNotConfigured),
    Conflict(ContractLocalApiErrorCodeV2Conflict),
    SessionClosePending(ContractLocalApiErrorCodeV2SessionClosePending),
    SessionSetupFailed(ContractLocalApiErrorCodeV2SessionSetupFailed),
    QueueNotConfigured(ContractLocalApiErrorCodeV2QueueNotConfigured),
    QueueAlreadyEnabled(ContractLocalApiErrorCodeV2QueueAlreadyEnabled),
}

/// O método solicitado é desconhecido.
///
/// `$defs.method_not_found` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32601`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2MethodNotFound;

impl ContractLocalApiErrorCodeV2MethodNotFound {
    pub const VALUE: i64 = -32601;
}

impl Serialize for ContractLocalApiErrorCodeV2MethodNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2MethodNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O comando ou seus parâmetros são inválidos para a API local.
///
/// `$defs.invalid_params` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32602`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2InvalidParams;

impl ContractLocalApiErrorCodeV2InvalidParams {
    pub const VALUE: i64 = -32602;
}

impl Serialize for ContractLocalApiErrorCodeV2InvalidParams {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2InvalidParams {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O serviço está indisponível, inclusive quando o prazo do host se esgota.
///
/// `$defs.internal_error` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32603`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2InternalError;

impl ContractLocalApiErrorCodeV2InternalError {
    pub const VALUE: i64 = -32603;
}

impl Serialize for ContractLocalApiErrorCodeV2InternalError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2InternalError {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Um limite de capacidade foi atingido.
///
/// `$defs.capacity_exceeded` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32003`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2CapacityExceeded;

impl ContractLocalApiErrorCodeV2CapacityExceeded {
    pub const VALUE: i64 = -32003;
}

impl Serialize for ContractLocalApiErrorCodeV2CapacityExceeded {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2CapacityExceeded {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O replay solicitado está indisponível ou o stream foi invalidado ou descartado.
///
/// `$defs.subscription_replay_unavailable` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32004`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2SubscriptionReplayUnavailable;

impl ContractLocalApiErrorCodeV2SubscriptionReplayUnavailable {
    pub const VALUE: i64 = -32004;
}

impl Serialize for ContractLocalApiErrorCodeV2SubscriptionReplayUnavailable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2SubscriptionReplayUnavailable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A API local recusou um comando de origem remota.
///
/// `$defs.remote_origin_refused` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32010`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2RemoteOriginRefused;

impl ContractLocalApiErrorCodeV2RemoteOriginRefused {
    pub const VALUE: i64 = -32010;
}

impl Serialize for ContractLocalApiErrorCodeV2RemoteOriginRefused {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2RemoteOriginRefused {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ferramenta solicitada não tem programa configurado.
///
/// `$defs.harness_not_configured` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32011`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2HarnessNotConfigured;

impl ContractLocalApiErrorCodeV2HarnessNotConfigured {
    pub const VALUE: i64 = -32011;
}

impl Serialize for ContractLocalApiErrorCodeV2HarnessNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2HarnessNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A chave de idempotência conflita com outro pedido ou o id da sessão está reservado.
///
/// `$defs.conflict` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32012`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2Conflict;

impl ContractLocalApiErrorCodeV2Conflict {
    pub const VALUE: i64 = -32012;
}

impl Serialize for ContractLocalApiErrorCodeV2Conflict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2Conflict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O fechamento automático esgotou as tentativas e ainda depende de um session.close.
///
/// `$defs.session_close_pending` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32013`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2SessionClosePending;

impl ContractLocalApiErrorCodeV2SessionClosePending {
    pub const VALUE: i64 = -32013;
}

impl Serialize for ContractLocalApiErrorCodeV2SessionClosePending {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2SessionClosePending {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A preparação falhou após a abertura do terminal e o fechamento foi agendado.
///
/// `$defs.session_setup_failed` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32014`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2SessionSetupFailed;

impl ContractLocalApiErrorCodeV2SessionSetupFailed {
    pub const VALUE: i64 = -32014;
}

impl Serialize for ContractLocalApiErrorCodeV2SessionSetupFailed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2SessionSetupFailed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve a fila pedida: nenhuma fila ligada ao log de auditoria ou id de fila desconhecido.
///
/// `$defs.queue_not_configured` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32015`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2QueueNotConfigured;

impl ContractLocalApiErrorCodeV2QueueNotConfigured {
    pub const VALUE: i64 = -32015;
}

impl Serialize for ContractLocalApiErrorCodeV2QueueNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2QueueNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila já está ligada; ligar de novo não registra nada.
///
/// `$defs.queue_already_enabled` in `urn:ads:contract:local-api.error-code:2`.
///
/// Always `-32016`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV2QueueAlreadyEnabled;

impl ContractLocalApiErrorCodeV2QueueAlreadyEnabled {
    pub const VALUE: i64 = -32016;
}

impl Serialize for ContractLocalApiErrorCodeV2QueueAlreadyEnabled {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV2QueueAlreadyEnabled {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
