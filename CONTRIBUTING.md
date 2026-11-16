# Como contribuir

## Contribuição externa: fechada por ora

O projeto está em construção e a licença não concede direitos de uso, cópia ou modificação (ver [LICENSE](LICENSE)). Por isso **não aceitamos pull requests nem issues de terceiros por enquanto**. Isso muda quando o mantenedor abrir contribuição e atualizar este arquivo. Relatos de segurança seguem o [SECURITY.md](SECURITY.md).

O restante deste documento descreve o fluxo do mantenedor e dos agentes que trabalham sob instrução dele.

## Fluxo de branches

`feature/<nº-da-issue>-<assunto>` (ou `fix/...`) → PR → `develop` → `homolog` → `main`.

| Branch | Modo expresso (agora) | Modo completo (depois) |
|---|---|---|
| `develop` | É a produção. PR obrigatório, check `ci-ok` verde, junção por rebase (automática), sem exigir branch atualizada, sem force-push nem exclusão; o papel de administrador do repositório tem exceção, reservada ao mantenedor | igual |
| `homolog` | Existe, sob a mesma proteção da `develop`, e fica sem uso | Recebe PR da `develop` |
| `main` | Existe, sob a mesma proteção da `develop`, e fica sem uso | Recebe PR da `homolog` |

Sair do modo expresso é decisão do mantenedor, registrada no parâmetro `profile` de `defaults/devflow.toml`. As regras de proteção, como estão aplicadas hoje, estão em [docs/rulesets.md](docs/rulesets.md). Agentes nunca usam a exceção do administrador: sempre PR com `ci-ok` verde.

## Uma issue por entrega

Todo trabalho começa numa issue (modelo `task`), aberta antes do código: título, tipo, área, objetivo, critério de aceite e princípios tocados (P1 a P8, ver `AGENTS.md`). Miudezas da mesma entrega viram itens de checklist da issue, não issues novas. A issue é pública: nada de dados pessoais, de empregador, caminhos locais ou segredos.

O PR aponta para a issue com `Closes #n`. A issue só é dada por pronta depois de conferida em produção.

### Título da issue

O título descreve o problema ou o resultado esperado; não é mensagem de commit ([ADR 0019](docs/adr/0019-descriptive-issue-and-pr-titles.md)).

- Em inglês, no imperativo ou como frase nominal, com inicial maiúscula.
- Sem prefixo `tipo(escopo):`, sem ponto final, até 72 caracteres.
- Curto e específico: é o que aparece em listas, buscas e referências cruzadas.

| Em vez de | Escreva |
|---|---|
| `fix(core): reject unicode path aliases` | Prevent deletions outside the workspace |
| `fix(ptyhost): replay on reconnect` | Terminal host loses output when the client reconnects |

### Rótulos

A classificação fica nos rótulos, não no título. Toda issue leva **um** rótulo de tipo e **pelo menos um** de área; quem abre a issue aplica os dois (`gh issue create --label "type: fix" --label "area: core"`), porque o formulário não aplica rótulo sozinho.

| Rótulo de tipo | Quando usar | Tipos de commit que costuma gerar |
|---|---|---|
| `type: feature` | Capacidade nova ou ampliada | `feat` |
| `type: fix` | Correção de defeito | `fix` |
| `type: chore` | Manutenção sem mudar o comportamento: estrutura, refatoração, dependências, build | `chore`, `refactor`, `build`, `perf`, `style`, `revert` |
| `type: docs` | Só documentação | `docs` |
| `type: test` | Só testes | `test` |
| `type: ci` | Esteira de CI e automações do repositório | `ci` |

O rótulo é da issue inteira; os commits dela podem ter tipos diferentes.

| Rótulo de área | O que cobre |
|---|---|
| `area: contracts` | `contracts/` e `crates/contracts`: Schemas, arquivos dourados e tipos gerados |
| `area: core` | `crates/core`: política e lógica do produto |
| `area: storage` | `crates/storage`: estado local |
| `area: ptyhost` | `crates/ptyhost`: processo das sessões de terminal |
| `area: daemon` | `crates/daemon`: composição e API local |
| `area: launcher` | `crates/launcher`: supervisor e atualização |
| `area: shell` | `crates/shell`: casca Tauri |
| `area: e2e` | `crates/e2e`: sessão cifrada do transporte remoto |
| `area: adapters` | `crates/adapters/*`: adaptadores de ferramentas, provedores, transporte e cofre |
| `area: ui` | `ui/`: interface web |
| `area: process` | Fluxo de trabalho, CI, regras do repositório, workspace e documentação de processo |

Área nova nasce junto com o módulo novo (mesma entrega que o registra em `docs/modules.md`). Os adaptadores dividem um rótulo só enquanto forem poucos.

## Commits

Formato de commit (Conventional Commits, em inglês):

```
feat(core): add event envelope

Signed-off-by: Nome <12345+usuario@users.noreply.github.com>
Co-authored-by: Nome do agente <noreply@exemplo.org>
```

- Assunto de até 50 caracteres, imperativo, sem ponto final. A descrição vai inteira em minúsculas, não só a primeira letra: siglas e identificadores também (`add ci job`, não `add CI job`). Tipos: `feat`, `fix`, `chore`, `refactor`, `docs`, `test`, `perf`, `build`, `ci`, `style`, `revert`.
- **Sem corpo descritivo.** Só o assunto, uma linha em branco e os trailers. A explicação vai na descrição do PR.
- Trailers só da lista permitida: `Signed-off-by` (obrigatório) e, no máximo, um `Co-authored-by`.
- O e-mail do autor é o endereço noreply do GitHub (`ID+usuario@users.noreply.github.com`).
- Cada commit é uma entrega completa (contrato, código e teste juntos). Correção de revisão entra como commit novo; nada de `amend` em commit já enviado nem de force-push.

Todas essas regras e seus valores ficam em `defaults/devflow.toml` e são aplicadas pelo mesmo script no hook local e no CI (`.github/scripts/check-commits.sh`). O script valida o arquivo de parâmetros antes de olhar qualquer mensagem: valor de tipo errado ou fora do domínio termina com código 2 (erro de configuração), sem analisar commit nenhum; mensagem fora do padrão termina com código 1. O hook roda essa mesma validação antes de tudo, então configuração inválida barra o commit com código 2 mesmo com `commits.enabled = false`.

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

- Título descritivo, na mesma regra do título da issue (em geral o mesmo). Como a junção é por rebase, os commits entram um a um e o título do PR não vira commit: o padrão de commit vale só para os commits. Descrição em português: o que mudou, por que, como testar, checklist P1 a P8 e `Closes #n`.
- Junção por rebase, com auto-merge ligado, depois do check `ci-ok` verde. Apague a branch após a junção.
- Antes de juntar, confira o `ci-ok` da execução mais recente do commit exato.

## Módulos de segurança

Proposta, aguardando ratificação do mantenedor. Mudança nos módulos abaixo pede **revisão independente** (outro modelo ou outra pessoa, que não escreveu a mudança) antes de juntar, além do `ci-ok` verde. Os achados da revisão viram commits novos no mesmo PR ou issues abertas antes da junção. O PR diz quem revisou e o que foi apontado.

| Módulo | Por quê |
|---|---|
| Portão de ações (`crates/core/src/gate`) | Decide o que pode apagar, mover ou sobrescrever arquivos |
| Executor de arquivos (`crates/adapters/fs`) | Executa no disco o que o portão licenciou |
| Armazenamento e cifra (`crates/storage`, cofre) | Guarda texto de terminal, prompts e caminhos cifrados, e a chave |
| Autenticação da API local ([ADR 0020](docs/adr/0020-local-api-authentication.md)) | Decide quem comanda os terminais do usuário |
| Ambiente do processo filho e entrada de comandos (adaptadores de ferramenta, `session.open` e `session.write` no núcleo) | Decide o que roda, com que ambiente e com que entrada; é onde credencial de CLI de IA poderia vazar (C1) |
| Canal do `ptyhost` (pipe do usuário) | Separa as sessões de um usuário das de outro (P3) |

Documentação de proposta (como uma ADR ainda não ratificada) não é mudança no módulo e pode ser juntada só com o `ci-ok`.

## Segredos e dados

Nunca commite segredos, `.env`, chaves, bancos, logs ou dados de usuário. O CI varre segredos (gitleaks) e uma lista de termos proibidos mantida fora do repo.
