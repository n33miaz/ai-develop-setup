# ads-adapter-api

Adapta as chamadas diretas às APIs de provedores ao contrato do núcleo. A chave de API, quando existir, fica no cofre do produto ([ADR 0010](../../../docs/adr/0010-api-key-for-automation-agy-headless-on-demand.md)).

Estado: só lógica. Não há rede, e o adaptador nunca lê, guarda nem registra uma chave: o pedido leva um `KeyHandle`, a referência `<provedor>/<apelido>` do apelido ativo, que o transporte de rede (outra entrega) resolve no cofre. Os erros e o `Debug` de configuração, pedido, resposta, resultado da chamada e `KeyHandle` não mostram texto de conversa, corpo de resposta nem referência de chave.

## O que há aqui

| Parte | Trabalho |
|---|---|
| `config` | Lê [`defaults/api-adapter.toml`](../../../defaults/api-adapter.toml) mais a configuração do usuário e valida: provedores por apelido (URL base HTTPS, modelo, nomes de cabeçalho, preços, versão da tabela de preços, apelidos de chave). |
| `keys` | Valida os apelidos de chave de um provedor e diz qual está ativo. |
| `chat` | Monta o corpo do formato Chat Completions e o devolve (`build_request`, `parse_request_body`); lê a resposta (`parse_response`). Os erros não levam texto do pedido nem da resposta. |
| `transport` | A porta de transporte (`Transport`): recebe o pedido pronto e devolve status e corpo, ou diz se o pedido com certeza não saiu ou se pode ter chegado ao provedor. Não há implementação de rede. |
| `call` | O fluxo de uma chamada (`run_call`): monta, reserva, envia uma vez e liquida. |
| `spend` | A guarda de gasto. |
| `usage` | O registro `urn:ads:record:api_usage:1` de uma chamada: alias do provedor, modelo, tokens e custo estimado em micro-dólares, sem conteúdo. O chamador o amplia para o `usage_event` do livro-caixa. |

## Fluxo de uma chamada

`run_call` recebe o transporte, a guarda de gasto (num `Mutex`), o provedor, o pedido, o mês e o instante da chamada. A guarda fica travada só para reservar e para liquidar, nunca durante o envio: uma chamada lenta não segura as outras, e as reservas em aberto contam para o teto.

| Como termina (`result`) | Envio | Gasto | Registro |
|---|---|---|---|
| Pedido inválido (`Request`), instante ou provedor que não cabem no registro ou instante fora do mês informado (`Record`), recusa da guarda (`Refused`) | nenhum | nada reservado | não |
| O transporte garante que não saiu (`NotSent`) | uma tentativa | reserva devolvida | não |
| Falha em que o pedido pode ter chegado (`Uncertain`), status fora de 2xx (`Status`), corpo grande demais ou ilegível (`Response`) | um | a reserva | `reserved` |
| Sucesso com a contagem de tokens | um | o uso informado, mesmo acima da reserva | `reported` |
| Sucesso sem a contagem de tokens | um | a reserva | `reserved` |

- O mês e o instante vêm do mesmo momento, e o instante (UTC) tem de cair no mês: o livro-caixa soma pelo mês do registro, que precisa ser o da reserva.
- O resultado traz sempre o gasto do mês corrente da guarda logo após a chamada (sem as reservas de outras chamadas ainda abertas, nem o custo de uma chamada reservada num mês que a guarda já deixou) e, quando há custo, o registro que o chamador grava no livro-caixa, com ou sem falha.
- Não há repetição nem troca de chave. Quem implementa `Transport` não segue redirecionamento, não repete e não lê o corpo além do necessário para saber que passou de `MAX_RESPONSE_BYTES`; um corpo cortado no limite nunca é devolvido como inteiro.
- Um panic no transporte atravessa o fluxo: a reserva continua presa na guarda até ela passar para um mês posterior e não há registro, então uma guarda nova, montada a partir do livro-caixa, não conhece esse valor.
- A guarda dentro do `Mutex` não deve ser substituída com chamadas abertas. Se for, a chamada que teria custo termina em `GuardReplaced`, sem o texto da resposta, com o registro e com o gasto da guarda nova, que não inclui esse custo.
- O registro `urn:ads:record:api_usage:1` não tem campo de desfecho: uma falha cobrada e um sucesso sem contagem de tokens produzem o mesmo registro (`reserved`, tokens zero). Quem precisa distinguir usa o `result`.

## Guarda de gasto

- Desligada por padrão (`enabled = false`). Ligada sem `monthly_cap_usd`, recusa toda chamada (`NoCap`).
- Antes de cada chamada reserva o pior caso: o tamanho do corpo conta como tokens de entrada e `max_tokens` como saída, ao preço do provedor. A chamada só sai se esse valor cabe no que sobra do teto; com o teto atingido, recusa (`CapReached`).
- Ao fim, `settle` troca a reserva pelo custo real; `release` devolve a reserva de uma chamada que não chegou ao provedor. Resposta sem a contagem de tokens custa a reserva inteira (`cost_basis = reserved`), então omitir a contagem não torna a chamada de graça.
- O mês e o valor já gasto vêm do chamador (do livro-caixa). Guardar o gasto entre execuções não é papel deste crate.
- O mês corrente da guarda é o mais recente com que ela foi montada ou em que recebeu um pedido de reserva. Um mês posterior começa sem gasto e sem reservas. O custo de uma chamada é do mês que a reservou: liquidada ou devolvida depois da virada, ela não muda o mês corrente.
- Relógio voltado para trás não zera nem libera o teto: a reserva conta no mês corrente da guarda. O mesmo vale para uma chamada que tomou o mês antes da virada e reservou depois dela. Nos dois casos o registro leva o mês anterior, e o gasto da guarda fica acima do total do livro-caixa no mês corrente.
- Quem remonta a guarda passa o mês mais recente entre o do relógio e o do último registro do livro-caixa, com o total desse mês: uma guarda montada num mês anterior zera o gasto ao avançar.
- O custo é uma estimativa a preço de lista; a fatura do provedor manda.
- A guarda não conhece chaves: o teto e o gasto são do adaptador inteiro. Trocar o apelido ativo não zera nem aumenta o teto. Quem recarrega a configuração só para trocar de chave mantém a mesma guarda, porque uma guarda nova não conhece as reservas em aberto da anterior.

## Várias chaves por provedor

- Cada provedor usa a lista `key_aliases` (a própria ou a geral) e escolhe um apelido em `active_key`. Sem nada configurado, há um apelido só, `default`.
- A configuração recusa lista vazia, com repetição ou com mais de 8 apelidos, apelido fora do formato (1 a 32 letras minúsculas, dígitos, `-` e `_`, começando por letra), `active_key` fora da lista e lista com vários apelidos sem `active_key`.
- A troca é manual: muda-se `active_key` e recarrega-se a configuração. Não há rotação nem troca automática quando uma chamada falha.
- Os erros de apelido apontam o campo e não repetem o valor. A recusa de texto com cara de segredo é uma heurística (prefixos comuns de chave e trechos longos com dígitos), não uma garantia: o apelido é um nome curto, nunca a chave.

## Configuração

Todas as chaves estão documentadas em `defaults/api-adapter.toml`; a do usuário só precisa das que muda. O formato é o mesmo para qualquer provedor compatível: o que muda por provedor é a URL base, o modelo, o caminho, o cabeçalho da chave e os preços.
