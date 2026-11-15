# Mapa de troca dos tipos escritos à mão

Dois módulos ainda escrevem à mão os tipos dos contratos: `crates/core/src/gate/event.rs` (portão de ações) e `crates/ptyhost/src/wire.rs` (contrato `pty-host/1`). Este mapa diz, para cada tipo público deles, qual tipo gerado o substitui e o que muda de forma. A troca em si é a entrega seguinte; quando ela terminar, este arquivo sai.

Vale para todos:

- Erro: os dois `ContractError` escritos à mão (e o `String` de `SessionId::new`) dão lugar a `ads_contracts::ContractError { what, detail }`.
- Texto validado: `Tipo::new(texto)`, `as_str()`, `TryFrom<String>`, `Display` e a leitura que valida existem igual nos gerados.
- Trecho escrito dentro de um Schema (sem `$ref`) vira um tipo por lugar. O mesmo padrão repetido em dois Schemas são dois tipos distintos; converte-se por `as_str()` e `new`.
- Campo `type` é `r#type` no Rust.
- Os gerados não derivam `PartialOrd`/`Ord`: a ordem de `Risk` e de `Decision` é regra do portão, não do Schema.

## `crates/core/src/gate/event.rs`

### Tipos comuns e textos

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `Id` | `CommonIdV1` | nenhuma |
| `Ulid` | `CommonUlidV1` | nenhuma |
| `Timestamp` | `CommonTimestampV1` | o gerado não tem `is_before`: a comparação de instantes fica no núcleo, como função sobre `as_str()` |
| `PlanHash` | `CommonSha256V1` | o gerado não calcula o hash (`PlanHash::of`); o cálculo fica no núcleo e constrói por `CommonSha256V1::new` |
| `RuleCode` | `CommonActionRuleV1` | nenhuma |
| `PathText` | `CommonAbspathV1` | o gerado confere o que o Schema confere; a normalização do núcleo (`AbsPath`) continua sendo quem produz o texto |
| `Verb` | `EventActionPlannedV1DataVerb`, `EventActionDeniedV1DataVerb` | um tipo por evento |
| `ErrorCode` | `EventActionFailedV1DataErrorCode` | nenhuma |
| `ShortText` | `EventApprovalRequestedV2DataSummary`, `EventApprovalRequestedV1DataSummary`, `EventApprovalDeniedV1DataReason`, `RecordApprovalV1Summary`, `RecordApprovalV1Reason` | um tipo por lugar |
| `PatternText` | `RecordActionPlanV1TargetsItemPattern` | nenhuma |
| `SubjectText` | `RecordActionPlanV1VerdictReasonsItemSubject` | nenhuma |
| `Risk` | `CommonRiskV1` | sem `Ord` |
| `DataClass` | `CommonDataClassV1` | nenhuma |
| `ActorKind`, `Actor` | `CommonActorV1Kind`, `CommonActorV1` | nenhuma |
| `Decision` | `RecordActionPlanV1VerdictDecision` | sem `Ord` e sem `risk()`: a correspondência com o risco fica no núcleo (o Schema a confere no registro do plano) |
| `Via` | `EventApprovalGrantedV1DataVia`, `RecordApprovalV1Via` | um tipo por lugar |

### Eventos

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `Envelope<ActionPlanned>` | `EventActionPlannedV1` | ver abaixo |
| `Envelope<ActionDenied>` | `EventActionDeniedV1` | ver abaixo |
| `Envelope<ActionStarted>` | `EventActionStartedV1` | ver abaixo |
| `Envelope<ActionExecuted>` | `EventActionExecutedV1` | ver abaixo |
| `Envelope<ActionFailed>` | `EventActionFailedV1` | ver abaixo |
| `Envelope<ApprovalRequested>` | `EventApprovalRequestedV2` | ver abaixo |
| `Envelope<ApprovalRequestedV1>` | `EventApprovalRequestedV1` | ver abaixo |
| `Envelope<ApprovalGranted>` | `EventApprovalGrantedV1` | ver abaixo |
| `Envelope<ApprovalDenied>` | `EventApprovalDeniedV1` | ver abaixo |
| `Envelope<ApprovalExpired>` | `EventApprovalExpiredV1` | ver abaixo |
| `ActionPlanned`, `ActionDenied`, … (o `data` de cada evento) | `EventActionPlannedV1Data`, `EventActionDeniedV1Data`, … (`Event<Nome>V<n>Data`) | mesmos campos, com os tipos da tabela acima |
| `RuleList` | `EventActionDeniedV1DataRules` | `as_slice()` igual; `into_vec()` a mais |

Diferenças do envelope:

- Não há envelope genérico: cada evento é um `struct` completo, com os nove campos públicos (`id`, `r#type`, `ts`, `stream`, `seq`, `actor`, `cause`, `data_class`, `data`). O `Envelope::new(meta, data)` do núcleo passa a montar esse `struct`.
- `r#type` é um tipo sem dados (`EventActionPlannedV1Type`, com `VALUE` igual à URN): não há como montar o evento com o `type` errado.
- `stream` é um texto validado por evento (`EventActionPlannedV1Stream`), que confere a forma e a família (`action/…`). Ele **não** confere que o `<id>` é o da entidade do `data`: essa regra não está no Schema e continua no núcleo, que monta o `stream`.
- `seq` continua `NonZeroU64`.

Continuam escritos à mão, por serem comportamento e não Schema: `EventMeta`, o traço `EventData` (URN, família e entidade de cada evento), o enum `Event` com `from_json`/`to_json` (o despacho pelo `type`), o vínculo entre `stream` e entidade.

### Registros

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `ActionPlanFields` | `RecordActionPlanV1Fields` | `version` é um tipo sem dados (`RecordActionPlanV1Version`), não `u64`; `targets` é `RecordActionPlanV1Targets` (1 a 10000, sem repetição), não `Vec`; `verdict` é o tipo conferido `RecordActionPlanV1Verdict` |
| `ActionPlanRecord` | `RecordActionPlanV1` | `Deref` para os campos no lugar de `fields()`. Confere tudo o que o Schema diz (quarentena, destino, veredito com risco e motivos). **Não** confere a ordem canônica dos alvos nem o `plan_hash`: `canonical()` e essas duas conferências continuam no núcleo, num tipo que envolve o gerado |
| `PlanOperation` | `RecordActionPlanV1Operation` | variantes `Delete`, `DeleteRecursive`, `Move`, `Overwrite`, `CleanUntracked`; `as_str()` igual |
| `PlanTarget` | `RecordActionPlanV1TargetsItem` | mesmos campos; o texto do alvo (`text()`) continua no núcleo |
| `PlanEstimate` | `RecordActionPlanV1Estimate` | nenhuma |
| `PlanReason` | `RecordActionPlanV1VerdictReasonsItem` | nenhuma |
| `PlanVerdict` | `RecordActionPlanV1Verdict` (`RecordActionPlanV1VerdictFields`) | passa a ser conferido: risco e motivos têm de bater com a decisão; `reasons` é `RecordActionPlanV1VerdictReasons` (até 1000) |
| `ApprovalStatus` | `RecordApprovalV1Status` | nenhuma |
| `ApprovalFields` | `RecordApprovalV1Fields` | `version` é um tipo sem dados |
| `ApprovalRecord` | `RecordApprovalV1` | `Deref` no lugar de `fields()` |

## `crates/ptyhost/src/wire.rs`

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `OutputHeader` | `ContractPtyHostFrameV1` | mesmos campos (`session_id`, `seq`, `length`) |
| `JsonRpc` | um tipo sem dados por mensagem (`ContractPtyHostResponseV1Jsonrpc`, `ContractPtyHostRequestV1HostHelloJsonrpc`, …) | um por lugar; todos têm `Default` |
| `RequestId` | `NonZeroU64` | nenhuma |
| `SessionId` | `CommonIdV1` | guarda `String`, não `Arc<str>`, e não deriva `Ord`: se o host precisa de cópia barata ou de ordem, embrulha |
| `Dimension` | `ContractPtyHostRequestV1SessionOpenParamsCols`/`Rows`, `ContractPtyHostRequestV1SessionResizeParamsCols`/`Rows` | um tipo por lugar; `get()` igual, sobre `u16` |
| `Base64` | `CommonBase64V1` | o gerado é o texto validado, não os bytes: codificar e decodificar continuam no host |
| `Request { id, call }` e `Call` | `ContractPtyHostRequestV1` | um `enum` com a marca `method`; cada variante guarda um `struct` com `jsonrpc`, `id` e `params` (`ContractPtyHostRequestV1SessionOpen`, …). O `id` fica dentro da variante |
| `HelloParams` | `ContractPtyHostRequestV1HostHelloParams` | `contract` é o valor fixo `pty-host/1`: versão desconhecida é recusada na leitura. Para responder `-32009`, o host lê o `contract` antes da leitura tipada (como já faz `Request::parse` com o método) |
| `OpenParams` | `ContractPtyHostRequestV1SessionOpenParams` | `program`, cada item de `args` e `cwd` são textos validados com tipo próprio |
| `WriteParams`, `ResizeParams`, `SubscribeParams`, `SessionParams`, `NoParams` | `ContractPtyHostRequestV1SessionWriteParams`, `…SessionResizeParams`, `…SessionSubscribeParams`, `…SessionCloseParams`, `…SessionListParams` | mesmos campos |
| `Response` | `ContractPtyHostResponseV1` (`ContractPtyHostResponseV1Fields`) | conferido: `result` e `error` são dois `Option` e a regra do Schema exige exatamente um |
| `ResultBody` | `ContractPtyHostResponseV1Result` | `enum` sem marca, variantes `Empty`, `Hello`, `ReplayBounds`, `Closed`, `SessionList` |
| `HelloResult`, `ReplayBounds`, `Closed`, `SessionList`, `SessionEntry` | `ContractPtyHostResponseV1Hello`, `…ReplayBounds`, `…Closed`, `…SessionList`, `…SessionListSessionsItem` | mesmos campos |
| `ErrorCode` | `ContractPtyHostResponseV1ErrorCode` | variantes pelo número (`Neg32004`), não pelo significado; `number()` igual. Os nomes com significado continuam no host, como constantes |
| `ErrorBody` | `ContractPtyHostResponseV1Error` (`…ErrorFields`) | conferido: `-32004` exige `data.oldest_seq` e `-32005` exige `data.next_seq`; `message` é texto validado |
| `ErrorData` | `ContractPtyHostResponseV1ErrorData` | mesmos campos |
| `Notification` | `ContractPtyHostNotificationV1` | `enum` com a marca `method`; cada variante guarda `jsonrpc` e `params` |
| `SessionExited`, `SubscriptionClosed` | `ContractPtyHostNotificationV1SessionExitedParams`, `…SubscriptionClosedParams` | mesmos campos |
| `CloseReason` | `ContractPtyHostNotificationV1SubscriptionClosedParamsReason` | nenhuma |

Continuam escritos à mão, por serem comportamento e não Schema: o codec do quadro binário (`Frame`, `WireError`, `encode_json`, `encode_output`, `decode_payload`, as constantes `KIND_*`), `Rejection` e `Request::parse` (que erro JSON-RPC responder a um pedido malformado), `HostMessage` (resposta ou notificação) e a conversão de `Base64` em bytes.
