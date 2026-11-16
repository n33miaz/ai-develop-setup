# Mapa de troca dos tipos escritos à mão

Dois módulos ainda escrevem à mão os tipos dos contratos: `crates/core/src/gate/event.rs` (portão de ações) e `crates/ptyhost/src/wire.rs` (contrato `pty-host/1`). Este mapa diz, para cada tipo público deles, qual tipo gerado o substitui e o que muda de forma. A troca em si é a entrega seguinte; quando ela terminar, este arquivo sai.

Vale para todos:

- Erro: os dois `ContractError` escritos à mão (e o `String` de `SessionId::new`) dão lugar a `ads_contracts::ContractError { what, detail }`.
- Texto validado: `Tipo::new(texto)`, `as_str()`, `TryFrom<String>`, `Display` e a leitura que valida existem igual nos gerados.
- Trecho escrito dentro de um Schema (sem `$ref`) vira um tipo por lugar. O mesmo padrão repetido em dois Schemas são dois tipos distintos; converte-se por `as_str()` e `new`.
- Campo `type` é `r#type` no Rust.
- Os gerados não derivam `PartialOrd`/`Ord`: a ordem de `Risk` e de `Decision` é regra do portão, não do Schema.
- Versões: o portão escreve `approval.requested:2`, `approval.granted:2`, `approval.denied:2`, `approval.expired:2` e `record.action-plan:2` (com `common.abspath:2`), e as tabelas apontam para os gerados dessas versões. Os tipos escritos à mão com sufixo `V1` são só de leitura e têm linha própria.

## `crates/core/src/gate/event.rs`

### Tipos comuns e textos

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `Id` | `CommonIdV1` | nenhuma |
| `Ulid` | `CommonUlidV1` | nenhuma |
| `Timestamp` | `CommonTimestampV1` | o gerado não tem `is_before`: a comparação de instantes fica no núcleo, como função sobre `as_str()` |
| `PlanHash` | `CommonSha256V1` | o gerado não calcula o hash (`PlanHash::of`); o cálculo fica no núcleo e constrói por `CommonSha256V1::new` |
| `RuleCode` | `CommonActionRuleV1` | nenhuma |
| `PathText` | `CommonAbspathV2` | o gerado confere o que o Schema confere; a normalização do núcleo (`AbsPath`) continua sendo quem produz o texto. Na leitura do registro `:1` o gerado é `CommonAbspathV1`, mais largo que `PathText` (aceita nome de dispositivo e nome curto do Windows): a recusa deles continua no núcleo |
| `Verb` | `EventActionPlannedV1DataVerb`, `EventActionDeniedV1DataVerb` | um tipo por evento |
| `ErrorCode` | `EventActionFailedV1DataErrorCode` | nenhuma |
| `ShortText` | `EventApprovalRequestedV2DataSummary`, `EventApprovalDeniedV2DataReason`, `RecordApprovalV1Summary`, `RecordApprovalV1Reason`; nos tipos só de leitura, `EventApprovalRequestedV1DataSummary` e `EventApprovalDeniedV1DataReason` | um tipo por lugar |
| `PatternText` | `RecordActionPlanV2TargetsItemPattern` | nenhuma |
| `SubjectText` | `RecordActionPlanV2VerdictReasonsItemSubject` | nenhuma |
| `Risk` | `CommonRiskV1` | sem `Ord` |
| `DataClass` | `CommonDataClassV1` | nenhuma |
| `ActorKind`, `Actor` | `CommonActorV1Kind`, `CommonActorV1` | nenhuma |
| `Decision` | `RecordActionPlanV2VerdictDecision` | sem `Ord` e sem `risk()`: a correspondência com o risco fica no núcleo (o Schema a confere no registro do plano) |
| `Via` | `EventApprovalGrantedV2DataVia`, `RecordApprovalV1Via`; no tipo só de leitura, `EventApprovalGrantedV1DataVia` | um tipo por lugar |

### Eventos

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `Envelope<ActionPlanned>` | `EventActionPlannedV1` | ver abaixo |
| `Envelope<ActionDenied>` | `EventActionDeniedV1` | ver abaixo |
| `Envelope<ActionStarted>` | `EventActionStartedV1` | ver abaixo |
| `Envelope<ActionExecuted>` | `EventActionExecutedV1` | ver abaixo |
| `Envelope<ActionFailed>` | `EventActionFailedV1` | ver abaixo |
| `Envelope<ApprovalRequested>` | `EventApprovalRequestedV2` | ver abaixo |
| `Envelope<ApprovalGranted>` | `EventApprovalGrantedV2` | ver abaixo |
| `Envelope<ApprovalDenied>` | `EventApprovalDeniedV2` | ver abaixo |
| `Envelope<ApprovalExpired>` | `EventApprovalExpiredV2` | ver abaixo |
| `Envelope<ApprovalRequestedV1>` | `EventApprovalRequestedV1` | só leitura; ver abaixo |
| `Envelope<ApprovalGrantedV1>` | `EventApprovalGrantedV1` | só leitura; ver abaixo |
| `Envelope<ApprovalDeniedV1>` | `EventApprovalDeniedV1` | só leitura; ver abaixo |
| `Envelope<ApprovalExpiredV1>` | `EventApprovalExpiredV1` | só leitura; ver abaixo |
| `ActionPlanned`, `ActionDenied`, … (o `data` de cada evento) | `EventActionPlannedV1Data`, `EventActionDeniedV1Data`, `EventApprovalGrantedV2Data`, … (`Event<Nome>V<n>Data`, com o `<n>` da linha do evento) | mesmos campos, com os tipos da tabela acima. Nas quatro aprovações `:2`, `action_id` é `CommonIdV1`, obrigatório; os tipos `…V1` não têm o campo |
| `RuleList` | `EventActionDeniedV1DataRules` | `as_slice()` igual; `into_vec()` a mais |

Diferenças do envelope:

- Não há envelope genérico: cada evento é um `struct` completo, com os nove campos públicos (`id`, `r#type`, `ts`, `stream`, `seq`, `actor`, `cause`, `data_class`, `data`). O `Envelope::new(meta, data)` do núcleo passa a montar esse `struct`.
- `r#type` é um tipo sem dados (`EventActionPlannedV1Type`, com `VALUE` igual à URN): não há como montar o evento com o `type` errado.
- `stream` é um texto validado por evento (`EventActionPlannedV1Stream`), que confere a forma e a família (`action/…`). Ele **não** confere que o `<id>` é o da entidade do `data`: essa regra não está no Schema e continua no núcleo, que monta o `stream`.
- `seq` continua `NonZeroU64`.
- As quatro aprovações `:1` (`ApprovalRequestedV1`, `ApprovalGrantedV1`, `ApprovalDeniedV1`, `ApprovalExpiredV1`) existem só como tipo de leitura: o portão não as escreve, não refaz estado a partir delas, e `Event::from_json` as recusa, porque não dizem de qual ação são. A troca pelos gerados `:1` não muda isso: o despacho pelo `type` continua aceitando só as `:2`.

Continuam escritos à mão, por serem comportamento e não Schema: `EventMeta`, o traço `EventData` (URN, família e entidade de cada evento), o enum `Event` com `from_json`/`to_json` (o despacho pelo `type`), o vínculo entre `stream` e entidade.

### Registros

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `ActionPlanFields` | `RecordActionPlanV2Fields` | à mão, um tipo só serve às duas versões e `version` é `u64`; nos gerados há um por versão e `version` é um tipo sem dados (`RecordActionPlanV2Version`). `targets` é `RecordActionPlanV2Targets` (1 a 10000, sem repetição), não `Vec`; `verdict` é o tipo conferido `RecordActionPlanV2Verdict`; `work_root`, `destination` e o `path` de cada alvo são `CommonAbspathV2` |
| `ActionPlanRecord` | `RecordActionPlanV2` | `Deref` para os campos no lugar de `fields()`. Confere tudo o que o Schema diz (quarentena, destino, veredito com risco e motivos, teto da estimativa). **Não** confere a ordem canônica dos alvos nem o `plan_hash`: `canonical()` e essas duas conferências continuam no núcleo, num tipo que envolve o gerado. `ActionPlanRecord::VERSION` (2) passa a ser o valor fixo de `RecordActionPlanV2Version` |
| `ActionPlanRecordV1` | `RecordActionPlanV1` (`RecordActionPlanV1Fields`) | só leitura: o portão escreve a `:2`. Mesma forma da `:2`, com `CommonAbspathV1` nos caminhos e `RecordActionPlanV1Estimate` (`items` e `bytes` em `u64`, sem teto). O gerado aceita os caminhos que só a `abspath:1` aceita; `PathText` os recusa, e essa recusa continua no núcleo, junto com a ordem dos alvos e o `plan_hash` |
| `PlanOperation` | `RecordActionPlanV2Operation` | variantes `Delete`, `DeleteRecursive`, `Move`, `Overwrite`, `CleanUntracked`; `as_str()` igual |
| `PlanTarget` | `RecordActionPlanV2TargetsItem` | mesmos campos; o texto do alvo (`text()`) continua no núcleo |
| `PlanEstimate` | `RecordActionPlanV2Estimate` | `items` e `bytes` são tipos conferidos (`RecordActionPlanV2EstimateItems`, `RecordActionPlanV2EstimateBytes`, de 0 a 2^53 − 1), não `u64`: o teto `ESTIMATE_MAX` do núcleo passa a ser o do tipo gerado |
| `PlanReason` | `RecordActionPlanV2VerdictReasonsItem` | nenhuma |
| `PlanVerdict` | `RecordActionPlanV2Verdict` (`RecordActionPlanV2VerdictFields`) | passa a ser conferido: risco e motivos têm de bater com a decisão; `reasons` é `RecordActionPlanV2VerdictReasons` (até 1000) |
| `ApprovalStatus` | `RecordApprovalV1Status` | nenhuma |
| `ApprovalFields` | `RecordApprovalV1Fields` | `version` é um tipo sem dados |
| `ApprovalRecord` | `RecordApprovalV1` | `Deref` no lugar de `fields()` |

## `crates/ptyhost/src/wire.rs`

| Escrito à mão | Gerado | Diferença |
|---|---|---|
| `OutputHeader` | `ContractPtyHostFrameV1` | `length` é `NonZeroU64` (o Schema não dá teto), não `NonZeroU32`: o limite de 32 bits do quadro continua conferido no codec |
| `JsonRpc` | um tipo sem dados por mensagem (`ContractPtyHostResponseV1Jsonrpc`, `ContractPtyHostRequestV1HostHelloJsonrpc`, …) | um por lugar; todos têm `Default` |
| `RequestId` | `NonZeroU64` | nenhuma |
| `SessionId` | `CommonIdV1` | guarda `String`, não `Arc<str>`, e não deriva `Ord`: se o host precisa de cópia barata ou de ordem, embrulha |
| `Dimension` | `ContractPtyHostRequestV1SessionOpenParamsCols`/`Rows`, `ContractPtyHostRequestV1SessionResizeParamsCols`/`Rows` | um tipo por lugar; `get()` igual, sobre `u16` |
| `Base64` | `CommonBase64V1` | o gerado é o texto validado, não os bytes: codificar e decodificar continuam no host |
| `Request { id, call }` e `Call` | `ContractPtyHostRequestV1` | um `enum` com a marca `method`; cada variante guarda um `struct` com `jsonrpc`, `id` e `params` (`ContractPtyHostRequestV1SessionOpen`, …). O `id` fica dentro da variante |
| `HelloParams` | `ContractPtyHostRequestV1HostHelloParams` | `contract` é o valor fixo `pty-host/1`: versão desconhecida é recusada na leitura. Para responder `-32009`, o host lê o `contract` antes da leitura tipada (como já faz `Request::parse` com o método) |
| `OpenParams` | `ContractPtyHostRequestV1SessionOpenParams` | `program`, cada item de `args` e `cwd` são textos validados com tipo próprio |
| `WriteParams`, `ResizeParams`, `SubscribeParams`, `SessionParams`, `NoParams` | `ContractPtyHostRequestV1SessionWriteParams`, `…SessionResizeParams`, `…SessionSubscribeParams`, `…SessionCloseParams`, `…SessionListParams` | mesmos campos |
| `Response { id, outcome }` | `ContractPtyHostResponseV1` (`ContractPtyHostResponseV1Fields`) | no lugar de `outcome: Result<ResultBody, ErrorBody>` há dois `Option` (`result` e `error`), e a regra do Schema exige exatamente um; o host converte para `Result` ao ler. Há o campo `jsonrpc` |
| `ResultBody` | `ContractPtyHostResponseV1Result` | `enum` sem marca, variantes `Empty`, `Hello`, `ReplayBounds`, `Closed`, `SessionList` |
| `HelloResult` | `ContractPtyHostResponseV1Hello` | `contract` é o valor fixo `pty-host/1`, não `String` |
| `ReplayBounds`, `Closed`, `SessionList`, `SessionEntry` | `ContractPtyHostResponseV1ReplayBounds`, `…Closed`, `…SessionList`, `…SessionListSessionsItem` | mesmos campos e mesmos tipos (`NonZeroU64`, `Option<i64>`) |
| `ErrorCode` | `ContractPtyHostResponseV1ErrorCode` | variantes pelo número (`Neg32004`), não pelo significado; `number()` igual. Os nomes com significado continuam no host, como constantes |
| `ErrorBody` | `ContractPtyHostResponseV1Error` (`…ErrorFields`) | conferido: `-32004` exige `data.oldest_seq` e `-32005` exige `data.next_seq`; `message` é texto validado de 1 a 500 caracteres (`ContractPtyHostResponseV1ErrorMessage`), não `String` |
| `ErrorData` | `ContractPtyHostResponseV1ErrorData` | mesmos campos |
| `Notification` | `ContractPtyHostNotificationV1` | `enum` com a marca `method`; cada variante guarda `jsonrpc` e `params` |
| `SessionExited`, `SubscriptionClosed` | `ContractPtyHostNotificationV1SessionExitedParams`, `…SubscriptionClosedParams` | mesmos campos e mesmos tipos (`last_seq` é `u64`) |
| `CloseReason` | `ContractPtyHostNotificationV1SubscriptionClosedParamsReason` | nenhuma |

Continuam escritos à mão, por serem comportamento e não Schema: o codec do quadro binário (`Frame`, `WireError`, `encode_json`, `encode_output`, `decode_payload`, as constantes `KIND_*`), `Rejection` e `Request::parse` (que erro JSON-RPC responder a um pedido malformado), `HostMessage` (resposta ou notificação) e a conversão de `Base64` em bytes.
