# Mapa de troca dos tipos escritos à mão

Dois módulos escreviam à mão os tipos dos contratos: `crates/core/src/gate/event.rs` (portão de ações) e `crates/ptyhost/src/wire.rs` (contrato `pty-host/1`). Este mapa diz, para cada tipo público deles, qual tipo gerado o substitui e o que muda de forma.

Estado: a troca do `ptyhost` e a da camada de wire do portão estão feitas. O mapa registra os tipos usados e os valores de domínio que continuam escritos à mão.

Vale para todos:

- Erro: os dois `ContractError` escritos à mão (e o `String` de `SessionId::new`) dão lugar a `ads_contracts::ContractError { what, detail }`.
- Texto validado: `Tipo::new(texto)`, `as_str()`, `TryFrom<String>`, `Display` e a leitura que valida existem igual nos gerados.
- Trecho escrito dentro de um Schema (sem `$ref`) vira um tipo por lugar. O mesmo padrão repetido em dois Schemas são dois tipos distintos; converte-se por `as_str()` e `new`.
- Campo `type` é `r#type` no Rust.
- Os gerados não derivam `PartialOrd`/`Ord`: a ordem de `Risk` e de `Decision` é regra do portão, não do Schema.
- Versões: o portão escreve `approval.requested:2`, `approval.granted:2`, `approval.denied:2`, `approval.expired:2` e `record.action-plan:2` (com `common.abspath:2`), e as tabelas apontam para os gerados dessas versões. Os tipos escritos à mão com sufixo `V1` são só de leitura e têm linha própria.

## `crates/core/src/gate/event.rs`

A leitura e a escrita de todo `Envelope<D>` passam pelo evento gerado indicado nas tabelas. `EventData::Wire` fixa o tipo por evento; `wire.rs` converte os campos explicitamente nos dois sentidos. Não há envelope de wire escrito à mão. O despacho de `Event::from_json` continua recusando as quatro aprovações `:1`; os envelopes tipados dessas versões continuam disponíveis para ler documentos antigos sem reconstruir ações.

`ActionPlanRecord`, `ActionPlanRecordV1` e `ApprovalRecord` leem e escrevem pelos registros gerados correspondentes. A conversão dos planos chama `ActionPlanFields::checked`: hash canônico, ordem estrita e identidade dos alvos e normalização dos caminhos continuam no núcleo, inclusive para os caminhos mais largos da `:1`. O teto da estimativa continua aplicado só à `:2` na construção de domínio, além da validação do tipo gerado na leitura.

Os aliases substituem os validadores manuais de `Id`, `Ulid`, `RuleCode`, `Verb`, `ErrorCode`, `ShortText`, `PatternText`, `SubjectText`, `RuleList`, `Actor`, `ActorKind`, `DataClass`, `Via`, `PlanOperation` e `ApprovalStatus`. `ContractError` também é o gerado (`detail` substitui `value`).

Continuam manuais por serem valores ou comportamento de domínio:

- `Timestamp` e `PlanHash`: validam o texto pelo gerado, mas preservam a comparação de instantes e o cálculo do hash. `PathText` exige que `AbsPath` aceite o texto sem reescrita.
- `Risk` e `Decision`: têm ordem e correspondência de risco que pertencem à política. Os mapas para os enums gerados são exaustivos.
- `EventMeta`, `Envelope`, `EventData`, `Event` e os dados de cada evento: mantêm a API da máquina e o vínculo stream ↔ entidade. Os dados são projeções de domínio, sem derivações de serialização; só o envelope converte para o evento gerado.
- `ActionPlanFields`, `PlanTarget`, `PlanEstimate`, `PlanReason`, `PlanVerdict`, `ApprovalFields` e os três wrappers de registro: preservam os construtores e as conferências do domínio. Os campos são projeções sem serialização própria; os wrappers usam exclusivamente os registros gerados.
- `canonical_plan`, a máquina, o replay e a licença: desserialização produz documentos conferidos, nunca `Action` nem `ExecutionPermit`. O replay devolve somente `ActionSnapshot` e continua verificando ação, aprovação, hash e ordem temporal entre documentos.

Regressões em `gate_golden` demonstram que o gerado aceita digest bem formado trocado, alvos fora da ordem canônica e stream de outra entidade, enquanto a conversão de domínio os recusa. Em `gate_replay`, uma concessão `:2` de outra ação é desserializada com sucesso e recusada pelo replay; uma concessão `:1` passa no gerado e não entra no despacho do portão. Os dourados e as expectativas existentes permanecem iguais.

As tabelas abaixo registram a correspondência dos tipos de wire; os nomes de domínio mantidos acima não são aliases quando precisam das conferências adicionais.

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

Trocado. O módulo reexporta os gerados com os nomes que o host já usava (`pub use … as …`), então o resto do crate e os testes continuam dizendo `SessionId`, `OpenParams`, `ResultBody`. O que ficou escrito à mão lê o tipo gerado primeiro e converte: nenhuma mensagem é aceita além do que o Schema aceita. O formato no fio não mudou, byte a byte: `tests/wire_bytes.rs` fixa os bytes de cada mensagem que o host emite e o código de erro de cada pedido malformado, e foi escrito e rodado contra os tipos antigos antes da troca.

### Trocados pelo gerado

| Escrito à mão (removido) | Gerado em uso | Nome no host |
|---|---|---|
| `JsonRpc` | `ContractPtyHostResponseV1Jsonrpc`, `ContractPtyHostRequestV1HostHelloJsonrpc`, `ContractPtyHostNotificationV1SessionExitedJsonrpc` | sem nome próprio: um por envelope |
| `SessionId` | `CommonIdV1` | `SessionId` |
| `Dimension` | `ContractPtyHostRequestV1SessionOpenParamsCols`/`Rows`, `…SessionResizeParamsCols`/`Rows` | sem nome próprio |
| `OpenParams` (e os validadores de `program`, `args` e `cwd`) | `ContractPtyHostRequestV1SessionOpenParams` | `OpenParams` |
| `WriteParams`, `ResizeParams`, `SubscribeParams`, `SessionParams`, `NoParams` | `ContractPtyHostRequestV1SessionWriteParams`, `…SessionResizeParams`, `…SessionSubscribeParams`, `…SessionCloseParams`, `…SessionListParams` | os mesmos nomes |
| `ResultBody` | `ContractPtyHostResponseV1Result` | `ResultBody` |
| `NoParams` usado como resultado vazio | `ContractPtyHostResponseV1Empty` | `Empty` |
| `HelloResult` | `ContractPtyHostResponseV1Hello` | `HelloResult` |
| `ReplayBounds`, `Closed`, `SessionList`, `SessionEntry` | `ContractPtyHostResponseV1ReplayBounds`, `…Closed`, `…SessionList`, `…SessionListSessionsItem` | os mesmos nomes |
| `ErrorData` | `ContractPtyHostResponseV1ErrorData` | `ErrorData` |
| `SessionExited`, `SubscriptionClosed` | `ContractPtyHostNotificationV1SessionExitedParams`, `…SubscriptionClosedParams` | os mesmos nomes |
| `CloseReason` | `ContractPtyHostNotificationV1SubscriptionClosedParamsReason` | `CloseReason` |
| `RawResponse`, `RawError` (leitura campo a campo) | `ContractPtyHostResponseV1` (`…Fields`), `ContractPtyHostResponseV1Error` (`…ErrorFields`) | internos |

O que mudou de forma para quem usa o host como biblioteca:

- `SessionId::new` devolve `ads_contracts::ContractError`, não `String`. O tipo guarda `String` e não deriva `Ord`: o host ordena as sessões por `as_str()` (a mesma ordem de antes) e copia o nome uma vez por quadro de saída.
- `program`, cada item de `args` e `cwd` de `OpenParams`, e as dimensões, são tipos conferidos: constroem-se por `try_into()` ou `new`. `data` de `WriteParams` é o texto `CommonBase64V1`, não os bytes.
- `ErrorData`, `ReplayBounds` e `Closed` não são mais `Copy`, e `ErrorData` não tem `Default`.
- `contract` de `HelloResult` é o valor fixo `pty-host/1` (`Default::default()`), não `String`.

### Ficaram escritos à mão, com conversão a partir do gerado

| Tipo do host | Gerado que ele lê | Por que ficou |
|---|---|---|
| `OutputHeader` | `ContractPtyHostFrameV1` | `length` é `NonZeroU32`: o prefixo do quadro conta o payload em 32 bits e o Schema não dá teto. A leitura passa pelo gerado e recusa o que não cabe; teste: `header_the_schema_accepts_with_a_length_over_32_bits_is_refused_by_the_codec` |
| `HelloParams` | nenhum | o gerado só lê `pty-host/1`. O host tem de ler uma versão que não fala para responder `-32009` e manter a conexão; `is_supported()` é a conferência. Teste: `hello_of_another_version_is_read_by_the_host_and_refused_by_the_generated_type` |
| `Request { id, call }`, `Call`, `Rejection`, `Request::parse` | os `params` de cada método são os gerados; o envelope não usa `ContractPtyHostRequestV1` | cada etapa da leitura que falha é um erro JSON-RPC diferente (`-32700`, `-32600`, `-32601`, `-32602`), e o `enum` gerado devolve um erro só. O envelope escrito à mão mantém a ordem das chaves que o host sempre leu e escreveu. Teste: `requests_the_host_writes_are_the_ones_the_generated_type_reads` |
| `Response { id, outcome }` | `ContractPtyHostResponseV1` | `outcome` é um `Result`; no fio são dois campos opcionais e o Schema exige exatamente um. A leitura passa pelo tipo conferido; a escrita monta os campos do gerado. Teste: `response_with_result_and_error_or_with_neither_is_refused_past_the_field_types` |
| `ErrorCode` | `ContractPtyHostResponseV1ErrorCode` | o host nomeia os códigos pelo significado (`ReplayEvicted`), o gerado pelo número (`Neg32004`). O mapa são dois `match` completos, um em cada sentido. Teste: `error_codes_map_one_to_one_onto_the_numbers_of_the_contract` |
| `ErrorBody` | `ContractPtyHostResponseV1Error` | leva o código pelo significado e o construtor que corta a mensagem em 500 caracteres. Lê e escreve pelo gerado conferido. Teste: `error_outside_the_contract_fails_to_encode_instead_of_going_out` |
| `Notification` | `ContractPtyHostNotificationV1` | lê pelo gerado. Escreve por um envelope próprio porque o `enum` gerado põe `method` antes de `jsonrpc` e os `params` na ordem do Schema; o host sempre escreveu `jsonrpc`, `method`, `params`, com as chaves de `params` em ordem alfabética, e essa ordem foi mantida |
| `Base64` | `CommonBase64V1` | o gerado é o texto conferido; `Base64` são os bytes, e codificar e decodificar continuam no host. Teste: `base64_text_the_schema_accepts_always_becomes_bytes_and_loose_text_never_gets_there` |

Continuam escritos à mão por serem comportamento e não Schema: o codec do quadro binário (`Frame`, `WireError`, `encode_json`, `encode_output`, `decode_payload`, `read_frame`, as constantes `KIND_*`) e `HostMessage` (resposta ou notificação).

### Diferenças de comportamento

- O texto de `error.message` de um `params` recusado (`-32602`) mudou de redação, porque agora vem da leitura dos gerados. O contrato só fixa o tamanho desse texto. O erro dos gerados cita o valor recusado; a resposta do host corta essa parte e diz só o nome do tipo (`not a valid CommonIdV1`), para não devolver ao cliente o que ele mandou (teste `refused_params_are_not_echoed_in_the_answer`). O texto novo diz qual campo foi recusado, mas não a regra: antes, `terminal size must be 1 to 1000`; agora, `not a valid ContractPtyHostRequestV1SessionOpenParamsCols`. Nenhum cliente do repositório lê esse texto, só o código.
- O texto de `error.message` de um envelope com `jsonrpc` diferente de `"2.0"` (`-32600`) também mudou: era `jsonrpc must be "2.0", got "1.0"`, agora é `expected "2.0", found "1.0"`. O código e o `id` da resposta não mudaram, e em nenhum pedido recusado de `tests/wire_bytes.rs` o código mudou.
- Um `ErrorBody` fora do contrato (mensagem vazia ou com mais de 500 caracteres, `-32004` sem `data.oldest_seq`, `-32005` sem `data.next_seq`) agora falha ao ser codificado; antes saía no fio. O servidor nunca monta um erro assim.

### Pendências

- `params` escrito como lista (`"params": []` em `session.list`, ou os campos na ordem, por posição) é aceito na leitura, porque os tipos derivados leem um `struct` também de uma sequência. Já era assim com os tipos escritos à mão e continua com os gerados; o Schema pede objeto. Fechar isso muda o que o host aceita, então não entrou nesta troca.
- `Request` não usa `ContractPtyHostRequestV1`: unir a leitura em etapas ao `enum` gerado pede que o gerado diga em que etapa falhou.
