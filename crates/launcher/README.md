# ads-launcher

Supervisor dos processos locais de uma instalação.

Execute `ads-launcher [--config <arquivo>]` com os binários do daemon e do host
ao lado do lançador. Os programas e `control_dir` relativos partem da pasta
do executável. Com `--config`, só os caminhos `daemon_config` e
`ptyhost_config` relativos partem da pasta do TOML.
O arquivo sobrepõe as chaves de [`defaults/launcher.toml`](../../defaults/launcher.toml).
Use uma pasta de controle exclusiva da instalação. Se ela não existe, o lançador
a cria acessível só pela conta atual: no Windows, com uma lista de acesso protegida
de uma entrada só, a do usuário, herdada pelos arquivos de dentro; no Unix, com modo
0700. Os arquivos de trava e de estado nascem com a mesma restrição, posta na criação
(lista protegida do usuário no Windows, modo 0600 no Unix), sem depender da herança.
Uma pasta ou trava já existente é recusada, com erro e sem iniciar nada, se o dono
não for a conta atual ou se outra conta tiver qualquer acesso a ela. No Windows, só o
sistema local e o grupo Administradores, que já podem assumir qualquer arquivo da
máquina, são tolerados além do usuário. Para corrigir, apague a pasta ou a trava para
o lançador recriá-la, ou restrinja-a à sua conta.
A pasta é aberta uma vez e conferida nesse identificador aberto; trava, estado e
sinais são abertos, movidos e apagados relativos a ele, nunca pelo caminho de novo,
e sem seguir link nem ponto de reanálise. Assim, trocar a pasta ou um ancestral por
outra depois da conferência não redireciona nada. No Windows, enquanto o lançador
roda, a pasta e as pastas acima dela também não podem ser renomeadas.
Uma trava de sistema operacional limita a execução a um lançador por pasta de
controle e fica ativa enquanto ele roda.
Os arquivos de trava são reutilizados após a saída. O daemon supervisionado
mantém sua própria trava (`daemon.lock`) na mesma pasta. Ao sair pelo sinal
`stop`, o lançador encerra o daemon. Na saída do supervisor que passa pela
limpeza de processos, ele também tenta encerrá-lo. Se o lançador
for encerrado à força, o daemon pode continuar vivo, mas outro daemon da mesma
instalação receberá uma recusa até o primeiro sair. Para recuperar a supervisão,
encerre o daemon anterior e peça um reinício pelo sinal `restart`.

O lançador inicia o host destacado e o daemon. No Windows, coloque também
`ads-shell.exe` ao lado do lançador. O modo desktop liga a saída de inicialização
do daemon diretamente à entrada padrão da casca por um pipe anônimo. O Rust
da casca autentica a conexão e encaminha mensagens à janela por IPC. Se a casca
sair pela bandeja ou falhar, o daemon e o host continuam sob supervisão. Use o
sinal `restart` para abrir outra janela com a nova credencial.

O lançador fecha a casca que abriu no sinal `stop`, em Ctrl+C ou Ctrl+Break no
console do Windows, quando desiste do daemon depois de `crash_window_limit`
falhas, ao substituí-la e ao sair do escopo do supervisor. No Windows, um pipe
anônimo por instância entrega o byte `Q` à casca, que encerra sua própria aplicação.
A variável `ADS_SHELL_CLOSE_PIPE` indica à casca que o identificador de erro padrão
é a ponta de leitura desse pipe. No Unix, o pedido usa `SIGTERM`. Se a entrega
falhar, o lançador registra o erro e segue para o término forçado. Durante o laço
do supervisor, o erro também aparece em `last_error`. Se a casca ainda estiver
viva após `shell_close_timeout_ms`, o lançador força o término pelo handle do
filho no Windows. Se o sistema encerrar o lançador sem executar a limpeza do
supervisor, a janela pode continuar aberta.

Para usar o navegador, configure `shell_autostart = false` e reinicie a instalação.
Nesse modo a URL autenticada volta a aparecer na saída padrão do daemon.

| Chave | Padrão | Uso |
|---|---|---|
| `shell_program` | `ads-shell.exe` no Windows | Caminho absoluto ou relativo ao lançador. |
| `shell_autostart` | `true` no Windows | Abre a janela ao iniciar o daemon. Em outros sistemas, vale o modo navegador. |
| `shell_width`, `shell_height` | 960, 640 | Tamanho inicial em pixels lógicos, de 1 a 65535. |
| `shell_start_minimized` | `false` | Minimiza a janela na abertura. |
| `shell_close_timeout_ms` | 3000 | Prazo entre o pedido de fechamento da janela e o término forçado. `0` força o término na hora. |

Altere as chaves no TOML passado por `--config` e reinicie o lançador.
As opções `daemon_config`
e `ptyhost_config` passam `--config` ao processo correspondente. O arquivo do
host também é passado ao daemon via `--host-config`, para que ambos usem o
mesmo canal. `child_env_allow` define quais variáveis os filhos herdam do
lançador, com o padrão em `defaults/launcher.toml` e as proibições de
`defaults/harnesses.toml`. Variáveis `WEBVIEW2_*` ficam fora do ambiente da casca. Para uma variável herdada chegar a uma ferramenta
iniciada pelo daemon, inclua seu nome nas duas listas, `child_env_allow` e
`[env] allow`. As atribuições de `[env.set]` são definidas pelo daemon.

Crie o arquivo vazio `restart` na pasta de controle para reiniciar o daemon e
zerar seu contador de falhas na janela móvel. Crie `stop` para encerrar o daemon,
a janela da casca e o lançador. Um arquivo de sinal cujo dono não é a conta atual, ou que no
Windows dá acesso a outra conta, fica no lugar, é ignorado e aparece em `last_error`.
O sinal apagado é o mesmo arquivo conferido. Ao adquirir a trava da pasta de
controle, o lançador descarta um `stop` válido que já estava presente e registra
esse descarte em `stop_pending_at_startup`. Para solicitar a parada da execução atual,
aguarde a publicação de seu estado antes de criar o arquivo. Durante a execução,
a parada prevalece sobre `restart`. O lançador consome pedidos de parada pendentes
também ao encerrar por interrupção de console. O host
permanece em execução com as sessões existentes. Enquanto supervisionado, ele
só é reiniciado após sair. Reassumir a supervisão de um host que ficou ativo
depois de `stop` ainda está pendente. Para retomar a supervisão em uma nova
execução do lançador, encerre o host anterior depois de fechar suas sessões.
Se o host anterior continuar ativo, ele mantém a exclusividade do canal e o
novo processo do host acaba desistindo após atingir o limite de tentativas.
Para voltar a tentar após a desistência do host, reinicie o lançador.

O JSON `state.json` é substituído atomicamente. Ele traz o PID do lançador e,
para cada filho, o PID acompanhado ou `null`, o número de novas tentativas,
os reinícios solicitados, as falhas na janela móvel, o indicador de desistência,
a última saída e o último erro ao iniciar. Saídas e falhas ao iniciar contam
para `crash_window_limit` dentro de `crash_window_ms`. Use essas chaves
no lugar de `rapid_exit_limit` e `rapid_exit_ms` em configurações antigas. A espera cresce até
`restart_max_ms`. O campo `stopped` marca a parada solicitada.
Erros no laço são informados no terminal e publicados em `last_error` ou
`state_write_error` quando o arquivo volta a aceitar gravação. A consulta
dos sinais e a publicação pendente são repetidas no ciclo seguinte. Os PIDs
são um instantâneo e podem ficar desatualizados se o lançador sair abruptamente.
O arquivo tem duração local e só a conta atual o lê.

A consulta de saúde e a confirmação do
commit dependem de um contrato ainda ausente no daemon. Manifesto, troca de
versão e rollback ficam para a entrega de atualização da
[ADR 0015](../../docs/adr/0015-ci-built-artifact-with-verified-self-update.md).
