# ads-adapter-claude

Adapta a ferramenta de linha de comando Claude ao contrato do núcleo: transforma um pedido `session.open` (ferramenta, apelido da pasta e tamanho) nos parâmetros de abertura do processo dos terminais (`session.open` do contrato `pty-host/1`).

## Uso

```rust
let config = ClaudeConfig::from_toml_str(&texto_do_arquivo_do_usuario, NameCase::host())?;
let params = open_params(&config, &pedido.params, process_env(), &DiskProbe)?;
```

O adaptador seleciona a tabela `[tools.claude]`. A leitura das camadas, a
validação e a preparação da sessão seguem a
[política de abertura do núcleo](../../core/README.md#abertura-de-ferramentas).

## Testes

`cargo test -p ads-adapter-claude -j 2` executa a suíte compartilhada de abertura
com o identificador desta ferramenta. Ela cobre o ambiente do pai envenenado,
nomes sensíveis em `allow` e `set`, erros sem o conteúdo da entrada e a validação
de caminhos, apelidos, modo e ferramenta.
