# 0024. Agent-authored commits

Status: aceita em 19/11/2026; substitui em parte a [ADR 0011](0011-single-ai-co-author-per-commit.md) na atribuição de autoria e a [ADR 0014](0014-pull-request-with-auto-merge-into-develop.md) na forma de junção dos PRs com commits de autoria de agente

## Contexto

O fluxo precisa registrar quem escreveu cada commit e preservar essa autoria ao juntar o PR.

## Decisão

O fluxo padrão usa `ai_attribution = "agent-author"`. A conta do agente é autora e committer, o mantenedor é o único coautor e o `Signed-off-by` usa a identidade do autor. A [ADR 0011](0011-single-ai-co-author-per-commit.md) continua valendo quanto ao limite de um coautor e à validação de trailers.

## Consequências

Os PRs desse fluxo são juntados com merge commit para preservar os commits originais. A configuração do repositório precisa permitir esse método.

## Condição de reversão

Se o mantenedor voltar a figurar como autor dos novos commits, altere `ai_attribution` para `"executor"` em `defaults/devflow.toml`.
