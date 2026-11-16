# 0014. Pull request with auto-merge into develop

Status: aceita em 14/11/2026

## Contexto
O fluxo é develop → homolog → main, e por enquanto a develop é a produção: o que entra nela é o que o computador do mantenedor passa a rodar.

## Decisão
"(a) PR obrigatório com o check verde e junção automática por rebase".

## Consequências
- Develop é a branch padrão: PR obrigatório, check `ci-ok`, junção automática por rebase, histórico linear, sem push forçado, regra válida também para administrador, sem revisor humano obrigatório e sem exigir branch atualizada.
- Homolog e main existem, protegidas e paradas, até o fim do período expresso.
- Branch de trabalho `feature/<issue>-<assunto>` ou `fix/<issue>-<assunto>`, apagada depois da junção.
- Antes de juntar, confere-se o resultado da execução mais recente do commit exato.

## Condição de reversão
Ligar "branch atualizada" se a develop ficar vermelha duas vezes por PRs incompatíveis; exigir revisor humano quando houver um segundo mantenedor; religar as promoções quando o mantenedor encerrar o período expresso.
