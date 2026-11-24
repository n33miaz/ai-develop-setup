# ads-adapter-api

Adapta as chamadas diretas às APIs de provedores ao contrato do núcleo. A chave de API, quando existir, fica no cofre do produto ([ADR 0010](../../../docs/adr/0010-api-key-for-automation-agy-headless-on-demand.md)).

Estado: só lógica pura. Não há rede, e o adaptador nunca lê, guarda nem registra uma chave: ela chega como um `KeyHandle`, uma referência que o chamador resolve no transporte (outra entrega). O `Debug` de pedido, resposta e `KeyHandle` não mostra texto nem referência.

## O que há aqui

| Parte | Trabalho |
|---|---|
| `config` | Lê [`defaults/api-adapter.toml`](../../../defaults/api-adapter.toml) mais a configuração do usuário e valida: provedores por apelido (URL base HTTPS, modelo, nomes de cabeçalho, preços, versão da tabela de preços). |
| `chat` | Monta o corpo do formato Chat Completions e o devolve (`build_request`, `parse_request_body`); lê a resposta (`parse_response`). Os erros não levam texto do pedido nem da resposta. |
| `spend` | A guarda de gasto. |
| `usage` | O registro `urn:ads:record:api_usage:1` de uma chamada: alias do provedor, modelo, tokens e custo estimado em micro-dólares, sem conteúdo. O chamador o amplia para o `usage_event` do livro-caixa. |

## Guarda de gasto

- Desligada por padrão (`enabled = false`). Ligada sem `monthly_cap_usd`, recusa toda chamada (`NoCap`).
- Antes de cada chamada reserva o pior caso: o tamanho do corpo conta como tokens de entrada e `max_tokens` como saída, ao preço do provedor. A chamada só sai se esse valor cabe no que sobra do teto; com o teto atingido, recusa (`CapReached`).
- Ao fim, `settle` troca a reserva pelo custo real; `release` devolve a reserva de uma chamada que não chegou ao provedor. Resposta sem a contagem de tokens custa a reserva inteira (`cost_basis = reserved`), então omitir a contagem não torna a chamada de graça.
- O mês e o valor já gasto vêm do chamador (do livro-caixa); mês posterior zera o gasto, relógio voltado para trás não zera. Guardar o gasto entre execuções não é papel deste crate.
- O custo é uma estimativa a preço de lista; a fatura do provedor manda.

## Configuração

Todas as chaves estão documentadas em `defaults/api-adapter.toml`; a do usuário só precisa das que muda. O formato é o mesmo para qualquer provedor compatível: o que muda por provedor é a URL base, o modelo, o caminho, o cabeçalho da chave e os preços.
