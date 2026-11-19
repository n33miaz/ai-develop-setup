# ads-storage

Guarda o estado local: SQLite, cifra por coluna e auditoria encadeada.

Estado: abre e confere o banco de auditoria (`AuditStore`, `src/open.rs`), recusando arquivo estranho, mais novo ou corrompido e `-journal` ou `-wal` sobrando que o SQLite desfaria ou apagaria; arquivo ainda não marcado é recusado sem alteração nenhuma, e o do produto, sem alterar os bytes do banco nem do `-wal` (pode sobrar um `-shm`), cifra e decifra valores com XChaCha20-Poly1305 (`src/cipher.rs`) e guarda eventos num log só-anexo, cifrado e encadeado (`EventLog`, `src/log.rs`). Ainda não confere as invariantes do portão, a chave ainda não vem do cofre e o daemon ainda não usa o log. Decisões na [ADR 0005](../../docs/adr/0005-column-level-encryption.md) e na [ADR 0021](../../docs/adr/0021-storage-atomic-file-and-cipher.md) (proposta); parâmetros em [`defaults/storage.toml`](../../defaults/storage.toml).

## Log de eventos

O esquema versão 3 acrescenta, sem mexer na versão 1, as tabelas `event` e `cipher_key`. Um banco versão 1 é migrado na abertura; a migração confere, dentro do lock de escrita e para qualquer versão, que o esquema do arquivo é exatamente o da versão dele. A versão 2 foi um formato anterior ao primeiro produtor e não é mais lida: a abertura recusa com `RetiredSchema` antes de o SQLite tocar o arquivo. Não há o que migrar, porque nenhum produtor gravou nela; um arquivo assim veio de teste e pode ir para a quarentena.

- `event` guarda um `urn:ads:event:envelope:1` por linha. `position` numera o log inteiro a partir de 1; `format` é o formato da linha (hoje 1); `id`, `type`, `ts`, `stream`, `seq`, autor, `cause` e `data_class` ficam em claro (ADR 0005); o `data` inteiro vai cifrado em `data_ct`, com `key_id` e `nonce`.
- `action_id` e `approval_id` ficam em claro, nulos fora das famílias do portão. Vêm do `data` dos eventos `action.*` (stream `action/<action_id>`) e `approval.*` (stream `approval/<approval_id>`), e o id da própria família tem de ser o do stream ([stream e entidade](../../contracts/README.md#stream-e-entidade)); tipo de uma família em stream de outra recusa com `InvalidEvent`. O banco confere o mesmo vínculo com `CHECK`, e a leitura confere de novo contra o `data` decifrado (`Tampered` se divergir). Os índices únicos do portão sobre essas colunas entram com a ponte.
- Os dados associados da cifra são o rótulo `urn:ads:storage:event-row`, o `format` e todas as colunas em claro da linha (inclusive `position`, `action_id`, `approval_id` e `prev_chain`; o `key_id` a cifra acrescenta sozinha), na montagem com prefixo de tamanho de `cipher.rs`. Trocar qualquer uma delas, o nonce ou o texto cifrado faz a leitura recusar com `Tampered`. Formato desconhecido recusa com `UnsupportedSchema` antes de decifrar; formato novo acrescenta um valor de `format`, sem mudar o significado do 1.
- `chain` é HMAC-SHA256 do rótulo `urn:ads:storage:chain-row`, do `format`, do `prev_chain` e de todas as colunas da linha, texto cifrado incluso; `prev_chain` é o `chain` da linha anterior (32 zeros na primeira). A chave do HMAC é derivada da chave da linha; a chave do provedor nunca é usada direto: dela saem, por HKDF-Expand com rótulos distintos, a chave da cifra e a da cadeia.
- `cipher_key` guarda só o identificador, a cifra e o `key_check` (nonce e etiqueta de uma cifra vazia), nunca a chave. A chave atual é registrada no primeiro `append`, na mesma transação; abrir com uma chave diferente para o mesmo identificador recusa com `WrongKey`, e também um provedor cujo `current()` e `by_id` do mesmo identificador devolvem chaves diferentes (conferido ao abrir e antes de cada `append`). Chave nova vale para linhas novas; as antigas continuam legíveis enquanto o provedor tiver a chave delas.
- Gatilhos recusam UPDATE e DELETE nas duas tabelas, inserção que substituiria linha existente (`INSERT OR REPLACE`, `REPLACE INTO` e UPSERT, porque o REPLACE apaga sem passar pelo gatilho de DELETE), `seq` que não seja o próximo do stream (também com `OR IGNORE`) e `position` ou `prev_chain` que não continuem a cadeia. A abertura compara o `sqlite_schema` inteiro com o da versão do arquivo e recusa gatilho removido ou alterado. Depois da migração, a conexão do produto só prepara leitura, `INSERT`, transação e consulta de pragma: DDL, `ATTACH`, pragma com valor, UPDATE e DELETE recusam com `Denied` mesmo vindo do próprio código.
- Limites das colunas em claro: `type` até 128 bytes e família do stream (antes da `/`) até 32; o id do stream já tem até 64 pelo contrato. Acima disso, `InvalidEvent` sem escrever; o esquema confere os mesmos tetos.

API: `EventLog::open(caminho, configuração, provedor de chaves)`; `head(stream)` devolve o último `seq` (0 sem eventos); `read_stream(stream, de_seq, limite)` decifra linha a linha, sem juntar a página antes, e para no `limite` (até `read_page_limit`) ou antes da linha cujo texto cifrado passaria de `read_page_max_bytes` (a primeira sempre entra; quem lê continua do último `seq` recebido); `verify_chain()` percorre o log inteiro e devolve o número de linhas ou `ChainBroken` com a posição da primeira linha alterada, forjada ou fora de ordem (`KeyUnavailable` se o provedor não tiver mais a chave de alguma linha). Erros são só categorias, sem SQL, caminho nem conteúdo.

`append(evento)` grava só se o `seq` do evento ainda for o próximo do stream, conferido dentro de `BEGIN IMMEDIATE`. Fora `Ok`, nada é gravado:

| Erro | Quando | O que fazer |
|---|---|---|
| `Conflict` | o `seq` já não é o próximo do stream | ler `head` de novo e remontar o evento |
| `Busy` | outro escritor segurou o lock além de `busy_timeout_ms` | repetir o mesmo evento |
| `Constraint` | o banco recusou a linha (`id` repetido ou gatilho) | não repetir igual |
| `InvalidEvent` | limite de `type` ou família, ou id do portão que não é o do stream | corrigir o evento |
| `TooLarge` | `data` serializado acima de `max_event_bytes` | guardar em outro lugar |
| `WrongKey` | provedor diferente da chave registrada ou incoerente consigo | conferir o cofre |

Quem chama `verify_chain`: a composição (daemon), ao iniciar e depois em intervalos, quando estiver ligada ao log; a abertura não chama, porque o custo cresce com o log inteiro. `read_stream` não depende dela: confere cada linha que lê.

Detecta: alteração, remoção no meio, reordenação, troca de texto cifrado entre linhas e linha acrescentada por quem não tem a chave, mesmo com os gatilhos intactos e um `INSERT` simples. Não detecta: cortar o fim do log nem voltar uma cópia anterior do arquivo inteiro, porque a cadeia não tem âncora fora do arquivo; nem o que faz quem tem a chave ([riscos](../../docs/risks.md#licença-de-execução-usada-duas-vezes)).

## Reparo de `-journal` sobrando

A abertura recusa qualquer `audit.db-journal` ao lado do banco, inclusive vazio ou com cabeçalho zerado, porque o SQLite o desfaria sobre o arquivo principal. Se a recusa persistir (`Corrupt` depois da espera, ou `Busy` com `busy_timeout_ms = 0`):

1. Pare o produto e confira que nenhum processo usa o banco.
2. Copie `audit.db`, `audit.db-journal` e, se existirem, `audit.db-wal` e `audit.db-shm` para uma pasta de quarentena. Não apague nada.
3. Se o journal começa com os bytes `d9 d5 05 f9 20 a1 63 d7`, ele guarda as páginas anteriores a uma transação interrompida. Mantenha a cópia junto do `audit.db` da mesma hora.
4. Mova `audit.db-journal` para a quarentena e abra de novo. A abertura confere cabeçalho, esquema e integridade; se recusar, restaure o backup.
