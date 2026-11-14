# 0017. Date-and-commit versioning during the express period

Status: aceita em 14/11/2026 (AR-17)

## Contexto
O release clássico acontece na branch principal, que fica parada durante o período expresso. O programa ainda precisa dizer qual versão é, para a conferência depois de cada atualização.

## Decisão
"(a) Sem ferramenta de release agora: versão pela data e pelo commit; release formal ao sair do expresso".

## Consequências
- Cada build se identifica como `dev-<data>-<commit curto>` e é publicado como pré-lançamento; ficam guardados os dez mais recentes.
- O endereço de saúde do núcleo informa o commit, que é o que o atualizador confere (ADR 0015).
- Não há lista de mudanças gerada automaticamente neste período; o histórico de PRs faz esse papel.

## Condição de reversão
Ligar a ferramenta de release, a numeração semântica e a lista de mudanças no primeiro instalador entregue a outra pessoa, ou quando o mantenedor encerrar o período expresso.
