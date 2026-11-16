# 0013. Only fast checks block

Status: aceita em 14/11/2026

## Contexto
Em repositório público os minutos de runner padrão não custam; o custo do CI é tempo de espera e alarme falso. Todos os portões de segurança devem existir desde o primeiro dia, mas nem todos precisam barrar cada PR.

## Decisão
"(a) Bloqueia só o rápido (até 5 min); o pesado roda semanal e avisa".

## Consequências
- Bloqueiam: formato de commit, segredos, termos proibidos, contratos, formatação, lint, testes, regra de dependência, licenças e avisos de segurança das dependências; e, só quando o respectivo código muda, o ciclo de PTY em Windows e a compilação da casca.
- Rodam agendados e só avisam: análise estática profunda, Scorecard, fixture em Linux, ensaio completo de atualização.
- Com Rust (ADR 0002) e a casca (ADR 0007) o alvo de 5 minutos só fecha com cache quente em PR que não toca PTY nem casca. Estimativa a medir no primeiro marco: mediana de 8 a 10 minutos, até 20 com cache frio.
- O build de release sai do caminho bloqueante e roda depois da junção.
- Cada peça desligada tem gatilho objetivo escrito para virar obrigatória.

## Condição de reversão
Se a mediana medida passar de 10 minutos, mover o lint para depois da junção; se uma falha de segurança ficar mais de uma semana sem ser vista, a análise correspondente passa a bloquear.
