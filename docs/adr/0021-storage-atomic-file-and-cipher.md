# 0021. Atomic storage file and authenticated column encryption

Status: escolha de um arquivo só é proposta, aguardando ratificação do mantenedor

## Contexto

O ADR 0004 descreve três bancos com donos separados. Entretanto, a documentação
oficial de [ATTACH do SQLite](https://www.sqlite.org/lang_attach.html) limita a
atomicidade em WAL a cada arquivo: uma queda durante COMMIT pode persistir apenas
parte de uma transação que altera arquivos anexados.

## Decisão

Usar SQLite embutido via `rusqlite`, sem features padrão, com `bundled`. Tudo que
participa de uma invariante ficará em `audit.db`; `state.db` será uma projeção
reconstruível. Métricas independentes podem manter arquivo próprio. Esta proposta
diverge da divisão literal do ADR 0004 para preservar atomicidade das invariantes.
Não existem tabelas de eventos nesta entrega: somente `meta`, esquema versão 1.

`synchronous = FULL` é fixo, sem alternativa de configuração. WAL, foreign keys,
trusted schema desligado, cell size check, timeout, application id (`0x41445331`)
e versão são conferidos por leitura. Arquivos existentes passam por inspeção
do cabeçalho de 100 bytes pela biblioteca padrão antes de chamar SQLite, seguida
por uma imagem privada em memória, sem criar auxiliares WAL nos originais.
A imagem combina o arquivo principal com quadros WAL confirmados, validando
cabeçalho, salts e checksums antes de aplicar os quadros até o último commit.
Caudas incompletas ou de outra geração não entram na imagem. A API segura
`deserialize_read_exact`, da feature `serialize` de `rusqlite`, entrega a imagem
a uma conexão SQLite somente leitura em memória. O modo de journal é ajustado
somente na cópia. Isso permite conferir também esquemas estrangeiros com
`application_id=0` e tabelas presentes apenas no WAL, sem alterar `-shm`.
A segunda inspeção usa uma transação de leitura consistente na conexão de escrita.
As conexões recebem `trusted_schema=OFF`, `cell_size_check=ON` e `DEFENSIVE` antes
de consultar o esquema. Arquivo existente abre sem permissão de criação; arquivo
vazio pode ser inicializado. O conjunto completo de objetos do esquema é conferido
antes de qualquer mudança persistente. Esquema desconhecido, identidade estrangeira
e corrupção recusam abertura sem migrar.

O portão continua no núcleo. A futura ponte com `Journal` pertence ao `ads-daemon`;
storage não dependerá do núcleo nem do executor, inclusive em testes. Abertura
ainda não conecta cifra ao banco nem autoriza execução. A composição futura
exigirá chave do cofre antes de persistir dados sensíveis.

## Dependências e parâmetros

Conferência em 16/11/2026 por `cargo info` e API oficial do crates.io:
`rusqlite =0.42.0` (MIT), anterior à 0.43.0 publicada em 15/11; a anterior também
é recente (11/11), escolha literal da anterior solicitada. `libsqlite3-sys 0.40.0`
é MIT; o SQLite embutido é domínio público e requer revisão dos avisos no pacote.
`chacha20poly1305 0.11.0`, `zeroize 1.9.1` e `getrandom 0.4.3` oferecem MIT ou
Apache-2.0. Nenhuma exceção de licença nova é autorizada.
`pkg-config` e `vcpkg` são dependências de compilação de `libsqlite3-sys`,
puxadas pelos padrões transitivos de `rusqlite` fora de wasm; ficam inertes com
`bundled`, que compila o SQLite incluído. Não são dependências de execução.

`defaults/storage.toml` é a fonte dos padrões, com sobreposição validada ao abrir.
`inspection_max_bytes` limita cada arquivo lido e a imagem final: padrão de
64 MiB, intervalo de 512 bytes a 1 GiB. Exceder o teto retorna `Full` sem escrita;
aumentar a chave permite abrir arquivos maiores. O pico inclui banco, WAL e a
cópia interna do SQLite, além dos descritores dos quadros. Falta de memória nas
reservas controladas também retorna `Full`.
Modo de cifra e journal só aceitam os formatos implementados; alternativas pedem
migração. Limites de eventos e paginação serão introduzidos junto das APIs que os
consumirem. A cifra usa XChaCha20-Poly1305 com nonce do sistema de 24 bytes, AAD
com campos ordenados prefixados por tamanho e domínio versionado. O provedor de
chaves é injetado; chave não implementa Debug, Display ou Clone. O armazenamento
próprio da chave é `Box<Zeroizing<[u8; 32]>>`: movimentos transferem o ponteiro,
sem copiar o array por valor. O construtor recebe `&[u8; 32]` e copia para a alocação; o chamador
continua responsável por apagar a origem. O texto decifrado retorna em
`Zeroizing<Vec<u8>>`. Esses buffers são zerados no descarte normal; isso não
promete apagar cópias externas, registradores, swap, dumps ou memória após abort.
O estado interno da cifra usa a feature `zeroize` da dependência.

## Riscos, demonstração e reversão

Responsável: mantenedor do armazenamento. FULL custa sincronização em cada commit;
timeout tem teto de 60 segundos e falha por categoria sem SQL, caminho ou payload.
A conferência completa de integridade pode ser desligada para reduzir abertura.
Testes demonstram preservação, recusas sem mudança e autenticação da cifra. Tempo
de abertura e falhas por categoria serão medidos quando a composição existir.

Não há garantia contra troca concorrente do caminho por outro processo; a pasta
por usuário e a trava de instância pertencem à composição futura. Backup e
restauração completa ainda dependem do log. Para reverter a proposta de arquivo,
criar migração explícita com backup e demonstração de atomicidade; nunca reduzir
user_version nem dividir invariantes entre arquivos WAL. As tabelas futuras devem
incrementar a versão a partir da versão 1 que contém apenas `meta`.

## Condição de reversão

Reabrir a decisão se uma invariante exigir dois arquivos ou se medições de latência
com `synchronous=FULL` excederem o orçamento de uso definido pela composição.
A mudança exige a migração com backup e demonstração de atomicidade descrita acima.
Este registro usa 0021; o número 0020 permanece sem documento nesta árvore.
