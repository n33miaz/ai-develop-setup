# ads-shell

Casca Tauri para Windows. O Rust lê porta e token do pipe herdado, fecha a
entrada padrão e autentica no WebSocket do daemon em loopback. A janela
principal carrega o bundle, envia comandos e recebe respostas e eventos por
IPC. A credencial fica no processo Rust. A janela reconecta automaticamente
após uma queda. A cada conexão, a casca exige que o daemon comprove conhecer
o token recebido no pipe antes de enviar a credencial ou encaminhar
comandos. A [configuração da interface](../../ui/README.md) define a espera,
o período de estabilidade e o limite de tentativas. Uma prova ausente ou
inválida bloqueia novas conexões nessa execução da casca e orienta a reabrir
pelo lançador para obter a credencial da execução atual.

Fechar a janela a oculta na bandeja. A opção de saída encerra apenas a casca.
Em uma nova execução do daemon, o lançador substitui a janela para usar a
nova porta e o novo token. Os parâmetros da janela ficam na
[configuração do lançador](../launcher/README.md).

O job `shell-check` empacota a interface e abre a janela real. A opção interna
`--smoke-test` exige a feature `desktop-smoke` e encerra a casca após encaminhar a primeira resposta autenticada ao IPC.
O teste verifica que daemon e host continuam ativos.
