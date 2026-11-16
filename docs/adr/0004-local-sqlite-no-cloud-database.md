# 0004. Local SQLite, no cloud database for now

Status: aceita em 14/11/2026 (AR-04)

## Contexto
O dado do produto nasce e é usado em um único computador. Bancos gerenciados grátis foram avaliados; nenhum resolve um problema atual, e todos colocariam transcrições e auditoria em um terceiro.

## Decisão
"(a) SQLite no PC e nenhum banco de nuvem agora", com a observação: "depois ampliamos para a opção b" (caixa de mensagens na Cloudflare quando o relé existir).

## Consequências
- Três bancos SQLite locais, cada um com um dono: estado, auditoria só-anexo e métricas. O [ADR 0021](0021-storage-atomic-file-and-cipher.md) propõe reunir invariantes em um arquivo para preservar atomicidade em WAL.
- Regra de divisão: arquivo versionado guarda o que o humano decide; banco guarda o que a máquina gera.
- Nenhuma dependência de serviço de banco externo nem disputa de limites grátis com outros projetos.
- A ampliação prevista usa o armazenamento do próprio relé (ADR 0008), só com texto cifrado.

## Condição de reversão
Reabrir se surgir um segundo computador executor que precise do mesmo estado, ou se o backup local não cumprir uma restauração testada.
