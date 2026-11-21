// Gerado por contracts/tools/gen-rust.mjs a partir de contracts/schemas/contract.local-api.error-code.v6.schema.json. Não edite à mão.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::support;

/// Código de erro da API local. Códigos que a API local atribui às próprias respostas JSON-RPC. A versão 6 acrescenta à :5 as recusas de fila desconhecida e de fila ainda não criada, e restringe QUEUE_NOT_CONFIGURED aos controles da fila que não subiram.
///
/// `urn:ads:contract:local-api.error-code:6`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContractLocalApiErrorCodeV6 {
    MethodNotFound(ContractLocalApiErrorCodeV6MethodNotFound),
    InvalidParams(ContractLocalApiErrorCodeV6InvalidParams),
    InternalError(ContractLocalApiErrorCodeV6InternalError),
    CapacityExceeded(ContractLocalApiErrorCodeV6CapacityExceeded),
    SubscriptionReplayUnavailable(ContractLocalApiErrorCodeV6SubscriptionReplayUnavailable),
    RemoteOriginRefused(ContractLocalApiErrorCodeV6RemoteOriginRefused),
    HarnessNotConfigured(ContractLocalApiErrorCodeV6HarnessNotConfigured),
    Conflict(ContractLocalApiErrorCodeV6Conflict),
    SessionClosePending(ContractLocalApiErrorCodeV6SessionClosePending),
    SessionSetupFailed(ContractLocalApiErrorCodeV6SessionSetupFailed),
    QueueNotConfigured(ContractLocalApiErrorCodeV6QueueNotConfigured),
    QueueAlreadyEnabled(ContractLocalApiErrorCodeV6QueueAlreadyEnabled),
    ApprovalNotConfigured(ContractLocalApiErrorCodeV6ApprovalNotConfigured),
    ApprovalNotFound(ContractLocalApiErrorCodeV6ApprovalNotFound),
    ApprovalAlreadyAnswered(ContractLocalApiErrorCodeV6ApprovalAlreadyAnswered),
    ApprovalStale(ContractLocalApiErrorCodeV6ApprovalStale),
    ApprovalPolicyChanged(ContractLocalApiErrorCodeV6ApprovalPolicyChanged),
    ApprovalOutOfOrder(ContractLocalApiErrorCodeV6ApprovalOutOfOrder),
    ApprovalNotApprovable(ContractLocalApiErrorCodeV6ApprovalNotApprovable),
    QueueAlreadyCreated(ContractLocalApiErrorCodeV6QueueAlreadyCreated),
    ApprovalReadable(ContractLocalApiErrorCodeV6ApprovalReadable),
    QueueNotFound(ContractLocalApiErrorCodeV6QueueNotFound),
    QueueNotCreated(ContractLocalApiErrorCodeV6QueueNotCreated),
}

/// O método solicitado é desconhecido.
///
/// `$defs.method_not_found` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32601`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6MethodNotFound;

impl ContractLocalApiErrorCodeV6MethodNotFound {
    pub const VALUE: i64 = -32601;
}

impl Serialize for ContractLocalApiErrorCodeV6MethodNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6MethodNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O comando ou seus parâmetros são inválidos para a API local.
///
/// `$defs.invalid_params` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32602`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6InvalidParams;

impl ContractLocalApiErrorCodeV6InvalidParams {
    pub const VALUE: i64 = -32602;
}

impl Serialize for ContractLocalApiErrorCodeV6InvalidParams {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6InvalidParams {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O serviço está indisponível, inclusive quando o prazo do host se esgota.
///
/// `$defs.internal_error` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32603`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6InternalError;

impl ContractLocalApiErrorCodeV6InternalError {
    pub const VALUE: i64 = -32603;
}

impl Serialize for ContractLocalApiErrorCodeV6InternalError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6InternalError {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Um limite de capacidade foi atingido.
///
/// `$defs.capacity_exceeded` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32003`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6CapacityExceeded;

impl ContractLocalApiErrorCodeV6CapacityExceeded {
    pub const VALUE: i64 = -32003;
}

impl Serialize for ContractLocalApiErrorCodeV6CapacityExceeded {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6CapacityExceeded {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O replay solicitado está indisponível ou o stream foi invalidado ou descartado.
///
/// `$defs.subscription_replay_unavailable` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32004`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6SubscriptionReplayUnavailable;

impl ContractLocalApiErrorCodeV6SubscriptionReplayUnavailable {
    pub const VALUE: i64 = -32004;
}

impl Serialize for ContractLocalApiErrorCodeV6SubscriptionReplayUnavailable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6SubscriptionReplayUnavailable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A API local recusou um comando de origem remota.
///
/// `$defs.remote_origin_refused` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32010`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6RemoteOriginRefused;

impl ContractLocalApiErrorCodeV6RemoteOriginRefused {
    pub const VALUE: i64 = -32010;
}

impl Serialize for ContractLocalApiErrorCodeV6RemoteOriginRefused {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6RemoteOriginRefused {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ferramenta solicitada não tem programa configurado.
///
/// `$defs.harness_not_configured` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32011`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6HarnessNotConfigured;

impl ContractLocalApiErrorCodeV6HarnessNotConfigured {
    pub const VALUE: i64 = -32011;
}

impl Serialize for ContractLocalApiErrorCodeV6HarnessNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6HarnessNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A chave de idempotência conflita com outro pedido ou o id da sessão está reservado.
///
/// `$defs.conflict` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32012`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6Conflict;

impl ContractLocalApiErrorCodeV6Conflict {
    pub const VALUE: i64 = -32012;
}

impl Serialize for ContractLocalApiErrorCodeV6Conflict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6Conflict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O fechamento automático esgotou as tentativas e ainda depende de um session.close.
///
/// `$defs.session_close_pending` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32013`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6SessionClosePending;

impl ContractLocalApiErrorCodeV6SessionClosePending {
    pub const VALUE: i64 = -32013;
}

impl Serialize for ContractLocalApiErrorCodeV6SessionClosePending {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6SessionClosePending {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A preparação falhou após a abertura do terminal e o fechamento foi agendado.
///
/// `$defs.session_setup_failed` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32014`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6SessionSetupFailed;

impl ContractLocalApiErrorCodeV6SessionSetupFailed {
    pub const VALUE: i64 = -32014;
}

impl Serialize for ContractLocalApiErrorCodeV6SessionSetupFailed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6SessionSetupFailed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve fila nenhuma: os controles da fila não subiram, por exemplo porque o log de auditoria não abriu.
///
/// `$defs.queue_not_configured` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32015`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6QueueNotConfigured;

impl ContractLocalApiErrorCodeV6QueueNotConfigured {
    pub const VALUE: i64 = -32015;
}

impl Serialize for ContractLocalApiErrorCodeV6QueueNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6QueueNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila já está ligada; ligar de novo não registra nada.
///
/// `$defs.queue_already_enabled` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32016`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6QueueAlreadyEnabled;

impl ContractLocalApiErrorCodeV6QueueAlreadyEnabled {
    pub const VALUE: i64 = -32016;
}

impl Serialize for ContractLocalApiErrorCodeV6QueueAlreadyEnabled {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6QueueAlreadyEnabled {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O núcleo não serve aprovações: nenhum portão ligado ao log de auditoria.
///
/// `$defs.approval_not_configured` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32017`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalNotConfigured;

impl ContractLocalApiErrorCodeV6ApprovalNotConfigured {
    pub const VALUE: i64 = -32017;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalNotConfigured {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalNotConfigured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// Nenhuma ação conhecida do portão com esse id, ou a ação não tem essa aprovação.
///
/// `$defs.approval_not_found` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32018`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalNotFound;

impl ContractLocalApiErrorCodeV6ApprovalNotFound {
    pub const VALUE: i64 = -32018;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A ação não aguarda mais resposta: já foi aprovada, negada, expirou ou terminou.
///
/// `$defs.approval_already_answered` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32019`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalAlreadyAnswered;

impl ContractLocalApiErrorCodeV6ApprovalAlreadyAnswered {
    pub const VALUE: i64 = -32019;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalAlreadyAnswered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalAlreadyAnswered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O hash do plano não é o da ação em espera, ou a validade da aprovação acabou (o portão registra a expiração). A resposta fica registrada.
///
/// `$defs.approval_stale` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32020`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalStale;

impl ContractLocalApiErrorCodeV6ApprovalStale {
    pub const VALUE: i64 = -32020;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalStale {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalStale {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A política ou a tolerância do relógio mudou desde o plano: o portão registrou a negação.
///
/// `$defs.approval_policy_changed` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32021`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalPolicyChanged;

impl ContractLocalApiErrorCodeV6ApprovalPolicyChanged {
    pub const VALUE: i64 = -32021;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalPolicyChanged {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalPolicyChanged {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O relógio do núcleo está antes do último passo da ação; nada foi decidido e a resposta pode ser repetida.
///
/// `$defs.approval_out_of_order` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32022`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalOutOfOrder;

impl ContractLocalApiErrorCodeV6ApprovalOutOfOrder {
    pub const VALUE: i64 = -32022;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalOutOfOrder {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalOutOfOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A API não deixa aprovar esta ação: o efeito concreto não está fixado no plano registrado. Negar continua permitido. A resposta fica registrada.
///
/// `$defs.approval_not_approvable` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32023`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalNotApprovable;

impl ContractLocalApiErrorCodeV6ApprovalNotApprovable {
    pub const VALUE: i64 = -32023;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalNotApprovable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalNotApprovable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila já foi criada.
///
/// `$defs.queue_already_created` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32024`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6QueueAlreadyCreated;

impl ContractLocalApiErrorCodeV6QueueAlreadyCreated {
    pub const VALUE: i64 = -32024;
}

impl Serialize for ContractLocalApiErrorCodeV6QueueAlreadyCreated {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6QueueAlreadyCreated {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O histórico do pedido de aprovação é legível: ele é respondido só por approval.answer, nunca fechado por approval.close. Nada foi gravado.
///
/// `$defs.approval_readable` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32025`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6ApprovalReadable;

impl ContractLocalApiErrorCodeV6ApprovalReadable {
    pub const VALUE: i64 = -32025;
}

impl Serialize for ContractLocalApiErrorCodeV6ApprovalReadable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6ApprovalReadable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// O id pedido não é o da fila configurada no núcleo; queue.configured informa o id certo.
///
/// `$defs.queue_not_found` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32026`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6QueueNotFound;

impl ContractLocalApiErrorCodeV6QueueNotFound {
    pub const VALUE: i64 = -32026;
}

impl Serialize for ContractLocalApiErrorCodeV6QueueNotFound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6QueueNotFound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}

/// A fila configurada ainda não foi criada; queue.create a cria.
///
/// `$defs.queue_not_created` in `urn:ads:contract:local-api.error-code:6`.
///
/// Always `-32027`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ContractLocalApiErrorCodeV6QueueNotCreated;

impl ContractLocalApiErrorCodeV6QueueNotCreated {
    pub const VALUE: i64 = -32027;
}

impl Serialize for ContractLocalApiErrorCodeV6QueueNotCreated {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for ContractLocalApiErrorCodeV6QueueNotCreated {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        support::expect_i64(deserializer, Self::VALUE).map(|()| Self)
    }
}
