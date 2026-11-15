# ads-adapter-fs

Executa no sistema de arquivos, dentro de uma cerca, as ações que o portão do núcleo licenciou.

O [portão](../../core/README.md#portão-de-ações-destrutivas-srcgate) decide com texto e não toca o disco. Este crate é onde o texto encontra o disco: é o único do workspace que move ou apaga arquivos do usuário, e só faz isso com a licença de execução do portão na mão.

| Arquivo | O que faz |
|---|---|
| `fence.rs` | A cerca e a resolução física: cada caminho vira uma lista de nomes abaixo da cerca, olhados no disco um a um, sem seguir link. |
| `survey.rs` | Os fatos reais de um alvo: se é raiz de repositório, quantos itens e quantos bytes. Só leitura. |
| `prepare.rs` | Tudo que é conferido antes da primeira alteração. O ensaio é isto e nada mais. |
| `disk.rs` | O único lugar que altera o disco: cinco operações de uma entrada só, cada uma reconferida logo antes. |
| `manifest.rs` | O manifesto e o desenho de uma leva de quarentena. |
| `journal.rs` | O traço do diário (a confirmação de `action.started`) e a implementação em memória. |
| `executor.rs` | A entrada: `execute`, `rehearse` e `measure`. |
| `reason.rs`, `report.rs` | Os motivos de recusa com os códigos de `action.failed`, e o relatório por alvo. |
| `limits.rs` | Parâmetros; os padrões ficam só em [`defaults/fs-executor.toml`](../../../defaults/fs-executor.toml). |

## Entrada

```
Executor::new(cerca, política)            // resolve a cerca uma vez; recusa raiz de unidade, `/` e caminho de rede
executor.execute(licença, action.started, diário) -> Completed { licença, relatório } | Failure { motivo, relatório }
executor.rehearse(&licença) -> relatório  // ensaio: todas as conferências, nenhuma alteração
executor.measure(raiz de trabalho, alvo)  // fatos reais, para o plano nascer com dado e não com palpite
```

- `execute` consome a licença (`ExecutionPermit`), que só `Action::authorize` constrói. Ela volta em `Completed`, para fechar a ação com `Command::Executed`; na falha, `motivo.error_code()` vai em `Command::Fail`.
- **Ordem com o log.** Nada é lido nem alterado antes de o diário (`Journal::confirm_started`) confirmar que o `action.started` daquela licença foi gravado. O evento tem de ser o da licença (mesma ação, mesmo hash, mesma aprovação). Persistir é dever de quem implementa o traço; `MemoryJournal` serve aos testes e recusa o segundo `action.started` da mesma ação.
- **A política é a de quem constrói o executor**, não a do plano: o plano da licença é julgado de novo com ela antes de qualquer coisa.

## O que é conferido antes da primeira alteração

1. O plano, julgado de novo pela política confiável, não pode sair negado, nem como um apagar que a política manda para a quarentena, nem exigindo uma aprovação que a licença não traz.
2. A raiz de trabalho tem de estar dentro da cerca (ou ser ela) e ser uma pasta de verdade. Cada alvo tem de estar estritamente dentro da raiz de trabalho.
3. **Resolução física.** O caminho é percorrido nome a nome a partir da cerca, com `symlink_metadata`. Link simbólico, junção ou qualquer outro ponto de reanálise no meio do caminho é recusa; no último nome é "o alvo é um link", e o que ele aponta nunca é olhado. Fora do Windows, entrada em outro volume é recusa (no Windows, volume montado em pasta é ponto de reanálise). Ao fim, o caminho resolvido pelo sistema tem de ser o mesmo que foi percorrido: outra grafia do mesmo lugar é recusa.
4. **Fatos.** O executor conta itens e bytes de cada alvo (sem seguir link; um link conta um) e olha se há `.git` no nível do alvo. O plano é julgado mais uma vez com esses fatos no lugar dos declarados. Veredito mais duro que o do plano declarado é recusa; com aprovação, mais itens ou mais bytes do que a estimativa aprovada também.
5. **Onde cada alvo cai de verdade.** O veredito julga a pasta que recebe os alvos; o executor julga o lugar final de cada um (`<destino>/<action_id>/items/...` na quarentena, o lugar de origem na restauração): estritamente dentro da raiz de trabalho, fora de pasta protegida pela política e, no disco, pelo mesmo percurso sem link.
6. Alvos sobrepostos (um dentro do outro), curinga, sobrescrever, limpar não rastreados e mover que não seja para a quarentena nem restauração: recusa. Ver [o que não é executado](#o-que-não-é-executado).

Qualquer recusa aqui acontece com o disco intacto.

## Alterações

Só `disk.rs` altera o disco, por cinco operações de uma entrada: criar pasta (falha se existe), criar arquivo novo (falha se existe), mover, remover uma entrada e remover uma árvore, que é a remoção de uma entrada repetida das folhas para a raiz. Cada operação, logo antes da chamada do sistema, confere de novo: estritamente abaixo do piso (a raiz de trabalho; na purga, a pasta de quarentena), nenhum link no caminho, a entrada ainda é o que era. Não há `remove_dir_all` nem `create_dir_all`. Um link é removido como link (`remove_dir` para link de pasta e junção no Windows, `remove_file` nos demais) e nunca é atravessado.

A sonda (`Probe`) é avisada de cada alteração **antes** da última conferência; é por ela que os testes falham um lote no meio e trocam uma pasta por um link entre a primeira conferência e a alteração.

### Quarentena

Mover para dentro de `<raiz de trabalho>/<quarantine_dir>` é quarentena. O destino vem do plano (`.../<16 hexadecimais>`); o executor cria dentro dele a pasta da ação, de forma exclusiva:

```
<destino>/<action_id>/manifest.json
<destino>/<action_id>/items/<caminho do alvo abaixo da raiz de trabalho>
```

- A pasta `<action_id>` é criada com falha se existir: dois pedidos iguais (mesmo hash, mesmo destino) ficam em pastas separadas, e a repetição da mesma ação é recusada sem tocar em nada.
- A estrutura abaixo de `items` é a do alvo abaixo da raiz de trabalho: `a/build` e `b/build` não colidem.
- Nada é sobrescrito: toda criação é exclusiva e o mover exige o lugar vago.
- O manifesto é gravado antes do primeiro movimento e lista a leva inteira: caminho original de cada item, `plan_hash`, `action_id` e o instante de `action.started`. O que está de fato na quarentena é o que existe em `items`.
- Um `action_id` que não serve de nome de pasta (com `:`, terminado em ponto, nome de dispositivo) é recusado.

### Restauração

Restaurar é uma ação como as outras, com licença: `fs.move` de **uma** pasta de leva para a raiz de trabalho. Cada item do manifesto que ainda está em `items` volta ao lugar de origem; as pastas de origem que sumiram são recriadas uma a uma. Lugar de origem ocupado recusa a restauração inteira antes do primeiro movimento. Item que não está mais na quarentena aparece no relatório como ausente. Sem nenhum item presente, recusa.

O manifesto lido do disco é dado, não instrução: versão, identificadores e cada nome são validados; o caminho original tem de ser exatamente a raiz de trabalho mais o lugar do item em `items`; a ação do manifesto tem de ser a da pasta.

### Apagar de verdade

Só existe em dois casos: esvaziar a quarentena (todos os alvos dentro dela, a regra `quarantine_purge` disparada e a aprovação presente na licença) e política com `quarantine_enabled = false`. Fora disso, um `fs.delete` ou `fs.delete_recursive` que chegue sem ter sido reescrito é recusado. `fs.delete` aceita arquivo, link ou pasta vazia.

### Falha no meio

O executor para na primeira falha: não desfaz e não continua. O relatório traz um passo por alvo, na ordem: feito, falhou (com o motivo e, no apagar recursivo, quantas entradas já tinham saído) ou pendente. O que já foi para a quarentena fica lá, com o manifesto, e volta por uma restauração. O evento `action.failed` leva só o código (é o contrato); o relatório é devolvido a quem chamou.

## Códigos de `action.failed`

Um código nunca muda de sentido; motivo novo ganha código novo.

| Código | Quando |
|---|---|
| `E_FS_STARTED_MISMATCH` | o evento entregue não é o `action.started` da licença |
| `E_FS_JOURNAL_REFUSED` | o diário não confirmou a gravação do `action.started` |
| `E_FS_PATH_STYLE` | caminho no estilo de outro sistema |
| `E_FS_OUTSIDE_FENCE` | caminho fora da cerca |
| `E_FS_OUTSIDE_WORK_ROOT` | alvo, destino ou entrada que não está estritamente dentro da raiz de trabalho |
| `E_FS_LINK_IN_PATH` | link simbólico, junção ou ponto de reanálise no caminho |
| `E_FS_NOT_PHYSICAL` | o caminho no disco não é o que foi escrito |
| `E_FS_OTHER_VOLUME` | o caminho entra em outro volume |
| `E_FS_MISSING` | o alvo, ou uma pasta no caminho, não existe |
| `E_FS_NOT_A_DIRECTORY` | algo que não é pasta onde tinha de haver uma |
| `E_FS_SPECIAL_FILE` | dispositivo, soquete ou pipe |
| `E_FS_UNSUPPORTED` | operação, ou forma dela, que este adaptador não executa |
| `E_FS_OVERLAPPING_TARGETS` | um alvo dentro de outro |
| `E_FS_POLICY_DENIES` | a política confiável nega o plano da licença |
| `E_FS_APPROVAL_MISSING` | a política confiável exige aprovação e a licença não traz |
| `E_FS_QUARANTINE_BYPASSED` | apagar de verdade o que a política manda para a quarentena |
| `E_FS_FACTS_CONTRADICT_PLAN` | o disco contradiz o declarado de um jeito que muda o veredito |
| `E_FS_ACTION_ID_UNUSABLE` | o `action_id` não serve de nome de pasta |
| `E_FS_DESTINATION_TAKEN` | a pasta de quarentena da ação, ou um lugar dentro dela, já existe |
| `E_FS_NOT_EMPTY` | `fs.delete` de pasta com conteúdo |
| `E_FS_MANIFEST_INVALID` | manifesto ausente, malformado ou que não casa com a pasta |
| `E_FS_RESTORE_PLACE_TAKEN` | o lugar de origem de um item está ocupado |
| `E_FS_NOTHING_TO_RESTORE` | nenhum item do manifesto está na quarentena |
| `E_FS_CHANGED` | a entrada mudou entre a conferência e a alteração |
| `E_FS_TOO_DEEP` | árvore mais funda que `max_depth` |
| `E_FS_IO` | o sistema operacional recusou a operação |

## O que não é executado

Recusado inteiro, por falta de definição (o caminho mais restritivo): alvo com curinga; `fs.overwrite`; `fs.clean_untracked`; `fs.move` para destino que não seja a quarentena nem uma restauração; alvo em raiz permitida fora da raiz de trabalho; caminho que só difere do disco por caixa fora do ASCII.

## Limites conhecidos

- **Janela entre a última conferência e a chamada do sistema.** A conferência e a alteração são chamadas separadas, por caminho. Fechar essa janela pede operações relativas a um identificador aberto (`openat`, `NtSetInformationFile`), que exigem `unsafe` ou dependência nova.
- **Mover não é "criar se não existe" atômico.** O lugar é conferido vago logo antes do `rename`. Na quarentena a pasta da ação é exclusiva e nova; na restauração, quem criar algo no lugar de origem nesse intervalo pode ser sobrescrito pelo `rename` (arquivo; no Unix também pasta vazia).
- **Repositório filho.** Só o `.git` no nível do alvo é conferido; um repositório mais fundo na árvore não barra a ação.
- **Raiz de trabalho.** A cerca é a fronteira confiável; a raiz de trabalho continua vindo do plano (limitada pela cerca).

## Testes

Todo teste que toca o disco está em `tests/` e trabalha numa pasta própria sob `CARGO_TARGET_TMPDIR` (`<tmp>/ads-fs/<teste>-<pid>-<nanos>-<n>/{fence,outside}`), criada por ele e nunca removida. A cerca é `fence`; `outside` é a irmã para onde os links apontam, com uma sentinela que tem de sobreviver. A "pasta do usuário" é o texto `<fence>/fake-home`. A junção do Windows é criada com `mklink /J` (sem `unsafe`); onde o sistema recusa criar link simbólico, o teste avisa e é pulado, e a junção cobre o caso.
