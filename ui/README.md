# Interface web

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

Para conectar a interface a terminais reais, siga o [guia do daemon](../crates/daemon/README.md).

Conectada, a interface mostra um botão de abertura para cada ferramenta que o
daemon informa em `harness.list`. Enquanto a consulta não volta, um aviso diz que
ela está em andamento. Sem botões, cada caso tem o seu aviso: lista vazia;
falta de resposta em 10 segundos (reabra a URL da execução); daemon anterior ao
comando, que responde `METHOD_NOT_FOUND` (atualize o daemon; reabrir a URL não resolve);
e outra recusa do daemon, com aviso neutro. Sem apelido de pasta anunciado
pelo daemon, os botões ficam desativados e um aviso explica o motivo. Cada
conexão autenticada tem uma geração própria: a lista, o apelido e os botões
valem só para a geração que os recebeu, então uma reconexão volta ao aviso de
consulta em andamento mesmo que a tela não chegue a mostrar a queda. A resposta
que chega depois do prazo é descartada sem derrubar a conexão. `VITE_CORE_HARNESSES` (identificadores separados
por vírgula, padrão vazio) vale só como reserva no modo de exemplo, sem núcleo,
onde os botões ficam desativados. `VITE_CORE_WORKDIR` escolhe o apelido de pasta,
com padrão `default`; se o daemon não conhecer esse apelido, a interface usa o
primeiro que ele informar, e nunca um que ele não informou. Ao abrir, a interface assina o stream e envia
`session.open`. Recusas aparecem como aviso traduzido. A sessão recebe foco
ao abrir, e a digitação e a colagem enviam `session.write` em blocos ordenados
de até 65.536 bytes.
O tamanho ajustado envia `session.resize`; `VITE_CORE_RESIZE_DEBOUNCE_MS` define a
espera após o último ajuste (padrão recomendado 150 ms, zero desliga a espera).
Essas variáveis são lidas ao iniciar o Vite ou construir a interface; reinicie ou
reconstrua para alterá-las. Sem callback de entrada, o componente continua só leitura.

O protocolo não cancela assinaturas. Por isso o cliente WebSocket mantém uma só assinatura no
daemon por stream e conexão e reparte os eventos entre os ouvintes locais, como cada terminal
montado. Sair só desliga o ouvinte; a assinatura continua, guardando os eventos recentes para quem
assinar depois, por exemplo ao voltar a uma sessão. Os eventos guardados chegam a esse ouvinte já
dentro de `subscribe`. Um ouvinte que pede eventos que já não estão guardados recebe só o próprio
fim (`SUBSCRIPTION_REPLAY_UNAVAILABLE`), sem afetar os outros ouvintes nem a conexão, mesmo que o daemon ainda tenha esses
eventos. O cliente não pede de novo ao daemon de propósito: sem cancelamento, uma segunda assinatura
no mesmo stream e conexão ficaria viva para sempre e exigiria ligar vários ids de pedido ao mesmo
stream. `VITE_CORE_REPLAY_EVENTS` (padrão 256) limita os eventos guardados por stream e deve
acompanhar `events_per_stream` do daemon: com valor menor, quem chega depois recebe menos histórico
do que o daemon teria. `VITE_CORE_REPLAY_BYTES` (padrão 4194304) limita o tamanho guardado por
stream, contado em unidades UTF-16 das mensagens recebidas (igual a bytes para texto ASCII). Os dois
valem de 1 em diante; 0, negativos e valores inválidos voltam ao padrão, porque sem nada guardado um
terminal remontado ficaria vazio e uma nova assinatura depois de reconectar daria por encerrada uma
sessão viva.

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
