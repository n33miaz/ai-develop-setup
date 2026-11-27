# ads-core

Política do produto (tarefas, aprovações, escopo, roteamento e orçamento), sem rede, banco nem PTY.

## Orçamento e roteamento

O [orçamento](src/budget.rs) avalia os quatro tetos do pacote contra o livro-caixa reconciliado da tarefa e o tempo decorrido passado pelo chamador. Conta uma tentativa por linha, inclusive falhas, e soma apenas USD faturável. Para a cota, soma a maior variação positiva entre as janelas de cada linha. Janelas sem par antes/depois e quedas de percentual contribuem zero. O padrão de aviso é 80%, configurável em [budget.toml](../../defaults/budget.toml). Atingir qualquer teto retorna as dimensões atingidas e a razão de cancelamento `budget`.

O [roteador](src/routing.rs) interpreta [routing.toml](../../defaults/routing.toml). Recebe tipo, tamanho e sensibilidade da tarefa, um catálogo com os modelos e esforços aceitos e as observações de cota do vínculo atual de cada ferramenta e provedor. Usa a primeira regra compatível, a ordem das faixas, o menor `rank` e depois a ordem lexicográfica dos identificadores e campos da rota. Cada candidato deve permitir explicitamente a classe da tarefa. A observação recente mais nova prevalece, com maior percentual em empate. Sem candidato elegível, a proposta fica vazia.

`shadow` é o padrão e a escolha manual configurada continua em vigor. `manual` só registra essa escolha, `propose` acrescenta a proposta e `auto` explícito retorna os dados da decisão calculada. O fallback em `shadow` e `propose` descreve a alternativa simulada. Quem integra o núcleo fornece o instante e os metadados do envelope e é responsável por publicar os eventos e aplicar decisões.

## Passagem entre sessões

`handoff` avalia os gatilhos da [ADR 0027](../../docs/adr/0027-session-handoff.md), calcula o próximo nome com o instante UTC recebido e valida a cadeia completa, da primeira passagem em diante. Valida vínculos consecutivos, continuidade da sessão de origem, linhagem, sequência sem saltos e unicidade dos identificadores de passagem, de sessão, de documento de estado e de prompt de abertura. Exige em toda a cadeia o usuário que consentiu na primeira passagem e recusa instante de criação anterior ao da passagem anterior; a comparação é entre instantes, não entre textos, e instantes iguais passam.

Antes de anexar uma passagem, o núcleo compara o registro `record.session-handoff:2` com o evento `accepted` de `session.handoff:4` lido do log autenticado. A passagem, a sessão de origem, o gatilho e o canal devem coincidir. O campo `consent.event_id` deve identificar o evento e `consent.user_id` deve identificar o dono da linhagem a quem o aparelho pertencia no instante do aceite. Para contexto ou troca de conta, o chamador usa `verify_consent`. Para `model_refusal`, usa `Offer::accept`, que exige o mesmo vínculo com o registro e confere a alternativa escolhida entre as oferecidas.

`handoff_document` gera, a partir de um modelo de entrada que o chamador fornece, o documento de passagem (trabalho aberto, decisões, regras em vigor, onde estão as coisas) e o prompt de abertura da sessão seguinte. O nome segue `<projeto> n<N> · <data>`, com `N` incrementado a cada passagem; da segunda sessão em diante o documento aponta para o anterior, e a primeira não tem anterior. Cada item é uma linha de texto, com limite de tamanho e de quantidade, para não forjar outra seção. A geração é pura: não lê arquivo, pasta de perfil nem credencial. O evento `session.handoff_proposed:2` valida esse vínculo e leva só metadados da proposta (sessão, limiar, percentual de contexto, `N` e documento anterior), nunca conteúdo da conversa.

As opções em [`defaults/handoff.toml`](../../defaults/handoff.toml) podem ser trocadas ao carregar outra política. Quem chama fornece a medição, quando disponível, e verifica o consentimento para a passagem específica com `verify_consent` ou, após recusa de modelo, com a oferta de `refusal`. Nos dois modos ativos, o aceite autoriza a continuidade; a recusa de modelo nunca segue sem ele. Para troca de conta, quem chama aplica primeiro a política da ADR 0022. A decisão do núcleo não abre sessões.

[`refusal`](src/refusal.rs) monta a oferta de outro modelo após recusa ou filtro de segurança, a partir dos eventos `session.refusal_detected:1` do mesmo trabalho. Usa a ordem e as regras do roteador e a cota observada, nunca repete ferramenta e modelo que já recusaram e não oferece nada com orçamento parado, passagem em `off` ou [`defaults/refusal.toml`](../../defaults/refusal.toml) desligado. A rota só é aceita com um evento `accepted` de `session.handoff:4` para a mesma passagem e sessão, emitido por um aparelho do dono da linhagem no instante do aceite. O evento deve trazer o gatilho `model_refusal`, o canal e o `alternative_id` da rota escolhida entre as oferecidas. `detected` monta o evento de recusa só para ferramenta, provedor e modelo do catálogo. O núcleo não detecta a recusa nem abre a sessão.

## Fila contínua

A [política da fila](src/queue.rs) restaura a fila parada e calcula transições usando o registro atual e, quando exigido, o aparelho autenticado. O aparelho autenticado leva o dono a que pertence e o canal (`Channel`, do próprio núcleo, convertido no `via` de cada evento). A política confere esse dono contra o da fila. A partida, o teto de custo e a falta de recursos param a fila com os dados de `queue.stopped` do daemon. Na partida, a restauração considera o retrato e o último estado registrado no log, e registra a parada se qualquer um dos dois indicar a fila ligada.

`change_queue_state` devolve o novo estado junto com os dados do evento (`queue.enabled` ou `queue.stopped` com `reason: owner`), montados do aparelho conferido. Uma mudança para o estado atual é recusada com `Unchanged`, então nunca sai parada para uma fila já parada. O produtor monta o envelope com `actor.id` igual ao `device_id` do aparelho devolvido.

Na partida, `restore_running_task` decide o destino da tarefa cujo último estado é `running`. O chamador passa o estado e a contagem reconstituídos pelo replay, para uma queda entre o log e o retrato não escapar da regra, e diz o que o host de terminal informou sobre a sessão da tarefa (`HostProcess`). A restauração e o cálculo de transições aceitam retratos `record.queue-task:1`, `:2` e `:3`. O daemon ainda precisa associar a tarefa à sessão, consultar o host, aplicar o prazo e solicitar o encerramento quando a sessão reaparecer. Os casos:
- `Keep`: o host lista a sessão com o processo rodando (`Kept`), e a tarefa continua em `running`. O núcleo volta a acompanhar a sessão, e a tentativa termina normalmente, mesmo com a fila parada.
- `AwaitHost`: o host ainda não respondeu (`Unconfirmed`) e não passaram `host_confirm_seconds` (60) desde que o núcleo começou a perguntar. A tarefa fica em `running` no log, sem evento novo, e nada a move até o host responder ou o prazo vencer. Esse estado é só do núcleo: a cada partida ele é calculado de novo.
- `Fail`: o host respondeu que a sessão terminou ou não existe (`Lost`), e a tarefa passa para `failed` com a mesma contagem, porque a tentativa interrompida já foi contada. Uma sessão que terminou enquanto o núcleo estava fora também cai aqui: o resultado dela não é julgado.
- `Abandon`: o prazo venceu sem resposta do host. A tarefa passa para `cancelled` com a mesma contagem e o motivo `host_unconfirmed`. O cancelamento é terminal, inclusive após replay e nova ativação da fila. A sessão anterior pode continuar viva.
- `Cancel`: a contagem é inconsistente (zero ou acima do teto). A tarefa passa para `cancelled` com a contagem gravada, e o chamador encerra o processo se o host o manteve ou o listar depois.

Hoje o daemon não adota sessões antigas ao reiniciar, então `Kept` ainda não acontece na prática.

O armazenamento devolve o stream como envelopes (`EventLog::read_queue`). `QueueStreamEvent::from_envelope` lê cada um no contrato do seu tipo, entre as versões que o replay aceita, e recusa tipo desconhecido (`UnknownType`) ou envelope fora do contrato (`Invalid`); `as_log` entrega a forma emprestada que o replay lê.

O replay (`replay_task`) recebe o stream inteiro da fila, a partir de `queue.created` com `seq` 1 e sem buraco, e uma consulta ao cadastro de aparelhos no instante de cada evento. Ele devolve o estado, o saldo, a última âncora e a política vigente, ou o motivo da recusa (`ReplayRefusal`). Além da ordem e do saldo, ele confere:
- todo evento é da fila e do stream `queue/<id>`;
- todo ator que é aparelho pertencia ao dono da fila naquele instante;
- a criação, a ativação, a parada pelo dono e a mudança de política vêm de um aparelho do dono e repetem o dono da fila;
- todo pedido manual, de qualquer tarefa da fila, foi criado por um aparelho do dono;
- a tarefa só entra em `running` com a fila ligada naquele ponto do log;
- `review → done` foi aceito por um aparelho do dono.
- os eventos novos de mudança de estado usam `queue.task_state_changed:2`. O construtor e a leitura do evento exigem `via` para aparelhos e recusam o campo para os demais atores. O replay aceita eventos da `:1` conforme as regras dessa versão.

`replay_queue` reconstrói numa só passada pelo stream o registro de todas as tarefas da fila, em `record.queue-task:3` e na ordem de criação, com a âncora e a política vigente. Aplica a cada tarefa as conferências de `replay_task`, que segue uma só e pula as mudanças das outras. Por isso uma mudança de estado de tarefa nunca criada no stream, ou uma conferência que falha em qualquer tarefa, recusa a fila inteira. Uma tarefa criada duas vezes, mesmo com pedidos distintos, é recusada (`TaskCreation`) pelos dois replays, qualquer que seja a tarefa que `replay_task` segue. `created_at` é o `ts` da criação e `updated_at` o do último evento da tarefa, ou o da criação quando o relógio recuou para antes dela: o registro nunca traz `updated_at` anterior a `created_at`. O título e a ferramenta vêm de `queue.task_created:4`. Para uma tarefa criada pelas versões 1 a 3, que não os registravam, o registro traz `null` nos dois, sem inventar valor, e a interface mostra que não foram registrados. Nenhum evento da fila registra branch nem pull request, então o registro reconstruído não os tem.

O agente e o daemon continuam podendo reprovar e cancelar tarefas. O replay confere a coerência dos atores que o log declara, não a autenticidade deles: o log não é assinado por aparelho, e uma linha forjada é recusada pela cadeia HMAC do armazenamento ([ADR 0021](../../docs/adr/0021-storage-atomic-file-and-cipher.md)). O replay recusa erro do produtor e log montado de forma errada nas conferências acima, mas não compara o `ts` de uma criação com o relógio real (ver abaixo).

Na criação da fila, `new_stopped` (ou `new_local_stopped` para o dono autenticado pela composição local) entrega o registro parado e os dados de `queue.created:2`, que exigem a política completa. `new_local_stopped` recebe o dono e a política como dados e não grava nada; quem garante que só o dono cria a fila é o produtor, que no daemon só grava a criação com a prova de sessão local. O produtor anexa esse evento no `seq` 1, com o aparelho autenticado como ator, antes de usar a fila. O replay confere as admissões contra a política gravada vigente em cada ponto do stream.

Para `queue.created:1`, a política é desconhecida até o primeiro `queue.policy_changed:1`. Nesse trecho o replay reconstrói as tarefas e confere os vínculos, atores, origens únicas, carimbos e transições com o teto gravado em cada tarefa, sem revalidar os limites desconhecidos. As admissões desse trecho continuam contando no histórico quando uma política passa a vigorar. O daemon ainda não admite tarefas nem grava mudanças de política pela API. Antes de habilitar a admissão, o produtor deve recusar novas tarefas quando o replay devolver política ausente e permitir que o dono grave a primeira política.

O dono altera a política por um aparelho autenticado. `change_queue_policy` confere o dono e o último `seq` observado e, como a ativação, aceita só `local_ui` e `tray`: recusa `remote_device` com `QueueRefusal::RemotePolicyChange`, porque uma política nova pode subir os tetos de admissão. A reancoragem do relógio continua aceitando `remote_device`. O produtor anexa `queue.policy_changed:1` com o aparelho autenticado como ator e condiciona o append ao mesmo `seq`. O evento fica coberto pela cadeia HMAC. A política entra em vigor após o append e vale para admissões posteriores. O replay confere o dono do aparelho no cadastro e aplica a política na ordem do stream. Cada tarefa conserva o teto de tentativas gravado na sua criação.

O replay usa o `ts` do envelope de cada criação como relógio daquela admissão. O armazenamento cobre esse campo pela cadeia HMAC. Como o relógio de cada criação é o próprio carimbo, o replay nunca devolve `AheadOfClock`. Ele confere a política, a origem, os limites, a ordem dos carimbos entre si (`BehindLastAdmission`) e a âncora (`ForeignAnchor`, `AnchorAfterCandidate`, `AnchorAheadOfClock`). Uma criação recusada por uma dessas conferências resulta em `Admission`. A reserva de IDs entre filas cabe ao produtor.

O cadastro responde pelo dono do aparelho em cada instante. Um aparelho mantém o dono que tinha quando agiu, e a revogação é uma marca com data: ela recusa os atos posteriores sem apagar os anteriores. O produtor usa o estado e o saldo reconstituídos antes de autorizar novas transições e reserva a origem na mesma operação que anexa a criação.

[defaults/queue.toml](../../defaults/queue.toml) fornece os limites embutidos para filas novas. Chaves opcionais ausentes usam os padrões distribuídos. Nas filas existentes o produtor usa a política reconstruída do log. A admissão (`can_admit_task`) recebe a política, o relógio, o dono da fila, o aparelho que pediu e a âncora de relógio em `AdmissionContext`. Ela devolve o motivo da recusa (`AdmissionRefusal`), para a interface poder explicá-lo. A admissão recusa:
- criação com teto de tentativas acima do configurado;
- pedido manual que não foi criado por um aparelho do dono da fila, com `actor.id` igual ao do aparelho;
- `task_id` já usado em qualquer fila;
- origem repetida na fila;
- a fila que atingiu o total de admissões (`admission_limit`, 1000), que inclui origens encerradas;
- na janela de `window_seconds` (uma hora) que termina no `ts` da criação:
  - mais de `manual_window_admissions` (10) pedidos manuais;
  - mais de `window_admissions` (10) tarefas vindas de issue ou de regra de bug;
  - mais de `bug_rule_window_admissions` (3) da mesma regra de bug.

`queue.task_created:4` é o contrato das novas gravações: exige um ator aparelho para pedidos manuais, usa os tipos comuns de apelido, de origem e de título e registra o título e a ferramenta da tarefa. O núcleo já lê essa versão; a gravação dela pelo produtor ainda não está integrada. O registro da tarefa não é gravado à parte: `replay_queue` o reconstrói do stream. `can_admit_creation` e o replay leem as quatro versões da criação, inclusive num mesmo histórico, e a origem repetida é recusada entre versões. O Schema da `:1` aceita pedidos manuais de agente ou daemon, mas o núcleo recusa esses pedidos em todas as versões e exige o vínculo do aparelho com o dono. Os pedidos manuais contam à parte, para que as origens automáticas não ocupem o espaço do dono.

O `ts` da criação vem do relógio do núcleo, nunca do pedido, e o produtor informa esse relógio em `now`. A admissão recusa a criação com `ts` mais de `clock_skew_seconds` (5) adiante de `now` (`AheadOfClock`) ou mais de `clock_rewind_seconds` (300) antes de uma admissão anterior da fila (`BehindLastAdmission`). Na admissão ao vivo, `AheadOfClock` compara o `ts` com o `now` informado pelo produtor, então um carimbo adiantado em relação ao relógio do núcleo não esvazia a janela. Um ajuste pequeno do relógio para trás não trava a fila. No replay, o `now` de cada criação é o próprio `ts`: só a ordem relativa dos carimbos e a âncora são conferidas, não o adiantamento em relação ao relógio real. Uma criação gravada com carimbo adiantado passa no replay. Se o adiantamento passar de `clock_rewind_seconds`, as admissões ao vivo seguintes recebem `BehindLastAdmission` até o relógio alcançar o carimbo ou o dono reancorar a fila ([registro de riscos](../../docs/risks.md#carimbo-adiantado-aceito-no-replay-da-fila)). Uma admissão anterior com `ts` depois do da candidata, dentro da tolerância, continua contando na janela.

Se o relógio do núcleo salta para a frente, `ts` e `now` podem avançar juntos e fazer admissões anteriores ficarem fora da janela. Depois da correção, `BehindLastAdmission` indica que a candidata ficou mais de `clock_rewind_seconds` antes de uma admissão anterior. O motivo tem precedência sobre os limites de quantidade e vale também para pedidos manuais. O dono pode reancorar o relógio da fila para retomar a admissão antes de o relógio voltar à faixa de tolerância. `reanchor_admission_clock` recebe o aparelho do dono, o relógio do núcleo no momento do ato (com o mesmo contrato do `ts` da criação), o `seq` sobre o qual o dono agiu e o último `seq` do stream lido do armazenamento. Os dois `seq` têm de ser iguais (`StaleSeq`). A função devolve a âncora (`ClockAnchor`). Na admissão, as criações até esse `seq` com `ts` posterior à âncora e ainda adiante do relógio contam como admitidas no instante da âncora. Elas continuam reservando a origem e ocupando a janela que começa ali. As criações que o relógio já alcançou não se movem, então nem uma âncora antiga tira uma criação comum da janela. A admissão recusa a âncora que cobre o `seq` da candidata ou um posterior (`AnchorAfterCandidate`), porque ela moveria admissões feitas depois do ato do dono. Também recusa a âncora com instante mais de `clock_skew_seconds` adiante do relógio (`AnchorAheadOfClock`). O produtor anexa `queue.clock_reanchored:1` com o aparelho autenticado como ator e o instante da âncora em `ts`. A anexação deve ser condicional ao último `seq` conferido para a âncora cobrir exatamente até `seq - 1`. O replay valida o vínculo do aparelho com o dono no instante do ato, aplica a âncora às criações posteriores e devolve a última âncora para uso nas novas admissões.

Os limites da janela se liberam sozinhos com o tempo. Uma fila que atingiu o total só volta a admitir quando o dono aumenta `admission_limit`. Criar outra fila também resolve, mas a chave de deduplicação inclui a fila: a fila nova não herda as origens reservadas, e a mesma issue pode ser admitida de novo.

`AuthenticatedDevice` não tem construtor público. Toda função pública deste crate pode ser chamada por qualquer crate que dependa dele, e a matriz de dependências deixa a camada de autenticação fora do núcleo. Por isso nenhum construtor aqui distingue a camada de autenticação de um chamador que copia o id do aparelho de um pedido. O construtor entra junto com essa camada e com o cadastro de aparelhos. Para o dono agindo pela interface local, cuja conexão a composição já autenticou, `change_local_queue_state` aplica a mesma transição de `change_queue_state` recebendo o dono como dado, sempre com o canal `LocalUi`, e não produz `AuthenticatedDevice`; por isso não abre nenhuma outra política reservada ao dono (mudança de política, reancoragem, aceite de tarefa). A política inicial de `new_local_stopped` e a transição de `change_local_queue_state` são só dados: o daemon as grava apenas dentro de uma chamada que traz a prova de sessão da conexão local, e esta camada não as autentica.

## Ambiente do processo filho

O núcleo expõe a política de [`ads-child-env`](../child-env/src/lib.rs) em `ads_core::child_env`.

Política pura do ambiente que um processo de ferramenta recebe: só passam os nomes da lista permitida que o processo pai tem, escritos como a lista os escreve, mais os valores que o produto define (`TERM`, `COLORTERM`). Nome em `allow` ou em `set` que casa com um padrão negado (`NOME`, `PREFIXO*`, `*SUFIXO`, `*TRECHO*`, sem diferença de maiúsculas) recusa a política na carga; o filtro confere de novo e o descarta. A exceção são os nomes inofensivos listados pelo produto para um padrão em `[env.deny_exceptions]` (por exemplo `WT_SESSION` para `*_SESSION*`); a camada do usuário pode negá-los, mas não criar exceções. Nomes repetidos (pela comparação do sistema: no Windows também letras fora do ASCII, `ÄX` e `äx`), nomes com `=`, `*` ou controle e valores de `set` com NUL ou controle também recusam. Os erros dizem a lista e a posição, nunca o texto recusado. Padrões de nome não reconhecem um segredo sob um nome qualquer: a garantia é que nada de nome reconhecidamente sensível passa e que o que passa está todo em `allow`. Os padrões ficam em [`defaults/harnesses.toml`](../../defaults/harnesses.toml) (`[env]`): a camada do usuário substitui `allow` e `set` e só acrescenta a `deny`. A comparação com `allow` ignora maiúsculas no Windows (`NameCase::Insensitive`). O filtro recebe o ambiente como dado; nesse caso só a caixa ASCII é ignorada e, com dois nomes do pai que só diferem na caixa, vale o escrito igual à lista, senão o menor. Para o ambiente real, `process_env` não lista o ambiente: consulta só cada nome permitido pela política validada, pela busca nativa do sistema, e o filho recebe o nome escrito como a lista o escreve. No Windows essa busca iguala também letras fora do ASCII (`allow = ["äx"]` acha o `ÄX` do pai) e, se o pai tiver duas grafias do mesmo nome, devolve a que o sistema achar primeiro. Isso não abre leitura de nome negado: para o Windows tomar dois nomes como um, os dois têm a mesma forma em maiúsculas, e a conferência de negação já testa o nome permitido nas formas em maiúsculas, em minúsculas e de largura cheia; um nome do pai que um padrão negaria tornaria negado o próprio nome permitido, recusado na carga. `tools/test_env_enumeration.py` reprova código de produção do núcleo e do daemon que liste o ambiente (`env::vars`, `vars_os`).

## Abertura de ferramentas

Em `src/tool_launch/`, o núcleo carrega
[`defaults/harnesses.toml`](../../defaults/harnesses.toml) com as substituições do
usuário em `[tools.<id>]`, `[workdirs]`, `[accounts]` e `[env]`. Outras tabelas de ferramenta
são tratadas pelos adaptadores correspondentes.

Cada adaptador declara a ferramenta com `ads_core::tool_harness!`, que tira do
identificador as chaves dos diagnósticos (`tools.<id>`, `tools.<id>.program`,
`tools.<id>.args`, `tools.<id>.mode`). O trait `Harness` fica aberto: selá-lo
poria a lista de ferramentas, com nomes de fornecedores, no núcleo. Uma
implementação de fora só carrega se o identificador tiver tabela em
`defaults/harnesses.toml`, e aí lê a mesma tabela, com a mesma política, que o
adaptador daquela ferramenta. Identificador sem tabela embutida é recusado com
`MissingKey`, mesmo que o arquivo do usuário traga a tabela completa.

O pedido `session.open` fornece a ferramenta, o apelido da pasta e o tamanho.
`open_params` monta o pedido do contrato `pty-host/1`. A função do núcleo aceita
qualquer `Harness` e recusa em tempo de execução, com `UnknownHarness`, o pedido
de outra ferramenta. Os adaptadores expõem uma versão restrita à configuração da
própria ferramenta: passar a de outra não compila, o que um exemplo de documentação
com `compile_fail` mostra ao lado de um exemplo idêntico que compila (um teste
confere que os dois só diferem na ferramenta).

`Probe` permite conferir a existência do programa e da pasta na hora da
abertura. `DiskProbe` faz essa consulta no disco.

### Perfis de conta

No arquivo de configuração do usuário, `[accounts.<ferramenta>.<apelido>]`
define `profile_dir`. O apelido segue as regras de `workdirs`. O caminho é
absoluto no estilo do sistema em que o núcleo roda e segue as mesmas regras
de sintaxe. A validação compara os componentes do caminho normalizado e
recusa pastas iguais ou aninhadas entre perfis e em relação às pastas de
`[workdirs]`. Configure pastas de perfil sem links simbólicos nem junções
no caminho, conforme o [risco registrado](../../docs/risks.md#contas-por-pasta-de-perfil).
A futura ligação na abertura deve ocorrer após o filtro final de `open_params`.

Em `[tools.<id>]`, `switch` aceita `manual`, `propose` (padrão) e `auto`.
`switch_at_pct` aceita inteiros de 1 a 100, com padrão 90% da cota observada.
Antigravity fica em `manual` sem configuração de troca.

`ProfileBinding` aplica a constante de ambiente do adaptador depois do filtro.
O processo pai e `env.set` continuam sujeitos a `env.deny`. A abertura de
sessões ainda usa o ambiente sem perfil. O teste de isolamento do mantenedor
e a medição de cota precedem a ativação da troca, conforme a
[ADR 0022](../../docs/adr/0022-account-profile-folders-and-automatic-switching.md).

### O que a abertura garante

- Quem pede a sessão escolhe só a ferramenta (pelo nome), o apelido da pasta e o
  tamanho. Programa, argumentos, ambiente e pasta vêm da configuração; o contrato
  do pedido recusa esses campos e a função não tem por onde recebê-los.
- Programa: caminho absoluto e literal, que precisa ser um arquivo existente
  quando a sessão abre. Caminho vazio (o padrão) desliga a ferramenta.
- Modo: só `interactive` (terminal operado pela pessoa); `headless` e chaves
  desconhecidas recusam a configuração.
- Formas de caminho recusadas no programa e na pasta: relativo (inclusive `C:pasta`
  e `\pasta`), variável (`%VAR%`, `$VAR`), `~`, `..`, caminho de rede, prefixos
  `\?\` e `\.\`, fluxo alternativo (`arquivo:fluxo`), nome curto do Windows
  (`NOME~1`), nome de dispositivo (`NUL`, `COM1`), componente terminado em ponto
  ou espaço, caracteres reservados, curinga e caractere de controle. Caminho
  escrito no estilo do outro sistema (`/usr/bin/x` no Windows, `C:\x.exe` fora
  dele) é recusado na carga, com erro próprio; `HostRules` diz à carga as regras
  do sistema (`HostRules::host()` fora dos testes).
- Argumentos: só os fixos da configuração, na ordem dela; com caractere de
  controle, a configuração é recusada.
- Pasta: o apelido precisa estar em `[workdirs]` (comparação exata) e a pasta
  precisa existir. A raiz de uma unidade ou `/` é recusada, para a ferramenta não
  trabalhar no disco inteiro. A pasta vai sempre preenchida, para o processo dos
  terminais nunca cair na pasta atual dele.
- Ambiente: exatamente os nomes de `env.allow` que o processo pai tem, mais os
  valores de `env.set` (`TERM`, `COLORTERM`). Nome em `allow` ou em `set` que
  casa com `env.deny` recusa a configuração, e o filtro ainda o descarta se
  chegar até ele. Os padrões negados cobrem o nome de credenciais (`*TOKEN*`,
  `*SECRET*`, `*PASSWORD*`, `*API_KEY*`, `*_KEY` e outros), o de variáveis que
  apontam para elas (`*_KEY_FILE`, `GOOGLE_APPLICATION_CREDENTIALS`) e a
  configuração de login de ferramentas e provedores (`CLAUDE_*`, `ANTHROPIC_*`,
  `OPENAI_*`, `CODEX_*`, `GH_*`, `AWS_*` e outros; lista completa no arquivo). A
  camada do usuário só acrescenta padrões a `deny`. Valor de `set` com NUL ou
  outro caractere de controle é recusado; valor do pai com NUL é ignorado.
- Erros de configuração e de abertura não carregam caminho, valor, nome de
  variável nem o texto do analisador de TOML: só a chave, a posição na lista, a
  linha ou um código.

### O que a abertura não garante

- Um padrão de nome não reconhece um segredo guardado sob um nome comum
  (`MEU_VALOR=<token>`): o que se garante é que nenhuma variável de nome
  reconhecidamente sensível passa e que tudo o que passa está em `env.allow`,
  revisável num lugar só.
- O que a ferramenta faz depois de aberta: o agente lê e edita arquivos conforme
  as permissões do processo, na pasta escolhida e fora dela; o portão de ações do
  produto não cobre o que se faz dentro do terminal.
- O login da ferramenta: fica com ela, no lugar padrão dela, achado pela pasta do
  usuário. As variáveis da pasta de configuração (`CLAUDE_CONFIG_DIR`,
  `CODEX_HOME`) e as demais que redirecionam o login não passam.
- A abertura resolve links do programa e da pasta de trabalho, aplica as regras
  de caminho ao destino e passa o caminho resolvido ao host. Veja os
  [riscos de troca de caminho](../../docs/risks.md#programa-ou-pasta-configurados-por-um-link).
- Que o programa seja o certo: confere-se que o arquivo existe, não quem o
  assinou. O arquivo pode sumir entre a conferência e a abertura; aí quem falha é
  a abertura no processo dos terminais.
- No Windows, programas `.cmd` e `.bat` (instalações por npm) rodam pelo
  `cmd.exe`, com as regras de aspas dele. Como os argumentos vêm só da
  configuração, isso fica documentado, não tratado.

### Testes

`cargo test -p ads-core -j 2` roda a suíte de abertura
(`src/tool_launch/tests/suite.rs`) com uma ferramenta que só existe nos testes,
com tabela própria acrescentada aos padrões. Cada adaptador roda a mesma suíte
com o seu identificador.

## Portão de ações destrutivas (`src/gate/`)

Nenhuma ação sobre o sistema de arquivos é executada sem passar por aqui. O portão não toca o disco: recebe o plano como dado e responde se, e como o quê, ele pode rodar. Os tipos só se encaixam de um jeito: o pedido só vira plano pela normalização, o veredito só sai de `classify`, a ação só nasce de um veredito (a de um veredito negado já nasce no estado final) e a licença de execução só sai de uma ação cujo hash do plano e cuja aprovação conferem. O que o portão escreve (eventos e o registro do plano) segue os contratos de [`contracts/`](../../contracts/README.md#portão-de-ações), e o estado de uma ação se reconstrói só desses eventos.

| Arquivo | O que faz |
|---|---|
| `path.rs` | Normalização de caminhos absolutos, Windows e POSIX, só por texto (sem `std::path`, para julgar igual em qualquer sistema). |
| `plan.rs` | Plano de ação (operação, alvos na ordem canônica, raiz de trabalho, estimativa) e o `plan_hash`. |
| `policy.rs` | Parâmetros do portão e as pastas protegidas. Os padrões ficam só em [`defaults/action-gate.toml`](../../defaults/action-gate.toml). |
| `classify.rs` | As regras e os seus códigos, o veredito ou erro de orçamento, a reescrita de apagar como mover para a quarentena e o registro do plano julgado. |
| `machine.rs` | Máquina de estados da ação, a licença de execução e a reconstrução do estado a partir dos eventos (`replay`). Cada transição devolve o evento do contrato. |
| `event.rs` | Valores de domínio, vínculo entre stream e entidade e serialização canônica do plano; a leitura e a escrita usam os contratos gerados. |
| `conversion.rs` | Conversões explícitas, campo a campo, entre os contratos de `ads-contracts` e os valores do portão, com as conferências que o Schema não exprime. |

### Normalização

O plano só existe com todos os caminhos absolutos e normalizados (separadores repetidos e `.` retirados, letra da unidade em maiúscula). Um caminho recusado invalida o plano inteiro; nada é consertado. São recusados:

- caminho relativo, inclusive `C:pasta` e `\pasta`;
- qualquer componente `..` (resolver por texto dá o caminho errado quando há um link no meio);
- variável ou til não expandidos: qualquer `$` ou `%`, e caminho ou componente começando com `~`;
- curinga (`*`, `?`) fora do último componente, e em qualquer lugar da raiz de trabalho ou do destino;
- caminho de rede (`\servidor\pasta`, `//servidor/pasta`) e os prefixos `\?\` e `\.\`;
- no Windows, o que é apelido de outro caminho: nome curto (`NOME~1`), componente terminado em ponto ou espaço, `:` depois da unidade, nomes de dispositivo com qualquer extensão (`NUL`, `nul.txt`, `COM0` a `COM9`, `LPT0` a `LPT9` e os mesmos com `¹`, `²` ou `³`) e `< > " |`;
- caractere de controle; `\` dentro de componente POSIX; lista de alvos vazia.
- o que não cabe no registro do plano: caminho normalizado com mais caracteres que `urn:ads:common:abspath:2`, ou alvo cujo texto inteiro (com o curinga) passa do `subject` do registro, onde uma regra o cita (erro `TooLong`), curinga maior que o `pattern` do registro (`PatternTooLong`) e mais alvos, depois de juntar os repetidos, do que o registro guarda (`TooManyTargets`). Os limites vêm das constantes geradas do contrato (`MAX_CHARS`, `MAX_ITEMS` em `ads-contracts`), não de número copiado.

Alvo com curinga é julgado como a pasta sobre a qual o curinga age: `pasta/*` vale como `pasta`.

#### Identidade de um alvo no Windows

No estilo Windows, dois alvos que só diferem na caixa de letras ASCII, no caminho ou no curinga (`C:\w\Build` e `c:\W\build`, `*.LOG` e `*.log`), são o mesmo alvo: é a mesma medida da contenção que permite (`is_within`). Eles viram um só, com `repo_root` verdadeiro se qualquer grafia o tinha, e fica a grafia que vem primeiro na ordem canônica do contrato (bytes, maiúscula antes de minúscula). A grafia gravada e o `plan_hash` não dependem, então, da ordem do pedido. Letra fora do ASCII (`é` e `É`) e qualquer caixa no estilo POSIX continuam alvos diferentes.

A ordem e a serialização do contrato não mudam: o registro guarda uma grafia real do pedido, para exibição, e o hash é tomado sobre ela. Por isso um plano pedido só como `C:\w\Build` e outro só como `C:\w\build` têm hashes diferentes, e a aprovação de um não serve para o outro. Dobrar a caixa no hash mudaria a serialização canônica e pediria nova versão do registro, sem ganho para o julgamento, que já ignora a caixa ASCII nos dois lados.

Limite conhecido: numa pasta em que o Windows foi configurado para diferenciar caixa, as duas grafias podem ser pastas diferentes, e o plano fica só com uma. Quem executa age sobre a grafia gravada: faz menos do que o pedido, nunca mais.

O Schema `urn:ads:common:abspath:2` recusa o mesmo que esta normalização recusa num caminho já normalizado; um corpus comum (`contracts/corpus/abspath.json`) roda pelos dois e um teste de cada lado cobra o mesmo veredito ([contracts/README.md](../../contracts/README.md#caminho-absoluto-o-schema-e-o-núcleo)). Tudo aqui é comparação de texto: duas grafias Unicode do mesmo nome (NFC e NFD, largura total) são nomes diferentes. Para permitir isso é o lado seguro (a outra grafia fica de fora); para negar é um limite: uma pasta protegida escrita na outra forma de normalização não é reconhecida só pelo texto, e quem executa tem de conferir no disco.

### Regras

Um alvo negado nega o plano inteiro. Comparação por componente (`proj2` não está dentro de `proj`), com duas medidas de "mesmo nome":

- **para permitir** ("está dentro da raiz de trabalho ou de uma raiz permitida", para alvo e para destino): no Windows só a caixa das letras ASCII é ignorada; nos demais, nada. Nomes que só uma dobra de caixa Unicode igualaria (`K` e o sinal Kelvin, `ß` e `SS`) são pastas diferentes e ficam de fora;
- **para negar** (pasta protegida, raiz de trabalho, raiz permitida, quarentena): a comparação mais larga, nos dois estilos, igualando o que qualquer mapeamento de caixa Unicode iguala. Larga de propósito: só serve para negar, nunca para permitir.

| Veredito | Quando |
|---|---|
| Negado | alvo que é raiz de unidade ou `/`; a pasta do usuário ou um ancestral dela; a pasta dos perfis ou o perfil de qualquer usuário; pasta do sistema (ou algo dentro dela); a raiz de trabalho ou algo que a contenha; uma raiz permitida ou algo que a contenha, mesmo que outra raiz a contenha; qualquer coisa que não esteja estritamente dentro da raiz de trabalho ou de uma raiz permitida; raiz de repositório (informada no plano); a pasta de quarentena ou algo que a contenha; o estado do próprio produto (ver abaixo), algo dentro dele ou algo que o contenha, com ou sem curinga |
| Negado | qualquer caminho escrito no estilo que não é o da pasta do usuário (`/c/...` contra `C:\...`): pode ser outra grafia de uma pasta protegida, e o texto sozinho não resolve isso |
| Negado | raiz de trabalho que é ela mesma uma pasta protegida (ou fica dentro do estado do produto); destino de "mover" protegido, fora das raízes permitidas ou dentro de um dos alvos; entrada que o "mover" criaria no destino (`destino/nome do alvo`) protegida, em cima de um dos alvos, ou que é a raiz de trabalho, uma raiz permitida ou a pasta de quarentena (ou contém uma delas) |
| Exige aprovação | apagar recursivo, limpar não rastreados, mover para dentro da quarentena ou apagar simples reescrito para a quarentena, acima do limite de itens ou de bytes, ou sem estimativa (limite zero pede aprovação sempre, qualquer que seja a estimativa); qualquer ação (sobrescrita inclusive) em raiz permitida fora da raiz de trabalho; "mover" com destino fora da raiz de trabalho; apagar algo que já está na quarentena |
| Permitido | o restante dentro da raiz de trabalho |

**Estado do próprio produto.** O banco de auditoria e seus arquivos, a pasta do cofre e da trava e a pasta de controle do lançador entram na política por `GatePolicy::with_product_paths`, que quem compõe o portão chama com os caminhos da configuração do produto (o núcleo não lê disco nem ambiente). A lista só cresce. O alvo que é um deles, fica dentro de um ou contém um é negado (`protected.product_data`), sempre, e não pede aprovação: a ação vem de uma sessão de ferramenta, que não é confiável, e apagar ou mover o log de auditoria apagaria o registro do próprio portão; não há caso legítimo de uma sessão mexer nele. Curinga é julgado pela pasta em que atua, como as demais regras, então `dados\*.log` é negado se `dados` guarda o banco. A raiz de trabalho e o destino de "mover" só são negados quando ficam **dentro** do estado do produto: uma pasta que apenas o contém continua sendo raiz de trabalho e destino, e o que cada mover cria (`destino/nome`, também pelo alcance do curinga) passa pela mesma regra. A comparação é a larga das regras de negar; por isso quem compõe passa todas as grafias de cada caminho: a configurada, a resolvida no disco (links, nome curto, caixa) e a de cada link do caminho, que também é estado do produto.

Os três níveis casam um a um com `urn:ads:common:risk:1`: permitido é `low`, exige aprovação é `medium`, negado é `high`.

#### Códigos de regra

Cada regra tem um código estável (`urn:ads:common:action-rule:1`), que vai no evento `action.denied` e no registro do plano. Um código nunca muda de sentido nem é reaproveitado; regra nova ganha código novo. A lista é a mesma do [`contracts/README.md`](../../contracts/README.md#registro-do-plano), e um teste reprova se alguma das duas ficar para trás.

`<proteção>` é o motivo de a pasta ser protegida: `filesystem_root` (raiz de unidade ou `/`), `user_home` (a pasta do usuário), `user_home_ancestor` (um ancestral dela), `user_profile` (a pasta dos perfis ou o perfil de qualquer usuário), `system_dir` (pasta do sistema), `other_path_style` (caminho no estilo que não é o da pasta do usuário) e `product_data` (estado do próprio produto: o banco de auditoria e seus arquivos, a pasta do cofre e da trava, a pasta de controle do lançador).

| Código | Veredito | Dispara quando | Caminho citado |
|---|---|---|---|
| `protected.<proteção>` | negado | o alvo é uma pasta protegida | o alvo |
| `work_root_protected.<proteção>` | negado | a raiz de trabalho é ela mesma uma pasta protegida | a raiz de trabalho |
| `work_root_itself` | negado | o alvo é a raiz de trabalho ou algo que a contém | o alvo |
| `allowed_root_itself` | negado | o alvo é uma raiz permitida ou algo que a contém, ainda que esteja dentro de outra raiz | o alvo |
| `outside_allowed_roots` | negado | o alvo não está estritamente dentro da raiz de trabalho nem de uma raiz permitida | o alvo |
| `repository_root` | negado | o alvo é raiz de repositório ou worktree, conforme os fatos registrados no plano | o alvo |
| `repository_metadata` | negado | o alvo ou o destino tem um componente `.git`, sem distinguir maiúsculas | o caminho |
| `work_root_unknown` | negado | a política está sem raízes vinculadas, tem uma lista vazia ou a raiz de trabalho não consta da lista | a raiz de trabalho |
| `quarantine_of_other_root` | negado | o alvo ou a entrada que seria movida cruza a quarentena de outro apelido, ou o destino fica dentro dela | o caminho |
| `quarantine_itself` | negado | o alvo é a pasta de quarentena ou algo que a contém | o alvo |
| `destination_protected.<proteção>` | negado | o destino de "mover", ou a entrada que ele criaria no destino, é uma pasta protegida | o destino ou a entrada |
| `destination_outside_allowed_roots` | negado | o destino de "mover" está fora da raiz de trabalho e das raízes permitidas | o destino |
| `destination_inside_target` | negado | o destino de "mover" fica dentro de um dos alvos, ou a entrada que ele criaria cai em cima de um deles (o próprio alvo inclusive: mover para onde já está) | o destino ou a entrada |
| `destination_entry_reserved` | negado | a entrada que o "mover" criaria no destino é a raiz de trabalho, uma raiz permitida ou a pasta de quarentena, ou contém uma delas | a entrada |
| `outside_work_root` | exige aprovação | o alvo está fora da raiz de trabalho (numa raiz permitida; fora delas o plano já está negado) | o alvo |
| `destination_outside_work_root` | exige aprovação | o destino de "mover" está fora da raiz de trabalho | o destino |
| `estimate_unknown` | exige aprovação | operação medida (ver abaixo) sem estimativa | a raiz de trabalho |
| `item_limit_exceeded` | exige aprovação | operação medida com mais itens que `approval_item_limit`, ou com esse limite em zero | a raiz de trabalho |
| `byte_limit_exceeded` | exige aprovação | operação medida com mais bytes que `approval_byte_limit`, ou com esse limite em zero | a raiz de trabalho |
| `quarantine_purge` | exige aprovação | apagar algo que já está na quarentena | o alvo |

Operação medida é apagar recursivo, limpar não rastreados e mover para dentro da pasta de quarentena. O mover entra na lista porque é nisso que um apagar vira ao ser reescrito, e o plano reescrito não diz se o apagar era recursivo: ele é tratado como o recursivo. Por isso o apagar simples, quando é reescrito para a quarentena, também é medido (com a quarentena desligada, não é).

O destino de "mover" é a pasta que recebe os alvos. O que o mover cria ou substitui é `destino/nome do alvo`, e essa entrada passa pelas regras de negar junto com a pasta receptora. Alvo com curinga não diz o nome: vale qualquer filho direto do destino que o curinga possa nomear (`*` e `?`, sem diferença de caixa), e esse filho é conferido contra as pastas reservadas e contra a pasta de **todos** os alvos do plano, a do próprio curinga inclusive. Por isso mover `tmp/*` e `out/a/*` para `out` é negado (o primeiro curinga pode criar `out/a`, sobre a qual o segundo age); mover `pasta/*` para a pasta que contém `pasta` é negado (o curinga pode nomear `pasta`); mover `pasta/*` para a raiz de trabalho com a quarentena ligada é negado (o curinga pode nomear a pasta da quarentena); e mover `pasta/*.log` não é. Na dúvida, o curinga pode nomear. Para alvo literal, a entrada é conferida nos dois sentidos: em cima de um alvo, dentro de um, ou contendo um.

**O veredito é fechado depois de julgar a reescrita.** Quando o apagar vira mover para a quarentena, o plano reescrito, com o destino gerado, passa por todas as regras de um "mover" pedido, antes de o veredito sair; o veredito guarda os motivos dos dois (cada um uma vez) e fica com o nível mais restritivo. Se o destino gerado for negado, o plano inteiro é negado e não é reescrito. Daí a propriedade que um teste cobra: reclassificar `veredito.plan()` dá o mesmo nível e não acha nada que o veredito já não traga.

O registro do plano cita toda regra que disparou, com o caminho, uma entrada por alvo. O evento `action.denied` cita só os códigos das regras que negam, cada um uma vez, na ordem em que dispararam; as que só pedem aprovação ficam no registro.

A chave `classification_match_steps` de [`defaults/action-gate.toml`](../../defaults/action-gate.toml)
limita a classificação de movimentos por passos determinísticos. O plano pedido e o movimento
gerado para quarentena usam o mesmo orçamento. A contagem inclui a indexação de nomes, as
comparações de prefixos de caminhos, o tamanho das entradas no destino e os passos do
casamento de curingas. O padrão comporta 10 mil nomes literais de 100 bytes em destinos comuns.
Ao esgotar o orçamento, `classify` devolve `ClassificationBudgetExceeded` antes de emitir um
veredito. A admissão recusa o plano sem gravar a ação ou emitir licença. Divida o lote ao
exceder o limite.

O diário reclassifica o plano gravado sob o teto vigente a cada consulta. Com um teto menor do
que o plano exige, `JournalGate::inspect` reconstrói o estado pelo histórico autenticado e
devolve `ClassificationBudgetExceeded` em `policy_warning`. Negar, expirar e registrar o
resultado de uma ação já iniciada (`Executed` ou `Fail`) continuam aceitos e gravados; a resposta
traz o mesmo aviso em `policy_warning`. Pedir aprovação, conceder a aprovação e iniciar
(`authorize`) devolvem o erro sem gravar nada e sem emitir licença. Para uma ação que ainda
aguarda aprovação ou início, suba o teto ou negue a ação e divida o plano.

Quarentena por padrão: apagar e apagar recursivo saem do veredito reescritos como "mover para `<raiz de trabalho>/<quarantine_dir>/<16 primeiros hexadecimais do hash do plano pedido>`". É o plano reescrito que recebe o `plan_hash`, a aprovação e a execução.

A política recusa na carga só o que dá para dizer sem a raiz de trabalho, no estilo da própria política (o da pasta do usuário):

- a forma do `quarantine_dir`: relativo, sem `..`, sem componente vazio ou `.` (com a quarentena ligada ou desligada);
- em política Windows, nome que lá não é pasta (`nul`, `com1`), ligada ou desligada;
- em política POSIX, com a quarentena ligada, o subcaminho que alguma raiz de trabalho diferente de `/` poria dentro de uma pasta listada: a pasta de sistema ou de perfis termina onde o subcaminho começa e tem algo antes (`posix_system_dirs = ['/srv/vault']` com `quarantine_dir = 'vault/q'`, para a raiz `/srv`).

Nome só compartilhado com pasta protegida do outro estilo ou do primeiro nível (`etc/q`, `home/q`, `Windows/q` numa raiz qualquer) é pasta comum e carrega. Se o destino gerado é protegido, quem diz é o julgamento da reescrita, com o estilo e a raiz de trabalho de cada plano.

### `plan_hash`

SHA-256 da [serialização canônica dos contratos](../../contracts/README.md#serialização-canônica-do-plan_hash), que este documento não repete: uma linha por campo, em ordem fixa, cada caminho precedido do seu tamanho em bytes, alvos na ordem canônica e sem repetição. A ordem dos alvos no pedido não muda o hash: permutar as mesmas entradas dá o mesmo hash, inclusive quando um alvo se repete no Windows com outra caixa. O hash é tomado sobre a grafia retida, a primeira das repetidas na ordem canônica ([identidade](#identidade-de-um-alvo-no-windows)); por isso `Build` com `build` fica `Build`, mas `build` com `build` fica `build` e dá outro hash. Trocar um alvo, a operação, a raiz, o destino, a estimativa ou a marca de raiz de repositório muda. O vetor de exemplo do contrato (o primeiro dourado válido de `record.action-plan`) é montado por um teste a partir de um plano e tem de dar o mesmo registro e o mesmo hash.

### Registro do plano

Todo veredito vira um `urn:ads:record:action-plan:2` (`Verdict::record`, devolvido por `Action::plan`; a `:1` é só lida, por `ActionPlanRecordV1`): a operação pedida, os alvos na ordem canônica, a raiz de trabalho, a estimativa, o veredito com os códigos, `quarantined` e `destination` (a pasta de quarentena quando o apagar foi reescrito; o destino pedido em "mover"). O registro só existe se cumprir o contrato inteiro, inclusive a ordem dos alvos e o `plan_hash` recalculado dos seus próprios campos; isso vale tanto para o que o núcleo monta quanto para o que ele lê. Caminho, curinga e quantidade de alvos acima dos limites do contrato já são recusados na normalização ([Normalização](#normalização)). O resto é conferido na montagem do registro e, se passa do limite, o plano fica sem registro e, por isso, não vira ação nem evento: a estimativa de itens ou de bytes acima de 2^53 − 1, que vem no pedido, e o que só aparece depois do julgamento, ou seja, mais regras disparadas do que `verdict.reasons` guarda e caminho gerado pelo julgamento (a pasta de quarentena, a entrada que o "mover" criaria citada como motivo) maior que o limite.

### Máquina de estados

```
veredito negado ─> negada

planejada ──(permitido)──────────────────────────────┐
    │                                                ├─ licença ─> executando ─> executada | falhou
    └─(exige aprovação)─> aprovação pedida ─> concedida ┘
                                │
                                └─> aprovação negada | expirada
```

| Transição | Evento | Stream |
|---|---|---|
| veredito negado | `action.denied:1` (o único evento da ação) | `action/<action_id>` |
| veredito permitido ou que exige aprovação | `action.planned:1` | `action/<action_id>` |
| pedir aprovação | `approval.requested:2`, com o `action_id` | `approval/<approval_id>` |
| conceder, negar, expirar | `approval.granted:2`, `approval.denied:2`, `approval.expired:2`, com o `action_id` | `approval/<approval_id>` |
| entregar a licença | `action.started:1`, com a aprovação usada ou `null` | `action/<action_id>` |
| concluir, falhar | `action.executed:1`, `action.failed:1` | `action/<action_id>` |

Toda transição produz exatamente um evento; `id`, instante, `seq`, autor, causa e classe do dado vêm de quem chama, porque o núcleo não tem relógio nem contador. O instante tem de existir no calendário (31 de fevereiro, 29 de fevereiro em ano não bissexto e segundo 60 fora de 23:59 são recusados, como no contrato). O `stream` de todo evento é o da entidade que o próprio `data` nomeia, na escrita e na leitura. A ação de um veredito negado nasce no estado final "negada": não aceita comando nem entrega licença.

Toda transição consome a ação (ela é passada por valor) e nem a ação nem a licença podem ser copiadas: dentro de um processo, uma aprovação rende uma licença e uma licença fecha uma execução, e o compilador recusa o contrário. Comando recusado devolve a ação como estava. As decisões de uma aprovação (conceder, negar, expirar) citam o `action_id` para o qual foram tomadas, no comando e no evento gravado: duas ações com o mesmo plano não dividem uma aprovação, nem em memória nem no log. O núcleo não guarda estado entre chamadas; **entre processos ou depois de uma queda, a unicidade é dever de quem persiste os eventos**: gravar `action.started` de forma atômica, recusando um segundo para a mesma ação, antes de entregar a licença ao executor, e dar cada `approval_id` a uma ação só. O `replay` recusa o log que tenha qualquer um dos dois repetido.

Cada evento é comparado com o anterior da mesma ação, desde o planejamento até o encerramento. Com o padrão zero, a ordem no caminho com aprovação é pedido ≤ concessão ≤ início ≤ fim. Instantes iguais valem (por exemplo, `05Z` e `05.000Z`). A chave `gate_clock_rewind_seconds` em [action-gate.toml](../../defaults/action-gate.toml) aceita de 0 a 60 segundos e mede os recuos com precisão de microssegundos. A chave da fila, `clock_rewind_seconds`, é recusada na configuração do portão. A tolerância é por transição e os recuos podem se acumular. Com 5 segundos, a concessão pode ficar 5 segundos antes do pedido e o início até 10 segundos antes do pedido. A validade da aprovação continua independente da tolerância.

O chamador que executa a operação deve carimbar o resultado com o maior instante entre o relógio e o início registrado. O daemon aplica esse piso tanto ao sucesso quanto à falha, preservando o resultado da operação se o relógio recuar durante a execução.

Um recuo acima do limite devolve `OutOfOrder` ao tentar anexar um evento, preserva o estado e permite repetir a transição com um carimbo dentro do limite. O diário reconstrói as transições na ordem autenticada da gravação. Se um recuo gravado ultrapassar a tolerância vigente, a consulta e o retorno de comandos bem-sucedidos sinalizam `HistoryOutOfOrder` em `policy_warning` quando não houver outro aviso de política. Ao tentar conceder uma aprovação ou iniciar a ação nessas condições, o diário grava `approval.denied` ou `action.failed`, respectivamente, com o código `E_CLOCK_POLICY_CHANGED` e devolve `HistoryOutOfOrder`. Isso permite ler diários antigos sem alteração no formato dos eventos. Fora do diário, `replay_with_clock_rewind` confere todos os carimbos com a tolerância informada.

São recusados, entre outros: licença sem a aprovação que o veredito exigia; plano, aprovação ou licença de outro hash; aprovação de outro `approval_id` ou tomada para outra ação; conceder ou negar com instante anterior ao do pedido; iniciar com instante anterior ao da concessão; conceder ou executar depois da validade; expirar antes dela; segunda licença para a mesma ação; qualquer comando em estado final.

`JournalGate` usa a porta `GateJournal` para gravar o plano e as transições e só devolve a licença depois de o diário confirmar `action.started`. Ao carregar a ação, confere identificadores de evento, sequências por stream e a correspondência do evento inicial com o plano autenticado. O estado é reconstruído com o veredito gravado. A política vigente julga planos novos e é conferida antes de pedir ou conceder aprovação e antes de iniciar uma ação. Se o veredito mudou, o portão devolve `PolicyChanged` depois de gravar `approval.denied` na concessão ou `action.failed` no início, com `E_POLICY_CHANGED`. É preciso planejar uma nova ação para usar a política vigente. A consulta expõe a divergência ou o esgotamento do orçamento de classificação em `policy_warning`. Após reduzir a tolerância, ainda é possível negar ou expirar uma aprovação pendente e registrar o sucesso ou a falha de uma ação iniciada. O novo evento segue a tolerância vigente e as regras de validade da aprovação. Um início já gravado restaura o estado `Executing` sem emitir outra licença. Cada transição passa por `GateJournal::transition`, que carrega o histórico, deixa o portão decidir o próximo evento e o grava; o padrão do traço é `load` seguido de `append`, e uma implementação pode fazer os três sob um só lock (o `EventLog` faz). A porta exige que a implementação autentique o histórico e compare sua extensão na mesma transação que grava a próxima transição. `StartedExecution` prova só a ordem (a licença nasce depois de o diário aceitar a gravação de `action.started`); a durabilidade é a da implementação de `GateJournal`, que é um traço público. No workspace, só o `EventLog` ancorado do armazenamento a garante.

O daemon registra os caminhos de repositório observados como alvos com `repo_root = true`. Ao conferir o veredito vigente, o diário usa esses mesmos alvos. A composição de arquivos existe como biblioteca Rust. Expor ações destrutivas pela API depende de resolver os pré-requisitos abertos ou registrar outra decisão.

### Pré-requisitos de execução da composição local

| Pré-requisito | Garantia e limite |
| --- | --- |
| Cópia de segurança | A quarentena preserva os itens movidos e grava o manifesto antes do primeiro movimento. Uma cópia independente e restaurável, inclusive antes de uma exclusão definitiva, ainda precisa de uma integração que a produza e verifique. O manifesto descreve os itens da leva. |
| Ensaio | `SessionGate::plan` ensaia planos que exigem aprovação e grava os efeitos ou a recusa. `SessionGate::execute` só emite `FileExecutionLicense` para uma ação aprovada se a resposta gravada identificar os efeitos do ensaio da mesma ação e do mesmo plano. Uma concessão antiga sem esse vínculo pode ser consultada e tem a execução recusada. Ações que a política permite sem aprovação continuam usando as verificações do executor. |
| Consentimento | `ApprovalDesk::answer` exige `LocalSession` e grava a resposta antes da concessão. Para executar, a composição exige que a concessão tenha sido causada por uma resposta local afirmativa gravada no diário. A autenticação prova acesso à sessão local, sem atestar a presença física de uma pessoa. |
| Nova conferência | `SessionGate::approved_effects` repete o ensaio antes de gravar o início. A licença leva os efeitos conferidos a `Executor::execute_approved`, que volta a compará-los antes da primeira alteração. `Disk` reabre e verifica as entradas e suas identidades antes de cada alteração. São verificações em disco, com as garantias de identidade do sistema de arquivos descritas no [executor](../adapters/fs/README.md). |
| Estimativa | `ActionPlan` mantém a estimativa declarada como opcional. `check_facts` mede as entradas, reaplica a política e recusa ultrapassar a estimativa presente em um plano aprovado. A licença de ação aprovada leva o inventário de efeitos observado, inclusive quando a estimativa está ausente. |
| Raiz de trabalho | `SessionGate::new` exige raiz configurada. `SessionGate::plan` usa a raiz da sessão e `JournalGate::for_work_root` confere a raiz registrada ao recarregar a ação. O executor confere a contenção em disco a partir de sua cerca. |
| Autor e canal | `ApprovalDesk` atribui o dispositivo configurado e o canal local ao gravar a resposta. A emissão da licença confere o contrato da resposta e a correspondência do ator e do canal com a concessão. Essa prova depende da autenticação local e da integridade do diário ancorado. Os campos de ator e canal do núcleo são metadados e não autenticam o dispositivo por si. |
| Assinatura remota | A composição de arquivos não recebe comandos remotos. A prova depende de integrar um transporte que autentique e verifique a assinatura do comando antes de entregá-lo ao portão. |

`StartedExecution` continua representando o aceite do início pelo diário. As interfaces de nível mais baixo em `ads-core` e `ads-adapter-fs` recebem a autorização do portão e confiam no chamador para fornecer as provas de integração acima. Na composição do daemon, apenas `FileExecutionLicense` encaminha a ação ao executor. O relógio que fornece os instantes continua sendo responsabilidade da composição.

### Estado a partir dos eventos

`replay` recebe os eventos de uma ação, na ordem em que foram gravados (os `action.*` do stream da ação e os `approval.*` da aprovação que o `approval.requested` amarra a ela), e devolve o estado: o mesmo que a máquina tem em memória, em todo passo de todo caminho. A máquina e o `replay` usam a mesma função `estado + evento -> estado`; não há segunda cópia das transições.

O `replay` recusa o que a máquina não escreveria: log vazio; primeiro evento que não seja `action.planned` nem `action.denied`; salto ou evento repetido; qualquer evento depois de um estado final; evento de outra ação, de outro `plan_hash` ou de outra aprovação, inclusive a decisão de aprovação tomada para outra ação de mesmo plano e mesmo `approval_id`; `action.started` sem a aprovação que o plano exige; evento fora da validade da aprovação; decisão com instante anterior ao do pedido e início com instante anterior ao da concessão; `action.planned` de risco `high` ou com elevação; e a versão `:1` dos quatro eventos de aprovação, que não diz de qual ação é. O resultado é só leitura: mostra o estado e não entrega licença.
