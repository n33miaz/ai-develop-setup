# Arquivos de instruções por ferramenta

O que cada ferramenta de IA de terminal lê como instruções do projeto, e se aceita um arquivo local que some ou substitua o [AGENTS.md](../AGENTS.md). Registro feito a partir da documentação oficial de cada ferramenta, com a data da consulta. Nada aqui vem de teste próprio nem de fonte de terceiros; o que a documentação oficial não diz está marcado **não verificado**.

Atualize a linha quando a ferramenta mudar de comportamento ou quando a documentação for consultada de novo.

## Resumo

| Ferramenta | Versão | Lê `AGENTS.md` | Lê `CLAUDE.md` | Arquivo local (fora do git) | Soma ou substitui | Importa outro arquivo | Fonte (consulta em 16/11/2026) |
|---|---|---|---|---|---|---|---|
| Claude Code | v2.1.277 ou posterior para ler `AGENTS.md` direto; demais itens sem versão citada | sim, só quando não há `CLAUDE.md` nem `CLAUDE.local.md` (padrão), ou pelo import | sim | `CLAUDE.local.md`; `.claude/settings.local.json` | soma: lido depois do `CLAUDE.md` da mesma pasta | sim, `@caminho` | [Memória do Claude Code](https://code.claude.com/docs/en/memory) |
| Codex | não verificado (a página não cita versão) | sim | não (só se listado em `project_doc_fallback_filenames`) | `AGENTS.override.md` | substitui: no máximo um arquivo por pasta, e o override vem antes do `AGENTS.md` | não documentado | [AGENTS.md no Codex](https://learn.chatgpt.com/docs/agent-configuration/agents-md) |
| Antigravity | não verificado (a página não cita versão) | sim | não citado | não documentado | soma: todas as regras encontradas são combinadas | não documentado | [Regras do Antigravity](https://antigravity.google/docs/rules) |

Nenhuma das três lê `AGENTS.local.md`: o Claude Code diz isso explicitamente; Codex e Antigravity não o citam entre os nomes que procuram.

## Claude Code

- **Projeto:** `./CLAUDE.md` ou `./.claude/CLAUDE.md`, nesta pasta e em todas as de cima, carregados na partida; os de subpastas carregam quando o agente mexe num arquivo delas. Também há regras em `.claude/rules/`.
- **Pessoal:** `~/.claude/CLAUDE.md` e `~/.claude/rules/`; da organização, um `CLAUDE.md` gerenciado em pasta do sistema.
- **Local:** `CLAUDE.local.md` na raiz do projeto, "carregado junto com o `CLAUDE.md` e tratado do mesmo jeito". Em cada pasta ele vem depois do `CLAUDE.md`. Os arquivos são concatenados, nunca sobrepostos. Configuração local em `.claude/settings.local.json`.
- **`AGENTS.md`:** por padrão é lido só quando não existe `CLAUDE.md`, `.claude/CLAUDE.md` nem `CLAUDE.local.md` nesta pasta ou acima. Neste repositório o `CLAUDE.md` existe e importa o `AGENTS.md` (`@AGENTS.md`), então o `AGENTS.md` é lido pelo import. Uma configuração (`claude-md-and-agents-md`) lê os dois juntos.
- **Não lidos:** `AGENTS.local.md`, `AGENTS.override.md` e o que está em `.agents/`.
- **Import:** `@caminho`, relativo ao arquivo que importa, até quatro níveis. Import que sai da pasta de trabalho pede aprovação uma vez por projeto.
- **Consequência aqui.** `CLAUDE.local.md` soma às regras deste repositório; não precisa repetir o `AGENTS.md`. Criar um `CLAUDE.local.md` não desliga o `AGENTS.md` neste repositório, porque ele chega pelo import do `CLAUDE.md`.

## Codex

- **Global:** na pasta do Codex (`~/.codex` ou `CODEX_HOME`), usa `AGENTS.override.md` se existir, senão `AGENTS.md`; só o primeiro não vazio.
- **Projeto:** percorre da raiz do projeto até a pasta atual e "inclui no máximo um arquivo por pasta", procurando nesta ordem: `AGENTS.override.md`, `AGENTS.md`, e os nomes de `project_doc_fallback_filenames`. Outros nomes são ignorados.
- **Junção:** concatena da raiz para baixo; o que está mais perto da pasta atual vem depois e prevalece.
- **Limite:** `project_doc_max_bytes`, padrão de 32 KiB somados; passando disso, os arquivos seguintes não entram.
- **Consequência aqui.** Um `AGENTS.override.md` na raiz **substitui** este `AGENTS.md` para o Codex: as regras deste arquivo deixam de ser lidas. Como não há import documentado, um override precisa copiar as regras, e a cópia envelhece. Prefira um `AGENTS.md` numa subpasta ou o arquivo global. `AGENTS.override.md` hoje não está no `.gitignore`.
- **Não verificado:** a versão mínima do Codex para esse comportamento; qualquer forma de import.

## Antigravity

- **Projeto:** `AGENTS.md` ou `GEMINI.md`, `.agents/AGENTS.md` ou `.agents/GEMINI.md` e `.agents/rules/*.md` (o nome antigo `.agent/rules/` ainda vale), na pasta de trabalho e nas de cima até a raiz do repositório. Em `.agents/rules/` só os `.md` do primeiro nível. Regras de subpastas abaixo da pasta de trabalho não são carregadas.
- **Global:** `~/.gemini/AGENTS.md`, `~/.gemini/GEMINI.md`; no aplicativo e na CLI também `~/.gemini/config/AGENTS.md` e `~/.gemini/config/GEMINI.md`; regras modulares em `~/.gemini/config/rules/*.md`, com cabeçalho YAML.
- **Junção:** "as regras são cumulativas, não de substituição": tudo o que é encontrado entra no prompt. A página não define precedência entre arquivos.
- **CLI (`agy`):** lê a partir da pasta onde foi iniciada até a raiz do repositório; não aceita menção manual de regra.
- **Não verificado:** versão; leitura de `CLAUDE.md`; existência de arquivo local fora do git; forma de import; o que vale quando duas regras se contradizem.

## Regra para quem cria um arquivo local

- Claude Code: use `CLAUDE.local.md`; ele soma.
- Codex: não use `AGENTS.override.md` na raiz deste repositório. Se precisar, ponha as preferências pessoais no arquivo global da pasta do Codex.
- Antigravity: nenhum arquivo local de projeto fora do git está documentado. Regra pessoal vai na pasta `~/.gemini` e soma ao `AGENTS.md`, sem substituí-lo.
- Ferramenta que não está nesta página: não presuma que o arquivo local soma. Comece o arquivo importando o `AGENTS.md` de forma explícita, como manda o [AGENTS.md](../AGENTS.md).
