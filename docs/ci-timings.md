# Tempos de CI e uso de disco em 14/11/2026

Retrato inicial do workspace, medido na API do GitHub a partir de `startedAt` e `completedAt` de cada job e passo. Os crates estavam em fase inicial, por isso os números refletem sobretudo a instalação da toolchain, o cache e a árvore de dependências. Os jobs atuais estão no [workflow de CI](../.github/workflows/ci.yml).

## Por job

| Job | Runner | Cache frio | Cache quente |
|---|---|---|---|
| `changes` | Ubuntu | 4 a 5 s | igual |
| `commits` | Ubuntu | 8 a 10 s | igual |
| `secrets` | Ubuntu | 5 a 7 s | igual |
| `rust` | Ubuntu | 22 s (sem cache salvo antes) | 25 s (reexecução do mesmo commit) |
| `shell-check` | Windows | 171 s | 64 s e 58 s |
| `pty-windows` | Windows | 113 s (primeira execução, no PR que criou o job) | a medir |
| `fs-windows` | Windows | a medir (no PR que criou o job) | a medir |
| `ci-ok` | Ubuntu | 2 a 3 s | igual |

- Caminho bloqueante quando a casca é tocada: o maior job é o `shell-check`, 171 s frio e cerca de 1 min quente. Sem tocar a casca, o `shell-check` é pulado e o caminho é o `rust` (cerca de 25 s nessa medição).
- Passos do `shell-check` (frio, depois quente): `cargo check -p ads-shell` levou 113 s e depois 14 a 19 s; instalar a toolchain fixada, 12 a 13 s; restaurar o cache, 6 s frio (nada a restaurar) e 18 a 19 s quente; salvar o cache, 27 s só na primeira execução.
- Passos do `rust` (quente): instalar a toolchain fixada, cerca de 10 s (o maior); `cargo-deny`, 3 s; testes, 2 s; formatação e lint, menos de 1 s cada nesta fase.
- Passos do `pty-windows` (frio): lint do `ads-ptyhost`, 29 s (compila `tokio`, `windows` e `portable-pty`); testes, 13 s, dos quais os 20 ciclos de ConPTY levam cerca de 1 s (901 ms no runner, 40 processos de sessão, nenhum remanescente, handles 97 antes e 97 depois); salvar o cache, 41 s só na primeira execução. No mesmo PR o `rust` levou 32 s com as dependências novas (lint 6 s, testes 4 s). O job só roda quando o código do PTY, o `Cargo.lock`, o `Cargo.toml`, a toolchain ou o próprio workflow mudam.
- A primeira execução do `rust` falhou no `cargo-deny` por erro de configuração (chave no lugar errado do `deny.toml`) e, como o cache só é salvo em sucesso, a execução seguinte ainda foi a fria; por isso o "frio" do `rust` vem dela.
- Estimativas anteriores (arquitetura): `rust` 8 a 12 min frio e `shell-check` 20 a 30 min frio. A medição ficou muito abaixo porque os crates tinham pouco código e a casca era um esqueleto. Não revisar a decisão de caminho bloqueante com estes números.

## Disco no PC

| Item | Tamanho |
|---|---|
| Pasta `target` compartilhada depois de `cargo check`, `clippy` e `test` nos crates padrão (casca não compilada) | 64 MB |
| Cache de fontes do Cargo (`registry`) depois de baixar a árvore do Tauri para listar as licenças | 342 MB (era 31 MB) |

Nessa medição, a pasta `target` ficou abaixo do teto de 6 GB.
