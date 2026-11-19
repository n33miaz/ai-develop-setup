# ads-storage

Guarda o estado local: SQLite, cifra por coluna e auditoria encadeada.

Estado: abre e confere o banco de auditoria (`AuditStore`, `src/open.rs`), recusando arquivo estranho, mais novo ou corrompido e `-journal` ou `-wal` sobrando que o SQLite desfaria ou apagaria; arquivo ainda não marcado é recusado sem alteração nenhuma, e o do produto, sem alterar os bytes do banco nem do `-wal` (pode sobrar um `-shm`), e cifra e decifra valores com XChaCha20-Poly1305 (`src/cipher.rs`). A cifra ainda não está ligada ao banco, o banco só tem a tabela `meta`, e a auditoria encadeada não existe. Decisões na [ADR 0005](../../docs/adr/0005-column-level-encryption.md) e na [ADR 0021](../../docs/adr/0021-storage-atomic-file-and-cipher.md) (proposta); parâmetros em [`defaults/storage.toml`](../../defaults/storage.toml).

## Reparo de `-journal` sobrando

A abertura recusa qualquer `audit.db-journal` ao lado do banco, inclusive vazio ou com cabeçalho zerado, porque o SQLite o desfaria sobre o arquivo principal. Se a recusa persistir (`Corrupt` depois da espera, ou `Busy` com `busy_timeout_ms = 0`):

1. Pare o produto e confira que nenhum processo usa o banco.
2. Copie `audit.db`, `audit.db-journal` e, se existirem, `audit.db-wal` e `audit.db-shm` para uma pasta de quarentena. Não apague nada.
3. Se o journal começa com os bytes `d9 d5 05 f9 20 a1 63 d7`, ele guarda as páginas anteriores a uma transação interrompida. Mantenha a cópia junto do `audit.db` da mesma hora.
4. Mova `audit.db-journal` para a quarentena e abra de novo. A abertura confere cabeçalho, esquema e integridade; se recusar, restaure o backup.
