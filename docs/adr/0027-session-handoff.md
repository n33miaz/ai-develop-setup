# 0027. Session handoff

Status: proposta

## Contexto

Sessões longas precisam de continuidade quando a janela de contexto cresce. A troca de perfil da [ADR 0022](0022-account-profile-folders-and-automatic-switching.md) e a oferta de outro modelo após recusa ou interrupção por filtro (#145) também podem iniciar uma sessão nova para o mesmo trabalho.

## Decisão

O produto usa o mesmo mecanismo para os três gatilhos. Uma proposta identifica a sessão de origem. Com o aceite, o produto guarda um documento curto de estado com pendências, decisões, regras em vigor e localização dos artefatos, junto com o prompt de abertura. Um componente do produto prepara esses textos a partir do contexto autorizado da sessão de origem. A nova sessão começa pelo prompt de abertura, que manda ler o documento de estado mantido pelo produto.

O padrão é `propose`. Cada passagem exige consentimento do usuário, com identificação opaca e canal local ou remoto. O modo `auto-with-consent` só permite iniciar automaticamente uma passagem por contexto ou por troca de conta após confirmação de consentimento para aquela passagem. Após recusa ou interrupção por filtro, o produto oferece a opção ao usuário e respeita as políticas aplicáveis ao destino. A troca de conta também continua sujeita à ADR 0022.

Cada sessão usa o nome `<projeto> n<N> · <data>`, com o apelido do projeto como nome da linhagem, `n1` para a origem e data UTC no formato `AAAA-MM-DD`. Cada passagem cria a sessão seguinte e aponta para a passagem que criou sua origem, quando houver. O registro autorizado vincula as duas sessões e referencia os dois artefatos do produto.

Credenciais, conteúdo de login de ferramentas, arquivos de perfil protegidos por C1 e segredos do cofre ficam fora do documento, do prompt, dos eventos e do registro. A futura integração deve restringir a entrada ao contexto autorizado e verificar ambos os artefatos antes de guardar ou entregar qualquer texto. Referências só podem ser resolvidas dentro do acervo do usuário que autorizou a passagem.

## Condição de reversão

Se a passagem perder estado necessário, quebrar o isolamento ou entregar material protegido, usar `off` e retomar o trabalho na sessão de origem ou em uma sessão aberta manualmente. Conservar a cadeia autorizada para diagnóstico com acesso restrito.

## Próximos passos

Integrar medição, consentimento, geração dos textos, armazenamento e abertura da sessão no daemon e na interface. Verificar o isolamento, a filtragem de ambos os artefatos e a recuperação em caso de falha antes de ativar o fluxo.
