# Proteção das branches

Regras de proteção das branches `develop`, `homolog` e `main`, **como estão aplicadas no repositório**. Este documento descreve o estado real e guarda os comandos; mudar a proteção é decisão do mantenedor. Os comandos usam o `gh` e a API de rulesets do GitHub; troque `<dono>/<repo>` pelo repositório.

Estado conferido em 15/11/2026 com os comandos de leitura da seção [Conferir](#conferir). Se a resposta deles divergir deste texto, o que vale é a resposta, e este arquivo tem de ser corrigido.

## Resumo

Há **um** ruleset, `protected-branches-base`, ativo, que cobre as três branches com as mesmas regras:

| Regra | Valor aplicado |
|---|---|
| Branches | `develop`, `homolog` e `main` |
| Pull request | obrigatório; nenhum aprovador exigido (mantenedor único) |
| Check obrigatório | `ci-ok`; **sem** exigir branch atualizada; não cobrado na criação da branch |
| Exclusão da branch | bloqueada |
| Force-push | bloqueado |
| Forma de junção | o ruleset admite `rebase` e `merge`; o repositório só deixa ligado o rebase (`allow_merge_commit` e `allow_squash_merge` desligados), então na prática a junção é só por rebase |
| Exceção (bypass) | o papel de **administrador do repositório**, sempre |

A exceção do administrador é decisão do mantenedor, de 15/11/2026, para ele poder fazer push direto quando precisar. Ela **diverge da ADR 0014**, que registra a regra valendo também para o administrador; a ADR não foi alterada. A exceção é do mantenedor: agentes nunca a usam, seguem sempre por PR com `ci-ok` verde, sem `--admin` e sem push direto (ver `AGENTS.md`).

No modo expresso (`profile = "express"` em `defaults/devflow.toml`) a `develop` é a produção; `homolog` e `main` existem, sob a mesma proteção, e ficam sem uso. Elas não estão travadas contra atualização: um PR com `ci-ok` verde entraria. Sair do modo expresso é religar as promoções develop → homolog → main; a proteção já serve. Gatilho para exigir "branch atualizada": a `develop` ficar vermelha duas vezes por PRs incompatíveis entre si.

## Conferir

Só leitura. Foram estes os comandos executados na conferência de 15/11/2026:

```bash
# lista os rulesets, com o id de cada um
gh api repos/<dono>/<repo>/rulesets

# detalhe de um ruleset: regras, branches cobertas e exceções
gh api repos/<dono>/<repo>/rulesets/<id>

# regras que valem de fato para uma branch
gh api repos/<dono>/<repo>/rules/branches/develop

# configurações do repositório que completam a proteção
gh api repos/<dono>/<repo> -q '{default_branch,allow_rebase_merge,allow_squash_merge,allow_merge_commit,allow_auto_merge,delete_branch_on_merge,security_and_analysis}'

# relato privado de vulnerabilidade
gh api repos/<dono>/<repo>/private-vulnerability-reporting
```

O que eles responderam:

- um ruleset só, `protected-branches-base`, `enforcement: active`, com o conteúdo da seção [Ruleset aplicado](#ruleset-aplicado);
- `rules/branches/develop` e `rules/branches/homolog`: `deletion`, `non_fast_forward`, `pull_request` e `required_status_checks`;
- repositório: `default_branch: develop`, `allow_rebase_merge: true`, `allow_squash_merge: false`, `allow_merge_commit: false`, `allow_auto_merge: true`, `delete_branch_on_merge: true`; `secret_scanning` e `secret_scanning_push_protection` ligados;
- relato privado de vulnerabilidade: ligado;
- a proteção de branch clássica (`gh api repos/<dono>/<repo>/branches/develop/protection`) responde 404: não há; tudo vem do ruleset.

## Ruleset aplicado

Transcrição do que a API devolve para o ruleset (sem os campos de identificação e de data), no formato que a criação aceita:

```json
{
  "name": "protected-branches-base",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [
    { "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }
  ],
  "conditions": {
    "ref_name": {
      "include": ["refs/heads/develop", "refs/heads/homolog", "refs/heads/main"],
      "exclude": []
    }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "required_reviewers": [],
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "require_extra_approval_for_unattributed_changes": true,
        "allowed_merge_methods": ["rebase", "merge"]
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": false,
        "do_not_enforce_on_create": true,
        "required_status_checks": [ { "context": "ci-ok" } ]
      }
    }
  ]
}
```

Como ler:

- `bypass_actors`: `RepositoryRole` com `actor_id` 5 é o papel de administrador do repositório; `always` vale para push direto e para junção de PR. Lista vazia faria a regra valer também para o administrador.
- `strict_required_status_checks_policy: false` é o "sem exigir branch atualizada".
- `do_not_enforce_on_create: true` deixa criar a branch sem o check.
- `allowed_merge_methods` lista `merge` além de `rebase`, mas o commit de merge está desligado nas configurações do repositório.

## Recriar ou mudar

Estes comandos **não foram executados** na conferência de 15/11/2026 (ela só leu). O corpo é a transcrição acima; rode e confira a resposta da API.

```bash
# recriar em outro repositório: salve o JSON acima em ruleset.json
gh api -X POST repos/<dono>/<repo>/rulesets --input ruleset.json

# mudar o que existe (o PUT substitui o ruleset inteiro)
gh api -X PUT repos/<dono>/<repo>/rulesets/<id> --input ruleset.json

# configurações do repositório
gh api -X PATCH repos/<dono>/<repo> \
  -F default_branch=develop \
  -F allow_rebase_merge=true \
  -F allow_squash_merge=false \
  -F allow_merge_commit=false \
  -F allow_auto_merge=true \
  -F delete_branch_on_merge=true

# varredura de segredos e push protection (segunda camada depois do gitleaks)
gh api -X PATCH repos/<dono>/<repo> --input - <<'JSON'
{"security_and_analysis":{"secret_scanning":{"status":"enabled"},"secret_scanning_push_protection":{"status":"enabled"}}}
JSON

# relato privado de vulnerabilidade (o SECURITY.md depende disto)
gh api -X PUT repos/<dono>/<repo>/private-vulnerability-reporting
```

Ordem num repositório novo: primeiro push, criar `develop`, `homolog` e `main` a partir do mesmo commit, configurações do repositório, ruleset, conferência. `default_branch` só pode apontar para uma branch que já exista no remoto. O check `ci-ok` só pode ser escolhido como obrigatório depois de ter rodado ao menos uma vez; se a API recusar, abra um PR de teste antes.

Para tirar a exceção do administrador, envie o ruleset com `"bypass_actors": []`.

## Desfazer

Também não executados na conferência.

```bash
gh api -X DELETE repos/<dono>/<repo>/rulesets/<id>
```

Para desligar sem apagar, troque `enforcement` para `disabled` com `gh api -X PUT`.

## Segredo do job `terms`

```bash
gh secret set FORBIDDEN_TERMS --repo <dono>/<repo> < lista-de-termos.txt
```

A lista fica fora do git. Uma regra por linha; `#` comenta; o prefixo `re:` marca expressão regular. O job `terms` só roda com a variável de repositório `TERMS_CHECK = on`; ligado e sem o segredo, ele reprova (falha fechada).
