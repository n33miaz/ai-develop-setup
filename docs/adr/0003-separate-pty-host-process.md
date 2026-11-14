# 0003. Separate PTY host process

Status: aceita em 14/11/2026 (AR-03)

## Contexto
A branch de desenvolvimento é a produção, então o programa instalado é atualizado várias vezes por semana. Se as sessões de terminal vivessem no processo do núcleo, cada atualização encerraria o trabalho em andamento.

## Decisão
"(a) Dois processos desde a primeira sessão: 'pty-host' estável + núcleo que reinicia".

## Consequências
- `ptyhost` segura os terminais e o anel de replay; o núcleo reinicia e retoma pelo número de sequência.
- Contrato `pty-host/1` entre os dois, por canal local do usuário.
- Nem o núcleo nem a casca são pais do `ptyhost`: quem o inicia é o lançador, destacado.
- O `ptyhost` só é trocado com zero sessões ou com aceite explícito de encerrá-las; mudança nele leva rótulo próprio no PR.
- Aceite: núcleo derrubado 20 vezes no meio de uma sessão sem perder um byte.

## Condição de reversão
Unir os processos só se o contrato entre eles se mostrar a principal fonte de defeitos por dois marcos seguidos, aceitando que atualização passa a encerrar sessões.
