# ads-ptyhost

Processo estável que segura as sessões de terminal e o anel de replay.

## Peças

| Arquivo | O que faz |
|---|---|
| `src/types.rs` | Tipos internos da sessão (`SessionId`, `Seq`, `OutputChunk`, `SpawnSpec`, erros). É o único ponto a trocar quando o contrato `pty-host/1` existir. |
| `src/backend.rs` | Traço `PtyBackend`: a costura entre a sessão e quem fornece o terminal. |
| `src/portable.rs` | Backend real, sobre `portable-pty` (ConPTY no Windows). Só este arquivo cita a biblioteca. |
| `src/fake.rs` | Backend falso, determinístico e sem processo real, para os testes. |
| `src/win.rs` | Chamadas do Windows: o Job Object que encerra a árvore de processos e as sondas de processo e de handle. Todo `unsafe` do crate fica aqui. |
| `src/replay.rs` | Anel de replay: pedaços de saída numerados por `seq`, limitado em bytes. |
| `src/session.rs`, `src/host.rs` | Sessão (bombeia a saída para o anel e para os assinantes) e o conjunto de sessões abertas. |

Os parâmetros e seus padrões estão em [`defaults/ptyhost.toml`](../../defaults/ptyhost.toml).

## Regras do replay

- Cada leitura do terminal vira um pedaço com `seq` crescente a partir de 1; o número nunca se repete.
- O cliente assina "a partir do seq N" (inclusive) e recebe primeiro o que o anel guarda e depois a saída ao vivo, em ordem, sem buraco e sem duplicata.
- Pedido anterior ao que o anel ainda guarda, ou à frente do que já foi numerado, é recusado com erro que informa o limite; nada é pulado em silêncio.
- Assinante lento, que enche a própria fila, tem a assinatura encerrada; ele reconecta a partir do último `seq` que viu e não perde nada (enquanto o anel alcançar).
- O anel continua disponível depois que o processo termina, até a sessão ser fechada.

## Limites conhecidos

- No Windows o processo raiz entra no Job Object logo depois de criado, não antes de começar a rodar. Um neto iniciado nesse instante ficaria fora do job. Fechar essa janela pede criar o processo suspenso, o que a biblioteca não expõe; é o que o plano B (ConPTY direto) resolve.
- O ConPTY abre perguntando a posição do cursor e segura o processo filho até a resposta. Como pode não haver cliente conectado, o backend responde "canto superior esquerdo" ao abrir. A pergunta continua na saída; um cliente que também responda manda uma resposta repetida ao programa.
- Fora do Windows só o processo raiz recebe o sinal de encerramento; o resto da árvore depende do fechamento do terminal. O produto roda no Windows; o caminho Unix existe para os testes do CI em Linux.
- O binário ainda não serve o canal local entre os processos: isso entra com o contrato `pty-host/1`.
