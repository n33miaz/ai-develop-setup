# Matriz de dependências

Gerado por `python tools/check_deps.py write-doc` a partir de `defaults/dependency-matrix.toml`. Não edite à mão: mude a matriz e regenere.

Regra (P2): cada crate só importa os crates internos da coluna "Pode depender de"; o que não está lá é proibido. O teste lê `cargo metadata` e reprova a diferença; o CI roda `python tools/check_deps.py check` em todo PR. Tipos de dependência que contam: `build`, `dev`, `normal`.

| Crate | Pasta | Pode depender de | Quem pode depender dele | Bibliotecas externas proibidas |
|---|---|---|---|---|
| `ads-adapter-antigravity` | `crates/adapters/antigravity` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-adapter-api` | `crates/adapters/api` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-adapter-claude` | `crates/adapters/claude` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-adapter-codex` | `crates/adapters/codex` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-adapter-collectors` | `crates/adapters/collectors` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-adapter-fs` | `crates/adapters/fs` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-adapter-relay` | `crates/adapters/relay` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-adapter-vault` | `crates/adapters/vault` | `ads-contracts`, `ads-core` | `ads-daemon` | nenhuma listada |
| `ads-contracts` | `crates/contracts` | nada interno | `ads-adapter-antigravity`, `ads-adapter-api`, `ads-adapter-claude`, `ads-adapter-codex`, `ads-adapter-collectors`, `ads-adapter-fs`, `ads-adapter-relay`, `ads-adapter-vault`, `ads-core`, `ads-daemon`, `ads-e2e`, `ads-launcher`, `ads-ptyhost`, `ads-shell`, `ads-storage` | nenhuma listada |
| `ads-core` | `crates/core` | `ads-contracts` | `ads-adapter-antigravity`, `ads-adapter-api`, `ads-adapter-claude`, `ads-adapter-codex`, `ads-adapter-collectors`, `ads-adapter-fs`, `ads-adapter-relay`, `ads-adapter-vault`, `ads-daemon`, `ads-launcher` | `axum`, `hyper`, `portable-pty`, `reqwest`, `rusqlite`, `sqlx`, `tokio-tungstenite`, `tungstenite` |
| `ads-daemon` | `crates/daemon` | `ads-adapter-antigravity`, `ads-adapter-api`, `ads-adapter-claude`, `ads-adapter-codex`, `ads-adapter-collectors`, `ads-adapter-fs`, `ads-adapter-relay`, `ads-adapter-vault`, `ads-contracts`, `ads-core`, `ads-e2e`, `ads-ptyhost`, `ads-storage`, `ads-testkit` | ninguém | nenhuma listada |
| `ads-e2e` | `crates/e2e` | `ads-contracts` | `ads-daemon`, `ads-shell` | nenhuma listada |
| `ads-launcher` | `crates/launcher` | `ads-contracts`, `ads-core` | ninguém | nenhuma listada |
| `ads-ptyhost` | `crates/ptyhost` | `ads-contracts`, `ads-testkit` | `ads-daemon` | nenhuma listada |
| `ads-shell` | `crates/shell` | `ads-contracts`, `ads-e2e` | ninguém | nenhuma listada |
| `ads-storage` | `crates/storage` | `ads-contracts` | `ads-daemon` | nenhuma listada |
| `ads-testkit` | `crates/testkit` | nada interno | `ads-daemon`, `ads-ptyhost` | nenhuma listada |

Para permitir uma dependência nova, edite a matriz no mesmo PR que a introduz e justifique no corpo do PR.
