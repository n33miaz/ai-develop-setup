# ads-shell

Casca Tauri para Windows. O Rust lê porta e token do pipe herdado, fecha a
entrada padrão e autentica no WebSocket do daemon em loopback. A janela
principal carrega o bundle, envia comandos e recebe respostas e eventos por
IPC. A credencial fica no processo Rust. A janela reconecta automaticamente
após uma queda. Uma reconexão ou um recarregamento da interface sempre volta ao
daemon que abriu a janela, cuja porta e token vieram no pipe, esteja ele ativo
ou não. Se esse daemon sair, a janela segue tentando até o lançador
substituí-la pela janela do novo daemon ou fechá-la ao desistir. Ao trocar de
conexão, a casca avisa a anterior de que ela terminou. Os comandos de IPC
conferem a URL do documento principal, não a origem de quem chama; por isso a
política de conteúdo da janela proíbe frames e objetos incorporados
(`frame-src 'none'`, `object-src 'none'`). A cada conexão, a casca exige que o daemon comprove conhecer
o token recebido no pipe antes de enviar a credencial ou encaminhar
comandos. A [configuração da interface](../../ui/README.md) define a espera,
o período de estabilidade e o limite de tentativas. Uma prova ausente ou
inválida bloqueia novas conexões nessa execução da casca e orienta a reabrir
pelo lançador para obter a credencial da execução atual. Se a prova estiver
ausente, confira se a casca e o daemon têm a mesma versão.

Fechar a janela a oculta na bandeja. A opção de saída encerra apenas a casca.
Em uma nova execução do daemon, o lançador substitui a janela para usar a
nova porta e o novo token. Se o daemon sair, o lançador espera entre as
tentativas de reiniciá-lo. Ao atingir `crash_window_limit` falhas, fecha a
janela sem abrir outra. O arquivo `restart` na pasta de controle reinicia o
daemon, zera a desistência e abre outra janela com a nova credencial. O host
mantém as sessões existentes. Os parâmetros da janela ficam na
[configuração do lançador](../launcher/README.md).

O job `shell-check` empacota a interface e abre a janela real. A opção interna
`--smoke-test` exige a feature `desktop-smoke` e encerra a casca após encaminhar pelo IPC a resposta bem-sucedida à consulta `harness.list` enviada pela janela.
O teste verifica que daemon e host continuam ativos.
