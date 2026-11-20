# Registro de riscos

Cada risco conhecido do produto, com dono, teto e plano B (P5, ver [AGENTS.md](../AGENTS.md)). Este arquivo não repete os detalhes: aponta para onde a mitigação está escrita ou implementada.

Estado da mitigação, em cada risco:

- **implementada e testada**: há código em `develop` e um teste ou um job de CI que a cobra;
- **decidida, não implementada**: há ADR ou Schema, e nenhum código a aplica ainda;
- **ausente**: nem decisão nem código.

"Teto" é o parâmetro que limita o pior caso, quando existe. "Não definido" quer dizer que ainda não há parâmetro.

## Resumo

| Risco | Dono | Estado da mitigação |
|---|---|---|
| [Ação destrutiva fora do escopo](#ação-destrutiva-fora-do-escopo) | portão de ações e executor de arquivos | implementada na biblioteca do daemon |
| [Troca no disco durante a execução](#troca-no-disco-durante-a-execução) | executor de arquivos | implementada e testada, com limites escritos |
| [Licença de execução usada duas vezes](#licença-de-execução-usada-duas-vezes) | portão, executor e armazenamento | protegido pelo diário persistente |
| [Perda de sessões de terminal](#perda-de-sessões-de-terminal) | host de terminais | implementada e testada; host supervisionado pelo [lançador](../crates/launcher/README.md) |
| [Outro usuário da máquina alcança as sessões](#outro-usuário-da-máquina-alcança-as-sessões) | host de terminais e daemon | implementada e testada no Windows; teste com duas contas ausente |
| [Credencial de CLI de IA vaza para o processo filho](#credencial-de-cli-de-ia-vaza-para-o-processo-filho) | adaptadores de ferramenta e host | parcial: filtro do ambiente implementado |
| [Programa ou pasta configurados por um link](#programa-ou-pasta-configurados-por-um-link) | adaptadores de ferramenta | implementada e testada; a abertura usa o caminho resolvido |
| [Página ou programa local comanda os terminais](#página-ou-programa-local-comanda-os-terminais) | daemon (API local) | implementada e testada; ADR proposta; conferência manual da porta da interface |
| [Dado sensível em claro no disco](#dado-sensível-em-claro-no-disco) | armazenamento | parcial: `data` dos eventos cifrado no banco e testado; chave ainda fora do cofre |
| [Perda do estado local](#perda-do-estado-local) | armazenamento | parcial: abertura recusa arquivo corrompido sem alterá-lo; backup ausente |
| [Atualização ruim instalada](#atualização-ruim-instalada) | lançador | decidida, não implementada |
| [Elevação de privilégio](#elevação-de-privilégio) | auxiliar de elevação | decidida, não implementada |
| [Exposição pelo transporte remoto](#exposição-pelo-transporte-remoto) | relé e sessão cifrada | decidida, não implementada |
| [Cliente remoto adulterado pelo Worker](#cliente-remoto-adulterado-pelo-worker) | relé e interface remota | ausente (proposta na ADR 0023) |
| [Comando remoto repetido ou de aparelho indevido](#comando-remoto-repetido-ou-de-aparelho-indevido) | núcleo e sessão cifrada | decidida (proposta), não implementada |
| [Suspensão de conta por uso automatizado de CLI](#suspensão-de-conta-por-uso-automatizado-de-cli) | adaptadores e política de troca de conta | decidida, não implementada |
| [Login sem chave do Console desfaz os perfis do Claude](#login-sem-chave-do-console-desfaz-os-perfis-do-claude) | mantenedor e adaptador Claude | só regra escrita |
| [Isolamento falso entre perfis do Codex](#isolamento-falso-entre-perfis-do-codex) | mantenedor e adaptador Codex | ausente; teste na máquina pendente |
| [Pasta de perfil lida ou alterada pelo produto](#pasta-de-perfil-lida-ou-alterada-pelo-produto) | núcleo (ligação ao perfil) e coletores | decidida, não implementada |
| [Chave passa na frente do login do perfil](#chave-passa-na-frente-do-login-do-perfil) | adaptadores de ferramenta | parcial: filtro do ambiente do pai |
| [Gasto de IA sem teto](#gasto-de-ia-sem-teto) | núcleo (orçamento) | parcial |
| [Texto de ferramenta tratado como instrução](#texto-de-ferramenta-tratado-como-instrução) | núcleo e adaptadores | só regra escrita |
| [Instruções de agente sobrepostas ou ignoradas](#instruções-de-agente-sobrepostas-ou-ignoradas) | mantenedor | parcial |
| [Segredo ou dado pessoal no repositório público](#segredo-ou-dado-pessoal-no-repositório-público) | mantenedor e CI | parcial |
| [Dependência com licença incompatível](#dependência-com-licença-incompatível) | mantenedor e CI | implementada e testada |
| [Cadeia de suprimento do CI e das dependências](#cadeia-de-suprimento-do-ci-e-das-dependências) | mantenedor e CI | parcial |
| [Código `unsafe`](#código-unsafe) | crates com exceção | implementada e testada |

## Arquivos e ações destrutivas

### Ação destrutiva fora do escopo

- **Descrição.** Um agente pede para apagar, mover ou sobrescrever algo fora da pasta de trabalho, uma pasta protegida (raiz, pasta do usuário, sistema) ou um repositório inteiro, por erro de texto, de caminho ou de plano.
- **Impacto.** Perda de dados do usuário sem volta.
- **Dono.** Portão de ações ([`crates/core`](../crates/core/README.md#portão-de-ações-destrutivas-srcgate)) e executor de arquivos ([`crates/adapters/fs`](../crates/adapters/fs/README.md)).
- **Teto.** `approval_item_limit` e `approval_byte_limit` (acima deles, aprovação), `quarantine_enabled` (apagar vira mover para a quarentena) em [`defaults/action-gate.toml`](../defaults/action-gate.toml); `definitive_delete` em [`defaults/fs-executor.toml`](../defaults/fs-executor.toml).
- **Mitigação existente (implementada e testada).** O portão normaliza o caminho só por texto, nega pastas protegidas, raiz de trabalho, raízes permitidas e repositórios, exige aprovação acima dos limites e reescreve apagar como quarentena ([regras](../crates/core/README.md#regras)); a aprovação vale só para o `plan_hash` do plano julgado ([máquina de estados](../crates/core/README.md#máquina-de-estados)). O executor julga de novo com a política confiável, abre tudo por identificador sem seguir link e só move ou apaga o que levantou ([o que é conferido](../crates/adapters/fs/README.md#o-que-é-conferido-antes-da-primeira-alteração)). Testes em `crates/core/tests/gate_*.rs` e `crates/adapters/fs/tests/`; os jobs `rust` e `fs-windows` do CI rodam os dois.
- **Plano B.** Quarentena com [restauração](../crates/adapters/fs/README.md#restauração); execução que para diz exatamente o que mudou ([falha no meio](../crates/adapters/fs/README.md#falha-no-meio-e-recuperação)).
- **O que falta.** A API local ainda não expõe operações de arquivo. Curinga, `fs.overwrite`, `fs.clean_untracked` e mover para fora da quarentena são recusados por falta de definição ([o que não é executado](../crates/adapters/fs/README.md#o-que-não-é-executado)). Purga de leva com repositório não tem regra. Um caminho protegido escrito em outra forma Unicode não é reconhecido pelo texto; só o executor, no disco, o alcança.

### Troca no disco durante a execução

- **Descrição.** Outro processo troca uma pasta por um link, renomeia ou substitui uma entrada entre a conferência e a operação.
- **Impacto.** A operação age sobre outro objeto, inclusive fora da cerca.
- **Dono.** Executor de arquivos.
- **Teto.** `definitive_delete = "contained"` (padrão): no Linux nenhum apagar definitivo.
- **Mitigação existente (implementada e testada).** Identificadores abertos em vez de caminhos, pastas seguradas no lugar no Windows, conferência de identidade e tamanho, mover sem substituir; o que é garantido para cada tipo de adversário está no [Modelo de ameaça](../crates/adapters/fs/README.md#modelo-de-ameaça). Testes em `tests/review.rs`, `tests/contain.rs` e `tests/settle.rs` do crate.
- **Plano B.** O passo sai "não verificado" quando a chegada não confere; nada é apagado nem sobrescrito por esses caminhos.
- **O que falta.** Tudo o que está em [Deliberadamente fora](../crates/adapters/fs/README.md#deliberadamente-fora): escritor concorrente depois da última conferência, contenção de quarentena e restauração contra processo hostil, apagar definitivo no Linux sob concorrência hostil, montagem real e corte de energia real sem teste, letra de unidade remapeada, caminhos longos e arquivo em uso sem teste.

### Licença de execução usada duas vezes

- **Descrição.** A mesma aprovação ou a mesma licença executa a ação mais de uma vez, por repetição, por reinício ou por dois executores.
- **Impacto.** A ação roda de novo sobre um disco que já mudou.
- **Dono.** Portão, executor e armazenamento.
- **Teto.** Um início por ação e uma ação por aprovação. O tempo de espera pelo lock é `busy_timeout_ms`.
- **Mitigação existente.** Dentro de um processo, nem a ação nem a licença podem ser copiadas e o `replay` recusa log com `action.started` repetido ([máquina de estados](../crates/core/README.md#máquina-de-estados)); o executor exige que o diário registre o consumo antes de tudo e lembra as licenças recebidas (`E_FS_PERMIT_REUSED`); a pasta da ação na quarentena é criada de forma exclusiva ([entrada do executor](../crates/adapters/fs/README.md#entrada)).
- **Plano B.** A quarentena deixa marca no disco que recusa a repetição.
- **Mitigação no log.** O [diário do portão](../crates/storage/README.md#diário-do-portão) verifica a cadeia HMAC antes de confiar em colisões, confirma a gravação antes de entregar a licença e bloqueia a reexecução após reinício. Os testes cobrem corrida entre conexões, abandono de uma licença já gravada, segundo início e aprovação de outra ação com `seq` livre, remoção do início gravado e alteração de um registro com conexão aberta.
- **O que falta.** Medir a latência em históricos grandes e expor a composição sob a API local com o fluxo de confirmação. Uma queda entre a atualização da âncora e o commit pode bloquear o banco até a recuperação. Depois de um início confirmado, o operador confere o disco e prepara outra ação com nova decisão se ainda houver trabalho.

## Terminais

### Perda de sessões de terminal

- **Descrição.** Atualizar, reiniciar ou derrubar o núcleo encerra as sessões de terminal em andamento ou perde parte da saída.
- **Impacto.** Trabalho de agente interrompido; saída perdida.
- **Dono.** Host de terminais ([`crates/ptyhost`](../crates/ptyhost/README.md)) e o cliente dele no [daemon](../crates/daemon/README.md).
- **Teto.** `replay_capacity_bytes`, `subscriber_queue_chunks`, `max_sessions` e `max_frame_bytes` em [`defaults/ptyhost.toml`](../defaults/ptyhost.toml).
- **Mitigação existente (implementada e testada).** Processo separado ([ADR 0003](adr/0003-separate-pty-host-process.md)) com anel de replay numerado: quem reconecta retoma pelo `seq`, sem buraco e sem duplicata, e pedido fora do anel volta erro com o limite ([regras do replay](../crates/ptyhost/README.md#regras-do-replay)). Testes `client_dropped_twenty_times_loses_and_repeats_nothing`, `real_terminal_survives_twenty_client_drops_without_losing_a_byte` e os 20 ciclos de ConPTY (`twenty_cycles_leave_no_process_and_no_handle`) no job `pty-windows`. Na API local, a preparação que vence após uma abertura confirmada preserva a vaga até o host confirmar a limpeza. O descarte por cota de uma espera acorda o assinante ocioso e encerra só a assinatura dessa encarnação. No Windows, uma sessão cujo shell sai deixando vivo um filho num console próprio, dentro do job, não deixa processo depois da devolução automática da vaga (`ended_session_leaves_no_child_behind_on_windows`). Os limites estão no [README do daemon](../crates/daemon/README.md).
- **Plano B.** Condição de reversão da ADR 0003; para o PTY no Windows, ConPTY direto.
- **O que falta.** Reassumir a supervisão do host após `stop` segue pendente no [lançador](../crates/launcher/README.md). Os limites de desligamento, criação de processos e conexões estão no [README do host](../crates/ptyhost/README.md#limites-conhecidos). O daemon não adota sessões antigas ao reiniciar. A fila já define o destino da tarefa em `running` quando o núcleo reinicia (espera de `host_confirm_seconds` pela confirmação do host e cancelamento terminal se o prazo vencer, sem gastar tentativa). O daemon ainda precisa parear tarefa e sessão, consultar o host, aplicar o prazo e solicitar o encerramento se a sessão reaparecer ([fila contínua](../crates/core/README.md#fila-contínua)).

### Outro usuário da máquina alcança as sessões

- **Descrição.** Outra conta da mesma máquina, ou um serviço, conecta no canal do host e lê ou comanda os terminais.
- **Impacto.** Quebra do isolamento entre usuários (P3).
- **Dono.** Host de terminais e daemon.
- **Teto.** Não se aplica.
- **Mitigação existente (implementada e testada).** Named pipe com lista de acesso de uma entrada só, a do usuário, e recusa de subir se o nome já existe; socket Unix `0600` ([como roda](../crates/ptyhost/README.md#como-roda)). No Windows o cliente só entrega a conexão depois de confirmar que o pipe é servido pelo usuário atual, antes de enviar qualquer byte ([daemon](../crates/daemon/README.md)). Testes `pipe_grants_access_to_the_current_user_only` em `crates/ptyhost/src/channel.rs`, `crates/ptyhost/tests/pipe_owner.rs` (recusa e liberação dos handles com falhas repetidas das consultas reais) e `crates/daemon/tests/channel_owner.rs` (pela entrada pública, o servidor filho que fecha processo ou token aos outros não recebe o `host.hello`; o pipe do próprio usuário recebe um `host.hello` completo).
- **Plano B.** Não há.
- **O que falta.** O teste com duas contas que o P3 pede não roda sozinho: a recusa de um pipe de outro usuário é testada com identificadores sintéticos, com as consultas reais comparadas a outra identidade e com um servidor do mesmo usuário cujo processo ou token os outros não abrem; os casos com outra conta e com níveis de elevação diferentes são testes manuais, ignorados por padrão e ainda não rodados ([limites conhecidos](../crates/ptyhost/README.md#limites-conhecidos)). Fora do Windows o cliente não confere quem serve o socket. Um host elevado que dá o pipe ao grupo de administradores é recusado por um cliente não elevado do mesmo usuário; um cliente elevado aceita o host não elevado do mesmo usuário (o que o código decide, ainda não conferido na máquina).

### Credencial de CLI de IA vaza para o processo filho

- **Descrição.** Variáveis de autenticação do ambiente do usuário, ou arquivos de credencial de um CLI de IA, chegam a um processo que não deveria vê-los, ou são lidos pelo produto (C1).
- **Impacto.** Credencial exposta ou login do CLI alterado.
- **Dono.** Adaptadores de ferramenta e host de terminais.
- **Teto.** Não se aplica.
- **Mitigação existente.** [`child_env`](../crates/core/src/child_env.rs) filtra o ambiente herdado com a lista comum `allow` e os padrões `deny` de [`[env]`](../defaults/harnesses.toml), para as duas ferramentas integradas. O processo filho vê só o `env` do pedido, nada herdado do host (testes `the_child_environment_is_exactly_what_the_spec_carries` e `child_sees_only_the_environment_of_the_spec`). O comando `session.open` da API local recusa `program`, `args`, `env` e `cwd` no Schema ([API local](../contracts/README.md#api-local)). Os erros do cliente do daemon não carregam ambiente (`errors_never_include_the_host_message_or_environment`).
- **Plano B.** Não há.
- **Mitigação existente (ambiente das ferramentas).** O filho recebe os nomes de `allow` e os valores de `env.set`. O lançador recusa credenciais conhecidas nas duas listas durante a carga, antes de preparar a abertura (`launcher_configuration_refuses_credentials_before_a_session_exists`). O teste `a_real_child_gets_exactly_the_allowed_names_and_the_set_values` executa um programa do sistema com o ambiente montado pelo núcleo e confere os nomes e valores recebidos. No Windows, a cópia do pai compara só ASCII. A recusa de duplicatas e a precedência de `env.set` usam uma comparação mais ampla, que também trata `ÄX` e `äx` como equivalentes.
- **O que falta.** Ainda falta o teste da regra C1 que compara o status do CLI antes e depois da sessão. Os padrões cobrem credenciais pelo nome, inclusive nomes genéricos de sessão como `APP_SESSION_ID`. Uma credencial sob um nome que escape dos padrões ainda pode passar se esse nome for incluído em `allow` ou `env.set`. Em Linux e macOS ainda falta verificar com um CLI real se a ferramenta e os programas que ela abre acham suas configurações sem `HOME`.

### Programa ou pasta configurados por um link

- **Descrição.** O programa ou a pasta de trabalho configurados são uma junção ou um link simbólico. As conferências da configuração leem só o texto; ao abrir a sessão, a busca segue o link.
- **Impacto.** Um link pode levar a outro programa, a uma pasta fora do local esperado, à raiz de um volume ou a um compartilhamento de rede.
- **Dono.** Adaptadores de ferramenta (`ads-core::tool_launch`).
- **Teto.** Não se aplica.
- **Mitigação existente.** Na abertura, o núcleo canonicaliza o caminho, recusa destinos de rede e prefixos de dispositivo no Windows, e recusa uma pasta de trabalho que resolva para a raiz. O programa deve resolver para um arquivo e a pasta para um diretório. O host recebe o caminho resolvido. Os testes em disco usam junções no Windows e links simbólicos nos demais sistemas.
- **Plano B.** Corrigir o caminho no arquivo de configuração se o destino for recusado.
- **O que falta.** O lançamento ainda usa caminhos. Uma entrada no caminho resolvido pode mudar entre a conferência e o uso pelo host. No Linux e no macOS uma montagem de rede (NFS, SMB) parece um caminho local e não é reconhecida. Passar o caminho resolvido quebra programas que dependem do nome com que são chamados (atalhos de gerenciadores de versão, `busybox`, atalhos de aplicativos da Microsoft Store no Windows); nesse caso, configure o programa real.

### Página ou programa local comanda os terminais

- **Descrição.** Uma página aberta no navegador (WebSocket não tem política de mesma origem), DNS rebinding, outro usuário da máquina ou outro programa conecta na API local em loopback.
- **Impacto.** Comando arbitrário num terminal do usuário com uma ferramenta de IA dentro.
- **Dono.** Daemon (API local).
- **Teto.** Prazo do `auth.hello` (padrão proposto 5 s).
- **Mitigação existente (implementada, ADR ainda proposta).** [ADR 0020](adr/0020-local-api-authentication.md): porta efêmera em `127.0.0.1`, conferência de `Host` e `Origin`, token por execução entregue pelo fragmento e conferido em tempo constante, recusa de origem remota. Os testes em [`local_api.rs`](../crates/daemon/tests/local_api.rs) exercitam a autenticação, inclusive o prazo único. O Schema `urn:ads:command:auth.hello:1` existe.
- **Plano B.** Condição de reversão da ADR 0020.
- **O que falta.** A ADR aguarda ratificação. A propriedade da porta da interface ainda exige [conferência manual](#outro-usuário-ocupa-a-porta-da-interface). O [README da interface](../ui/README.md) descreve o cliente.

### Outro usuário ocupa a porta da interface

- **Descrição.** Outra conta ocupa a porta de desenvolvimento da interface antes do servidor legítimo; sua página recebe o token no fragmento da URL impressa.
- **Impacto.** Controle das sessões do usuário, mesmo com a API escutando só em loopback.
- **Dono.** Daemon (entrega do token) e interface (servidor de desenvolvimento).
- **Teto.** Um token por execução; o alcance fica nas sessões dessa execução. Não há teto de dano dentro de um terminal autorizado, portanto não abrir a URL sem verificar a propriedade da porta.
- **Mitigação existente.** `ui_base_url` parametrizável em `defaults/daemon.toml`; `allowed_origins` vazio deriva somente a origem dessa URL. O Vite usa porta fixa e `strictPort` no desenvolvimento e na prévia, escutando em `127.0.0.1`; o `ui_base_url` padrão usa o mesmo endereço, e não `localhost`, para o navegador não tentar `[::1]:5173`, que outra conta pode ocupar sem colidir com o `strictPort`. Um teste compara os dois endereços. Só abrir depois de conferir que o processo que escuta é do próprio usuário. Configuração e igualdade exata das origens têm testes; a propriedade da porta da interface ainda depende da conferência manual.
- **Plano B.** Não abrir a URL quando a propriedade não puder ser confirmada; parar o daemon para invalidar o token e reiniciá-lo depois de resolver a ocupação da porta.
- **O que falta.** O daemon servir a interface na sua porta efêmera ou a casca Tauri entregar o token por canal do usuário, conforme o [limite conhecido da ADR 0020](adr/0020-local-api-authentication.md#limite-conhecido). O teste com duas contas no Windows ainda falta.

### Outra conta ocupa vagas de autenticação pendente

- **Descrição.** Outra conta local pode manter conexões TCP abertas e renová-las para ocupar as vagas pendentes.
- **Impacto.** Novas conexões da interface podem falhar. As conexões já autenticadas têm vagas próprias.
- **Dono.** Daemon (API local).
- **Teto.** O padrão de `max_pending_auth` é 32 vagas, com o prazo de `auth_timeout_ms` por conexão, incluindo o handshake.
- **Mitigação existente.** O padrão de 32 exige mais sockets simultâneos para ocupar toda a fila. O limite conta tarefas, inclusive as que estão sendo expulsas, até terminarem. Com a fila cheia, a conexão mais antiga que ainda não concluiu o upgrade é expulsa e a nova entra quando a vaga volta: segurar as vagas com sockets crus exige abri-los sem parar. A espera entre falhas de aceitação permite que o serviço continue atendendo as conexões já autenticadas. Uma conexão que já fez upgrade conserva a vaga até autenticar ou vencer `auth_timeout_ms`, então a interface não é expulsa entre o upgrade e o `auth.hello`; se todas as vagas estiverem nessa fase, a conexão nova é recusada. Testes em `crates/daemon/tests/local_api.rs` (33ª conexão, conexão em autenticação preservada, recusa com todas em autenticação) e em `crates/daemon/src/api.rs` (teto que inclui as expulsas).
- **Plano B.** Ajustar `max_pending_auth` em `defaults/daemon.toml` ou na configuração local e reiniciar o daemon.
- **O que falta.** Quem renova sockets mais rápido do que a interface conclui o upgrade ainda pode expulsá-la antes dele; quem faz upgrade em todas as vagas e não manda `auth.hello` bloqueia conexões novas até `auth_timeout_ms` vencer, e precisa repetir isso a cada prazo. Não há limite por endereço nem por conta, porque em loopback todas as contas chegam de `127.0.0.1`. A saída é a mesma do risco anterior: o daemon servir a interface ou a casca Tauri entregar o token por canal só do usuário.

## Estado local

### Dado sensível em claro no disco

- **Descrição.** Texto de terminal, prompts e caminhos gravados sem cifra, ou chave em arquivo.
- **Impacto.** Quem lê o disco lê o trabalho do usuário.
- **Dono.** Armazenamento ([`crates/storage`](../crates/storage/README.md)) e cofre ([`crates/adapters/vault`](../crates/adapters/vault/README.md)).
- **Teto.** Não se aplica.
- **Mitigação existente.** [ADR 0005](adr/0005-column-level-encryption.md): cifra autenticada por coluna, chave no cofre do sistema. A cifra (XChaCha20-Poly1305, com dados associados autenticados) está implementada e testada em `crates/storage/src/cipher.rs`, inclusive recusa de chave e de dados associados trocados e diagnóstico sem chave nem texto ([ADR 0021](adr/0021-storage-atomic-file-and-cipher.md), proposta). O log de eventos (`crates/storage/src/log.rs`) grava o `data` de todo evento cifrado, com as colunas em claro da linha como dados associados; o teste da ADR 0005 procura uma frase sentinela no banco, no `-wal` e no `-shm`, reabre e recusa chave errada (`crates/storage/src/event_log_tests.rs`). Hoje nada sensível chega ao log: nenhum produtor está ligado a ele, o anel de replay fica só em memória e o cliente do daemon não persiste saída nem tem telemetria ([daemon](../crates/daemon/README.md)).
- **Plano B.** Banco inteiro cifrado (condição de reversão da ADR 0005).
- **O que falta.** A integração da chave com o cofre ainda falta. A composição deve recusar a persistência sensível se o cofre estiver indisponível ou se a plataforma não for Windows. O limite por entrada e a execução do teste local estão no [README do cofre](../crates/adapters/vault/README.md). Ficam em claro, por decisão da ADR 0005, `id`, `type`, `ts`, `stream`, `seq`, autor, `cause`, classe do dado, `action_id` e `approval_id` (os dois só nos eventos do portão, e o primeiro já está no nome do stream): revelam o ritmo de uso e os nomes dos streams.

### Perda do estado local

- **Descrição.** Banco local corrompido ou apagado.
- **Impacto.** Perda de auditoria, métricas e estado das tarefas.
- **Dono.** Armazenamento.
- **Mitigação existente.** A abertura confere o arquivo antes de tocá-lo e recusa arquivo estranho, mais novo, truncado ou corrompido sem alterar os bytes do banco nem do `-wal`, recusa `-journal` ou `-wal` sobrando que o SQLite desfaria ou apagaria e recusa WAL do produto em que o SQLite descartaria commits confirmados ([reparo manual](../crates/storage/README.md#reparo-de--journal-sobrando)); `synchronous = FULL`; as linhas e os registros de chaves ficam num arquivo só ([ADR 0021](adr/0021-storage-atomic-file-and-cipher.md), proposta). Testes em `crates/storage/tests/open.rs`. O log de eventos é só-anexo e encadeado: gatilhos recusam UPDATE, DELETE, substituição e salto de `seq` por stream, e `verify_chain` aponta a primeira linha alterada, forjada, fora de ordem ou faltando no meio, com a cadeia em HMAC de chave derivada da chave da cifra; a conexão do produto recusa DDL depois da migração (`crates/storage/src/event_log_tests.rs`).
- **Teto.** `inspection_max_bytes`, `integrity_check_on_open`, `max_event_bytes`, `read_page_limit` e `read_page_max_bytes` em [`defaults/storage.toml`](../defaults/storage.toml).
- **Plano B.** Restaurar uma cópia que corresponda à âncora ou usar a [recuperação confirmada](../crates/storage/README.md#âncora-no-cofre) após conferir a cadeia. Cofre inacessível ou divergência bloqueiam escrita.
- **O que falta.** Backup e restauração testada (P5), trava de instância e pasta por usuário (a ADR 0021 as deixa para a composição). Quem escreve no arquivo pode remover os gatilhos e alterar linhas; sem a chave não recalcula a cadeia, e a abertura confere a âncora para detectar alterações no fim do log e nos registros de chaves. A abertura também inspeciona o esquema e verifica a cadeia integralmente. Falta expor ao dono um comando do daemon para preparar, conferir e aplicar o plano de recuperação.

## Atualização e privilégio

### Atualização ruim instalada

- **Descrição.** Uma versão juntada à `develop` (que é a produção) quebra o programa instalado, ou um binário adulterado é instalado.
- **Impacto.** Produto parado ou código não confiável rodando.
- **Dono.** Lançador ([`crates/launcher`](../crates/launcher/README.md)).
- **Teto.** Duas trocas por dia; prova de 10 minutos com volta em três quedas (ADR 0015).
- **Mitigação existente (decidida, não implementada).** [ADR 0015](adr/0015-ci-built-artifact-with-verified-self-update.md): manifesto assinado no CI e conferido com chave embutida, versão anterior guardada, volta automática. Os Schemas `record.update-manifest` e `record.update-status` existem.
- **Plano B.** Suspender a atualização automática (condição de reversão da ADR 0015).
- **O que falta.** A atualização e a conferência de saúde e do commit esperado seguem pendentes, conforme o [README do lançador](../crates/launcher/README.md). O job de artefato do CI está só descrito em comentário.

### Elevação de privilégio

- **Descrição.** O produto ganha direitos de administrador além do necessário, ou um núcleo comprometido se eleva.
- **Impacto.** Comprometimento da máquina.
- **Dono.** Auxiliar de elevação (sem crate ainda).
- **Teto.** Lista fechada de verbos; uma confirmação do sistema por ação.
- **Mitigação existente (decidida, não implementada).** [ADR 0009](adr/0009-elevation-by-uac-signed-broker-later.md): UAC por ação, nada elevado residente; corretor assinado desenhado e desligado.
- **Plano B.** Voltar a só mostrar o comando (condição de reversão da ADR 0009).
- **O que falta.** Não há código de elevação, nem teste de que o daemon roda como usuário comum.

## Remoto e provedores

### Exposição pelo transporte remoto

- **Descrição.** O relé vê texto em claro, guarda credencial, ou aceita comando não assinado.
- **Impacto.** Terminais comandados de fora da máquina.
- **Dono.** Relé ([`crates/adapters/relay`](../crates/adapters/relay/README.md)) e sessão cifrada ([`crates/e2e`](../crates/e2e/README.md)).
- **Teto.** `remote.transport = none` por padrão, decidido na ADR 0008; o parâmetro ainda não existe em `defaults/`.
- **Mitigação existente (decidida, não implementada).** [ADR 0008](adr/0008-relay-worker-as-first-remote-transport.md): cifra de ponta a ponta, comandos assinados pelo aparelho, relé na conta do próprio usuário. Schemas `relay.envelope` e `relay.device`; a API local recusa `origin: "remote"` (ADR 0020).
- **Plano B.** Trocar de transporte (condição de reversão da ADR 0008).
- **O que falta.** Relé e sessão cifrada são esqueletos.

### Cliente remoto adulterado pelo Worker

- **Descrição.** O Worker que serve a PWA (ADR 0008) é comprometido, por exemplo pela conta do provedor, e entrega ao celular um JavaScript adulterado.
- **Impacto.** O cliente adulterado lê o que decifra e assina comandos em nome do aparelho; a cifra de ponta a ponta não protege contra quem entrega o código do cliente.
- **Dono.** Relé e interface remota.
- **Teto.** `remote.transport = none` por padrão (ADR 0008). Não definido com o remoto ligado.
- **Mitigação existente.** Ausente. A [ADR 0023](adr/0023-remote-session-encryption.md) (proposta) registra o risco.
- **Plano B.** Desligar o transporte remoto e revogar todos os aparelhos no PC.
- **O que falta.** PWA instalada e fixada por versão, com conferência de integridade do pacote; confirmação no PC para ação destrutiva vinda do remoto; proteção da conta do Worker (segundo fator) no roteiro de instalação.

### Comando remoto repetido ou de aparelho indevido

- **Descrição.** Um comando assinado é reenviado, um QR de pareamento é fotografado, ou um aparelho roubado continua inscrito.
- **Impacto.** Comando executado de novo ou por quem não devia, num terminal do usuário.
- **Dono.** Núcleo (conferência de assinatura e inscrição) e sessão cifrada ([`crates/e2e`](../crates/e2e/README.md)).
- **Teto.** Validade proposta de 5 minutos para o código de pareamento e de 60 segundos para o comando; número máximo de aparelhos configurável ([ADR 0023](adr/0023-remote-session-encryption.md), proposta).
- **Mitigação existente (decidida, não implementada).** O Schema `command.request` já exige `device_id`, assinatura, `nonce`, `counter` e `expires_at` para origem remota; a ADR 0023 propõe código de uso único confirmado no PC, passkey conferida pelo núcleo dentro do canal e revogação no PC.
- **Plano B.** Revogar o aparelho; desligar o transporte remoto.
- **O que falta.** Biblioteca, uso por WASM e conta do Worker aguardam decisão do mantenedor; persistência do contador antes de executar; vetores de teste comuns ao PC e ao celular.

### Suspensão de conta por uso automatizado de CLI

- **Descrição.** Uso automatizado de uma CLI de IA fora dos termos do fornecedor, inclusive troca automática entre contas de assinatura.
- **Impacto.** Conta do usuário suspensa.
- **Dono.** Adaptadores de ferramenta e política de troca de conta.
- **Teto.** `adapters.agy.headless = false` por padrão, decidido na ADR 0010. Troca de conta: `switch = "propose"` por padrão (o mantenedor confirma cada troca) até o isolamento por perfil estar validado; Antigravity fixo em manual ([ADR 0022](adr/0022-account-profile-folders-and-automatic-switching.md)).
- **Mitigação existente (decidida, não implementada).** [ADR 0010](adr/0010-api-key-for-automation-agy-headless-on-demand.md): automação pela chave de API, modo sem interface só depois de aceite registrado. [ADR 0022](adr/0022-account-profile-folders-and-automatic-switching.md): troca só entre pastas de perfil em que o mantenedor fez o login, nunca no meio da sessão. O FAQ do Antigravity declara a rotação automática fora dos termos; Claude Code e Codex documentam várias contas por pasta, sem dizer se a troca automática entre elas é aceita.
- **Plano B.** Voltar a troca para manual em todas as ferramentas (condição de reversão da ADR 0022); retirar a CLI do produto (condição de reversão da ADR 0010).
- **O que falta.** Os adaptadores de Antigravity e API são esqueletos; os parâmetros `adapters.agy.headless`, `switch` e `switch_at_pct` ainda não existem em `defaults/`. Os termos atuais de Claude e Codex sobre troca automática entre contas não foram conferidos.

### Gasto de IA sem teto

- **Descrição.** Tarefas ou tentativas repetidas consomem cota ou custo sem limite.
- **Impacto.** Custo inesperado ou cota esgotada.
- **Dono.** Núcleo (orçamento e roteamento).
- **Teto.** USD faturável, percentual de cota, turnos e tempo definidos no pacote de cada tarefa.
- **Mitigação existente.** O núcleo avalia o consumo observado e calcula uma rota alternativa quando há cota. Veja os parâmetros e a interpretação das medições no [README do núcleo](../crates/core/README.md#orçamento-e-roteamento).
- **Plano B.** Manter a rota manual em observação. Sem alternativa elegível, a proposta fica vazia.
- **O que falta.** Coletar uso e aplicar a decisão de parada entre tentativas. Até essa integração, a avaliação no núcleo não interrompe o gasto.

### Texto de ferramenta tratado como instrução

- **Descrição.** Saída de terminal, conteúdo da web ou o pacote de uma tarefa é obedecido como ordem.
- **Impacto.** Ação não pedida pelo usuário.
- **Dono.** Núcleo e adaptadores.
- **Teto.** Não se aplica.
- **Mitigação existente.** Só regra: o [AGENTS.md](../AGENTS.md) diz que esse texto é dado; o Schema `record.task-packet` diz que permissões vêm da configuração do usuário, não do pacote; toda ação destrutiva passa pelo portão.
- **Plano B.** O portão de ações limita o dano em arquivos.
- **O que falta.** Nada no código separa instrução de dado além do portão.

## Contas por pasta de perfil

Riscos da troca de conta por pasta de perfil ([ADR 0022](adr/0022-account-profile-folders-and-automatic-switching.md)). O núcleo valida a configuração de perfil e oferece a ligação após o filtro de ambiente. A abertura ainda usa o filtro sem ligação a perfil.

### Login sem chave do Console desfaz os perfis do Claude

- **Descrição.** Num perfil do Claude Code, alguém escolhe o login do Console sem criar chave. A documentação oficial diz que esse login fica fora da pasta de configuração e desfaz todos os logins claude.ai da máquina.
- **Impacto.** Todos os perfis do Claude perdem o login de uma vez; a troca de conta para.
- **Dono.** Mantenedor (preparo dos perfis) e adaptador Claude (observação do status).
- **Teto.** Não se aplica.
- **Mitigação existente.** Só regra: o roteiro de preparo dos perfis proíbe esse login.
- **Plano B.** Refazer o login claude.ai de cada perfil, pela própria ferramenta.
- **O que falta.** A sonda de status (`claude auth status`, campo `authMethod`) acusar perfil fora do método esperado e bloquear a troca para ele.

### Isolamento falso entre perfis do Codex

- **Descrição.** Com o login no chaveiro do sistema, dois valores de `CODEX_HOME` podem usar a mesma entrada; a documentação oficial não diz qual é o padrão de `cli_auth_credentials_store` nem se a entrada do chaveiro depende de `CODEX_HOME`.
- **Impacto.** Dois perfis na mesma conta sem ninguém perceber, ou o login de um perfil sobrescrevendo o de outro.
- **Dono.** Mantenedor (teste na máquina) e adaptador Codex.
- **Teto.** Não se aplica.
- **Mitigação existente.** Ausente. A ADR 0022 só dá o isolamento como confirmado com `cli_auth_credentials_store = "file"` em cada perfil e depois do teste na máquina.
- **Plano B.** Troca de conta do Codex só manual; se o isolamento por pasta não funcionar, a ADR 0022 é revista.
- **O que falta.** O teste de isolamento na máquina do mantenedor; a sonda conferir pelo `codex login status` que cada perfil está no método esperado. Com `file`, `auth.json` fica em texto na pasta do perfil, protegido só pelas permissões da pasta do usuário.

### Pasta de perfil lida ou alterada pelo produto

- **Descrição.** Um coletor de uso ou outro módulo abre arquivo de credencial dentro da pasta do perfil, ou o caminho do perfil chega ao filho por herança ou por `env.set`.
- **Impacto.** Quebra da regra C1; credencial exposta.
- **Dono.** Núcleo (ligação ao perfil) e coletores de uso.
- **Teto.** Lista de subpastas permitidas por ferramenta.
- **Mitigação existente.** O núcleo valida a sobreposição de caminhos por texto. A ligação ao perfil está fora da abertura de ferramentas. O filtro `env.deny` continua barrando `CLAUDE_*` e `CODEX_*`. Coletores que leiam subpastas do perfil dependem de decisão do mantenedor e de exceção explícita na regra C1; até lá, só o comando de status.
- **Plano B.** Remover a ligação ao perfil (condição de reversão da ADR 0022).
- **O que falta.** A pasta do perfil e seus ancestrais ainda podem conter links simbólicos ou junções que contornem a comparação por texto. O núcleo deve conferir esses caminhos no PR que ligar os perfis à abertura, após a validação do isolamento na máquina do mantenedor. Até lá, a abertura segue sem seleção de perfil. Qualquer coletor de perfil exige a decisão prevista na ADR e um teste de arquivo sentinela.

### Chave passa na frente do login do perfil

- **Descrição.** Uma variável de chave (`ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`) ou um `apiKeyHelper` tem precedência sobre o login da pasta, e a sessão roda em outra conta ou é cobrada por uso.
- **Impacto.** Consumo na conta errada ou custo por chave não previsto.
- **Dono.** Adaptadores de ferramenta.
- **Teto.** Não definido.
- **Mitigação existente (parcial).** O filtro do núcleo barra essas variáveis do processo pai ([credencial de CLI](#credencial-de-cli-de-ia-vaza-para-o-processo-filho)).
- **Plano B.** Não abrir sessão no perfil cujo status mostra método diferente do esperado.
- **O que falta.** Configurações gravadas dentro do perfil são da ferramenta e o produto não as lê; só a sonda de status (`authMethod`) pode acusar o problema.

## Repositório e processo

### Instruções de agente sobrepostas ou ignoradas

- **Descrição.** Um agente de terminal não lê o `AGENTS.md`, ou um arquivo local o substitui sem ninguém perceber.
- **Impacto.** Regras deste repositório deixam de valer para quem altera o código.
- **Dono.** Mantenedor.
- **Teto.** Não se aplica.
- **Mitigação existente.** `CLAUDE.md` importa o `AGENTS.md`; os arquivos locais conhecidos estão no `.gitignore`; o que cada ferramenta lê está em [tools.md](tools.md).
- **Plano B.** Hooks e CI cobram as regras de commit independentemente do agente.
- **O que falta.** `AGENTS.override.md`, que substitui o `AGENTS.md` para o Codex na mesma pasta, não está no `.gitignore`. Nenhuma das ferramentas registradas lê `AGENTS.local.md` (ver [tools.md](tools.md)).

### Segredo ou dado pessoal no repositório público

- **Descrição.** Segredo, dado pessoal, nome de empregador ou caminho de máquina entra num commit, issue ou PR.
- **Impacto.** Exposição permanente (todo commit é público).
- **Dono.** Mantenedor e CI.
- **Teto.** Não se aplica.
- **Mitigação existente.** Job `secrets` (gitleaks, configuração em `.gitleaks.toml`) bloqueia no CI; `.gitignore` exclui `.env*`, chaves, bancos e logs; [ADR 0016](adr/0016-sanitized-public-issue-per-delivery.md) para issues.
- **Plano B.** Mover issues para rastreador privado (condição de reversão da ADR 0016).
- **O que falta.** O job `terms` (termos proibidos) só roda com a variável de repositório `TERMS_CHECK = on` e o segredo `FORBIDDEN_TERMS`; hoje a variável não está configurada e a conferência roda só localmente, antes do push.

### Dependência com licença incompatível

- **Descrição.** Uma dependência GPL, AGPL ou outra incompatível com distribuição fechada entra na árvore.
- **Impacto.** Impedimento de distribuir o produto.
- **Dono.** Mantenedor e CI.
- **Teto.** Lista permitida em [`deny.toml`](../deny.toml).
- **Mitigação existente (implementada e testada).** `cargo deny check` no job `rust` (nas duas árvores, com e sem a casca); `npm run licenses` no job `ui`; exceção MPL-2.0 por nome para a árvore do Tauri ([ADR 0018](adr/0018-mpl-2-0-exception-for-the-tauri-dependency-tree.md), proposta).
- **Plano B.** Retirar a casca ou trocar a biblioteca (condição de reversão da ADR 0018).
- **O que falta.** O arquivo de avisos de terceiros do pacote, previsto na ADR 0018, sai com o empacotamento.

### Cadeia de suprimento do CI e das dependências

- **Descrição.** Uma Action ou dependência comprometida executa código no CI ou no produto.
- **Impacto.** Segredo do CI exposto, binário adulterado.
- **Dono.** Mantenedor e CI.
- **Teto.** Não se aplica.
- **Mitigação existente.** Actions fixadas por SHA de commit em `.github/workflows/`; `Cargo.lock` e `package-lock.json` versionados e usados com `--locked` e `npm ci --ignore-scripts`; avisos de segurança pelo `cargo deny`; CodeQL e Scorecard semanais, só avisando ([ADR 0013](adr/0013-only-fast-checks-block.md), `.github/workflows/weekly.yml`).
- **Plano B.** Condição de reversão da ADR 0013 (a análise passa a bloquear).
- **O que falta.** Revisão de dependências por PR. O CI não tem esse job, e o `ci.yml` não traz mais o comentário que o previa; o gatilho que ele citava (existir `Cargo.lock` ou `package-lock.json`) já está cumprido. O limite para o job entrar no `ci-ok` está em [ci-timings.md](ci-timings.md).

### Código `unsafe`

- **Descrição.** Erro de memória em chamada direta ao sistema.
- **Impacto.** Falha ou brecha no host de terminais, no executor de arquivos ou no cofre.
- **Dono.** `ads-ptyhost`, `ads-adapter-fs` e `ads-adapter-vault`.
- **Teto.** Orçamento por crate em [unsafe.md](unsafe.md).
- **Mitigação existente (implementada e testada).** `unsafe_code = "forbid"` no workspace; exceção só em um módulo Windows de cada crate, com cada bloco listado e comentado; o lint reprova a compilação fora deles.
- **Plano B.** Não há.
- **O que falta.** Manter as chamadas nativas do cofre dentro do orçamento documentado em [docs/unsafe.md](unsafe.md).

## Compatibilidade dos contratos da fila contínua

- **Dono.** Núcleo e contratos.
- **Limite.** O título da tarefa aceita marca combinante que não segue uma letra ou um número. O apelido e a origem já são os tipos comuns em `queue.task_created:3` e `record.queue-task:2`; as versões anteriores repetem as mesmas regras, conferidas contra os tipos comuns por `tools/corpus.test.mjs`.
- **Plano.** Exigir que marcas combinantes sigam letra ou número em uma versão futura do registro e da criação. Até lá, preservar a leitura das versões existentes.

## Perda do contexto de admissão da fila

- **Descrição.** A política inicial de uma fila antiga pode se perder se o dono ainda não tinha registrado a política no stream.
- **Impacto.** Sem os dados históricos íntegros, o núcleo pode recusar o replay e a retomada de uma tarefa em execução.
- **Dono.** Núcleo e produtor da fila.
- **Teto.** A fila afetada. Cada nova admissão fica sujeita aos limites vigentes em [queue.toml](../defaults/queue.toml).
- **Mitigação existente.** O replay deriva as mudanças de política dos eventos do dono, usa o `ts` de cada criação e de cada reancoragem coberto pela cadeia HMAC e valida cada admissão com a política da época.
- **Plano B.** Manter a fila parada e a tarefa em `running` sem nova tentativa. Recuperar do backup íntegro o stream e, quando necessária, a política inicial antes de repetir o replay e conferir a sessão no host.
- **O que falta.** Integrar a gravação condicional das mudanças do dono ao produtor conforme o [README do núcleo](../crates/core/README.md#fila-contínua).

## Carimbo adiantado aceito no replay da fila

- **Descrição.** No replay, o relógio de cada criação é o próprio `ts` gravado, então `AheadOfClock` não dispara. Um produtor com a chave da cadeia HMAC, por erro ou de propósito, pode gravar uma criação com carimbo adiantado, e o replay a aceita. A admissão ao vivo recusaria essa criação.
- **Impacto.** A criação adiantada deixa de contar as admissões anteriores na própria janela e passa a ocupar a janela das seguintes. Se o adiantamento passar de `clock_rewind_seconds`, as admissões ao vivo seguintes são recusadas com `BehindLastAdmission`: a fila trava em vez de abrir.
- **Dono.** Produtor da fila.
- **Teto.** `admission_limit` em [queue.toml](../defaults/queue.toml), que não depende de horário, e o teto de tentativas de cada tarefa.
- **Mitigação existente (implementada e testada).** A admissão ao vivo recusa com `AheadOfClock` o `ts` mais de `clock_skew_seconds` adiante do `now` informado pelo produtor. O replay confere a ordem dos carimbos entre si e a âncora. A âncora do dono (`reanchor_admission_clock`) faz as criações cobertas que estão adiante do relógio contarem no instante da âncora. Ver o [README do núcleo](../crates/core/README.md#fila-contínua).
- **Plano B.** O dono reancora a fila para retomar a admissão. Se o histórico estiver adulterado, manter a fila parada e recuperar o stream do backup íntegro antes de repetir o replay.
- **O que falta.** O daemon ainda não chama a admissão ao vivo nem o replay da fila. O replay não detecta o carimbo adiantado sozinho; a detecção vem de `BehindLastAdmission` na admissão seguinte.
