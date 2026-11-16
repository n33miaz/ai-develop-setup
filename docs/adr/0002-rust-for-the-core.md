# 0002. Rust for the core

Status: aceita em 14/11/2026

## Contexto
O núcleo executa CLIs de IA em terminal virtual, guarda auditoria e roda como usuário comum. Havia um critério escrito para comparar Rust e .NET 10 por spikes lado a lado, e uma recomendação de .NET pela facilidade de leitura do mantenedor.

## Decisão
"(c) Rust agora, sem spike comparativo".

## Consequências
- Workspace Cargo com um crate por componente; regras de dependência cobradas no CI.
- A casca Tauri e o núcleo ficam na mesma linguagem: duas ferramentas de build (Rust e Node) em vez de três.
- O mantenedor não lê Rust. A auditoria passa a ser por comportamento: contratos e arquivos dourados, matriz de dependências, relatório de comportamento por PR, revisão por quem não escreveu o código, orçamento de `unsafe`, demonstração a cada marco.
- Os limites do critério de spikes continuam valendo como aceite de marco: 20 ciclos limpos de PTY com fixture e com as CLIs reais, distribuição até 120 MB, partida até 1.500 ms, memória em repouso até 250 MB, ambiente do processo filho por lista permitida, login das CLIs intacto.
- Compilação pesada: release só no CI; no PC do mantenedor, uma pasta `target` compartilhada, limite de jobs e apenas `cargo check` e testes do crate tocado.
- O caminho bloqueante do CI deixa de caber em 5 minutos em todos os casos (ADR 0013).

## Condição de reversão
Reverter para .NET 10 se, depois de dois lotes e do plano B (ConPTY direto), o PTY não fechar 20 de 20 no Windows, ou se o mantenedor declarar que a auditoria por comportamento não basta. Os contratos, a interface e os adaptadores por subprocesso sobrevivem à troca.
