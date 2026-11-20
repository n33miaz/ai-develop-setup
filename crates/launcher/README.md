# ads-launcher

Supervisor dos processos locais de uma instalação.

Execute `ads-launcher [--config <arquivo>]` com os binários do daemon e do host
ao lado do lançador. Com `--config`, caminhos relativos partem da pasta do TOML.
O arquivo sobrepõe as chaves de [`defaults/launcher.toml`](../../defaults/launcher.toml).
Use uma pasta de controle exclusiva da instalação, gravável apenas pela sua conta.
A trava de sistema operacional fica ativa enquanto o lançador está rodando.
O arquivo da trava permanece no disco e pode ser reutilizado após a saída.

O lançador inicia o host destacado e o daemon. A saída padrão do daemon continua
disponível no terminal para obter a URL autenticada. As opções `daemon_config`
e `ptyhost_config` passam `--config` ao processo correspondente. O arquivo do
host também é passado ao daemon via `--host-config`, para que ambos usem o
mesmo canal. Os filhos recebem apenas as variáveis de sistema necessárias
para a execução local e a resolução do canal.

Crie o arquivo vazio `restart` na pasta de controle para reiniciar o daemon e
zerar seu contador de falhas consecutivas. Crie `stop` para encerrar o daemon e
o lançador. Se os dois arquivos estiverem presentes, a parada prevalece. O host
permanece em execução com as sessões existentes. Enquanto supervisionado, ele
só é reiniciado após sair. Para retomar o acompanhamento do host em uma nova
execução do launcher, encerre o host anterior depois de fechar suas sessões.
Se o host anterior continuar ativo, ele mantém a exclusividade do canal e o
novo processo do host acaba desistindo após atingir o limite de tentativas.

O JSON `state.json` é substituído atomicamente. Ele traz o PID do lançador e,
para cada filho, o PID acompanhado ou `null`, o número de novas tentativas, as
falhas rápidas consecutivas, o indicador de desistência, a última saída e o
último erro ao iniciar. O campo `stopped` marca a parada solicitada. Os PIDs
são um instantâneo e podem ficar desatualizados se o lançador sair abruptamente.
O arquivo tem duração local e pode ser lido por outras ferramentas autorizadas
pela pasta de controle.

Este passo acompanha saídas de processos. A consulta de saúde e confirmação do
commit dependem de um contrato ainda ausente no daemon. Manifesto, troca de
versão e rollback ficam para a entrega de atualização da
[ADR 0015](../../docs/adr/0015-ci-built-artifact-with-verified-self-update.md).
