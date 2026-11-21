# Mapa de módulos

Uma frase por módulo (P1): se a frase precisa de "e também", o módulo está grande demais. Todo crate novo entra aqui no mesmo PR; `python tools/check_deps.py check` reprova crate sem entrada. Quem pode depender de quem está em [dependency-matrix.md](dependency-matrix.md).

A coluna "Estado" diz o que existe hoje na `develop`. "Esqueleto" é um crate que compila sem comportamento do produto.

No daemon, `api` compõe os comandos autenticados da interface com o host;
`auth` protege o upgrade e a credencial por execução; `events` conserva streams
limitados em memória; `pty_client` transporta o contrato público do host;
`startup` carrega a configuração; `tool_launcher` compõe os adaptadores de abertura;
`main` inicia o serviço.

| Módulo | Pasta | Responsabilidade | Estado |
|---|---|---|---|
| `ads-contract-schemas` | `contracts` | Fonte dos contratos: JSON Schemas versionados, arquivos dourados e os tipos gerados a partir deles. | Em uso: Schemas, dourados, validador e geradores ([README](../contracts/README.md)). |
| `ads-contracts` | `crates/contracts` | Tipos Rust de mensagem e de registro gerados dos Schemas, válidos por construção. | Em uso: tipos gerados de todos os Schemas ([README](../crates/contracts/README.md)). Agendamento e execução da fila estão pendentes. |
| `ads-core` | `crates/core` | Política do produto: tarefas, aprovações, escopo, roteamento e orçamento. | Portão de ações destrutivas, política de abertura de ferramentas, configuração e ligação a perfis de conta (sem ativação na abertura) e funções puras de roteamento em observação, orçamento, passagem entre sessões e oferta de outro modelo após recusa implementados e testados ([README](../crates/core/README.md)); política da fila contínua em funções puras; integração com a execução de tarefas pendente. |
| `ads-storage` | `crates/storage` | Estado local: SQLite, cifra por coluna e auditoria encadeada. | Abertura conferida do banco de auditoria, cifra autenticada e log de eventos só-anexo, cifrado e encadeado, com um stream por fila de tarefas, implementados e testados ([README](../crates/storage/README.md), [ADR 0021](adr/0021-storage-atomic-file-and-cipher.md)); o daemon fornece a âncora do cofre em `audit`; faltam as invariantes do portão, o provedor de chaves do cofre, a ligação com o executor e a ligação do stream da fila no daemon. |
| `ads-ptyhost` | `crates/ptyhost` | Processo estável que segura as sessões de terminal e o anel de replay, servidos ao núcleo pelo contrato `pty-host/1`. | Implementado e testado, com os ciclos de ConPTY no CI ([README](../crates/ptyhost/README.md)). |
| `ads-daemon` | `crates/daemon` | Composição que liga o núcleo, os adaptadores e a API local de HTTP e WebSocket. | Cliente do `ptyhost`, que confere o dono do canal, e a API local autenticada por WebSocket ([README](../crates/daemon/README.md)); abre Claude e Codex pelo host com configuração do usuário. |
| `ads-testkit` | `crates/testkit` | Auxiliares só de teste: um processo filho do binário de teste que faz o papel de servidor de outra conta. | Em uso como dependência de desenvolvimento do `ads-ptyhost` e do `ads-daemon`; nunca entra num binário do produto. |
| `ads-launcher` | `crates/launcher` | Supervisor estável que inicia os outros processos e aplica as atualizações. | Supervisiona daemon e host por instalação, com sinais locais e estado JSON ([README](../crates/launcher/README.md)). Atualização pendente. |
| `ads-shell` | `crates/shell` | Casca Tauri (janela, bandeja e painel) como cliente fino do núcleo. | Esqueleto: janela vazia que carrega a interface e ícone de bandeja. |
| `ads-e2e` | `crates/e2e` | Sessão cifrada de ponta a ponta do transporte remoto, para o PC e a PWA. | Esqueleto. |
| `ads-adapter-claude` | `crates/adapters/claude` | Adapta a ferramenta de linha de comando Claude ao contrato do núcleo. | Implementado: identifica a ferramenta para a política de abertura do núcleo, com testes compartilhados. |
| `ads-adapter-codex` | `crates/adapters/codex` | Adapta a ferramenta de linha de comando Codex ao contrato do núcleo. | Implementado: identifica a ferramenta para a política de abertura do núcleo, com testes compartilhados. |
| `ads-adapter-antigravity` | `crates/adapters/antigravity` | Adapta a ferramenta de linha de comando Antigravity ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-api` | `crates/adapters/api` | Adapta as chamadas diretas às APIs de provedores ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-vault` | `crates/adapters/vault` | Adapta o cofre de segredos do sistema operacional ao contrato do núcleo. | Gerenciador de Credenciais do Windows, com operações por nome fixo ([README](../crates/adapters/vault/README.md)). |
| `ads-adapter-collectors` | `crates/adapters/collectors` | Coleta, só em leitura, os dados de uso das ferramentas. | Esqueleto. |
| `ads-adapter-relay` | `crates/adapters/relay` | Adapta o transporte do relé remoto ao contrato do núcleo. | Esqueleto; só os Schemas do relé existem. |
| `ads-adapter-fs` | `crates/adapters/fs` | Executa no sistema de arquivos, dentro de uma cerca, as ações que o portão do núcleo licenciou. | Implementado e testado, ainda não chamado por nenhum módulo ([README](../crates/adapters/fs/README.md)). |
| `ads-ui` | `ui` | Interface web, cliente fino do núcleo: mostra o estado que os eventos recebidos descrevem. | Tela com barra lateral e terminal e o cliente da API local, usado quando recebe a URL com o token; sem ela, mostra dados de exemplo ([README da interface](../ui/README.md)). Entrada, redimensionamento com debounce e abertura por ferramenta configurada implementados na interface. |
