# ads-core

Política do produto (tarefas, aprovações, escopo, roteamento e orçamento), sem rede, banco nem PTY.

## Portão de ações destrutivas (`src/gate/`)

Nenhuma ação sobre o sistema de arquivos é executada sem passar por aqui. O portão não toca o disco: recebe o plano como dado e responde se, e como o quê, ele pode rodar. Os tipos só se encaixam de um jeito: o pedido só vira plano pela normalização, o veredito só sai de `classify`, a ação só nasce de um veredito que não seja "negado" e a licença de execução só sai de uma ação cujo hash do plano e cuja aprovação conferem.

| Arquivo | O que faz |
|---|---|
| `path.rs` | Normalização de caminhos absolutos, Windows e POSIX, só por texto (sem `std::path`, para julgar igual em qualquer sistema). |
| `plan.rs` | Plano de ação (operação, alvos, raiz de trabalho, estimativa), a serialização canônica e o `plan_hash` (SHA-256). |
| `policy.rs` | Parâmetros do portão e as pastas protegidas. Os padrões ficam só em [`defaults/action-gate.toml`](../../defaults/action-gate.toml). |
| `classify.rs` | As regras: `plano + política -> veredito` (permitido, exige aprovação, negado) e a reescrita de apagar como mover para a quarentena. |
| `machine.rs` | Máquina de estados da ação e a licença de execução. Cada transição devolve o evento do contrato. |
| `event.rs` | Tipos dos eventos `action.*` e `approval.*`, escritos à mão e conferidos contra os dourados de `contracts/golden/`. Provisório: sai quando o `ads-contracts` tiver os tipos gerados. |

### Normalização

O plano só existe com todos os caminhos absolutos e normalizados (separadores repetidos e `.` retirados, letra da unidade em maiúscula). Um caminho recusado invalida o plano inteiro; nada é consertado. São recusados:

- caminho relativo, inclusive `C:pasta` e `\pasta`;
- qualquer componente `..` (resolver por texto dá o caminho errado quando há um link no meio);
- variável ou til não expandidos: qualquer `$` ou `%`, e caminho ou componente começando com `~`;
- curinga (`*`, `?`) fora do último componente, e em qualquer lugar da raiz de trabalho ou do destino;
- caminho de rede (`\servidor\pasta`, `//servidor/pasta`) e os prefixos `\?\` e `\.\`;
- no Windows, o que é apelido de outro caminho: nome curto (`NOME~1`), componente terminado em ponto ou espaço, `:` depois da unidade, nomes de dispositivo (`NUL`, `COM1`) e `< > " |`;
- caractere de controle; `\` dentro de componente POSIX; lista de alvos vazia.

Alvo com curinga é julgado como a pasta sobre a qual o curinga age: `pasta/*` vale como `pasta`.

### Regras

Um alvo negado nega o plano inteiro. Comparação por componente (`proj2` não está dentro de `proj`); sem diferenciar maiúsculas no Windows e, nas regras de negação, em nenhum dos dois estilos.

| Veredito | Quando |
|---|---|
| Negado | alvo que é raiz de unidade ou `/`; a pasta do usuário ou um ancestral dela; a pasta dos perfis ou o perfil de qualquer usuário; pasta do sistema (ou algo dentro dela); a raiz de trabalho ou algo que a contenha; qualquer coisa que não esteja estritamente dentro da raiz de trabalho ou de uma raiz permitida; raiz de repositório (informada no plano); a pasta de quarentena ou algo que a contenha |
| Negado | qualquer caminho escrito no estilo que não é o da pasta do usuário (`/c/...` contra `C:\...`): pode ser outra grafia de uma pasta protegida, e o texto sozinho não resolve isso |
| Negado | raiz de trabalho que é ela mesma uma pasta protegida; destino de "mover" protegido, fora das raízes permitidas ou dentro de um dos alvos |
| Exige aprovação | apagar recursivo ou limpar não rastreados acima do limite de itens ou de bytes, ou sem estimativa; qualquer ação (sobrescrita inclusive) em raiz permitida fora da raiz de trabalho; "mover" com destino fora da raiz de trabalho; apagar algo que já está na quarentena |
| Permitido | o restante dentro da raiz de trabalho |

Os três níveis casam um a um com `urn:ads:common:risk:1`: permitido é `low`, exige aprovação é `medium`, negado é `high`.

Quarentena por padrão: apagar e apagar recursivo saem do veredito reescritos como "mover para `<raiz de trabalho>/<quarantine_dir>/<16 primeiros hexadecimais do hash do plano pedido>`". É o plano reescrito que recebe o `plan_hash`, a aprovação e a execução.

### `plan_hash`

SHA-256 do texto canônico do plano: uma linha por campo, em ordem fixa (versão do formato, verbo, raiz de trabalho, destino, estimativa, quantidade de alvos, alvos em ordem e sem repetição), cada caminho precedido do seu tamanho em bytes. A ordem e a grafia dos alvos no pedido não mudam o hash; trocar um alvo, a operação, a raiz, o destino, a estimativa ou a marca de raiz de repositório muda.

### Máquina de estados

```
planejada ──(permitido)──────────────────────────────┐
    │                                                ├─ licença ─> executando ─> executada | falhou
    └─(exige aprovação)─> aprovação pedida ─> concedida ┘
                                │
                                └─> negada | expirada
```

Cada transição produz um evento (`action.planned`, `approval.requested`, `approval.granted`, `approval.denied`, `approval.expired`, `action.executed`, `action.failed`); `id`, instante, `seq`, autor, causa e classe do dado vêm de quem chama, porque o núcleo não tem relógio nem contador. Veredito negado não cria ação nem evento. São recusados, entre outros: licença sem a aprovação que o veredito exigia; plano, aprovação ou licença de outro hash; aprovação de outro `approval_id`; conceder ou executar depois da validade; expirar antes dela; segunda licença para a mesma ação; qualquer comando em estado final.
