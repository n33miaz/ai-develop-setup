# Orçamento de `unsafe`

Regra: `unsafe_code = "forbid"` vale para todo o workspace (`[workspace.lints.rust]` no `Cargo.toml` da raiz) e cada crate herda com `[lints] workspace = true`. As exceções em uso são duas, cada uma com uma tabela `[lints]` própria com `unsafe_code = "deny"` e o lint liberado em um só módulo, compilado apenas no Windows: `ads-ptyhost` (`crates/ptyhost/src/win.rs`) e `ads-adapter-fs` (`crates/adapters/fs/src/sys/windows.rs`). Fora desses módulos, `unsafe` continua reprovando a compilação.

Exceção prevista, ainda não usada: `ads-adapter-vault` vai precisar de chamadas do sistema (cofre de credenciais). Quando o primeiro bloco entrar, o mesmo PR troca a herança desse crate do mesmo jeito e lista cada bloco abaixo, com o motivo. A contagem só sobe com justificativa aqui.

| Crate | Blocos `unsafe` | Motivo |
|---|---|---|
| `ads-ptyhost` | 27 | chamadas da API do Windows, listadas abaixo |
| `ads-adapter-fs` | 4 | chamadas da API do Windows para abrir, mover e apagar por identificador aberto, listadas abaixo; no Linux as mesmas operações vêm do `rustix`, sem `unsafe` |
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
| `pipe_server` | 3 | `GetNamedPipeServerProcessId`, `OpenProcess` (só `PROCESS_QUERY_LIMITED_INFORMATION`), `OwnedHandle::from_raw_handle` | o cliente descobrir qual processo serve o pipe que abriu, para ler o usuário dele |
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
