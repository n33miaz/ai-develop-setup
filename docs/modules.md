# Mapa de módulos

Uma frase por módulo (P1): se a frase precisa de "e também", o módulo está grande demais. Todo crate novo entra aqui no mesmo PR; `python tools/check_deps.py check` reprova crate sem entrada. Quem pode depender de quem está em [dependency-matrix.md](dependency-matrix.md).

| Módulo | Pasta | Responsabilidade |
|---|---|---|
| `ads-contracts` | `crates/contracts` | Tipos de mensagem e de evento gerados dos Schemas, sem lógica. |
| `ads-core` | `crates/core` | Política do produto: tarefas, aprovações, escopo, roteamento e orçamento. |
| `ads-storage` | `crates/storage` | Estado local: SQLite, cifra por coluna e auditoria encadeada. |
| `ads-ptyhost` | `crates/ptyhost` | Processo estável que segura as sessões de terminal e o anel de replay. |
| `ads-daemon` | `crates/daemon` | Composição que liga o núcleo, os adaptadores e a API local de HTTP e WebSocket. |
| `ads-launcher` | `crates/launcher` | Supervisor estável que inicia os outros processos e aplica as atualizações. |
| `ads-shell` | `crates/shell` | Casca Tauri (janela, bandeja e painel) como cliente fino do núcleo. |
| `ads-e2e` | `crates/e2e` | Sessão cifrada de ponta a ponta do transporte remoto, para o PC e a PWA. |
| `ads-adapter-claude` | `crates/adapters/claude` | Adapta a ferramenta de linha de comando Claude ao contrato do núcleo. |
| `ads-adapter-codex` | `crates/adapters/codex` | Adapta a ferramenta de linha de comando Codex ao contrato do núcleo. |
| `ads-adapter-antigravity` | `crates/adapters/antigravity` | Adapta a ferramenta de linha de comando Antigravity ao contrato do núcleo. |
| `ads-adapter-api` | `crates/adapters/api` | Adapta as chamadas diretas às APIs de provedores ao contrato do núcleo. |
| `ads-adapter-vault` | `crates/adapters/vault` | Adapta o cofre de segredos do sistema operacional ao contrato do núcleo. |
| `ads-adapter-collectors` | `crates/adapters/collectors` | Coleta, só em leitura, os dados de uso das ferramentas. |
| `ads-adapter-relay` | `crates/adapters/relay` | Adapta o transporte do relé remoto ao contrato do núcleo. |
