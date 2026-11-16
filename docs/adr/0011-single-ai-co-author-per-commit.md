# 0011. Single AI co-author per commit

Status: aceita em 14/11/2026

## Contexto
Os commits levam atribuição de IA, sem corpo descritivo. Mais de um agente pode trabalhar na mesma entrega, e uma das ferramentas pode inserir a própria linha de atribuição por conta própria.

## Decisão
"(a) Um coautor por commit: quem escreveu a maior parte; a linha automática do Codex é barrada".

## Consequências
- Mensagem: assunto em inglês de até 50 caracteres, linha em branco e trailers; nenhum parágrafo de corpo.
- O hook `commit-msg` é a única fonte do trailer `Co-authored-by`, a partir da chave de perfil `ai_attribution`; qualquer linha de atribuição inserida por ferramenta é recusada pelo hook e pelo CI.
- No máximo um coautor de IA por commit; o orquestrador informa o executor ao hook.
- Desligar a atribuição é trocar a chave para `none`, sem reescrever histórico.

## Condição de reversão
O mantenedor avisou que vai pedir o fim da atribuição; nesse momento a chave muda e esta ADR é substituída.
