# 0020. Local API authentication

Status: proposta, aguardando ratificação do mantenedor

Em vigor no código, pendente de ratificação pelo mantenedor.

## Contexto
A interface (no navegador, e depois na casca Tauri, ADR 0007) fala com o núcleo por um WebSocket em loopback. Por essa conexão passam a abertura de sessões de terminal, o que se digita nelas e a saída delas. Quem fala com essa porta comanda um terminal do usuário com uma ferramenta de IA dentro.

Escutar só em `127.0.0.1` não basta:

- **WebSocket não tem política de mesma origem.** O navegador deixa qualquer página aberta, de qualquer site, abrir `ws://127.0.0.1:<porta>`. O único sinal que ele manda é o cabeçalho `Origin`, que o servidor tem de conferir; não há pré-checagem de CORS.
- **DNS rebinding.** Um domínio do atacante pode passar a resolver para `127.0.0.1` depois de carregado. A página continua com a origem do atacante, mas o pedido chega ao loopback com um `Host` que não é o do núcleo.
- **Outro usuário da mesma máquina alcança o loopback.** No Windows, `127.0.0.1` é da máquina, não do usuário: qualquer conta com sessão aberta (ou um serviço) conecta na mesma porta. Isso viola P3 (usuários totalmente isolados). O `ptyhost` já se protege disso com um canal só do usuário (ADR 0003); a API local, por ser TCP, precisa de um segredo.
- **Programas locais do próprio usuário** que não são navegador não mandam `Origin` nenhum, ou mandam o que quiserem. Conferir `Origin` barra páginas, não programas.

## Decisão
Proposta:

1. **Endereço.** O núcleo escuta só em `127.0.0.1`, numa porta efêmera escolhida pelo sistema a cada execução. Nunca em `0.0.0.0`, `::` ou interface de rede.
2. **`Host` e `Origin`.** No pedido de upgrade do WebSocket, o núcleo confere:
   - `Host` igual a `127.0.0.1:<porta>` ou `localhost:<porta>` (barra DNS rebinding);
   - `Origin` presente e numa lista permitida exata, sem curinga. Padrão de desenvolvimento: `http://127.0.0.1:5173`, o mesmo endereço em que o Vite escuta. No modo desktop, a lista contém somente `http://tauri.localhost`, origem da WebView no Windows.
   Qualquer falha responde `403` e fecha, sem dizer qual conferência falhou.
3. **Token por execução.** A cada partida o núcleo gera um token de 32 bytes (256 bits; o mínimo aceitável é 128) de um gerador criptográfico do sistema, codificado em base64url sem preenchimento (43 caracteres). O núcleo mantém o token em memória durante a execução.
4. **Entrega pelo fragmento.** O núcleo imprime uma única linha com a URL da interface e o token no fragmento: `http://127.0.0.1:5173/#core=ws://127.0.0.1:<porta>&token=<token>`. O fragmento não sai do navegador em pedido HTTP nenhum, nem vai para o `Referer`. A interface lê o fragmento, guarda o token só na sessão da aba e limpa a URL.
5. **Primeira mensagem.** A interface manda o token como primeira mensagem do WebSocket, no comando `auth.hello` (contrato `urn:ads:command:auth.hello:1`). Nunca em query string, cabeçalho de URL ou subprotocolo, que acabam em log e em histórico.
6. **Conferência.** O núcleo compara o token em tempo constante. Antes de um `auth.hello` válido nenhum outro comando é aceito; token errado, mensagem que não é `auth.hello` ou falta dele dentro do prazo (padrão 5 s) fecham a conexão. Nenhuma resposta diz se o token estava perto de certo.
7. **Origem remota recusada.** Todo comando que chega pela API local com `_meta.origin = "remote"` é recusado, mesmo autenticado: comando remoto só entra pelo transporte remoto, assinado (ADR 0008).
8. **Nada em disco nem em log.** O token e a saída de terminal nunca são gravados em arquivo nem em log, inclusive em mensagem de erro e em modo de depuração. No modo navegador, a linha com a URL vai para a saída padrão do processo que o usuário abriu. No modo desktop, a linha de conexão vai para o pipe herdado pela casca.

Parâmetros (P8): a lista de origens permitidas e o prazo do `auth.hello` ficam na configuração do núcleo, com os padrões acima. Endereço, tamanho mínimo do token e a regra do fragmento não são parâmetros: afrouxá-los desfaz a proteção.

## Por que porta efêmera
- Uma porta fixa é um alvo conhecido: página maliciosa e outro usuário da máquina sabem onde bater sem procurar, e a varredura deixa de ser necessária.
- Duas execuções (dois usuários na mesma máquina, ou uma cópia antiga ainda de pé) não disputam a mesma porta; com porta fixa, o primeiro a ocupar recebe as conexões do segundo.
- A porta não é segredo: a proteção é o token. Ela só tira a previsibilidade e o conflito de graça.

## Alternativas descartadas
- **Só loopback.** Não barra página aberta no navegador nem outro usuário da máquina.
- **Token em query string** (`ws://...?token=`). A URL do pedido vai para log de servidor, de proxy e de ferramenta de desenvolvimento.
- **Token em cookie.** O navegador manda cookie de `localhost` junto com pedidos que outras páginas disparam; volta o problema de origem.
- **Token fixo em arquivo de configuração.** Vira segredo em disco, de vida longa, que outro programa lê; o token por execução morre com o processo.
- **Named pipe ou socket do usuário**, como o `ptyhost`. O navegador não fala com nenhum dos dois.
- **TLS em loopback.** Não autentica o cliente e pede certificado instalado na máquina.

## Consequências
- O núcleo precisa de: escuta em `127.0.0.1` com porta efêmera, conferência de `Host` e `Origin` no upgrade, gerador do token, prazo e máquina de estados da conexão (antes e depois do `auth.hello`), comparação em tempo constante, recusa de `origin: "remote"` e testes de cada recusa.
- A interface lê o fragmento, guarda o token na sessão da aba, limpa a URL e manda `auth.hello` antes de tudo. Sem token, fica no modo de exemplo.
- Reiniciar o núcleo troca a porta e o token: a interface aberta perde a conexão e o usuário abre a URL nova.
- No Windows, a casca Tauri recebe porta e token pelo pipe anônimo configurado pelo lançador. A casca entrega esses dados uma vez por IPC à interface empacotada, que usa o token em memória.
- Este módulo entra na lista de módulos de segurança do `CONTRIBUTING.md`: mudanças nele exigem uma revisão a mais antes de juntar.

## Limite conhecido

A porta da interface também pertence à máquina, não ao usuário. Se outra conta
ocupar essa porta antes, a página servida por ela recebe o fragmento e pode usar
o token. `ui_base_url` em `defaults/daemon.toml` parametriza a URL impressa e aceita somente HTTP com host em loopback. Por
padrão a lista de origens deriva somente dessa URL. Isso reduz a lista permitida,
mas não comprova quem serve a página.

Em desenvolvimento, o servidor da interface deve usar porta fixa e `strictPort`.
A URL só deve ser aberta depois de conferir que a porta é do próprio usuário;
porta ocupada exige interromper a abertura e investigar. Não há proteção automática
contra outra conta nessa etapa. O próximo passo é o daemon servir a interface na
sua porta efêmera ou a casca Tauri entregar o token por um canal próprio do usuário.
Esse limite não altera a decisão acima; o risco tem dono, teto e plano B em
[risks.md](../risks.md#outro-usuário-ocupa-a-porta-da-interface).

## Condição de reversão
Trocar o mecanismo se a casca Tauri, verificada no Windows, não conseguir mandar um `Origin` estável e exclusivo dela; se uma revisão de segurança mostrar caminho para ler o fragmento ou o token de fora da aba; ou se o núcleo passar a escutar fora do loopback (aí o transporte é o remoto, ADR 0008, com cifra e assinatura).
