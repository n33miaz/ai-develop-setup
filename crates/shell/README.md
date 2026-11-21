# ads-shell

Casca Tauri para Windows. O Rust lê porta e token do pipe herdado, fecha a
entrada padrão e autentica no WebSocket do daemon em loopback. A janela
principal carrega o bundle, envia comandos e recebe respostas e eventos por
IPC. A credencial fica no processo Rust. A cada conexão, inclusive depois de
recarregar a interface, a casca exige que o daemon comprove conhecer o token
recebido no pipe antes de enviar a credencial ou encaminhar comandos. Se o
daemon que abriu a janela saiu ou a verificação falha, a interface orienta a
reabrir pelo lançador. Após uma tentativa frustrada, a casca encerra as
tentativas para aquela credencial.

Fechar a janela a oculta na bandeja. A opção de saída encerra apenas a casca.
Em uma nova execução do daemon, o lançador substitui a janela para usar a
nova porta e o novo token. Os parâmetros da janela ficam na
[configuração do lançador](../launcher/README.md).

O job `shell-check` empacota a interface e abre a janela real. A opção interna
`--smoke-test` exige a feature `desktop-smoke` e encerra a casca após encaminhar a primeira resposta autenticada ao IPC.
O teste verifica que daemon e host continuam ativos.
