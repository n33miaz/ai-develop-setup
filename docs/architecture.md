# Arquitetura local

O fluxo local no Windows usa processos separados para manter os terminais ativos durante reinícios do daemon. A interface roda na janela da casca desktop ou, no modo navegador, numa aba. O estado de cada módulo está no [mapa de módulos](modules.md).

```mermaid
flowchart LR
    Window["Janela principal"]
    Shell["Casca desktop"]
    Browser["Navegador"]
    Daemon["Daemon"]
    Host["ptyhost"]
    Terminal["ConPTY"]
    Tool["Ferramenta de linha de comando"]
    Launcher["Lançador"]
    Storage["Armazenamento local"]
    Vault["Cofre do sistema"]
    Window <-->|IPC só com a janela principal| Shell
    Daemon -->|Pipe anônimo: porta e token| Shell
    Shell <-->|WebSocket autenticado em loopback, sem Origin| Daemon
    Browser <-->|WebSocket autenticado em loopback| Daemon
    Daemon <-->|Pipe do usuário, pty-host/1| Host
    Host <-->|Entrada, saída e tamanho| Terminal
    Terminal <--> Tool
    Launcher -->|supervisão| Daemon
    Launcher -->|supervisão| Host
    Launcher -->|abre e troca a janela| Shell
    Daemon -->|log ancorado| Storage
    Daemon -->|chave e âncora do log| Vault
```

Os controles persistentes de fila e aprovações dependem da abertura do log ancorado e da preparação das identidades locais. Se uma dessas etapas falhar, o daemon mantém a API local ativa e deixa esses controles indisponíveis.

## Fronteiras de confiança

- **Casca, janela e daemon.** No modo desktop, o lançador liga a saída padrão do daemon à entrada padrão da casca por um pipe anônimo, e só por ele o daemon entrega porta e token. O Rust da casca guarda o token, autentica no WebSocket e repassa mensagens por IPC somente à janela principal; o JavaScript da janela nunca recebe a credencial. Antes de enviar `auth.hello`, a casca verifica no upgrade a prova de que o servidor conhece o token e está na mesma conexão TCP ([contrato da prova](../contracts/README.md#prova-do-daemon-no-desktop), [risco da porta reocupada](risks.md#outra-conta-ocupa-a-porta-do-daemon-após-a-saída)). O daemon recusa qualquer `Origin` nesse modo, porque `http://tauri.localhost` é compartilhada com outras páginas. Veja a [ADR 0020](adr/0020-local-api-authentication.md), o [modo desktop do daemon](../crates/daemon/README.md#modo-desktop), o [README da casca](../crates/shell/README.md) e o [risco da origem compartilhada](risks.md#origem-da-webview-compartilhada-no-windows).
- **Navegador e daemon.** No modo navegador, o daemon confere `Host` e `Origin` no upgrade e exige o token da execução em `auth.hello` antes de aceitar comandos. Quem abre a URL de conexão precisa conferir que o servidor da interface pertence à própria conta. Veja a [ADR 0020](adr/0020-local-api-authentication.md) e o [risco da porta da interface](risks.md#outro-usuário-ocupa-a-porta-da-interface).
- **Daemon e ptyhost.** No Windows, o host restringe o pipe ao usuário. Antes de enviar mensagens, o cliente confere a identidade do processo servidor e o dono do objeto do pipe. O host mantém sessões e replay em memória, independentemente da conexão do daemon. Veja a [ADR 0003](adr/0003-separate-pty-host-process.md) e o [canal do host](../crates/ptyhost/README.md#como-roda).
- **Host e ferramenta.** O daemon prepara a abertura usando configuração local e a política do núcleo. O host inicia o executável via ConPTY com o ambiente do pedido. A ferramenta roda com as permissões do usuário. Veja a [fronteira das credenciais](risks.md#credencial-de-cli-de-ia-vaza-para-o-processo-filho).
- **Armazenamento.** O módulo grava eventos num log só-anexo, com dados cifrados e cadeia com chave. O daemon abre o banco canônico com a âncora no cofre do sistema, que recusa corte, cópia antiga ou troca do banco, e o diário do portão grava no log ancorado ([âncora no cofre](../crates/storage/README.md#âncora-no-cofre)). O portão nega qualquer ação sobre o estado do próprio produto (banco de auditoria, pasta do cofre e travas), e a fila de tarefas grava seu stream nesse log pelo controle do dono na API local (`queue.get`, `queue.start`, `queue.stop`) ([fila no daemon](../crates/daemon/README.md#fila-de-tarefas)). O dono lista e responde pela mesma API as ações que o portão segurou (`approval.list`, `approval.answer`); a resposta é gravada no log antes de o portão agir, e a API só aprova ação cujo efeito concreto está fixado: o plano registrado ou os efeitos concretos que o ensaio gravou, conferidos de novo antes da licença e antes da primeira alteração ([aprovações no daemon](../crates/daemon/README.md#aprovações-do-portão)). Veja as ADRs [0004](adr/0004-local-sqlite-no-cloud-database.md), [0005](adr/0005-column-level-encryption.md) e [0021](adr/0021-storage-atomic-file-and-cipher.md).
- **Lançador.** O lançador supervisiona o daemon e o host de terminais. No Windows, ele também abre a casca e a substitui quando o daemon renasce com nova porta e novo token; se a casca sair, daemon e host continuam. Para abrir outra janela, o operador cria o sinal `restart` na pasta de controle; o lançador reinicia o daemon e abre a nova casca com a credencial dele. O [README do lançador](../crates/launcher/README.md) descreve o comportamento atual e as pendências. A atualização com verificação de artefato está prevista na [ADR 0015](adr/0015-ci-built-artifact-with-verified-self-update.md).
