# ads-storage

Guarda o estado local: SQLite, cifra por coluna e auditoria encadeada.

Estado: abre e confere o banco de auditoria (`AuditStore`, `src/open.rs`), recusando arquivo estranho, mais novo ou corrompido e `-journal` ou `-wal` sobrando que o SQLite desfaria ou apagaria; arquivo ainda não marcado é recusado sem alteração nenhuma, e o do produto, sem alterar os bytes do banco nem do `-wal` (pode sobrar um `-shm` e, sem `-wal` antes, um `-wal` vazio), cifra e decifra valores com XChaCha20-Poly1305 (`src/cipher.rs`) e guarda eventos num log só-anexo, cifrado e encadeado (`EventLog`, `src/log.rs`). Ainda não confere as invariantes do portão, a chave ainda não vem do cofre e o daemon ainda não usa o log. Decisões na [ADR 0005](../../docs/adr/0005-column-level-encryption.md) e na [ADR 0021](../../docs/adr/0021-storage-atomic-file-and-cipher.md) (proposta); parâmetros em [`defaults/storage.toml`](../../defaults/storage.toml).

## Log de eventos

A versão 3 acrescentou `event` e `cipher_key`. A versão 4 acrescenta `log_genesis` e atualiza o gatilho de inserção. Bancos das versões 1 e 3 são migrados na abertura, dentro do lock de escrita que confere o esquema de origem. Um log vazio recebe uma gênese aleatória de 32 bytes. Um log já preenchido na versão 3 conserva a gênese de 32 zeros para manter as linhas autenticadas. A versão 2 veio de testes e pode ir para a quarentena. A abertura retorna `RetiredSchema` antes de chamar o SQLite quando encontra essa versão no cabeçalho principal ou no último estado confirmado do WAL durante a inspeção.

- `event` guarda um `urn:ads:event:envelope:1` por linha. `position` numera o log inteiro a partir de 1; `format` é o formato da linha (hoje 1); `id`, `type`, `ts`, `stream`, `seq`, autor, `cause` e `data_class` ficam em claro (ADR 0005); o `data` inteiro vai cifrado em `data_ct`, com `key_id` e `nonce`.
- `action_id` e `approval_id` ficam em claro, nulos fora das famílias do portão. Vêm do `data` dos eventos `action.*` (stream `action/<action_id>`) e `approval.*` (stream `approval/<approval_id>`), e o id da própria família tem de ser o do stream ([stream e entidade](../../contracts/README.md#stream-e-entidade)); tipo de uma família em stream de outra recusa com `InvalidEvent`. O banco confere com `CHECK` o mesmo vínculo, ids nulos fora dessas famílias e tipo `action.*` ou `approval.*` só no stream da própria família; a leitura confere de novo contra o `data` decifrado (`Tampered` se divergir). Os índices únicos do portão sobre essas colunas entram com a ponte.
- Os dados associados da cifra são o rótulo `urn:ads:storage:event-row`, o `format` e todas as colunas em claro da linha (inclusive `position`, `action_id`, `approval_id` e `prev_chain`; o `key_id` a cifra acrescenta sozinha), na montagem com prefixo de tamanho de `cipher.rs`. A leitura retorna `ChainBroken` para identificador de chave inválido ou ausente de `cipher_key` e quando consegue confirmar que a linha autentica com outro identificador registrado. Outras falhas de autenticação da linha retornam `Tampered`. Formato desconhecido recusa com `UnsupportedSchema` antes de decifrar.
- `chain` é HMAC-SHA256 do rótulo `urn:ads:storage:chain-row`, do `format`, do `prev_chain` e de todas as colunas da linha, texto cifrado incluso. A primeira linha usa a gênese de `log_genesis` em `prev_chain`. As seguintes usam o `chain` da linha anterior. Da chave do provedor saem, por HKDF-Expand com rótulos distintos, a chave da cifra dos dados, a do `key_check` e a da cadeia. A chave do provedor tem de ser uniforme (32 bytes de gerador aleatório criptográfico).
- `cipher_key` guarda só o identificador, a cifra e o `key_check` (nonce e etiqueta de uma cifra vazia), nunca a chave. A chave atual é registrada no primeiro `append`, na mesma transação; abrir com uma chave diferente para o mesmo identificador recusa com `WrongKey` (que também pode indicar uma linha de `cipher_key` forjada, assim como `KeyUnavailable`), e também um provedor cujo `current()` e `by_id` do mesmo identificador devolvem chaves diferentes (conferido ao abrir e antes de cada `append`). Chave nova vale para linhas novas; as antigas continuam legíveis enquanto o provedor tiver a chave delas.
- Gatilhos recusam UPDATE e DELETE nas duas tabelas, inserção que substituiria linha existente (`INSERT OR REPLACE`, `REPLACE INTO` e UPSERT, porque o REPLACE apaga sem passar pelo gatilho de DELETE), `seq` que não seja o próximo do stream (também com `OR IGNORE`) e `position` ou `prev_chain` que não continuem a cadeia. A abertura compara o `sqlite_schema` inteiro com o da versão do arquivo e recusa gatilho removido ou alterado. Depois da migração, a conexão do produto só prepara leitura, `INSERT`, transação e consulta de pragma: DDL, `ATTACH`, pragma com valor, UPDATE e DELETE recusam com `Denied` mesmo vindo do próprio código.
- Limites das colunas em claro: `type` até 128 bytes e família do stream (antes da `/`) até 32; o id do stream já tem até 64 pelo contrato. Acima disso, `InvalidEvent` sem escrever; o esquema confere os mesmos tetos.

API: `EventLog::open(caminho, configuração, provedor de chaves)`; `head(stream)` devolve o último `seq` (0 sem eventos); `read_stream(stream, de_seq, limite)` decifra linha a linha, sem juntar a página antes, e para no `limite` (até `read_page_limit`) ou antes da linha cujo texto cifrado passaria de `read_page_max_bytes` (a primeira sempre entra; quem lê continua do último `seq` recebido); `verify_chain()` percorre o log inteiro e devolve o número de linhas ou `ChainBroken` com a posição da primeira linha alterada, forjada, fora de ordem, de formato desconhecido ou de chave que o provedor não entrega ou que não confere com o `key_check` (chave antiga perdida aparece assim: mantenha as chaves antigas no cofre). Erros são só categorias, sem SQL, caminho nem conteúdo.

`append(evento)` grava só se o `seq` do evento ainda for o próximo do stream, conferido dentro de `BEGIN IMMEDIATE`. Fora `Ok`, nada é gravado:

| Erro | Quando | O que fazer |
|---|---|---|
| `Conflict` | o `seq` já não é o próximo do stream | ler `head` de novo e remontar o evento |
| `Busy` | outro escritor segurou o lock além de `busy_timeout_ms` | repetir o mesmo evento |
| `Constraint` | o banco recusou a linha (`id` repetido ou gatilho) | não repetir igual |
| `InvalidEvent` | limite de `type` ou família, ou id do portão que não é o do stream | corrigir o evento |
| `TooLarge` | `data` serializado acima de `max_event_bytes` | guardar em outro lugar |
| `WrongKey` | provedor diferente da chave registrada ou incoerente consigo | conferir o cofre |

Quem chama `verify_chain`: a composição (daemon), ao iniciar e depois em intervalos, quando estiver ligada ao log; a abertura não chama, porque o custo cresce com o log inteiro. `read_stream` não depende dela para cada linha que lê: decifra e autentica a linha e recusa com `ChainBroken` um `seq` faltando dentro da página, inclusive no `de_seq` pedido. Não percebe linhas faltando fora da página nem o corte do fim.

`verify_chain` aceita cada `key_id` em um único trecho contíguo da cadeia global e aponta a primeira posição que volta a usar um identificador anterior. A ordem vem das linhas, pois identificadores de chave são opacos.

Em Windows x64, o teste `reading_numeric_arrays_measures_retained_event_memory` mediu uma página com dois eventos de 8.192 zeros cada. Cada evento ocupou 16.413 bytes cifrados e 262.144 bytes nos slots do array JSON já interpretado. As capacidades dos arrays e do vetor de eventos somaram 525.088 bytes na página. Essa contagem é um limite inferior da memória retida. Metadados, demais alocações, buffers transitórios e o cache do SQLite ficam fora dela.

### Âncora no cofre

A gênese fica no próprio banco. A âncora externa depende do cofre e deve vincular a identidade do banco, a última posição, o MAC correspondente e os identificadores de chave autorizados. A integração precisa conferir esse registro na abertura e definir a recuperação quando a gravação no banco e a atualização do cofre forem interrompidas entre si. Até lá, truncamento do fim, restauração de uma cópia anterior e substituição coerente do banco ou do WAL podem passar. Consulte os [riscos da auditoria](../../docs/risks.md#licença-de-execução-usada-duas-vezes).

## Reparo de `-journal` sobrando

A abertura recusa qualquer `audit.db-journal` ao lado do banco, inclusive vazio ou com cabeçalho zerado, porque o SQLite o desfaria sobre o arquivo principal. Se a recusa persistir (`Corrupt` depois da espera, ou `Busy` com `busy_timeout_ms = 0`):

1. Pare o produto e confira que nenhum processo usa o banco.
2. Copie `audit.db`, `audit.db-journal` e, se existirem, `audit.db-wal` e `audit.db-shm` para a subpasta `copia` de uma pasta de quarentena, com os mesmos nomes. Não apague nada.
3. Se o journal começa com os bytes `d9 d5 05 f9 20 a1 63 d7`, ele guarda as páginas anteriores a uma transação interrompida. Mantenha a cópia junto do `audit.db` da mesma hora.
4. Mova `audit.db-journal` para a subpasta `movidos` da quarentena e abra de novo. A abertura confere cabeçalho, esquema e integridade.
5. Se recusar, mova antes `audit.db-wal` e `audit.db-shm` para `movidos` e só então restaure o `audit.db` do backup, sem `-wal` nem `-shm` ao lado (ou com os do próprio backup). Nada liga o WAL ao arquivo principal: um `-wal` deixado ao lado seria aplicado sobre o backup.

## Reparo de `-wal` recusado

A abertura recusa com `Corrupt` se encontrar um marcador de commit com o salt atual depois de um quadro completo inválido. Isso inclui salt divergente com checksum válido, quadro zerado e quadro de geração anterior. A abertura ainda ignora o último quadro incompleto e a cauda de geração anterior quando nenhum marcador de commit com o salt atual segue a quebra. Uma queda legítima também pode deixar uma quebra antes do commit da última transação e exigir reparo.

A verificação protege contra dano acidental. Quem pode escrever no `-wal` pode ocultar commits truncando o arquivo, trocando o salt da cauda ou recalculando os checksums. A defesa planejada para detectar o corte do fim do log é uma âncora fora do arquivo.

1. Pare o produto e confira que nenhum processo usa o banco.
2. Copie `audit.db`, `audit.db-wal` e, se existir, `audit.db-shm` para a subpasta `copia` de uma pasta de quarentena, com os mesmos nomes. Não apague nada.
3. Se a quebra for compatível com uma queda e houver evidência para descartar adulteração, tente preservar os commits anteriores à quebra. Numa pasta fora da pasta do produto e de `copia`, faça uma segunda cópia de `audit.db` e `audit.db-wal` a partir de `copia`, com os mesmos nomes. Abra essa segunda cópia com uma ferramenta SQLite e execute `PRAGMA integrity_check`. Prossiga apenas se o resultado for `ok`. Execute `PRAGMA wal_checkpoint(TRUNCATE)` nessa mesma cópia, confira que o resultado é `0|0|0` e feche a ferramenta. O SQLite recupera até o último commit válido antes da quebra. A transação que contém o primeiro quadro inválido e todas as posteriores ficam fora do resultado, inclusive eventuais commits confirmados.
4. Antes de instalar o banco recuperado, confira os caminhos e confirme a substituição. Mova o `audit.db`, o `audit.db-wal` e o `audit.db-shm` atual, se existir, para `movidos` na quarentena. Copie apenas o `audit.db` recuperado para o local original e reabra o produto: a abertura confere cabeçalho, esquema e integridade do banco recuperado. Preserve `copia` e `movidos`.
5. Se optar por voltar ao último checkpoint, confira os caminhos e confirme a perda de todas as transações que estavam só no WAL. Mova `audit.db-wal` e, se existir, `audit.db-shm` para `movidos` e reabra o produto. O corte do fim do log pode passar pela verificação `verify_chain`. Em caso de suspeita de adulteração, restaure um backup confiável conforme o reparo de `-journal`, mantendo o banco substituído e seus arquivos auxiliares em quarentena.

## Recusa com `-wal` ao lado

Recusa por esquema, identidade ou integridade deixa `audit.db` e `audit.db-wal` intactos; se não havia `-wal`, pode sobrar um `-wal` vazio e um `-shm`, que a abertura seguinte do mesmo arquivo usa normalmente. Antes de qualquer reparo, copie os três arquivos para a quarentena. Ao tirar o `audit.db` do lugar (para começar do zero ou restaurar), leve junto `-wal` e `-shm`: um `-wal`, mesmo vazio, ao lado de um principal ausente faz a abertura esperar `busy_timeout_ms` e recusar com `Corrupt`, sem criar banco nem apagar nada. O reparo é mover o par para a quarentena; a abertura nunca apaga esse `-wal` sozinha.
