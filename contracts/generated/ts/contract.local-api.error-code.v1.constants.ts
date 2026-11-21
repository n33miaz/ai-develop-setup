/* Gerado por tools/gen-ts.mjs a partir de schemas/contract.local-api.error-code.v1.schema.json. Não edite à mão. */

/** O método solicitado é desconhecido. */
export const METHOD_NOT_FOUND = -32601

/** O comando ou seus parâmetros são inválidos para a API local. */
export const INVALID_PARAMS = -32602

/** O serviço está indisponível, inclusive quando o prazo do host se esgota. */
export const INTERNAL_ERROR = -32603

/** Um limite de capacidade foi atingido. */
export const CAPACITY_EXCEEDED = -32003

/** O replay solicitado está indisponível ou o stream foi invalidado ou descartado. */
export const SUBSCRIPTION_REPLAY_UNAVAILABLE = -32004

/** A API local recusou um comando de origem remota. */
export const REMOTE_ORIGIN_REFUSED = -32010

/** A ferramenta solicitada não tem programa configurado. */
export const HARNESS_NOT_CONFIGURED = -32011

/** A chave de idempotência conflita com outro pedido ou o id da sessão está reservado. */
export const CONFLICT = -32012

/** O fechamento automático esgotou as tentativas e ainda depende de um session.close. */
export const SESSION_CLOSE_PENDING = -32013

/** A preparação falhou após a abertura do terminal e o fechamento foi agendado. */
export const SESSION_SETUP_FAILED = -32014

/** O núcleo não serve a fila pedida: nenhuma fila ligada ao log de auditoria ou id de fila desconhecido. */
export const QUEUE_NOT_CONFIGURED = -32015

/** A fila já está ligada; ligar de novo não registra nada. */
export const QUEUE_ALREADY_ENABLED = -32016
