# 0022. Account profile folders and automatic account switching

Status: aceita em 17/11/2026 quanto ao objetivo (troca automática de conta) e à ordem (primeiro validar o isolamento por pasta de perfil). O modo automático só liga depois que o teste de isolamento descrito abaixo passar na máquina do mantenedor. Substitui em parte a [ADR 0010](0010-api-key-for-automation-agy-headless-on-demand.md) e esclarece o texto da regra C1 do [AGENTS.md](../../AGENTS.md).

## Contexto

O mantenedor usa mais de uma conta de assinatura por ferramenta de IA de terminal e quer que o produto passe para outra conta quando a cota de uma acaba. A ADR 0010 diz "Rotação automática de contas continua proibida", e a regra C1 proíbe ler, copiar, mover, registrar ou alterar o login de qualquer CLI de IA.

As próprias ferramentas oferecem um caminho que não toca no login: uma pasta de configuração por conta, escolhida por variável de ambiente, em que a pessoa faz o login uma vez pela própria ferramenta.

O que a documentação oficial diz (consulta em 19/11/2026):

- **Claude Code.** "To stay signed in to multiple accounts at once [...] give each account its own configuration directory. When you start `claude`, set the `CLAUDE_CONFIG_DIR` environment variable to the directory for the account you want to use. Each directory has its own settings, session history, and claude.ai login or API key." No Windows o login fica em `.credentials.json` dentro dessa pasta. Exceção explícita: "Separate directories don't keep two Claude Console sign-ins without an API key apart", e o login "Console sem chave" "signs you out of any claude.ai login stored on the machine". Variáveis de chave (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`) e `apiKeyHelper` têm precedência sobre o login de `/login`. `claude auth status` mostra o estado em JSON (ou texto com `--text`), sai com 0 se logado e 1 se não, e traz `configDirectory` (desde a v2.1.268) e `authMethod` (`none`, `claude.ai`, `oauth_token`, `api_key`, `api_key_helper`, `third_party`). Fontes: [autenticação](https://code.claude.com/docs/en/authentication), [variáveis de ambiente](https://code.claude.com/docs/en/env-vars), [referência da CLI](https://code.claude.com/docs/en/cli-reference).
- **Codex.** "Codex stores its local state under `CODEX_HOME` (defaults to `~/.codex`)": `config.toml`, `auth.json` (se o armazenamento for em arquivo) ou o chaveiro do sistema, histórico e caches. `cli_auth_credentials_store` aceita `file | keyring | auto | ephemeral`; com `file`, as credenciais ficam em `auth.json` sob `CODEX_HOME`; `keyring` usa o cofre do sistema; `auto` usa o cofre quando existe e cai para o arquivo. `codex login status` mostra o método ativo. A opção `--profile` do Codex é só uma camada de configuração (`<nome>.config.toml`): não isola conta. Fontes: [autenticação](https://learn.chatgpt.com/docs/auth), [configuração avançada](https://learn.chatgpt.com/codex/config-file/config-advanced), [referência de configuração](https://learn.chatgpt.com/codex/config-file/config-reference).
- **Antigravity.** O FAQ diz que "wrapping `agy` inside third-party coding agents or applications, or automating account rotation" "is not supported under consumer plans", e que usar software de terceiros para acessar o Antigravity viola os termos e pode levar à suspensão da conta. Fonte: [FAQ do Antigravity](https://antigravity.google/docs/faq).

## Decisão

1. **Perfil é uma pasta.** Um perfil é uma pasta isolada por conta e por ferramenta, com caminho absoluto vindo da configuração do usuário (fora do repositório), por exemplo `[accounts.claude.<apelido>] profile_dir = "<caminho absoluto>"`. Apelido segue a regra dos apelidos de pasta de trabalho e nunca é e-mail.
2. **Login fora do produto.** O mantenedor faz o login de cada perfil uma vez, pela própria ferramenta, fora do produto. O produto nunca executa login nem logout, nunca cria, copia, move ou apaga material de autenticação e nunca abre, lista nem confere arquivos dentro da pasta do perfil.
3. **O produto só escolhe a pasta.** A variável documentada da ferramenta (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`) é uma constante do adaptador, não um parâmetro de ambiente. Ela entra no ambiente do filho **depois** do filtro de ambiente do núcleo, por um tipo próprio de ligação ao perfil, com o valor que vem de `profile_dir`. Ela nunca é herdada do processo pai e nunca passa por `env.set`: os padrões `CLAUDE_*` e `CODEX_*` continuam em `env.deny` em `defaults/harnesses.toml`, e é isso que garante que nenhum outro caminho a defina.
4. **Observação.** Por enquanto o produto observa a conta só pelo comando de status. Um segundo caminho fica condicionado:
   - o comando de status da ferramenta (`claude auth status`, `codex login status`), com uma lista de campos permitidos (estado logado, método, provedor, tipo de plano, pasta de configuração usada). E-mail, organização e qualquer identificador de conta ficam fora da lista;
   - coletores de uso que leem só subpastas permitidas do perfil (por exemplo históricos de sessão), nunca arquivo de credencial, com teste de arquivo sentinela que reprova se um coletor tocar fora da lista. Este caminho só vale depois de uma decisão do mantenedor sobre de onde vem a medição de consumo e de uma exceção explícita no texto da regra C1, que hoje proíbe abrir ou listar arquivos dentro da pasta do perfil.
5. **Troca nunca no meio da sessão.** Uma sessão aberta fica no perfil em que abriu até acabar. Trocar de conta é abrir a próxima sessão em outro perfil.
6. **Política parametrizada (P8).** `switch = "manual" | "propose" | "auto"` por ferramenta, com `switch_at_pct` (percentual da cota observada que dispara a troca). Padrão recomendado: `propose` (o produto sugere a troca e o mantenedor confirma) até o isolamento estar validado e a medição de cota existir. `auto` é o objetivo declarado e só liga depois disso.
7. **Antigravity fica manual.** Para o Antigravity o valor é fixo em `manual`, sem chave para mudar, porque o fornecedor declara a rotação automática fora dos termos. Mudar isso pede uma decisão específica do mantenedor com aceite de risco por escrito.
8. **Validar antes.** Antes de qualquer código de troca, o mantenedor confere na máquina dele, com pastas de teste, que dois perfis ficam em contas diferentes e que o login de um não desfaz o do outro, no Claude Code e no Codex.

### Decisão do mantenedor e o que acontece se o perfil não funcionar

Resposta do mantenedor, em 17/11/2026: "vamos testar a questão do perfil para a troca, mas anote que se não der certo isso, quebraremos a regra e faremos o que for para deixar a troca automatica".

Fica registrado: o objetivo é a troca **automática** de conta. Primeiro valida-se o isolamento por pasta de perfil. Se o isolamento não funcionar em alguma ferramenta, esta ADR será revista para permitir o que for necessário para a troca ser automática, inclusive mudar o texto da regra C1, com nova ADR que diga exatamente o que passa a ser permitido e com os riscos dessa escolha.

## Substituição parcial da ADR 0010

Esta ADR substitui a ADR 0010 **só** no item "Rotação automática de contas continua proibida". Passa a valer: troca automática de conta entre perfis de pasta, para Claude Code e Codex, nas condições acima; para o Antigravity, a troca continua manual. O restante da ADR 0010 continua valendo: automação pela chave de API, modo sem interface do `agy` atrás de `adapters.agy.headless = false` e de aceite registrado.

## Esclarecimento do texto da regra C1

A regra C1 continua proibindo ler, copiar, mover, registrar em log ou alterar o login de qualquer CLI de IA. Fica explícito no [AGENTS.md](../../AGENTS.md) que o produto pode definir a variável documentada de pasta de configuração de uma ferramenta, com caminho absoluto vindo da configuração do produto; nunca a herda, nunca abre, lista nem confere arquivos dentro dela e nunca executa login ou logout. Escolher qual pasta a ferramenta usa não lê nem altera o login: quem lê e grava o login é a própria ferramenta, na pasta que o mantenedor já preparou.

## Consequências

- O código atual veta isso de propósito e continua correto até a implementação: `defaults/harnesses.toml` nega `CLAUDE_*` e `CODEX_*`, `crates/core/src/child_env.rs` descarta nomes negados mesmo em `env.set`, e o [README do núcleo](../../crates/core/README.md#abertura-de-ferramentas) documenta o filtro das variáveis que redirecionam o login. A implementação acrescenta a ligação ao perfil depois do filtro, sem afrouxar `env.deny`, e atualiza esses textos no mesmo PR.
- Os contratos já trazem a troca de conta: `routing.proposed:2`, `routing.decided:2` e `routing.fallback:2` têm `account_alias` em cada rota, `account.switched:1` registra a seleção do perfil para a próxima sessão e `account.status_observed:1` registra os metadados permitidos do status. A `routing.fallback:1` continua como está e não tem campo de conta (a descrição do Schema dela diz "Nunca troca de conta"). Um plano B pode mudar de perfil só entre perfis configurados e só na próxima sessão, e a mudança de perfil da ferramenta de destino também gera `account.switched:1`, como diz o [README dos contratos](../../contracts/README.md#eventos-de-conta). Nenhuma regra do validador confere isso, e o núcleo ainda emite só a `routing.fallback:1`.
- Os contratos `record.usage_event` e os de roteamento já usam `account_alias`, que nunca é credencial nem e-mail; o mesmo apelido identifica o perfil.
- Um teste com duas pastas de perfil e uma variável de chave envenenada no processo pai prova que só a pasta escolhida chega ao filho e que nenhuma variável de chave passa.
- `claude auth status` permite conferir, sem abrir nada, que a ferramenta usou a pasta esperada (`configDirectory`) e que nenhuma chave passou na frente do login (`authMethod` diferente de `api_key`, `oauth_token` e `api_key_helper`).
- Claude Desktop com pasta de perfil não está verificado e fica fora desta ADR.

## Riscos

Detalhes, dono, teto e plano B em [risks.md](../risks.md#contas-por-pasta-de-perfil).

- **Login "Console sem chave" do Claude desloga todos.** Esse tipo de login fica fora da pasta de configuração e desfaz todos os logins claude.ai da máquina. O roteiro de preparo dos perfis proíbe esse login; o status observado (`authMethod`) mostra quando um perfil não está no método esperado.
- **Antigravity.** O fornecedor declara que automatizar a rotação viola os termos. Fica manual até decisão específica.
- **Codex com chaveiro possivelmente compartilhado.** A documentação oficial não diz qual é o padrão de `cli_auth_credentials_store` nem se a entrada do chaveiro depende de `CODEX_HOME`. O isolamento só é dado como confirmado com `cli_auth_credentials_store = "file"` em cada perfil e depois do teste na máquina. Com `file`, `auth.json` fica em texto na pasta do perfil, protegido só pelas permissões da pasta do usuário.
- **Chave que passa na frente do perfil.** Uma variável de chave no ambiente ou nas configurações do perfil faz a ferramenta usar a chave em vez do login da pasta. O filtro do núcleo barra as variáveis do processo pai; as configurações gravadas dentro do perfil são da ferramenta e aparecem no status.
- **Termos de uso ambíguos.** Claude e Codex documentam várias contas por pasta, mas não dizem se a troca automática entre contas de assinatura de uma mesma pessoa é aceita. Há risco de suspensão. O padrão `propose` deixa a decisão de cada troca com o mantenedor até ele escolher `auto`.

## Condição de reversão

- Se o teste de isolamento falhar numa ferramenta, a troca automática por pasta não liga para ela e esta ADR é revista conforme a decisão do mantenedor acima.
- Voltar para troca manual em todas as ferramentas se qualquer fornecedor avisar suspensão, publicar termos contra a troca entre contas, ou se o teste de status antes e depois mostrar um login alterado pelo produto.
- Remover a ligação ao perfil se uma revisão de segurança mostrar o produto lendo arquivo de dentro da pasta fora das subpastas permitidas.
