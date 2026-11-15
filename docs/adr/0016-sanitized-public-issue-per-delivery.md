# 0016. Sanitized public issue per delivery

Status: aceita em 14/11/2026 (AR-16)

## Contexto
Todo pedido que vira entrega deve nascer de uma issue. O repositório é público, e pedidos podem citar pessoas, organizações ou caminhos de máquina.

## Decisão
"(a) Issue pública sem dados pessoais, uma por entrega; o texto literal continua fora do git".

## Consequências
- Uma issue por entrega, criada antes do trabalho, com título curto em inglês, objetivo, critério de aceite e princípios tocados; miudezas viram itens da mesma issue. A forma do título e a classificação por rótulos estão na [ADR 0019](0019-descriptive-issue-and-pr-titles.md).
- Branch, PR e commits apontam para a issue; ela fecha na junção e só é dada por pronta depois da conferência em produção.
- O texto original do pedido fica fora do repositório, com o par "pedido ↔ issue" registrado lá.
- O verificador de termos proibidos cobre também título e corpo de issue e de PR.
- No produto vira parâmetro: `workflow.issue_per_task` e `workflow.issue_visibility`.

## Condição de reversão
Mover as issues para um rastreador privado se um dado pessoal ou de terceiros chegar a uma issue pública.
