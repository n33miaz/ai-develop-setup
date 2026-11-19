# ads-storage

Guarda o estado local: SQLite, cifra por coluna e auditoria encadeada.

Estado: abre e confere o banco de auditoria (`AuditStore`, `src/open.rs`), recusando arquivo estranho, mais novo ou corrompido e `-journal` ou `-wal` sobrando que o SQLite desfaria ou apagaria; arquivo ainda não marcado é recusado sem alteração nenhuma, e o do produto, sem alterar os bytes do banco nem do `-wal` (pode sobrar um `-shm`), cifra e decifra valores com XChaCha20-Poly1305 (`src/cipher.rs`) e guarda eventos num log só-anexo, cifrado e encadeado (`EventLog`, `src/log.rs`). Ainda não confere as invariantes do portão, a chave ainda não vem do cofre e o daemon ainda não usa o log. Decisões na [ADR 0005](../../docs/adr/0005-column-level-encryption.md) e na [ADR 0021](../../docs/adr/0021-storage-atomic-file-and-cipher.md) (proposta); parâmetros em [`defaults/storage.toml`](../../defaults/storage.toml).

## Log de eventos

O esquema versão 2 acrescenta, sem mexer na versão 1, as tabelas `event` e `cipher_key`. Um banco versão 1 é migrado na abertura, depois de o esquema dele conferir com o da versão 1.

- `event` guarda um `urn:ads:event:envelope:1` por linha. `position` numera o log inteiro a partir de 1; `id`, `type`, `ts`, `stream`, `seq`, autor, `cause` e `data_class` ficam em claro (ADR 0005); o `data` inteiro vai cifrado em `data_ct`, com `key_id` e `nonce`.
- Os dados associados da cifra são o rótulo `urn:ads:storage:event:2` seguido de todas as colunas em claro da linha (inclusive `position` e `prev_chain`; o `key_id` a cifra acrescenta sozinha), na montagem com prefixo de tamanho de `cipher.rs`. Trocar qualquer uma delas, o nonce ou o texto cifrado faz a leitura recusar com `Tampered`.
- `chain` é SHA-256 do rótulo `urn:ads:storage:chain:1`, do `prev_chain` e de todas as colunas da linha, texto cifrado incluso; `prev_chain` é o `chain` da linha anterior (32 zeros na primeira). `verify_chain` confere sem chave e devolve `ChainBroken` com a posição da primeira linha alterada ou fora de ordem.
- `cipher_key` guarda só o identificador, a cifra e o `key_check` (nonce e etiqueta de uma cifra vazia), nunca a chave. A chave atual é registrada no primeiro `append`, na mesma transação; abrir com uma chave diferente para o mesmo identificador recusa com `WrongKey`. Chave nova vale para linhas novas; as antigas continuam legíveis enquanto o provedor tiver a chave delas.
- Gatilhos recusam UPDATE e DELETE nas duas tabelas, inserção que substituiria linha existente (`INSERT OR REPLACE` apaga sem passar pelo gatilho de DELETE), `seq` que não seja o próximo do stream e `position` ou `prev_chain` que não continuem a cadeia. A abertura compara o `sqlite_schema` inteiro com o da versão do arquivo e recusa gatilho removido ou alterado.

API: `EventLog::open(caminho, configuração, provedor de chaves)`; `append(evento)` grava só se o `seq` do evento ainda for o próximo do stream, conferido dentro de `BEGIN IMMEDIATE` (senão `Conflict`); `head(stream)` devolve o último `seq` (0 sem eventos); `read_stream(stream, de_seq, limite)` decifra até `read_page_limit` eventos; `verify_chain()`. Erros são só categorias, sem SQL, caminho nem conteúdo. `max_event_bytes` limita o `data` serializado (`TooLarge` sem escrever).

Não detecta: quem escreve no arquivo pode tirar os gatilhos, alterar linhas, recalcular a cadeia a partir delas e recriar os gatilhos; cortar o fim do log também não deixa rastro, porque a cadeia não tem âncora fora do arquivo ([riscos](../../docs/risks.md#perda-do-estado-local)).

## Reparo de `-journal` sobrando

A abertura recusa qualquer `audit.db-journal` ao lado do banco, inclusive vazio ou com cabeçalho zerado, porque o SQLite o desfaria sobre o arquivo principal. Se a recusa persistir (`Corrupt` depois da espera, ou `Busy` com `busy_timeout_ms = 0`):

1. Pare o produto e confira que nenhum processo usa o banco.
2. Copie `audit.db`, `audit.db-journal` e, se existirem, `audit.db-wal` e `audit.db-shm` para uma pasta de quarentena. Não apague nada.
3. Se o journal começa com os bytes `d9 d5 05 f9 20 a1 63 d7`, ele guarda as páginas anteriores a uma transação interrompida. Mantenha a cópia junto do `audit.db` da mesma hora.
4. Mova `audit.db-journal` para a quarentena e abra de novo. A abertura confere cabeçalho, esquema e integridade; se recusar, restaure o backup.
