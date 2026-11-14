# 0008. Relay Worker as the first remote transport

Status: aceita em 14/11/2026 (AR-08)

## Contexto
O acesso remoto por túnel exige um domínio próprio. As alternativas sem domínio eram uma rede privada com aplicativo em cada aparelho ou um relé por Worker em endereço `workers.dev`.

## Decisão
"(c) Adiantar o relé por Worker em endereço workers.dev".

## Consequências
- Um Worker na conta do próprio usuário serve a PWA e um ponto de encontro WebSocket; o computador e o celular abrem conexões de saída até ele.
- O relé guarda só chaves públicas e mensagens cifradas com validade; nunca texto em claro nem credencial de IA.
- Cifra de ponta a ponta por biblioteca de protocolo Noise; a passkey é conferida pelo núcleo, dentro do canal cifrado; comandos assinados pelo aparelho.
- A caixa de mensagens prevista na ADR 0004 vive no armazenamento do próprio relé.
- Desligado por padrão (`remote.transport = none`); nada é publicado sem ação explícita do usuário. Túnel e rede privada continuam como valores do mesmo parâmetro.
- O projeto nunca opera relé compartilhado: cada usuário usa a própria conta.
- A passkey fica ligada ao endereço do Worker; trocar o endereço exige reinscrever aparelhos.

## Condição de reversão
Trocar de transporte se o uso normal de um dia passar de 80% de qualquer limite grátis, se os termos do provedor mudarem contra esse uso, ou se a inspeção do armazenamento do relé encontrar qualquer dado em claro.
