# 0019. Descriptive issue and pull request titles

Status: aceita em 15/11/2026

## Contexto
O modelo de tarefa obrigava o título da issue a seguir o padrão dos commits (`tipo(escopo): assunto`, até 50 caracteres, minúsculas), e as issues ficavam com cara de commit. A ADR 0016 pede só "título curto em inglês". O que se apurou sobre o costume:

- A [documentação do GitHub](https://docs.github.com/en/issues/tracking-your-work-with-issues/learning-about-issues/quickstart) pede título descritivo para a issue.
- A especificação [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) trata de mensagens de commit; não fala de issues.
- Os guias de projetos abertos pedem que o título da issue resuma o problema ou o resultado esperado, curto e específico, porque é ele que aparece em listas, buscas e referências cruzadas.
- O prefixo de tipo é usado nos commits e, em muitos times, no título do PR, principalmente quando o PR vira o commit (junção por squash) ou alimenta automação de release.
- A classificação da issue se faz por rótulo e por tipo de issue, não pelo título.

Aqui a junção é por rebase (ADR 0014): os commits do PR entram um a um, o título do PR não vira commit e não há automação de release que leia títulos (ADR 0017).

Com a [ADR 0024](0024-agent-authored-commits.md), o título do PR aparece no corpo do commit de merge no fluxo de autoria de agente. A convenção continua valendo, pois os commits originais preservam seus assuntos e a publicação de pré-lançamentos usa o SHA, conforme a [ADR 0025](0025-semver-dev-prereleases-until-first-installer.md).

## Decisão
Cada coisa com a sua convenção:

- **Issue:** título descritivo do problema ou do resultado, em inglês, no imperativo ou como frase nominal, com inicial maiúscula, sem prefixo `tipo(escopo):`, sem ponto final, até 72 caracteres. Exemplos: "Prevent deletions outside the workspace", "Terminal host loses output when the client reconnects". A classificação vai em dois rótulos: um de tipo (`type: ...`) e pelo menos um de área (`area: ...`), listados no `CONTRIBUTING.md`.
- **Commit:** não muda. Conventional Commits com as regras de `defaults/devflow.toml`, aplicadas por `.github/scripts/check-commits.sh`.
- **Pull request:** título descritivo, na mesma regra da issue (em geral o mesmo título), com `Closes #n` no corpo.

## Alternativas descartadas
- **Manter o prefixo de commit na issue.** Repete no título o que o rótulo já diz, gasta o espaço útil com `tipo(escopo):`, força o problema a caber em 50 caracteres de minúsculas e descreve a issue pela mudança de código, quando ela descreve um problema ou um resultado (uma issue pode render vários commits de tipos diferentes).
- **Prefixo só no título do PR.** Só se paga quando o título do PR vira o commit (squash) ou alimenta lista de mudanças automática. Com junção por rebase nenhum dos dois acontece: seria uma regra sem leitor, e uma segunda cópia do tipo que já está em cada commit.

## Consequências
- O modelo de tarefa deixa de sugerir título em formato de commit e passa a pedir tipo e área; quem abre a issue aplica os dois rótulos (um formulário de issue não aplica rótulo a partir de um campo).
- As issues já abertas e fechadas foram renomeadas e rotuladas; títulos de PRs já juntados ficam como estão.
- O título descritivo não é conferido por máquina: vale a revisão. O checador de commits continua cuidando só de commits.
- A ADR 0016 continua valendo; esta só detalha o "título curto em inglês" dela.

## Condição de reversão
Voltar a exigir o prefixo de tipo no título do PR se a junção passar a ser por squash ou se uma ferramenta de release passar a ler títulos de PR (ver ADR 0017). Criar uma conferência automática do título se três issues seguidas saírem fora da convenção.
