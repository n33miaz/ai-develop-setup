# ads-ptyhost

Processo estável que segura as sessões de terminal e o anel de replay.

## Peças

| Arquivo | O que faz |
|---|---|
| `src/wire.rs` | Contrato `pty-host/1`: codec do quadro e tipos das mensagens. As formas JSON são os tipos gerados de `ads-contracts`, reexportados com os nomes do host; ficam escritos aqui o quadro binário, a escolha do erro JSON-RPC de um pedido malformado e as conversões onde o host guarda o valor de outra forma (ver [`swap-map.md`](../contracts/swap-map.md)). Conferido contra os arquivos dourados de `contracts/golden/` e contra os bytes fixados em `tests/wire_bytes.rs`. Só este arquivo conhece o layout de bytes e os nomes dos campos JSON. |
| `src/channel.rs` | Canal local: named pipe no Windows, socket Unix nos demais, aberto só para o usuário atual. Também o lado do cliente (`connect`), que no Windows só entrega a conexão depois de confirmar que o pipe é servido pelo usuário atual. |
| `src/server.rs` | Servidor do contrato: uma tarefa por cliente, sessões compartilhadas por todos e independentes de qualquer conexão. |
| `src/types.rs` | Tipos internos da sessão (`Seq`, `OutputChunk`, `SpawnSpec`, erros). O nome da sessão (`SessionId`) vem de `wire.rs`: é opaco e escolhido pelo cliente. |
| `src/backend.rs` | Traço `PtyBackend`: a costura entre a sessão e quem fornece o terminal. |
| `src/portable.rs` | Backend real, sobre `portable-pty` (ConPTY no Windows). Só este arquivo cita a biblioteca. |
| `src/fake.rs` | Backend falso, determinístico e sem processo real, para os testes. |
| `src/win.rs` | Chamadas do Windows: o Job Object que encerra a árvore de processos, o named pipe restrito ao usuário, a conferência de quem serve um pipe e as sondas de processo, de handle e de lista de acesso. Todo `unsafe` do crate fica aqui. |
| `src/replay.rs` | Anel de replay: pedaços de saída numerados por `seq`, limitado em bytes. |
| `src/session.rs`, `src/host.rs` | Sessão (bombeia a saída para o anel e para os assinantes) e o conjunto de sessões abertas. |

Os parâmetros e seus padrões estão em [`defaults/ptyhost.toml`](../../defaults/ptyhost.toml).

## Como roda

`ads-ptyhost [--config <arquivo>]` sobe o servidor e imprime uma linha, `ads-ptyhost <versão> listening on <canal>`, quando está pronto para aceitar clientes. O arquivo de `--config` é a camada do usuário: um TOML com as chaves de `defaults/ptyhost.toml` que se quer trocar.

O canal é do usuário e de mais ninguém:

- Windows: named pipe `\\.\pipe\<channel_name>.<SID do usuário>`. A lista de acesso do pipe tem uma entrada só, a do usuário que roda o processo; clientes remotos são recusados; e o processo não sobe se já existir um pipe com esse nome (ninguém se põe no lugar dele).
- Do lado do cliente (Windows): qualquer usuário da máquina pode criar um pipe com esse nome antes de o host subir. Por isso `channel::connect` abre o pipe e, antes de entregar a conexão (portanto antes de qualquer byte, inclusive o `host.hello`), confere duas coisas: o processo que serve o pipe (`GetNamedPipeServerProcessId`) roda como o usuário atual, lido do token dele; e o dono do objeto do pipe, posto por quem criou o nome, é o usuário atual ou o dono padrão do token dele (o grupo de administradores, num processo elevado com essa política). As duas precisam passar, e falha em qualquer chamada é recusa: o número de processo pode ser reaproveitado depois que o servidor sai, e o dono do objeto não diz quem segura o lado servidor agora. Abrir o processo ou o token de outro usuário é negado a uma conta comum, o que também vira recusa.
- Demais sistemas: socket `$XDG_RUNTIME_DIR/<channel_name>.sock` com modo `0600`. Um arquivo de socket deixado por uma execução que caiu é substituído; qualquer outra coisa no caminho impede a subida.

## Regras do contrato

O contrato está em [`contracts/README.md`](../../contracts/README.md). O que este processo decide dentro dele:

- `host.hello` é o primeiro pedido. Qualquer outro antes dele volta `-32600`; versão diferente de `pty-host/1` volta `-32009` e a conexão continua esperando um `host.hello` válido.
- JSON ilegível volta `-32700` com `id` nulo; envelope fora do contrato, `-32600`; método desconhecido, `-32601`; `params` fora do contrato, `-32602`. A conexão continua.
- Quadro de tipo desconhecido, maior que `max_frame_bytes`, ou de saída (`0x02`) vindo do cliente encerra a conexão. As sessões não são afetadas.
- Um cliente tem no máximo uma assinatura por sessão. Um novo `session.subscribe` encerra a anterior entre dois quadros: depois da resposta, todo quadro de saída daquela sessão pertence à assinatura nova.
- Quando a saída de uma sessão termina, cada assinante que recebeu tudo recebe `session.exited`. Quem não está assinando vê o estado em `session.list`.
- Assinante lento recebe o que já estava na fila dele e depois `subscription.closed` com `reason: "slow_subscriber"`; fechar a sessão avisa os assinantes com `reason: "session_closed"`. Nos dois, `last_seq` é o último pedaço entregue naquela assinatura.
- Cliente que cai (no meio de um quadro ou não) não afeta a sessão: ao voltar, assina a partir do último `seq` que recebeu inteiro, mais 1.
- Enquanto o terminal de um `session.open` está sendo criado, o nome e a vaga já estão reservados: outro `session.open` com o mesmo nome volta `-32002` e a reserva conta em `max_sessions`. A sessão só aparece em `session.list` e só aceita os outros pedidos (que até lá voltam `-32001`) depois de criada. Se a criação falha, nome e vaga são devolvidos.
- Um pedido que espera pelo terminal (criar, escrever, redimensionar, encerrar) segura só a conexão que o fez. Os outros clientes continuam sendo atendidos e a saída das outras sessões continua saindo: esse trabalho roda fora das threads do atendimento, e a trava do conjunto de sessões só é segurada para consultar ou alterar o conjunto.
- Uma assinatura que acabou (o cliente saiu, assinou de novo ou ficou para trás) deixa de ocupar memória na hora, mesmo que a sessão nunca mais produza saída.

## Regras do replay

- Cada leitura do terminal vira um pedaço com `seq` crescente a partir de 1; o número nunca se repete.
- O cliente assina "a partir do seq N" (inclusive) e recebe primeiro o que o anel guarda e depois a saída ao vivo, em ordem, sem buraco e sem duplicata.
- Pedido anterior ao que o anel ainda guarda, ou à frente do que já foi numerado, é recusado com erro que informa o limite; nada é pulado em silêncio.
- Assinante lento, que enche a própria fila, tem a assinatura encerrada; ele reconecta a partir do último `seq` que viu e não perde nada (enquanto o anel alcançar).
- O anel continua disponível depois que o processo termina, até a sessão ser fechada.

## Limites conhecidos

- No Windows o processo raiz entra no Job Object logo depois de criado, não antes de começar a rodar. Um neto iniciado nesse instante ficaria fora do job. Fechar essa janela pede criar o processo suspenso, o que a biblioteca não expõe; é o que o plano B (ConPTY direto) resolve.
- O ConPTY abre perguntando a posição do cursor e segura o processo filho até a resposta. Como pode não haver cliente conectado, o backend responde "canto superior esquerdo" ao abrir. A pergunta continua na saída; um cliente que também responda manda uma resposta repetida ao programa.
- Fora do Windows só o processo raiz recebe o sinal de encerramento; o resto da árvore depende do fechamento do terminal. O produto roda no Windows; o caminho Unix existe para os testes do CI em Linux.
- O processo roda até ser encerrado por quem o iniciou; não há pedido de desligamento no contrato `pty-host/1`. Quem o inicia destacado é o lançador, que ainda não existe.
- Um cliente conectado que para de ler não segura memória além da fila dele (`subscriber_queue_chunks`), mas a conexão fica aberta até ele sair.
- Não há teto para o número de conexões nem de assinaturas vivas ao mesmo tempo, nem prazo para um pedido que espera pelo terminal: um terminal que nunca responde segura a conexão que pediu e uma thread de trabalho até a sessão ser encerrada.
- Fora do Windows, sem `XDG_RUNTIME_DIR` o processo só sobe com `channel_path` preenchido.
- Fora do Windows, o cliente não confere quem serve o socket: a proteção é a pasta `XDG_RUNTIME_DIR`, só do usuário. Com `channel_path` apontando para outro lugar, quem escolhe o caminho responde por ele.
- A recusa de um pipe servido por outro usuário não tem teste automático com duas contas. O que os testes conferem, sempre sem nenhum byte chegando ao servidor:
  - a decisão, com identificadores sintéticos, e o pipe do próprio host passando;
  - um servidor filho, do mesmo usuário, que fecha o próprio processo ou o próprio token a todos menos à conta do sistema, como estaria o servidor de outra conta: o cliente recusa na abertura do processo ou do token, pela entrada pública do daemon, e o servidor não recebe o `host.hello`;
  - um pipe real lido pelas consultas reais e comparado com outra identidade (a recusa vem da comparação, não de uma consulta que falhou), e um arquivo comum no lugar do pipe (abre, mas ninguém o serve);
  - falhas repetidas em cada consulta, inclusive depois de abrir processo e token, que nomeiam a consulta que falhou e não deixam handle aberto (`tests/pipe_owner.rs` e os testes de `src/win.rs`).

  Os casos com outra conta e a matriz de elevação ficam em testes manuais, ignorados por padrão e ainda não rodados ([testes manuais](#testes-manuais-do-dono-do-pipe)). Um processo de outro usuário servindo um pipe de dono aceito só tem o teste sintético. Um administrador da máquina passa por qualquer uma dessas conferências e está fora do que elas protegem.
- Um host iniciado elevado (de um terminal de administrador) pode dar o pipe ao grupo de administradores; um cliente não elevado do mesmo usuário recusa esse pipe, porque o dono padrão dele é o próprio usuário. Na direção contrária, um cliente elevado aceita o pipe de um host não elevado do mesmo usuário, cujo dono é o próprio usuário. Isso é o que o código decide; na máquina, ainda não foi conferido.

## Testes manuais do dono do pipe

Dois testes de `tests/pipe_owner.rs` são ignorados por padrão, porque pedem outra conta ou outro nível de elevação. Sem as variáveis que leem, falham com a instrução de onde está este texto.

- `serve_for_another_account`, rodado pela outra conta: cria `ADS_PIPE_OWNER_ENDPOINT` (por exemplo `\\.\pipe\ads-owner-check`) com lista de acesso que deixa entrar `ADS_PIPE_OWNER_CLIENT_SID` (o identificador do usuário cliente, de `whoami /user`), espera um cliente por até dois minutos e falha se receber qualquer byte.
- `connect_to_a_prepared_server`, rodado pelo usuário: conecta em `ADS_PIPE_OWNER_ENDPOINT` com a conferência real e compara com `ADS_PIPE_OWNER_EXPECT`.

Como rodar: `cargo test -p ads-ptyhost --test pipe_owner --no-run` mostra o caminho do binário; copie-o para uma pasta que a outra conta leia e execute. A outra conta roda o binário copiado com `--ignored --exact serve_for_another_account` num terminal aberto com `runas /user:<conta> cmd`; o usuário roda `cargo test -p ads-ptyhost --test pipe_owner -- --ignored --exact connect_to_a_prepared_server`. Nas linhas com host, o servidor é o `ads-ptyhost` com `channel_path` igual ao endpoint.

Há dois jeitos de recusar, e o teste distingue os dois:

- `unavailable`: a lista de acesso do pipe barra a abertura. O host só deixa entrar o próprio usuário, então o host de outra conta nem chega a ser consultado.
- `refused`: a abertura passa, e a recusa vem depois de ler quem serve o pipe (`NotOwnedByUser`), sem nenhum byte enviado.

Resultado esperado pelo código, ainda não conferido na máquina:

| Servidor | Cliente | `ADS_PIPE_OWNER_EXPECT` |
|---|---|---|
| `serve_for_another_account`, de outra conta comum | qualquer | `refused` |
| `serve_for_another_account`, de outro administrador, elevado (dono: grupo de administradores) | elevado | `refused` |
| host de outra conta, elevado ou não | qualquer | `unavailable` |
| host do usuário, não elevado | não elevado | `accepted` |
| host do usuário, elevado | elevado | `accepted` |
| host do usuário, elevado, com o pipe dado ao grupo de administradores | não elevado | `refused` |
| host do usuário, não elevado | elevado | `accepted` |
