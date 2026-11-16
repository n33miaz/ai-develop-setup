# ads-core

Política do produto (tarefas, aprovações, escopo, roteamento e orçamento), sem rede, banco nem PTY.

## Portão de ações destrutivas (`src/gate/`)

Nenhuma ação sobre o sistema de arquivos é executada sem passar por aqui. O portão não toca o disco: recebe o plano como dado e responde se, e como o quê, ele pode rodar. Os tipos só se encaixam de um jeito: o pedido só vira plano pela normalização, o veredito só sai de `classify`, a ação só nasce de um veredito (a de um veredito negado já nasce no estado final) e a licença de execução só sai de uma ação cujo hash do plano e cuja aprovação conferem. O que o portão escreve (eventos e o registro do plano) segue os contratos de [`contracts/`](../../contracts/README.md#portão-de-ações), e o estado de uma ação se reconstrói só desses eventos.

| Arquivo | O que faz |
|---|---|
| `path.rs` | Normalização de caminhos absolutos, Windows e POSIX, só por texto (sem `std::path`, para julgar igual em qualquer sistema). |
| `plan.rs` | Plano de ação (operação, alvos na ordem canônica, raiz de trabalho, estimativa) e o `plan_hash`. |
| `policy.rs` | Parâmetros do portão e as pastas protegidas. Os padrões ficam só em [`defaults/action-gate.toml`](../../defaults/action-gate.toml). |
| `classify.rs` | As regras e os seus códigos: `plano + política -> veredito` (permitido, exige aprovação, negado), a reescrita de apagar como mover para a quarentena e o registro do plano julgado. |
| `machine.rs` | Máquina de estados da ação, a licença de execução e a reconstrução do estado a partir dos eventos (`replay`). Cada transição devolve o evento do contrato. |
| `event.rs` | Todos os tipos de contrato do portão num módulo só: os eventos `action.*` e `approval.*`, os registros `record.action-plan` e `record.approval` (este só para leitura), os tipos comuns que eles usam e a serialização canônica do plano. Escritos à mão e conferidos contra os dourados de `contracts/golden/`; saem quando o `ads-contracts` tiver os tipos gerados dos Schemas (outra entrega). |

### Normalização

O plano só existe com todos os caminhos absolutos e normalizados (separadores repetidos e `.` retirados, letra da unidade em maiúscula). Um caminho recusado invalida o plano inteiro; nada é consertado. São recusados:

- caminho relativo, inclusive `C:pasta` e `\pasta`;
- qualquer componente `..` (resolver por texto dá o caminho errado quando há um link no meio);
- variável ou til não expandidos: qualquer `$` ou `%`, e caminho ou componente começando com `~`;
- curinga (`*`, `?`) fora do último componente, e em qualquer lugar da raiz de trabalho ou do destino;
- caminho de rede (`\servidor\pasta`, `//servidor/pasta`) e os prefixos `\?\` e `\.\`;
- no Windows, o que é apelido de outro caminho: nome curto (`NOME~1`), componente terminado em ponto ou espaço, `:` depois da unidade, nomes de dispositivo com qualquer extensão (`NUL`, `nul.txt`, `COM0` a `COM9`, `LPT0` a `LPT9` e os mesmos com `¹`, `²` ou `³`) e `< > " |`;
- caractere de controle; `\` dentro de componente POSIX; lista de alvos vazia.

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

**O veredito é fechado depois de julgar a reescrita.** Quando o apagar vira mover para a quarentena, o plano reescrito, com o destino gerado, passa por todas as regras de um "mover" pedido, antes de o veredito sair; o veredito guarda os achados dos dois (cada um uma vez) e fica com o nível mais restritivo. Se o destino gerado for negado, o plano inteiro é negado e não é reescrito. Daí a propriedade que um teste cobra: reclassificar `veredito.plan()` dá o mesmo nível e não acha nada que o veredito já não traga.

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

Todo veredito vira um `urn:ads:record:action-plan:2` (`Verdict::record`, devolvido por `Action::plan`; a `:1` é só lida, por `ActionPlanRecordV1`): a operação pedida, os alvos na ordem canônica, a raiz de trabalho, a estimativa, o veredito com os códigos, `quarantined` e `destination` (a pasta de quarentena quando o apagar foi reescrito; o destino pedido em "mover"). O registro só existe se cumprir o contrato inteiro, inclusive a ordem dos alvos e o `plan_hash` recalculado dos seus próprios campos; isso vale tanto para o que o núcleo monta quanto para o que ele lê. Plano que não cabe nos limites do contrato (caminho com mais de 4096 caracteres, curinga com mais de 255, mais de 10000 alvos ou de 1000 regras disparadas, estimativa de itens ou de bytes acima de 2^53 − 1) não tem registro e, por isso, não vira ação nem evento.

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
