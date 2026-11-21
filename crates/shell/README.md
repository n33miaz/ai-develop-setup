# ads-shell

Casca Tauri para Windows. O Rust lê porta e token do pipe herdado, fecha a
entrada padrão e autentica no WebSocket do daemon em loopback. A janela
principal carrega o bundle, envia comandos e recebe respostas e eventos por
IPC. A credencial fica no processo Rust. A cada conexão, inclusive depois de
recarregar a interface, a casca exige que o daemon comprove conhecer o token
recebido no pipe antes de enviar a credencial ou encaminhar comandos. Enquanto
abre a conexão, a interface tenta de novo após falhas transitórias (porta
fechada, handshake, autenticação recusada ou prazo de 5 s), a partir de 250 ms,
dobrando a espera até 5 s por padrão e sem limite de tentativas. A
[configuração da interface](../../ui/README.md) define esses limites.

Essa espera vale só para a abertura. Depois de conectada, se a conexão com o
daemon cai ou um envio falha, a casca avisa a janela, a interface fica
desconectada, rejeita os pedidos pendentes e não reconecta sozinha. Se o
daemon saiu, a janela só é substituída quando o lançador consegue iniciar uma
nova execução: ele espera entre as tentativas e, ao atingir
`crash_window_limit` falhas, desiste e fecha a janela sem abrir outra. Se a
mesma execução do daemon continua ativa, recarregue a interface para abrir uma
conexão nova com a credencial desta casca. Nos dois casos, o arquivo `restart`
na pasta de controle do [lançador](../launcher/README.md) reinicia o daemon,
zera a desistência e abre outra janela com a nova credencial; o host mantém as
sessões existentes.

Uma prova ausente indica que
é preciso conferir se a casca e o daemon têm a mesma versão. Uma prova
inválida bloqueia novas conexões nessa execução da casca e orienta a reabrir
pelo lançador para obter a credencial da execução atual.

Fechar a janela a oculta na bandeja. A opção de saída encerra apenas a casca.
Em uma nova execução do daemon, o lançador substitui a janela para usar a
nova porta e o novo token. Os parâmetros da janela ficam na
[configuração do lançador](../launcher/README.md).

O job `shell-check` empacota a interface e abre a janela real. A opção interna
`--smoke-test` exige a feature `desktop-smoke` e encerra a casca após encaminhar pelo IPC a resposta bem-sucedida à consulta `harness.list` enviada pela janela.
O teste verifica que daemon e host continuam ativos.
