# Tempos de CI e uso de disco

Tempos medidos na API do GitHub a partir de `startedAt` e `completedAt` de cada job. Os jobs atuais estão no [workflow de CI](../.github/workflows/ci.yml); o alvo do modo expresso é chegar ao `ci-ok` em até 5 minutos.

## Por job, em 21/11/2026

Base: as dez últimas execuções verdes de push na `develop` (de 01:40 a 06:41 UTC) e as dez últimas execuções verdes de PR (de 04:14 a 06:41 UTC), todas depois de o `shell-check` passar a abrir a janela real da casca contra o daemon. Desde então o `shell-check` roda `npm ci` e o build da interface, `cargo test -p ads-shell`, o build de três binários e a janela, e o filtro dele cobre núcleo, armazenamento, daemon, `ptyhost`, launcher, adaptadores, contratos e `ui/`, por isso roda em quase todo push. Pushes: 44851523707, 44862461568, 44862500685, 44863460609, 44869483759, 44870851856, 44872189037, 44872626453, 44876734654 e 44876740145. PRs: 44865077281, 44869221173, 44869893703, 44870504811, 44872263915, 44872414330, 44873191388, 44874211791, 44874507501 e 44876782432. Os jobs condicionais só rodam quando uma entrada deles muda; a coluna de execuções diz em quantas das dez de push cada um rodou.

| Job | Runner | Execuções (de 10) | Duração |
|---|---|---|---|
| `changes` | Ubuntu | 10 | 4 a 12 s |
| `secrets` | Ubuntu | 10 | 7 a 16 s |
| `commits` | Ubuntu | só em PR | 46 a 61 s (nos 10 PRs) |
| `terms` | Ubuntu | 0 (desligado) | não roda |
| `contracts` | Ubuntu | 4 | 22 a 28 s |
| `ui` | Ubuntu | 3 | 69 a 95 s |
| `rust` | Ubuntu | 10 | 105 a 184 s |
| `shell-check` | Windows | 8 | 135 a 243 s |
| `pty-windows` | Windows | 7 | 154 a 238 s |
| `fs-windows` | Windows | 6 | 73 a 161 s |
| `storage-windows` | Windows | 7 | 81 a 161 s |
| `vault-windows` | Windows | 6 | 66 a 115 s |
| `launcher-windows` | Windows | 7 | 76 a 139 s |
| `ci-ok` | Ubuntu | 10 | 2 a 5 s |

- Do início da execução ao fim do `ci-ok`: de 138 a 260 s nos pushes da `develop` (mediana de cerca de 214 s) e de 136 a 263 s nos PRs (mediana de cerca de 225 s). Nessas vinte execuções o alvo de 5 minutos se cumpre, com cerca de 40 s de folga no pior caso, contra cerca de 1 minuto antes da janela.
- O alvo não se cumpriu em 2 das 12 execuções verdes de push desde a entrada da janela. Na 44847882720 (o push que a trouxe), 329 s: quem segurou o `ci-ok` foi o `pty-windows`, que começou 96 s depois do início da execução (90 s depois do fim do `changes`, sua única dependência) e terminou aos 323 s. O `shell-check` terminou antes, aos 284 s, com o cache Rust restaurado só por chave parcial e sem cache do npm. Na 44851035577, 360 s: o próprio `changes` só começou 100 s depois do início da execução, o `shell-check` começou 85 s depois do fim do `changes` e levou 167 s, e terminou por último. Os horários mostram esses intervalos, mas não registram a causa; a espera por runners é a explicação provável e fica fora do controle do repositório.
- Caminho crítico: o `shell-check` e o `pty-windows` predominam, o `pty-windows` mais vezes no push e o `shell-check` mais vezes no PR. O último job a terminar antes do `ci-ok` foi o `pty-windows` em 5 dos 10 pushes, o `shell-check` em 3 e o `rust` em 2 (os dois sem jobs Windows); nos PRs, o `shell-check` em 6 de 10, o `pty-windows` em 3 e o `rust` em 1. As faixas de duração dos dois jobs Windows se sobrepõem quase inteiras.
- Passos do `shell-check` nas 17 execuções em que ele rodou (8 pushes e 9 PRs): compilar a interface (`npm ci` e build), 41 a 68 s; testes da casca, 14 a 44 s; build dos três binários do teste de conexão, 22 a 46 s; abrir a janela e autenticar, 7 a 12 s; restaurar o cache Rust, 18 a 27 s; instalar a toolchain, 10 a 14 s; Node, 7 a 16 s. O cache do npm veio pela chave primária nas 17; o cache Rust, pela chave primária em 11 e só por chave parcial em 6 (3 pushes e 3 PRs). A restauração parcial não alongou os passos de forma visível: a variação entre execuções com chave primária já é maior (a interface foi de 41 a 68 s).
- Passos do `pty-windows` nas 14 execuções em que ele rodou (7 pushes e 7 PRs): lint, 24 a 41 s; testes do `ptyhost`, 21 a 42 s; testes do cliente do daemon, 27 a 56 s; parâmetros de abertura das ferramentas, 21 a 39 s; restaurar o cache Rust, 14 a 25 s. O cache Rust veio pela chave primária em 11 e só por chave parcial em 3 (2 pushes e 1 PR).
- Salvar o cache Rust: só acontece em push na `develop` quando a chave primária não existia (restauração parcial). Foi o caso em 3 dos 8 pushes com `shell-check`, somando de 21 a 28 s ao fim do job, e em 2 dos 7 pushes com `pty-windows`, somando 29 e 33 s. Com acerto na chave primária não há salvamento.
- Por isso push e PR não se comparam direto: parte dos pushes paga o salvamento do cache, e o PR paga o job `commits` (que roda em paralelo). Num PR (44873191388) o `ci-ok` só começou 47 s depois do fim do último job (o `pty-windows`, aos 213 s); de novo, os horários mostram o intervalo, sem registrar a causa.
- Os jobs Windows rodam em paralelo; somar os tempos não dá o tempo da execução.

### Lint da casca

Desde o fim da amostra acima, o `shell-check` roda `cargo clippy -p ads-shell --all-targets` antes dos testes da casca, em série com os demais passos. Nas seis primeiras execuções verdes com o passo (pushes 44900418240 e 44902766285; PRs 44900034328, 44901649324, 44902756290 e 44902832474), o lint levou de 48 a 69 s, o segundo passo mais longo do job; só passou a compilação da interface (56 a 65 s) em 1 das 6. O `shell-check` inteiro levou de 219 a 267 s, contra 135 a 243 s na amostra acima. Do início da execução ao fim do `ci-ok`, de 233 a 282 s; o `shell-check` foi o último job a terminar em 5 das 6, e a folga para o alvo de 5 minutos caiu para cerca de 18 s no pior caso.

### Opções para recuperar folga (não aplicadas)

- Rodar a janela real e o build dos três binários (de 29 a 58 s na amostra) só quando mudam `crates/shell/`, `ui/` ou `tools/smoke_desktop.py`, mantendo o filtro amplo para o build da interface e os testes da casca. O teste de conexão também depende do daemon e do `ptyhost`: com o filtro estreito, uma mudança neles que quebre a conexão passa sem a janela. O ganho no `ci-ok` só existe quando o `shell-check` é o último a terminar, e para no fim da próxima dependência do `ci-ok`. Tirando esses passos de cada uma das 17 execuções da amostra com `shell-check`, o ganho seria zero em 5 dos 8 pushes e em 3 dos 9 PRs, e de 5 a 58 s nas demais.
- Dividir o `shell-check` em dois jobs Windows paralelos (testes da casca; interface, binários e janela). Cada um paga de novo a preparação (cerca de 1 minuto entre toolchain, cache e Node), e são mais runners Windows disputando a fila.
- Evitar compilar a interface no `shell-check` quando `ui/` não mudou, reaproveitando uma compilação anterior. Exige guardar o resultado entre execuções e conferir que ele corresponde ao commit.
- Encurtar o `pty-windows` junto: sem isso, os cortes no `shell-check` deixam o `pty-windows` como caminho crítico. Nos 14 jobs da amostra ele durou de 138 a 238 s, perto de 4 minutos no pior caso; essa é a duração do job, sem a espera para começar nem o `ci-ok`. Com 90 s de espera, como na 44847882720, só o `pty-windows` já levou a execução a 329 s.

## Antes da janela, em 20/11/2026

Base: as dez últimas execuções verdes de push na `develop` (de 21:00 a 22:31 UTC) e as oito últimas execuções verdes de PR, antes de o `shell-check` abrir a janela real. A coluna de execuções diz em quantas das dez cada job rodou.

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

- Do início da execução ao fim do `ci-ok`: de 148 a 242 s nos pushes da `develop` e de 148 a 195 s nos PRs.
- O job mais longo foi o `pty-windows`, que então tinha o filtro mais amplo, em 9 das 10 execuções de push (empatado com o `rust` numa delas); na outra, o `launcher-windows`.

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
