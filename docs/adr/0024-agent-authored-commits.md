# 0024. Autoria de commits por agentes

## Decisão

O fluxo padrão usa `ai_attribution = "agent-author"`. A conta do agente é autora e committer, o mantenedor é o único coautor e o `Signed-off-by` usa a identidade do autor. A [ADR 0011](0011-single-ai-co-author-per-commit.md) continua valendo quanto ao limite de um coautor e à validação de trailers.

## Consequências

Os PRs desse fluxo são juntados com merge commit para preservar os commits originais. A configuração do repositório precisa permitir esse método.

## Condição de reversão

Se o mantenedor voltar a figurar como autor dos novos commits, altere `ai_attribution` para `"executor"` em `defaults/devflow.toml`.
