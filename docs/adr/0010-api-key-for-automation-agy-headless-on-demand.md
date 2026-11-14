# 0010. API key for automation, headless agy only on demand

Status: aceita em 14/11/2026 (AR-10)

## Contexto
O uso automatizado da CLI Antigravity por aplicativo de terceiros não é coberto pelos termos do fornecedor, com risco de suspensão da conta. O mesmo modelo é alcançável por chave de API.

## Decisão
"(a) Automático usa a chave de API; 'agy -p' só quando você dispara, depois do aceite gravado".

## Consequências
- Dois adaptadores: um de API (chave no cofre do produto) usado pelo roteador e pelo revisor; um da CLI, interativo por padrão.
- O modo sem interface da CLI fica atrás de `adapters.agy.headless = false` e só liga depois de um aceite de risco registrado por escrito pelo usuário.
- Rotação automática de contas continua proibida.
- O uso por chave pode ser cobrado ou limitado pelo plano do provedor; entra no livro-caixa como custo faturável.

## Condição de reversão
Rever se o fornecedor publicar permissão explícita para uso automatizado da CLI, ou retirar a CLI do produto se houver aviso de suspensão.
