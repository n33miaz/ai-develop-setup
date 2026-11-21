# ads-shell

Casca Tauri para Windows. O lançador abre a interface empacotada, que autentica
no WebSocket do daemon em loopback. A origem da janela é
`http://tauri.localhost` e a navegação fica restrita a ela. A casca lê uma
linha do pipe herdado com a porta decimal, um espaço e o token base64url.
A interface recebe os dados uma vez por IPC e os mantém em memória.

Fechar a janela a oculta na bandeja. A opção de saída encerra apenas a casca.
Em uma nova execução do daemon, o lançador substitui a janela para usar a
nova porta e o novo token. Os parâmetros da janela ficam na
[configuração do lançador](../launcher/README.md).

O job `shell-check` empacota a interface e abre a janela real. A opção interna
`--smoke-test` encerra a casca assim que a interface confirma o `auth.hello`.
O teste verifica que daemon e host continuam ativos.
