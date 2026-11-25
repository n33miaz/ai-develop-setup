# ads-adapter-api

Adapta as chamadas diretas às APIs de provedores ao contrato do núcleo. A chave de API, quando existir, fica no cofre do produto ([ADR 0010](../../../docs/adr/0010-api-key-for-automation-agy-headless-on-demand.md)).

Estado: só lógica pura. Não há rede, e o adaptador nunca lê, guarda nem registra uma chave: o pedido leva um `KeyHandle`, a referência `<provedor>/<apelido>` do apelido ativo, que o transporte (outra entrega) resolve no cofre. O `Debug` de configuração, pedido, resposta e `KeyHandle` não mostra texto de conversa nem referência de chave.

## O que há aqui

| Parte | Trabalho |
|---|---|
| `config` | Lê [`defaults/api-adapter.toml`](../../../defaults/api-adapter.toml) mais a configuração do usuário e valida: provedores por apelido (URL base HTTPS, modelo, nomes de cabeçalho, preços, versão da tabela de preços, apelidos de chave). |
| `keys` | Valida os apelidos de chave de um provedor e diz qual está ativo. |
| `chat` | Monta o corpo do formato Chat Completions e o devolve (`build_request`, `parse_request_body`); lê a resposta (`parse_response`). Os erros não levam texto do pedido nem da resposta. |
| `spend` | A guarda de gasto. |
| `usage` | O registro `urn:ads:record:api_usage:1` de uma chamada: alias do provedor, modelo, tokens e custo estimado em micro-dólares, sem conteúdo. O chamador o amplia para o `usage_event` do livro-caixa. |

## Guarda de gasto

- Desligada por padrão (`enabled = false`). Ligada sem `monthly_cap_usd`, recusa toda chamada (`NoCap`).
- Antes de cada chamada reserva o pior caso: o tamanho do corpo conta como tokens de entrada e `max_tokens` como saída, ao preço do provedor. A chamada só sai se esse valor cabe no que sobra do teto; com o teto atingido, recusa (`CapReached`).
- Ao fim, `settle` troca a reserva pelo custo real; `release` devolve a reserva de uma chamada que não chegou ao provedor. Resposta sem a contagem de tokens custa a reserva inteira (`cost_basis = reserved`), então omitir a contagem não torna a chamada de graça.
- O mês e o valor já gasto vêm do chamador (do livro-caixa); mês posterior zera o gasto, relógio voltado para trás não zera. Guardar o gasto entre execuções não é papel deste crate.
- O custo é uma estimativa a preço de lista; a fatura do provedor manda.
- A guarda não conhece chaves: o teto e o gasto são do adaptador inteiro. Trocar o apelido ativo não zera nem aumenta o teto. Quem recarrega a configuração só para trocar de chave mantém a mesma guarda, porque uma guarda nova não conhece as reservas em aberto da anterior.

## Várias chaves por provedor

- Cada provedor usa a lista `key_aliases` (a própria ou a geral) e escolhe um apelido em `active_key`. Sem nada configurado, há um apelido só, `default`.
- A configuração recusa lista vazia, com repetição ou com mais de 8 apelidos, apelido fora do formato (1 a 32 letras minúsculas, dígitos, `-` e `_`, começando por letra), `active_key` fora da lista e lista com vários apelidos sem `active_key`.
- A troca é manual: muda-se `active_key` e recarrega-se a configuração. Não há rotação nem troca automática quando uma chamada falha.
- Os erros de apelido apontam o campo e não repetem o valor. A recusa de texto com cara de segredo é uma heurística (prefixos comuns de chave e trechos longos com dígitos), não uma garantia: o apelido é um nome curto, nunca a chave.

## Configuração

Todas as chaves estão documentadas em `defaults/api-adapter.toml`; a do usuário só precisa das que muda. O formato é o mesmo para qualquer provedor compatível: o que muda por provedor é a URL base, o modelo, o caminho, o cabeçalho da chave e os preços.
