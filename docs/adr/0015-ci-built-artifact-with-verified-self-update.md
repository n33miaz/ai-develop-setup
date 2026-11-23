# 0015. CI-built artifact with verified self-update

Status: aceita em 14/11/2026

## Contexto
Para a develop ser a produção, o código juntado precisa virar o programa instalado. O computador do mantenedor tem pouca memória e disco e não deve compilar release. Não há certificado de assinatura de código. Outro projeto do autor já tem um mecanismo de atualização em uso.

## Decisão
"(a) O CI gera o programa a cada junção; o PC baixa, confere e troca, com volta automática", com a observação: "veja como o [outro projeto do autor] faz as atualizações e adote (o que envolve também)" (o nome do projeto foi omitido da citação).

## Consequências
- O lançador permanece independente de `ads-core` para que alterações no núcleo possam ser distribuídas sem recompilar o supervisor. Ambos usam `ads-child-env` para aplicar as regras de ambiente de [harnesses.toml](../../defaults/harnesses.toml). Como a atualização preserva o binário do lançador, mudanças nas regras embutidas nele entram em vigor quando a instalação substitui esse binário.
- Adotado como está lá: lançador estável fora do que é trocado, um por instalação, com recuperação de troca interrompida; supervisor com espera crescente; candidata validada antes de entrar; versão anterior guardada; prova de 10 minutos com volta automática em três quedas e lista de versões bloqueadas; pedidos de "atualizar", "reiniciar" e "voltar" por arquivo-sinal; estado da atualização visível; ensaio de atualização como portão; no máximo duas trocas por dia em horário de uso.
- Adaptado: binários prontos do CI em vez de código-fonte; pastas por versão com um ponteiro trocado de forma atômica; autoteste do binário; manifesto com SHA-256 de cada arquivo, assinado no CI e conferido com chave pública embutida no lançador; troca do núcleo e da casca sem tocar no `ptyhost`; o lançador exige que o endereço de saúde informe o commit esperado em 60 segundos.
- O atualizador é o lançador, não o plugin da casca, porque o plugin não tem volta automática e exige fechar o aplicativo.
- A chave de assinatura do manifesto é gerada pelo mantenedor e guardada como segredo do CI.
- Atualizar o `ptyhost` encerra sessões e só acontece com zero sessões ou aceite explícito.

## Condição de reversão
Passar a build local se o serviço de CI ficar indisponível por mais de 48 horas; suspender a atualização automática se uma versão ruim passar pela prova de 10 minutos duas vezes.
