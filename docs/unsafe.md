# Orçamento de `unsafe`

Regra: `unsafe_code = "forbid"` vale para todo o workspace (`[workspace.lints.rust]` no `Cargo.toml` da raiz) e cada crate herda com `[lints] workspace = true`. Hoje há **zero** blocos `unsafe`.

Exceções previstas, ainda não usadas: `ads-ptyhost` e `ads-adapter-vault` vão precisar de chamadas do sistema (ConPTY, Job Object, cofre de credenciais). Quando o primeiro bloco entrar, o mesmo PR troca a herança desses dois crates por uma tabela `[lints]` própria com `unsafe_code = "deny"` e lista cada bloco abaixo, com o motivo. A contagem só sobe com justificativa aqui.

| Crate | Blocos `unsafe` | Motivo |
|---|---|---|
| todos | 0 | nenhum |
