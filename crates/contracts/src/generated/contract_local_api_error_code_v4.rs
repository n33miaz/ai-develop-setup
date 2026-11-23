// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.error-code.v4.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::support;

/// Código de erro da API local. Códigos que a API local atribui às próprias respostas JSON-RPC. A versão 4 acrescenta à :3 a recusa de criação de uma fila existente.
///
/// `urn:ads:contract:local-api.error-code:4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiErrorCodeV4 {
    MethodNotFound(ContractLocalApiErrorCodeV4MethodNotFound),
    InvalidParams(ContractLocalApiErrorCodeV4InvalidParams),
    InternalError(ContractLocalApiErrorCodeV4InternalError),
    CapacityExceeded(ContractLocalApiErrorCodeV4CapacityExceeded),
    SubscriptionReplayUnavailable(ContractLocalApiErrorCodeV4SubscriptionReplayUnavailable),
    RemoteOriginRefused(ContractLocalApiErrorCodeV4RemoteOriginRefused),
    HarnessNotConfigured(ContractLocalApiErrorCodeV4HarnessNotConfigured),
    Conflict(ContractLocalApiErrorCodeV4Conflict),
    SessionClosePending(ContractLocalApiErrorCodeV4SessionClosePending),
    SessionSetupFailed(ContractLocalApiErrorCodeV4SessionSetupFailed),
    QueueNotConfigured(ContractLocalApiErrorCodeV4QueueNotConfigured),
    QueueAlreadyEnabled(ContractLocalApiErrorCodeV4QueueAlreadyEnabled),
    ApprovalNotConfigured(ContractLocalApiErrorCodeV4ApprovalNotConfigured),
    ApprovalNotFound(ContractLocalApiErrorCodeV4ApprovalNotFound),
    ApprovalAlreadyAnswered(ContractLocalApiErrorCodeV4ApprovalAlreadyAnswered),
    ApprovalStale(ContractLocalApiErrorCodeV4ApprovalStale),
    ApprovalPolicyChanged(ContractLocalApiErrorCodeV4ApprovalPolicyChanged),
    ApprovalOutOfOrder(ContractLocalApiErrorCodeV4ApprovalOutOfOrder),
    ApprovalNotApprovable(ContractLocalApiErrorCodeV4ApprovalNotApprovable),
    QueueAlreadyCreated(ContractLocalApiErrorCodeV4QueueAlreadyCreated),
}

/// O método solicitado é desconhecido.
///
/// `$defs.method_not_found` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32601`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4MethodNotFound;

impl ContractLocalApiErrorCodeV4MethodNotFound {
    pub const VALUE: i64 = -32601;
}

impl Serialize for ContractLocalApiErrorCodeV4MethodNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4MethodNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O comando ou seus parâmetros são inválidos para a API local.
///
/// `$defs.invalid_params` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32602`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4InvalidParams;

impl ContractLocalApiErrorCodeV4InvalidParams {
    pub const VALUE: i64 = -32602;
}

impl Serialize for ContractLocalApiErrorCodeV4InvalidParams {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4InvalidParams {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O serviço está indisponível, inclusive quando o prazo do host se esgota.
///
/// `$defs.internal_error` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32603`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4InternalError;

impl ContractLocalApiErrorCodeV4InternalError {
    pub const VALUE: i64 = -32603;
}

impl Serialize for ContractLocalApiErrorCodeV4InternalError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4InternalError {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Um limite de capacidade foi atingido.
///
/// `$defs.capacity_exceeded` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32003`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4CapacityExceeded;

impl ContractLocalApiErrorCodeV4CapacityExceeded {
    pub const VALUE: i64 = -32003;
}

impl Serialize for ContractLocalApiErrorCodeV4CapacityExceeded {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4CapacityExceeded {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O replay solicitado está indisponível ou o stream foi invalidado ou descartado.
///
/// `$defs.subscription_replay_unavailable` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32004`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4SubscriptionReplayUnavailable;

impl ContractLocalApiErrorCodeV4SubscriptionReplayUnavailable {
    pub const VALUE: i64 = -32004;
}

impl Serialize for ContractLocalApiErrorCodeV4SubscriptionReplayUnavailable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4SubscriptionReplayUnavailable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A API local recusou um comando de origem remota.
///
/// `$defs.remote_origin_refused` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32010`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4RemoteOriginRefused;

impl ContractLocalApiErrorCodeV4RemoteOriginRefused {
    pub const VALUE: i64 = -32010;
}

impl Serialize for ContractLocalApiErrorCodeV4RemoteOriginRefused {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4RemoteOriginRefused {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ferramenta solicitada não tem programa configurado.
///
/// `$defs.harness_not_configured` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32011`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4HarnessNotConfigured;

impl ContractLocalApiErrorCodeV4HarnessNotConfigured {
    pub const VALUE: i64 = -32011;
}

impl Serialize for ContractLocalApiErrorCodeV4HarnessNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4HarnessNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A chave de idempotência conflita com outro pedido ou o id da sessão está reservado.
///
/// `$defs.conflict` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32012`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4Conflict;

impl ContractLocalApiErrorCodeV4Conflict {
    pub const VALUE: i64 = -32012;
}

impl Serialize for ContractLocalApiErrorCodeV4Conflict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4Conflict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O fechamento automático esgotou as tentativas e ainda depende de um session.close.
///
/// `$defs.session_close_pending` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32013`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4SessionClosePending;

impl ContractLocalApiErrorCodeV4SessionClosePending {
    pub const VALUE: i64 = -32013;
}

impl Serialize for ContractLocalApiErrorCodeV4SessionClosePending {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4SessionClosePending {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A preparação falhou após a abertura do terminal e o fechamento foi agendado.
///
/// `$defs.session_setup_failed` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32014`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4SessionSetupFailed;

impl ContractLocalApiErrorCodeV4SessionSetupFailed {
    pub const VALUE: i64 = -32014;
}

impl Serialize for ContractLocalApiErrorCodeV4SessionSetupFailed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4SessionSetupFailed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve a fila pedida: nenhuma fila ligada ao log de auditoria ou id de fila desconhecido.
///
/// `$defs.queue_not_configured` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32015`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4QueueNotConfigured;

impl ContractLocalApiErrorCodeV4QueueNotConfigured {
    pub const VALUE: i64 = -32015;
}

impl Serialize for ContractLocalApiErrorCodeV4QueueNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4QueueNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila já está ligada; ligar de novo não registra nada.
///
/// `$defs.queue_already_enabled` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32016`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4QueueAlreadyEnabled;

impl ContractLocalApiErrorCodeV4QueueAlreadyEnabled {
    pub const VALUE: i64 = -32016;
}

impl Serialize for ContractLocalApiErrorCodeV4QueueAlreadyEnabled {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4QueueAlreadyEnabled {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve aprovações: nenhum portão ligado ao log de auditoria.
///
/// `$defs.approval_not_configured` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32017`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4ApprovalNotConfigured;

impl ContractLocalApiErrorCodeV4ApprovalNotConfigured {
    pub const VALUE: i64 = -32017;
}

impl Serialize for ContractLocalApiErrorCodeV4ApprovalNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4ApprovalNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Nenhuma ação conhecida do portão com esse id, ou a ação não tem essa aprovação.
///
/// `$defs.approval_not_found` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32018`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4ApprovalNotFound;

impl ContractLocalApiErrorCodeV4ApprovalNotFound {
    pub const VALUE: i64 = -32018;
}

impl Serialize for ContractLocalApiErrorCodeV4ApprovalNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4ApprovalNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ação não aguarda mais resposta: já foi aprovada, negada, expirou ou terminou.
///
/// `$defs.approval_already_answered` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32019`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4ApprovalAlreadyAnswered;

impl ContractLocalApiErrorCodeV4ApprovalAlreadyAnswered {
    pub const VALUE: i64 = -32019;
}

impl Serialize for ContractLocalApiErrorCodeV4ApprovalAlreadyAnswered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4ApprovalAlreadyAnswered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O hash do plano não é o da ação em espera, ou a validade da aprovação acabou (o portão registra a expiração). A resposta fica registrada.
///
/// `$defs.approval_stale` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32020`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4ApprovalStale;

impl ContractLocalApiErrorCodeV4ApprovalStale {
    pub const VALUE: i64 = -32020;
}

impl Serialize for ContractLocalApiErrorCodeV4ApprovalStale {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4ApprovalStale {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A política ou a tolerância do relógio mudou desde o plano: o portão registrou a negação.
///
/// `$defs.approval_policy_changed` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32021`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4ApprovalPolicyChanged;

impl ContractLocalApiErrorCodeV4ApprovalPolicyChanged {
    pub const VALUE: i64 = -32021;
}

impl Serialize for ContractLocalApiErrorCodeV4ApprovalPolicyChanged {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4ApprovalPolicyChanged {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O relógio do núcleo está antes do último passo da ação; nada foi decidido e a resposta pode ser repetida.
///
/// `$defs.approval_out_of_order` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32022`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4ApprovalOutOfOrder;

impl ContractLocalApiErrorCodeV4ApprovalOutOfOrder {
    pub const VALUE: i64 = -32022;
}

impl Serialize for ContractLocalApiErrorCodeV4ApprovalOutOfOrder {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4ApprovalOutOfOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A API não deixa aprovar esta ação: o efeito concreto não está fixado no plano registrado. Negar continua permitido. A resposta fica registrada.
///
/// `$defs.approval_not_approvable` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32023`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4ApprovalNotApprovable;

impl ContractLocalApiErrorCodeV4ApprovalNotApprovable {
    pub const VALUE: i64 = -32023;
}

impl Serialize for ContractLocalApiErrorCodeV4ApprovalNotApprovable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4ApprovalNotApprovable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila já foi criada.
///
/// `$defs.queue_already_created` in `urn:ads:contract:local-api.error-code:4`.
///
/// Always `-32024`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV4QueueAlreadyCreated;

impl ContractLocalApiErrorCodeV4QueueAlreadyCreated {
    pub const VALUE: i64 = -32024;
}

impl Serialize for ContractLocalApiErrorCodeV4QueueAlreadyCreated {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV4QueueAlreadyCreated {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
