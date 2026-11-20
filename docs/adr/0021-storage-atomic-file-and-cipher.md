# 0021. Atomic storage file and authenticated column encryption

Status: escolha de um arquivo só é proposta, aguardando ratificação do mantenedor

Em vigor no código, pendente de ratificação pelo mantenedor.

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
O esquema versão 1 tem só `meta`, a versão 3 acrescenta o log de eventos e a
versão 4 registra a gênese no banco. A versão 2 está aposentada.

`synchronous = FULL` é fixo, sem alternativa de configuração. WAL, foreign keys,
trusted schema desligado, cell size check, timeout, application id (`0x41445331`),
`DEFENSIVE` e versão são conferidos por leitura. Antes de chamar SQLite, a
abertura procura arquivos que ele desfaria ou apagaria sozinho: qualquer
`-journal` ao lado do banco (seria revertido sobre o arquivo principal) e `-wal`
ao lado de um principal vazio ou ausente (SQLite apaga esse WAL e inicia um banco
novo). Enquanto existirem, a abertura espera até `busy_timeout_ms`, porque a
criação concorrente tem uma janela curta com `-journal`. Se ficarem parados
durante a segunda metade da espera, recusa com `Corrupt` sem alterar a pasta
(uma mudança só no começo não basta para `Busy`); se mudaram nessa metade, ou com
`busy_timeout_ms = 0` (que recusa na hora e não distingue criação em andamento de
sobra), recusa com `Busy`. `Corrupt` aqui quer dizer parado por meio prazo, não
necessariamente abandonado; por isso todo reparo começa parando o produto. Somada ao timeout do SQLite, a espera
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
com cabeçalho danificado. Depois de um quadro completo inválido, qualquer
marcador de commit com o salt atual provoca recusa com `Corrupt`. Uma queda
pode deixar o mesmo padrão, conforme o reparo no README do crate. O último
quadro incompleto e a cauda de geração anterior seguem ignorados quando
nenhum marcador de commit com o salt atual segue a quebra. A verificação mira
dano acidental. Quem pode escrever no `-wal` pode ocultar commits truncando
o arquivo, trocando o salt da cauda ou recalculando checksums. O SQLite também
pode aplicar um WAL inteiro de outro banco do produto se os checksums forem
válidos. A abertura do log confere o estado visível com a âncora fora do
arquivo. Em seguida a inspeção roda numa transação de leitura
consistente na conexão de escrita, com custo que não depende de copiar o banco, e
a verificação de integridade roda uma vez. A conexão sempre abre com
`SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE`, desligado só depois que a inspeção passa, sem
depender do tamanho do `-wal` visto antes de abrir: a recusa não transfere os
quadros recusados para o arquivo principal nem apaga o `-wal`, e os bytes do
banco e do `-wal` ficam iguais. A recusa pode deixar um `-shm` criado e, sem
`-wal` antes, um `-wal` vazio; a abertura seguinte do mesmo arquivo os usa
normalmente. Se o `audit.db` for tirado sem o par, esse `-wal` vazio ao lado de
um principal ausente faz a abertura recusar com `Corrupt`; ela não o apaga, e o
reparo (mover o par para a quarentena) está no README do crate. Para o arquivo não marcado, a mesma transação repete
a inspeção depois da imagem. As conexões recebem `trusted_schema=OFF`,
`cell_size_check=ON` e `DEFENSIVE` antes de consultar o esquema. Arquivo
existente abre sem permissão de criação; arquivo vazio pode ser inicializado. O
conjunto completo de objetos do esquema é conferido antes de qualquer mudança
persistente. Esquema desconhecido, identidade estrangeira e corrupção recusam
abertura sem migrar.

O portão continua no núcleo. A futura ponte com `Journal` pertence ao `ads-daemon`;
storage não dependerá do núcleo nem do executor, inclusive em testes.
`AuditStore::open` não recebe chave; `EventLog` liga a cifra ao log de eventos, e
nada no armazenamento autoriza execução. A composição futura exigirá chave do
cofre antes de persistir dados sensíveis.

## Dependências e parâmetros

Conferência em 16/11/2026 por `cargo info` e API oficial do crates.io:
`rusqlite =0.42.0` (MIT), anterior à 0.43.0 publicada em 15/11; a anterior também
é recente (11/11), escolha literal da anterior solicitada. `libsqlite3-sys 0.40.0`
é MIT; o SQLite embutido é domínio público e requer revisão dos avisos no pacote.
`chacha20poly1305 0.11.0`, `zeroize 1.9.1` e `getrandom 0.4.3` oferecem MIT ou
Apache-2.0. A cadeia com chave usa `sha2 0.11.1` (MIT ou Apache-2.0) com
a feature `zeroize` e `subtle 2.6.1` (BSD-3-Clause); a feature `hooks` de `rusqlite`, para o autorizador, não
acrescenta crate. `cargo deny check` passa. Nenhuma exceção de licença nova é
autorizada.
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
uma leitura decifra, e `read_page_max_bytes` (padrão 8 MiB, de `max_event_bytes`
a 256 MiB) limita o texto cifrado de uma página, que é decifrada linha a linha.
O teto conta texto cifrado, não memória: cada evento devolvido fica como JSON
interpretado (`serde_json::Map`), que pode ocupar várias vezes o tamanho do texto.
O [README de storage](../../crates/storage/README.md#log-de-eventos) registra a
medição de uma página de arrays numéricos. O SQLite já leu a linha que passa do
teto antes de a página parar. A cifra usa XChaCha20-Poly1305 com nonce do sistema de 24 bytes, AAD
com campos ordenados prefixados por tamanho e domínio versionado. O provedor de
chaves é injetado; chave não implementa Debug, Display ou Clone. O armazenamento
próprio da chave é `Box<Zeroizing<[u8; 32]>>`: movimentos transferem o ponteiro,
sem copiar o array por valor. O construtor recebe `&[u8; 32]` e copia para a alocação; o chamador
continua responsável por apagar a origem. O texto decifrado retorna em
`Zeroizing<Vec<u8>>`. Esses buffers são zerados no descarte normal; isso não
promete apagar cópias externas, registradores, swap, dumps ou memória após abort.
A cifra e o SHA-256 usado no HMAC habilitam a feature `zeroize`. O HMAC de
chave de 32 bytes compõe duas instâncias de SHA-256 conforme a RFC 2104.
Os blocos de preenchimento e os resultados intermediários usam `Zeroizing`.
A finalização reinicializa as instâncias SHA-256 no lugar. A limpeza é de melhor
esforço, pois movimentos de dados e registradores podem deixar cópias.

## Log de eventos

A versão 3 só acrescenta objetos à versão 1: `cipher_key` (identificador, cifra
e `key_check`, nunca a chave) e `event` (um envelope por linha, `data` cifrado).
A versão 4 acrescenta `log_genesis` e atualiza o gatilho de continuidade. Logs
vazios recebem 32 bytes de gênese do gerador aleatório do sistema. Logs preenchidos na
versão 3 mantêm a gênese de 32 zeros para preservar a autenticação da primeira linha,
sem separação entre bancos pela gênese. A migração retorna `LegacyKeyReuse` antes de
alterar o esquema se um identificador de chave aparecer em mais de um trecho, pois
a versão 3 permitia esse retorno. A rotação só avança para um
identificador de chave ainda não usado. Na transação de `append`, uma chave
já registrada só pode gravar se for a da última linha, quando houver linhas. Bancos das versões 1 e 3 são
migrados na abertura. A migração roda numa transação `IMMEDIATE`
e, dentro dela, compara o esquema do arquivo com o da versão dele para qualquer
versão, inclusive a atual: outro processo pode ter migrado ou alterado o arquivo
entre a inspeção e o lock. A abertura compara o `sqlite_schema` inteiro,
inclusive índices automáticos e `sqlite_sequence`, com o que os scripts da versão
do arquivo produzem num banco em memória.

A versão 2 foi o mesmo log num formato anterior ao primeiro produtor e fica
aposentada: a abertura a recusa com `RetiredSchema` se a versão estiver no
cabeçalho principal ou na página 1 confirmada do WAL, antes de chamar o SQLite. Migrar não é possível sem perder garantias: a abertura não
recebe chave, e as linhas da versão 2 só passariam ao formato novo decifradas,
cifradas de novo e reencadeadas com a chave; descartá-las exigiria apagar
tabelas numa migração que só acrescenta. Nenhum produtor gravou na versão 2,
então só arquivos de teste a usam.

Formato da linha. Cada linha leva `format` (hoje 1). Os dados associados são o
rótulo `urn:ads:storage:event-row`, o `format` e todas as colunas em claro,
inclusive `position`, `action_id`, `approval_id` e `prev_chain`; o `key_id` a
cifra acrescenta sozinha. A leitura recusa formato desconhecido antes de decifrar.
Uma coluna em claro nova entra como `format` 2, com a sua lista de campos, sem
mudar o significado das linhas 1: não há dois formatos distinguidos por limite
de `position` nem por data.

`action_id` e `approval_id` ficam em claro para os índices únicos do portão
(uso único da licença), que entram com a ponte. Saem do `data` dos eventos
`action.*` e `approval.*`; o id da família tem de ser o do stream
(`contracts/README.md`, stream e entidade) e o tipo, da mesma família do stream.
Fora dessas famílias ficam nulos. `CHECK` no esquema repete as três regras:
id da família igual ao do stream, ids nulos fora de `action/` e `approval/`, e
tipo `action.*` ou `approval.*` só no stream da mesma família (e só ele lá).
Assim uma linha inserida sem chave também não ocupa, fora do stream da ação ou
da aprovação, a vaga que os índices da ponte vão vigiar. A leitura confere de
novo os ids contra o `data` decifrado.

Cadeia com chave. `chain` é HMAC-SHA256 do rótulo `urn:ads:storage:chain-row`,
do `format`, do `prev_chain` e de todas as colunas, texto cifrado incluso. A chave
do provedor só alimenta derivação: por HKDF-Expand (RFC 5869, um bloco, a chave
tem de ser uniforme, o que o contrato de `KeyProvider` exige: 32 bytes de gerador
aleatório criptográfico, nunca derivados de senha; por isso dispensa o Extract),
com rótulos distintos, saem três chaves: a da cifra dos dados
(`urn:ads:cipher:subkey:aead:1`), a do `key_check`
(`urn:ads:cipher:subkey:key-check:1`) e a da cadeia
(`urn:ads:cipher:subkey:chain:1`). A chave do provedor só serve à derivação, e
cada chave derivada a uma finalidade. O teste confere a derivação com o vetor
A.1 da RFC 5869. Cada linha usa a chave da cadeia derivada da chave da própria
linha, então `verify_chain` precisa do provedor. Ela confere `position`,
`prev_chain` e `format` antes de pedir chave. Também recusa a volta a um
identificador de chave de um trecho anterior. Uma linha fora de sequência, de formato
desconhecido, de `key_id` que o provedor não entrega ou cujo `key_check` não
confere sai como `ChainBroken` naquela posição, porque não se distingue de linha
forjada: chaves antigas têm de ficar no cofre. A comparação do HMAC é em tempo
constante. `append` busca a chave uma vez (`current()` e `by_id` do mesmo id,
que têm de coincidir, senão `WrongKey`) e usa esse mesmo material no
`key_check`, na cifra e na cadeia; a abertura também recusa provedor incoerente.
Uma linha com identificador de chave indisponível retorna `ChainBroken` na
leitura. A âncora inclui o resumo dos registros de chave e detecta um
`key_id` pré-registrado, inclusive quando o `key_check` foi forjado.

O que a cadeia detecta, sem decifrar `data`: alteração de qualquer coluna,
remoção no meio, reordenação, troca de texto cifrado entre linhas e linha
acrescentada por quem não tem a chave, inclusive por um `INSERT` simples que passa
pelos gatilhos. A cadeia pode aceitar um fim cortado, uma cópia anterior ou outro
banco inteiro com a própria gênese. Quem tem uma chave ainda fornecida pelo cofre
pode cortar o log ao fim do trecho dessa chave e acrescentar linhas autenticadas
com ela, mantendo o trecho contíguo. A verificação depende da integridade
das chaves. A [âncora externa](../../crates/storage/README.md#âncora-no-cofre)
vincula gênese, último `position`, `chain` e registros de chaves ao cofre
em cada `append`. A abertura e o próximo `append` conferem essa âncora.
Sob o lock de escrita, uma mudança em `PRAGMA data_version` também exige
nova autenticação da cadeia antes de gravar
([log de eventos](../../crates/storage/README.md#log-de-eventos)).
Detectar não é impedir: um `INSERT` simples sem chave é aceito pelo banco e só
`verify_chain` e `read_stream` o recusam depois. Com os índices únicos do
portão, uma linha assim pode ocupar a vaga de uma licença (um `action.started`
forjado) e bloquear a verdadeira: negação, não uso duplo. A ponte deve conferir
o HMAC da linha com que colidiu antes de confiar no conflito do índice.

Gatilhos recusam UPDATE, DELETE, substituição de linha (o `INSERT OR REPLACE`,
o `REPLACE INTO` e o UPSERT do SQLite apagariam sem disparar o gatilho de DELETE),
salto ou repetição de `seq` por stream, inclusive com `OR IGNORE`, e `position`
ou `prev_chain` fora da sequência. Depois da migração, um autorizador do SQLite
na conexão do produto só permite leitura, `INSERT`, transação, função e consulta
de pragma; DDL, `ATTACH`, pragma com valor, UPDATE e DELETE recusam com `Denied`.
Ele não protege de outra conexão aberta no mesmo arquivo. Limites das colunas em
claro: `type` até 128 bytes, família do stream até 32 (o id já tem até 64 pelo
contrato), conferidos no `append` e no esquema.

`append` lê o último `seq` do stream dentro de `BEGIN IMMEDIATE` e só grava se o
evento trouxer o seguinte: de duas conexões disputando o mesmo `seq`, uma entra e
a outra recebe `Conflict` (ler `head` e remontar). `Busy` é o lock além de
`busy_timeout_ms`. Antes de tentar gravar novamente, confira o banco contra a
âncora. `Constraint` indica recusa do banco. A abertura ancorada confere a cadeia
inteira sob o lock de escrita ([log de eventos](../../crates/storage/README.md#log-de-eventos)).
O daemon fornece a âncora do cofre. As invariantes do portão e a ligação com o
executor ficam para entregas seguintes.

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
incrementar a versão a partir da versão 3, só acrescentando objetos. Gatilhos
não protegem de quem escreve no arquivo: ele pode removê-los e alterar linhas,
mas sem a chave não recalcula a cadeia. A âncora fora do arquivo detecta
o corte do fim do log e a volta de uma cópia antiga.

## Condição de reversão

Reabrir a decisão se uma invariante exigir dois arquivos ou se medições de latência
com `synchronous=FULL` excederem o orçamento de uso definido pela composição.
A mudança exige a migração com backup e demonstração de atomicidade descrita acima.
O número 0020 é a autenticação da API local ([0020](0020-local-api-authentication.md)); este registro usa 0021.
