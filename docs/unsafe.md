# Orçamento de `unsafe`

Regra: `unsafe_code = "forbid"` vale para todo o workspace (`[workspace.lints.rust]` no `Cargo.toml` da raiz) e cada crate herda com `[lints] workspace = true`. As exceções em uso são sete, cada uma com uma tabela `[lints]` própria com `unsafe_code = "deny"` e o lint liberado em um só módulo, compilado apenas no Windows: `ads-ptyhost` (`crates/ptyhost/src/win.rs`), `ads-adapter-fs` (`crates/adapters/fs/src/sys/windows.rs`), `ads-testkit` (`crates/testkit/src/win.rs`), `ads-adapter-vault` (`crates/adapters/vault/src/windows.rs`), `ads-private-dir` (`crates/private-dir/src/platform/win.rs`), `ads-launcher` (`crates/launcher/src/console/win.rs`) e `ads-shell` (`crates/shell/src/stdin.rs`). O `ads-testkit` é só de teste: entra apenas como dependência de desenvolvimento e não chega a nenhum binário do produto, nem com `--all-features`. Fora desses módulos, `unsafe` continua reprovando a compilação. O `python tools/check_deps.py check` (job `rust` do CI) reprova a tabela copiada que divergir de `[workspace.lints]` além do `unsafe_code`, para um lint novo do workspace não ficar de fora dessas exceções.

A contagem só sobe com justificativa aqui.

| Crate | Blocos `unsafe` | Motivo |
|---|---|---|
| `ads-ptyhost` | 30 | chamadas da API do Windows, listadas abaixo |
| `ads-adapter-fs` | 5 | chamadas da API do Windows para abrir, mover e apagar por identificador aberto e para classificar o volume, listadas abaixo; no Linux as mesmas operações vêm do `rustix`, sem `unsafe` |
| `ads-testkit` | 7 | só de teste, fora de qualquer binário do produto; listados abaixo |
| `ads-adapter-vault` | 6 | operações por nome no cofre do Windows e tratamento da memória devolvida |
| `ads-private-dir` | 21 | chamadas da API do Windows para criar pastas privadas e arquivos de controle só do usuário, conferir dono e lista de acesso e operar relativo à pasta aberta, listadas abaixo (3 delas habilitadas em testes ou com `test-support`); no Unix o mesmo vem da biblioteca padrão e do `rustix`, sem `unsafe` |
| `ads-launcher` | 4 | tratador de interrupções do console e leitura do horário de início do próprio processo, com 2 blocos de teste |
| `ads-shell` | 2 | retirar e fechar a entrada padrão herdada antes de iniciar a WebView |
| `ads-storage` | 0 | SQLite e cifra pelas APIs seguras das dependências; herda `forbid` |
| demais | 0 | nenhum |

## `ads-ptyhost` (`crates/ptyhost/src/win.rs`)

Cada bloco é uma única operação (uma chamada da API do Windows pelo crate `windows`, a criação do pipe pelo `tokio`, ou a leitura de um dado que a chamada anterior preencheu) e leva um comentário `SAFETY` no código. Os handles criados passam na hora para um `OwnedHandle` da biblioteca padrão, que os fecha sozinho; a memória que o sistema aloca para o chamador fica num guarda que a libera ao sair de escopo.

| Onde | Blocos | Chamada | Motivo |
|---|---|---|---|
| `JobObject::kill_on_close` | 3 | `CreateJobObjectW`, `OwnedHandle::from_raw_handle`, `SetInformationJobObject` | criar o Job Object que mata a árvore de processos ao ser fechado |
| `JobObject::assign` | 1 | `AssignProcessToJobObject` | pôr o processo raiz da sessão no job |
| `JobObject::terminate` | 1 | `TerminateJobObject` | encerrar a árvore inteira ao fechar a sessão |
| `JobObject::process_ids` | 1 | `QueryInformationJobObject` | listar os processos vivos da sessão |
| `ProcessProbe::open`, `ProcessProbe::is_alive` | 3 | `OpenProcess`, `OwnedHandle::from_raw_handle`, `WaitForSingleObject` | acompanhar o mesmo processo pelo handle e consultar seu encerramento sem bloquear |
| `own_handle_count` | 1 | `GetProcessHandleCount` | medir os handles abertos do próprio processo (prova de que não vazam) |
| `process_table` | 4 | `CreateToolhelp32Snapshot`, `OwnedHandle::from_raw_handle`, `Process32FirstW`, `Process32NextW` | listar os processos do sistema com o pai (prova de que não sobra filho) |
| `LocalMemory::drop` | 1 | `LocalFree` | liberar a memória que o sistema alocou (texto do identificador, descritor de segurança) |
| `take_text` | 1 | `PWSTR::to_string` | copiar um texto devolvido pelo sistema antes de liberá-lo |
| `sid_text` | 1 | `ConvertSidToStringSidW` | escrever um identificador de segurança em texto (`S-1-...`), forma em que os donos são comparados |
| `process_token` | 2 | `OpenProcessToken` (só `TOKEN_QUERY`), `OwnedHandle::from_raw_handle` | abrir para leitura o token de um processo: o próprio ou o do servidor de um pipe |
| `token_sid` | 2 | `GetTokenInformation`, leitura do ponteiro inicial do `TOKEN_USER` ou `TOKEN_OWNER` preenchido | ler o usuário de um token ou o dono que ele dá aos objetos que cria |
| `own_token` | 1 | `GetCurrentProcess` | pseudo handle do próprio processo, sempre válido; daí saem o usuário (para nomear o canal e restringi-lo a ele) e o dono padrão |
| `pipe_server_after` | 3 | `GetNamedPipeServerProcessId`, `OpenProcess` (só `PROCESS_QUERY_LIMITED_INFORMATION`), `OwnedHandle::from_raw_handle` | o cliente descobrir qual processo serve o pipe que abriu, para ler o usuário dele |
| `object_owner_sid` | 1 | `GetSecurityInfo` (só o dono) | ler o dono do objeto do pipe, posto por quem criou o nome |
| `create_user_pipe` | 2 | `ConvertStringSecurityDescriptorToSecurityDescriptorW`, `ServerOptions::create_with_security_attributes_raw` (do `tokio`) | criar o named pipe com lista de acesso de uma entrada só, a do usuário atual |
| `access_list_text` | 2 | `GetSecurityInfo`, `ConvertSecurityDescriptorToStringSecurityDescriptorW` | ler a lista de acesso de um objeto (prova de que só o usuário abre o canal) |

## `ads-adapter-fs` (`crates/adapters/fs/src/sys/windows.rs`)

A biblioteca padrão só abre, move e apaga por caminho, e um caminho é resolvido de novo a cada chamada: quem troca uma pasta por um link entre a conferência e o uso redireciona a operação. Fechar isso pede abrir cada entrada relativa a uma pasta já aberta, sem seguir ponto de reanálise, e mover e apagar pelo identificador aberto. O Windows só oferece isso pela API nativa. Cada bloco é uma única chamada e leva um comentário `SAFETY`; os identificadores criados passam na hora para um `OwnedHandle`, que os fecha sozinho; as respostas do sistema são lidas de um buffer de palavras de 64 bits por aritmética comum, sem ponteiro. A junção dos testes continua criada com `mklink /J`, sem `unsafe`.

| Onde | Blocos | Chamada | Motivo |
|---|---|---|---|
| `open_at` | 2 | `NtCreateFile`, `OwnedHandle::from_raw_handle` | abrir ou criar uma entrada por um nome só, relativa a uma pasta aberta, sem seguir ponto de reanálise |
| `query` | 1 | `GetFileInformationByHandleEx` | ler do identificador aberto os atributos, a identidade (volume e identificador de arquivo), o tamanho e a lista de nomes de uma pasta |
| `apply` | 1 | `NtSetInformationFile` | mover sem substituir o destino e apagar, os dois sobre o identificador aberto |
| `query_volume` | 1 | `NtQueryVolumeInformationFile` | ler do identificador aberto da cerca o sistema de arquivos do volume, se ele abre por identificador e se é remoto: só onde a identidade separa um objeto recriado do substituído o executor apaga de verdade e restaura |

## `ads-testkit` (`crates/testkit/src/win.rs`), só de teste

Um processo filho do binário de teste fecha o próprio processo ou o próprio token a todos menos à conta do sistema, para fazer o papel do servidor de outra conta sem criar conta nem mudar permissões da máquina. O crate é dependência de desenvolvimento do `ads-ptyhost` e do `ads-daemon` e de nenhum outro alvo.

| Onde | Blocos | Chamada | Motivo |
|---|---|---|---|
| `LocalMemory::drop` | 1 | `LocalFree` | liberar o descritor de segurança que o sistema alocou |
| `restrict_own_process` | 6 | `ConvertStringSecurityDescriptorToSecurityDescriptorW`, `GetSecurityDescriptorDacl`, `GetCurrentProcess`, `OpenProcessToken` (só `WRITE_DAC`), `OwnedHandle::from_raw_handle`, `SetSecurityInfo` | trocar a lista de acesso do próprio processo ou do próprio token |

## `ads-adapter-vault` (`crates/adapters/vault/src/windows.rs`)

| Onde | Blocos | Operação | Motivo |
|---|---|---|---|
| `Target::store` | 1 | `CredWriteW` | guardar bytes no cofre do usuário atual |
| `Target::read` | 1 | `CredReadW` | ler a entrada pelo nome exato |
| `Target::remove` | 1 | `CredDeleteW` | remover a entrada pelo nome exato |
| `ReadCredential::blob` | 2 | desreferenciar a estrutura e montar a fatia mutável | copiar o segredo e zerar o buffer original antes de liberá-lo |
| `ReadCredential::drop` | 1 | `CredFree` | liberar uma vez o buffer alocado pelo sistema |

## `ads-private-dir` (`crates/private-dir/src/platform/win.rs`)

A biblioteca padrão não cria pasta nem arquivo com lista de acesso própria, não lê dono e lista de acesso e só abre, move e apaga por caminho, que o sistema resolve de novo a cada chamada. Por isso a pasta de controle é aberta uma vez, sem seguir ponto de reanálise e sem compartilhar o direito de apagar (o que impede renomear a pasta e as pastas acima dela enquanto o lançador roda), conferida nesse identificador, e a trava, o estado e os sinais são abertos relativos a ele, com a mesma técnica do `ads-adapter-fs`. A auditoria reutiliza a criação e a conferência pelo identificador, que fica aberto durante o uso do banco. Cada bloco é uma única chamada, ou a leitura do que a chamada anterior preencheu, e leva um comentário `SAFETY`; os identificadores abertos passam na hora para um `OwnedHandle`, e a memória que o sistema aloca fica num guarda que a libera ao sair de escopo.

| Onde | Blocos | Chamada | Motivo |
|---|---|---|---|
| `LocalMemory::drop` | 1 | `LocalFree` | liberar a memória que o sistema alocou (texto do identificador, descritor de segurança) |
| `take_text` | 1 | `PWSTR::to_string` | copiar um texto devolvido pelo sistema antes de liberá-lo |
| `sid_text` | 1 | `ConvertSidToStringSidW` | escrever um identificador de segurança em texto (`S-1-...`), forma em que donos e entradas são comparados |
| `token_sid` | 2 | `GetTokenInformation`, leitura do ponteiro inicial do `TOKEN_USER` ou `TOKEN_OWNER` preenchido | ler o usuário atual e o dono que ele dá aos objetos que cria |
| `current_identity` | 2 | `OpenProcessToken` (só `TOKEN_QUERY`) com `GetCurrentProcess`, `OwnedHandle::from_raw_handle` | abrir para leitura o token do próprio processo |
| `private_descriptor` | 1 | `ConvertStringSecurityDescriptorToSecurityDescriptorW` | montar o descritor com dono e lista de acesso protegida de uma entrada só, a do usuário |
| `create_dir` | 1 | `CreateDirectoryW` | criar a pasta privada com esse descritor, entrada herdada pelos arquivos de dentro |
| `is_network_path` | 1 | `GetDriveTypeW` | detectar unidade de rede pela raiz terminada em NUL antes de abrir o banco de auditoria |
| `kind` | 1 | `GetFileInformationByHandleEx` | ler do identificador aberto se a entrada é pasta, arquivo ou ponto de reanálise |
| `open_at` | 2 | `NtCreateFile`, `OwnedHandle::from_raw_handle` | abrir os arquivos existentes da auditoria e abrir ou criar a trava, o estado temporário e os sinais por um nome só, relativos à pasta aberta, sem seguir ponto de reanálise, e criar arquivo já com o descritor privado |
| `apply` | 1 | `NtSetInformationFile` | mover o estado temporário sobre o estado e apagar um sinal, os dois pelo identificador em que foram conferidos |
| `security` | 4 | `GetSecurityInfo` (dono e lista de acesso), `GetAclInformation`, `GetAce`, leitura do cabeçalho da entrada | conferir o dono e cada entrada da lista de acesso da pasta, dos arquivos da auditoria, da trava e de cada sinal, no identificador aberto |
| `access_list_text`, `canonical_access_list` e `descriptor_text` (testes ou `test-support`) | 3 | `GetSecurityInfo`, conversões de descritor de segurança | comparar a lista de acesso com a forma de escrita do Windows, incluindo proteção, herança e abreviação de identificadores |

## `ads-launcher` (`crates/launcher/src/console/win.rs`)

O tratador registra Ctrl+C e Ctrl+Break em uma variável atômica para o supervisor encerrar os processos. O mesmo módulo lê o horário de criação do próprio processo.

| Onde | Blocos | Chamada | Motivo |
|---|---|---|---|
| `install_console_handler` | 1 | `SetConsoleCtrlHandler` | registrar o retorno de chamada que marca a parada em uma variável atômica |
| `process_started` | 1 | `GetProcessTimes` com `GetCurrentProcess` | ler o horário de início do próprio processo, para separar um sinal `stop` deixado por uma execução anterior de um criado depois do início |
| `generate_console_interrupt` (só testes) | 2 | `SetConsoleCtrlHandler`, `GenerateConsoleCtrlEvent` | habilitar e gerar Ctrl+C dentro de um console isolado para verificar a limpeza |

## `ads-shell` (`crates/shell/src/stdin.rs`)

A casca duplica o pipe de entrada em um identificador próprio e lê a linha inicial sem o buffer global de stdin. Antes de criar a WebView, `SetStdHandle` desassocia o identificador herdado da entrada padrão e `OwnedHandle::from_raw_handle` assume seu fechamento. A duplicata é fechada após a leitura.
