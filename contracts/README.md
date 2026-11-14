# Contratos

Fonte única de toda mensagem e todo registro do produto: JSON Schema 2020-12, um arquivo por contrato, com exemplos dourados que o CI confere. Módulos só conversam por estes contratos (P2). Os tipos de TypeScript saem daqui por geração; nenhum tipo de mensagem é escrito à mão.

## Pastas

| Pasta | Conteúdo |
|---|---|
| `schemas/` | Um arquivo por Schema: `<tipo>.<nome>.v<versão>.schema.json` |
| `golden/<tipo>.<nome>.v<versão>/valid/` | Exemplos que têm de passar (pelo menos 2) |
| `golden/<tipo>.<nome>.v<versão>/invalid/` | Exemplos que têm de falhar (pelo menos 3), mais `index.json` |
| `generated/ts/` | Tipos TypeScript gerados, versionados; o CI reprova se estiverem desatualizados |
| `tools/` | Validador, gerador e os testes deles |

## Comandos

```bash
cd contracts
npm ci
npm run validate   # Schemas válidos, referências resolvidas, URNs únicas, dourados no lugar
npm run gen:ts     # regera generated/ts
npm run gen:check  # reprova se generated/ts não for o que o gerador produz agora
npm test           # testes do próprio validador e do gerador
```

Os testes montam árvores de contratos de mentira em `.test-tmp/` (fora do git), uma subpasta nova por execução, e não apagam nada: a limpeza dessa pasta é manual.

O `validate` confere, entre outras coisas: todo `$id` segue a URN e é único; o nome do arquivo bate com o `$id`; todo objeto com `properties` declara `additionalProperties`; cada `examples[0]` é o primeiro dourado válido; cada dourado válido passa; cada dourado inválido falha **pelo motivo declarado**; este README cita todo Schema.

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
6. `npm run gen:ts`, depois `npm run validate` e `npm test`. Inclua `generated/ts` no mesmo commit.

Dados dos dourados são sempre sintéticos: sem nomes reais, e-mails, caminhos de máquina nem valores que lembrem credencial.

## Convenções que valem em todos os Schemas

- Instantes em UTC com `Z`. Digest é `sha256:` mais 64 hexadecimais minúsculos. Bytes vão em base64 padrão.
- Entidades são identificadas por `urn:ads:common:id:1` (opaco, sem arroba); eventos e comandos, por ULID.
- Campo que pode faltar de verdade é opcional; campo que existe e pode não ter valor é obrigatório e aceita `null`.
- Regras entre campos (`if`/`then`) só cobrem o que depende de valores do próprio documento. Regra que precisa de outro documento (o hash de um plano contra o plano, por exemplo) é teste de contrato, não Schema.
- `data_class` (`public`, `internal`, `private`, `secret`) acompanha todo evento; texto de terminal e perguntas de permissão nunca são `public`.

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
| `urn:ads:common:actor:1` | Quem causou um evento ou uma decisão: um aparelho, um agente de IA ou o próprio daemon. |

### Envelopes

| Schema | O que descreve |
|---|---|
| `urn:ads:event:envelope:1` | Envelope único de todo evento: fato gravado só-anexo, numerado por stream, com autor, causa e classe do dado. Cada tipo de evento estende este envelope e fixa type, stream e data. |

### Comandos

| Schema | O que descreve |
|---|---|
| `urn:ads:command:request:1` | Comando JSON-RPC 2.0 enviado ao núcleo. Comando é pedido, não fato; a chave de idempotência evita executar duas vezes, e a origem remota exige assinatura do aparelho, nonce, contador e validade. |
| `urn:ads:command:response:1` | Resposta JSON-RPC 2.0 a um comando: ou result ou error, nunca os dois. |

### Eventos de sessão

| Schema | O que descreve |
|---|---|
| `urn:ads:event:session.created:1` | Uma sessão de terminal foi aberta no host de PTY para um agente de IA. |
| `urn:ads:event:session.output:1` | Bytes que o terminal produziu, em base64, com deslocamento no fluxo da sessão. No transporte ao vivo viajam em quadro binário; este é o registro. |
| `urn:ads:event:session.resized:1` | O terminal da sessão mudou de tamanho. |
| `urn:ads:event:session.ended:1` | A sessão terminou: o processo saiu, foi morto, o host de PTY reiniciou ou houve erro. Saída normal traz o código de saída. |
| `urn:ads:event:session.permission_asked:1` | O CLI de IA pediu permissão para algo (comando, escrita, rede). O resumo pode conter texto do usuário, então a classe é private ou secret. |

### Eventos de tarefa

| Schema | O que descreve |
|---|---|
| `urn:ads:event:task.created:1` | Uma tarefa nasceu. Só o hash do objetivo vai no evento; o texto fica no pacote da tarefa. |
| `urn:ads:event:task.started:1` | A execução da tarefa começou, numa sessão ou ainda sem sessão. |
| `urn:ads:event:task.completed:1` | A tarefa terminou com entrega; registra se os testes passaram e o digest do diff. |
| `urn:ads:event:task.failed:1` | A tarefa terminou sem entrega, com um código de erro estável e a indicação de se vale tentar de novo. |
| `urn:ads:event:task.cancelled:1` | A tarefa foi interrompida por decisão do dono, por teto de custo, por pressão de recursos, por política ou por ter sido substituída. |

### Eventos de aprovação

| Schema | O que descreve |
|---|---|
| `urn:ads:event:approval.requested:1` | Uma ação com risco pede aprovação: vale para o hash do plano e expira na validade indicada. |
| `urn:ads:event:approval.granted:1` | A aprovação foi concedida para o hash do plano, por um dos três caminhos permitidos. |
| `urn:ads:event:approval.denied:1` | A aprovação foi negada para o hash do plano, com motivo opcional. |
| `urn:ads:event:approval.expired:1` | O prazo da aprovação passou sem decisão; o plano precisa ser refeito. |

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
| `urn:ads:event:action.executed:1` | A ação foi executada exatamente como o plano do hash indicado, com a aprovação citada (ou sem aprovação, quando a política a dispensa). |
| `urn:ads:event:action.failed:1` | A ação não terminou como o plano previa; registra o hash do plano e um código de erro estável. O que foi feito pela metade é assunto do plano de reparo, não deste evento. |

### Registros

| Schema | O que descreve |
|---|---|
| `urn:ads:record:task-packet:1` | O que um agente recebe para executar uma tarefa: objetivo, critérios de aceite, restrições, orçamento e entradas. O receptor trata o pacote como dado, nunca como instrução; permissões vêm da configuração do usuário. |
| `urn:ads:record:usage_event:1` | Uma linha do livro-caixa: uma chamada ou turno de IA medido, com tokens, duração, custo de lista, custo faturável (só chave de API), janela de cota, fonte e chave de deduplicação. Só-anexo; alias de conta nunca é credencial nem e-mail. |
| `urn:ads:record:approval:1` | Estado de uma aprovação: vale para o hash de um plano e expira. Pendente e expirada não têm decisor; concedida e negada têm autor e instante da decisão. |
| `urn:ads:record:host_sample:1` | Uma observação de um sinal da máquina (disco, memória, etc.) feita por um coletor só leitura. Amostra ausente nunca vira zero; corrigir é gravar outra amostra com outro sample_id. |
| `urn:ads:record:host_event:1` | Um alerta derivado de amostras: cita a regra, a gravidade e os sample_id que o justificam. Confirmar é um evento posterior; o alerta original não muda. |
| `urn:ads:record:update-manifest:1` | O que o CI publica a cada junção: qual build é, de qual commit saiu e o SHA-256 de cada arquivo. O lançador confere a assinatura do manifesto (arquivo à parte, sobre os bytes exatos deste JSON) com a chave pública embutida e depois o digest de cada arquivo, antes de trocar qualquer coisa. |
| `urn:ads:record:update-status:1` | Estado visível da auto-atualização, escrito pelo lançador: o que está rodando, a versão anterior guardada, a candidata em andamento, a prova depois da troca, as quedas contadas e as versões bloqueadas. Baixando, conferindo e trocando têm candidata; em prova há prazo; falha traz código de erro. |
| `urn:ads:record:parameter:1` | Um parâmetro do produto como o núcleo o apresenta: a chave, o tipo, o padrão recomendado, o valor em vigor e a explicação. Todo comportamento com mais de uma escolha razoável é um registro destes. Parâmetro nunca guarda segredo: segredo fica no cofre. |

### Contratos entre processos

O contrato `harness-adapter/1` é o descritor abaixo. O contrato `pty-host/1` são os quatro Schemas `pty-host.*` mais o quadro binário descrito em [Contrato pty-host/1](#contrato-pty-host1).

| Schema | O que descreve |
|---|---|
| `urn:ads:contract:harness-adapter:1` | O que um adaptador declara ao núcleo no contrato harness-adapter/1: qual ferramenta adapta, se é linha de comando ou API, em que modos e papéis atende e quem guarda a credencial. Adaptador de linha de comando nunca toca na credencial que a ferramenta administra; adaptador de API usa a chave do cofre do produto e não tem modo interativo. |
| `urn:ads:contract:pty-host.request:1` | Pedido JSON-RPC 2.0 do núcleo ao host de PTY no contrato pty-host/1, com envelope próprio: id inteiro por conexão, sem chave de idempotência nem assinatura, porque o canal é local e do próprio usuário. Os bytes de saída do terminal não vêm em resposta: viajam no quadro binário. |
| `urn:ads:contract:pty-host.response:1` | Resposta JSON-RPC 2.0 do host de PTY: ou result ou error, nunca os dois. O result tem uma das formas do contrato (vazio, saudação, limites do replay, encerramento ou lista de sessões). Erro de replay informa o limite: nada é pulado em silêncio. |
| `urn:ads:contract:pty-host.notification:1` | Notificação JSON-RPC 2.0 (sem id, sem resposta) do host de PTY ao núcleo: o processo de uma sessão terminou, ou uma assinatura foi encerrada. last_seq é o último pedaço numerado (0 se não houve saída); o núcleo reassina a partir de last_seq + 1 sem perder nada. |
| `urn:ads:contract:pty-host.frame:1` | Cabeçalho, em JSON, do quadro binário que leva os bytes de saída do terminal no contrato pty-host/1: de qual sessão são, o número de sequência do pedaço e quantos bytes vêm logo depois do cabeçalho. Os bytes não fazem parte do JSON; o layout do quadro está no README. |

