# ai-develop-setup

Codinome de uma plataforma local para delegar trabalho de desenvolvimento a agentes de IA: um daemon em Rust, uma casca Tauri e uma interface em React 19 + Vite + TypeScript. O nome comercial vem depois.

## Estado

Em construção, sem versão publicada. Já existem os contratos e os tipos gerados deles, o portão de ações destrutivas, o executor de arquivos, o host de terminais com o cliente dele no daemon, a abertura do banco local com a cifra por coluna, a API local autenticada servida pelo daemon e a interface com o cliente dela; o estado de cada módulo está em [docs/modules.md](docs/modules.md) e os riscos conhecidos em [docs/risks.md](docs/risks.md).

- Regras para quem altera o repo (pessoas e agentes): [AGENTS.md](AGENTS.md)
- Como contribuir e o fluxo de branches: [CONTRIBUTING.md](CONTRIBUTING.md)
- Segurança e relato de vulnerabilidade: [SECURITY.md](SECURITY.md)

## Interface e conexão local

Em `ui/`, execute `npm ci` e `npm run dev`. O Vite escuta em `127.0.0.1:5173`
no desenvolvimento e na prévia, o mesmo endereço do `ui_base_url` padrão do núcleo. Se a porta estiver ocupada, o Vite para em vez
de pular para outra. Sem credenciais, a interface mantém
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
dos eventos desses streams, sem dados de exemplo.

Para abrir Claude ou Codex pelo navegador no Windows:

1. Na raiz, inicie o host com `cargo run -p ads-ptyhost -j 2`.
2. Localize os executáveis com `where.exe claude.exe` e `where.exe codex.exe`
   (com o Codex instalado pelo npm, veja no README do daemon onde fica o
   `codex.exe`; não use os arquivos `.cmd`). Salve os caminhos absolutos em
   `[tools.claude]` e `[tools.codex]` num TOML fora do repositório e dentro do
   seu perfil, como `C:\Users\voce\ads-local\harnesses.toml`. Em `[workdirs]`,
   dê o apelido `default` a uma pasta de teste do perfil, sem nada importante.
   O TOML, a pasta de trabalho e o programa precisam ser graváveis só por você;
   nunca use a raiz de um disco de dados nem pasta compartilhada. Veja o
   exemplo completo no
   [README do daemon](crates/daemon/README.md#abrir-claude-ou-codex-no-navegador).
3. Em outro terminal, na raiz, rode `cargo build -p ads-daemon -j 2` e
   `.\target\debug\ads-daemon.exe --config 'C:\Users\voce\ads-local\harnesses.toml'`,
   com o caminho absoluto do seu arquivo.
4. Em `ui/`, rode `npm ci` e `npm run dev`.
   Depois que o Vite confirmar a porta, abra a URL completa impressa pelo daemon
   (`http://127.0.0.1:5173/#core=...`) e clique em **Abrir claude** ou
   **Abrir codex**. Os botões vêm da configuração do daemon, pelo comando
   `harness.list`.

O agente dentro do terminal edita arquivos e executa comandos conforme as
permissões dele. Argumentos que desligam as confirmações ou a área isolada da
ferramenta, somados ao controle do terminal pela API, dão a quem usa a interface
acesso total à sua conta.
Conectada, a interface mostra um botão de abertura para cada ferramenta que o
daemon informa em `harness.list`; lista vazia ou recusada não mostra botão
(a recusa aparece como aviso). `VITE_CORE_HARNESSES` (identificadores separados
por vírgula, padrão vazio) vale só como reserva no modo de exemplo, sem núcleo,
onde os botões ficam desativados. `VITE_CORE_WORKDIR` escolhe o apelido de pasta,
com padrão `default`; se o daemon não conhecer esse apelido, a interface usa o
primeiro que ele informar. Ao abrir, a interface assina o stream e envia
`session.open`. Recusas aparecem como aviso traduzido. A sessão recebe foco
ao abrir, e a digitação e a colagem enviam `session.write` em blocos ordenados
de até 65.536 bytes.
O tamanho ajustado envia `session.resize`; `VITE_CORE_RESIZE_DEBOUNCE_MS` define a
espera após o último ajuste (padrão recomendado 150 ms, zero desliga a espera).
Essas variáveis são lidas ao iniciar o Vite ou construir a interface; reinicie ou
reconstrua para alterá-las. Sem callback de entrada, o componente continua só leitura.

O contrato atual não traz um marcador de fim do replay nem seu último `seq`.
Na API local, os eventos chegam em ordem de sequência; a fonte classifica como
histórico cada bloco cujo `ts` é anterior ou igual ao instante local da assinatura.
Isso pressupõe o mesmo relógio do daemon local; a igualdade é conservadora para a
resolução de milissegundos. Cada bloco é analisado pelo xterm até seu callback de
conclusão antes do próximo. Durante a análise de um bloco histórico, a entrada
emitida pelo xterm é descartada, inclusive respostas de posição do cursor e atributos
do dispositivo. Isso também suspende brevemente a digitação durante essa análise;
após ela, a entrada normal e as respostas a consultas novas passam. Não se usa um
temporizador para presumir que o replay acabou.

Queda, recusa da autenticação ou falha de protocolo deixam a interface desconectada
e rejeitam pedidos pendentes. Não há reconexão automática: reabra a URL da execução.
Para voltar aos exemplos, limpe `ads.local-api` do armazenamento da aba e recarregue
sem fragmento. O cliente não registra token nem saída do terminal em logs.

Os comandos enviados e as mensagens recebidas são validados contra os Schemas.
O build gera validadores estáticos com o Ajv já presente nas dependências de
desenvolvimento, sem compilação dinâmica no navegador nem alteração de CSP.
Replay desde zero aceita o início do histórico ainda retido; depois disso,
uma lacuna na sequência encerra a conexão. Eventos repetidos são ignorados.

## Instalar a interface como aplicativo

Em `ui/`, execute `npm ci`, `npm run build` e `npm run preview` para
testar a instalação no PC. Abra o endereço local informado pelo Vite. Para servir
o aplicativo em outro endereço, publique `ui/dist/` em um servidor estático
HTTPS, incluindo o manifesto, os ícones e o service worker.

- No PC, abra a interface em um navegador compatível e use **Instalar
  aplicativo** na barra de endereço ou no menu do navegador.
- No Android, abra o endereço HTTPS no Chrome e use a opção para instalar ou
  adicionar à tela inicial. No iPhone, abra no Safari e toque em
  **Compartilhar** e **Adicionar à Tela de Início**.

Ao receber o aviso de atualização, recarregue a página. O acesso remoto ao
núcleo ainda não existe. No celular, uma interface servida por HTTPS pode
ser instalada e aberta no modo de exemplo.

## Licença

Código aberto à vista, direitos reservados: o código pode ser lido, estudado e avaliado, mas usar, copiar, modificar ou distribuir exige licença comercial escrita. Não é software livre nem open source. Texto completo, com a versão em português que prevalece, em [LICENSE](LICENSE).
