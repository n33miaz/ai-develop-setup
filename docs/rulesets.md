# Proteção das branches

Regras de proteção das três branches no modo expresso (`profile = "express"` em `defaults/devflow.toml`). Os comandos usam o `gh` e a API de rulesets do GitHub; troque `<dono>/<repo>` pelo repositório. Este documento só descreve e guarda os comandos: aplicar é decisão do mantenedor, depois da ordem de criação do repositório abaixo.

## Resumo

| Branch | Regra no modo expresso |
|---|---|
| `develop` | É a produção. PR obrigatório; check `ci-ok` obrigatório; junção só por rebase; **sem** exigir branch atualizada; sem force-push; sem apagar; vale também para o administrador (ninguém tem bypass); nenhum aprovador exigido (mantenedor único) |
| `homolog` | Protegida e parada: sem criar, atualizar, apagar nem force-push; sem bypass |
| `main` | Igual à `homolog` |

Sair do modo expresso é trocar as regras de `homolog` e `main` para PR obrigatório com `ci-ok` e religar as promoções develop → homolog → main. Gatilho para exigir "branch atualizada" na `develop`: ela ficar vermelha duas vezes por PRs incompatíveis entre si.

## Ordem

1. Crie o repositório, faça o primeiro push e crie as branches `develop`, `homolog` e `main` a partir do mesmo commit (as regras de `homolog` e `main` impedem criar e atualizar, então elas têm de existir antes).
2. Ajuste as configurações do repositório (comando 1).
3. Aplique os três rulesets (comandos 2 a 4).
4. Confira com `gh api repos/<dono>/<repo>/rulesets`.

O check `ci-ok` só pode ser escolhido como obrigatório depois de ter rodado ao menos uma vez; se a API recusar, abra um PR de teste antes.

## Comandos

### 1. Configurações do repositório

```bash
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

`default_branch` só pode apontar para uma branch que já exista no remoto.

### 2. Ruleset da `develop`

```bash
gh api -X POST repos/<dono>/<repo>/rulesets --input - <<'JSON'
{
  "name": "develop",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": { "ref_name": { "include": ["refs/heads/develop"], "exclude": [] } },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "allowed_merge_methods": ["rebase"]
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": false,
        "required_status_checks": [ { "context": "ci-ok" } ]
      }
    }
  ]
}
JSON
```

`bypass_actors` vazio faz a regra valer também para o administrador. `strict_required_status_checks_policy: false` é o "sem exigir branch atualizada".

### 3. Ruleset da `homolog`

```bash
gh api -X POST repos/<dono>/<repo>/rulesets --input - <<'JSON'
{
  "name": "homolog-parada",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": { "ref_name": { "include": ["refs/heads/homolog"], "exclude": [] } },
  "rules": [
    { "type": "creation" },
    { "type": "update" },
    { "type": "deletion" },
    { "type": "non_fast_forward" }
  ]
}
JSON
```

### 4. Ruleset da `main`

```bash
gh api -X POST repos/<dono>/<repo>/rulesets --input - <<'JSON'
{
  "name": "main-parada",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": { "ref_name": { "include": ["refs/heads/main"], "exclude": [] } },
  "rules": [
    { "type": "creation" },
    { "type": "update" },
    { "type": "deletion" },
    { "type": "non_fast_forward" }
  ]
}
JSON
```

Os tipos de regra `creation`, `update`, `deletion` e `non_fast_forward` e os parâmetros acima foram escritos de memória, sem conferir a documentação do GitHub: rode antes do uso e confira a resposta da API.

## Desfazer

```bash
gh api repos/<dono>/<repo>/rulesets            # lista, com o id de cada um
gh api -X DELETE repos/<dono>/<repo>/rulesets/<id>
```

Para desligar sem apagar, troque `enforcement` para `disabled` com `gh api -X PUT`.

## Segredo do job `terms`

```bash
gh secret set FORBIDDEN_TERMS --repo <dono>/<repo> < lista-de-termos.txt
```

A lista fica fora do git. Uma regra por linha; `#` comenta; o prefixo `re:` marca expressão regular. Sem o segredo, o job `terms` reprova (falha fechada).
