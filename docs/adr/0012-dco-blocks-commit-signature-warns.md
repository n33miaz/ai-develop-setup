# 0012. DCO and format block, commit signature warns

Status: aceita em 14/11/2026 (AR-12)

## Contexto
Três mecanismos diferentes estavam em jogo: a linha DCO, a assinatura criptográfica do commit e a assinatura do instalador. A assinatura do commit exige chave configurada e disponível para os agentes, e a junção por rebase pela interface recria os commits sem o selo.

## Decisão
"(a) DCO e formato bloqueiam já; assinatura só avisa até a chave existir".

## Consequências
- O job `commits` bloqueia por formato, tamanho, ausência de corpo, trailers fora da lista, falta de `Signed-off-by` e e-mail de autor diferente do endereço privado do provedor.
- A conferência de assinatura roda sobre os commits do PR e só emite aviso.
- O `Signed-off-by` posto por um agente vale como atestado do mantenedor por instrução permanente, registrada no guia de contribuição.

## Condição de reversão
A assinatura passa a bloquear quando a chave estiver configurada e dez PRs seguidos passarem sem aviso.
