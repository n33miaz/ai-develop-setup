# Histórico dos contratos

Formato: uma seção por data. Contrato novo, campo opcional acrescentado, versão nova de uma URN e contrato aposentado entram aqui. As regras de versão estão no [README](README.md).

## 2026-11-21

- `harness.list.response:2` e `local-api.server-message:8` informam os apelidos das pastas ausentes na partida, quando houver. Sem pastas ausentes, a resposta mantém a forma da versão anterior.

Motivo da fila ausente e id da fila configurada.

- Contratos novos (`:1`): comando `queue.configured`, só com `_meta`, e a resposta `queue.configured.response`, com o id da fila que o núcleo serve, criada ou não.
- Versões novas:
  - `local-api.error-code:6`, convivendo com a `:1` à `:5`: os códigos da `:5` mais `QUEUE_NOT_FOUND` (`-32026`, id que não é o da fila configurada) e `QUEUE_NOT_CREATED` (`-32027`, fila configurada ainda não criada). `QUEUE_NOT_CONFIGURED` passa a dizer só que os controles da fila não subiram. O núcleo passa a emitir a `:6`.
  - `local-api.server-message:9`, convivendo com a `:1` à `:8`: a resposta de `queue.configured`. O núcleo passa a escrever a `:9`.

Contagem dos pedidos de aprovação com histórico ilegível.

- Versões novas:
  - `approval.list.response:3`, convivendo com a `:1` e a `:2`: o `result` traz `unreadable`, quantos pedidos pendentes com histórico ilegível a listagem deixou de fora. O núcleo passa a responder a `:3`.
  - `local-api.server-message:7`, convivendo com a `:1` à `:6`: a resposta de `approval.list` na `:3`. O núcleo passa a escrever a `:7`; para quem envia `approval.list`, a resposta muda de forma, sem negociação.

Fechamento de pedidos de aprovação com histórico ilegível.

- Contratos novos (`:1`):
  - Comando `approval.close` (`action_id` e `decision`, `deny` ou `expire`): o dono fecha os pedidos pendentes de uma ação cujo histórico o núcleo não lê. Nunca aprova.
  - Evento `closure.recorded`: o fechamento, com aparelho, dono, decisão, os pedidos fechados e a chave de idempotência, gravado no stream `closure/<action_id>` antes de qualquer efeito. Família de stream nova `closure`, amarrada a `action_id`.
- Versão nova, convivendo com a `:1` à `:4`: `local-api.error-code:5`, com os códigos da `:4` mais `APPROVAL_READABLE` (`-32025`). O núcleo passa a emitir a `:5`.

Efeitos concretos das ações aprovadas.

- Contratos novos (`:1`):
  - Registro `record.action-effects`: o que o ensaio de uma ação achou no disco, passo a passo, com cada entrada (nomes, tipo, tamanho e identidade) e o resumo do manifesto de uma restauração.
  - Evento `effects.recorded`: esse registro e o seu hash (SHA-256 da forma canônica do registro), gravados pelo núcleo uma vez só (`seq` 1) no stream `effects/<action_id>`. Família de stream nova `effects`, amarrada a `action_id`.
  - Evento `effects.refused`: o código da recusa do ensaio, no mesmo stream e no lugar de `effects.recorded`.
- Versões novas, convivendo com a `:1`:
  - `answer.recorded:2`: acrescenta `effects_hash` obrigatório (`null` quando a ação não tem efeitos gravados). O núcleo passa a gravar a `:2`.
  - `approval.list.response:2`: cada aprovação traz `effects`, `effects_hash` e `effects_error` (ou `null`), os motivos novos `effects_refused`, `effects_unreadable` e `effects_not_listed`, e o `result` traz `more`. O núcleo passa a responder a `:2`.
- Versão nova, convivendo com a `:1` à `:4`: `local-api.server-message:5`, com a resposta de `approval.list` na `:2`. O núcleo passa a escrever a `:5`; para quem envia `approval.list`, a resposta muda de forma, sem negociação.

Aprovações do portão de ações pela API local.

- Contratos novos (`:1`):
  - Comandos `approval.list` (só `_meta`, só lê) e `approval.answer` (`action_id`, `approval_id`, `plan_hash` e `decision`, `approve` ou `deny`).
  - Resposta `approval.list.response`: `result` fechado com `approvals`, cada uma com a aprovação pedida, os motivos de não ser aprovável pela API (`not_approvable`) e o registro `record.action-plan:2`.
  - Evento `answer.recorded`: a resposta do dono, com a chave de idempotência do comando, gravada no stream `answer/<approval_id>` antes de o portão agir. Família de stream nova `answer`, amarrada a `approval_id`.
- Versão nova, convivendo com a `:1` à `:3`: `local-api.server-message:4`, que aceita também a resposta de `approval.list`. O núcleo passa a escrever a `:4`.
- Versão nova, convivendo com a `:1` e a `:2`: `local-api.error-code:3`, com os códigos da `:2` mais `APPROVAL_NOT_CONFIGURED` (`-32017`), `APPROVAL_NOT_FOUND` (`-32018`), `APPROVAL_ALREADY_ANSWERED` (`-32019`), `APPROVAL_STALE` (`-32020`), `APPROVAL_POLICY_CHANGED` (`-32021`), `APPROVAL_OUT_OF_ORDER` (`-32022`) e `APPROVAL_NOT_APPROVABLE` (`-32023`). O núcleo passa a emitir a `:3`.

- `queue.created:2` exige a política inicial completa. A `:1` continua legível.
- `common.queue-policy:1` define os limites usados na criação da fila e em `queue.policy_changed:1`, que mantém o formato aceito.

Controle da fila pela API local.

- Contratos novos (`:1`):
  - Comandos `queue.get`, `queue.start` e `queue.stop`: estendem `command.request:1` com `params` fechado em `queue_id` e `_meta`. `queue.get` só lê.
  - Resposta `queue.get.response`: `result` fechado com `queue_id` e `state` (`stopped` ou `enabled`).
- Versão nova, convivendo com a `:1` e a `:2`: `local-api.server-message:3`, que aceita também a resposta de `queue.get`. O núcleo passa a escrever a `:3`.
- Versão nova, convivendo com a `:1`: `local-api.error-code:2`, com os códigos da `:1` mais `QUEUE_NOT_CONFIGURED` (`-32015`) e `QUEUE_ALREADY_ENABLED` (`-32016`). O núcleo passa a emitir a `:2`; a `:1`, fechada, continua recusando os dois números.
- `queue.stop` não passa pela cache de idempotência: expressa o estado desejado, e `replayed` diz se a fila já estava parada.

## 2026-11-20

- Tipo comum novo `common.queue-task-title:1`: o título da tarefa da fila, com as mesmas regras que os registros `:1` e `:2` repetem por extenso.
- `queue.task_created:4` registra também o título e a ferramenta da tarefa, obrigatórios, e nunca é `public`. As versões 1 a 3 continuam legíveis.
- `record.queue-task:3` (`version` 3) é o registro que o replay reconstrói do log: `title` e `tool` são obrigatórios e `null` quando a criação é anterior à `queue.task_created:4`. Os registros `:1` e `:2` continuam legíveis.
- `session.refusal_detected:1` registra, só com metadados, a recusa de um pedido ou a interrupção por filtro de segurança, com ferramenta, provedor, modelo e tipo. O modelo é um identificador curto do catálogo, nunca texto livre.
- `session.handoff:3` acrescenta `alternative_id`, a alternativa do catálogo que o aparelho aceitou numa oferta após recusa de modelo. A `:2` continua valendo para os demais gatilhos.
- `record.session-handoff:1` registra a passagem autorizada e `session.handoff:1` registra proposta, aceite ou recusa.
- `session.handoff:2` exige um aparelho como autor de aceite e recusa. `record.session-handoff:2` referencia o evento de aceite em `consent.event_id`.
- `queue.policy_changed:1` registra a política completa aprovada por um aparelho do dono para as admissões posteriores da fila.
- `queue.task_state_changed:2` exige canal para aparelhos, recusa canal para os demais atores e registra o abandono por sessão sem confirmação como cancelamento terminal. O replay continua lendo a `:1` com as regras originais.
- Tipos comuns novos: `common.workdir-alias:1` (apelido da pasta de trabalho) e `common.queue-task-origin:1` (origem da tarefa da fila), com as mesmas regras que o registro e a criação de tarefa repetiam.
- Versões novas que referem esses tipos, sem mudar o que aceitam: `queue.task_created:3` e `record.queue-task:2` (`version` 2). Mudam os tipos gerados; a `:1` e a `:2` da criação e a `:1` do registro continuam legíveis.
- `queue.task_created:2` exige ator do tipo aparelho em pedidos manuais e convive com a `:1`.
- `queue.clock_reanchored:1` registra a reancoragem do relógio de admissão pelo dono da fila.
- `local-api.error-code:1` define os códigos de erro emitidos pela API local.
- Eventos de roteamento `proposed`, `decided` e `fallback` ganham a versão
  `:2` com `account_alias` (nulo quando não há perfil). As versões `:1` convivem com as novas.
- `account.switched:1` registra a seleção de perfil para a próxima sessão.
- `account.status_observed:1` registra metadados permitidos pelo status da
  ferramenta em evento privado.

## 2026-11-19

Lista de ferramentas na API local. Nenhum contrato existente foi alterado.

- Contratos novos (`:1`):
  - Comando `harness.list`: estende `command.request:1` com `params` fechado só no `_meta`. Só leitura.
  - Resposta `harness.list.response`: `result` fechado com `harnesses` e `workdir_aliases`, listas sem repetição de `common.id:1`. Só nomes; programa, argumentos, ambiente e caminho não cabem.
- Versão nova, convivendo com a `:1`: `local-api.server-message:2`, que aceita também a resposta de `harness.list`. O núcleo passa a escrever a `:2`.

Contratos v1 da fila contínua: registros da tarefa e do controle pelo dono, estado compartilhado da tarefa e eventos de criação, ativação, parada, inclusão de tarefa e mudança de estado.

## 2026-11-16

Contratos da API local (interface ↔ núcleo), todos `:1`. Nenhum contrato existente foi alterado.

- Comandos, cada um estendendo `command.request:1` com `method` fixo e `params` fechado: `auth.hello` (token da execução), `session.open` (sessão, ferramenta, apelido da pasta e tamanho; recusa `program`, `args`, `env` e `cwd`), `session.write` (1 a 65536 bytes em base64), `session.resize`, `session.close` e `events.subscribe` (stream e `after_seq`).
- Contrato entre processos: `local-api.server-message` (resposta `command.response:2` ou evento `event.envelope:1`).
- Gerador de Rust: extensão fechada de um objeto com propriedades (o `params` de cada comando) vira um `struct` fechado só com o que a extensão nomeia; `true` numa propriedade herda o Schema da base. Nenhum tipo gerado existente mudou.

## 2026-11-15

Lacunas apontadas pelos consumidores (interface e portão de ações). Nenhum contrato `:1` teve regra apertada.

- Versões novas, convivendo com a `:1`:
  - `event.approval.requested:2`: `action_id` obrigatório, amarrando a aprovação à ação.
  - `command.response:2`: `result` fechado em `accepted` (sempre `true`) e `replayed` (repetição de comando idempotente).
- Contratos novos (`:1`):
  - Tipos comuns: `abspath` (caminho absoluto normalizado, Windows e POSIX) e `action-rule` (código de regra do portão).
  - Eventos: `action.denied` (plano negado), `action.started` (início da execução) e `session.activity_changed` (sessão ativa ou ociosa).
  - Registro: `action-plan`, com a serialização canônica do `plan_hash` descrita no README.
- Campo opcional acrescentado (a versão não muda):
  - `event.task.created:1`: `title`.
  - `event.session.created:1`: `display_name`.
  - Evento que leva um desses campos não pode ser `public`.
- Código de regra novo na lista do README (a versão de `action-rule` e de `action-plan` não muda): `allowed_root_itself`, alvo que é uma raiz permitida ou a contém.
- Portão de ações: destino das ações, decisões de aprovação e caminho absoluto. Nenhum contrato `:1` foi alterado.
  - Versões novas, convivendo com a `:1`:
    - `event.approval.granted:2`, `event.approval.denied:2` e `event.approval.expired:2`: `action_id` obrigatório. Sem ele a decisão de uma aprovação não dizia para qual ação foi tomada, e duas ações com o mesmo plano e o mesmo `approval_id` ficavam indistinguíveis no log. O portão passa a escrever a `:2` e não reconstrói estado a partir da `:1`.
    - `common.abspath:2`: recusa, no estilo Windows, nome de dispositivo como componente (`NUL`, `COM1`, `COM¹`, com qualquer extensão) e nome curto 8.3 (`PROGRA~1`). `record.action-plan:1` continua referindo a `abspath:1`; passar o registro para a `:2` é versão nova do registro, ainda por fazer.
  - `corpus/abspath.json`: corpus comum do Schema `abspath:2` e do normalizador do núcleo, com a lista (vazia) das diferenças conhecidas. Descrito no README.
  - Código de regra novo na lista do README (a versão de `action-rule` e de `action-plan` não muda): `destination_entry_reserved`, entrada que um "mover" criaria no destino (`destino/nome do alvo`) e que é a raiz de trabalho, uma raiz permitida ou a pasta de quarentena, ou contém uma delas. `destination_protected.<proteção>` e `destination_inside_target` passam a citar também essa entrada, além da pasta receptora.
  - Ordem no tempo dentro de uma aprovação (pedido, decisão, início) documentada no README como regra entre documentos.
- Portão de ações: máximo da estimativa no registro do plano. Nenhum contrato `:1` foi alterado.
  - Versão nova, convivendo com a `:1`: `record.action-plan:2`. Aperta a `:1` em dois pontos: os caminhos passam a `common.abspath:2`, e `estimate.items` e `estimate.bytes` ganham máximo 9007199254740991 (2^53 − 1), o maior inteiro que todo leitor representa sem perda. Na `:1` o Schema não tinha máximo e o leitor Rust recusava acima de 2^64 − 1. O campo `version` do registro é `2`; a serialização do `plan_hash` não muda. O portão passa a escrever a `:2`.
  - `golden/record.action-plan.v1/valid/02-quarantined.json` (apagar sem estimativa, reescrito e permitido) não tem equivalente na `:2`: é um registro válido que o portão não escreve mais.
- Regra `stream` ↔ identificador em `data` documentada no README e conferida pelo validador em todo dourado válido. Dourados corrigidos, que citavam no `stream` outra entidade que a de `data`: `event.approval.expired.v1/valid/02-variant1.json`, `event.approval.requested.v1/valid/02-variant1.json` e `event.task.created.v1/valid/02-variant1.json`.

## 2026-11-14

Primeira versão dos contratos (todos `:1`).

- Tipos comuns: `ulid`, `id`, `timestamp`, `sha256`, `base64`, `data-class`, `risk`, `relpath`, `actor`.
- Envelope de evento e de comando (JSON-RPC 2.0), com a resposta.
- Eventos: `session.created`, `session.output`, `session.resized`, `session.ended`, `session.permission_asked`; `task.created`, `task.started`, `task.completed`, `task.failed`, `task.cancelled`; `approval.requested`, `approval.granted`, `approval.denied`, `approval.expired`; `usage.recorded`, `usage.reconciled`; `host.sample_recorded`, `host.alert_raised`, `host.alert_acked`.
- Registros: `task-packet`, `usage_event`, `approval`, `host_sample`, `host_event`.

Segunda leva, no mesmo dia (todos `:1`; nenhum contrato da primeira leva foi alterado).

- Contratos entre processos: `harness-adapter` (descritor do adaptador) e `pty-host.request`, `pty-host.response`, `pty-host.notification`, `pty-host.frame` (o contrato `pty-host/1`, com o layout do quadro binário no README).
- Eventos: `routing.proposed`, `routing.decided`, `routing.fallback`; `review.requested`, `review.completed`, `review.failed`; `action.planned`, `action.executed`, `action.failed`.
- Registros: `update-manifest`, `update-status`, `parameter`.
- Relé (só Schema): `envelope`, `device`.
