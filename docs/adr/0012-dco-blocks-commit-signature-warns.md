# 0012. DCO and format block, commit signature warns

Status: aceita em 14/11/2026

## Contexto
Três mecanismos diferentes estavam em jogo: a linha DCO, a assinatura criptográfica do commit e a assinatura do instalador. A assinatura do commit exige chave configurada e disponível para os agentes, e a junção por rebase pela interface recria os commits sem o selo.

A [ADR 0024](0024-agent-authored-commits.md) passou a usar merge commit no fluxo de autoria de agente e preserva os commits originais. A decisão continua valendo porque a assinatura ainda depende de chave configurada.

## Decisão
"(a) DCO e formato bloqueiam já; assinatura só avisa até a chave existir".

## Consequências
- O job `commits` bloqueia por formato, tamanho, ausência de corpo, trailers fora da lista, falta de `Signed-off-by` e e-mail de autor fora do padrão noreply do GitHub.
- A conferência de assinatura roda sobre os commits do PR e só emite aviso.
- O `Signed-off-by` usa a identidade do autor, inclusive no modo de autoria de agente da [ADR 0024](0024-agent-authored-commits.md).

## Condição de reversão
A assinatura passa a bloquear quando a chave estiver configurada e dez PRs seguidos passarem sem aviso.
