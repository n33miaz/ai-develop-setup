# 0026. Operating system vault for product secrets

Status: proposta em 20/11/2026, em vigor no código, aguardando ratificação do mantenedor

## Contexto

A [ADR 0005](0005-column-level-encryption.md) reserva ao cofre do sistema os segredos que protegem dados locais. O núcleo precisa de um contrato independente da plataforma para acessar somente os nomes reservados pelo produto.

## Decisão

Usar credenciais genéricas do Gerenciador de Credenciais no Windows, com persistência local para a conta do usuário atual. O adaptador usa o crate `windows` do workspace e operações por alvo exato. A proteção do Windows é por conta e máquina. Processos da mesma conta também podem acessar o cofre. [Documentação da estrutura CREDENTIALW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw).

O contrato reserva nomes fixos e nomes de âncora derivados da identidade do banco, buffers que são zerados ao sair e erros sem conteúdo do segredo. Plataformas sem adaptador retornam erro explícito. O daemon injeta a âncora por identidade no log persistente pelo contrato de armazenamento. A entrada `audit-database`
fixa a identidade por usuário. A entrada `audit-anchor-<id>` guarda o compromisso e o
resumo do caminho normalizado. A fixação encerra o uso da entrada legada `audit-anchor`,
inclusive quando o dono confirma uma nova identidade ou um novo caminho. A [API e a recuperação da âncora](../../crates/storage/README.md#âncora-no-cofre) descrevem como SQLite e cofre são conferidos.

## Alternativas

- Uma biblioteca multiplataforma reduziria o código nativo próprio, mas exigiria validar o serviço e as dependências de cada plataforma atendida.
- Um arquivo protegido pelo sistema acrescentaria ao produto a responsabilidade pelo ciclo de vida do arquivo e pela recuperação. Arquivos e variáveis de ambiente em claro não atendem à fronteira definida na ADR 0005.

## Condição de reversão

Reavaliar o backend se o cofre ficar indisponível em sessões Windows que o produto precise atender, ou se os limites de persistência ou tamanho impedirem guardar os registros previstos. Qualquer troca com dados existentes exige migração com restauração exercitada e preservação da leitura das chaves antigas durante a transição.
