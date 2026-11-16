# ai-develop-setup

Codinome de uma plataforma local para delegar trabalho de desenvolvimento a agentes de IA: um daemon em Rust, uma casca Tauri e uma interface em React 19 + Vite + TypeScript. O nome comercial vem depois.

## Estado

Em construção. Ainda não há código de produto nem versão publicada: este repositório só tem o alicerce (regras, guardas de commit e CI).

- Regras para quem altera o repo (pessoas e agentes): [AGENTS.md](AGENTS.md)
- Como contribuir e o fluxo de branches: [CONTRIBUTING.md](CONTRIBUTING.md)
- Segurança e relato de vulnerabilidade: [SECURITY.md](SECURITY.md)

## Interface e conexão local

Em `ui/`, execute `npm ci` e `npm run dev`. Sem credenciais, a interface mantém
os exemplos locais. A URL entregue pelo núcleo contém `#core=ws://127.0.0.1:<porta>&token=<token>`;
a interface remove o fragmento, guarda as credenciais apenas no `sessionStorage`
da aba e autentica com `auth.hello` antes de enviar comandos. Recarregar a aba
reutiliza as credenciais; reiniciar o núcleo exige abrir a URL nova.
Armazenamento indisponível não impede a conexão iniciada pelo fragmento.

`VITE_CORE_SESSION_STREAMS` configura os streams de sessões conhecidos, separados
por vírgula, por exemplo `session/session-a,session/session-b`. O padrão recomendado
é vazio: os contratos atuais não oferecem descoberta de sessões nem assinatura
global. Configure os streams no ambiente de `npm run dev` ou `npm run build`;
reinicie o servidor ou reconstrua a interface para trocar a lista. Nenhum token
deve entrar nessa variável. No modo ao vivo, a lista e o terminal são reconstruídos
dos eventos desses streams, sem dados de exemplo. Entrada e redimensionamento do
terminal ficam para outra entrega.

Queda, recusa da autenticação ou falha de protocolo deixam a interface desconectada
e rejeitam pedidos pendentes. Não há reconexão automática: reabra a URL da execução.
Para voltar aos exemplos, limpe `ads.local-api` do armazenamento da aba e recarregue
sem fragmento. O cliente não registra token nem saída do terminal em logs.

Os comandos enviados e as mensagens recebidas são validados contra os Schemas.
O build gera validadores estáticos com o Ajv já presente nas dependências de
desenvolvimento, sem compilação dinâmica no navegador nem alteração de CSP.
Replay desde zero aceita o início do histórico ainda retido; depois disso,
uma lacuna na sequência encerra a conexão. Eventos repetidos são ignorados.

## Licença

Código aberto à vista, direitos reservados: o código pode ser lido, estudado e avaliado, mas usar, copiar, modificar ou distribuir exige licença comercial escrita. Não é software livre nem open source. Texto completo, com a versão em português que prevalece, em [LICENSE](LICENSE).
