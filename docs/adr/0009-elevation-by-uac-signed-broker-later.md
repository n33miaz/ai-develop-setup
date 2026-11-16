# 0009. Elevation by UAC now, signed broker later

Status: aceita em 14/11/2026

## Contexto
Algumas ações de manutenção exigem administrador. O núcleo nunca roda elevado. O objetivo declarado é, no futuro, poder fazer tudo pelo celular com segurança, e ninguém confirma uma janela de elevação à distância.

## Decisão
"(a) UAC por ação agora; corretor com passkey desenhado e desligado, com gatilho".

## Consequências
- Hoje: um auxiliar à parte, com verbos fixos, uma confirmação do sistema por ação; nada elevado fica residente (`elevation.mode = uac_per_action`).
- Desenhado e desligado: um corretor elevado que aceita só uma lista fechada de verbos e confere ele mesmo a assinatura da passkey sobre o hash do plano, com `nonce` e validade, usando chaves públicas que só administrador grava. Um núcleo comprometido não eleva sozinho.
- Gatilho para ligar: passkey e sessão cifrada em vigor e 10 execuções limpas do mesmo verbo por confirmação local.

## Condição de reversão
Voltar a "só mostrar o comando" se o auxiliar elevado apresentar qualquer execução fora da lista de verbos.
