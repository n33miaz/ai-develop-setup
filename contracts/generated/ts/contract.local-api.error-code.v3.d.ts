/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.local-api.error-code.v3.schema.json. Não edite à mão. */

/**
 * Código de erro da API local. Códigos que a API local atribui às próprias respostas JSON-RPC. A versão 3 acrescenta à :2 os códigos das aprovações do portão de ações; a :2 e a :1 continuam valendo para quem só conhece os códigos anteriores.
 */
export type ContractLocalApiErrorCodeV3 =
  | MethodNotFound
  | InvalidParams
  | InternalError
  | CapacityExceeded
  | SubscriptionReplayUnavailable
  | RemoteOriginRefused
  | HarnessNotConfigured
  | Conflict
  | SessionClosePending
  | SessionSetupFailed
  | QueueNotConfigured
  | QueueAlreadyEnabled
  | ApprovalNotConfigured
  | ApprovalNotFound
  | ApprovalAlreadyAnswered
  | ApprovalStale
  | ApprovalPolicyChanged
  | ApprovalOutOfOrder
  | ApprovalNotApprovable;
/**
 * O método solicitado é desconhecido.
 */
export type MethodNotFound = -32601;
/**
 * O comando ou seus parâmetros são inválidos para a API local.
 */
export type InvalidParams = -32602;
/**
 * O serviço está indisponível, inclusive quando o prazo do host se esgota.
 */
export type InternalError = -32603;
/**
 * Um limite de capacidade foi atingido.
 */
export type CapacityExceeded = -32003;
/**
 * O replay solicitado está indisponível ou o stream foi invalidado ou descartado.
 */
export type SubscriptionReplayUnavailable = -32004;
/**
 * A API local recusou um comando de origem remota.
 */
export type RemoteOriginRefused = -32010;
/**
 * A ferramenta solicitada não tem programa configurado.
 */
export type HarnessNotConfigured = -32011;
/**
 * A chave de idempotência conflita com outro pedido ou o id da sessão está reservado.
 */
export type Conflict = -32012;
/**
 * O fechamento automático esgotou as tentativas e ainda depende de um session.close.
 */
export type SessionClosePending = -32013;
/**
 * A preparação falhou após a abertura do terminal e o fechamento foi agendado.
 */
export type SessionSetupFailed = -32014;
/**
 * O núcleo não serve a fila pedida: nenhuma fila ligada ao log de auditoria ou id de fila desconhecido.
 */
export type QueueNotConfigured = -32015;
/**
 * A fila já está ligada; ligar de novo não registra nada.
 */
export type QueueAlreadyEnabled = -32016;
/**
 * O núcleo não serve aprovações: nenhum portão ligado ao log de auditoria.
 */
export type ApprovalNotConfigured = -32017;
/**
 * Nenhuma ação conhecida do portão com esse id, ou a ação não tem essa aprovação.
 */
export type ApprovalNotFound = -32018;
/**
 * A ação não aguarda mais resposta: já foi aprovada, negada, expirou ou terminou.
 */
export type ApprovalAlreadyAnswered = -32019;
/**
 * O hash do plano não é o da ação em espera, ou a validade da aprovação acabou (o portão registra a expiração). A resposta fica registrada.
 */
export type ApprovalStale = -32020;
/**
 * A política ou a tolerância do relógio mudou desde o plano: o portão registrou a negação.
 */
export type ApprovalPolicyChanged = -32021;
/**
 * O relógio do núcleo está antes do último passo da ação; nada foi decidido e a resposta pode ser repetida.
 */
export type ApprovalOutOfOrder = -32022;
/**
 * A API não deixa aprovar esta ação: o efeito concreto não está fixado no plano registrado. Negar continua permitido. A resposta fica registrada.
 */
export type ApprovalNotApprovable = -32023;
