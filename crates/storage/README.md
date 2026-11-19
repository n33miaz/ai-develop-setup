# ads-storage

Guarda o estado local: SQLite, cifra por coluna e auditoria encadeada.

Estado: abre e confere o banco de auditoria (`AuditStore`, `src/open.rs`), recusando arquivo estranho, mais novo ou corrompido e `-journal` ou `-wal` sobrando que o SQLite desfaria ou apagaria; arquivo ainda não marcado é recusado sem alteração nenhuma, e o do produto, sem alterar o conteúdo, e cifra e decifra valores com XChaCha20-Poly1305 (`src/cipher.rs`). A cifra ainda não está ligada ao banco, o banco só tem a tabela `meta`, e a auditoria encadeada não existe. Decisões na [ADR 0005](../../docs/adr/0005-column-level-encryption.md) e na [ADR 0021](../../docs/adr/0021-storage-atomic-file-and-cipher.md) (proposta); parâmetros em [`defaults/storage.toml`](../../defaults/storage.toml).
