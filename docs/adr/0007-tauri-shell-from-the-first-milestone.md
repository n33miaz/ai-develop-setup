# 0007. Tauri shell from the first milestone

Status: aceita em 14/11/2026 (AR-07)

## Contexto
A interface funciona no navegador servida pelo núcleo. A casca de desktop acrescenta janela própria, ícone na bandeja e painel suspenso, ao custo de mais tempo de build.

## Decisão
"(b) Tauri desde o primeiro marco".

## Consequências
- A casca existe desde o marco inicial, ainda vazia, para o custo de build aparecer cedo.
- Ela é cliente fino: carrega o mesmo bundle da PWA e fala com o núcleo pelo mesmo contrato WebSocket em loopback. Não contém política, banco nem PTY, e a regra de dependência impede que passe a conter.
- Fechar, travar ou atualizar a casca não encerra sessão (ADR 0003).
- A casca compila no CI; no PC do mantenedor a interface é desenvolvida no navegador.
- Job próprio em runner Windows, que só roda quando a casca muda.

## Condição de reversão
Adiar a casca se o job dela mantiver a mediana do caminho bloqueante acima de 10 minutos depois dos cortes previstos na ADR 0013.
