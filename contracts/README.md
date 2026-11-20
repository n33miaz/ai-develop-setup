# Contratos

Fonte única de toda mensagem e todo registro do produto: JSON Schema 2020-12, um arquivo por contrato, com exemplos dourados que o CI confere. Módulos só conversam por estes contratos (P2). Os tipos de TypeScript e de Rust saem daqui por geração; nenhum tipo de mensagem é escrito à mão.

## Pastas

| Pasta | Conteúdo |
|---|---|
| `schemas/` | Um arquivo por Schema: `<tipo>.<nome>.v<versão>.schema.json` |
| `golden/<tipo>.<nome>.v<versão>/valid/` | Exemplos que têm de passar (pelo menos 2) |
| `golden/<tipo>.<nome>.v<versão>/invalid/` | Exemplos que têm de falhar (pelo menos 3), mais `index.json` |
| `corpus/` | Casos comuns a dois validadores: o Schema e o código que lê o mesmo dado. Hoje, `abspath.json` ([caminho absoluto](#caminho-absoluto-o-schema-e-o-núcleo)) |
| `generated/ts/` | Tipos TypeScript gerados, versionados; o CI reprova se estiverem desatualizados. A interface os importa por `ui/src/api/contracts.ts` |
| `../crates/contracts/src/generated/` | Tipos Rust gerados (crate `ads-contracts`), versionados; o CI reprova se estiverem desatualizados. Como cada parte do Schema vira tipo está no [README do crate](../crates/contracts/README.md) |
| `tools/` | Validador, geradores (`gen-ts.mjs`, `gen-rust.mjs`), regras entre campos (`rules.mjs`) e os testes deles |

## Comandos

```bash
cd contracts
npm ci
npm run validate   # Schemas válidos, referências resolvidas, URNs únicas, dourados no lugar
npm run gen:ts     # regera generated/ts
npm run gen:rust   # regera ../crates/contracts/src/generated
npm run gen:check  # reprova se os tipos gerados (TypeScript e Rust) não forem o que os geradores produzem agora
npm test           # testes do próprio validador e dos geradores
```

O gerador de Rust passa a saída pelo `rustfmt` da versão fixada em `rust-toolchain.toml`, então `gen:rust`, `gen:check` e `npm test` precisam do Rust instalado. Ele para com erro diante de palavra-chave ou expressão regular que não sabe traduzir, em vez de gerar um tipo mais frouxo que o Schema, e nunca apaga arquivo: módulo que sobrou de um Schema removido é acusado e sai pelo nome. Cada dourado é conferido também contra os tipos Rust (`cargo test -p ads-contracts`): o válido tem de ir e voltar igual, o inválido tem de ser recusado.

Os testes montam árvores de contratos de mentira em `.test-tmp/` (fora do git), uma subpasta nova por execução, e não apagam nada: a limpeza dessa pasta é manual.

O `validate` confere, entre outras coisas: todo `$id` segue a URN e é único; o nome do arquivo bate com o `$id`; todo objeto com `properties` declara `additionalProperties`; cada `examples[0]` é o primeiro dourado válido; cada dourado válido passa; cada dourado inválido falha **pelo motivo declarado**; cada dourado válido cumpre as regras que o Schema não exprime (`tools/rules.mjs`: [stream e entidade](#stream-e-entidade) e [`plan_hash`](#serialização-canônica-do-plan_hash)); este README cita todo Schema.

## Identificadores (ADR 0006)

`urn:ads:<tipo>:<nome>:<versão>`, sem domínio e sem nome de fornecedor de IA.

| Tipo | Uso |
|---|---|
| `common` | Tipos pequenos reaproveitados (id, instante, digest, autor) |
| `event` | Envelope e cada tipo de evento (`urn:ads:event:session.created:1`) |
| `command` | Envelope de comando e de resposta (JSON-RPC 2.0) |
| `record` | Registros persistidos ou trocados (tarefa, uso, aprovação, amostra) |
| `contract` | Contratos entre processos (adaptador de IA, host de PTY) |
| `relay` | Mensagens do relé remoto |

A versão é a versão **maior** do contrato. Nome: minúsculas, dígitos, `_` e `-`, com pontos separando partes (`session.created`).

## Regras de versão

1. **Campo novo e opcional** não muda a versão: o contrato continua `:1`. Entra no CHANGELOG.
2. **Remover campo, mudar o tipo, apertar uma regra ou tornar obrigatório um campo opcional** cria uma versão nova da URN (`:2`). O arquivo `:1` não é alterado: os dois convivem enquanto houver quem os use.
3. **Um id nunca é reutilizado.** Contrato aposentado fica no CHANGELOG como aposentado; ninguém lhe dá outro significado.
4. Afrouxar uma regra (aceitar mais do que antes) conta como mudança compatível, mas só vale se todo leitor antigo continuar lendo o que o novo escreve; na dúvida, versão nova.
5. Todo PR que toca `schemas/` atualiza o CHANGELOG e os dourados no mesmo commit.

## Como acrescentar um contrato

1. Escolha o tipo e o nome e crie `schemas/<tipo>.<nome>.v1.schema.json` com `$schema` 2020-12, `$id`, `title` e `description` em português, `additionalProperties: false` onde couber e `examples` com o primeiro dourado válido.
2. Reaproveite os tipos `common` por `$ref` (`urn:ads:common:id:1`) em vez de repetir padrões.
3. Evento novo: estenda o envelope com `allOf` e fixe `type` (a própria URN), `stream` e `data` (veja qualquer `event.*`).
4. Crie `golden/<tipo>.<nome>.v1/valid/` com pelo menos 2 arquivos e `invalid/` com pelo menos 3. Cada inválido muda **um campo só** de um válido e ganha uma entrada em `invalid/index.json`: `rule` (a regra violada, em português), `keyword` e `instancePath` do erro esperado e, se quiser, `params`.
5. Acrescente uma linha na tabela abaixo e uma entrada no CHANGELOG.
6. `npm run gen:ts` e `npm run gen:rust`, depois `npm run validate`, `npm test` e `cargo test -p ads-contracts`. Inclua `generated/ts` e `crates/contracts/src/generated` no mesmo commit.

Dados dos dourados são sempre sintéticos: sem nomes reais, e-mails, caminhos de máquina nem valores que lembrem credencial.

## Convenções que valem em todos os Schemas

- Instantes em UTC com `Z`. Digest é `sha256:` mais 64 hexadecimais minúsculos. Bytes vão em base64 padrão.
- Entidades são identificadas por `urn:ads:common:id:1` (opaco, sem arroba); eventos e comandos, por ULID.
- Campo que pode faltar de verdade é opcional; campo que existe e pode não ter valor é obrigatório e aceita `null`.
- Regras entre campos (`if`/`then`) só cobrem o que depende de valores do próprio documento. Regra que precisa de outro documento (o hash de um plano contra o plano, por exemplo) é teste de contrato, não Schema.
- `data_class` (`public`, `internal`, `private`, `secret`) acompanha todo evento; texto de terminal e perguntas de permissão nunca são `public`, e evento que leva texto escrito pelo usuário (`title` em `task.created`, `display_name` em `session.created`) também não.
- Caminho absoluto é `urn:ads:common:abspath:2`, sempre já normalizado: `C:\pasta\arquivo` ou `/pasta/arquivo`, com `C:\` e `/` como raízes. O Schema recusa caminho relativo, `..`, `.`, variável (`$`, `%`), til no começo de componente, curinga, separador sobrando e caminho de rede; no estilo Windows, também componente terminado em ponto ou espaço, nome de dispositivo e nome curto 8.3. A `:1` não recusa os dois últimos: ver [Caminho absoluto: o Schema e o núcleo](#caminho-absoluto-o-schema-e-o-núcleo).

## Caminho absoluto: o Schema e o núcleo

Dois validadores leem o mesmo texto: o Schema `urn:ads:common:abspath:2` e o normalizador do núcleo (`ads-core`, que só grava caminho que ele mesmo aceita e não reescreve). Os dois têm de concordar. `corpus/abspath.json` é o corpus comum: cada caso traz o caminho e se ele vale; `tools/corpus.test.mjs` roda o Schema e `crates/core/tests/gate_golden.rs` roda o núcleo sobre o mesmo arquivo, e cada um reprova se o seu veredito não for o declarado. Caso novo de divergência entra no corpus antes da correção.

A `:2` acrescenta à `:1`, só no estilo Windows (no POSIX esses nomes são nomes comuns):

| Recusado | Regra |
|---|---|
| Nome de dispositivo como componente | `CON`, `PRN`, `AUX`, `NUL`, `COM0` a `COM9`, `LPT0` a `LPT9` e os mesmos com `¹`, `²` ou `³` no lugar do dígito; em qualquer caixa ASCII, com qualquer extensão (o radical vai até o primeiro ponto, sem os espaços finais: `nul.tar.gz`, `NUL .txt`) |
| Nome curto 8.3 | til seguido de dígitos até o fim do nome ou até um ponto (`PROGRA~1`, `REPORT~2.TXT`); `a~b` e `v~1x` valem |
| Componente terminado em ponto ou espaço | já era recusado na `:1`; a `:2` ganhou os dourados |

Diferenças conhecidas entre os dois validadores, em `known_differences` do corpus: **nenhuma**. A lista existe para que uma divergência aceita de propósito fique escrita, com o veredito de cada lado e o motivo, em vez de escondida.

O que continua sendo só do núcleo, porque texto nenhum resolve:

- **Normalização da entrada.** O Schema só aceita o caminho já normalizado; quem transforma `c:/work//proj/./src/` em `C:\work\proj\src`, ou recusa, é o núcleo.
- **Identidade no sistema de arquivos.** Link simbólico, junção, ponto de montagem, apelido de volume, caixa e forma Unicode (NFC e NFD são textos diferentes, e alguns sistemas de arquivos os tratam como o mesmo nome). O Schema e o núcleo comparam texto; quem executa confere no disco.
- **Política.** Pasta protegida, raiz de trabalho, raízes permitidas e quarentena dependem da configuração e do plano, não do caminho sozinho.
- **Regras entre campos** do registro do plano: ordem canônica e não repetição dos alvos e o `plan_hash` (abaixo).

`urn:ads:record:action-plan:2` refere a `abspath:2`; a `:1` do registro continua referindo a `abspath:1` e por isso ainda aceita nome de dispositivo e nome curto (quem os recusa, na `:1`, é o núcleo). O portão escreve a `:2`.

## Stream e entidade

O `stream` de um evento é `<família>/<id>`, e o `<id>` é o da entidade que o próprio `data` nomeia. JSON Schema não compara dois campos do mesmo documento, então o Schema fixa só a família; a igualdade é regra de contrato, conferida pelo validador em todo dourado válido e exportada para quem produz ou lê eventos (`streamBindingError` em `tools/rules.mjs`).

| Família | Campo de `data` que dá o `<id>` |
|---|---|
| `session` | `session_id` |
| `task` | `task_id` |
| `approval` | `approval_id` |
| `action` | `action_id` |
| `review` | `review_id` |
| `routing` | `task_id` |
| `queue` | `queue_id` |

`host` e `usage` ficam fora da regra: o stream deles agrupa por máquina ou por tarefa, e nem todo evento dessas famílias leva esse identificador em `data`.

## Comandos e respostas

`urn:ads:command:response:1` deixa o `result` aberto. `urn:ads:command:response:2` o fecha em `{ "accepted": true, "replayed": <booleano> }`:

- `accepted` é sempre `true`: comando recusado responde com `error`, nunca com `result`.
- `replayed` é `false` na primeira vez que a chave de idempotência (`params._meta.idempotency_key`) é aceita e `true` em toda repetição da mesma chave. Na repetição nada é executado de novo.
- O efeito do comando não vem no `result`: volta como evento.

As duas versões convivem; quem ainda responde `{ "accepted": true }` segue na `:1`.

## Portão de ações

### Ciclo de uma ação nos eventos

Cada estado da ação é reconstruível do log. Os eventos `action.*` ficam no stream `action/<action_id>`; os `approval.*`, no stream `approval/<approval_id>`, amarrados à ação pelo `action_id` que os quatro levam na versão `:2` (o pedido e cada decisão: conceder, negar, expirar) e pelo `approval_id` de `action.started` e `action.executed`. A `:1` dos quatro não diz de qual ação é: duas ações com o mesmo plano e o mesmo `approval_id` ficariam indistinguíveis no log, e por isso o portão não a escreve nem reconstrói estado a partir dela.

Ordem no tempo dentro de uma aprovação, conferida pelo portão na escrita e na reconstrução: o instante (`ts`) de `approval.granted` e de `approval.denied` não é anterior ao de `approval.requested`; o de `action.started` não é anterior ao de `approval.granted`; `approval.requested`, `approval.granted` e `action.started` são anteriores a `expires_at`, e `approval.expired` não é. Instantes iguais valem. É regra entre documentos, não de Schema.

| Evento | Estado depois dele |
|---|---|
| `action.denied` | negada (final). É o único evento da ação: vem no lugar de `action.planned` |
| `action.planned` | planejada |
| `approval.requested` | aguardando aprovação |
| `approval.granted` | aprovada |
| `approval.denied` | aprovação negada (final) |
| `approval.expired` | aprovação expirada (final) |
| `action.started` | executando |
| `action.executed` | executada (final) |
| `action.failed` | falhou (final) |

"Negada" é veredito do portão sobre um plano normalizado. Pedido que nem chega a virar plano (alvo relativo, variável não expandida, lista vazia) não tem caminho absoluto nem hash, e por isso não tem evento nem registro.

### Registro do plano

`urn:ads:record:action-plan:2` guarda o plano como o portão o julgou. A `:1` tem os mesmos campos; a `:2` a aperta em dois pontos (regra 2 de versão), e é a que o portão escreve:

- os caminhos (`work_root`, `destination`, `targets[].path`) são `urn:ads:common:abspath:2`;
- `estimate.items` e `estimate.bytes` vão de 0 a 9007199254740991 (2^53 − 1). É o intervalo comum a todo leitor: um número JSON lido em JavaScript é um double e só representa inteiros sem perda até aí, e o leitor Rust usa 64 bits sem sinal. Na `:1` o Schema não tinha máximo e o leitor Rust recusava acima de 2^64 − 1: dois leitores, dois vereditos. Plano com estimativa acima do máximo não tem registro e, por isso, não vira ação.

O `version` do registro acompanha a versão da URN (`1` ou `2`). O texto do `plan_hash` não muda entre as duas: a primeira linha continua `ads.action-plan/1`, que é a versão da serialização, não a do registro.

Campos:

- `operation` é a operação **pedida** (`fs.delete`, `fs.delete_recursive`, `fs.move`, `fs.overwrite`, `fs.clean_untracked`).
- `targets`: cada alvo tem `path` (caminho absoluto literal), `pattern` (curinga sobre os filhos de `path`, ou `null`) e `repo_root` (o chamador viu que o alvo é raiz de repositório). Vêm na ordem canônica, sem repetição.
- `work_root`, e `estimate` com `items` e `bytes` (`null` quando ninguém mediu).
- `verdict`: `decision` (`allowed`, `requires_approval`, `denied`), o `risk` correspondente (`low`, `medium`, `high`) e `reasons`, uma entrada por regra que disparou, com o código da regra (`urn:ads:common:action-rule:1`) e o caminho em que disparou.
- `quarantined` e `destination`: quando `quarantined` é `true`, o apagar pedido foi reescrito como mover para a pasta de quarentena, e `destination` é essa pasta (o destino reescrito). Em `fs.move`, `destination` é o destino pedido. Nos demais casos é `null`.
- `plan_hash` cobre o plano **em vigor**: o reescrito, quando há quarentena. É o mesmo hash de `action.planned`, das aprovações e de `action.started`, `action.executed` e `action.failed`. O nome da pasta de quarentena sai do hash do plano como pedido, o que este registro não reconfere.

Códigos de regra em uso (código novo não muda a versão do Schema). Onde há `<proteção>`, ela é uma de `filesystem_root`, `user_home`, `user_home_ancestor`, `user_profile`, `system_dir`, `other_path_style`.

| Veredito | Códigos |
|---|---|
| `denied` | `protected.<proteção>`, `work_root_protected.<proteção>`, `work_root_itself`, `allowed_root_itself`, `outside_allowed_roots`, `repository_root`, `quarantine_itself`, `destination_protected.<proteção>`, `destination_outside_allowed_roots`, `destination_inside_target`, `destination_entry_reserved` |
| `requires_approval` | `outside_work_root`, `destination_outside_work_root`, `estimate_unknown`, `item_limit_exceeded`, `byte_limit_exceeded`, `quarantine_purge` |

### Serialização canônica do plan_hash

`plan_hash` é `sha256:` mais o SHA-256, em hexadecimais minúsculos, dos bytes UTF-8 do texto abaixo. Uma linha por campo, nesta ordem, cada linha terminada por `\n` (inclusive a última), sem espaço sobrando:

```
ads.action-plan/1
op:<operação em vigor>
work_root:<n>:<work_root>
destination:<n>:<destination>
estimate:<items>:<bytes>
targets:<quantidade de alvos>
target:<repo_root>:<n>:<texto do alvo>
```

- `<operação em vigor>`: `fs.move` quando `quarantined` é `true`; senão, `operation`.
- `<n>` é o tamanho, em bytes UTF-8, do caminho que vem logo depois: nenhum caminho consegue se passar por dois campos.
- `destination` nulo vira a linha `destination:-`; `estimate` nulo vira `estimate:unknown`. `items` e `bytes` vão em decimal, sem zeros à esquerda.
- Uma linha `target:` por alvo, na ordem do registro. `<repo_root>` é `1` ou `0`. O texto do alvo é `path`; com curinga, `path`, o separador do estilo (`\` ou `/`; nenhum quando `path` é a raiz) e `pattern`.

Normalização, feita antes de gravar o registro (o Schema a exige nos caminhos):

- Caminhos na forma de `urn:ads:common:abspath:1`: unidade em maiúscula, um separador só, sem separador no fim, sem `.`; nada é reparado além disso (`..`, variável e til recusam o plano inteiro).
- Alvos repetidos viram um só; se um dos repetidos é raiz de repositório, fica `repo_root` verdadeiro.
- Ordem dos alvos: primeiro os de estilo Windows, depois os POSIX; depois pela letra da unidade; depois componente a componente, comparando os bytes UTF-8 de cada nome (o caminho mais curto vem antes do que o estende; maiúscula e minúscula são diferentes); por fim, alvo sem curinga antes do mesmo `path` com curinga, e curingas pelos bytes. É comparação por componente, não do texto inteiro: `/a/b` vem antes de `/a-b`.

Exemplo (o primeiro dourado válido do registro; o mesmo vetor do teste do núcleo):

```
ads.action-plan/1
op:fs.delete_recursive
work_root:10:/work/proj
destination:-
estimate:3:4096
targets:2
target:0:12:/work/proj/a
target:0:12:/work/proj/b
```

resulta em `sha256:c7b870d89ef435eb431fcdf8e99f82f2b10c9b05a25eb86eb2ed603fa366eb94`.

O validador recalcula o hash e confere a ordem dos alvos em todo dourado válido do registro, nas duas versões (`planHash` e `actionPlanErrors` em `tools/rules.mjs`).

## Contrato pty-host/1

Canal local do usuário entre o núcleo (cliente) e o host de PTY (servidor). O host segura as sessões e o anel de replay; o núcleo reinicia, reconecta e retoma pelo `seq` (ADR 0003).

### Quadro

Tudo o que passa pelo canal vai dentro de um quadro. Inteiros em big-endian.

| Deslocamento | Tamanho | Campo |
|---|---|---|
| 0 | 1 | `kind`: `0x01` mensagem JSON-RPC, `0x02` saída de terminal. Outro valor encerra a conexão. |
| 1 | 4 | `payload_len` (u32): quantos bytes vêm depois deste campo |
| 5 | `payload_len` | `payload` |

`payload` de um quadro `0x01`: uma mensagem JSON em UTF-8, que é um `pty-host.request`, um `pty-host.response` ou um `pty-host.notification`.

`payload` de um quadro `0x02` (só do host para o núcleo):

| Deslocamento | Tamanho | Campo |
|---|---|---|
| 0 | 2 | `header_len` (u16) |
| 2 | `header_len` | cabeçalho: JSON em UTF-8 que valida contra `urn:ads:contract:pty-host.frame:1` |
| 2 + `header_len` | `length` do cabeçalho | bytes do terminal, sem transformação |

`payload_len` tem de ser igual a `2 + header_len + length`; quadro que não fecha a conta encerra a conexão. O JSON Schema cobre só o cabeçalho: os bytes não cabem em Schema e a conta dos tamanhos é teste de contrato.

### Métodos

| Método | `params` | `result` |
|---|---|---|
| `host.hello` | `contract` | `contract` |
| `session.open` | `session_id`, `program`, `args`, `cwd`, `env`, `cols`, `rows` | vazio |
| `session.write` | `session_id`, `data` (base64) | vazio |
| `session.resize` | `session_id`, `cols`, `rows` | vazio |
| `session.subscribe` | `session_id`, `from_seq` (ou `null`: tudo o que o anel guarda) | `oldest_seq`, `next_seq` |
| `session.close` | `session_id` | `exit_code` |
| `session.list` | vazio | `sessions` |

- O primeiro pedido de toda conexão é `host.hello`; versão que o host não fala volta como erro `-32009`.
- O processo filho enxerga só o `env` do pedido: nada é herdado do host.
- Depois de `session.subscribe` o host manda quadros `0x02` em ordem de `seq`, primeiro o que o anel guarda e depois a saída ao vivo, sem buraco e sem duplicata. A assinatura vale até a conexão cair, a sessão ser fechada ou o aviso `subscription.closed`.
- Avisos (sem `id`): `session.exited` e `subscription.closed`, os dois com `last_seq`.

### Códigos de erro

Além dos cinco do JSON-RPC 2.0 (`-32700`, `-32600`, `-32601`, `-32602`, `-32603`):

| Código | Significado |
|---|---|
| `-32001` | Sessão desconhecida |
| `-32002` | Já existe sessão com esse `session_id` |
| `-32003` | Limite de sessões atingido (`data.limit`) |
| `-32004` | O `seq` pedido já saiu do anel (`data.oldest_seq` obrigatório) |
| `-32005` | O `seq` pedido ainda não existe (`data.next_seq` obrigatório) |
| `-32006` | A sessão já terminou |
| `-32007` | Não foi possível iniciar o processo |
| `-32008` | Falha de leitura ou escrita no terminal |
| `-32009` | Versão de contrato não suportada |

## API local

WebSocket em loopback entre a interface (navegador ou casca) e o núcleo. Como a conexão é autenticada está na [ADR 0020](../docs/adr/0020-local-api-authentication.md). Cada mensagem é um texto JSON.

- Da interface ao núcleo: comandos `command.request:1`, cada método com Schema próprio que fixa `method` e fecha `params` (tabela [API local](#api-local-1)). O primeiro comando de toda conexão é `auth.hello`; antes dele o núcleo não aceita outro.
- Do núcleo à interface: `urn:ads:contract:local-api.server-message:1`, que é a resposta `command.response:2` a um comando ou um evento `event.envelope:1` de um stream assinado por `events.subscribe`. A saída do terminal chega como evento `session.output:1`.
- A interface escolhe a ferramenta pelo identificador do adaptador (`harness`) e a pasta pelo apelido (`workdir_alias`). `session.open` recusa `program`, `args`, `env` e `cwd`: o que roda, com que argumentos, em que ambiente e em que pasta sai só da configuração do usuário no núcleo.
- Limites: colunas e linhas de 1 a 1000 (os mesmos do `pty-host/1`); `session.write` leva de 1 a 65536 bytes (87384 caracteres em base64), o que cabe com folga no quadro padrão do `pty-host/1` (`max_frame_bytes` de 1 MiB). Texto maior vai em vários comandos, na ordem.
- O Schema aceita `origin: "remote"` no `_meta`, como o envelope de comando. Quem recusa comando remoto na API local é o núcleo (ADR 0020), não o Schema: o mesmo comando pode chegar, assinado, pelo transporte remoto.

`server-message` é `anyOf`, não `oneOf`: as duas alternativas são objetos fechados e cada uma exige um campo que a outra recusa (`jsonrpc` na resposta; `type` no evento), então nenhum documento cabe nas duas e `anyOf` aceita exatamente o mesmo que `oneOf` aceitaria. O gerador de Rust só traduz `oneOf` com um campo de valor fixo que distinga as alternativas.

## Fila contínua

A criação fica registrada em `queue.created`, sempre em `stopped`. Na partida, o núcleo restaura toda fila em `stopped`, inclusive se o retrato persistido ou o último evento indicar `enabled`. Uma nova ativação precisa de `queue.enabled` após a partida.

O produtor obtém `owner_id` do cadastro de identidade autenticada ao criar a fila. Em `queue.enabled` e `queue.stopped`, compara o aparelho autenticado com esse cadastro e registra o identificador em `actor.id` e `data.owner_id`. A ativação aceita só `local_ui` e `tray`. Criação, parada e aceite de tarefa também aceitam `remote_device`, reservado para um futuro transporte autenticado e assinado.

`queue.task_created` registra o estado inicial `queued`, zero tentativas e o teto imutável de 1 a 10, incluindo a primeira. O teto e a origem são recuperados do evento de criação. A chave de deduplicação combina fila, apelido e origem. A origem manual usa um identificador estável do pedido, a issue usa seu número no repositório do apelido e a regra de bug usa o par regra e alvo estável. O produtor reserva a chave e aplica o limite da fila ao anexar a criação, em uma operação, preservando chaves de tarefas encerradas e no replay.

O apelido é um identificador de cadastro e a resolução da pasta usa esse cadastro. Branch opcional segue a forma ASCII `feature/...` ou `fix/...` e referências opcionais de pull request são números positivos do repositório cadastrado. O executor passa `--` antes da referência em comandos Git que aceitam esse separador.

| Estado atual | Destinos permitidos |
|---|---|
| `queued` | `running` se a fila correspondente estiver ligada e houver tentativa, ou `cancelled` |
| `running` | `review`, `failed` ou `cancelled` |
| `review` | `done` com aceite do dono autenticado registrado em `actor` e `via`, `failed` ou `cancelled` |
| `failed` | `queued` se houver tentativa, ou `cancelled` |
| `done`, `cancelled` | nenhum |

`failed` sem saldo encerra a execução. Cancelar continua permitido para encerrar administrativamente a tarefa, inclusive com contagem inconsistente. A entrada em `running` consome uma tentativa. As outras transições preservam a contagem. O produtor confere a mudança contra o registro atual antes de emitir `queue.task_state_changed`. A contagem respeita o teto e a data de atualização é igual ou posterior à criação.

## Atualização

O manifesto (`urn:ads:record:update-manifest:1`) é assinado no CI. A assinatura fica em arquivo à parte e cobre os bytes exatos do arquivo do manifesto, para não depender de forma canônica de JSON; o lançador a confere com a chave pública embutida antes de ler qualquer campo, e só então confere o digest de cada arquivo (ADR 0015).

## Schemas

### Tipos comuns

| Schema | O que descreve |
|---|---|
| `urn:ads:common:ulid:1` | Identificador de evento ou de comando: ULID de 26 caracteres em Crockford base32, ordenável por tempo. |
| `urn:ads:common:id:1` | Identificador opaco de entidade (tarefa, sessão, aparelho, alias): 1 a 64 caracteres de letras, dígitos e . _ : -, sem espaço nem arroba. |
| `urn:ads:common:timestamp:1` | Instante em UTC no formato RFC 3339 com sufixo Z, com fração opcional de até 6 dígitos. |
| `urn:ads:common:sha256:1` | Digest SHA-256 no formato sha256: seguido de 64 hexadecimais minúsculos. |
| `urn:ads:common:base64:1` | Bytes em base64 padrão (RFC 4648) com preenchimento; vazio é permitido. |
| `urn:ads:common:data-class:1` | Classe de sensibilidade de um dado: public, internal, private ou secret. Segredo nunca sai da máquina. |
| `urn:ads:common:risk:1` | Nível de risco de uma ação ou tarefa: low, medium ou high. |
| `urn:ads:common:relpath:1` | Caminho relativo com barra normal, sem raiz, sem letra de unidade e sem segmento '..'. |
| `urn:ads:common:abspath:1` | Caminho absoluto literal, já normalizado, no estilo Windows ou POSIX: sem relativo, '..', variável, til, curinga nem caminho de rede. Aceita nome de dispositivo e nome curto do Windows. |
| `urn:ads:common:abspath:2` | O mesmo caminho, recusando também, no estilo Windows, nome de dispositivo e nome curto 8.3. Convive com a `:1`. |
| `urn:ads:common:action-rule:1` | Código estável de uma regra do portão de ações que disparou num plano. |
| `urn:ads:common:actor:1` | Quem causou um evento ou uma decisão: um aparelho, um agente de IA ou o próprio daemon. |
| `urn:ads:common:queue-task-state:1` | Estado de uma tarefa da fila. |

### Envelopes

| Schema | O que descreve |
|---|---|
| `urn:ads:event:envelope:1` | Envelope único de todo evento: fato gravado só-anexo, numerado por stream, com autor, causa e classe do dado. Cada tipo de evento estende este envelope e fixa type, stream e data. |

### Comandos

| Schema | O que descreve |
|---|---|
| `urn:ads:command:request:1` | Comando JSON-RPC 2.0 enviado ao núcleo. Comando é pedido, não fato; a chave de idempotência evita executar duas vezes, e a origem remota exige assinatura do aparelho, nonce, contador e validade. |
| `urn:ads:command:response:1` | Resposta JSON-RPC 2.0 a um comando: ou result ou error, nunca os dois. O result é aberto. |
| `urn:ads:command:response:2` | A mesma resposta com o result fechado: comando aceito e o sinal replayed, que marca a repetição de um comando idempotente. Convive com a `:1`. |

### API local

Os seis comandos estendem `urn:ads:command:request:1` com `allOf`, fixam `method` e fecham `params` (o `_meta` continua o do envelope).

| Schema | O que descreve |
|---|---|
| `urn:ads:command:auth.hello:1` | Primeira mensagem de toda conexão da interface com a API local: o token aleatório da execução (32 bytes em base64url sem preenchimento). |
| `urn:ads:command:session.open:1` | Abre uma sessão de terminal com uma ferramenta e uma pasta escolhidas só pelo nome (identificador do adaptador e apelido). Nunca leva programa, argumentos, ambiente nem pasta. |
| `urn:ads:command:session.write:1` | Bytes para a entrada do terminal, em base64: de 1 a 65536 bytes por comando. |
| `urn:ads:command:session.resize:1` | Novo tamanho do terminal, de 1 a 1000 colunas e linhas. |
| `urn:ads:command:session.close:1` | Encerra a sessão de terminal e o processo dela. |
| `urn:ads:command:events.subscribe:1` | Assina os eventos de um stream a partir de um seq (0: tudo o que o núcleo guarda). |
| `urn:ads:contract:local-api.server-message:1` | O que o núcleo manda à interface pelo WebSocket: resposta `command.response:2` ou evento `event.envelope:1`. |

### Eventos de sessão

| Schema | O que descreve |
|---|---|
| `urn:ads:event:session.created:1` | Uma sessão de terminal foi aberta no host de PTY para um agente de IA. Leva, opcional, o nome de exibição. |
| `urn:ads:event:session.activity_changed:1` | A sessão passou de ativa para ociosa ou de ociosa para ativa. Sessão recém-criada conta como ativa. |
| `urn:ads:event:session.output:1` | Bytes que o terminal produziu, em base64, com deslocamento no fluxo da sessão. No transporte ao vivo viajam em quadro binário; este é o registro. |
| `urn:ads:event:session.resized:1` | O terminal da sessão mudou de tamanho. |
| `urn:ads:event:session.ended:1` | A sessão terminou: o processo saiu, foi morto, o host de PTY reiniciou ou houve erro. Saída normal traz o código de saída. |
| `urn:ads:event:session.permission_asked:1` | O CLI de IA pediu permissão para algo (comando, escrita, rede). O resumo pode conter texto do usuário, então a classe é private ou secret. |

### Eventos de tarefa

| Schema | O que descreve |
|---|---|
| `urn:ads:event:task.created:1` | Uma tarefa nasceu. O objetivo vai só como hash (o texto fica no pacote da tarefa); o título, opcional, é o rótulo curto que a interface mostra. |
| `urn:ads:event:task.started:1` | A execução da tarefa começou, numa sessão ou ainda sem sessão. |
| `urn:ads:event:task.completed:1` | A tarefa terminou com entrega; registra se os testes passaram e o digest do diff. |
| `urn:ads:event:task.failed:1` | A tarefa terminou sem entrega, com um código de erro estável e a indicação de se vale tentar de novo. |
| `urn:ads:event:task.cancelled:1` | A tarefa foi interrompida por decisão do dono, por teto de custo, por pressão de recursos, por política ou por ter sido substituída. |

### Eventos da fila

| URN | Significado |
|---|---|
| `urn:ads:event:queue.task_created:1` | Uma tarefa entrou na fila. |
| `urn:ads:event:queue.task_state_changed:1` | Uma tarefa mudou de estado. |
| `urn:ads:event:queue.created:1` | A fila foi criada parada. |
| `urn:ads:event:queue.enabled:1` | O dono ligou a fila. |
| `urn:ads:event:queue.stopped:1` | O dono parou a fila. |

### Eventos de aprovação

| Schema | O que descreve |
|---|---|
| `urn:ads:event:approval.requested:1` | Uma ação com risco pede aprovação: vale para o hash do plano e expira na validade indicada. Não diz de qual ação é. |
| `urn:ads:event:approval.requested:2` | O mesmo pedido com action_id obrigatório, que amarra a aprovação à ação. Convive com a `:1`. |
| `urn:ads:event:approval.granted:1` | A aprovação foi concedida para o hash do plano, por um dos três caminhos permitidos. Não diz para qual ação. |
| `urn:ads:event:approval.granted:2` | A mesma concessão com action_id obrigatório: a ação para a qual a decisão foi tomada. Convive com a `:1`. |
| `urn:ads:event:approval.denied:1` | A aprovação foi negada para o hash do plano, com motivo opcional. Não diz para qual ação. |
| `urn:ads:event:approval.denied:2` | A mesma negação com action_id obrigatório. Convive com a `:1`. |
| `urn:ads:event:approval.expired:1` | O prazo da aprovação passou sem decisão; o plano precisa ser refeito. Não diz de qual ação. |
| `urn:ads:event:approval.expired:2` | A mesma expiração com action_id obrigatório. Convive com a `:1`. |

### Eventos de uso

| Schema | O que descreve |
|---|---|
| `urn:ads:event:usage.recorded:1` | Uma chamada ou turno de IA foi medido: o dado é um registro usage_event completo, gravado só-anexo no banco de métricas. |
| `urn:ads:event:usage.reconciled:1` | Duas fontes mediram o mesmo turno; ficou um registro e as outras foram descartadas pela chave de deduplicação. |

### Eventos do host

| Schema | O que descreve |
|---|---|
| `urn:ads:event:host.sample_recorded:1` | Um coletor mediu um sinal da máquina; o dado é um registro host_sample. |
| `urn:ads:event:host.alert_raised:1` | Uma regra de alerta disparou a partir de amostras; o dado é um registro host_event. |
| `urn:ads:event:host.alert_acked:1` | O dono confirmou ter visto um alerta; o alerta original não é alterado. |

### Eventos de roteamento

| Schema | O que descreve |
|---|---|
| `urn:ads:event:routing.proposed:1` | O roteador propôs, para uma tarefa, a faixa, a ferramenta, o provedor e o modelo, citando a regra que usou. Proposta não é decisão: nada roda por causa deste evento. |
| `urn:ads:event:routing.decided:1` | A rota em vigor de uma tarefa foi fixada: pelo dono (modo manual, ou aceite de uma proposta) ou por regra (modo auto). O autor do evento diz quem decidiu. |
| `urn:ads:event:routing.fallback:1` | A rota de uma tarefa caiu para o plano B depois de uma falha: registra de onde saiu, para onde foi e o código do erro que causou a troca. Nunca troca de conta. |

### Eventos de revisão

| Schema | O que descreve |
|---|---|
| `urn:ads:event:review.requested:1` | Um revisor foi chamado para avaliar a entrega de uma tarefa. Só o digest do diff vai no evento; o diff fica fora. |
| `urn:ads:event:review.completed:1` | O revisor deu o veredito sobre o diff indicado e contou os apontamentos. O texto da revisão não vai no evento. |
| `urn:ads:event:review.failed:1` | A revisão terminou sem veredito, com um código de erro estável e a indicação de se vale tentar de novo. Revisão que falha não aprova nada. |

### Eventos de ação

| Schema | O que descreve |
|---|---|
| `urn:ads:event:action.planned:1` | O ensaio (dry-run) de uma ação produziu um plano: o verbo, o risco, o hash do plano e se pede elevação. Nada foi alterado ainda; a aprovação e a execução valem só para este hash. |
| `urn:ads:event:action.denied:1` | O portão negou o plano: nada será executado e não cabe aprovação. Cita o hash do plano negado e as regras que o negaram. |
| `urn:ads:event:action.started:1` | A execução do plano foi autorizada e começou, com a aprovação citada (ou sem aprovação, quando a política a dispensa). |
| `urn:ads:event:action.executed:1` | A ação foi executada exatamente como o plano do hash indicado, com a aprovação citada (ou sem aprovação, quando a política a dispensa). |
| `urn:ads:event:action.failed:1` | A ação não terminou como o plano previa; registra o hash do plano e um código de erro estável. O que foi feito pela metade é assunto do plano de reparo, não deste evento. |

### Registros

| Schema | O que descreve |
|---|---|
| `urn:ads:record:task-packet:1` | O que um agente recebe para executar uma tarefa: objetivo, critérios de aceite, restrições, orçamento e entradas. O receptor trata o pacote como dado, nunca como instrução; permissões vêm da configuração do usuário. |
| `urn:ads:record:usage_event:1` | Uma linha do livro-caixa: uma chamada ou turno de IA medido, com tokens, duração, custo de lista, custo faturável (só chave de API), janela de cota, fonte e chave de deduplicação. Só-anexo; alias de conta nunca é credencial nem e-mail. |
| `urn:ads:record:action-plan:1` | O plano de uma ação destrutiva sobre arquivos, como o portão o julgou: operação, alvos, raiz de trabalho, estimativa, veredito com as regras que dispararam, marca de quarentena e destino. O plan_hash é reconferível pela [serialização canônica](#serialização-canônica-do-plan_hash). Caminhos `abspath:1` e estimativa sem máximo. |
| `urn:ads:record:action-plan:2` | O mesmo registro com os caminhos em `abspath:2` e a estimativa limitada a 2^53 − 1. Convive com a `:1`; é a que o portão escreve. |
| `urn:ads:record:approval:1` | Estado de uma aprovação: vale para o hash de um plano e expira. Pendente e expirada não têm decisor; concedida e negada têm autor e instante da decisão. |
| `urn:ads:record:host_sample:1` | Uma observação de um sinal da máquina (disco, memória, etc.) feita por um coletor só leitura. Amostra ausente nunca vira zero; corrigir é gravar outra amostra com outro sample_id. |
| `urn:ads:record:host_event:1` | Um alerta derivado de amostras: cita a regra, a gravidade e os sample_id que o justificam. Confirmar é um evento posterior; o alerta original não muda. |
| `urn:ads:record:update-manifest:1` | O que o CI publica a cada junção: qual build é, de qual commit saiu e o SHA-256 de cada arquivo. O lançador confere a assinatura do manifesto (arquivo à parte, sobre os bytes exatos deste JSON) com a chave pública embutida e depois o digest de cada arquivo, antes de trocar qualquer coisa. |
| `urn:ads:record:update-status:1` | Estado visível da auto-atualização, escrito pelo lançador: o que está rodando, a versão anterior guardada, a candidata em andamento, a prova depois da troca, as quedas contadas e as versões bloqueadas. Baixando, conferindo e trocando têm candidata; em prova há prazo; falha traz código de erro. |
| `urn:ads:record:parameter:1` | Um parâmetro do produto como o núcleo o apresenta: a chave, o tipo, o padrão recomendado, o valor em vigor e a explicação. Todo comportamento com mais de uma escolha razoável é um registro destes. Parâmetro nunca guarda segredo: segredo fica no cofre. |
| `urn:ads:record:queue-task:1` | Registro de uma tarefa da fila. |
| `urn:ads:record:task-queue:1` | Identidade, dono e estado da fila. |

### Contratos entre processos

O contrato `harness-adapter/1` é o descritor abaixo. O contrato `pty-host/1` são os quatro Schemas `pty-host.*` mais o quadro binário descrito em [Contrato pty-host/1](#contrato-pty-host1).

| Schema | O que descreve |
|---|---|
| `urn:ads:contract:harness-adapter:1` | O que um adaptador declara ao núcleo no contrato harness-adapter/1: qual ferramenta adapta, se é linha de comando ou API, em que modos e papéis atende e quem guarda a credencial. Adaptador de linha de comando nunca toca na credencial que a ferramenta administra; adaptador de API usa a chave do cofre do produto e não tem modo interativo. |
| `urn:ads:contract:pty-host.request:1` | Pedido JSON-RPC 2.0 do núcleo ao host de PTY no contrato pty-host/1, com envelope próprio: id inteiro por conexão, sem chave de idempotência nem assinatura, porque o canal é local e do próprio usuário. Os bytes de saída do terminal não vêm em resposta: viajam no quadro binário. |
| `urn:ads:contract:pty-host.response:1` | Resposta JSON-RPC 2.0 do host de PTY: ou result ou error, nunca os dois. O result tem uma das formas do contrato (vazio, saudação, limites do replay, encerramento ou lista de sessões). Erro de replay informa o limite: nada é pulado em silêncio. |
| `urn:ads:contract:pty-host.notification:1` | Notificação JSON-RPC 2.0 (sem id, sem resposta) do host de PTY ao núcleo: o processo de uma sessão terminou, ou uma assinatura foi encerrada. last_seq é o último pedaço numerado (0 se não houve saída); o núcleo reassina a partir de last_seq + 1 sem perder nada. |
| `urn:ads:contract:pty-host.frame:1` | Cabeçalho, em JSON, do quadro binário que leva os bytes de saída do terminal no contrato pty-host/1: de qual sessão são, o número de sequência do pedaço e quantos bytes vêm logo depois do cabeçalho. Os bytes não fazem parte do JSON; o layout do quadro está no README. |

### Relé

Só Schema: o relé ainda não tem código.

| Schema | O que descreve |
|---|---|
| `urn:ads:relay:envelope:1` | A única coisa que o relé transporta e guarda na caixa de mensagens: texto cifrado de ponta a ponta, de um aparelho para outro, com validade. O relé nunca vê texto em claro nem credencial; depois da validade a mensagem é descartada. |
| `urn:ads:relay:device:1` | O que o relé guarda de um aparelho: o identificador opaco e a chave pública (32 bytes em base64). Chave privada, passkey e credencial de IA nunca chegam ao relé; a passkey é conferida pelo núcleo, dentro do canal cifrado. |
