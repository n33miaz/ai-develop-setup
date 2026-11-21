// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.error-code.v3.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::support;

/// Código de erro da API local. Códigos que a API local atribui às próprias respostas JSON-RPC. A versão 3 acrescenta à :2 os códigos das aprovações do portão de ações; a :2 e a :1 continuam valendo para quem só conhece os códigos anteriores.
///
/// `urn:ads:contract:local-api.error-code:3`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiErrorCodeV3 {
    MethodNotFound(ContractLocalApiErrorCodeV3MethodNotFound),
    InvalidParams(ContractLocalApiErrorCodeV3InvalidParams),
    InternalError(ContractLocalApiErrorCodeV3InternalError),
    CapacityExceeded(ContractLocalApiErrorCodeV3CapacityExceeded),
    SubscriptionReplayUnavailable(ContractLocalApiErrorCodeV3SubscriptionReplayUnavailable),
    RemoteOriginRefused(ContractLocalApiErrorCodeV3RemoteOriginRefused),
    HarnessNotConfigured(ContractLocalApiErrorCodeV3HarnessNotConfigured),
    Conflict(ContractLocalApiErrorCodeV3Conflict),
    SessionClosePending(ContractLocalApiErrorCodeV3SessionClosePending),
    SessionSetupFailed(ContractLocalApiErrorCodeV3SessionSetupFailed),
    QueueNotConfigured(ContractLocalApiErrorCodeV3QueueNotConfigured),
    QueueAlreadyEnabled(ContractLocalApiErrorCodeV3QueueAlreadyEnabled),
    ApprovalNotConfigured(ContractLocalApiErrorCodeV3ApprovalNotConfigured),
    ApprovalNotFound(ContractLocalApiErrorCodeV3ApprovalNotFound),
    ApprovalAlreadyAnswered(ContractLocalApiErrorCodeV3ApprovalAlreadyAnswered),
    ApprovalStale(ContractLocalApiErrorCodeV3ApprovalStale),
    ApprovalPolicyChanged(ContractLocalApiErrorCodeV3ApprovalPolicyChanged),
    ApprovalOutOfOrder(ContractLocalApiErrorCodeV3ApprovalOutOfOrder),
}

/// O método solicitado é desconhecido.
///
/// `$defs.method_not_found` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32601`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3MethodNotFound;

impl ContractLocalApiErrorCodeV3MethodNotFound {
    pub const VALUE: i64 = -32601;
}

impl Serialize for ContractLocalApiErrorCodeV3MethodNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3MethodNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O comando ou seus parâmetros são inválidos para a API local.
///
/// `$defs.invalid_params` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32602`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3InvalidParams;

impl ContractLocalApiErrorCodeV3InvalidParams {
    pub const VALUE: i64 = -32602;
}

impl Serialize for ContractLocalApiErrorCodeV3InvalidParams {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3InvalidParams {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O serviço está indisponível, inclusive quando o prazo do host se esgota.
///
/// `$defs.internal_error` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32603`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3InternalError;

impl ContractLocalApiErrorCodeV3InternalError {
    pub const VALUE: i64 = -32603;
}

impl Serialize for ContractLocalApiErrorCodeV3InternalError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3InternalError {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Um limite de capacidade foi atingido.
///
/// `$defs.capacity_exceeded` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32003`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3CapacityExceeded;

impl ContractLocalApiErrorCodeV3CapacityExceeded {
    pub const VALUE: i64 = -32003;
}

impl Serialize for ContractLocalApiErrorCodeV3CapacityExceeded {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3CapacityExceeded {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O replay solicitado está indisponível ou o stream foi invalidado ou descartado.
///
/// `$defs.subscription_replay_unavailable` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32004`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3SubscriptionReplayUnavailable;

impl ContractLocalApiErrorCodeV3SubscriptionReplayUnavailable {
    pub const VALUE: i64 = -32004;
}

impl Serialize for ContractLocalApiErrorCodeV3SubscriptionReplayUnavailable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3SubscriptionReplayUnavailable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A API local recusou um comando de origem remota.
///
/// `$defs.remote_origin_refused` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32010`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3RemoteOriginRefused;

impl ContractLocalApiErrorCodeV3RemoteOriginRefused {
    pub const VALUE: i64 = -32010;
}

impl Serialize for ContractLocalApiErrorCodeV3RemoteOriginRefused {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3RemoteOriginRefused {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ferramenta solicitada não tem programa configurado.
///
/// `$defs.harness_not_configured` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32011`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3HarnessNotConfigured;

impl ContractLocalApiErrorCodeV3HarnessNotConfigured {
    pub const VALUE: i64 = -32011;
}

impl Serialize for ContractLocalApiErrorCodeV3HarnessNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3HarnessNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A chave de idempotência conflita com outro pedido ou o id da sessão está reservado.
///
/// `$defs.conflict` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32012`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3Conflict;

impl ContractLocalApiErrorCodeV3Conflict {
    pub const VALUE: i64 = -32012;
}

impl Serialize for ContractLocalApiErrorCodeV3Conflict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3Conflict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O fechamento automático esgotou as tentativas e ainda depende de um session.close.
///
/// `$defs.session_close_pending` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32013`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3SessionClosePending;

impl ContractLocalApiErrorCodeV3SessionClosePending {
    pub const VALUE: i64 = -32013;
}

impl Serialize for ContractLocalApiErrorCodeV3SessionClosePending {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3SessionClosePending {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A preparação falhou após a abertura do terminal e o fechamento foi agendado.
///
/// `$defs.session_setup_failed` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32014`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3SessionSetupFailed;

impl ContractLocalApiErrorCodeV3SessionSetupFailed {
    pub const VALUE: i64 = -32014;
}

impl Serialize for ContractLocalApiErrorCodeV3SessionSetupFailed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3SessionSetupFailed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve a fila pedida: nenhuma fila ligada ao log de auditoria ou id de fila desconhecido.
///
/// `$defs.queue_not_configured` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32015`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3QueueNotConfigured;

impl ContractLocalApiErrorCodeV3QueueNotConfigured {
    pub const VALUE: i64 = -32015;
}

impl Serialize for ContractLocalApiErrorCodeV3QueueNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3QueueNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila já está ligada; ligar de novo não registra nada.
///
/// `$defs.queue_already_enabled` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32016`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3QueueAlreadyEnabled;

impl ContractLocalApiErrorCodeV3QueueAlreadyEnabled {
    pub const VALUE: i64 = -32016;
}

impl Serialize for ContractLocalApiErrorCodeV3QueueAlreadyEnabled {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3QueueAlreadyEnabled {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve aprovações: nenhum portão ligado ao log de auditoria.
///
/// `$defs.approval_not_configured` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32017`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3ApprovalNotConfigured;

impl ContractLocalApiErrorCodeV3ApprovalNotConfigured {
    pub const VALUE: i64 = -32017;
}

impl Serialize for ContractLocalApiErrorCodeV3ApprovalNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3ApprovalNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Nenhuma ação conhecida do portão com esse id.
///
/// `$defs.approval_not_found` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32018`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3ApprovalNotFound;

impl ContractLocalApiErrorCodeV3ApprovalNotFound {
    pub const VALUE: i64 = -32018;
}

impl Serialize for ContractLocalApiErrorCodeV3ApprovalNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3ApprovalNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ação não aguarda mais resposta: já foi aprovada, negada, expirou ou terminou.
///
/// `$defs.approval_already_answered` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32019`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3ApprovalAlreadyAnswered;

impl ContractLocalApiErrorCodeV3ApprovalAlreadyAnswered {
    pub const VALUE: i64 = -32019;
}

impl Serialize for ContractLocalApiErrorCodeV3ApprovalAlreadyAnswered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3ApprovalAlreadyAnswered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A aprovação ou o hash do plano não são os da ação, ou a validade da aprovação acabou.
///
/// `$defs.approval_stale` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32020`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3ApprovalStale;

impl ContractLocalApiErrorCodeV3ApprovalStale {
    pub const VALUE: i64 = -32020;
}

impl Serialize for ContractLocalApiErrorCodeV3ApprovalStale {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3ApprovalStale {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A política ou a tolerância do relógio mudou desde o plano: o portão registrou a negação.
///
/// `$defs.approval_policy_changed` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32021`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3ApprovalPolicyChanged;

impl ContractLocalApiErrorCodeV3ApprovalPolicyChanged {
    pub const VALUE: i64 = -32021;
}

impl Serialize for ContractLocalApiErrorCodeV3ApprovalPolicyChanged {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3ApprovalPolicyChanged {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O relógio do núcleo está antes do último passo da ação; nada foi decidido e a resposta pode ser repetida.
///
/// `$defs.approval_out_of_order` in `urn:ads:contract:local-api.error-code:3`.
///
/// Always `-32022`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV3ApprovalOutOfOrder;

impl ContractLocalApiErrorCodeV3ApprovalOutOfOrder {
    pub const VALUE: i64 = -32022;
}

impl Serialize for ContractLocalApiErrorCodeV3ApprovalOutOfOrder {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV3ApprovalOutOfOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
