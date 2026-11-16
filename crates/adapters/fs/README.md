# ads-adapter-fs

Executa no sistema de arquivos, dentro de uma cerca, as ações que o portão do núcleo licenciou.

O [portão](../../core/README.md#portão-de-ações-destrutivas-srcgate) decide com texto e não toca o disco. Este crate é onde o texto encontra o disco: é o único do workspace que move ou apaga arquivos do usuário, e só faz isso com a licença de execução do portão na mão.

| Arquivo | O que faz |
|---|---|
| `sys/` | As chamadas do sistema, sempre por identificador aberto: abrir um filho por um nome só, relativo a uma pasta aberta, sem seguir link; identidade, tipo e montagem lidos do que foi aberto; criar, mover sem substituir e apagar. É o único lugar com chamada do sistema e, no Windows, com `unsafe`. |
| `fence.rs` | A cerca, aberta uma vez, e a resolução: cada caminho vira uma lista de nomes abaixo dela, abertos um a um. |
| `survey.rs` | Os fatos reais de um alvo (itens, bytes, repositório) e o inventário: qual objeto está sob cada nome da árvore. Só leitura. |
| `prepare.rs` | Tudo que é conferido antes da primeira alteração. O ensaio é isto e nada mais. |
| `disk.rs` | O único lugar que altera o disco: operações de uma entrada só, cada uma alcançada de novo a partir da cerca e feita sobre o que está aberto. |
| `manifest.rs` | O manifesto e o desenho de uma leva de quarentena. |
| `journal.rs` | O traço do diário: a ordem com o log e o uso único da licença. |
| `executor.rs` | A entrada: `execute`, `rehearse` e `measure`. |
| `reason.rs`, `report.rs` | Os motivos de recusa com os códigos de `action.failed`, e o relatório. |
| `limits.rs` | Parâmetros; os padrões ficam só em [`defaults/fs-executor.toml`](../../../defaults/fs-executor.toml). |

## Entrada

```
Executor::new(cerca, política)            // resolve e abre a cerca uma vez; recusa raiz de unidade, `/`, caminho de rede e sistema sem suporte
executor.execute(licença, action.started, diário) -> Completed | Failure { motivo, etapa, relatório }
executor.rehearse(&licença) -> relatório  // ensaio: todas as conferências, nenhuma alteração
executor.measure(raiz de trabalho, alvo)  // fatos reais, para o plano nascer com dado e não com palpite
```

- `execute` consome a licença (`ExecutionPermit`), que só `Action::authorize` constrói, e **não a devolve**. `Completed` guarda a licença gasta fora do alcance de quem chamou; `Completed::close(ação, meta)` a entrega direto ao portão (`Command::Executed`) e devolve a ação fechada, o evento `action.executed` e o relatório. Aceita ou recusada pelo portão, a licença não volta. Na falha, `motivo.error_code()` vai em `Command::Fail`.
- **Ordem com o log e uso único.** Nada é lido nem alterado antes de o diário (`Journal::consume`) registrar que a licença daquele `action.started` foi gasta. O evento tem de ser o da licença (mesma ação, mesmo hash, mesma aprovação). O contrato do traço: devolver `Ok` no máximo uma vez por ação, para sempre, com registro durável, atômico, compartilhado entre executores e que sobrevive a reinício. Este crate não traz nenhum diário: o de memória mora só nos testes (`tests/common`), porque esquece tudo ao reiniciar. Como segunda barreira, cada `Executor` lembra as ações de que já recebeu licença e recusa a segunda (`E_FS_PERMIT_REUSED`), mesmo com outro diário; essa memória acaba com o processo.
- **A política é a de quem constrói o executor**, não a do plano: o plano da licença é julgado de novo com ela antes de qualquer coisa.
- **Sistemas.** Linux e Windows. Em qualquer outro, `Executor::new` recusa (`UnsupportedSystem`): faltam as chamadas de que a segurança depende, e nada é simulado por caminho.

## Identificadores abertos, não caminhos

Um caminho é resolvido de novo pelo sistema a cada chamada. Conferir um caminho e depois usá-lo deixa uma janela: quem troca uma pasta por um link nesse intervalo redireciona a operação. Por isso o executor não entrega caminho ao sistema:

- A cerca é resolvida e aberta **uma vez**, na construção. É o único caminho do crate.
- Todo o resto é aberto por **um nome só**, relativo à pasta aberta antes (`openat` com `O_NOFOLLOW` no Linux; `NtCreateFile` com pasta raiz e `FILE_OPEN_REPARSE_POINT` no Windows). Um link é aberto como link, nunca seguido. Nome com separador, `.`, `..` ou fluxo alternativo (`:`) não é nome.
- O que vale é a **identidade** do que foi aberto (dispositivo e inode; no Windows, volume e identificador de arquivo de 128 bits), lida do próprio identificador. A grafia do caminho só serve para chegar lá.
- Criar, mover e apagar são feitos **sobre o que está aberto**: criação relativa à pasta aberta do destino; mover e apagar pelo identificador da própria entrada no Windows (`NtSetInformationFile`), e por nome dentro da pasta aberta no Linux (`renameat2`, `unlinkat`), com a identidade conferida logo antes.

Trocar um componente do caminho por um link depois da última conferência não redireciona mais nada: a operação acontece no objeto aprovado, ou falha.

## O que é conferido antes da primeira alteração

1. O plano, julgado de novo pela política confiável, não pode sair negado, nem como um apagar que a política manda para a quarentena, nem exigindo uma aprovação que a licença não traz.
2. A raiz de trabalho tem de estar dentro da cerca (ou ser ela) e ser uma pasta de verdade. Cada alvo tem de estar estritamente dentro da raiz de trabalho. A comparação com a cerca é letra por letra: outra caixa de um nome da cerca pode ser outra pasta.
3. **Resolução.** Cada nome é aberto a partir da cerca. Link simbólico, junção ou qualquer ponto de reanálise no meio do caminho é recusa; no último nome é "o alvo é um link", e o que ele aponta nunca é olhado. No Windows o nome tem de ser exatamente o que a pasta lista: outra caixa ou o nome curto (8.3) é recusa, então o texto que o portão julgou é a entrada que será aberta, também em pasta sensível a maiúsculas.
4. **Volume e montagem.** Entrada em outro volume é recusa. No Linux, entrada em outra montagem também, inclusive montagem do mesmo dispositivo (bind mount): a identificação de montagem e a marca de raiz de montagem vêm do `statx`; kernel ou sistema de arquivos que não informa as duas recusa tudo. No Windows, volume montado em pasta é ponto de reanálise: visto como link, nunca atravessado.
5. **Fatos e inventário.** O executor percorre a árvore de cada alvo por identificadores abertos (sem seguir link; um link conta um), conta itens e bytes com soma verificada (estouro é recusa) e registra o inventário: a identidade e o tipo de cada entrada. O plano é julgado mais uma vez com os fatos reais. Veredito mais duro que o declarado é recusa; com aprovação, mais itens ou mais bytes do que a estimativa aprovada também.
6. **Repositórios.** `.git` no nível do alvo é o fato "raiz de repositório", julgado pela política. `.git` (pasta, ou o arquivo de uma worktree) em qualquer profundidade abaixo disso é recusa (`E_FS_NESTED_REPOSITORY`), para apagar e para quarentena. O nome é comparado sem distinguir caixa: `.GIT` conta.
7. **Onde cada alvo cai de verdade.** O veredito julga a pasta que recebe os alvos; o executor julga o lugar final de cada um: estritamente dentro da raiz de trabalho, fora de pasta protegida pela política e, no disco, pela mesma abertura sem link.
8. **Tamanho do manifesto.** O manifesto da leva é montado e medido antes de qualquer alteração; maior que `max_manifest_bytes` é recusa (`E_FS_MANIFEST_TOO_LARGE`): não se cria leva que o executor não saberia restaurar.
9. Alvos sobrepostos, curinga, sobrescrever, limpar não rastreados e mover que não seja para a quarentena nem restauração: recusa. Ver [o que não é executado](#o-que-não-é-executado).

Qualquer recusa aqui acontece com o disco intacto (`Failure.stage == Checks`).

## Alterações

Só `disk.rs` altera o disco: criar pasta (falha se existe), criar arquivo novo (falha se existe), mover, remover uma entrada e remover uma árvore, que é a remoção de uma entrada repetida das folhas para a raiz. Não há exclusão nem criação recursiva da biblioteca. Cada operação alcança a entrada de novo a partir da cerca e confere: estritamente abaixo do piso (a raiz de trabalho; na purga, a pasta de quarentena), nenhum link no caminho e, na entrada, **o mesmo objeto do inventário**.

- **Só o que foi aprovado.** Um arquivo trocado por outro com o mesmo nome é recusa (`E_FS_CHANGED`). Ao apagar uma árvore, cada pasta tem de conter exatamente os nomes do inventário, e cada entrada tem de ser o objeto do inventário: entrada nova não é apagada, e a pasta onde ela está fica. Ao mover, a árvore inteira é comparada com o inventário logo antes.
- **Mover nunca substitui.** A recusa de destino existente é do próprio sistema, na mesma chamada que move (`RENAME_NOREPLACE`; no Windows, renomear sem substituição). Sistema de arquivos sem essa primitiva recusa o movimento; nada é simulado com "conferir e depois mover".
- **Durabilidade.** Depois de cada criação, movimento ou remoção de alvo, a pasta envolvida é sincronizada (as duas, no movimento). O manifesto é sincronizado como arquivo e na pasta da leva antes do primeiro movimento. Um passo só conta como feito depois disso; se a alteração foi feita e a sincronização falhou, o passo sai como não confirmado (`E_FS_UNCONFIRMED`), nunca como feito.

A sonda (`Probe`) é avisada de cada alteração duas vezes: antes de qualquer abertura (`before`) e depois da última conferência, com os identificadores abertos, logo antes da chamada do sistema (`opened`). É por ela que os testes param a execução em qualquer passo e mexem no disco em volta de uma conferência.

### Quarentena

Mover para dentro de `<raiz de trabalho>/<quarantine_dir>` é quarentena. O destino vem do plano (`.../<16 hexadecimais>`); o executor cria dentro dele a pasta da ação, de forma exclusiva:

```
<destino>/<action_id>/manifest.json
<destino>/<action_id>/items/<caminho do alvo abaixo da raiz de trabalho>
```

- A pasta `<action_id>` é criada com falha se existir: dois pedidos iguais ficam em pastas separadas, e a repetição da mesma ação é recusada sem tocar em nada, mesmo por um executor e um diário que a esqueceram.
- A estrutura abaixo de `items` é a do alvo abaixo da raiz de trabalho: `a/build` e `b/build` não colidem.
- O manifesto lista a leva inteira: caminho original de cada item, `plan_hash`, `action_id` e o instante de `action.started`. O que está de fato na quarentena é o que existe em `items`.
- Um `action_id` que não serve de nome de pasta (com `:`, terminado em ponto, nome de dispositivo) é recusado.

### Restauração

Restaurar é uma ação como as outras, com licença: `fs.move` de **uma** pasta de leva para a raiz de trabalho. Cada item do manifesto que ainda está em `items` volta ao lugar de origem; as pastas de origem que sumiram são recriadas uma a uma. Lugar de origem ocupado recusa a restauração inteira antes do primeiro movimento, e o que aparecer no lugar depois dessa conferência não é substituído. Item que não está mais na quarentena aparece no relatório como ausente. Sem nenhum item presente, recusa.

O manifesto é lido pela pasta aberta da leva, do arquivo que foi aberto: link não é seguido, tem de ser arquivo regular e a leitura para em `max_manifest_bytes`. Lido do disco é dado, não instrução: versão, identificadores e cada nome são validados; o caminho original tem de ser exatamente a raiz de trabalho mais o lugar do item em `items`; a ação do manifesto tem de ser a da pasta.

### Apagar de verdade

Só existe em dois casos: esvaziar a quarentena (todos os alvos dentro dela, a regra `quarantine_purge` disparada e a aprovação presente na licença) e política com `quarantine_enabled = false`. Fora disso, um `fs.delete` ou `fs.delete_recursive` que chegue sem ter sido reescrito é recusado. `fs.delete` aceita arquivo, link ou pasta vazia. Arquivo somente leitura sai como qualquer outro.

### Falha no meio e recuperação

O executor para na primeira falha: não desfaz e não continua. `Failure.stage` diz até onde foi, e o relatório traz primeiro a infraestrutura (`staging`: cada pasta e o manifesto, com "pendente", "já existia", "criado", "falhou" ou "não confirmado") e depois um passo por alvo (pendente, feito, ausente, falhou, não confirmado, fora do lugar; no apagar recursivo, quantas entradas já tinham saído). `Report::untouched()` responde se alguma coisa mudou no disco.

| Etapa | O que mudou no disco |
|---|---|
| `Checks` | nada |
| `Staging` | nenhum alvo; as pastas e o manifesto que o relatório marca como criados (ou não confirmados) existem |
| `Steps` | os alvos marcados como feitos ou não confirmados, e a parte do alvo que falhou que o relatório informa |

O que uma nova execução encontra depois de uma leva interrompida:

- **A mesma ação nunca roda de novo**: o diário recusa, e a pasta da ação já existe.
- **Parou antes de o manifesto estar gravado e sincronizado**: nenhum alvo saiu do lugar (a ordem garante). A pasta da leva fica, sem itens; uma restauração dela é recusada (`E_FS_MANIFEST_INVALID`) e ela sai pela purga da quarentena.
- **Parou depois**: cada alvo está inteiro em exatamente um lugar, o de origem ou `items`. A restauração da leva devolve o que foi e marca o resto como ausente; pode ser repetida até terminar.
- **Uma restauração interrompida** é retomada por outra restauração da mesma leva.
- Uma ação nova para os mesmos alvos ganha a própria pasta e não é afetada.

Os testes param a execução em cada chamada da sonda (criações, gravação do manifesto, movimentos e sincronizações) e conferem esse estado. Queda de energia de verdade não é simulada: o que se garante é a ordem das sincronizações.

O evento `action.failed` leva só o código (é o contrato); o relatório é devolvido a quem chamou.

## Códigos de `action.failed`

Um código nunca muda de sentido; motivo novo ganha código novo.

| Código | Quando |
|---|---|
| `E_FS_STARTED_MISMATCH` | o evento entregue não é o `action.started` da licença |
| `E_FS_JOURNAL_REFUSED` | o diário não registrou o consumo da licença |
| `E_FS_PERMIT_REUSED` | este executor já recebeu uma licença da mesma ação |
| `E_FS_PATH_STYLE` | caminho no estilo de outro sistema |
| `E_FS_OUTSIDE_FENCE` | caminho fora da cerca |
| `E_FS_OUTSIDE_WORK_ROOT` | alvo, destino ou entrada que não está estritamente dentro da raiz de trabalho |
| `E_FS_LINK_IN_PATH` | link simbólico, junção ou ponto de reanálise no caminho |
| `E_FS_NOT_PHYSICAL` | o nome não é o que a pasta lista (outra caixa, nome curto) ou não é um nome |
| `E_FS_OTHER_VOLUME` | o caminho entra em outro volume |
| `E_FS_MOUNT_POINT` | o caminho entra em outra montagem, ou o sistema não informa a montagem |
| `E_FS_MISSING` | o alvo, ou uma pasta no caminho, não existe |
| `E_FS_NOT_A_DIRECTORY` | algo que não é pasta onde tinha de haver uma |
| `E_FS_SPECIAL_FILE` | dispositivo, soquete ou pipe |
| `E_FS_UNSUPPORTED` | operação, ou forma dela, que este adaptador não executa |
| `E_FS_OVERLAPPING_TARGETS` | um alvo dentro de outro |
| `E_FS_POLICY_DENIES` | a política confiável nega o plano da licença |
| `E_FS_APPROVAL_MISSING` | a política confiável exige aprovação e a licença não traz |
| `E_FS_QUARANTINE_BYPASSED` | apagar de verdade o que a política manda para a quarentena |
| `E_FS_FACTS_CONTRADICT_PLAN` | o disco contradiz o declarado de um jeito que muda o veredito |
| `E_FS_NESTED_REPOSITORY` | a árvore de um alvo contém um repositório |
| `E_FS_OVERFLOW` | uma contagem de itens ou de bytes não cabe em 64 bits |
| `E_FS_ACTION_ID_UNUSABLE` | o `action_id` não serve de nome de pasta |
| `E_FS_DESTINATION_TAKEN` | a pasta de quarentena da ação, ou um lugar dentro dela, já existe |
| `E_FS_NOT_EMPTY` | `fs.delete` de pasta com conteúdo |
| `E_FS_MANIFEST_INVALID` | manifesto ausente, que não é arquivo, grande demais, malformado ou que não casa com a pasta |
| `E_FS_MANIFEST_TOO_LARGE` | o manifesto da leva passaria do limite que o executor lê de volta |
| `E_FS_RESTORE_PLACE_TAKEN` | o lugar de origem de um item está ocupado |
| `E_FS_NOTHING_TO_RESTORE` | nenhum item do manifesto está na quarentena |
| `E_FS_CHANGED` | a entrada não é o objeto aprovado, ou a árvore dela tem algo a mais ou a menos |
| `E_FS_MOVED_UNAPPROVED` | o que um movimento levou ao destino não é a entrada aprovada (só no Linux; ver limites) |
| `E_FS_UNCONFIRMED` | a alteração foi feita, mas o sistema não confirmou que está no disco |
| `E_FS_TOO_DEEP` | árvore mais funda que `max_depth` |
| `E_FS_IO` | o sistema operacional recusou a operação |

## O que não é executado

Recusado inteiro, por falta de definição (o caminho mais restritivo): alvo com curinga; `fs.overwrite`; `fs.clean_untracked`; `fs.move` para destino que não seja a quarentena nem uma restauração; alvo em raiz permitida fora da raiz de trabalho; alvo que contém um repositório abaixo do primeiro nível; no Windows, caminho escrito em caixa diferente da que está no disco; sistema que não seja Linux nem Windows.

## Limites conhecidos

- **Linux: a folha entre a conferência e a chamada.** `unlinkat` e `renameat2` agem sobre um nome dentro da pasta aberta, não sobre um objeto aberto. A pasta não pode ser trocada, mas a entrada sob aquele nome pode, entre a comparação de identidade e a chamada: a operação continua dentro da pasta aprovada e atinge o que estiver sob o nome. No movimento, a identidade do que chegou é conferida depois e a diferença vira `E_FS_MOVED_UNAPPROVED`, com o passo marcado como "fora do lugar" (o que foi movido está inteiro no destino e volta por uma restauração). No apagar não há conferência possível depois. No Windows a janela não existe: a operação é feita no identificador conferido.
- **Entradas criadas dentro de uma árvore durante o movimento.** A árvore é comparada com o inventário logo antes de mover; o que for criado nela entre essa comparação e a chamada vai junto para a quarentena (nada é destruído). Impedir isso pede bloquear a árvore contra outros processos.
- **Purga de leva com repositório.** Uma leva que guarda um repositório tem `.git` abaixo do primeiro nível e a purga dela é recusada. Falta definir como se esvazia.
- **Uso único depois de reinício.** Com a quarentena desligada não fica marca no disco; depois de reiniciar, quem impede a segunda execução é só o diário. O contrato do traço exige isso, mas a implementação persistente ainda não existe.
- **Windows: pasta com arquivo aberto por outro processo.** Mover uma pasta falha com acesso negado enquanto outro processo (antivírus, indexador) tem aberto um arquivo dentro dela. O executor para e relata (`E_FS_IO`); nada é tentado de novo.
- **Durabilidade.** A sincronização de pasta usa `fsync` no Linux e `FlushFileBuffers` no Windows. Nenhum teste corta a energia; o apagar recursivo sincroniza só a pasta do alvo, no fim.
- **Montagem.** A recusa de montagem no Linux é testada pela função de decisão com respostas sintéticas do sistema, não com uma montagem de verdade (pede privilégio).
- **Custo.** No Windows, conferir a grafia de um nome lista a pasta onde ele está, e cada entrada de uma árvore é aberta mais de uma vez (levantamento, comparação, alteração).
- **Raiz de trabalho.** A cerca é a fronteira confiável; a raiz de trabalho continua vindo do plano (limitada pela cerca).

## Testes

Todo teste que toca o disco está em `tests/` e trabalha numa pasta própria sob `CARGO_TARGET_TMPDIR` (`<tmp>/ads-fs/<teste>-<pid>-<nanos>-<n>/{fence,outside}`), criada por ele e nunca removida. A cerca é `fence`; `outside` é a irmã para onde os links e as trocas apontam, com uma sentinela que tem de sobreviver. A "pasta do usuário" é o texto `<fence>/fake-home`. A junção do Windows é criada com `mklink /J`; onde o sistema recusa criar link simbólico ou pasta sensível a maiúsculas, o teste avisa `SKIPPED` e sai.

| Arquivo | O que prova |
|---|---|
| `tests/sys.rs` | a camada de sistema sozinha: abrir sem seguir link, identidade, grafia, leitura limitada; criar, mover sem substituir e apagar pelo que está aberto |
| `tests/fence.rs` | a cerca, só com conferências (`rehearse`, `measure`): nenhuma alteração |
| `tests/execute.rs` | quarentena, restauração, apagar, falha no meio, ordem com o log |
| `tests/review.rs` | os achados da revisão independente: troca depois da última conferência, destino que aparece no último instante, entrada não aprovada, repositório aninhado, parada em cada passo, licença usada duas vezes |
