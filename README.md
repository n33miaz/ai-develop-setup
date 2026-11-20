# ai-develop-setup

[![CI](https://github.com/n33miaz/ai-develop-setup/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/n33miaz/ai-develop-setup/actions/workflows/ci.yml?query=branch%3Adevelop) [![Licença: direitos reservados](https://img.shields.io/badge/licen%C3%A7a-direitos%20reservados-blue)](LICENSE)

Codinome de uma plataforma local para delegar trabalho de desenvolvimento a agentes de IA: um daemon em Rust, uma casca Tauri e uma interface em React 19 + Vite + TypeScript. O nome comercial vem depois.

## Estado

Em construção, sem versão estável: cada junção na `develop` publica um pré-lançamento de desenvolvimento ([ADR 0025](docs/adr/0025-semver-dev-prereleases-until-first-installer.md)). Já existem os contratos e os tipos gerados deles, o portão de ações destrutivas, o executor de arquivos, o host de terminais com o cliente dele no daemon, a abertura do banco local com a cifra por coluna, a API local autenticada servida pelo daemon e a interface com o cliente dela; o estado de cada módulo está em [docs/modules.md](docs/modules.md) e os riscos conhecidos em [docs/risks.md](docs/risks.md).

## Pré-requisitos

O fluxo completo de terminais usa ConPTY no Windows 10/11. O modo de exemplo é executado no navegador, e o CI também roda a suíte Rust padrão no Ubuntu.

| Ferramenta | Versão ou requisito | Referência no repositório |
|---|---|---|
| Rust via rustup | Toolchain 1.96.0, mínimo 1.96 | [rust-toolchain.toml](rust-toolchain.toml), [Cargo.toml](Cargo.toml) |
| Build Tools do MSVC | Ferramentas C++ para compilar o SQLite embutido pelo recurso `bundled` de `rusqlite` | [Cargo.toml](Cargo.toml) |
| Node.js com npm | Mínimo 24, usado pelo CI de contratos. O desenvolvimento da interface e o CI dela usam a versão de [ui/.node-version](ui/.node-version) | [ui/package.json](ui/package.json), [contracts/package.json](contracts/package.json), [CI](.github/workflows/ci.yml) |
| Python | 3, para as verificações do repositório | [tools/check_deps.py](tools/check_deps.py), [CI](.github/workflows/ci.yml) |
| Bash | Git Bash no Windows para o hook e o script de commit | [.githooks/commit-msg](.githooks/commit-msg), [tools/commit-as-agent.sh](tools/commit-as-agent.sh) |
| Ferramenta de linha de comando de IA | Opcional para a demonstração. Instale uma das ferramentas compatíveis para abrir um terminal real | [README do daemon](crates/daemon/README.md) |

## Ver em 2 minutos

Na raiz do repositório, execute:

```bash
cd ui
npm ci
npm run dev
```

Abra `http://127.0.0.1:5173` em uma aba nova para explorar a lista de sessões e o terminal com dados de exemplo, sem iniciar o daemon.

## Rodar completo

Siga o [README do daemon](crates/daemon/README.md) para abrir uma ferramenta em um terminal real pelo navegador no Windows.

## Rodar os testes

Comandos das suítes do [CI](.github/workflows/ci.yml):

| Pasta | Comandos |
|---|---|
| Raiz | `cargo test --locked` e `python tools/check_deps.py check` |
| `contracts/` | `npm ci --ignore-scripts` e `npm test` |
| `ui/` | `npm ci --ignore-scripts` e `npm test` |

## Mapa de documentos

- [Arquitetura local](docs/architecture.md) — processos, canais e fronteiras de confiança.
- [Cliente da interface](ui/README.md) — conexão, terminal e instalação como aplicativo.
- [Decisões de arquitetura](docs/adr/README.md) — decisões e seus estados.
- [Regras do repositório](AGENTS.md) e [guia de contribuição](CONTRIBUTING.md) — regras para alterações e fluxo de branches.
- [Política de segurança](SECURITY.md) — escopo e relato de vulnerabilidades.

## Licença

Código aberto à vista, direitos reservados: o código pode ser lido, estudado e avaliado, mas usar, copiar, modificar ou distribuir exige licença comercial escrita. Não é software livre nem open source. Texto completo, com a versão em português que prevalece, em [LICENSE](LICENSE).
