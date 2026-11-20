# 0025. SemVer dev pre-releases until the first installer

Status: aceita em 20/11/2026. Substitui a [0017](0017-date-and-commit-versioning-during-express.md).

## Contexto
A ADR 0017 decidiu identificar cada build por `dev-<data>-<commit curto>`, publicada como pré-lançamento, sem lista de mudanças, até sair do período expresso. Nada disso foi implementado: o repositório não tinha tag nem release, e os manifestos seguem em `0.0.0`.

O formato `dev-<data>-<commit>` fica fora do versionamento semântico (SemVer 2.0.0), que é o que ferramentas, gerenciadores de pacote e pessoas esperam ao comparar versões. Ele teria de ser convertido depois. Os PRs já seguem títulos descritivos e os commits seguem Conventional Commits, então as notas geradas pelo GitHub a partir dos PRs saem legíveis sem ferramenta nova.

## Decisão
- Quando o CI de um push na `develop` termina verde (o que exige o `ci-ok`), o workflow `prerelease.yml` publica um pré-lançamento em SemVer, `<base>-dev.<AAAAMMDD>.<N>+<commit curto>`; por exemplo `0.1.0-dev.20261120.3+abc1234`. `N` conta as publicações do dia (UTC) na mesma versão base e recomeça em 1 no dia seguinte.
- A tag é a mesma versão sem os metadados de build: `v0.1.0-dev.20261120.3`. O `+commit` fica no título e no texto da release, porque o `+` atrapalha links e ferramentas que leem tags. Pelo SemVer, os metadados de build não contam na ordem das versões, então nada se perde.
- A release é marcada como pré-lançamento e nunca como a mais recente. As notas são geradas pelo GitHub a partir dos PRs juntados desde o pré-lançamento anterior que está na história do commit. Depois de criar, o workflow confere que a tag aponta para o commit avaliado.
- Ficam guardados os dez pré-lançamentos publicados mais novos do canal, na ordem de precedência do SemVer. A poda segue a regra de ação destrutiva do projeto:
  - cópia: a lista de cada tag a apagar com o seu commit fica no resumo da execução, o que basta para recriá-la;
  - ensaio: o plano é calculado e mostrado antes de qualquer exclusão, e `prune = "dry-run"` para aí;
  - confirmação: a política "guardar dez" foi aprovada pelo mantenedor e está em `defaults/release.toml`; cada alvo é conferido de novo antes de sair (release ainda pré-lançamento e não rascunho; tag sem release ligada);
  - nova conferência: o inventário completo é relido e não pode sobrar nada a podar.
  Só entram releases e tags que casam com o padrão do canal; rascunhos, releases estáveis e outros canais nunca. A recém-publicada tem de estar entre as mantidas. A poda roda em toda execução, então uma interrompida no meio termina na seguinte.
- A `0.1.0` estável sai com o primeiro instalador entregue a outra pessoa. Só então a `main` volta a receber a `develop`. Até lá, a `main` parada é uma exceção consciente à convenção de lançar pela branch principal: o produto ainda não foi lançado.
- Tudo é parâmetro em `defaults/release.toml`: ligar ou desligar, versão base, canal, prefixo da tag, quantos guardar e modo da poda (`apply`, `dry-run`, `off`). A numeração e o plano de poda saem de `tools/release.py`, testado em `tools/test_release.py`.

## Consequências
- Ainda não há o que baixar: enquanto o CI não gerar o programa ([ADR 0015](0015-ci-built-artifact-with-verified-self-update.md)), cada pré-lançamento é só tag e notas. Quando houver binário, ele leva a versão completa, com o `+commit`, e o endereço de saúde do núcleo informa o commit, que é o que o atualizador confere (ADR 0015).
- Os manifestos (`Cargo.toml`, `package.json`) continuam em `0.0.0` até a build do CI injetar a versão. A fonte da versão é `defaults/release.toml`.
- Junções muito próximas podem ganhar um pré-lançamento só, o do commit mais novo: quem chega com a `develop` já adiante não publica. Se o CI desse commit mais novo falhar, a próxima junção verde publica, e as notas dela cobrem também os PRs que ficaram sem pré-lançamento.
- O primeiro pré-lançamento do canal não tem tag anterior; o GitHub escolhe o ponto de partida das notas, o que hoje é o início do histórico.
- Trocar a versão base no meio de um dia recomeça `N`; a ordem de retenção segue o SemVer, então baixar a versão base faz os pré-lançamentos novos ficarem atrás dos antigos. Versão base só sobe.

## Condição de reversão
Na `0.1.0`, passar a lançamentos estáveis por versão (com a `main` recebendo a `develop` a cada versão) e rever se os pré-lançamentos continuam a cada junção. Desligar com `enabled = "off"` se as publicações atrapalharem o fluxo.
