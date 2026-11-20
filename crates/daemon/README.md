# ads-daemon

O executável serve a API local por WebSocket em `127.0.0.1`, com porta efêmera e
autenticação por execução ([ADR 0020](../../docs/adr/0020-local-api-authentication.md)).
Claude e Codex são abertos pelo host de terminais com programa, argumentos e
pasta definidos no TOML do usuário. Ferramenta sem programa recebe
`HARNESS_NOT_CONFIGURED`. Ferramenta desconhecida, programa ou pasta
inexistente e apelido de pasta fora da configuração recebem `INVALID_PARAMS`.

## Abrir Claude ou Codex no navegador

1. Na raiz do repositório, inicie o host de terminais em um terminal.

   ```powershell
   cargo run -p ads-ptyhost -j 2
   ```

2. Em outro terminal, use `where.exe claude.exe` e `where.exe codex.exe` para
   localizar os executáveis instalados. O programa precisa ser um `.exe`; não
   aponte para `claude.cmd` nem `codex.cmd`. Se o Codex foi instalado pelo npm,
   `where.exe` acha só `codex.cmd`; o executável real fica em
   `<prefixo do npm>\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`
   (o prefixo aparece em `npm prefix -g`). Escreva o caminho já expandido:
   `%VAR%`, `$VAR`, `~`, `..` e nomes curtos como `NOME~1` são recusados.

   Salve o TOML fora do repositório e dentro do seu perfil, por exemplo em
   `C:\Users\voce\ads-local\harnesses.toml` (troque `C:\Users\voce` pela sua
   pasta de perfil, escrita por extenso). Crie também, dentro do perfil, uma
   pasta de teste vazia, sem nada importante, como destino do apelido
   `default`. Se só tiver uma ferramenta instalada, omita a tabela da outra.

   ```toml
   [workdirs]
   default = 'C:\Users\voce\ads-local\pasta-de-teste'

   [tools.claude]
   program = 'C:\Users\voce\.local\bin\claude.exe'

   [tools.codex]
   program = 'C:\Users\voce\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe'
   ```

   **O TOML, a pasta de trabalho e o programa precisam ser graváveis só por
   você.** Não use a raiz de um disco de dados (como `D:\`) nem pastas
   compartilhadas: no Windows elas costumam aceitar escrita de qualquer usuário
   autenticado. Quem puder gravar ali pode trocar o programa ou os argumentos,
   ou deixar na pasta de trabalho arquivos de instrução e configurações com
   comandos automáticos que o agente lê e executa com a sua conta.

3. Na raiz, compile o daemon e inicie o executável com o caminho absoluto do
   TOML. Rodar o executável direto, em vez de `cargo run`, evita que as pastas
   de compilação do repositório entrem no `PATH` que o agente herda.

   ```powershell
   cargo build -p ads-daemon -j 2
   .\target\debug\ads-daemon.exe --config 'C:\Users\voce\ads-local\harnesses.toml'
   ```

4. Em um terceiro terminal, inicie a interface.

   ```powershell
   Set-Location ui
   npm ci
   npm run dev
   ```

   Conectada, a interface pergunta ao daemon (`harness.list`) quais ferramentas
   estão ligadas e mostra um botão para cada uma; `VITE_CORE_HARNESSES` não é
   necessária.

   Depois que o Vite confirmar a porta, abra no navegador a URL completa
   impressa pelo daemon
   (`http://127.0.0.1:5173/#core=ws://127.0.0.1:<porta>&token=<token>`) e
   clique em **Abrir claude** ou **Abrir codex**.

O agente dentro do terminal edita arquivos e executa comandos conforme as
permissões dele. Argumentos que desligam as confirmações ou a área isolada da
ferramenta, somados ao controle do terminal pela API, dão a quem usa a interface
acesso total à sua conta; não os coloque em `args` sem entender esse efeito.

A saída padrão contém somente essa linha. O token de 32 bytes aleatórios vem do
gerador do sistema e existe só em memória; o fragmento não faz parte do pedido
HTTP. A interface envia `auth.hello` como primeira mensagem. Antes dela, mensagem
inválida, token errado ou prazo vencido encerram a conexão. `Host` e `Origin`
inválidos recebem o mesmo HTTP 403 antes do upgrade; cabeçalhos repetidos,
origem ausente e query string também são recusados.

O arquivo de `--config` é opcional. As tabelas `[tools.claude]`,
`[tools.codex]`, `[workdirs]`, `[accounts]` e `[env]` sobrepõem os
[padrões das ferramentas](../../defaults/harnesses.toml). `[env]` e
`[workdirs]` valem para as duas. As tabelas de perfil em `[accounts]`
são aceitas para as duas ferramentas citadas. Entradas para outras ferramentas
são recusadas. Sem arquivo, ambas ficam desligadas. As chaves
da API na raiz do mesmo TOML sobrepõem os padrões do daemon listados abaixo.
Reinicie o daemon para reler a configuração. A configuração inválida impede a
partida e a mensagem omite o conteúdo do arquivo.
O host usa o canal padrão do usuário, cuja identidade é conferida antes da conexão.
`--host-config <arquivo>` permite selecionar outra configuração do host (inclusive
um canal exclusivo para testes), com as mesmas validações de `HostConfig`.

`--config` e `--host-config` aceitam só caminho absoluto literal de um arquivo
comum de até 1 MiB (`startup::MAX_CONFIG_BYTES`, fixo porque protege a leitura do
próprio arquivo de configuração). São recusados caminho relativo, de rede
(`\servidor\...`), de dispositivo ou pipe (`\.\`, `\?\`), com variável, `~`,
`..` ou nome curto, além de pastas.

O ambiente do processo pai é lido uma vez, na partida, e filtrado na hora pela
política `[env]`; o daemon guarda só o resultado, sem as variáveis negadas.

## Comandos, limites e recuperação

Os nomes, valores e significados dos erros emitidos pela API local estão no
[contrato de códigos de erro](../../contracts/schemas/contract.local-api.error-code.v1.schema.json).

`api::Launcher::prepare` recebe os parâmetros gerados de `session.open` (ferramenta,
apelido de pasta, identificador e tamanho) e devolve `OpenParams` do ptyhost.
O daemon confere que identificador e tamanho foram preservados. Cada adaptador
aplica de novo a política de ambiente ao ambiente já filtrado antes de enviar
os parâmetros ao host.
Os tipos gerados validam todos os comandos e eventos contra os contratos.
`session.write`, `session.resize` e `session.close` usam o cliente do host.
`events.subscribe` envia eventos com `seq > after_seq`, seguidos dos novos,
incluindo uma assinatura feita antes de abrir a sessão. Nenhum `fs.*` está registrado.
Todo comando remoto é recusado, mesmo com autenticação válida.

`harness.list` (só depois de `auth.hello`; `params` leva apenas o `_meta`
obrigatório de todo comando) responde
`{"harnesses": [...], "workdir_aliases": [...]}`
([`command.harness.list.response:1`](../../contracts/README.md#api-local)): os
identificadores das ferramentas com `program` preenchido, na ordem `claude`,
`codex`, e os apelidos de `[workdirs]` em ordem de nome. Só nomes: programa,
argumentos, ambiente e caminhos nunca saem do daemon, e o contrato fechado da
resposta não tem onde levá-los. A lista é montada uma vez, na partida (reinicie
para reler a configuração), e não confere o disco: uma ferramenta listada ainda
pode receber `INVALID_PARAMS` em `session.open` se o programa ou a pasta não existir.
Como só lê, não passa pela cache de idempotência: repetir a chave devolve a
lista de novo, nunca `replayed`. `api::Launcher::catalog` fornece a lista; o
padrão é vazio, como em `DisabledLauncher`. A resposta é a única mensagem
que precisa da `local-api.server-message:2`; quem não envia `harness.list`
continua recebendo só mensagens da `:1`. Um daemon anterior ao comando o recusa
com `METHOD_NOT_FOUND`, e a interface mostra que o daemon precisa ser atualizado.

Padrões em [defaults/daemon.toml](../../defaults/daemon.toml), aplicados ao iniciar:

| Chave | Padrão | Efeito e alternativa |
|---|---|---|
| `ui_base_url` | `http://127.0.0.1:5173` | URL HTTP da interface em localhost, 127.0.0.1 ou [::1], sem caminho, query ou fragmento; substitua pela interface cuja porta foi verificada. |
| `allowed_origins` | `[]` (deriva `http://127.0.0.1:5173`) | Vazia deriva somente a origem de `ui_base_url`; lista explícita substitui a derivação, sem curingas ou caminhos. |
| `auth_timeout_ms` | 5000 | Prazo total único do upgrade e da primeira mensagem; reduza para limitar conexões ociosas. |
| `max_sessions` | 32 | Sessões ainda mantidas no host e, em separado, históricos de sessões fechadas. Aumentar permite mais sessões e usa mais memória. |
| `max_write_bytes` | 65536 | Bytes decodificados por escrita; pode diminuir, nunca exceder o contrato. |
| `events_per_stream` | 256 | Eventos retidos por stream; aumente para ampliar o replay. |
| `idempotency_keys` | 1024 | Chaves aceitas retidas em FIFO; aumente para ampliar a janela de deduplicação. |
| `command_queue` | 64 | Pedidos esperando o host; aumente para mais concorrência. |
| `max_connections` | 32 | Conexões autenticadas simultâneas; excedentes são fechadas ao autenticar. |
| `max_pending_auth` | 32 | Tarefas de conexões ainda sem autenticar, inclusive as que estão sendo expulsas; separadas das autenticadas. Com a fila cheia, a mais antiga que ainda não fez upgrade é expulsa e a nova entra quando a vaga volta; se todas já fizeram upgrade, a nova é recusada. |
| `accept_retry_ms` | 50 | Espera após erro de aceitação TCP; reduz tentativas sob falta de recursos. |
| `max_message_bytes` | 131072 | Limite de mensagem e quadro WebSocket recebido; reduzir pode impedir escritas grandes. |
| `send_timeout_ms` | 5000 | Prazo por envio; cliente lento é desconectado. |
| `host_timeout_ms` | 10000 | Prazo único por comando no host e na conexão inicial. Cada chamada ao host começa dentro do prazo, e uma resposta já recebida vale mesmo se o prazo vencer enquanto o daemon trata um evento do host. Falha encerra o serviço para evitar repetir efeito incerto. Duas exceções não encerram: no fechamento automático o prazo esgotado conta como tentativa falha, e na preparação depois de um `session.open` confirmado ele responde `SESSION_SETUP_FAILED`, como qualquer outra falha dessa etapa (ver abaixo). |
| `retired_sessions` | 1024 | Ids de sessões podadas que não podem ser reabertos nesta execução (acima do limite, o mais antigo volta a ser aceito) e, em separado, streams assinados antes do `session.open` (acima do limite, a espera mais antiga termina com `SUBSCRIPTION_REPLAY_UNAVAILABLE` no id da assinatura, mesmo se a conexão estiver ociosa). |
| `subscription_ids_per_stream` | 2 | Ids de assinatura por conexão: este valor vezes `max_sessions`. Reassinar a mesma encarnação na mesma conexão soma um id, que só sai quando o stream termina; ids de encarnação já terminada não contam. A interface mantém uma assinatura por stream e conexão, então raramente passa de um id por stream; aumente para clientes que reassinam muito. |
| `close_retries` | 3 | Tentativas do fechamento automático de sessão invalidada, de `open` que falhou ou de sessão que terminou sozinha; esgotadas numa sessão invalidada, a assinatura do stream recebe `SESSION_CLOSE_PENDING`. |
| `close_retry_ms` | 1000 | Espera entre tentativas do fechamento automático. |

Os inteiros de capacidade aceitam de 1 a 1048576 (`max_message_bytes`: mínimo
1024); prazos aceitam de 1 a 300000 ms. Valores altos elevam o teto de memória.
Quando há uma lacuna irrecuperável, o daemon invalida o stream e solicita o
fechamento ao host. Recusa do host (fora `UnknownSession`) ou prazo esgotado
repetem o pedido até `close_retries` vezes, com aviso no erro padrão sem
conteúdo. Quem já assinava o stream não recebe nada enquanto as tentativas
correm; o resultado final chega com o id da assinatura: `SESSION_CLOSE_PENDING` se as tentativas se esgotarem, `SUBSCRIPTION_REPLAY_UNAVAILABLE` se o host confirmar. Só essa
assinatura termina. Um `events.subscribe` novo desse stream recebe `SUBSCRIPTION_REPLAY_UNAVAILABLE`
durante as tentativas e `SESSION_CLOSE_PENDING` depois delas, até um `session.close` confirmado.
`session.close` continua aceito enquanto o registro da sessão invalidada estiver
retido. Cada tentativa vale só para a instância da sessão que a gerou e é
descartada quando a vaga é devolvida. Se um `open` falhar depois de o host abrir o
terminal, seja pelo prazo, seja por recusa do host ou do registro, a resposta é
sempre `SESSION_SETUP_FAILED`, inclusive com a fila de pedidos do cliente do host
cheia; só uma falha que encerra o serviço, como a perda do host ou a parada do cliente dele por
qualquer causa, responde `INTERNAL_ERROR`. Recusas anteriores ao terminal aberto mantêm o próprio código.
Com `SESSION_SETUP_FAILED`, o fechamento vai para a mesma fila, com prazo próprio por tentativa;
o registro e a vaga ficam até o host confirmar, e então o id volta a ser aceito.
A conexão que abriu não recebe nenhum evento dessa sessão: os eventos só seguem
depois da resposta, e o stream invalidado já não os tem. O resultado final chega
a uma assinatura feita antes do `open`: `SUBSCRIPTION_REPLAY_UNAVAILABLE` quando o host confirma o
fechamento, `SESSION_CLOSE_PENDING` quando as tentativas se esgotam.
`UnknownSession` no fechamento libera a vaga; num stream válido (não invalidado) que ainda não
tinha evento terminal, grava `ended` com `reason: "error"`. Um `ended` já gravado fica como está, e
um stream invalidado não recebe `ended`. Quando o processo de uma
sessão termina sozinho, o daemon grava `ended` e pede o fechamento ao host pela mesma fila, sem
esperar a interface, inclusive se um `session.close` do cliente estiver em andamento e o host o
recusar. Cada encarnação tem no máximo uma tentativa pendente, contando a que está em execução; reagendar mantém o saldo e o prazo dela. O fechamento encerra a árvore de
processos que restar e devolve a vaga: um servidor de desenvolvimento ou outro processo que a
ferramenta tenha iniciado e que ainda esteja vivo é encerrado nessa hora, mesmo sem pedido da
interface. O stream liberado mantém os eventos ainda retidos, dentro de `events_per_stream` e até a
poda descrita abaixo. Um `session.close` dessa sessão depois da liberação é aceito sem novo efeito
enquanto o registro dela estiver retido, ou seja, até ela ser podada; depois disso recebe `INVALID_PARAMS`.
O id podado não pode ser reaberto enquanto a lápide existir, e nenhuma tentativa automática da
encarnação anterior alcança uma reaberta. Quando a lápide expira e o id volta a ser aceito, um
`session.close` com ele se refere à sessão atual: o protocolo identifica a sessão só pelo id, e a
interface gera um id novo a cada abertura. Se as tentativas se esgotarem, a vaga fica ocupada e o
stream não muda; um `session.close` do cliente continua aceito e a libera. Ao atingir o limite de registros fechados,
o daemon descarta os mais antigos conforme novas sessões são fechadas. O stream
descartado vira lápide e o mesmo `session_id` é recusado com `CONFLICT` enquanto
ela existir (`retired_sessions`). Cada assinatura fica presa à encarnação do
stream que assinou; quem assina antes do `session.open` fica presa à que o open
preencher. Se ela for podada, removida ou tiver o fechamento resolvido, mesmo
depois de a lápide expirar e o id voltar a ser usado, a assinatura recebe o
primeiro resultado final com o seu id (`SESSION_CLOSE_PENDING` se as tentativas se esgotaram,
`SUBSCRIPTION_REPLAY_UNAVAILABLE` nos demais casos) e termina sem derrubar a conexão, sem nunca entregar
eventos de outra sessão. Uma invalidação no meio de um lote suspende os quadros
seguintes. Assinaturas novas são validadas pelo estado atual.

Uma execução conserva no máximo `2 × max_sessions × events_per_stream` eventos
(um stream que sai do log, mesmo ainda assinado, descarta os seus);
o tamanho de cada saída também é limitado pelo quadro do cliente do host.
Assinaturas por conexão são limitadas a `max_sessions` streams. Assinar de novo, na mesma
conexão, a mesma encarnação de um stream junta o id à assinatura existente: a entrega segue um
cursor só, o menor pedido, e o resultado final chega a cada id. Se o novo `after_seq` for menor que
o que já saiu, os eventos desde ele saem de novo; o consumidor descarta por `seq` o que já tinha. Os
ids por conexão são limitados a `subscription_ids_per_stream × max_sessions`, sem contar os de
encarnação já terminada; acima disso a assinatura é recusada com `CAPACITY_EXCEEDED`. Se a encarnação
assinada antes já terminou, o fim dela chega aos ids antigos antes da confirmação da nova. Filas, conexões e chaves
também têm limites separados. Seq e offset recomeçam ao reiniciar.

Uma chave ainda retida com o mesmo método e parâmetros devolve `replayed: true`,
sem repetir o efeito; outro conteúdo com a mesma chave recebe `CONFLICT`. O id do
pedido pode mudar. A cache guarda só a chave e SHA-256 dos parâmetros, nunca a
entrada do terminal. Depois da expulsão FIFO não há garantia de deduplicação.
`after_seq = 0` pede tudo que resta no stream; cursor descartado ou futuro recebe
`SUBSCRIPTION_REPLAY_UNAVAILABLE`. Se o cliente ficar para trás durante uma assinatura, recebe `SUBSCRIPTION_REPLAY_UNAVAILABLE`
com o id do pedido de assinatura antes de a conexão fechar;
nenhum buraco é ocultado. Reconecte com o último seq recebido ou peça 0
explicitamente para aceitar o histórico ainda disponível.

Eventos `created`, `output`, `resized` e `ended` ficam só na memória, em ordem por
stream; saída usa base64 com offset em bytes. Encerramento solicitado só gera
`ended` depois da saída pendente do host. Recusas explícitas do host voltam ao
cliente com o código do contrato `pty-host/1`,
sem encerrar o serviço. Falha do canal ou prazo do host encerra
o daemon; as sessões continuam sob responsabilidade do ptyhost.
Eventos são consumidos também durante chamadas ao host; a fila limitada aplica
contrapressão ao canal, e o anel limitado recusa assinantes lentos. Falha depois
de abrir uma sessão agenda seu fechamento no host antes de liberar o id.
Lacuna na sequência ou recusa da reassinatura do host invalida somente o stream
afetado, que passa a recusar replay até reiniciar; os demais continuam disponíveis.
Reiniciar não adota sessões antigas nesta implementação. Para desfazer, pare o daemon e volte
ao consumidor anterior do cliente, sem migração de dados.

## Dependências e diagnóstico

`tokio-tungstenite =0.30.0` (MIT, publicada em 11/07/2026) fornece WebSocket sobre
o Tokio já usado pelo host, sem servidor HTTP adicional nem TLS em loopback.
A data supera o mínimo de 14 dias; versão e licença foram conferidas no
[registro da versão](https://docs.rs/crate/tokio-tungstenite/0.30.0) e no
[manifesto publicado](https://docs.rs/crate/tokio-tungstenite/0.30.0/source/Cargo.toml).
`subtle 2.6.1` (BSD-3-Clause) faz a comparação em tempo constante;
`base64 0.23.1` (MIT OR Apache-2.0) codifica os bytes; `futures-util 0.3.34`
(MIT OR Apache-2.0) fornece stream/sink; `time 0.3.55` (MIT OR Apache-2.0)
formata timestamps. Versões resolvidas estão no `Cargo.lock`.

O token e o log de eventos omitem seu conteúdo em `Debug`; erros públicos contêm só códigos
e mensagens estáticas. `pty_client::Event` também omite bytes em `Debug`.
`log 0.4.35` (MIT OR Apache-2.0), já transitiva, usa `max_level_off`:
macros da fachada `log` são eliminadas do executável, inclusive nas dependências,
pois o transporte tem diagnóstico capaz de mostrar payloads. Isso vale também
em debug e não é configurável, por segurança. Não há logger nem arquivo de saída.
`zeroize`, já presente no workspace, limpa o token ao descartá-lo e a URL logo
após a impressão, inclusive quando o envio falha. O `auth.hello` é consumido
antes do laço autenticado; sua cópia do token é limpa. A única
exposição intencional do token é a linha inicial na saída padrão.

Os testes exercitam recusas de upgrade e autenticação, comandos negados,
deduplicação entre conexões, limites, replay e eventos com o backend falso.
Verificam os tipos gerados para cada mensagem, sequência, offsets, tamanhos,
fim natural e encerramento pedido, além da ausência de credenciais e saída em
diagnósticos. O CI Windows inclui mudanças em `defaults/daemon.toml`.

## Cliente do host

A biblioteca expõe `pty_client::PtyClient`,
cliente assíncrono do contrato `pty-host/1` através do codec público do ptyhost.
`OpenParams`, `WriteParams`, `ResizeParams` e `SessionParams` são os tipos gerados
reexportados pelo host; o cliente não constrói JSON nem quadros próprios.

`connect` recebe `HostConfig` para resolver o canal com `channel::endpoint`, abre-o
com `channel::connect` e completa `host.hello`. No Windows o canal só entrega a
conexão depois de confirmar que o pipe é servido pelo usuário atual; senão o erro é
`NotOwnedByUser` e nenhum byte foi enviado (`tests/channel_owner.rs`, também contra um servidor real cujo processo ou token o cliente não consegue abrir). O host precisa
estar disponível; iniciar o processo e decidir quando tentar conectar novamente cabe ao chamador. `over` permite fornecer outro
transporte assíncrono, inclusive `tokio::io::duplex` nos testes.

`subscribe(session_id, after_seq)` começa depois da sequência informada; `Some(0)`
começa em 1 e `None` pede o registro mais antigo disponível. `next` entrega saída e
notificações na ordem recebida. `subscription.closed` provoca nova assinatura em
`last_seq + 1`; falha nessa tentativa aparece em `ResubscribeFailed`, com código e
limites do host. Não há salto silencioso quando o replay foi descartado. Após um
`close`, a tentativa pode receber `UnknownSession`, porque o host removeu a sessão.

`ClientConfig` é alterável a cada conexão. O padrão recomendado vem de
`ClientConfig::recommended(&host_config)`: `max_frame_bytes` usa o limite de quadro
do host; `queue_capacity` usa `subscriber_queue_chunks`. Este último limita a fila
de eventos, a fila de comandos e o número de pedidos pendentes. Para consumidores comuns, uma fila de eventos
cheia encerra a conexão com `Capacity`; o consumidor drena os eventos já guardados
e reconecta com a última sequência consumida. Depois que o cliente para, qualquer que seja a
causa, toda chamada responde `Disconnected`, e só `next` informa a causa; numa chamada,
`Capacity` significa apenas fila de pedidos cheia, com o cliente ainda ativo. A composição da API habilita contrapressão nessa fila e a consome também ao
esperar respostas, preservando o canal sob rajadas de saída. A memória é limitada pelo produto do
limite de quadro pelo tamanho das filas, além dos comandos fornecidos pelo chamador.
Erros guardam apenas categorias, códigos e limites; mensagens do host e payloads
recusados não são registrados. Não há persistência de saída nem telemetria.

A leitura dedicada preserva quadros parciais se uma chamada ou `next` for cancelada.
Cancelar a espera de uma chamada não desfaz um comando já enviado. Prazos de espera
são aplicados pelo chamador com `tokio::time::timeout`. Descartar o cliente encerra
suas tarefas e o canal, preservando as sessões no host; outro cliente pode retomá-las.

Para executar os testes Windows, compile antes o binário utilizado pelo teste:

```text
cargo build -p ads-ptyhost -j 2
cargo test -p ads-daemon -j 2
```

O teste cria a configuração sob a pasta de artefatos do Cargo e usa um pipe exclusivo.
Configura `claude` e `codex` com o programa `cmd.exe` e argumentos fixos
`/d /c echo <ferramenta>-ok [%<chave>%] [%SystemRoot%]`, e o daemon recebe
`ANTHROPIC_API_KEY` e `OPENAI_API_KEY` com um valor de teste. Pela API WebSocket,
confere que a chave chega literal ao filho (`%...%` não expandido, ou seja, a
variável não existe nele) e que `SystemRoot`, permitida, chega com o valor do
pai; depois encerra o host de teste. Não abre agentes nem shells interativos. Só no Windows,
`ended_session_leaves_no_child_behind_on_windows` (`tests/local_api.rs`) roda o host no próprio
processo de teste com ConPTY: um `cmd.exe` abre outro, minimizado num console próprio, que espera
60 s com `timeout.exe`, e sai depois de 3 s. O teste acha esse filho pelo processo pai, não pela lista
do Job Object, confere que ele está no job e que, quando o daemon devolve a vaga sozinho, ele e o que
iniciou não continuam vivos. Fechar o console do terminal não o encerraria; só o job encerra. Os demais testes usam `FakeBackend` e rodam também em Linux.

Para desfazer a integração, remova o consumidor da biblioteca; nenhuma sessão é
migrada nem formato de armazenamento é alterado. A contiguidade de `seq`, os códigos
de erro e a saída do comando real são as evidências de funcionamento nos testes.

## Porta da interface em desenvolvimento

Antes de abrir a URL, confira que a porta de `ui_base_url` pertence ao próprio
usuário. `npm run dev` na pasta `ui` já fixa a porta 5173 com `strictPort`
(`ui/vite.config.ts`).
Se a porta estiver ocupada, pare e investigue; não abra a URL nem aceite outra
porta automaticamente. Configurar uma URL diferente não comprova sua propriedade.
Outro usuário pode servir uma página nessa porta e ler o fragmento com o token.
O risco e o próximo passo estão na [ADR 0020](../../docs/adr/0020-local-api-authentication.md#limite-conhecido)
e em [riscos](../../docs/risks.md#outro-usuário-ocupa-a-porta-da-interface).
