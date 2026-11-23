# Como contribuir

## Contribuição externa: fechada por ora

O projeto está em construção e a licença não concede direitos de uso, cópia ou modificação (ver [LICENSE](LICENSE)). Por isso **não aceitamos pull requests nem issues de terceiros por enquanto**. Isso muda quando o mantenedor abrir contribuição e atualizar este arquivo. Relatos de segurança seguem o [SECURITY.md](SECURITY.md).

O restante deste documento descreve o fluxo do mantenedor e dos agentes que trabalham sob instrução dele.

## Fluxo de branches

`feature/<nº-da-issue>-<assunto>` (ou `fix/...`, `ci/...`, `docs/...`) → PR → `develop` → `homolog` → `main`.

| Branch | Modo expresso (agora) | Modo completo (depois) |
|---|---|---|
| `develop` | É a produção. PR obrigatório, check `ci-ok` verde, junção com merge commit no fluxo de autoria de agente, sem exigir branch atualizada, sem force-push nem exclusão; o papel de administrador do repositório tem exceção, reservada ao mantenedor | igual |
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

Durante um merge da branch local `develop`, o hook aceita o assunto padrão do Git (`Merge branch 'develop' into <branch>`) como exceção ao formato e ao limite de tamanho do assunto. As regras de autoria, trailers, texto público e nome da branch continuam valendo.

O mesmo script confere o texto público (ver "Texto público" abaixo) no assunto de cada commit e no nome da branch: recusa, por palavra inteira e sem diferenciar maiúsculas, os termos de processo de `commits.process_terms` e os códigos internos de organização de `commits.internal_code_patterns`, as duas listas em `defaults/devflow.toml`. No CI o nome da branch é o do PR; no hook, o da branch atual (em HEAD destacado não há o que conferir). No assunto o hífen une a palavra (`round-trip` passa); no nome da branch ele separa.

### DCO

Cada commit leva `Signed-off-by`, atestando o [Developer Certificate of Origin 1.1](https://developercertificate.org/). O hook acrescenta o trailer com a identidade do autor do commit.

### Autoria de agente

O modo padrão `agent-author` registra a conta do agente como autora e committer e inclui o mantenedor como coautor. Com o hook ligado e os arquivos em stage, execute:

```bash
bash tools/commit-as-agent.sh codex "feat(core): add event envelope"
```

Escolha `claude` ou `codex` conforme quem escreveu a maior parte. O script lê `agent.*` de `defaults/devflow.toml` e o hook acrescenta o trailer a partir de `coauthor.owner`. Para um commit do mantenedor, use o Git diretamente com `ADS_AI_AUTHOR` vazia ou ausente.

Para voltar à autoria do mantenedor com coautoria do agente, mude `ai_attribution` para `"executor"` e informe o agente em `ADS_AI_AUTHOR`. Para desativar a coautoria, use `"none"`.

### Ligar o hook e testar

```bash
git config core.hooksPath .githooks
bash .github/scripts/check-commits.test.sh
```

## Pull requests

- Título descritivo, na mesma regra do título da issue (em geral o mesmo). O padrão de mensagem vale para os commits da branch. Descrição em português: o que mudou, por que, como testar, checklist P1 a P8 e `Closes #n`.
- Todo PR recebe uma revisão automática (configurada em `.coderabbit.yaml`); apontamentos de severidade alta ou média são resolvidos, com teste, antes da junção.
- No fluxo de autoria de agente, use merge commit depois do check `ci-ok` verde para preservar os commits e suas identidades. Apague a branch após a junção.
- Antes de juntar, confira o `ci-ok` da execução mais recente do commit exato.

## Módulos de segurança

Proposta, aguardando ratificação do mantenedor. Mudanças nos módulos abaixo exigem uma revisão a mais antes de juntar, além do `ci-ok` verde. Cada correção pedida vira commit novo no mesmo PR ou issue aberta antes da junção.

| Módulo | Por quê |
|---|---|
| Portão de ações (`crates/core/src/gate`) | Decide o que pode apagar, mover ou sobrescrever arquivos |
| Executor de arquivos (`crates/adapters/fs`) | Executa no disco o que o portão licenciou |
| Armazenamento e cifra (`crates/storage`, cofre) | Guarda texto de terminal, prompts e caminhos cifrados, e a chave |
| Autenticação da API local ([ADR 0020](docs/adr/0020-local-api-authentication.md)) | Decide quem comanda os terminais do usuário |
| Ambiente do processo filho e entrada de comandos (adaptadores de ferramenta, `session.open` e `session.write` no núcleo) | Decide o que roda, com que ambiente e com que entrada; é onde credencial de CLI de IA poderia vazar (C1) |
| Canal do `ptyhost` (pipe do usuário) | Separa as sessões de um usuário das de outro (P3) |
| Ponte da casca (`crates/shell/src/bridge.rs`, `crates/shell/src/stdin.rs`) | Recebe o token pelo pipe herdado, exige a prova do daemon antes de enviá-lo e decide o que chega à janela pelo IPC |
| Pastas privadas e sinais do lançador (`crates/private-dir`, sinais em `crates/launcher/src/supervisor.rs`) | Decidem quem para ou reinicia a instalação e quem alcança a pasta de controle e a pasta da auditoria |

Documentação de proposta (como uma ADR ainda não ratificada) não é mudança no módulo e pode ser juntada só com o `ci-ok`.

## Texto público

Títulos e descrições de issue e PR, nomes de branch, assuntos de commit, comentários, README, ADR e este arquivo descrevem o assunto e o resultado da entrega, como ficou. Não contam o histórico do processo interno de trabalho (etapas, idas e vindas, o que cada conferência encontrou), não trazem códigos internos de organização (identificadores de marco, de cartão, de tarefa ou de etapa) e não trazem marcas de ferramentas nem rodapé de autoria por ferramenta. O único rastro de ferramenta permitido é o trailer `Co-authored-by`, regido por `defaults/devflow.toml`.

- Uma descrição que mudou ao longo do PR é reescrita como um texto único e atual, não acrescida de seções por etapa.
- Risco corrigido é descrito pelo que ele era, onde foi tratado e o teste que o cobre.
- Termos do produto continuam valendo: eventos e contratos de revisão de código, o campo `model`, "tipos gerados", "round trip" no sentido de ida e volta de serialização, e "revisão" no sentido genérico ("fica aberto para revisão antes de juntar").
- O checador de commits aplica a parte mecânica desta regra (ver "Commits"); o resto é conferido na revisão do PR.

## Versões

Até a primeira versão estável, quando o CI de uma junção na `develop` termina verde, sai um pré-lançamento (junções muito próximas podem sair num só) em SemVer, `0.1.0-dev.<AAAAMMDD>.<N>+<commit curto>`, com tag `v0.1.0-dev.<AAAAMMDD>.<N>` e notas geradas dos PRs; ficam os dez mais novos ([ADR 0025](docs/adr/0025-semver-dev-prereleases-until-first-installer.md)). Os parâmetros estão em `defaults/release.toml`. Ninguém cria tag nem release à mão: o workflow `prerelease.yml` faz isso.

## Segredos e dados

Nunca commite segredos, `.env`, chaves, bancos, logs ou dados de usuário. O CI varre segredos (gitleaks). A lista de termos proibidos, mantida fora do repo, é conferida localmente antes do push; o job `terms` do CI só roda com a variável de repositório `TERMS_CHECK = on` e o segredo `FORBIDDEN_TERMS`, e hoje está desligado (ver [AGENTS.md](AGENTS.md)).
