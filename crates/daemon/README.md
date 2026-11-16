# ads-daemon

O executável serve a API local por WebSocket em `127.0.0.1`, com porta efêmera e
autenticação por execução ([ADR 0020](../../docs/adr/0020-local-api-authentication.md)).
**Nenhuma ferramenta real está ligada:** o `DisabledLauncher` recusa qualquer
`harness` com `-32011` (`harness not configured`). Não há execução de arquivos,
transporte remoto, persistência, retomada de sessões após reiniciar ou recarregar
a interface, nem inicialização automática do ptyhost.

## Desenvolvimento

Use três terminais, a partir da raiz do repositório:

```text
cargo run -p ads-ptyhost -j 2
cargo run -p ads-daemon -j 2
```

No terceiro terminal, entre em `ui` e execute `npm run dev`. Abra a URL exata
impressa pelo daemon: `http://localhost:5173/#core=ws://127.0.0.1:<porta>&token=<token>`.
A saída padrão contém somente essa linha. O token de 32 bytes aleatórios vem do
gerador do sistema e existe só em memória; o fragmento não faz parte do pedido
HTTP. A interface envia `auth.hello` como primeira mensagem. Antes dela, mensagem
inválida, token errado ou prazo vencido encerram a conexão. `Host` e `Origin`
inválidos recebem o mesmo HTTP 403 antes do upgrade; cabeçalhos repetidos,
origem ausente e query string também são recusados.

`cargo run -p ads-daemon -j 2 -- --config defaults/daemon.toml` usa explicitamente
o arquivo de configuração. Outro TOML pode sobrepor só as chaves desejadas;
chaves desconhecidas e valores inválidos são recusados sem reproduzir seu conteúdo.
O host usa o canal padrão do usuário, cuja identidade é conferida antes da conexão.

## Comandos, limites e recuperação

`api::Launcher::prepare` recebe os parâmetros gerados de `session.open` (ferramenta,
apelido de pasta, identificador e tamanho) e devolve `OpenParams` do ptyhost.
O daemon confere que identificador e tamanho foram preservados. Os testes fornecem
um launcher para `FakeBackend`; o executável fornece somente `DisabledLauncher`.
Os tipos gerados validam todos os comandos e eventos contra os contratos.
`session.write`, `session.resize` e `session.close` usam o cliente do host.
`events.subscribe` envia eventos com `seq > after_seq`, seguidos dos novos,
incluindo uma assinatura feita antes de abrir a sessão. Nenhum `fs.*` está registrado.
Todo comando remoto é recusado, mesmo com autenticação válida.

Padrões em [defaults/daemon.toml](../../defaults/daemon.toml), aplicados ao iniciar:

| Chave | Padrão | Efeito e alternativa |
|---|---|---|
| `allowed_origins` | `http://localhost:5173`, `http://127.0.0.1:5173` | Lista exata; substitua pelas origens verificadas da interface, sem curingas ou caminhos. |
| `auth_timeout_ms` | 5000 | Prazo do upgrade e, depois, da primeira mensagem; reduza para limitar conexões ociosas. |
| `max_sessions` | 32 | Total de streams por execução, incluindo encerrados; aumente para mais sessões, com maior teto de memória. |
| `max_write_bytes` | 65536 | Bytes decodificados por escrita; pode diminuir, nunca exceder o contrato. |
| `events_per_stream` | 256 | Eventos retidos por stream; aumente para ampliar o replay. |
| `idempotency_keys` | 1024 | Chaves aceitas retidas em FIFO; aumente para ampliar a janela de deduplicação. |
| `command_queue` | 64 | Pedidos esperando o host; aumente para mais concorrência. |
| `max_connections` | 32 | Conexões simultâneas, incluindo não autenticadas; excedentes são fechadas. |
| `max_message_bytes` | 131072 | Limite de mensagem e quadro WebSocket recebido; reduzir pode impedir escritas grandes. |
| `send_timeout_ms` | 5000 | Prazo por envio; cliente lento é desconectado. |
| `host_timeout_ms` | 10000 | Prazo por comando no host e na conexão inicial; falha encerra o serviço para evitar repetir efeito incerto. |

Os inteiros de capacidade aceitam de 1 a 1048576 (`max_message_bytes`: mínimo
1024); prazos aceitam de 1 a 300000 ms. Valores altos elevam o teto de memória.
Uma execução conserva no máximo `max_sessions × events_per_stream` eventos;
o tamanho de cada saída também é limitado pelo quadro do cliente do host.
Assinaturas por conexão são limitadas a `max_sessions`. Filas, conexões e chaves
também têm limites separados. Seq e offset recomeçam ao reiniciar.

Uma chave ainda retida com o mesmo método e parâmetros devolve `replayed: true`,
sem repetir o efeito; outro conteúdo com a mesma chave recebe `-32012`. O id do
pedido pode mudar. A cache guarda só a chave e SHA-256 dos parâmetros, nunca a
entrada do terminal. Depois da expulsão FIFO não há garantia de deduplicação.
`after_seq = 0` pede tudo que resta no stream; cursor descartado ou futuro recebe
`-32004`. Se o cliente ficar para trás durante uma assinatura, a conexão fecha;
nenhum buraco é ocultado. Reconecte com o último seq recebido ou peça 0
explicitamente para aceitar o histórico ainda disponível.

Eventos `created`, `output`, `resized` e `ended` ficam só na memória, em ordem por
stream; saída usa base64 com offset em bytes. Encerramento solicitado só gera
`ended` depois da saída pendente do host. Falha do canal ou prazo do host encerra
o daemon; as sessões continuam sob responsabilidade do ptyhost. Reiniciar não
adota sessões antigas nesta implementação. Para desfazer, pare o daemon e volte
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
`zeroize`, já presente no workspace, limpa o token ao descartá-lo. A única
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
`NotOwnedByUser` e nenhum byte foi enviado (`tests/channel_owner.rs`). O host precisa
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
de eventos, a fila de comandos e o número de pedidos pendentes. Uma fila de eventos
cheia encerra a conexão com `Capacity`; o consumidor drena os eventos já guardados
e reconecta com a última sequência consumida. A memória é limitada pelo produto do
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
Executa apenas `cmd.exe /d /c echo daemon-pty-client-ok`, com ambiente permitido
explícito, e encerra o processo de teste do host ao terminar. Não abre agentes nem
shells interativos. Os demais testes usam `FakeBackend` e rodam também em Linux.

Para desfazer a integração, remova o consumidor da biblioteca; nenhuma sessão é
migrada nem formato de armazenamento é alterado. A contiguidade de `seq`, os códigos
de erro e a saída do comando real são as evidências de funcionamento nos testes.
