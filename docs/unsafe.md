# Orçamento de `unsafe`

Regra: `unsafe_code = "forbid"` vale para todo o workspace (`[workspace.lints.rust]` no `Cargo.toml` da raiz) e cada crate herda com `[lints] workspace = true`. A única exceção em uso é o `ads-ptyhost`, que tem uma tabela `[lints]` própria com `unsafe_code = "deny"` e libera o lint em um só módulo, `crates/ptyhost/src/win.rs` (compilado apenas no Windows). Fora desse módulo, `unsafe` continua reprovando a compilação.

Exceção prevista, ainda não usada: `ads-adapter-vault` vai precisar de chamadas do sistema (cofre de credenciais). Quando o primeiro bloco entrar, o mesmo PR troca a herança desse crate do mesmo jeito e lista cada bloco abaixo, com o motivo. A contagem só sobe com justificativa aqui.

| Crate | Blocos `unsafe` | Motivo |
|---|---|---|
| `ads-ptyhost` | 22 | chamadas da API do Windows, listadas abaixo |
| demais | 0 | nenhum |

## `ads-ptyhost` (`crates/ptyhost/src/win.rs`)

Cada bloco é uma única operação (uma chamada da API do Windows pelo crate `windows`, a criação do pipe pelo `tokio`, ou a leitura de um dado que a chamada anterior preencheu) e leva um comentário `SAFETY` no código. Os handles criados passam na hora para um `OwnedHandle` da biblioteca padrão, que os fecha sozinho; a memória que o sistema aloca para o chamador fica num guarda que a libera ao sair de escopo.

| Onde | Blocos | Chamada | Motivo |
|---|---|---|---|
| `JobObject::kill_on_close` | 3 | `CreateJobObjectW`, `OwnedHandle::from_raw_handle`, `SetInformationJobObject` | criar o Job Object que mata a árvore de processos ao ser fechado |
| `JobObject::assign` | 1 | `AssignProcessToJobObject` | pôr o processo raiz da sessão no job |
| `JobObject::terminate` | 1 | `TerminateJobObject` | encerrar a árvore inteira ao fechar a sessão |
| `JobObject::process_ids` | 1 | `QueryInformationJobObject` | listar os processos vivos da sessão |
| `own_handle_count` | 1 | `GetProcessHandleCount` | medir os handles abertos do próprio processo (prova de que não vazam) |
| `process_table` | 4 | `CreateToolhelp32Snapshot`, `OwnedHandle::from_raw_handle`, `Process32FirstW`, `Process32NextW` | listar os processos do sistema com o pai (prova de que não sobra filho) |
| `LocalMemory::drop` | 1 | `LocalFree` | liberar a memória que o sistema alocou (texto do identificador, descritor de segurança) |
| `take_text` | 1 | `PWSTR::to_string` | copiar um texto devolvido pelo sistema antes de liberá-lo |
| `current_user_sid` | 5 | `OpenProcessToken`, `OwnedHandle::from_raw_handle`, `GetTokenInformation`, leitura do `TOKEN_USER` preenchido, `ConvertSidToStringSidW` | saber quem é o usuário do processo, para nomear o canal e restringi-lo a ele |
| `create_user_pipe` | 2 | `ConvertStringSecurityDescriptorToSecurityDescriptorW`, `ServerOptions::create_with_security_attributes_raw` (do `tokio`) | criar o named pipe com lista de acesso de uma entrada só, a do usuário atual |
| `access_list_text` | 2 | `GetSecurityInfo`, `ConvertSecurityDescriptorToStringSecurityDescriptorW` | ler a lista de acesso de um objeto (prova de que só o usuário abre o canal) |
