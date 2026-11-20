# 0018. MPL-2.0 exception for the Tauri dependency tree

Status: proposta em 14/11/2026, aguarda aceite do mantenedor

Em vigor no código, pendente de ratificação pelo mantenedor.

## Contexto
A licença do produto é proprietária, com código aberto à vista, e só aceita dependências MIT, Apache-2.0, BSD, ISC, Zlib e Unicode; MPL-2.0 só com exceção registrada (ver `AGENTS.md` e `deny.toml`). O primeiro `cargo metadata` da árvore do Tauri 2.12.5 para `x86_64-pc-windows-msvc` listou 239 bibliotecas externas. Todas cabem na lista permitida, escolhendo-se a alternativa permitida nas licenças duplas, **exceto cinco, todas MPL-2.0**:

| Crate | Versão | Quem o traz |
|---|---|---|
| `cssparser` | 0.37.0 | `dom_query` e `selectors`, usados por `tauri-utils` |
| `cssparser-macros` | 0.7.1 | `cssparser` |
| `dtoa-short` | 0.3.5 | `cssparser` |
| `selectors` | 0.38.0 | `dom_query`, usado por `tauri-utils` |
| `option-ext` | 0.2.0 | `dirs-sys`, usado por `dirs`, que `tauri` e `tauri-build` usam para achar pastas do usuário |

Os cinco entram só pela casca (`ads-shell`); nenhum outro crate do workspace os alcança.

## Decisão
Liberar MPL-2.0 **apenas** para esses cinco crates, por nome, em `[[licenses.exceptions]]` do `deny.toml`. MPL-2.0 continua fora da lista geral: qualquer outro crate com essa licença reprova o `cargo-deny`.

## Consequências
- A MPL-2.0 tem copyleft por arquivo: obriga a manter aberto o código dos arquivos da própria biblioteca que forem modificados, não o do programa que a usa. Usamos as bibliotecas sem modificá-las, como dependência do `crates.io`, então nada do código do produto fica sujeito à licença.
- Para distribuir o binário da casca, o texto da licença e o aviso de cada uma entram no arquivo de avisos de terceiros do pacote (tarefa da entrega de empacotamento).
- Nova versão ou novo crate MPL-2.0 na árvore reprova o portão e volta a este registro.
- Nenhum desses crates é usado pelo núcleo, pelo `ptyhost` nem pelo `launcher`; a regra de dependência impede que a casca vire dependência deles.

## Condição de reversão
Revogar a exceção se o mantenedor decidir não aceitar MPL-2.0 em nenhuma forma (a casca então sai da árvore ou troca de biblioteca), se for preciso modificar um desses crates (a modificação teria de ser publicada, o que pede nova decisão), ou se uma versão futura do Tauri deixar de trazê-los (remover a exceção correspondente).
