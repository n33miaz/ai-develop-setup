# Mapa de módulos

Uma frase por módulo (P1): se a frase precisa de "e também", o módulo está grande demais. Todo crate novo entra aqui no mesmo PR; `python tools/check_deps.py check` reprova crate sem entrada. Quem pode depender de quem está em [dependency-matrix.md](dependency-matrix.md).

A coluna "Estado" diz o que existe hoje na `develop`. "Esqueleto" é um crate que compila sem comportamento do produto.

No daemon, `api` compõe os comandos autenticados da interface com o host;
`approvals` liga as respostas do dono às portas de sessão do portão, com registro no log ancorado;
`audit` abre o log de auditoria ancorado no cofre junto com o estado do produto que ele protege;
`auth` protege o upgrade e a credencial por execução e emite a prova de sessão de cada conexão; `events` conserva streams
limitados em memória; `gate` liga o executor de arquivos ao diário do portão nesse log;
`product` prepara as identidades e a chave dos controles persistentes;
`diagnostic` escreve avisos para o dono na saída de erro;
`pty_client` transporta o contrato público do host;
`queue` liga o controle da fila pelo dono ao stream da fila no log ancorado;
`startup` carrega a configuração e acopla os controles persistentes à API;
`tool_launcher` compõe os adaptadores de abertura;
`main` inicia o serviço ([fila](../crates/daemon/README.md#fila-de-tarefas),
[aprovações](../crates/daemon/README.md#aprovações-do-portão)).

| Módulo | Pasta | Responsabilidade | Estado |
|---|---|---|---|
| `ads-contract-schemas` | `contracts` | Fonte dos contratos: JSON Schemas versionados, arquivos dourados e os tipos gerados a partir deles. | Em uso: Schemas, dourados, validador e geradores ([README](../contracts/README.md)). |
| `ads-contracts` | `crates/contracts` | Tipos Rust de mensagem e de registro gerados dos Schemas, válidos por construção. | Em uso: tipos gerados de todos os Schemas ([README](../crates/contracts/README.md)). |
| `ads-child-env` | `crates/child-env` | Valida e filtra as variáveis de ambiente dos processos filhos. | Em uso pelo núcleo e pelo lançador, com a política base de [harnesses.toml](../defaults/harnesses.toml). |
| `ads-core` | `crates/core` | Política do produto: tarefas, aprovações, escopo, roteamento e orçamento. | Portão de ações destrutivas com porta de diário, política de abertura de ferramentas, configuração e ligação a perfis de conta (sem ativação na abertura) e funções puras de roteamento em observação, orçamento, passagem entre sessões e oferta de outro modelo após recusa implementados e testados ([README](../crates/core/README.md)); política da fila contínua em funções puras; integração com a execução de tarefas pendente. |
| `ads-storage` | `crates/storage` | Estado local: SQLite, cifra por coluna e log encadeado. | Abertura conferida do banco de eventos, cifra autenticada e log de eventos só-anexo, cifrado e encadeado, com um stream por fila de tarefas, implementados e testados ([README](../crates/storage/README.md), [ADR 0021](adr/0021-storage-atomic-file-and-cipher.md)). O diário do portão usa o log ancorado e persiste o uso único da licença. |
| `ads-ptyhost` | `crates/ptyhost` | Processo estável que segura as sessões de terminal e o anel de replay, servidos ao núcleo pelo contrato `pty-host/1`. | Implementado e testado, com os ciclos de ConPTY no CI ([README](../crates/ptyhost/README.md)). |
| `ads-daemon` | `crates/daemon` | Composição que liga o núcleo, os adaptadores e a API local de HTTP e WebSocket. | Cliente do `ptyhost`, que confere o dono do canal, e a API local autenticada por WebSocket ([README](../crates/daemon/README.md)). Abre as ferramentas pelo host com configuração do usuário e liga o portão persistente ao executor de arquivos na biblioteca Rust. No Windows, o executável usa o cofre para a chave e a âncora do log e identidades da pasta privada para servir os [controles de fila](../crates/daemon/README.md#fila-de-tarefas) e de [aprovações](../crates/daemon/README.md#aprovações-do-portão). No modo desktop, prova à casca que conhece a credencial da execução. |
| `ads-testkit` | `crates/testkit` | Auxiliares só de teste: um processo filho do binário de teste que faz o papel de servidor de outra conta. | Em uso como dependência de desenvolvimento do `ads-ptyhost` e do `ads-daemon`; nunca entra num binário do produto. |
| `ads-private-dir` | `crates/private-dir` | Cria e confere pastas privadas do usuário. | Usado pela pasta de controle do lançador e pela pasta da auditoria do daemon. |
| `ads-launcher` | `crates/launcher` | Supervisor estável que inicia os outros processos e aplica as atualizações. | Supervisiona daemon e host por instalação, com sinais locais e estado JSON; no Windows, abre a casca, a substitui quando o daemon reinicia e a fecha no `stop`, em Ctrl+C ou Ctrl+Break e quando desiste do daemon. A pasta de controle é privada da conta e ignora sinais de outra conta ([README](../crates/launcher/README.md)). Atualização pendente. |
| `ads-shell` | `crates/shell` | Casca Tauri (janela, bandeja e painel) como cliente fino do núcleo. | Janela Windows com interface empacotada, conexão autenticada ao daemon, prova do daemon exigida antes da credencial, reconexão após queda e bandeja ([README](../crates/shell/README.md)). Painel pendente. |
| `ads-e2e` | `crates/e2e` | Sessão cifrada de ponta a ponta do transporte remoto, para o PC e a PWA. | Esqueleto. |
| `ads-adapter-claude` | `crates/adapters/claude` | Adapta a ferramenta de linha de comando Claude ao contrato do núcleo. | Implementado: identifica a ferramenta para a política de abertura do núcleo, com testes compartilhados. |
| `ads-adapter-codex` | `crates/adapters/codex` | Adapta a ferramenta de linha de comando Codex ao contrato do núcleo. | Implementado: identifica a ferramenta para a política de abertura do núcleo, com testes compartilhados. |
| `ads-adapter-antigravity` | `crates/adapters/antigravity` | Adapta a ferramenta de linha de comando Antigravity ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-api` | `crates/adapters/api` | Adapta as chamadas diretas às APIs de provedores ao contrato do núcleo. | Esqueleto. |
| `ads-adapter-vault` | `crates/adapters/vault` | Adapta o cofre de segredos do sistema operacional ao contrato do núcleo. | Gerenciador de Credenciais do Windows, com operações por nome fixo ([README](../crates/adapters/vault/README.md)). |
| `ads-adapter-collectors` | `crates/adapters/collectors` | Coleta, só em leitura, os dados de uso das ferramentas. | Esqueleto. |
| `ads-adapter-relay` | `crates/adapters/relay` | Adapta o transporte do relé remoto ao contrato do núcleo. | Esqueleto; só os Schemas do relé existem. |
| `ads-adapter-fs` | `crates/adapters/fs` | Executa no sistema de arquivos, dentro de uma cerca, as ações que o portão do núcleo licenciou. | Ligado ao diário pela composição do daemon ([README](../crates/adapters/fs/README.md)). |
| `ads-ui` | `ui` | Interface web, cliente fino do núcleo: mostra o estado que os eventos recebidos descrevem. | Tela com barra lateral e terminal e o cliente da API local: na casca desktop, pela ponte IPC; no navegador, pela URL de conexão com o token; sem conexão configurada, mostra dados de exemplo ([README da interface](../ui/README.md)). Entrada, redimensionamento com debounce e abertura por ferramenta configurada implementados na interface. No modo conectado, os [painéis de fila e aprovações](../ui/README.md#fila-de-tarefas-e-aprovações) usam os controles do daemon. O painel [Uso e contas](../ui/README.md#uso-e-contas) mostra perfis, cota, orçamento e sugestão de roteamento só com dados de exemplo; conectado, avisa que a API local ainda não oferece esses dados. |
