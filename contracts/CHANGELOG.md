# Histórico dos contratos

Formato: uma seção por data. Contrato novo, campo opcional acrescentado, versão nova de uma URN e contrato aposentado entram aqui. As regras de versão estão no [README](README.md).

## 2026-11-15

Lacunas apontadas pelos consumidores (interface e portão de ações). Nenhum contrato `:1` teve regra apertada.

- Versões novas, convivendo com a `:1`:
  - `event.approval.requested:2`: `action_id` obrigatório, amarrando a aprovação à ação.
  - `command.response:2`: `result` fechado em `accepted` (sempre `true`) e `replayed` (repetição de comando idempotente).
- Contratos novos (`:1`):
  - Tipos comuns: `abspath` (caminho absoluto normalizado, Windows e POSIX) e `action-rule` (código de regra do portão).
  - Eventos: `action.denied` (plano negado), `action.started` (início da execução) e `session.activity_changed` (sessão ativa ou ociosa).
  - Registro: `action-plan`, com a serialização canônica do `plan_hash` descrita no README.
- Campo opcional acrescentado (a versão não muda):
  - `event.task.created:1`: `title`.
  - `event.session.created:1`: `display_name`.
  - Evento que leva um desses campos não pode ser `public`.
- Código de regra novo na lista do README (a versão de `action-rule` e de `action-plan` não muda): `allowed_root_itself`, alvo que é uma raiz permitida ou a contém.
- Regra `stream` ↔ identificador em `data` documentada no README e conferida pelo validador em todo dourado válido. Dourados corrigidos, que citavam no `stream` outra entidade que a de `data`: `event.approval.expired.v1/valid/02-variant1.json`, `event.approval.requested.v1/valid/02-variant1.json` e `event.task.created.v1/valid/02-variant1.json`.

## 2026-11-14

Primeira versão dos contratos (todos `:1`).

- Tipos comuns: `ulid`, `id`, `timestamp`, `sha256`, `base64`, `data-class`, `risk`, `relpath`, `actor`.
- Envelope de evento e de comando (JSON-RPC 2.0), com a resposta.
- Eventos: `session.created`, `session.output`, `session.resized`, `session.ended`, `session.permission_asked`; `task.created`, `task.started`, `task.completed`, `task.failed`, `task.cancelled`; `approval.requested`, `approval.granted`, `approval.denied`, `approval.expired`; `usage.recorded`, `usage.reconciled`; `host.sample_recorded`, `host.alert_raised`, `host.alert_acked`.
- Registros: `task-packet`, `usage_event`, `approval`, `host_sample`, `host_event`.

Segunda leva, no mesmo dia (todos `:1`; nenhum contrato da primeira leva foi alterado).

- Contratos entre processos: `harness-adapter` (descritor do adaptador) e `pty-host.request`, `pty-host.response`, `pty-host.notification`, `pty-host.frame` (o contrato `pty-host/1`, com o layout do quadro binário no README).
- Eventos: `routing.proposed`, `routing.decided`, `routing.fallback`; `review.requested`, `review.completed`, `review.failed`; `action.planned`, `action.executed`, `action.failed`.
- Registros: `update-manifest`, `update-status`, `parameter`.
- Relé (só Schema): `envelope`, `device`.
