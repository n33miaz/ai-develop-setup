# 0023. Remote session encryption

Status: proposta, aguardando decisão do mantenedor (biblioteca, uso da mesma biblioteca no celular por WASM e conta do Worker)

## Contexto

A [ADR 0008](0008-relay-worker-as-first-remote-transport.md) escolheu um relé por Worker, na conta do próprio usuário, como primeiro transporte remoto, e fixou o princípio: cifra de ponta a ponta por protocolo Noise, passkey conferida pelo núcleo dentro do canal cifrado e comandos assinados pelo aparelho. Os contratos já existem: `urn:ads:relay:envelope:1` (o relé só carrega texto cifrado com validade), `urn:ads:relay:device:1` (o relé só guarda identificador opaco e chave pública de 32 bytes) e `urn:ads:command:request:1` (comando remoto exige `device_id`, assinatura de 64 bytes, `nonce`, `counter` e `expires_at`). A API local recusa `origin: "remote"` ([ADR 0020](0020-local-api-authentication.md)).

Falta decidir como o canal é montado, como um aparelho entra e sai, e que bibliotecas fazem isso no PC e no celular.

## Decisão (proposta)

1. **Canal.** Padrão Noise entre o núcleo e o aparelho, com chaves estáticas X25519 dos dois lados, AEAD ChaCha20-Poly1305 e hash BLAKE2s ou SHA-256. Na inscrição, o padrão `XX` (nenhum lado conhece o outro ainda); depois, `IK` (o aparelho já conhece a chave estática do núcleo). O relé só vê os quadros do handshake e os envelopes cifrados.
2. **Pareamento por QR.** O PC mostra um QR com o endereço do relé, a chave pública estática do núcleo e um código de uso único com validade curta (padrão proposto: 5 minutos). O celular lê o QR, abre o handshake e manda o código dentro do canal. O núcleo mostra no PC um código curto derivado do hash do handshake, e o aparelho só é inscrito depois que o mantenedor confirma no PC que o código é igual ao do celular. O código de uso único é consumido na primeira tentativa, certa ou errada.
3. **Passkey dentro do canal.** Depois do handshake, o aparelho prova a passkey (WebAuthn) ao núcleo dentro do canal cifrado. O relé nunca vê a passkey nem participa da conferência. A passkey fica ligada ao endereço do Worker (ADR 0008).
4. **Comandos assinados.** Cada comando remoto leva assinatura Ed25519 do aparelho sobre a serialização canônica do comando, com `nonce` aleatório, `counter` estritamente crescente por aparelho e `expires_at` curto (padrão proposto: 60 segundos). O núcleo recusa assinatura inválida, contador repetido ou menor que o último aceito, `nonce` já visto dentro da validade e comando vencido. O contador aceito é persistido antes de executar.
5. **Mesma biblioteca no celular.** A PWA usa a mesma implementação do PC compilada para WASM, para os dois lados não divergirem. Esta escolha depende de decisão do mantenedor; a alternativa é uma implementação própria em TypeScript conferida contra os mesmos vetores de teste.
6. **Revogação.** O mantenedor revoga um aparelho no PC; o núcleo apaga a chave pública dele e recusa qualquer comando seguinte. O relé recebe só a remoção do registro.
7. **Parâmetros (P8).** Validade do código de pareamento, validade do comando e número máximo de aparelhos ficam na configuração do núcleo. Padrão Noise, primitivas e a confirmação no PC não são parâmetros: afrouxá-los desfaz a proteção.

## Bibliotecas candidatas

Licença declarada no crates.io, consulta em 19/11/2026. Licença do arquivo `LICENSE` de cada repositório: **não verificado**.

| Uso | Biblioteca | Versão | Licença declarada | Situação na lista de `deny.toml` |
|---|---|---|---|---|
| Noise | `snow` | 0.10.0 | Apache-2.0 OR MIT | permitida |
| Noise (alternativa) | `clatter` | 2.3.0 | MIT | permitida |
| Assinatura | `ed25519-dalek` | 3.0.0 | BSD-3-Clause | permitida |
| Troca de chaves | `x25519-dalek` | 3.0.0 | BSD-3-Clause | permitida |

**Não verificado:** compilação dessas bibliotecas para `wasm32-unknown-unknown` e o tamanho resultante; suporte a Ed25519 no WebCrypto dos navegadores do celular; auditoria de segurança publicada de cada uma; serialização canônica do comando para assinatura (a definir junto com o contrato).

## Riscos

Detalhes, dono, teto e plano B em [risks.md](../risks.md#remoto-e-provedores).

- **O Worker serve a própria PWA.** Se a conta do Worker for comprometida, ele pode entregar JavaScript adulterado ao celular, que então lê o que decifra e assina o que quiser. A cifra de ponta a ponta não protege contra quem entrega o código do cliente. Mitigação a desenhar: PWA instalada e fixada por versão, com conferência de integridade do pacote, e confirmação no PC para ação destrutiva vinda do remoto.
- **Comando repetido.** Mitigado por `nonce`, `counter` e `expires_at`, já exigidos pelo `command.request`; depende de o núcleo persistir o contador antes de executar.
- **Pareamento interceptado.** Quem fotografa o QR tem o código por poucos minutos; a confirmação do código derivado no PC barra a inscrição de outro aparelho.
- **Aparelho roubado.** A passkey pede desbloqueio do aparelho; a revogação no PC corta o acesso.

## Consequências

- `crates/e2e` implementa o canal; o núcleo confere passkey e assinatura; `crates/adapters/relay` só transporta envelopes. `remote.transport = none` continua o padrão (ADR 0008).
- Os módulos de canal, conferência de assinatura e inscrição de aparelho entram na lista de módulos de segurança do `CONTRIBUTING.md`.
- Vetores de teste comuns ao PC e ao celular: handshake, assinatura, recusa de contador repetido, de comando vencido e de aparelho revogado.

## Condição de reversão

Trocar a biblioteca se uma auditoria ou aviso de segurança apontar falha sem correção, se ela não compilar para WASM com tamanho aceitável, ou se a licença conferida no repositório divergir da declarada. Rever o desenho inteiro se a passkey não puder ser conferida pelo núcleo dentro do canal.
