# ads-core

Política do produto (tarefas, aprovações, escopo, roteamento e orçamento), sem rede, banco nem PTY.

## Orçamento e roteamento

O [orçamento](src/budget.rs) avalia os quatro tetos do pacote contra o livro-caixa reconciliado da tarefa e o tempo decorrido passado pelo chamador. Conta uma tentativa por linha, inclusive falhas, e soma apenas USD faturável. Para a cota, soma a maior variação positiva entre as janelas de cada linha. Janelas sem par antes/depois e quedas de percentual contribuem zero. O padrão de aviso é 80%, configurável em [budget.toml](../../defaults/budget.toml). Atingir qualquer teto retorna as dimensões atingidas e a razão de cancelamento `budget`.

O [roteador](src/routing.rs) interpreta [routing.toml](../../defaults/routing.toml). Recebe tipo, tamanho e sensibilidade da tarefa, um catálogo com os modelos e esforços aceitos e as observações de cota do vínculo atual de cada ferramenta e provedor. Usa a primeira regra compatível, a ordem das faixas, o menor `rank` e depois a ordem lexicográfica dos identificadores e campos da rota. Cada candidato deve permitir explicitamente a classe da tarefa. A observação recente mais nova prevalece, com maior percentual em empate. Sem candidato elegível, a proposta fica vazia.

`shadow` é o padrão e a escolha manual configurada continua em vigor. `manual` só registra essa escolha, `propose` acrescenta a proposta e `auto` explícito retorna os dados da decisão calculada. O fallback em `shadow` e `propose` descreve a alternativa simulada. Quem integra o núcleo fornece o instante e os metadados do envelope e é responsável por publicar os eventos e aplicar decisões.

## Fila contínua

A [política da fila](src/queue.rs) restaura a fila parada e calcula transições usando o registro atual e, quando exigido, o aparelho autenticado. O aparelho autenticado leva o dono a que pertence e o canal (`Channel`, do próprio núcleo, convertido no `via` de cada evento). A política confere esse dono contra o da fila. A partida, o teto de custo e a falta de recursos param a fila com os dados de `queue.stopped` do daemon. Na partida, a restauração considera o retrato e o último estado registrado no log, e registra a parada se qualquer um dos dois indicar a fila ligada.

`change_queue_state` devolve o novo estado junto com os dados do evento (`queue.enabled` ou `queue.stopped` com `reason: owner`), montados do aparelho conferido. Uma mudança para o estado atual é recusada com `Unchanged`, então nunca sai parada para uma fila já parada. O produtor monta o envelope com `actor.id` igual ao `device_id` do aparelho devolvido.

Na partida, `restore_running_task` decide o destino da tarefa cujo último estado é `running`, em três casos:
- `Keep`: o host manteve o processo, e a tarefa continua em `running`. O núcleo volta a acompanhar a sessão, e a tentativa termina normalmente, mesmo com a fila parada.
- `Fail`: o host não manteve o processo, e a tarefa passa para `failed` com a mesma contagem, porque a tentativa interrompida já foi contada.
- `Cancel`: a contagem é inconsistente (zero ou acima do teto). A tarefa passa para `cancelled` com a contagem gravada, e o chamador encerra o processo se o host o manteve.

O replay (`replay_task`) recebe o stream inteiro da fila, a partir de `queue.created` com `seq` 1 e sem buraco, e uma consulta ao cadastro de aparelhos no instante de cada evento. Ele devolve o estado e o saldo, ou o motivo da recusa (`ReplayRefusal`). Além da ordem e do saldo, ele confere:
- todo evento é da fila e do stream `queue/<id>`;
- todo ator que é aparelho pertencia ao dono da fila naquele instante;
- a criação, a ativação e a parada pelo dono vêm de um aparelho do dono e repetem o dono da fila;
- todo pedido manual, de qualquer tarefa da fila, foi criado por um aparelho do dono;
- a tarefa só entra em `running` com a fila ligada naquele ponto do log;
- `review → done` foi aceito por um aparelho do dono.

O agente e o daemon continuam podendo reprovar e cancelar tarefas. O replay confere a coerência dos atores que o log declara, não a autenticidade deles: o log não é assinado por aparelho, e uma linha forjada é recusada pela cadeia HMAC do armazenamento ([ADR 0021](../../docs/adr/0021-storage-atomic-file-and-cipher.md)). O replay protege contra erro do produtor e contra log montado de forma errada.

O replay não refaz a admissão: o teto de tentativas de cada criação vale como gravado, e as janelas não são reaplicadas. A política em vigor e a âncora de relógio de cada criação não estão no log. Conferir criações antigas contra a política atual recusaria um histórico legítimo depois que o dono baixasse um limite. Quem decide a admissão é o produtor.

O cadastro responde pelo dono do aparelho em cada instante. Um aparelho mantém o dono que tinha quando agiu, e a revogação é uma marca com data: ela recusa os atos posteriores sem apagar os anteriores. O produtor usa o estado e o saldo reconstituídos antes de autorizar novas transições e reserva a origem na mesma operação que anexa a criação.

[queue.toml](../../defaults/queue.toml) fornece os limites. O integrador pode passar uma política diferente com `QueuePolicy`. Chave ausente num arquivo anterior assume o valor de `queue.toml`. A admissão (`can_admit_task`) recebe a política, o relógio, o dono da fila, o aparelho que pediu e a âncora de relógio em `AdmissionContext`. Ela devolve o motivo da recusa (`AdmissionRefusal`), para a interface poder explicá-lo. A admissão recusa:
- criação com teto de tentativas acima do configurado;
- pedido manual que não foi criado por um aparelho do dono da fila, com `actor.id` igual ao do aparelho;
- `task_id` já usado em qualquer fila;
- origem repetida na fila;
- a fila que atingiu o total de admissões (`admission_limit`, 1000), que inclui origens encerradas;
- na janela de `window_seconds` (uma hora) que termina no `ts` da criação:
  - mais de `manual_window_admissions` (10) pedidos manuais;
  - mais de `window_admissions` (10) tarefas vindas de issue ou de regra de bug;
  - mais de `bug_rule_window_admissions` (3) da mesma regra de bug.

Os pedidos manuais contam à parte, para que as origens automáticas não ocupem o espaço do dono.

O `ts` da criação vem do relógio do núcleo, nunca do pedido, e o produtor informa esse relógio em `now`. A admissão recusa a criação com `ts` mais de `clock_skew_seconds` (5) adiante de `now` (`AheadOfClock`) ou mais de `clock_rewind_seconds` (300) antes de uma admissão anterior da fila (`BehindLastAdmission`). Assim, um carimbo adiantado em relação ao relógio do núcleo não esvazia a janela, e um ajuste pequeno do relógio para trás não trava a fila. Uma admissão anterior com `ts` depois do da candidata, dentro da tolerância, continua contando na janela.

Se o próprio relógio do núcleo salta para a frente, `ts` e `now` andam juntos: a candidata passa e a janela se esvazia uma vez. Quando o relógio é corrigido, toda admissão seguinte fica além da tolerância e é recusada com `BehindLastAdmission`, inclusive os pedidos manuais do dono, até o tempo real alcançar a última admissão. A saída é o dono reancorar o relógio da fila. `reanchor_admission_clock` recebe o aparelho do dono, o relógio do núcleo no momento do ato (com o mesmo contrato do `ts` da criação), o `seq` sobre o qual o dono agiu e o último `seq` do stream lido do armazenamento. Os dois `seq` têm de ser iguais (`StaleSeq`). A função devolve a âncora (`ClockAnchor`). Na admissão, as criações até esse `seq` com `ts` posterior à âncora e ainda adiante do relógio contam como admitidas no instante da âncora. Elas continuam reservando a origem e ocupando a janela que começa ali. As criações que o relógio já alcançou não se movem, então nem uma âncora antiga tira uma criação comum da janela. A admissão recusa a âncora que cobre o `seq` da candidata ou um posterior (`AnchorAfterCandidate`), porque ela moveria admissões feitas depois do ato do dono. Também recusa a âncora com instante mais de `clock_skew_seconds` adiante do relógio (`AnchorAheadOfClock`). A âncora não tem construtor público fora dessa função. Gravar a âncora no log pede um evento novo nos contratos. Até ele existir, a função é pura e o produtor guarda a âncora, que se perde se ele não a mantiver.

Os limites da janela se liberam sozinhos com o tempo. Uma fila que atingiu o total só volta a admitir quando o dono aumenta `admission_limit`. Criar outra fila também resolve, mas a chave de deduplicação inclui a fila: a fila nova não herda as origens reservadas, e a mesma issue pode ser admitida de novo.

`AuthenticatedDevice` não tem construtor público. Toda função pública deste crate pode ser chamada por qualquer crate que dependa dele, e a matriz de dependências deixa a camada de autenticação fora do núcleo. Por isso nenhum construtor aqui distingue a camada de autenticação de um chamador que copia o id do aparelho de um pedido. O construtor entra junto com essa camada e com o cadastro de aparelhos.

## Ambiente do processo filho (`src/child_env.rs`)

Política pura do ambiente que um processo de ferramenta recebe: só passam os nomes da lista permitida que o processo pai tem, escritos como a lista os escreve, mais os valores que o produto define (`TERM`, `COLORTERM`). Nome em `allow` ou em `set` que casa com um padrão negado (`NOME`, `PREFIXO*`, `*SUFIXO`, `*TRECHO*`, sem diferença de maiúsculas) recusa a política na carga; o filtro confere de novo e o descarta. A exceção são os nomes inofensivos listados pelo produto para um padrão em `[env.deny_exceptions]` (por exemplo `WT_SESSION` para `*_SESSION*`); a camada do usuário pode negá-los, mas não criar exceções. Nomes repetidos (pela comparação do sistema: no Windows também letras fora do ASCII, `ÄX` e `äx`), nomes com `=`, `*` ou controle e valores de `set` com NUL ou controle também recusam. Os erros dizem a lista e a posição, nunca o texto recusado. Padrões de nome não reconhecem um segredo sob um nome qualquer: a garantia é que nada de nome reconhecidamente sensível passa e que o que passa está todo em `allow`. Os padrões ficam em [`defaults/harnesses.toml`](../../defaults/harnesses.toml) (`[env]`): a camada do usuário substitui `allow` e `set` e só acrescenta a `deny`. A comparação com `allow` ignora maiúsculas no Windows (`NameCase::Insensitive`); com dois nomes do pai que só diferem na caixa, vale o escrito igual à lista, senão o menor. O núcleo não lê o ambiente: quem chama o passa como dado.

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
| `classify.rs` | As regras e os seus códigos: `plano + política -> veredito` (permitido, exige aprovação, negado), a reescrita de apagar como mover para a quarentena e o registro do plano julgado. |
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
- o que não cabe no registro do plano: caminho normalizado com mais caracteres que `urn:ads:common:abspath:2` (erro `TooLong`), curinga maior que o `pattern` do registro (`PatternTooLong`) e mais alvos, depois de juntar os repetidos, do que o registro guarda (`TooManyTargets`). Os limites vêm das constantes geradas do contrato (`MAX_CHARS`, `MAX_ITEMS` em `ads-contracts`), não de número copiado.

Alvo com curinga é julgado como a pasta sobre a qual o curinga age: `pasta/*` vale como `pasta`.

O Schema `urn:ads:common:abspath:2` recusa o mesmo que esta normalização recusa num caminho já normalizado; um corpus comum (`contracts/corpus/abspath.json`) roda pelos dois e um teste de cada lado cobra o mesmo veredito ([contracts/README.md](../../contracts/README.md#caminho-absoluto-o-schema-e-o-núcleo)). Tudo aqui é comparação de texto: duas grafias Unicode do mesmo nome (NFC e NFD, largura total) são nomes diferentes. Para permitir isso é o lado seguro (a outra grafia fica de fora); para negar é um limite: uma pasta protegida escrita na outra forma de normalização não é reconhecida só pelo texto, e quem executa tem de conferir no disco.

### Regras

Um alvo negado nega o plano inteiro. Comparação por componente (`proj2` não está dentro de `proj`), com duas medidas de "mesmo nome":

- **para permitir** ("está dentro da raiz de trabalho ou de uma raiz permitida", para alvo e para destino): no Windows só a caixa das letras ASCII é ignorada; nos demais, nada. Nomes que só uma dobra de caixa Unicode igualaria (`K` e o sinal Kelvin, `ß` e `SS`) são pastas diferentes e ficam de fora;
- **para negar** (pasta protegida, raiz de trabalho, raiz permitida, quarentena): a comparação mais larga, nos dois estilos, igualando o que qualquer mapeamento de caixa Unicode iguala. Larga de propósito: só serve para negar, nunca para permitir.

| Veredito | Quando |
|---|---|
| Negado | alvo que é raiz de unidade ou `/`; a pasta do usuário ou um ancestral dela; a pasta dos perfis ou o perfil de qualquer usuário; pasta do sistema (ou algo dentro dela); a raiz de trabalho ou algo que a contenha; uma raiz permitida ou algo que a contenha, mesmo que outra raiz a contenha; qualquer coisa que não esteja estritamente dentro da raiz de trabalho ou de uma raiz permitida; raiz de repositório (informada no plano); a pasta de quarentena ou algo que a contenha |
| Negado | qualquer caminho escrito no estilo que não é o da pasta do usuário (`/c/...` contra `C:\...`): pode ser outra grafia de uma pasta protegida, e o texto sozinho não resolve isso |
| Negado | raiz de trabalho que é ela mesma uma pasta protegida; destino de "mover" protegido, fora das raízes permitidas ou dentro de um dos alvos; entrada que o "mover" criaria no destino (`destino/nome do alvo`) protegida, em cima de um dos alvos, ou que é a raiz de trabalho, uma raiz permitida ou a pasta de quarentena (ou contém uma delas) |
| Exige aprovação | apagar recursivo, limpar não rastreados, mover para dentro da quarentena ou apagar simples reescrito para a quarentena, acima do limite de itens ou de bytes, ou sem estimativa (limite zero pede aprovação sempre, qualquer que seja a estimativa); qualquer ação (sobrescrita inclusive) em raiz permitida fora da raiz de trabalho; "mover" com destino fora da raiz de trabalho; apagar algo que já está na quarentena |
| Permitido | o restante dentro da raiz de trabalho |

Os três níveis casam um a um com `urn:ads:common:risk:1`: permitido é `low`, exige aprovação é `medium`, negado é `high`.

#### Códigos de regra

Cada regra tem um código estável (`urn:ads:common:action-rule:1`), que vai no evento `action.denied` e no registro do plano. Um código nunca muda de sentido nem é reaproveitado; regra nova ganha código novo. A lista é a mesma do [`contracts/README.md`](../../contracts/README.md#registro-do-plano), e um teste reprova se alguma das duas ficar para trás.

`<proteção>` é o motivo de a pasta ser protegida: `filesystem_root` (raiz de unidade ou `/`), `user_home` (a pasta do usuário), `user_home_ancestor` (um ancestral dela), `user_profile` (a pasta dos perfis ou o perfil de qualquer usuário), `system_dir` (pasta do sistema) e `other_path_style` (caminho no estilo que não é o da pasta do usuário).

| Código | Veredito | Dispara quando | Caminho citado |
|---|---|---|---|
| `protected.<proteção>` | negado | o alvo é uma pasta protegida | o alvo |
| `work_root_protected.<proteção>` | negado | a raiz de trabalho é ela mesma uma pasta protegida | a raiz de trabalho |
| `work_root_itself` | negado | o alvo é a raiz de trabalho ou algo que a contém | o alvo |
| `allowed_root_itself` | negado | o alvo é uma raiz permitida ou algo que a contém, ainda que esteja dentro de outra raiz | o alvo |
| `outside_allowed_roots` | negado | o alvo não está estritamente dentro da raiz de trabalho nem de uma raiz permitida | o alvo |
| `repository_root` | negado | o alvo é raiz de repositório (informada no plano) | o alvo |
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

Quarentena por padrão: apagar e apagar recursivo saem do veredito reescritos como "mover para `<raiz de trabalho>/<quarantine_dir>/<16 primeiros hexadecimais do hash do plano pedido>`". É o plano reescrito que recebe o `plan_hash`, a aprovação e a execução.

A política recusa na carga só o que dá para dizer sem a raiz de trabalho, no estilo da própria política (o da pasta do usuário):

- a forma do `quarantine_dir`: relativo, sem `..`, sem componente vazio ou `.` (com a quarentena ligada ou desligada);
- em política Windows, nome que lá não é pasta (`nul`, `com1`), ligada ou desligada;
- em política POSIX, com a quarentena ligada, o subcaminho que alguma raiz de trabalho diferente de `/` poria dentro de uma pasta listada: a pasta de sistema ou de perfis termina onde o subcaminho começa e tem algo antes (`posix_system_dirs = ['/srv/vault']` com `quarantine_dir = 'vault/q'`, para a raiz `/srv`).

Nome só compartilhado com pasta protegida do outro estilo ou do primeiro nível (`etc/q`, `home/q`, `Windows/q` numa raiz qualquer) é pasta comum e carrega. Se o destino gerado é protegido, quem diz é o julgamento da reescrita, com o estilo e a raiz de trabalho de cada plano.

### `plan_hash`

SHA-256 da [serialização canônica dos contratos](../../contracts/README.md#serialização-canônica-do-plan_hash), que este documento não repete: uma linha por campo, em ordem fixa, cada caminho precedido do seu tamanho em bytes, alvos na ordem canônica e sem repetição. A ordem e a grafia dos alvos no pedido não mudam o hash; trocar um alvo, a operação, a raiz, o destino, a estimativa ou a marca de raiz de repositório muda. O vetor de exemplo do contrato (o primeiro dourado válido de `record.action-plan`) é montado por um teste a partir de um plano e tem de dar o mesmo registro e o mesmo hash.

### Registro do plano

Todo veredito vira um `urn:ads:record:action-plan:2` (`Verdict::record`, devolvido por `Action::plan`; a `:1` é só lida, por `ActionPlanRecordV1`): a operação pedida, os alvos na ordem canônica, a raiz de trabalho, a estimativa, o veredito com os códigos, `quarantined` e `destination` (a pasta de quarentena quando o apagar foi reescrito; o destino pedido em "mover"). O registro só existe se cumprir o contrato inteiro, inclusive a ordem dos alvos e o `plan_hash` recalculado dos seus próprios campos; isso vale tanto para o que o núcleo monta quanto para o que ele lê. Caminho, curinga e quantidade de alvos acima dos limites do contrato já são recusados na normalização ([Normalização](#normalização)). O que só aparece depois do julgamento continua sem registro e, por isso, não vira ação nem evento: mais regras disparadas do que `verdict.reasons` guarda, estimativa de itens ou de bytes acima de 2^53 − 1, e caminho gerado pelo julgamento (a pasta de quarentena, a entrada que o "mover" criaria citada como motivo) maior que o limite.

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

O estado guarda o instante do pedido de aprovação (`requested_at`) e o da concessão (`granted_at`), e a ordem é cobrada: pedido ≤ concessão ≤ início, e pedido ≤ negação, tudo dentro da validade. Instante igual vale (comparado como instante: `05Z` e `05.000Z` são o mesmo). O núcleo não tem relógio: confere a ordem do que recebe, e relógio confiável é dever de quem chama. O instante dos eventos que fecham a ação (`action.executed`, `action.failed`) não é comparado com o do início, para que uma ação iniciada sempre possa ser fechada.

São recusados, entre outros: licença sem a aprovação que o veredito exigia; plano, aprovação ou licença de outro hash; aprovação de outro `approval_id` ou tomada para outra ação; conceder ou negar com instante anterior ao do pedido; iniciar com instante anterior ao da concessão; conceder ou executar depois da validade; expirar antes dela; segunda licença para a mesma ação; qualquer comando em estado final.

### Estado a partir dos eventos

`replay` recebe os eventos de uma ação, na ordem em que foram gravados (os `action.*` do stream da ação e os `approval.*` da aprovação que o `approval.requested` amarra a ela), e devolve o estado: o mesmo que a máquina tem em memória, em todo passo de todo caminho. A máquina e o `replay` usam a mesma função `estado + evento -> estado`; não há segunda cópia das transições.

O `replay` recusa o que a máquina não escreveria: log vazio; primeiro evento que não seja `action.planned` nem `action.denied`; salto ou evento repetido; qualquer evento depois de um estado final; evento de outra ação, de outro `plan_hash` ou de outra aprovação, inclusive a decisão de aprovação tomada para outra ação de mesmo plano e mesmo `approval_id`; `action.started` sem a aprovação que o plano exige; evento fora da validade da aprovação; decisão com instante anterior ao do pedido e início com instante anterior ao da concessão; `action.planned` de risco `high` ou com elevação; e a versão `:1` dos quatro eventos de aprovação, que não diz de qual ação é. O resultado é só leitura: mostra o estado e não entrega licença.
