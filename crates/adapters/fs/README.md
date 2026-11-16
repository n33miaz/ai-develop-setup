# ads-adapter-fs

Executa no sistema de arquivos, dentro de uma cerca, as ações que o portão do núcleo licenciou.

O [portão](../../core/README.md#portão-de-ações-destrutivas-srcgate) decide com texto e não toca o disco. Este crate é onde o texto encontra o disco: é o único do workspace que move ou apaga arquivos do usuário, e só faz isso com a licença de execução do portão na mão.

**Estado: não ligado a nenhum executor real.** Nada no workspace chama este crate. O que ele garante, e contra quem, está em [Modelo de ameaça](#modelo-de-ameaça); o que ele não garante está em [Deliberadamente fora](#deliberadamente-fora).

| Arquivo | O que faz |
|---|---|
| `sys/` | As chamadas do sistema, sempre por identificador aberto: abrir um filho por um nome só, relativo a uma pasta aberta, sem seguir link; identidade, tipo, tamanho e montagem lidos do que foi aberto; criar, mover sem substituir, apagar e, onde o sistema permite, segurar uma pasta no lugar. É o único lugar com chamada do sistema e, no Windows, com `unsafe`. |
| `fence.rs` | A cerca, aberta uma vez a partir da raiz do volume, e a resolução: cada caminho vira uma lista de nomes abaixo dela, abertos um a um. |
| `survey.rs` | Os fatos reais de um alvo (itens, bytes, repositório) e o inventário: qual objeto, com que tamanho, está sob cada nome da árvore. Só leitura. |
| `prepare.rs` | Tudo que é conferido antes da primeira alteração. O ensaio é isto e nada mais. |
| `disk.rs` | O único lugar que altera o disco: operações de uma entrada só, cada uma alcançada de novo a partir da cerca e feita sobre o que está aberto. |
| `manifest.rs` | O manifesto e o desenho de uma leva de quarentena. |
| `journal.rs` | O traço do diário: a ordem com o log e o uso único da licença. |
| `executor.rs` | A entrada: `execute`, `rehearse` e `measure`. |
| `reason.rs`, `report.rs` | Os motivos de recusa com os códigos de `action.failed`, e o relatório. |
| `limits.rs` | Parâmetros; os padrões ficam só em [`defaults/fs-executor.toml`](../../../defaults/fs-executor.toml). |

## Entrada

```
Executor::new(cerca, política)            // resolve o caminho e abre a cerca nome a nome; recusa raiz de unidade, `/`, caminho de rede, link que aparece no caminho depois da resolução e sistema sem suporte
executor.execute(licença, action.started, diário) -> Completed | Failure { motivo, etapa, relatório }
executor.rehearse(&licença) -> relatório  // ensaio: todas as conferências, nenhuma alteração
executor.measure(raiz de trabalho, alvo)  // fatos reais, para o plano nascer com dado e não com palpite
```

- `execute` consome a licença (`ExecutionPermit`), que só `Action::authorize` constrói, e **não a devolve**. `Completed` guarda a licença gasta fora do alcance de quem chamou; `Completed::close(ação, meta)` a entrega direto ao portão (`Command::Executed`) e devolve a ação fechada, o evento `action.executed` e o relatório. Aceita ou recusada pelo portão, a licença não volta. Na falha, `motivo.error_code()` vai em `Command::Fail`.
- **Ordem com o log e uso único.** Nada é lido nem alterado antes de o diário (`Journal::consume`) registrar que a licença daquele `action.started` foi gasta. O evento tem de ser o da licença (mesma ação, mesmo hash, mesma aprovação). O contrato do traço: devolver `Ok` no máximo uma vez por ação, para sempre, com registro durável, atômico, compartilhado entre executores e que sobrevive a reinício. Este crate não traz nenhum diário: o de memória mora só nos testes (`tests/common`). Como segunda barreira, cada `Executor` lembra as ações de que já recebeu licença e recusa a segunda (`E_FS_PERMIT_REUSED`), mesmo com outro diário; essa memória acaba com o processo.
- **A política é a de quem constrói o executor**, não a do plano: o plano da licença é julgado de novo com ela antes de qualquer coisa.
- **Sistemas.** Linux e Windows de 64 bits. Em outro sistema `Executor::new` recusa (`UnsupportedSystem`); Windows de 32 bits não compila. Nada é simulado por caminho.

## Identificadores abertos, não caminhos

Um caminho é resolvido de novo pelo sistema a cada chamada. Conferir um caminho e depois usá-lo deixa uma janela: quem troca uma pasta por um link nesse intervalo redireciona a operação. Por isso o executor só entrega ao sistema um caminho, o da raiz do volume (`/`, `C:\`):

- **A cerca é aberta nome a nome a partir da raiz do volume**, cada nome relativo à pasta aberta antes, sem seguir link. A resolução do caminho pedido (`canonicalize`) só diz quais nomes percorrer; não prova nada. Se um nome no caminho virou link depois dela, a cerca não abre (`NotPlain`). O texto publicado da cerca é a lista de nomes que foi percorrida.
- **Cerca pedida através de link é aceita.** A resolução troca o link pelos nomes reais antes da travessia, e o texto publicado é esse caminho real, não o pedido: os planos têm de escrever os caminhos com ele (outra grafia é `E_FS_OUTSIDE_FENCE`). Quem escolhe a cerca é quem constrói o executor, e o que ele recebe é a pasta real que o texto nomeia; recusar seria só recusar quem chega por um atalho. O que é recusado é o link que aparece no caminho **depois** da resolução.
- **Texto e identificador durante a construção.** No Windows cada pasta do caminho é segurada no lugar assim que é aberta, e procurada de novo sob o nome na pasta de cima, segurada: até a travessia acabar, nenhuma pode ser renomeada nem movida, e o texto nomeia a pasta aberta. Nos dois sistemas os nomes são percorridos uma segunda vez a partir da raiz do volume, e cada um tem de ser o objeto da primeira travessia (`NotPlain` se não). No Linux, onde nada segura, isso vê uma pasta do caminho deslocada durante a travessia e não devolvida antes da segunda; uma pasta deslocada e devolvida entre as duas não é vista. Depois da construção as pastas acima da cerca são soltas: quem troca a própria cerca depois disso está fora do modelo (ver [Modelo de ameaça](#modelo-de-ameaça)).
- Todo o resto é aberto por **um nome só**, relativo à pasta aberta antes (`openat` com `O_NOFOLLOW` no Linux; `NtCreateFile` com pasta raiz e `FILE_OPEN_REPARSE_POINT` no Windows). Um link é aberto como link, nunca seguido. Nome com separador, `.`, `..` ou fluxo alternativo (`:`) não é nome.
- O que vale é a **identidade** do que foi aberto (dispositivo e inode; no Windows, volume e identificador de arquivo de 128 bits), lida do próprio identificador. A grafia do caminho só serve para chegar lá.
- Criar, mover e apagar são feitos **sobre o que está aberto**: criação relativa à pasta aberta do destino; mover e apagar pelo identificador da própria entrada no Windows (`NtSetInformationFile`), e por nome dentro da pasta aberta no Linux (`renameat2`, `unlinkat`), com a identidade conferida logo antes.

**Um identificador fixa o objeto, não o lugar.** Uma pasta aberta continua podendo ser renomeada ou movida por outro processo, e as operações relativas a ela vão junto. No Windows o executor a segura: abre a pasta sem conceder a outros o direito de apagar, e enquanto esse identificador existe o sistema recusa renomear ou mover a pasta e as pastas acima dela. No Linux não existe como segurar. Essa diferença decide onde o executor apaga de verdade (ver [Apagar de verdade](#apagar-de-verdade)).

## O que é conferido antes da primeira alteração

1. O plano, julgado de novo pela política confiável, não pode sair negado, nem como um apagar que a política manda para a quarentena, nem exigindo uma aprovação que a licença não traz.
2. A raiz de trabalho tem de estar dentro da cerca (ou ser ela) e ser uma pasta de verdade. Cada alvo tem de estar estritamente dentro da raiz de trabalho. A comparação com a cerca é letra por letra: outra caixa de um nome da cerca pode ser outra pasta.
3. **Resolução.** Cada nome é aberto a partir da cerca. Link simbólico, junção ou qualquer ponto de reanálise no meio do caminho é recusa; no último nome é "o alvo é um link", e o que ele aponta nunca é olhado. No Windows o nome tem de ser exatamente o que a pasta lista: outra caixa ou o nome curto (8.3) é recusa, então o texto que o portão julgou é a entrada que será aberta, também em pasta sensível a maiúsculas.
4. **Volume e montagem.** Entrada em outro volume é recusa. No Linux, entrada em outra montagem também, inclusive montagem do mesmo dispositivo (bind mount): a identificação de montagem e a marca de raiz de montagem vêm do `statx`; kernel ou sistema de arquivos que não informa as duas recusa tudo. No Windows, volume montado em pasta é ponto de reanálise: visto como link, nunca atravessado. Acima da cerca, no caminho da raiz do volume até ela, montagem é permitida.
5. **Fatos e inventário.** O executor percorre a árvore de cada alvo por identificadores abertos (sem seguir link; um link conta um), conta itens e bytes com soma verificada (estouro é recusa) e registra o inventário: a identidade, o tipo e, para arquivo, o tamanho de cada entrada. O plano é julgado mais uma vez com os fatos reais. Veredito mais duro que o declarado é recusa; com aprovação, mais itens ou mais bytes do que a estimativa aprovada também.
6. **Repositórios.** `.git` no nível do alvo é o fato "raiz de repositório", julgado pela política. `.git` (pasta, ou o arquivo de uma worktree) em qualquer profundidade abaixo disso é recusa (`E_FS_NESTED_REPOSITORY`), para apagar e para quarentena. O nome é comparado sem distinguir caixa: `.GIT` conta.
7. **Onde cada alvo cai de verdade.** O veredito julga a pasta que recebe os alvos; o executor julga o lugar final de cada um: estritamente dentro da raiz de trabalho, fora de pasta protegida pela política e, no disco, pela mesma abertura sem link. Na **restauração**, o lugar de cada item do manifesto é julgado **pelo próprio portão** (um plano de mover só aquele item para a pasta de origem, classificado com a política confiável): valem as regras de entrada de um mover, inclusive `destination_entry_reserved` (a entrada criada não pode ser a raiz de trabalho, uma raiz permitida ou a quarentena, nem conter uma delas). Isso é feito para todos os itens antes de olhar o disco de qualquer um; a regra não é copiada aqui.
8. **Tamanho do manifesto.** O manifesto da leva é montado e medido antes de qualquer alteração; maior que `max_manifest_bytes` é recusa (`E_FS_MANIFEST_TOO_LARGE`).
9. **Apagar de verdade** só passa onde o parâmetro `definitive_delete` permite (ver abaixo).
10. Alvos sobrepostos, curinga, sobrescrever, limpar não rastreados, mover que não seja para a quarentena nem restauração, e alvo que é a pasta privada do executor ou a contém: recusa. Ver [o que não é executado](#o-que-não-é-executado).

Qualquer recusa aqui acontece com o disco intacto (`Failure.stage == Checks`).

## Alterações

Só `disk.rs` altera o disco: criar pasta (falha se existe), criar arquivo novo (falha se existe), mover e remover uma entrada. Não há exclusão nem criação recursiva da biblioteca. Cada operação alcança a entrada de novo a partir da cerca e confere: estritamente abaixo do piso (a raiz de trabalho; na purga, a pasta de quarentena; e a pasta de espera da própria execução), nenhum link no caminho e, na entrada, **o mesmo objeto do inventário, com o mesmo tamanho**.

- **Só o que foi aprovado.** Um arquivo trocado por outro com o mesmo nome é recusa, e um arquivo que cresceu ou encolheu mantendo a identidade (escrito direto ou por outro hardlink) também (`E_FS_CHANGED`). Antes de mover uma árvore, e de novo antes de esvaziá-la, ela inteira é comparada com o inventário: nomes, identidades e tamanhos. O tamanho é conferido uma última vez na hora de remover cada entrada (no Windows, no identificador dela; no Linux, pelo nome dentro da pasta aberta).
- **Mover nunca substitui.** A recusa de destino existente é do próprio sistema, na mesma chamada que move (`RENAME_NOREPLACE`; no Windows, renomear sem substituição). Sistema de arquivos sem essa primitiva recusa o movimento.
- **Um movimento feito é dito como feito.** Depois de o sistema aceitar o movimento, o que chegou é conferido (no Linux, por nome; no Windows o identificador movido já é a prova) e as duas pastas são sincronizadas de qualquer jeito. Se a conferência falha, o passo sai "não verificado" (`E_FS_MOVE_UNVERIFIED`): o alvo saiu do lugar e o relatório não promete onde ele está.
- **Durabilidade.** Depois de cada criação, movimento e remoção, a pasta envolvida é sincronizada (as duas, no movimento). Um passo só conta como feito depois disso; se a alteração foi feita e a sincronização falhou, sai como não confirmado (`E_FS_UNCONFIRMED`), nunca como feito.
- **O que foi criado é dito como criado.** Criar uma pasta é uma chamada só (`mkdirat`; no Windows, `NtCreateFile` com criação exclusiva) e nada é aberto depois dela: o sucesso dessa chamada é "criado". O manifesto é gravado em três passos: criar (exclusivo), escrever, sincronizar. Qualquer falha depois da criação, da pasta ou do arquivo, deixa a entrada no disco, e o relatório diz isso ("não confirmado": existe, incompleto ou sem confirmação), nunca "não criado". Em toda saída depois da criação a pasta onde a entrada está é sincronizada mesmo assim, e o detalhe diz se essa confirmação também falhou. Nenhum alvo foi movido nesse ponto.

A sonda (`Probe`) é avisada de cada alteração: antes de qualquer abertura (`before`), depois da última conferência com os identificadores abertos (`opened`) e, nas alterações de mais de uma chamada, depois de uma chamada que já mudou o disco (`midway`: pasta ou arquivo criado, arquivo escrito, movimento feito). É por ela que os testes param a execução em qualquer passo e mexem no disco em volta de uma conferência.

### Quarentena

Mover para dentro de `<raiz de trabalho>/<quarantine_dir>` é quarentena. O destino vem do plano (`.../<16 hexadecimais>`); o executor cria dentro dele a pasta da ação, de forma exclusiva:

```
<destino>/<action_id>/manifest.json
<destino>/<action_id>/items/<caminho do alvo abaixo da raiz de trabalho>
```

- A pasta `<action_id>` é criada com falha se existir: dois pedidos iguais ficam em pastas separadas, e a repetição da mesma ação é recusada sem tocar em nada, mesmo por um executor e um diário que a esqueceram.
- A estrutura abaixo de `items` é a do alvo abaixo da raiz de trabalho: `a/build` e `b/build` não colidem.
- O manifesto lista a leva inteira: caminho original de cada item, `plan_hash`, `action_id` e o instante de `action.started`. É gravado e sincronizado, como arquivo e na pasta da leva, antes do primeiro movimento. O que está de fato na quarentena é o que existe em `items`.
- Um `action_id` que não serve de nome de pasta (com `:`, terminado em ponto, nome de dispositivo) é recusado.

### Restauração

Restaurar é uma ação como as outras, com licença: `fs.move` de **uma** pasta de leva para a raiz de trabalho. Cada item do manifesto que ainda está em `items` volta ao lugar de origem; as pastas de origem que sumiram são recriadas uma a uma. O lugar de cada item é julgado pelo portão (item 7 acima). Lugar de origem ocupado recusa a restauração inteira antes do primeiro movimento, e o que aparecer no lugar depois dessa conferência não é substituído. Item que não está mais na quarentena aparece no relatório como ausente. Sem nenhum item presente, recusa.

O manifesto é lido pela pasta aberta da leva, do arquivo que foi aberto: link não é seguido, tem de ser arquivo regular e a leitura para em `max_manifest_bytes`. Lido do disco é dado, não instrução: versão, identificadores e cada nome são validados; o caminho original tem de ser exatamente a raiz de trabalho mais o lugar do item em `items`; a ação do manifesto tem de ser a da pasta.

### Apagar de verdade

Só existe em dois casos: esvaziar a quarentena (todos os alvos dentro dela, a regra `quarantine_purge` disparada e a aprovação presente na licença) e política com `quarantine_enabled = false`. Fora disso, um `fs.delete` ou `fs.delete_recursive` que chegue sem ter sido reescrito é recusado. `fs.delete` aceita arquivo, link ou pasta vazia.

Apagar age sobre um nome dentro de uma pasta aberta. Se outro processo tira essa pasta de dentro da cerca entre a última conferência e a chamada, a exclusão vai junto. O parâmetro `definitive_delete` diz onde o executor apaga:

| Valor | O que faz |
|---|---|
| `"contained"` (padrão) | Só apaga onde o sistema segura a pasta no lugar: **Windows**. No **Linux todo apagar definitivo é recusado** (`E_FS_DELETE_UNCONTAINED`): árvore, entrada única e purga. Lá o executor só faz quarentena e restauração, e **não esvazia a quarentena**. |
| `"isolated"` | Apaga nos dois sistemas. No Linux fica a janela descrita acima, contra um processo do mesmo usuário; o executor reconfere, antes de cada remoção, que cada pasta do caminho ainda está onde estava (`E_FS_DISPLACED` se não), o que estreita a janela e não a fecha. |
| `"refuse"` | Nunca apaga de verdade. |

Como uma árvore é apagada, nos modos que apagam:

1. A árvore **não é esvaziada onde está**. É movida inteira, sem substituir nada (o mesmo movimento conferido de qualquer mover), para a pasta de espera da execução: `<raiz de trabalho>/<private_dir>/<action_id>/<n>`. A pasta `<action_id>` é criada de forma exclusiva (no Linux, só para o usuário do executor). Os caminhos que outros processos conhecem deixam de alcançar a árvore nesse movimento.
2. Lá a árvore é comparada de novo com o inventário (nomes, identidades, tamanhos).
3. Cada pasta é aberta por identificador e, no Windows, **segurada no lugar**: ela e todas as pastas acima dela até a cerca ficam abertas sem o direito de apagar concedido a outros, então nenhuma pode ser renomeada, movida ou removida enquanto é esvaziada. Cada entrada é reivindicada pelo próprio identificador (identidade e tamanho conferidos), também sem conceder a outros o direito de apagar: da reivindicação até a remoção o sistema recusa renomeá-la ou movê-la, por qualquer nome dela. Depois disso ela é procurada de novo sob o nome, na pasta segurada, com a mesma identidade, e só então removida. Uma pasta é solta antes de ser reivindicada (as duas aberturas não convivem); quem a tirar do lugar nesse intervalo faz a reivindicação, pelo nome na pasta de cima segurada, não achar o objeto aprovado. Link é removido como link, nunca atravessado.
4. Cada pasta que perdeu entradas é sincronizada antes de ser deixada, também quando a remoção para no meio. O relatório distingue entradas vistas sair (`removed`) de entradas cuja pasta foi confirmada depois (`confirmed`).
5. No fim a pasta `<action_id>` é removida como qualquer entrada: a pasta onde ela está segurada no lugar (Windows), ela reivindicada sem conceder a outros o direito de apagar, procurada de novo sob o nome e conferida contra a identidade que tinha ao ser criada. Se algo falha antes da remoção, ela fica e o relatório continua dizendo "criado", o que é verdade, e a execução termina com sucesso. Se a remoção foi feita e a sincronização da pasta de cima falhou, a execução falha (`E_FS_UNCONFIRMED`, etapa `Clearing`) com todos os passos feitos e a pasta marcada "removida no fim, sem confirmação".

Uma entrada única (arquivo, link, pasta vazia) é removida onde está, com a pasta dela segurada no lugar e ela mesma reivindicada do mesmo jeito (no Windows). No Linux, onde nada segura, ela é procurada de novo sob o nome logo antes da remoção (`E_FS_DISPLACED` se saiu), o que estreita a janela e não a fecha. Reivindicar falha se outro processo já tem a entrada aberta com o direito de apagar.

### Falha no meio e recuperação

O executor para na primeira falha: não desfaz e não continua. `Failure.stage` diz até onde foi, e o relatório traz primeiro a infraestrutura (`staging`: cada pasta e o manifesto, com "pendente", "já existia", "criado", "removido no fim", "removido no fim, sem confirmação", "falhou" ou "não confirmado") e depois um passo por alvo. `Report::untouched()` responde se alguma coisa mudou no disco.

| Estado do passo | O que quer dizer |
|---|---|
| `Pending` | não tocado |
| `Done` | feito, com as pastas envolvidas confirmadas |
| `Absent` | listado no manifesto, não estava na quarentena |
| `Failed` | parou antes de mudar o alvo: ele está onde estava, inteiro |
| `Held` | um apagar definitivo parou depois de mover o alvo para a pasta de espera (`held` do passo) e conferir que ele chegou lá: o que sobrou dele está lá; `removed` e `confirmed` contam o que saiu |
| `Unconfirmed` | a alteração foi feita e o sistema não confirmou que está no disco |
| `Unverified` | o sistema fez o movimento e a conferência depois dele falhou: o alvo saiu do lugar; onde está não é prometido. Vale também para o movimento à pasta de espera: `held` é então só o lugar tentado |

| Etapa | O que mudou no disco |
|---|---|
| `Checks` | nada |
| `Staging` | nenhum alvo; as pastas e o manifesto que o relatório marca como criados ou não confirmados existem |
| `Steps` | o que os estados dos passos dizem |
| `Clearing` | todos os passos feitos; a pasta de espera da execução foi removida e a remoção não foi confirmada no disco |

O que uma nova execução encontra depois de uma interrupção:

- **A mesma ação nunca roda de novo**: o diário recusa, e a pasta da ação (na quarentena ou na pasta privada) já existe.
- **Quarentena que parou antes de o manifesto estar gravado e sincronizado**: nenhum alvo saiu do lugar. A pasta da leva fica, sem itens, com ou sem um manifesto incompleto; uma restauração dela é recusada (`E_FS_MANIFEST_INVALID` ou `E_FS_NOTHING_TO_RESTORE`) e ela sai pela purga.
- **Quarentena que parou depois**: cada alvo está inteiro em exatamente um lugar, o de origem ou `items`. A restauração da leva devolve o que foi e marca o resto como ausente; pode ser repetida até terminar.
- **Restauração interrompida**: retomada por outra restauração da mesma leva.
- **Apagar definitivo interrompido**: com o passo `Held`, o que sobrou da árvore está em `<private_dir>/<action_id>/<n>`, e o relatório diz o caminho. Com o passo `Unverified`, a árvore foi mandada para esse caminho e a chegada não foi conferida: é o primeiro lugar a olhar, não um lugar prometido. Não volta sozinho para o lugar de origem. Uma ação nova de apagar com esse caminho como alvo termina o serviço; a pasta privada em si, ou uma pasta que a contém, nunca é alvo.

Os testes param a execução em cada chamada da sonda e conferem esse estado. Queda de energia de verdade não é simulada: o que se garante é a ordem das sincronizações.

O evento `action.failed` leva só o código (é o contrato); o relatório é devolvido a quem chamou.

## Modelo de ameaça

O que é garantido, por tipo de adversário. "Dentro da cerca" quer dizer: nenhuma entrada fora da pasta aberta como cerca é criada, movida, lida ou apagada pelo executor.

**(a) Agente descuidado, sem concorrência** (o plano está errado, mal escrito ou desatualizado; ninguém mexe no disco durante a execução). Garantido nos dois sistemas:

- nada fora da cerca nem fora da raiz de trabalho é tocado, qualquer que seja o texto do plano: link, junção, `..`, outra caixa, nome curto, outro volume e outra montagem são recusa;
- só é movido ou apagado o que foi levantado: mesmas entradas, mesmas identidades, mesmos tamanhos; mais itens ou bytes do que o aprovado é recusa;
- nada é sobrescrito: toda criação é exclusiva e todo movimento recusa destino existente;
- repositório (no alvo ou abaixo dele) não é apagado nem posto em quarentena sem a política dizer; a restauração não cria nem cobre pasta reservada;
- uma execução que para diz exatamente o que mudou, e o que foi para a quarentena volta por uma restauração.

**(b) Processo concorrente não hostil** (outra ferramenta, um editor, uma compilação, usando caminhos, sem intenção de enganar). Além do item (a):

- trocas feitas antes da última conferência são vistas e viram recusa (entrada nova, entrada que sumiu, outro objeto com o mesmo nome, arquivo que mudou de tamanho, link no caminho);
- trocas feitas depois da última conferência, **no Windows**, não redirecionam a operação: mover e apagar agem no identificador aberto da própria entrada, então acontecem no objeto aberto ou falham;
- **no Linux**, `renameat2` e `unlinkat` agem sobre o nome dentro da pasta aberta, não sobre um objeto aberto. Uma ferramenta comum que troca a entrada sob o nome entre a comparação de identidade e a chamada (renomeia o arquivo antigo e publica outro com o mesmo nome, como faz uma compilação) faz o movimento levar a entrada nova. O que se garante é a **detecção depois**: a conferência da chegada, pelo nome no destino, não acha o objeto aprovado e o passo sai "não verificado" (`E_FS_MOVE_UNVERIFIED`). A consequência é recuperável: a outra entrada foi para a quarentena (restaurável) ou, numa restauração, voltou para a raiz de trabalho; nada foi apagado nem sobrescrito. Com `definitive_delete = "isolated"` não há essa garantia para a remoção: uma entrada publicada sob o nome entre a última conferência e o `unlinkat` é apagada no lugar da aprovada, sem detecção e sem volta (é por isso que o padrão no Linux não apaga);
- um destino que aparece no último instante não é substituído;
- uma árvore a apagar é tirada do alcance dos caminhos (movida para a pasta de espera) antes da primeira remoção, e comparada de novo lá.

Não garantido em (b): o que outro processo escreve num arquivo entre a última conferência de tamanho e a chamada vai junto (para a quarentena, restaurável; ou é apagado, onde o executor apaga); entradas criadas dentro de uma árvore entre a comparação e o movimento vão junto com ela.

**(c) Processo hostil, do mesmo usuário, com permissão de escrita dentro da cerca**, agindo durante a execução:

- **Windows.** Não consegue redirecionar um apagar: as pastas em uso por uma remoção ficam seguradas no lugar, da cerca até a pasta que está sendo esvaziada (o sistema recusa movê-las; testado entre duas entradas e com uma entrada aberta), e cada entrada é removida pelo próprio identificador, com identidade e tamanho conferidos nele, aberto sem conceder a outros o direito de apagar: a própria entrada, arquivo ou pasta vazia, não pode ser renomeada para fora entre a reivindicação e a remoção (testado: o sistema recusa com violação de compartilhamento). Não consegue fazer um movimento sobrescrever. **Consegue**, numa quarentena, numa restauração ou no movimento de uma árvore para a pasta de espera, mover a pasta de destino entre a abertura e a chamada: o item então cai onde essa pasta estiver (não é destruído; no caso da pasta de espera, a remoção não começa, porque o caminho até ela é refeito a partir da cerca). **Consegue** também renomear a própria entrada de origem entre a reivindicação e o movimento (a reivindicação de um movimento, ao contrário da de uma remoção, deixa a outros o direito de renomear): o movimento pelo identificador a leva de onde ela estiver, inclusive de fora da cerca, para o destino. Nada é apagado nem sobrescrito por esse caminho. **Consegue** fazer a execução falhar (segurando arquivos abertos, ocupando nomes).
- **Linux.** Um descritor fixa o objeto, não o lugar, e nada segura uma pasta. Por isso, no padrão, **o executor não apaga nada de verdade no Linux**: não há chamada de remoção alcançável (testado: a sonda não vê nenhuma alteração). Numa quarentena ou restauração o adversário **consegue** mover a pasta de origem ou de destino entre a abertura e a chamada, fazendo o item sair de ou cair em outro lugar, inclusive fora da cerca, e **consegue** trocar a entrada sob o nome entre a comparação de identidade e a chamada, fazendo outra entrada da mesma pasta ser movida (o passo sai "não verificado" quando a conferência da chegada percebe). Nada é apagado nem sobrescrito por esses caminhos. Com `definitive_delete = "isolated"`, o mesmo adversário consegue fazer remoções acontecerem fora da cerca; esse modo não é para ambiente com adversário.
- **Nos dois.** Não é adversário coberto quem pode trocar a própria cerca, ou uma pasta acima dela, antes de o executor ser construído ou depois (no Linux, também durante a construção, nos limites descritos em [Identificadores abertos](#identificadores-abertos-não-caminhos)), quem controla a política confiável ou o diário, nem quem tem privilégio de administrador.

## Deliberadamente fora

- **Escritor concorrente entre a última conferência e a chamada.** O tamanho de cada arquivo é conferido no levantamento, antes do movimento, de novo na pasta de espera e no identificador de cada entrada ao removê-la. O que for escrito depois dessa última leitura e antes da chamada não é visto. Fechar isso pede bloquear o arquivo contra escrita, que o executor não faz.
- **Entradas criadas numa árvore entre a comparação com o inventário e o movimento.** Vão junto com a árvore (para a quarentena ou para a pasta de espera, onde a segunda comparação as recusa antes de qualquer remoção).
- **Contenção de quarentena e restauração contra processo hostil** (item (c)): a pasta de origem ou de destino, e no Windows a própria entrada movida, podem ser movidas entre a abertura e a chamada. No Windows daria para segurá-las e reivindicar a entrada sem conceder o direito de apagar, como no apagar; não foi feito nesta entrega.
- **Apagar definitivo no Linux sob concorrência hostil.** Não há primitiva que segure uma pasta no lugar; o padrão é recusar. Consequência: no Linux o executor não esvazia a quarentena.
- **Diário persistente e uso único entre reinícios.** O contrato do traço exige; a implementação não existe. Com a quarentena desligada não fica marca no disco, e depois de um reinício só o diário impede a segunda execução.
- **Montagem real.** A recusa de montagem é testada pela função de decisão com respostas sintéticas do sistema; a coleta pelo `statx` só é exercitada no caso "mesma montagem". Montar de verdade pede privilégio.
- **Corte de energia real.** Garante-se a ordem das sincronizações (`fsync` de pasta no Linux, `FlushFileBuffers` no Windows), testada pela sequência de chamadas; nenhum teste corta a energia.
- **Letra de unidade remapeada no Windows** (`subst`, mapeamento por usuário): a raiz do volume é aberta pelo caminho `\\?\C:\`, e quem pode remapear a letra muda o que ela é.
- **Windows de 32 bits** (não compila) e **outros sistemas** (recusam).
- **Caminhos acima de 260 caracteres e arquivo em uso por outro processo** não têm teste. No Windows, mover uma pasta falha com acesso negado enquanto outro processo (antivírus, indexador) tem um arquivo aberto dentro dela, e segurar uma pasta falha se outro processo a tem aberta para apagar ou renomear; o executor para e relata, sem tentar de novo. Isso vale também para cada pasta do caminho da raiz do volume até a cerca durante `Executor::new`, que então falha (`Unresolved`).
- **Purga de leva com repositório**: recusada (o `.git` fica abaixo do primeiro nível da leva). Falta definir como se esvazia.
- **Custo.** Cada entrada de uma árvore é aberta várias vezes (levantamento, comparações, alteração), cada pasta apagada recebe uma sincronização, e no Windows conferir a grafia de um nome lista a pasta onde ele está, inclusive cada pasta da raiz do volume até a cerca na construção.
- **Raiz de trabalho.** A cerca é a fronteira confiável; a raiz de trabalho continua vindo do plano (limitada pela cerca).

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
| `E_FS_POLICY_DENIES` | a política confiável nega o plano da licença, ou o lugar real de um item |
| `E_FS_APPROVAL_MISSING` | a política confiável exige aprovação e a licença não traz |
| `E_FS_QUARANTINE_BYPASSED` | apagar de verdade o que a política manda para a quarentena |
| `E_FS_FACTS_CONTRADICT_PLAN` | o disco contradiz o declarado de um jeito que muda o veredito |
| `E_FS_NESTED_REPOSITORY` | a árvore de um alvo contém um repositório |
| `E_FS_OVERFLOW` | uma contagem de itens ou de bytes não cabe em 64 bits |
| `E_FS_ACTION_ID_UNUSABLE` | o `action_id` não serve de nome de pasta |
| `E_FS_DESTINATION_TAKEN` | a pasta da ação (na quarentena ou na pasta privada), ou um lugar dentro dela, já existe |
| `E_FS_NOT_EMPTY` | `fs.delete` de pasta com conteúdo |
| `E_FS_MANIFEST_INVALID` | manifesto ausente, que não é arquivo, grande demais, malformado ou que não casa com a pasta |
| `E_FS_MANIFEST_TOO_LARGE` | o manifesto da leva passaria do limite que o executor lê de volta |
| `E_FS_RESTORE_PLACE_TAKEN` | o lugar de origem de um item está ocupado |
| `E_FS_NOTHING_TO_RESTORE` | nenhum item do manifesto está na quarentena |
| `E_FS_CHANGED` | a entrada não é o objeto aprovado, tem outro tamanho, ou a árvore dela tem algo a mais ou a menos |
| `E_FS_MOVE_UNVERIFIED` | o sistema fez o movimento e a conferência depois dele falhou |
| `E_FS_DELETE_UNCONTAINED` | apagar definitivo onde o sistema não segura a pasta no lugar, ou com `definitive_delete = "refuse"` |
| `E_FS_DISPLACED` | uma pasta que estava sendo esvaziada não está mais onde o executor a pôs |
| `E_FS_UNCONFIRMED` | a alteração foi feita, mas o sistema não confirmou que está no disco |
| `E_FS_TOO_DEEP` | árvore mais funda que `max_depth` |
| `E_FS_IO` | o sistema operacional recusou a operação |

## O que não é executado

Recusado inteiro, por falta de definição (o caminho mais restritivo): alvo com curinga; `fs.overwrite`; `fs.clean_untracked`; `fs.move` para destino que não seja a quarentena nem uma restauração; alvo em raiz permitida fora da raiz de trabalho; alvo que contém um repositório abaixo do primeiro nível; alvo que é a pasta privada do executor ou a contém; apagar definitivo no Linux (no padrão); no Windows, caminho escrito em caixa diferente da que está no disco; sistema que não seja Linux nem Windows de 64 bits.

## Testes

Todo teste que toca o disco está em `tests/` e trabalha numa pasta própria sob `CARGO_TARGET_TMPDIR` (`<tmp>/ads-fs/<teste>-<pid>-<nanos>-<n>/{fence,outside}`), criada por ele e nunca removida. A cerca é `fence`; `outside` é a irmã para onde os links e as trocas apontam, com uma sentinela que tem de sobreviver. A "pasta do usuário" é o texto `<fence>/fake-home`. A junção do Windows é criada com `mklink /J`; onde o sistema recusa criar link simbólico ou pasta sensível a maiúsculas, o teste avisa `SKIPPED` e sai.

O executor dos testes usa `definitive_delete = "isolated"`, para os testes de apagar rodarem também no Linux; o padrão de fábrica tem testes próprios, por sistema (`by_default_nothing_is_removed_for_good_where_folders_cannot_be_held`).

| Arquivo | O que prova |
|---|---|
| `tests/sys.rs` | a camada de sistema sozinha: abrir sem seguir link, identidade, grafia, leitura limitada; criar, mover sem substituir e apagar pelo que está aberto; uma pasta segurada não sai do lugar (Windows) e uma só aberta sai (os dois) |
| `tests/fence.rs` | a cerca, só com conferências (`rehearse`, `measure`): nenhuma alteração |
| `tests/execute.rs` | quarentena, restauração, apagar, falha no meio, ordem com o log |
| `tests/review.rs` | a primeira revisão independente: troca depois da última conferência, destino que aparece no último instante, entrada não aprovada, repositório aninhado, parada em cada passo, licença usada duas vezes |
| `tests/contain.rs` | a segunda: cerca aberta nome a nome, arquivo que cresceu, movimento não verificado, manifesto parado depois de criado, árvore apagada na pasta de espera com cada pasta sincronizada, pasta em esvaziamento que não sai do lugar, padrão por sistema, restauração sobre pasta reservada |
| `tests/settle.rs` | a terceira: cerca cujo caminho se mexe durante a travessia, entrada reivindicada para remoção que não sai da cerca, movimento à pasta de espera não verificado, remoção protegida e relatada da pasta de espera, pasta ou manifesto parado depois de criado com a pasta de cima confirmada |
