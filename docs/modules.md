# Mapa de módulos

Uma frase por módulo (P1): se a frase precisa de "e também", o módulo está grande demais. Todo crate novo entra aqui no mesmo PR; `python tools/check_deps.py check` reprova crate sem entrada. Quem pode depender de quem está em [dependency-matrix.md](dependency-matrix.md).

A coluna "Estado" diz o que existe hoje na `develop`. "Esqueleto" é um crate que compila sem comportamento do produto.

| Módulo | Pasta | Responsabilidade | Estado |
|---|---|---|---|
| `ads-contract-schemas` | `contracts` | Fonte dos contratos: JSON Schemas versionados, arquivos dourados e os tipos gerados a partir deles. | Em uso: Schemas, dourados, validador e geradores ([README](../contracts/README.md)). |
| `ads-contracts` | `crates/contracts` | Tipos Rust de mensagem e de registro gerados dos Schemas, válidos por construção. | Em uso: tipos gerados de todos os Schemas ([README](../crates/contracts/README.md)). |
| `ads-core` | `crates/core` | Política do produto: tarefas, aprovações, escopo, roteamento e orçamento. | Portão de ações destrutivas implementado e testado ([README](../crates/core/README.md)); tarefas, roteamento e orçamento ainda não. |
| `ads-storage` | `crates/storage` | Estado local: SQLite, cifra por coluna e auditoria encadeada. | Esqueleto; abertura do banco e cifra em andamento (issue #47). |
| `ads-ptyhost` | `crates/ptyhost` | Processo estável que segura as sessões de terminal e o anel de replay, servidos ao núcleo pelo contrato `pty-host/1`. | Implementado e testado, com os ciclos de ConPTY no CI ([README](../crates/ptyhost/README.md)). |
| `ads-daemon` | `crates/daemon` | Composição que liga o núcleo, os adaptadores e a API local de HTTP e WebSocket. | Só o cliente do `ptyhost` ([README](../crates/daemon/README.md)); o executável ainda não serve a API local. |
| `ads-launcher` | `crates/launcher` | Supervisor estável que inicia os outros processos e aplica as atualizações. | Esqueleto: o executável só imprime o nome. |
| `ads-shell` | `crates/shell` | Casca Tauri (janela, bandeja e painel) como cliente fino do núcleo. | Esqueleto: janela vazia que carrega a interface e ícone de bandeja. |
| `ads-e2e` | `crates/e2e` | Sessão cifrada de ponta a ponta do transporte remoto, para o PC e a PWA. | Esqueleto. |
| `ads-adapter-claude` | `crates/adapters/claude` | Adapta a ferramenta de linha de comando Claude ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-codex` | `crates/adapters/codex` | Adapta a ferramenta de linha de comando Codex ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-antigravity` | `crates/adapters/antigravity` | Adapta a ferramenta de linha de comando Antigravity ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-api` | `crates/adapters/api` | Adapta as chamadas diretas às APIs de provedores ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-vault` | `crates/adapters/vault` | Adapta o cofre de segredos do sistema operacional ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-collectors` | `crates/adapters/collectors` | Coleta, só em leitura, os dados de uso das ferramentas. | Esqueleto. |
| `ads-adapter-relay` | `crates/adapters/relay` | Adapta o transporte do relé remoto ao contrato do núcleo. | Esqueleto; só os Schemas do relé existem. |
| `ads-adapter-fs` | `crates/adapters/fs` | Executa no sistema de arquivos, dentro de uma cerca, as ações que o portão do núcleo licenciou. | Implementado e testado, ainda não chamado por nenhum módulo ([README](../crates/adapters/fs/README.md)). |
| `ads-ui` | `ui` | Interface web, cliente fino do núcleo: mostra o estado que os eventos recebidos descrevem. | Esqueleto de tela com dados de exemplo e cliente falso; tipos dos contratos em uso (sem README próprio). |
