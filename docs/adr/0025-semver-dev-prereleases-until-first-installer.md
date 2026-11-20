# 0025. SemVer dev pre-releases until the first installer

Status: aceita em 20/11/2026. Substitui a [0017](0017-date-and-commit-versioning-during-express.md).

## Contexto
A ADR 0017 decidiu identificar cada build por `dev-<data>-<commit curto>`, publicada como pré-lançamento, sem lista de mudanças, até sair do período expresso. Nada disso foi implementado: o repositório não tinha tag nem release, e os manifestos seguem em `0.0.0`.

O formato `dev-<data>-<commit>` fica fora do versionamento semântico (SemVer 2.0.0), que é o que ferramentas, gerenciadores de pacote e pessoas esperam ao comparar versões. Ele teria de ser convertido depois. Os PRs já seguem títulos descritivos e os commits seguem Conventional Commits, então as notas geradas pelo GitHub a partir dos PRs saem legíveis sem ferramenta nova.

## Decisão
- A cada junção na `develop` com o CI verde, o workflow `prerelease.yml` publica um pré-lançamento em SemVer, `<base>-dev.<AAAAMMDD>.<N>+<commit curto>`; por exemplo `0.1.0-dev.20261120.3+abc1234`. `N` recomeça em 1 a cada dia (UTC) e cresce a cada publicação do dia.
- A tag é a mesma versão sem os metadados de build: `v0.1.0-dev.20261120.3`. O `+commit` fica no título e no texto da release, porque o `+` atrapalha links e ferramentas que leem tags. Pelo SemVer, os metadados de build não contam na ordem das versões, então nada se perde.
- A release é marcada como pré-lançamento e nunca como a mais recente. As notas são geradas pelo GitHub a partir dos PRs juntados desde o pré-lançamento anterior.
- Ficam guardados os dez pré-lançamentos mais novos do canal. A poda apaga a release e a tag dos mais antigos, só das que casam com o padrão do canal e estão marcadas como pré-lançamento. Ela nunca apaga a recém-publicada e confere o resultado depois de apagar.
- A `0.1.0` estável sai com o primeiro instalador entregue a outra pessoa. Só então a `main` volta a receber a `develop`. Até lá, a `main` parada é uma exceção consciente à convenção de lançar pela branch principal: o produto ainda não foi lançado.
- Tudo é parâmetro em `defaults/release.toml`: ligar ou desligar, versão base, canal, prefixo da tag, quantos guardar e modo da poda (`apply`, `dry-run`, `off`). A numeração e o plano de poda saem de `tools/release.py`, testado em `tools/test_release.py`.

## Consequências
- Ainda não há o que baixar: enquanto o CI não gerar o programa ([ADR 0015](0015-ci-built-artifact-with-verified-self-update.md)), cada pré-lançamento é só tag e notas. Quando houver binário, ele leva a versão completa, com o `+commit`, e o endereço de saúde do núcleo informa o commit, que é o que o atualizador confere (ADR 0015).
- Os manifestos (`Cargo.toml`, `package.json`) continuam em `0.0.0` até a build do CI injetar a versão. A fonte da versão é `defaults/release.toml`.
- Junções muito próximas podem ganhar um pré-lançamento só, o do commit mais novo: quem chega com a `develop` já adiante não publica.
- A primeira release gera notas desde o início do histórico, porque não há tag anterior.

## Condição de reversão
Na `0.1.0`, passar a lançamentos estáveis por versão (com a `main` recebendo a `develop` a cada versão) e rever se os pré-lançamentos continuam a cada junção. Desligar com `enabled = "off"` se as publicações atrapalharem o fluxo.
