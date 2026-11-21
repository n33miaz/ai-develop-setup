# ads-daemon

O executável serve a API local por WebSocket em `127.0.0.1`, com porta efêmera e
autenticação por execução ([ADR 0020](../../docs/adr/0020-local-api-authentication.md)).
Claude e Codex são abertos pelo host de terminais com programa, argumentos e
pasta definidos no TOML do usuário. Ferramenta sem programa recebe
`HARNESS_NOT_CONFIGURED`. Ferramenta desconhecida, programa ou pasta
inexistente e apelido de pasta fora da configuração recebem `INVALID_PARAMS`.

## Modo desktop

O lançador seleciona `--desktop` antes das opções de configuração e conecta
a saída padrão do daemon ao pipe privado da casca. Nesse modo o daemon envia
porta e token somente nesse canal. A API exige um upgrade sem cabeçalho
`Origin` e mantém a conferência de `Host` e do token. O processo Rust da casca
envia `auth.hello` antes de encaminhar mensagens da janela.

## Portão de ações de arquivo

Na composição de auditoria, passe `ToolLauncher::workdirs()` à abertura e à recuperação.
O daemon confere a localização e a privacidade da pasta antes de abrir o SQLite.
A configuração e as recusas estão no [README do armazenamento](../storage/README.md#âncora-no-cofre).

`ToolLauncher::session_gate` recebe os parâmetros que abriram a sessão no host, com o apelido já resolvido, e vincula o portão e o executor da cerca a todos os diretórios de `[workdirs]`. Recebe também o `ProductLog`, que só `audit::open_product` cria: o log de auditoria aberto junto com o estado do produto que ele usa. Esse estado é o banco como configurado, com `-wal`, `-shm` e `-journal`, a pasta do cofre e da trava `audit-binding.lock` e, quando o daemon segura a trava do lançador (`InstanceLock`, de `--instance-lock`), a pasta dela. Cada caminho entra na grafia configurada, na resolvida no disco e, para cada link ou junção no caminho, na do próprio link no lugar real: remover um deles quebraria o caminho configurado. Pode faltar uma entrada que ainda não foi criada (um arquivo auxiliar do SQLite, o banco antes da primeira abertura); link cujo destino não existe, destino de link escrito com `..`, caminho de rede ou mais de 32 links recusam a abertura. Esses caminhos são sempre negados como alvo, no plano e de novo na execução. O portão só aceita o `ProductLog` com o mesmo estado com que foi montado; outro devolve `JournalError::PolicyChanged`. `SessionGate::plan` usa a pasta da sessão, inspeciona os alvos dentro dos limites de `defaults/fs-executor.toml` e grava os fatos de repositório no log ancorado. Se a inspeção falhar, o plano é recusado antes da gravação. As operações `inspect`, `apply` e `execute` só aceitam ações da raiz da sessão. Quem chama fornece a estimativa e cumpre os pré-requisitos de aprovação antes de chamar `SessionGate::execute`. O portão grava `action.started` antes de chamar o executor. Ao terminar, o daemon tenta gravar o resultado e devolve separadamente o relatório da operação e o status dessa gravação. Um início confirmado continua consumido mesmo se o resultado não puder ser gravado. A composição está disponível em Rust. A API local ainda não expõe comandos de execução de arquivos. Os instantes e identificadores de `start` e `finish` vêm de quem chama, e só o `ts` decide a validade da aprovação: quando a API existir, o daemon deve carimbar `ts` e `id` ele mesmo. Os itens da issue #26 que seguem abertos estão no [portão do núcleo](../core/README.md#máquina-de-estados).

## Abrir Claude ou Codex no navegador

1. Na raiz do repositório, inicie o host de terminais em um terminal.

   ```powershell
   cargo run -p ads-ptyhost -j 2
   ```

2. Em outro terminal, use `where.exe claude.exe` e `where.exe codex.exe` para
   localizar os executáveis instalados. O programa precisa ser um `.exe`; não
   aponte para `claude.cmd` nem `codex.cmd`. Se o Codex foi instalado pelo npm,
   `where.exe` acha só `codex.cmd`; o executável real fica em
   `<prefixo do npm>\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`
   (o prefixo aparece em `npm prefix -g`). Escreva o caminho já expandido:
   `%VAR%`, `$VAR`, `~`, `..` e nomes curtos como `NOME~1` são recusados.

   Salve o TOML fora do repositório e dentro do seu perfil, por exemplo em
   `C:\Users\voce\ads-local\harnesses.toml` (troque `C:\Users\voce` pela sua
   pasta de perfil, escrita por extenso). Crie também, dentro do perfil, uma
   pasta de teste vazia, sem nada importante, como destino do apelido
   `default`. Se só tiver uma ferramenta instalada, omita a tabela da outra.

   ```toml
   [workdirs]
   default = 'C:\Users\voce\ads-local\pasta-de-teste'

   [tools.claude]
   program = 'C:\Users\voce\.local\bin\claude.exe'

   [tools.codex]
   program = 'C:\Users\voce\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe'
   ```

   **O TOML, a pasta de trabalho e o programa precisam ser graváveis só por
   você.** Não use a raiz de um disco de dados (como `D:\`) nem pastas
   compartilhadas: no Windows elas costumam aceitar escrita de qualquer usuário
   autenticado. Quem puder gravar ali pode trocar o programa ou os argumentos,
   ou deixar na pasta de trabalho arquivos de instrução e configurações com
   comandos automáticos que o agente lê e executa com a sua conta.

3. Na raiz, compile o daemon e inicie o executável com o caminho absoluto do
   TOML. Rodar o executável direto, em vez de `cargo run`, evita que as pastas
   de compilação do repositório entrem no `PATH` que o agente herda.

   ```powershell
   cargo build -p ads-daemon -j 2
   .\target\debug\ads-daemon.exe --config 'C:\Users\voce\ads-local\harnesses.toml'
   ```

4. Em um terceiro terminal, inicie a interface.

   ```powershell
   Set-Location ui
   npm ci
   npm run dev
   ```

   Conectada, a interface pergunta ao daemon (`harness.list`) quais ferramentas
   estão ligadas e mostra um botão para cada uma; `VITE_CORE_HARNESSES` não é
   necessária.

   Depois que o Vite confirmar a porta, abra no navegador a URL completa
   impressa pelo daemon
   (`http://127.0.0.1:5173/#core=ws://127.0.0.1:<porta>&token=<token>`) e
   clique em **Abrir claude** ou **Abrir codex**.

O agente dentro do terminal edita arquivos e executa comandos conforme as
permissões dele. Argumentos que desligam as confirmações ou a área isolada da
ferramenta, somados ao controle do terminal pela API, dão a quem usa a interface
acesso total à sua conta; não os coloque em `args` sem entender esse efeito.

A saída padrão contém somente essa linha. O token de 32 bytes aleatórios vem do
gerador do sistema e existe só em memória; o fragmento não faz parte do pedido
HTTP. A interface envia `auth.hello` como primeira mensagem. Antes dela, mensagem
inválida, token errado ou prazo vencido encerram a conexão. `Host` e `Origin`
inválidos recebem o mesmo HTTP 403 antes do upgrade; cabeçalhos repetidos,
origem ausente e query string também são recusados.

O arquivo de `--config` é opcional. As tabelas `[tools.claude]`,
`[tools.codex]`, `[workdirs]`, `[accounts]` e `[env]` sobrepõem os
[padrões das ferramentas](../../defaults/harnesses.toml). `[env]` e
`[workdirs]` valem para as duas. As tabelas de perfil em `[accounts]`
são aceitas para as duas ferramentas citadas. Entradas para outras ferramentas
são recusadas. Sem arquivo, ambas ficam desligadas. As chaves
da API na raiz do mesmo TOML sobrepõem os padrões do daemon listados abaixo.
Reinicie o daemon para reler a configuração. A configuração inválida impede a
partida e a mensagem omite o conteúdo do arquivo.
O host usa o canal padrão do usuário, cuja identidade é conferida antes da conexão.
`--host-config <arquivo>` permite selecionar outra configuração do host (inclusive
um canal exclusivo para testes), com as mesmas validações de `HostConfig`.

`--config` e `--host-config` aceitam só caminho absoluto literal de um arquivo
comum de até 1 MiB (`startup::MAX_CONFIG_BYTES`, fixo porque protege a leitura do
próprio arquivo de configuração). São recusados caminho relativo, de rede
(`\servidor\...`), de dispositivo ou pipe (`\.\`, `\?\`), com variável, `~`,
`..` ou nome curto, além de pastas.

O ambiente do processo pai é lido uma vez, na partida, e filtrado na hora pela
política `[env]`; o daemon guarda só o resultado, sem as variáveis negadas.

## Comandos, limites e recuperação

Os nomes, valores e significados dos erros emitidos pela API local estão no
[contrato de códigos de erro](../../contracts/schemas/contract.local-api.error-code.v4.schema.json).

| Nome | Código na resposta |
|---|---|
| `METHOD_NOT_FOUND` | `-32601` |
| `INVALID_PARAMS` | `-32602` |
| `INTERNAL_ERROR` | `-32603` |
| `CAPACITY_EXCEEDED` | `-32003` |
| `SUBSCRIPTION_REPLAY_UNAVAILABLE` | `-32004` |
| `REMOTE_ORIGIN_REFUSED` | `-32010` |
| `HARNESS_NOT_CONFIGURED` | `-32011` |
| `CONFLICT` | `-32012` |
| `SESSION_CLOSE_PENDING` | `-32013` |
| `SESSION_SETUP_FAILED` | `-32014` |
| `QUEUE_NOT_CONFIGURED` | `-32015` |
| `QUEUE_ALREADY_CREATED` | `-32024` |
| `QUEUE_ALREADY_ENABLED` | `-32016` |
| `APPROVAL_NOT_CONFIGURED` | `-32017` |
| `APPROVAL_NOT_FOUND` | `-32018` |
| `APPROVAL_ALREADY_ANSWERED` | `-32019` |
| `APPROVAL_STALE` | `-32020` |
| `APPROVAL_POLICY_CHANGED` | `-32021` |
| `APPROVAL_OUT_OF_ORDER` | `-32022` |
| `APPROVAL_NOT_APPROVABLE` | `-32023` |

`api::Launcher::prepare` recebe os parâmetros gerados de `session.open` (ferramenta,
apelido de pasta, identificador e tamanho) e devolve `OpenParams` do ptyhost.
O daemon confere que identificador e tamanho foram preservados. Cada adaptador
aplica de novo a política de ambiente ao ambiente já filtrado antes de enviar
os parâmetros ao host.
Os tipos gerados validam todos os comandos e eventos contra os contratos.
`session.write`, `session.resize` e `session.close` usam o cliente do host.
`events.subscribe` envia eventos com `seq > after_seq`, seguidos dos novos,
incluindo uma assinatura feita antes de abrir a sessão. Nenhum `fs.*` está registrado.
Todo comando remoto é recusado, mesmo com autenticação válida.

`harness.list` (só depois de `auth.hello`; `params` leva apenas o `_meta`
obrigatório de todo comando) responde
`{"harnesses": [...], "workdir_aliases": [...]}`
([`command.harness.list.response:1`](../../contracts/README.md#api-local)): os
identificadores das ferramentas com `program` preenchido, na ordem `claude`,
`codex`, e os apelidos de `[workdirs]` em ordem de nome. Só nomes: programa,
argumentos, ambiente e caminhos nunca saem do daemon, e o contrato fechado da
resposta não tem onde levá-los. A lista é montada uma vez, na partida (reinicie
para reler a configuração), e não confere o disco: uma ferramenta listada ainda
pode receber `INVALID_PARAMS` em `session.open` se o programa ou a pasta não existir.
Como só lê, não passa pela cache de idempotência: repetir a chave devolve a
lista de novo, nunca `replayed`. `api::Launcher::catalog` fornece a lista; o
padrão é vazio, como em `DisabledLauncher`. Essa resposta exige a
`local-api.server-message:2`; quem não envia `harness.list` nem `queue.get`
continua recebendo só mensagens da `:1`. O daemon escreve a `:5`, que acrescenta a resposta de `approval.list` na `:2`. Um daemon anterior ao comando o recusa
com `METHOD_NOT_FOUND`, e a interface mostra que o daemon precisa ser atualizado.

## Fila de tarefas

A fila nasce parada e só o dono a liga. `queue.create`, `queue.get`, `queue.start` e
`queue.stop` nomeiam a fila em `params.queue_id` e não aceitam mais nenhum
campo: o dono e o aparelho vêm do daemon, nunca do pedido. Valem as mesmas
conferências dos outros comandos: só depois de `auth.hello`, e origem remota
recebe `REMOTE_ORIGIN_REFUSED`.

- `queue.create` grava `queue.created:2` com a política de `defaults/queue.toml`,
  o aparelho do dono como ator e o id do comando em `cause`. A fila fica
  disponível parada após a gravação. A cache de idempotência permite repetir o
  pedido confirmado. Um novo pedido para uma fila existente recebe
  `QUEUE_ALREADY_CREATED`.
- `queue.get` responde `{"queue_id": ..., "state": "stopped" | "enabled"}`
  ([`command.queue.get.response:1`](../../contracts/README.md#api-local)). Só lê
  e, como `harness.list`, fica fora da cache de idempotência.
- `queue.start` liga a fila só depois de gravar `queue.enabled` no log de
  auditoria ancorado, com o aparelho local como ator, o dono, `via: local_ui`,
  o `ts` do relógio do daemon e o id do comando em `cause`. A fila já ligada
  recebe `QUEUE_ALREADY_ENABLED` e nada é gravado. Se a gravação falhar, a fila
  continua parada e a resposta é `INTERNAL_ERROR`, sem derrubar o serviço.
- `queue.stop` expressa o estado desejado e é sempre aceito. Ele não consulta
  a cache de idempotência: repetir uma parada antiga depois de religar a fila a
  para de novo, e uma chave já usada por outro comando não impede a parada.
  `replayed` é `false` quando esta parada desligou a fila e `true` quando ela
  já estava parada (nada é gravado). A fila ligada para antes de o evento ser
  gravado (`queue.stopped` com `reason: owner`), então nenhuma tarefa nova é
  admitida mesmo se a gravação falhar. Nesse caso a resposta é
  `INTERNAL_ERROR`, embora a fila tenha parado, e a parada que faltou é gravada
  antes da próxima mudança feita pelo mesmo processo, com o id do comando de
  parada em `cause` e o `ts` do momento da gravação. Se o processo terminar
  antes, a causa se perde: a próxima abertura grava a parada como
  `reason: restart`, com o daemon como ator.
- Fila que o daemon não serve, ou daemon sem fila ligada, recebe
  `QUEUE_NOT_CONFIGURED`.

`queue::LoggedQueue::bind` liga uma fila já criada ao stream `queue/<id>` do log
ancorado: recusa um stream criado por outro dono e, a cada abertura, restaura a
fila parada e grava `queue.stopped` com `reason: restart` se o log dizia ligada.
Quando a composição usa um `ProductLog`, passe `ProductLog::shared_log()` à fila
para usar a mesma conexão SQLite do portão. A primeira aquisição da trava da
conexão, em `bind` e `open`, é a que lê o `busy_timeout_ms` do armazenamento e
por isso espera sem prazo enquanto o portão usa a conexão; cada aquisição
seguinte usa esse prazo, assim como a espera por uma trava do SQLite. Se o portão ainda estiver usando a conexão ao fim do
prazo, a API responde `INTERNAL_ERROR` com a mensagem `audit log busy`. A fila
já estará parada se o comando era `queue.stop`. Os controles da fila rodam em
tarefas bloqueantes, mantendo o atendimento de assinaturas e do terminal
enquanto aguardam a gravação.
Com o stream vazio, `bind` recusa com `NotCreated` e não grava nada. Criar a fila
(`queue.created:2`, com o aparelho do dono como ator e a política inicial) é ato
do dono: `open` (política embutida em `defaults/queue.toml`) e `open_with_policy`
(política fornecida pela composição) exigem a prova de sessão `auth::LocalSession`,
a mesma de ligar e desligar; sem ela a chamada não compila. Num stream que já
existe, as duas apenas ligam a fila e a política da criação continua valendo. Um
processo mantém no máximo um handle por fila (a segunda abertura recebe
`AlreadyOpen`); a composição compartilha esse handle com todos os consumidores.
O carregamento de um arquivo do usuário está pendente. Dono e aparelho
(`QueueOwner`) são dados da configuração do núcleo; criar e agir na fila exigem
essa prova. Só este crate a emite (`Token::open_session` é restrita ao crate), e
em produção o único emissor é o laço de conexão, ao conferir o token do
`auth.hello`. A prova sela a API da fila; quem tem o próprio `EventLog` grava
abaixo dessa camada, e o replay confere a coerência dos atores, não a
autenticidade deles (ver o README do núcleo). A transição usa
`ads_core::queue::change_local_queue_state`, sem fabricar `AuthenticatedDevice`.
Na partida, a composição passa o log aberto e os identificadores estáveis da
fila, do dono e do aparelho a `queue::LocalQueue::bind_or_pending` e entrega o
controle a `LocalApi::with_queue`. Apenas `NotCreated` deixa a fila aguardando
`queue.create`. Outras falhas de abertura são devolvidas à composição.

O executável serve `queue::DisabledQueue` e responde `QUEUE_NOT_CONFIGURED`.
Para ligar essa composição no Windows, a inicialização precisa obter ou
provisionar com segurança a chave de cifra por coluna no cofre, resolver
as chaves do banco pelos identificadores gravados e fornecer os identificadores
estáveis da fila, do dono e do aparelho. `audit::open_product` já usa
`OsVault` para a âncora e recebe o `KeyProvider` de quem o chama.

## Aprovações do portão

O dono vê e responde as ações que o portão segurou para aprovação. Valem as
conferências dos outros comandos: só depois de `auth.hello`, e origem remota
recebe `REMOTE_ORIGIN_REFUSED`. O dono e o aparelho vêm do daemon
(`approvals::ApprovalOwner`), nunca do pedido, e cada chamada a
`approvals::ApprovalControl` exige a prova `auth::LocalSession`.

- `approval.list` (só `_meta`) responde `{"approvals": [...], "more": ...}`
  ([`command.approval.list.response:2`](../../contracts/README.md#api-local)):
  da mais antiga à mais nova, cada ação cuja aprovação foi pedida e ainda não
  foi concedida, negada nem expirada, com `approval_id`, `risk`, `summary`,
  `requested_at`, `expires_at`, `not_approvable`, o registro do plano
  (`record.action-plan:2`) como o portão o julgou e os efeitos concretos
  (`record.action-effects:1`) com o seu `effects_hash`, ou os dois `null`, e o
  código da recusa do ensaio em `effects_error`. Só ações de uma pasta de
  sessão servida aqui: o filtro vem antes do limite, então pedidos de outras
  pastas nunca ocupam a página. No máximo `read_page_limit` da configuração do
  armazenamento por pedido, e nunca mais de 1000 (o limite do contrato da
  resposta). As ações entram da mais antiga à mais nova enquanto couberem em
  `approval_list_max_bytes`. A mais antiga sempre entra, mesmo acima desse teto.
  `more: true` indica que uma ação da lista ficou para depois pelo limite de
  itens ou de bytes. Um registro de efeitos maior que metade desse teto não é
  mostrado e a ação fica com `effects_not_listed`. A listagem e a resposta
  conferem o tamanho cifrado descontando o invólucro e a tag antes de ler os efeitos.
  A leitura é feita sob a trava da mesa, uma ação por vez e na ordem do pedido:
  cada item é montado e medido assim que lido. Se o tamanho cifrado já mostra
  que os efeitos listáveis excedem o espaço restante, a leitura do registro
  fica para a próxima página. O teto limita a montagem da página e a leitura
  dos efeitos. O tempo sob a trava também inclui a autenticação da cadeia
  inteira e a leitura dos históricos de outras pastas ou ilegíveis, inclusive
  ao procurar mais um pedido listável após atingir o limite de itens. O daemon
  omite pedidos cujo histórico é ilegível, registra o `action_id` no log e
  segue a listagem. O portão precisa de um histórico legível para negar ou
  expirar um pedido. Pedido ou decisão gravados fora do formato do
  portão são ignorados. Só lê e fica fora da cache de idempotência.
- O que o registro do plano garante: a operação, os alvos como foram pedidos
  (caminho e curinga), o destino, se há quarentena, a estimativa e as regras.
  Ele não lista o conteúdo de uma pasta alvo nem os itens do manifesto de uma
  restauração. Isso fica nos efeitos concretos: quando o veredito pede
  aprovação, `SessionGate::plan` ensaia o plano que o portão entrega ao executor
  (`Executor::rehearse_effects`) e grava o resultado em `effects.recorded:1`, uma
  vez só, no stream `effects/<action_id>` (o log recusa um segundo `seq` 1).
  Ensaio recusado (por exemplo maior que `max_effects_bytes`, com
  `E_FS_EFFECTS_TOO_LARGE`) grava `effects.refused:1` com o código no mesmo
  lugar, e registro que o log ou o contrato recusam grava
  `E_EFFECTS_UNRECORDABLE`; se nem isso for gravado, fica um aviso no log do
  processo. O que o executor compara é nome, tipo, identidade no volume e, de
  arquivo, tamanho: conteúdo reescrito no mesmo objeto, com o mesmo tamanho, e
  datas não entram. Objeto apagado e recriado sob o mesmo nome só é visto onde a
  identidade do volume é estável; nos outros volumes (FAT, exFAT, rede, não
  classificado) o ensaio de apagar de verdade e de restaurar é recusado com
  `E_FS_IDENTITY_UNSTABLE` ([identidade por sistema de
  arquivos](../adapters/fs/README.md#identidade-por-sistema-de-arquivos)).
- A API só aprova ação cujo efeito concreto está fixado. `not_approvable`
  (`approvals::not_approvable`) lista por que não: `estimate_unknown` (sem
  estimativa e sem efeitos gravados), `wildcard_target` (alvo com curinga, que o
  executor não executa), `definitive_recursive_delete` (`fs.clean_untracked`
  sem quarentena, ou `fs.delete_recursive` sem quarentena e sem efeitos
  gravados) e `restore_from_manifest` (mover de volta à raiz de trabalho sem
  efeitos gravados). Quando o plano precisa de efeitos e não há o que mostrar,
  vem também o porquê: `effects_refused` (o ensaio foi recusado),
  `effects_unreadable` (o registro gravado não passa na leitura, com aviso no
  log do processo; a listagem das outras ações segue) ou `effects_not_listed`.
  Aprovar uma dessas grava a resposta e volta `APPROVAL_NOT_APPROVABLE`, com a
  ação ainda aguardando; negar continua permitido e nunca lê os efeitos.
- Na execução, `SessionGate::execute` confere os efeitos **antes de pedir a
  licença ao portão**: com a ação aprovada, lê o `effects_hash` da resposta que
  causou a concessão, exige que seja o do registro gravado, refaz o ensaio e
  exige o mesmo registro. O executor confere de novo antes da primeira
  alteração (`Executor::execute_approved`). Diferença, registro ausente ou
  aprovação sem registro para um plano que precisa dele: o disco não é tocado, a
  ação termina em `action.failed` com `E_FS_EFFECTS_CHANGED` e só um plano novo,
  com aprovação nova, volta a executar. Política ou tolerância do relógio
  mudada desde a aprovação é recusada antes dessa conferência, pelo portão,
  como `E_POLICY_CHANGED` ou `E_CLOCK_POLICY_CHANGED`.
- Isso deixa a execução aprovada mais estrita do que antes: toda ação aprovada
  pela API com efeitos gravados, inclusive um apagar com estimativa que vai para
  a quarentena, é recusada a qualquer diferença no disco desde o plano (numa
  pasta em que uma ferramenta continua escrevendo, por exemplo), e uma
  concessão vinda de fora da API (bandeja) para um plano que precisa de efeitos
  falha fechada.
- `approval.answer` leva `action_id`, `approval_id`, `plan_hash` (o que o dono
  viu) e `decision` (`approve` ou `deny`). Antes de gravar qualquer coisa, o
  daemon confere que a ação existe e que a aprovação é a pedida no histórico
  dela, em qualquer estado (`APPROVAL_NOT_FOUND`), e que a ação ainda aguarda
  resposta (`APPROVAL_ALREADY_ANSWERED`); nessas recusas nada é gravado. Depois
  grava `answer.recorded:2` no stream `answer/<approval_id>`: o aparelho como
  ator, o dono, o `ts` do relógio do daemon, a decisão, o hash do plano, o
  `effects_hash` dos efeitos gravados da ação (ou `null`), a chave de
  idempotência e o id do comando em `cause`. Se essa gravação falhar, a
  resposta é `INTERNAL_ERROR` e o portão não age. Só então o portão registra
  `approval.granted:2` ou `approval.denied:2`, com o mesmo ator e `cause`
  apontando para o registro da resposta.
- Resposta velha fica registrada e é recusada com `APPROVAL_STALE`: com outro
  hash que não o do plano em espera, o portão não age; com a validade vencida,
  aprovando ou negando, o portão registra `approval.expired:2` e a ação sai da
  lista.
- Aprovar reconfere a política atual e a ordem no tempo. Política ou tolerância
  do relógio mudada desde o plano vira negação registrada pelo portão
  (`E_POLICY_CHANGED` ou `E_CLOCK_POLICY_CHANGED`) e a resposta
  `APPROVAL_POLICY_CHANGED`. Relógio antes do último passo da ação devolve
  `APPROVAL_OUT_OF_ORDER` sem decisão; a resposta do dono já ficou gravada e
  pode ser repetida com a mesma chave. Aprovar não executa nada: a licença de
  execução só sai em `SessionGate::execute`, que confere política e tempo de
  novo.
- Repetições: a cache em memória (`idempotency_keys` comandos aceitos) devolve
  `replayed: true` sem chamar o portão. Fora dela, a chave fica no log, junto da
  resposta, enquanto o log existir: a mesma chave com o mesmo conteúdo nunca
  grava outra resposta. Se o portão já decidiu por causa dela, volta o mesmo
  resultado (aceito com `replayed: true`, ou a mesma recusa); se só a resposta
  foi gravada (falha entre os dois registros, relógio atrasado, reinício), a
  decisão é tomada de novo com `cause` na resposta gravada; recusas que não
  dependem do portão voltam iguais. A mesma chave com outra decisão ou outro
  hash recebe `CONFLICT`. A busca é no stream da aprovação nomeada.

`approvals::ApprovalDesk` guarda o log ancorado (`audit::ProductLog`) e as
portas de sessão (`gate::SessionGate`) ligadas a ele; planejar, pedir aprovação
e executar passam por `ApprovalDesk::with_gates`, sob a mesma trava das
respostas. `approval.answer` roda em uma tarefa bloqueante e cada espera
pela trava da mesa ou da conexão usa `busy_timeout_ms`. Ao fim do prazo,
a API responde `INTERNAL_ERROR` com a mensagem `audit log busy`. Se um pânico
invalidar a trava da conexão, a fila e o portão recusam novos acessos ao log.
A lista vem de `EventLog::pending_approvals`, que confere a âncora e
a cadeia uma vez antes de ler e aplica o filtro da mesa antes do limite. `LocalApi::with_approvals` entrega o controle à API. O
executável ainda não monta o portão, porque falta o provedor de chaves do
cofre; até lá ele serve `approvals::DisabledApprovals`, que recusa os dois
comandos com `APPROVAL_NOT_CONFIGURED`.

Padrões em [defaults/daemon.toml](../../defaults/daemon.toml), aplicados ao iniciar:

| Chave | Padrão | Efeito e alternativa |
|---|---|---|
| `ui_base_url` | `http://127.0.0.1:5173` | URL HTTP da interface em localhost, 127.0.0.1 ou [::1], sem caminho, query ou fragmento; substitua pela interface cuja porta foi verificada. |
| `allowed_origins` | `[]` (deriva `http://127.0.0.1:5173`) | Vazia deriva somente a origem de `ui_base_url`; lista explícita substitui a derivação, sem curingas ou caminhos. |
| `auth_timeout_ms` | 5000 | Prazo total único do upgrade e da primeira mensagem; reduza para limitar conexões ociosas. |
| `max_sessions` | 32 | Sessões ainda mantidas no host e, em separado, históricos de sessões fechadas. Aumentar permite mais sessões e usa mais memória. |
| `max_write_bytes` | 65536 | Bytes decodificados por escrita; pode diminuir, nunca exceder o contrato. |
| `events_per_stream` | 256 | Eventos retidos por stream; aumente para ampliar o replay. |
| `idempotency_keys` | 1024 | Chaves aceitas retidas em FIFO; aumente para ampliar a janela de deduplicação. |
| `command_queue` | 64 | Pedidos esperando o host; aumente para mais concorrência. |
| `max_connections` | 32 | Conexões autenticadas simultâneas; excedentes são fechadas ao autenticar. |
| `max_pending_auth` | 32 | Tarefas de conexões ainda sem autenticar, inclusive as que estão sendo expulsas; separadas das autenticadas. Com a fila cheia, a mais antiga que ainda não fez upgrade é expulsa e a nova entra quando a vaga volta; se todas já fizeram upgrade, a nova é recusada. |
| `accept_retry_ms` | 50 | Espera após erro de aceitação TCP; reduz tentativas sob falta de recursos. |
| `max_message_bytes` | 131072 | Limite de mensagem e quadro WebSocket recebido; reduzir pode impedir escritas grandes. |
| `send_timeout_ms` | 5000 | Prazo por envio; cliente lento é desconectado. |
| `approval_list_max_bytes` | 4194304 | Teto de uma resposta de `approval.list`, de 1 MiB a 8 MiB: metade do quadro de 16 MiB que a casca desktop aceita, para caber também a ação mais antiga, listada mesmo quando passa do teto. Ações além dele ficam para a próxima listagem (`more`); registro de efeitos maior que a metade não é mostrado nem aprovável pela API. |
| `host_timeout_ms` | 10000 | Prazo único por comando no host e na conexão inicial. Cada chamada ao host começa dentro do prazo, e uma resposta já recebida vale mesmo se o prazo vencer enquanto o daemon trata um evento do host. Falha encerra o serviço para evitar repetir efeito incerto. Duas exceções não encerram: no fechamento automático o prazo esgotado conta como tentativa falha, e na preparação depois de um `session.open` confirmado ele responde `SESSION_SETUP_FAILED`, como qualquer outra falha dessa etapa (ver abaixo). |
| `retired_sessions` | 1024 | Ids de sessões podadas que não podem ser reabertos nesta execução (acima do limite, o mais antigo volta a ser aceito) e, em separado, streams assinados antes do `session.open` (acima do limite, a espera mais antiga termina com `SUBSCRIPTION_REPLAY_UNAVAILABLE` no id da assinatura, mesmo se a conexão estiver ociosa). |
| `subscription_ids_per_stream` | 2 | Ids de assinatura por conexão: este valor vezes `max_sessions`. Reassinar a mesma encarnação na mesma conexão soma um id, que só sai quando o stream termina; ids de encarnação já terminada não contam. A interface mantém uma assinatura por stream e conexão, então raramente passa de um id por stream; aumente para clientes que reassinam muito. |
| `close_retries` | 3 | Tentativas do fechamento automático de sessão invalidada, de `open` que falhou ou de sessão que terminou sozinha; esgotadas numa sessão invalidada, a assinatura do stream recebe `SESSION_CLOSE_PENDING`. |
| `close_retry_ms` | 1000 | Espera entre tentativas do fechamento automático. |

Os inteiros de capacidade aceitam de 1 a 1048576 (`max_message_bytes`: mínimo
1024); prazos aceitam de 1 a 300000 ms. Valores altos elevam o teto de memória.
Quando há uma lacuna irrecuperável, o daemon invalida o stream e solicita o
fechamento ao host. Recusa do host (fora `UnknownSession`) ou prazo esgotado
repetem o pedido até `close_retries` vezes, com aviso no erro padrão sem
conteúdo. Quem já assinava o stream não recebe nada enquanto as tentativas
correm; o resultado final chega com o id da assinatura: `SESSION_CLOSE_PENDING` se as tentativas se esgotarem, `SUBSCRIPTION_REPLAY_UNAVAILABLE` se o host confirmar. Só essa
assinatura termina. Um `events.subscribe` novo desse stream recebe `SUBSCRIPTION_REPLAY_UNAVAILABLE`
durante as tentativas e `SESSION_CLOSE_PENDING` depois delas, até um `session.close` confirmado.
`session.close` continua aceito enquanto o registro da sessão invalidada estiver
retido. Cada tentativa vale só para a instância da sessão que a gerou e é
descartada quando a vaga é devolvida. Se um `open` falhar depois de o host abrir o
terminal, seja pelo prazo, seja por recusa do host ou do registro, a resposta é
sempre `SESSION_SETUP_FAILED`, inclusive com a fila de pedidos do cliente do host
cheia; só uma falha que encerra o serviço, como a perda do host ou a parada do cliente dele por
qualquer causa, responde `INTERNAL_ERROR`. Recusas anteriores ao terminal aberto mantêm o próprio código.
Com `SESSION_SETUP_FAILED`, o fechamento vai para a mesma fila, com prazo próprio por tentativa;
o registro e a vaga ficam até o host confirmar, e então o id volta a ser aceito.
A conexão que abriu não recebe nenhum evento dessa sessão: os eventos só seguem
depois da resposta, e o stream invalidado já não os tem. O resultado final chega
a uma assinatura feita antes do `open`: `SUBSCRIPTION_REPLAY_UNAVAILABLE` quando o host confirma o
fechamento, `SESSION_CLOSE_PENDING` quando as tentativas se esgotam.
`UnknownSession` no fechamento libera a vaga; num stream válido (não invalidado) que ainda não
tinha evento terminal, grava `ended` com `reason: "error"`. Um `ended` já gravado fica como está, e
um stream invalidado não recebe `ended`. Quando o processo de uma
sessão termina sozinho, o daemon grava `ended` e pede o fechamento ao host pela mesma fila, sem
esperar a interface, inclusive se um `session.close` do cliente estiver em andamento e o host o
recusar. Cada encarnação tem no máximo uma tentativa pendente, contando a que está em execução; reagendar mantém o saldo e o prazo dela. O fechamento encerra a árvore de
processos que restar e devolve a vaga: um servidor de desenvolvimento ou outro processo que a
ferramenta tenha iniciado e que ainda esteja vivo é encerrado nessa hora, mesmo sem pedido da
interface. O stream liberado mantém os eventos ainda retidos, dentro de `events_per_stream` e até a
poda descrita abaixo. Um `session.close` dessa sessão depois da liberação é aceito sem novo efeito
enquanto o registro dela estiver retido, ou seja, até ela ser podada; depois disso recebe `INVALID_PARAMS`.
O id podado não pode ser reaberto enquanto a lápide existir, e nenhuma tentativa automática da
encarnação anterior alcança uma reaberta. Quando a lápide expira e o id volta a ser aceito, um
`session.close` com ele se refere à sessão atual: o protocolo identifica a sessão só pelo id, e a
interface gera um id novo a cada abertura. Se as tentativas se esgotarem, a vaga fica ocupada e o
stream não muda; um `session.close` do cliente continua aceito e a libera. Ao atingir o limite de registros fechados,
o daemon descarta os mais antigos conforme novas sessões são fechadas. O stream
descartado vira lápide e o mesmo `session_id` é recusado com `CONFLICT` enquanto
ela existir (`retired_sessions`). Cada assinatura fica presa à encarnação do
stream que assinou; quem assina antes do `session.open` fica presa à que o open
preencher. Se ela for podada, removida ou tiver o fechamento resolvido, mesmo
depois de a lápide expirar e o id voltar a ser usado, a assinatura recebe o
primeiro resultado final com o seu id (`SESSION_CLOSE_PENDING` se as tentativas se esgotaram,
`SUBSCRIPTION_REPLAY_UNAVAILABLE` nos demais casos) e termina sem derrubar a conexão, sem nunca entregar
eventos de outra sessão. Uma invalidação no meio de um lote suspende os quadros
seguintes. Assinaturas novas são validadas pelo estado atual.

Uma execução conserva no máximo `2 × max_sessions × events_per_stream` eventos
(um stream que sai do log, mesmo ainda assinado, descarta os seus);
o tamanho de cada saída também é limitado pelo quadro do cliente do host.
Assinaturas por conexão são limitadas a `max_sessions` streams. Assinar de novo, na mesma
conexão, a mesma encarnação de um stream junta o id à assinatura existente: a entrega segue um
cursor só, o menor pedido, e o resultado final chega a cada id. Se o novo `after_seq` for menor que
o que já saiu, os eventos desde ele saem de novo; o consumidor descarta por `seq` o que já tinha. Os
ids por conexão são limitados a `subscription_ids_per_stream × max_sessions`, sem contar os de
encarnação já terminada; acima disso a assinatura é recusada com `CAPACITY_EXCEEDED`. Se a encarnação
assinada antes já terminou, o fim dela chega aos ids antigos antes da confirmação da nova. Filas, conexões e chaves
também têm limites separados. Seq e offset recomeçam ao reiniciar.

Uma chave ainda retida com o mesmo método e parâmetros devolve `replayed: true`,
sem repetir o efeito; outro conteúdo com a mesma chave recebe `CONFLICT`. O id do
pedido pode mudar. A cache guarda só a chave e SHA-256 dos parâmetros, nunca a
entrada do terminal. Depois da expulsão FIFO não há garantia de deduplicação.
`after_seq = 0` pede tudo que resta no stream; cursor descartado ou futuro recebe
`SUBSCRIPTION_REPLAY_UNAVAILABLE`. Se o cliente ficar para trás durante uma assinatura, recebe `SUBSCRIPTION_REPLAY_UNAVAILABLE`
com o id do pedido de assinatura antes de a conexão fechar;
nenhum buraco é ocultado. Reconecte com o último seq recebido ou peça 0
explicitamente para aceitar o histórico ainda disponível.

Eventos `created`, `output`, `resized` e `ended` ficam só na memória, em ordem por
stream; saída usa base64 com offset em bytes. Encerramento solicitado só gera
`ended` depois da saída pendente do host. Recusas explícitas do host voltam ao
cliente com o código do contrato `pty-host/1`,
sem encerrar o serviço. Falha do canal ou prazo do host encerra
o daemon; as sessões continuam sob responsabilidade do ptyhost.
Eventos são consumidos também durante chamadas ao host; a fila limitada aplica
contrapressão ao canal, e o anel limitado recusa assinantes lentos. Falha depois
de abrir uma sessão agenda seu fechamento no host antes de liberar o id.
Lacuna na sequência ou recusa da reassinatura do host invalida somente o stream
afetado, que passa a recusar replay até reiniciar; os demais continuam disponíveis.
Reiniciar não adota sessões antigas nesta implementação. Para desfazer, pare o daemon e volte
ao consumidor anterior do cliente, sem migração de dados.

## Dependências e diagnóstico

`tokio-tungstenite =0.30.0` (MIT, publicada em 11/07/2026) fornece WebSocket sobre
o Tokio já usado pelo host, sem servidor HTTP adicional nem TLS em loopback.
A data supera o mínimo de 14 dias; versão e licença foram conferidas no
[registro da versão](https://docs.rs/crate/tokio-tungstenite/0.30.0) e no
[manifesto publicado](https://docs.rs/crate/tokio-tungstenite/0.30.0/source/Cargo.toml).
`subtle 2.6.1` (BSD-3-Clause) faz a comparação em tempo constante;
`base64 0.23.1` (MIT OR Apache-2.0) codifica os bytes; `futures-util 0.3.34`
(MIT OR Apache-2.0) fornece stream/sink; `time 0.3.55` (MIT OR Apache-2.0)
formata timestamps. Versões resolvidas estão no `Cargo.lock`.

O token e o log de eventos omitem seu conteúdo em `Debug`; erros públicos contêm só códigos
e mensagens estáticas. `pty_client::Event` também omite bytes em `Debug`.
`log 0.4.35` (MIT OR Apache-2.0), já transitiva, usa `max_level_off`:
macros da fachada `log` são eliminadas do executável, inclusive nas dependências,
pois o transporte tem diagnóstico capaz de mostrar payloads. Isso vale também
em debug e não é configurável, por segurança. Não há logger nem arquivo de saída.
`zeroize`, já presente no workspace, limpa o token ao descartá-lo e a URL logo
após a impressão, inclusive quando o envio falha. O `auth.hello` é consumido
antes do laço autenticado; sua cópia do token é limpa. O daemon entrega o token na linha inicial de conexão, ao navegador em desenvolvimento ou pelo pipe da casca no modo desktop.

Os testes exercitam recusas de upgrade e autenticação, comandos negados,
deduplicação entre conexões, limites, replay e eventos com o backend falso.
Verificam os tipos gerados para cada mensagem, sequência, offsets, tamanhos,
fim natural e encerramento pedido, além da ausência de credenciais e saída em
diagnósticos. O CI Windows inclui mudanças em `defaults/daemon.toml`.

## Cliente do host

A biblioteca expõe `pty_client::PtyClient`,
cliente assíncrono do contrato `pty-host/1` através do codec público do ptyhost.
`OpenParams`, `WriteParams`, `ResizeParams` e `SessionParams` são os tipos gerados
reexportados pelo host; o cliente não constrói JSON nem quadros próprios.

`connect` recebe `HostConfig` para resolver o canal com `channel::endpoint`, abre-o
com `channel::connect` e completa `host.hello`. No Windows o canal só entrega a
conexão depois de confirmar que o pipe é servido pelo usuário atual; senão o erro é
`NotOwnedByUser` e nenhum byte foi enviado (`tests/channel_owner.rs`, também contra um servidor real cujo processo ou token o cliente não consegue abrir). O host precisa
estar disponível; iniciar o processo e decidir quando tentar conectar novamente cabe ao chamador. `over` permite fornecer outro
transporte assíncrono, inclusive `tokio::io::duplex` nos testes.

`subscribe(session_id, after_seq)` começa depois da sequência informada; `Some(0)`
começa em 1 e `None` pede o registro mais antigo disponível. `next` entrega saída e
notificações na ordem recebida. `subscription.closed` provoca nova assinatura em
`last_seq + 1`; falha nessa tentativa aparece em `ResubscribeFailed`, com código e
limites do host. Não há salto silencioso quando o replay foi descartado. Após um
`close`, a tentativa pode receber `UnknownSession`, porque o host removeu a sessão.

`ClientConfig` é alterável a cada conexão. O padrão recomendado vem de
`ClientConfig::recommended(&host_config)`: `max_frame_bytes` usa o limite de quadro
do host; `queue_capacity` usa `subscriber_queue_chunks`. Este último limita a fila
de eventos, a fila de comandos e o número de pedidos pendentes. Para consumidores comuns, uma fila de eventos
cheia encerra a conexão com `Capacity`; o consumidor drena os eventos já guardados
e reconecta com a última sequência consumida. Depois que o cliente para, qualquer que seja a
causa, toda chamada responde `Disconnected`, e só `next` informa a causa; numa chamada,
`Capacity` significa apenas fila de pedidos cheia, com o cliente ainda ativo. A composição da API habilita contrapressão nessa fila e a consome também ao
esperar respostas, preservando o canal sob rajadas de saída. A memória é limitada pelo produto do
limite de quadro pelo tamanho das filas, além dos comandos fornecidos pelo chamador.
Erros guardam apenas categorias, códigos e limites; mensagens do host e payloads
recusados não são registrados. Não há persistência de saída nem telemetria.

A leitura dedicada preserva quadros parciais se uma chamada ou `next` for cancelada.
Cancelar a espera de uma chamada não desfaz um comando já enviado. Prazos de espera
são aplicados pelo chamador com `tokio::time::timeout`. Descartar o cliente encerra
suas tarefas e o canal, preservando as sessões no host; outro cliente pode retomá-las.

Para executar os testes Windows, compile antes o binário utilizado pelo teste:

```text
cargo build -p ads-ptyhost -j 2
cargo test -p ads-daemon -j 2
```

O teste cria a configuração sob a pasta de artefatos do Cargo e usa um pipe exclusivo.
Configura `claude` e `codex` com o programa `cmd.exe` e argumentos fixos
`/d /c echo <ferramenta>-ok [%<chave>%] [%SystemRoot%]`, e o daemon recebe
`ANTHROPIC_API_KEY` e `OPENAI_API_KEY` com um valor de teste. Pela API WebSocket,
confere que a chave chega literal ao filho (`%...%` não expandido, ou seja, a
variável não existe nele) e que `SystemRoot`, permitida, chega com o valor do
pai; depois encerra o host de teste. Não abre agentes nem shells interativos. Só no Windows,
`ended_session_leaves_no_child_behind_on_windows` (`tests/local_api.rs`) roda o host no próprio
processo de teste com ConPTY: um `cmd.exe` abre outro, minimizado num console próprio, que espera
60 s com `timeout.exe`, e sai depois de 3 s. O teste acha esse filho pelo processo pai, não pela lista
do Job Object, confere que ele está no job e que, quando o daemon devolve a vaga sozinho, ele e o que
iniciou não continuam vivos. Fechar o console do terminal não o encerraria; só o job encerra. Os demais testes usam `FakeBackend` e rodam também em Linux.

Para desfazer a integração, remova o consumidor da biblioteca; nenhuma sessão é
migrada nem formato de armazenamento é alterado. A contiguidade de `seq`, os códigos
de erro e a saída do comando real são as evidências de funcionamento nos testes.

## Porta da interface em desenvolvimento

Antes de abrir a URL, confira que a porta de `ui_base_url` pertence ao próprio
usuário. `npm run dev` na pasta `ui` já fixa a porta 5173 com `strictPort`
(`ui/vite.config.ts`).
Se a porta estiver ocupada, pare e investigue; não abra a URL nem aceite outra
porta automaticamente. Configurar uma URL diferente não comprova sua propriedade.
Outro usuário pode servir uma página nessa porta e ler o fragmento com o token.
O risco e o próximo passo estão na [ADR 0020](../../docs/adr/0020-local-api-authentication.md#limite-conhecido)
e em [riscos](../../docs/risks.md#outro-usuário-ocupa-a-porta-da-interface).
