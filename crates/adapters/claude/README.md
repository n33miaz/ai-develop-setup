# ads-adapter-claude

Adapta a ferramenta de linha de comando Claude ao contrato do núcleo: transforma um pedido `session.open` (ferramenta, apelido da pasta e tamanho) nos parâmetros de abertura do processo dos terminais (`session.open` do contrato `pty-host/1`).

## Uso

```rust
let config = ClaudeConfig::from_toml_str(&texto_do_arquivo_do_usuario, NameCase::host())?;
let params = open_params(&config, &pedido.params, process_env(), &DiskProbe)?;
```

Os parâmetros vêm de [`defaults/harnesses.toml`](../../../defaults/harnesses.toml) com o arquivo do usuário por cima (fora do repositório). O arquivo explica cada chave: `[env]`, `[workdirs]` e `[tools.claude]` (`program`, `args`, `mode`). O adaptador lê só a tabela `tools.claude`; as tabelas de outras ferramentas ficam para os adaptadores delas.

## O que o adaptador garante

- Quem pede a sessão escolhe só a ferramenta (pelo nome), o apelido da pasta e o tamanho. Programa, argumentos, ambiente e pasta vêm da configuração; o contrato do pedido recusa esses campos e a função não tem por onde recebê-los.
- Programa: caminho absoluto e literal da configuração (sem caminho relativo, variável, `~`, `..`, caminho de rede nem nome curto do Windows), que precisa ser um arquivo existente quando a sessão abre. Vazio (o padrão) desliga a ferramenta.
- Argumentos: só os fixos da configuração, na ordem dela.
- Pasta: o apelido precisa estar em `[workdirs]` (comparação exata) e a pasta precisa existir. A pasta vai sempre preenchida, para o processo dos terminais nunca cair na pasta atual dele.
- Ambiente do filho: exatamente os nomes de `env.allow` que o processo pai tem, mais os valores de `env.set` (`TERM`, `COLORTERM`). Nomes que casam com `env.deny` (`*_API_KEY`, `*_TOKEN`, `*_SECRET`, `*_PASSWORD`, `*_KEY`, `GH_TOKEN`, `GITHUB_TOKEN`, `ANTHROPIC_*`, `OPENAI_*`, `AWS_*`, `AZURE_*`) nunca passam, mesmo que alguém os ponha em `allow`; a camada do usuário só acrescenta padrões a `deny`. A política é do núcleo (`ads_core::child_env`), para todo adaptador de ferramenta usar a mesma.
- Erros não carregam caminho, valor nem nome de variável.

## O que o adaptador não garante

- O que a ferramenta faz depois de aberta. O agente dentro do terminal lê e edita arquivos conforme as permissões dele, na pasta escolhida e fora dela; o portão de ações do produto não cobre isso.
- O login da ferramenta: fica com ela, fora do produto. Se a ferramenta precisa de uma variável para achar o próprio login, ela tem de estar em `allow` e não pode casar com `deny`.
- Que o programa seja o certo: o adaptador confere que o arquivo existe, não quem o assinou.
- Programas `.cmd` e `.bat` (instalações por npm) rodam pelo `cmd.exe`, com as regras de aspas dele. Como os argumentos vêm só da configuração, isso fica documentado, não tratado.
- O arquivo pode sumir entre a conferência e a abertura; aí quem falha é a abertura no processo dos terminais.

## Testes

`cargo test -p ads-adapter-claude` e `cargo test -p ads-core --test child_env`: ambiente do pai envenenado (chaves de provedores, `GH_TOKEN`, `*_TOKEN`, `*_SECRET`, `AWS_*`, com variações de caixa), inclusive com esses nomes na lista permitida e com `deny = []` na camada do usuário; ferramenta desconhecida, caminho relativo, caminho inexistente, programa vazio, apelido desconhecido e modo `headless` recusados.
