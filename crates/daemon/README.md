# ads-daemon

O executável ainda só imprime o nome. A biblioteca expõe `pty_client::PtyClient`,
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
