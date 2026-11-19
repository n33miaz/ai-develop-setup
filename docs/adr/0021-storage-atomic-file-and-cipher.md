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
O esquema versão 1 tem só `meta`; a versão 2 acrescenta o log de eventos (abaixo).

`synchronous = FULL` é fixo, sem alternativa de configuração. WAL, foreign keys,
trusted schema desligado, cell size check, timeout, application id (`0x41445331`),
`DEFENSIVE` e versão são conferidos por leitura. Antes de chamar SQLite, a
abertura procura arquivos que ele desfaria ou apagaria sozinho: qualquer
`-journal` ao lado do banco (seria revertido sobre o arquivo principal) e `-wal`
ao lado de um principal vazio ou ausente (SQLite apaga esse WAL e inicia um banco
novo). Enquanto existirem, a abertura espera até `busy_timeout_ms`, porque a
criação concorrente tem uma janela curta com `-journal`. Se continuarem parados
até o prazo, recusa com `Corrupt` sem alterar a pasta; se ainda estiverem mudando,
ou com `busy_timeout_ms = 0` (que recusa na hora e não distingue criação em
andamento de sobra), recusa com `Busy`. Somada ao timeout do SQLite, a espera
total pode chegar a cerca de duas vezes `busy_timeout_ms`. O reparo de um
`-journal` sobrando está no README do crate.
Arquivos existentes passam por inspeção do cabeçalho de 100 bytes pela biblioteca
padrão. Arquivo ainda não marcado (`application_id=0`) segue para uma imagem
privada em memória, sem criar auxiliares WAL nos originais. A imagem combina o
arquivo principal com quadros WAL confirmados, validando cabeçalho, salts e
checksums antes de aplicar os quadros até o último commit. Caudas incompletas ou
de outra geração não entram na imagem. A API segura `deserialize_read_exact`, da
feature `serialize` de `rusqlite`, entrega a imagem a uma conexão SQLite somente
leitura em memória. O modo de journal é ajustado somente na cópia. Isso permite
conferir esquemas estrangeiros com `application_id=0` e tabelas presentes apenas
no WAL, sem alterar `-shm`. Arquivo já marcado pelo produto não passa pela imagem.
Dele se lê o `-wal` em fluxo, um quadro por vez: o cabeçalho (magic, versão,
página e checksum) é conferido, porque o SQLite descartaria em silêncio um WAL
com cabeçalho danificado; e a abertura recusa com `Corrupt` quando, depois do
primeiro quadro inválido, há um commit com o salt atual além do commit da
transação que contém a quebra. O SQLite para no primeiro quadro inválido e
perderia esses commits já confirmados. Quebra só na última transação é queda
comum e abre normalmente. Em seguida a inspeção roda numa transação de leitura
consistente na conexão de escrita, com custo que não depende de copiar o banco, e
a verificação de integridade roda uma vez. Com `-wal` pendente, a conexão abre
com `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE`, desligado só depois que a inspeção passa:
a recusa não transfere os quadros recusados para o arquivo principal, e os bytes
do banco e do `-wal` ficam iguais. A recusa pode deixar um `-shm` criado e, sem
`-wal` pendente, criar e remover `-wal` e `-shm`. Para o arquivo não marcado, a mesma transação repete
a inspeção depois da imagem. As conexões recebem `trusted_schema=OFF`,
`cell_size_check=ON` e `DEFENSIVE` antes de consultar o esquema. Arquivo
existente abre sem permissão de criação; arquivo vazio pode ser inicializado. O
conjunto completo de objetos do esquema é conferido antes de qualquer mudança
persistente. Esquema desconhecido, identidade estrangeira e corrupção recusam
abertura sem migrar.

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
`inspection_max_bytes` limita cada arquivo lido e a imagem final de arquivos ainda
não marcados: padrão de 64 MiB, intervalo de 512 bytes a 1 GiB. Exceder o teto
retorna `Full` sem escrita; aumentar a chave permite abrir arquivos maiores. O
pico inclui banco, WAL e a cópia interna do SQLite, além dos descritores dos
quadros. Falta de memória nas reservas controladas também retorna `Full`. Arquivo
do produto não tem esse teto, porque o log de auditoria só cresce.
`busy_timeout_ms` também limita a espera por `-journal` ou `-wal` sobrando.
Modo de cifra e journal só aceitam os formatos implementados; alternativas pedem
migração. `max_event_bytes` (padrão 1 MiB, 64 bytes a 16 MiB) limita o `data` serializado
de um evento; `read_page_limit` (padrão 500, 1 a 10000) limita quantos eventos
uma leitura decifra. A cifra usa XChaCha20-Poly1305 com nonce do sistema de 24 bytes, AAD
com campos ordenados prefixados por tamanho e domínio versionado. O provedor de
chaves é injetado; chave não implementa Debug, Display ou Clone. O armazenamento
próprio da chave é `Box<Zeroizing<[u8; 32]>>`: movimentos transferem o ponteiro,
sem copiar o array por valor. O construtor recebe `&[u8; 32]` e copia para a alocação; o chamador
continua responsável por apagar a origem. O texto decifrado retorna em
`Zeroizing<Vec<u8>>`. Esses buffers são zerados no descarte normal; isso não
promete apagar cópias externas, registradores, swap, dumps ou memória após abort.
O estado interno da cifra usa a feature `zeroize` da dependência.

## Log de eventos (esquema versão 2)

A versão 2 só acrescenta objetos: `cipher_key` (identificador, cifra e
`key_check`, nunca a chave) e `event` (um envelope por linha, `data` cifrado).
Banco versão 1 é migrado na abertura depois de conferir que o esquema dele é
exatamente o da versão 1; a migração roda numa transação `IMMEDIATE` e repete a
conferência dentro dela. A abertura compara o `sqlite_schema` inteiro, inclusive
índices automáticos e `sqlite_sequence`, com o que os scripts da versão do
arquivo produzem num banco em memória.

Os dados associados de cada linha são um rótulo de versão e todas as colunas em
claro, inclusive `position` e `prev_chain`; a cadeia (`chain`, SHA-256) cobre
`prev_chain` e todas as colunas, texto cifrado incluso, e por isso é conferida
sem chave. Gatilhos recusam UPDATE, DELETE, substituição de linha (o
`INSERT OR REPLACE` do SQLite apaga sem disparar o gatilho de DELETE), salto ou
repetição de `seq` por stream e `position` ou `prev_chain` fora da sequência.
`append` lê o último `seq` do stream dentro de `BEGIN IMMEDIATE` e só grava se o
evento trouxer o seguinte: de duas conexões disputando o mesmo `seq`, uma entra e
a outra recebe `Conflict`. Invariantes do portão e ligação com o daemon ficam para
entregas seguintes.

## Riscos, demonstração e reversão

Responsável: mantenedor do armazenamento. FULL custa sincronização em cada commit;
timeout tem teto de 60 segundos e falha por categoria sem SQL, caminho ou payload.
A conferência completa de integridade pode ser desligada para reduzir abertura.
Testes demonstram preservação, recusas sem mudança e autenticação da cifra. A
medida de abertura fica no teste ignorado `open_time_of_large_product_file`
(`cargo test -p ads-storage --release -- --ignored --nocapture`, tamanho em
`ADS_STORAGE_PROBE_MIB`). Em 19/11/2026, compilação release no Windows, um arquivo
do produto de 60 MiB abria em 412 ms com integridade e 89 ms sem ela quando
passava pela imagem; sem a imagem, 198 ms e 4 ms. Um de 200 MiB, antes recusado
com `Full` no padrão, abre em cerca de 0,6 s com integridade e 3 ms sem ela.
Falhas por categoria serão medidas quando a composição existir.

Não há garantia contra troca concorrente do caminho por outro processo; a pasta
por usuário e a trava de instância pertencem à composição futura. Backup e
restauração completa ainda dependem do log. Para reverter a proposta de arquivo,
criar migração explícita com backup e demonstração de atomicidade; nunca reduzir
user_version nem dividir invariantes entre arquivos WAL. As tabelas futuras devem
incrementar a versão a partir da versão 2, só acrescentando objetos. Gatilhos
não protegem de quem escreve no arquivo: ele pode removê-los, alterar linhas e
recalcular a cadeia, e o corte do fim do log não deixa rastro sem uma âncora fora
do arquivo.

## Condição de reversão

Reabrir a decisão se uma invariante exigir dois arquivos ou se medições de latência
com `synchronous=FULL` excederem o orçamento de uso definido pela composição.
A mudança exige a migração com backup e demonstração de atomicidade descrita acima.
O número 0020 é a autenticação da API local ([0020](0020-local-api-authentication.md)); este registro usa 0021.
