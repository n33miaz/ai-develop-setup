# 0027. Session handoff

Status: proposta em 20/11/2026, aguardando ratificação do mantenedor

## Contexto

Sessões longas precisam de continuidade quando a janela de contexto cresce. A troca de perfil da [ADR 0022](0022-account-profile-folders-and-automatic-switching.md) e a oferta de outro modelo após recusa ou interrupção por filtro (#145) também podem iniciar uma sessão nova para o mesmo trabalho.

## Decisão

O produto usa o mesmo mecanismo para os três gatilhos. Uma proposta identifica a sessão de origem. Com o aceite, o produto guarda um documento curto de estado com pendências, decisões, regras em vigor e localização dos artefatos, junto com o prompt de abertura. Um componente do produto prepara esses textos a partir do contexto autorizado da sessão de origem. A nova sessão começa pelo prompt de abertura, que manda ler o documento de estado mantido pelo produto.

O padrão é `propose`, que autoriza a continuidade após o aceite. Cada passagem exige consentimento do usuário, com identificação opaca, canal local ou remoto e referência ao evento de aceite emitido pelo aparelho do usuário. Antes de encaminhar uma troca de conta à política de passagem, a integração aplica o modo de troca da ferramenta definido pela ADR 0022, incluindo a exigência de decisão manual quando aplicável.

Regra única para os dois modos ativos: nenhuma passagem segue sem consentimento confirmado para ela. Em `auto-with-consent`, só as passagens por contexto e por troca de conta podem ser iniciadas automaticamente depois desse consentimento. O gatilho `model_refusal` nunca é automático, em nenhum modo: começa sempre por uma proposta com as alternativas e só segue com um aceite específico, emitido por um aparelho do dono da linhagem, que nomeia a alternativa escolhida. Após recusa ou interrupção por filtro, o destino respeita as políticas aplicáveis a ele.

### Oferta de outro modelo após recusa ou filtro

A detecção é registrada pelo evento `urn:ads:event:session.refusal_detected:1`, só com metadados: sessão, ferramenta, provedor, modelo e tipo (`refusal` ou `safety_filter`). O texto do pedido e o da resposta não entram no evento. O modelo é uma referência curta ao catálogo configurado (minúsculas, dígitos e `. _ -`, até 48 caracteres), nunca texto da sessão; o núcleo só monta o evento para uma combinação de ferramenta, provedor e modelo que existe no catálogo.

O núcleo monta a oferta a partir das recusas do mesmo trabalho, da mais antiga para a mais nova. A sessão de origem é a da última recusa. As alternativas saem do catálogo na ordem do roteador, com as mesmas regras de sensibilidade, faixa e cota. Ficam de fora:

- toda combinação de ferramenta e modelo que já recusou, com qualquer provedor ou esforço;
- rotas bloqueadas por cota;
- rotas repetidas com a mesma ferramenta, provedor e modelo.

Não há oferta quando `enabled = false`, quando a passagem está em `off`, quando o orçamento da tarefa manda parar ou quando nenhuma alternativa sobra. Os parâmetros ficam em [`defaults/refusal.toml`](../../defaults/refusal.toml):

- `scope`: qualquer ferramenta ou só outros modelos da mesma;
- `order`: ordem do roteador, outras ferramentas primeiro (padrão) ou mesma ferramenta primeiro;
- `max_alternatives`: quantidade de alternativas.

A oferta identifica cada alternativa pelo id do candidato no catálogo. A rota exige um evento `accepted` de `session.handoff:4` para a mesma passagem e sessão de origem, com o gatilho `model_refusal`, o canal do aceite e o `alternative_id` da rota escolhida. O núcleo consulta o registro de aparelhos usado no replay da fila para confirmar que o aparelho pertencia ao dono da linhagem no instante do aceite. Só a alternativa aceita pode iniciar a nova sessão, que recebe o contexto pela passagem desta ADR.

Cada sessão usa o nome `<projeto> n<N> · <data>`, com o apelido do projeto como nome da linhagem, `n1` para a origem e data UTC no formato `AAAA-MM-DD`. Cada passagem cria a sessão seguinte e aponta para a passagem que criou sua origem, quando houver. O registro autorizado vincula as duas sessões e referencia os dois artefatos do produto.

Credenciais, conteúdo de login de ferramentas, arquivos de perfil protegidos por C1 e segredos do cofre ficam fora do documento, do prompt, dos eventos e do registro. A futura integração deve restringir a entrada ao contexto autorizado e verificar ambos os artefatos antes de guardar ou entregar qualquer texto. Referências só podem ser resolvidas dentro do acervo do usuário que autorizou a passagem. Antes de anexar uma passagem, a integração deve conferir que o usuário é o mesmo da linhagem, que o instante de criação não retrocede e que os identificadores de documento e prompt não se repetem na cadeia.

### Anexação à cadeia

O núcleo anexa à cadeia com `append_handoff`, para contexto ou troca de conta, e com `append_refusal_handoff`, para `model_refusal`; a integração deve usar essas funções. A primeira confere o aceite com `verify_consent`. A segunda chama `Offer::accept`, que também exige a alternativa escolhida na oferta, e devolve a rota aceita. Ambas recebem o registro da passagem e o evento do log autenticado. Exigem a mesma passagem, sessão de origem, referência ao evento, gatilho e canal, além do consentimento em nome do dono da linhagem, emitido por um aparelho dele no instante do aceite. Depois validam a cadeia inteira com o registro conferido no fim; em qualquer erro, a cadeia fica como estava.

## Condição de reversão

Se a passagem perder estado necessário, quebrar o isolamento ou entregar material protegido, usar `off` e retomar o trabalho na sessão de origem ou em uma sessão aberta manualmente. Conservar a cadeia autorizada para diagnóstico com acesso restrito. Se só a oferta após recusa causar problema, usar `enabled = false` em `defaults/refusal.toml`; os outros gatilhos continuam.

## Próximos passos

Integrar medição, consentimento, geração dos textos, armazenamento e abertura da sessão no daemon e na interface. Verificar o isolamento, a filtragem de ambos os artefatos e a recuperação em caso de falha antes de ativar o fluxo.

Para a oferta após recusa, falta detectar a recusa e o filtro na saída da ferramenta e emitir o evento. Falta também mostrar a oferta na interface e ligar o registro de aparelhos real ao aceite.
