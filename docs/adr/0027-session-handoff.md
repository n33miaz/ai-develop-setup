# 0027. Session handoff

Status: proposta

## Contexto

Sessões longas precisam de continuidade quando a janela de contexto cresce. A troca de perfil da [ADR 0022](0022-account-profile-folders-and-automatic-switching.md) e a oferta de outro modelo após recusa ou interrupção por filtro (#145) também podem iniciar uma sessão nova para o mesmo trabalho.

## Decisão

O produto usa o mesmo mecanismo para os três gatilhos. Uma proposta identifica a sessão de origem. Com o aceite, o produto guarda um documento curto de estado com pendências, decisões, regras em vigor e localização dos artefatos, junto com o prompt de abertura. Um componente do produto prepara esses textos a partir do contexto autorizado da sessão de origem. A nova sessão começa pelo prompt de abertura, que manda ler o documento de estado mantido pelo produto.

O padrão é `propose`, que autoriza a continuidade após o aceite. Cada passagem exige consentimento do usuário, com identificação opaca, canal local ou remoto e referência ao evento de aceite emitido pelo aparelho do usuário. O modo `auto-with-consent` só permite iniciar automaticamente uma passagem por contexto ou por troca de conta após confirmação de consentimento para aquela passagem. Após recusa ou interrupção por filtro, o produto oferece a opção ao usuário e respeita as políticas aplicáveis ao destino. Antes de encaminhar uma troca de conta à política de passagem, a integração aplica o modo de troca da ferramenta definido pela ADR 0022, incluindo a exigência de decisão manual quando aplicável.

Com o consentimento confirmado para aquela passagem, os dois modos deixam qualquer gatilho seguir. A recusa de modelo nunca começa sozinha: sem aceite, fica na proposta.

### Oferta de outro modelo após recusa ou filtro

A detecção é registrada pelo evento `urn:ads:event:session.refusal_detected:1`, só com metadados: sessão, ferramenta, provedor, modelo e tipo (`refusal` ou `safety_filter`). O texto do pedido e o da resposta não entram no evento.

O núcleo monta a oferta a partir das recusas do mesmo trabalho, da mais antiga para a mais nova. A sessão de origem é a da última recusa. As alternativas saem do catálogo na ordem do roteador, com as mesmas regras de sensibilidade, faixa e cota. Ficam de fora:

- toda combinação de ferramenta e modelo que já recusou, com qualquer provedor ou esforço;
- rotas bloqueadas por cota;
- rotas repetidas com a mesma ferramenta, provedor e modelo.

Não há oferta quando `enabled = false`, quando a passagem está em `off`, quando o orçamento da tarefa manda parar ou quando nenhuma alternativa sobra. Os parâmetros ficam em [`defaults/refusal.toml`](../../defaults/refusal.toml):

- `scope`: qualquer ferramenta ou só outros modelos da mesma;
- `order`: ordem do roteador, outras ferramentas primeiro (padrão) ou mesma ferramenta primeiro;
- `max_alternatives`: quantidade de alternativas.

A oferta é só uma proposta. A rota escolhida vale apenas com um evento `accepted` de `session.handoff:2` emitido por um aparelho, para a mesma passagem e a mesma sessão de origem, e só se estiver entre as alternativas oferecidas. Quem chama confere que o aparelho é do dono da linhagem. A sessão nova recebe o contexto pela mesma passagem desta ADR, com o gatilho `model_refusal`.

Cada sessão usa o nome `<projeto> n<N> · <data>`, com o apelido do projeto como nome da linhagem, `n1` para a origem e data UTC no formato `AAAA-MM-DD`. Cada passagem cria a sessão seguinte e aponta para a passagem que criou sua origem, quando houver. O registro autorizado vincula as duas sessões e referencia os dois artefatos do produto.

Credenciais, conteúdo de login de ferramentas, arquivos de perfil protegidos por C1 e segredos do cofre ficam fora do documento, do prompt, dos eventos e do registro. A futura integração deve restringir a entrada ao contexto autorizado e verificar ambos os artefatos antes de guardar ou entregar qualquer texto. Referências só podem ser resolvidas dentro do acervo do usuário que autorizou a passagem. Antes de anexar uma passagem, a integração deve conferir que o usuário é o mesmo da linhagem, que o instante de criação não retrocede e que os identificadores de documento e prompt não se repetem na cadeia.

## Condição de reversão

Se a passagem perder estado necessário, quebrar o isolamento ou entregar material protegido, usar `off` e retomar o trabalho na sessão de origem ou em uma sessão aberta manualmente. Conservar a cadeia autorizada para diagnóstico com acesso restrito. Se só a oferta após recusa causar problema, usar `enabled = false` em `defaults/refusal.toml`; os outros gatilhos continuam.

## Próximos passos

Integrar medição, consentimento, geração dos textos, armazenamento e abertura da sessão no daemon e na interface. Verificar o isolamento, a filtragem de ambos os artefatos e a recuperação em caso de falha antes de ativar o fluxo.

Para a oferta após recusa, falta detectar a recusa e o filtro na saída da ferramenta e emitir o evento. Falta também mostrar a oferta na interface e registrar a rota escolhida. Hoje o `record.session-handoff:2` não guarda a rota de destino: a escolha precisa de um registro próprio ou de uma versão nova do contrato.
