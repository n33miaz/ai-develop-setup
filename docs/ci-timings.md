# Tempos de CI e uso de disco

Tempos medidos na API do GitHub a partir de `startedAt` e `completedAt` de cada job. Os jobs atuais estão no [workflow de CI](../.github/workflows/ci.yml); o alvo do modo expresso é chegar ao `ci-ok` em até 5 minutos.

## Por job, em 20/11/2026

Base: as dez últimas execuções verdes de push na `develop` (de 21:00 a 22:31 UTC) e as oito últimas execuções verdes de PR. Os jobs condicionais só rodam quando uma entrada deles muda, por isso a coluna de execuções diz em quantas das dez cada um rodou.

| Job | Runner | Execuções (de 10) | Duração |
|---|---|---|---|
| `changes` | Ubuntu | 10 | 4 a 10 s |
| `secrets` | Ubuntu | 10 | 8 a 18 s |
| `commits` | Ubuntu | só em PR | 41 a 59 s (nos 8 PRs) |
| `terms` | Ubuntu | 0 (desligado) | não roda |
| `contracts` | Ubuntu | 3 | 24 a 35 s |
| `ui` | Ubuntu | 3 | 66 a 73 s |
| `rust` | Ubuntu | 10 | 114 a 178 s |
| `shell-check` | Windows | 6 | 54 a 92 s |
| `pty-windows` | Windows | 10 | 115 a 226 s |
| `fs-windows` | Windows | 8 | 78 a 142 s |
| `storage-windows` | Windows | 7 | 73 a 135 s |
| `vault-windows` | Windows | 8 | 68 a 158 s |
| `launcher-windows` | Windows | 5 | 82 a 152 s |
| `ci-ok` | Ubuntu | 10 | 2 a 5 s |

- Do início da execução ao fim do `ci-ok`: de 148 a 242 s nos pushes da `develop` (cerca de 3 min no meio da faixa) e de 148 a 195 s nos PRs. O alvo de 5 minutos se cumpre, com cerca de 1 minuto de folga no pior caso medido.
- Caminho crítico: o `pty-windows`, que tem o filtro mais amplo (núcleo, daemon, armazenamento, contratos e adaptadores), rodou nas dez e foi o job mais longo em 9 das 10 execuções de push, empatado com o `rust` numa delas. Na outra, o mais longo foi o `launcher-windows`. O `rust` vem logo atrás, de 114 a 178 s.
- Os jobs Windows rodam em paralelo; somar os tempos não dá o tempo da execução.

## Retrato inicial, em 14/11/2026

Os crates estavam em fase inicial, por isso os números refletem sobretudo a instalação da toolchain, o cache e a árvore de dependências.

| Job | Runner | Cache frio | Cache quente |
|---|---|---|---|
| `changes` | Ubuntu | 4 a 5 s | igual |
| `commits` | Ubuntu | 8 a 10 s | igual |
| `secrets` | Ubuntu | 5 a 7 s | igual |
| `rust` | Ubuntu | 22 s (sem cache salvo antes) | 25 s (reexecução do mesmo commit) |
| `shell-check` | Windows | 171 s | 64 s e 58 s |
| `pty-windows` | Windows | 113 s (primeira execução, no PR que criou o job) | não medido nesse dia |
| `ci-ok` | Ubuntu | 2 a 3 s | igual |

- Caminho bloqueante quando a casca é tocada: o maior job é o `shell-check`, 171 s frio e cerca de 1 min quente. Sem tocar a casca, o `shell-check` é pulado e o caminho é o `rust` (cerca de 25 s nessa medição).
- Passos do `shell-check` (frio, depois quente): `cargo check -p ads-shell` levou 113 s e depois 14 a 19 s; instalar a toolchain fixada, 12 a 13 s; restaurar o cache, 6 s frio (nada a restaurar) e 18 a 19 s quente; salvar o cache, 27 s só na primeira execução.
- Passos do `rust` (quente): instalar a toolchain fixada, cerca de 10 s (o maior); `cargo-deny`, 3 s; testes, 2 s; formatação e lint, menos de 1 s cada nesta fase.
- Passos do `pty-windows` (frio): lint do `ads-ptyhost`, 29 s (compila `tokio`, `windows` e `portable-pty`); testes, 13 s, dos quais os 20 ciclos de ConPTY levam cerca de 1 s (901 ms no runner, 40 processos de sessão, nenhum remanescente, handles 97 antes e 97 depois); salvar o cache, 41 s só na primeira execução. No mesmo PR o `rust` levou 32 s com as dependências novas (lint 6 s, testes 4 s).
- A primeira execução do `rust` falhou no `cargo-deny` por erro de configuração (chave no lugar errado do `deny.toml`) e, como o cache só é salvo em sucesso, a execução seguinte ainda foi a fria; por isso o "frio" do `rust` vem dela.
- Estimativas anteriores (arquitetura): `rust` 8 a 12 min frio e `shell-check` 20 a 30 min frio. A medição ficou muito abaixo porque os crates tinham pouco código e a casca era um esqueleto. Não revisar a decisão de caminho bloqueante com estes números.

## Filtros, cache e verificações agendadas

- Os filtros dos jobs Windows cobrem os crates que cada job usa, suas dependências transitivas no workspace e os arquivos de origem dos contratos. O teste Python compara os filtros com o grafo produzido por `cargo metadata --no-deps`.
- Só execuções do mesmo PR se substituem. Cada push na `develop` tem seu próprio grupo de concorrência.
- O cache Rust usa uma chave por sistema no Linux e uma por job no Windows, para cada job gravar as próprias dependências; as diferenças de ambiente ficam no hash. A casca mantém seu hash próprio por causa de `RUSTFLAGS`. Só a `develop` salva cache. PRs apenas restauram.
- O `pty-windows` testa a biblioteca do núcleo, o ambiente do processo filho e os adaptadores. Os testes de propriedade do portão rodam no Linux.
- O workflow semanal verifica avisos do RustSec com `cargo deny` nos mesmos alvos do CI e roda `npm audit --audit-level=high` em `ui/` e `contracts/`, incluindo as dependências de desenvolvimento. Os checks completos do `cargo deny` continuam no job `rust`.

Ao medir de novo, registre a data, quantas execuções entraram, o caminho até o `ci-ok` e os acertos de cache. A revisão de dependências por PR, quando existir, só entra no `ci-ok` se levar menos de um minuto.

## Disco no PC

| Item | Tamanho |
|---|---|
| Pasta `target` compartilhada depois de `cargo check`, `clippy` e `test` nos crates padrão (casca não compilada) | 64 MB |
| Cache de fontes do Cargo (`registry`) depois de baixar a árvore do Tauri para listar as licenças | 342 MB (era 31 MB) |

Nessa medição, a pasta `target` ficou abaixo do teto de 6 GB.
