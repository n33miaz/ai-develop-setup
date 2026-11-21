# Política de segurança

## Como relatar uma vulnerabilidade

Use o canal privado do GitHub: na aba **Security** deste repositório, escolha **Report a vulnerability** (relato privado de vulnerabilidade). O relato fica visível só para o mantenedor.

**Não abra issue pública** para vulnerabilidade e não cole segredos, tokens, dados pessoais ou dados de terceiros em nenhum relato.

Ajuda ter no relato: o que foi observado, passos para reproduzir, versão ou commit afetado e o impacto que você imagina.

## O que esperar

O projeto está em construção, sem versão estável. O suporte é o melhor esforço do mantenedor, sem prazo contratual, e vale apenas para o estado atual da branch `develop`. Quando houver correção, o relato é fechado com o commit que a traz.

## Escopo

Vale para o código e a configuração deste repositório. Falhas em ferramentas de terceiros que o projeto apenas chama devem ser relatadas aos responsáveis por elas.

## Superfície e modelo de ameaças

O modelo considera páginas no navegador, programas locais sem o token por execução, outras contas na máquina e adulteração do banco local.
A API em loopback verifica `Host`, `Origin` e o token por execução.
No Windows, o pipe é restrito ao usuário e o cliente confere quem serve o canal.
Na janela desktop, a porta e o token chegam à casca por um pipe anônimo herdado do lançador; a casca exige que o daemon prove conhecer o token antes de enviá-lo e repassa mensagens por IPC só à janela principal ([arquitetura](docs/architecture.md)).
A pasta de controle do lançador e a pasta da auditoria são privadas da conta, e sinais de outra conta são ignorados.
A abertura de ferramentas filtra o ambiente do processo filho.
O log de eventos usa cifra autenticada e encadeamento por HMAC. A chave ainda não vem do cofre.
O administrador da máquina fica fora do modelo. O que já existe e o que falta contra programas da mesma conta do usuário está nos riscos.
Veja as mitigações e limites em [Riscos e mitigações](docs/risks.md).

## Se você achar um segredo no repositório

Relate pelo mesmo canal privado. Não o copie nem o repasse.
