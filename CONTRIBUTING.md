# Como contribuir

## Contribuição externa: fechada por ora

O projeto está em construção e a licença não concede direitos de uso, cópia ou modificação (ver [LICENSE](LICENSE)). Por isso **não aceitamos pull requests nem issues de terceiros por enquanto**. Isso muda quando o mantenedor abrir contribuição e atualizar este arquivo. Relatos de segurança seguem o [SECURITY.md](SECURITY.md).

O restante deste documento descreve o fluxo do mantenedor e dos agentes que trabalham sob instrução dele.

## Fluxo de branches

`feature/<nº-da-issue>-<assunto>` (ou `fix/...`) → PR → `develop` → `homolog` → `main`.

| Branch | Modo expresso (agora) | Modo completo (depois) |
|---|---|---|
| `develop` | É a produção. PR obrigatório, check `ci-ok` verde, junção por rebase (automática), sem exigir branch atualizada, sem force-push; vale também para o administrador | igual |
| `homolog` | Existe, protegida e parada | Recebe PR da `develop` |
| `main` | Existe, protegida e parada | Recebe PR da `homolog` |

Sair do modo expresso é decisão do mantenedor, registrada no parâmetro `profile` de `defaults/devflow.toml`. As regras de proteção estão em [docs/rulesets.md](docs/rulesets.md).

## Uma issue por entrega

Todo trabalho começa numa issue (modelo `task`), aberta antes do código: título curto em inglês, objetivo, critério de aceite e princípios tocados (P1 a P8, ver `AGENTS.md`). Miudezas da mesma entrega viram itens de checklist da issue, não issues novas. A issue é pública: nada de dados pessoais, de empregador, caminhos locais ou segredos.

O PR aponta para a issue com `Closes #n`. A issue só é dada por pronta depois de conferida em produção.

## Commits

Formato de commit (Conventional Commits, em inglês):

```
feat(core): add event envelope

Signed-off-by: Nome <12345+usuario@users.noreply.github.com>
Co-authored-by: Nome do agente <noreply@exemplo.org>
```

- Assunto de até 50 caracteres, em minúsculas, imperativo, sem ponto final. Tipos: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`, `build`, `ci`, `style`, `revert`.
- **Sem corpo descritivo.** Só o assunto, uma linha em branco e os trailers. A explicação vai na descrição do PR.
- Trailers só da lista permitida: `Signed-off-by` (obrigatório) e, no máximo, um `Co-authored-by`.
- O e-mail do autor é o endereço noreply do GitHub (`ID+usuario@users.noreply.github.com`).
- Cada commit é uma entrega completa (contrato, código e teste juntos). Correção de revisão entra como commit novo; nada de `amend` em commit já enviado nem de force-push.

Todas essas regras e seus valores ficam em `defaults/devflow.toml` e são aplicadas pelo mesmo script no hook local e no CI (`.github/scripts/check-commits.sh`).

### DCO e instrução permanente aos agentes

Cada commit leva `Signed-off-by`, atestando o [Developer Certificate of Origin 1.1](https://developercertificate.org/). **O mantenedor deu instrução permanente: agentes de IA que commitam por ordem dele assinam o `Signed-off-by` em nome do mantenedor**, como parte do trabalho dirigido e revisado por ele. O hook acrescenta a linha com a identidade git configurada.

### Coautor de IA

O hook acrescenta o `Co-authored-by` a partir da variável de ambiente `ADS_AI_AUTHOR`:

| Valor | Efeito |
|---|---|
| `claude` | acrescenta o coautor definido em `coauthor.claude` |
| `codex` | acrescenta o coautor definido em `coauthor.codex` |
| vazio ou ausente | nenhum coautor neste commit |

Regras: um coautor por commit, o agente que escreveu a maior parte, alternando conforme quem trabalhou. A linha de atribuição que o Codex injeta por conta própria é barrada. Para parar de atribuir no futuro, troque `ai_attribution` para `"none"` em `defaults/devflow.toml`; o histórico não é reescrito.

### Ligar o hook e testar

```bash
git config core.hooksPath .githooks
bash .github/scripts/check-commits.test.sh
```

## Pull requests

- Título no mesmo padrão do commit. Descrição em português: o que mudou, por que, como testar, checklist P1 a P8 e `Closes #n`.
- Junção por rebase, com auto-merge ligado, depois do check `ci-ok` verde. Apague a branch após a junção.
- Antes de juntar, confira o `ci-ok` da execução mais recente do commit exato.

## Segredos e dados

Nunca commite segredos, `.env`, chaves, bancos, logs ou dados de usuário. O CI varre segredos (gitleaks) e uma lista de termos proibidos mantida fora do repo.
