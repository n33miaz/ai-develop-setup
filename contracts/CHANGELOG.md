# Histórico dos contratos

Formato: uma seção por data. Contrato novo, campo opcional acrescentado, versão nova de uma URN e contrato aposentado entram aqui. As regras de versão estão no [README](README.md).

## 2026-11-14

Primeira versão dos contratos (todos `:1`).

- Tipos comuns: `ulid`, `id`, `timestamp`, `sha256`, `base64`, `data-class`, `risk`, `relpath`, `actor`.
- Envelope de evento e de comando (JSON-RPC 2.0), com a resposta.
- Eventos: `session.created`, `session.output`, `session.resized`, `session.ended`, `session.permission_asked`; `task.created`, `task.started`, `task.completed`, `task.failed`, `task.cancelled`; `approval.requested`, `approval.granted`, `approval.denied`, `approval.expired`; `usage.recorded`, `usage.reconciled`; `host.sample_recorded`, `host.alert_raised`, `host.alert_acked`.
- Registros: `task-packet`, `usage_event`, `approval`, `host_sample`, `host_event`.

Segunda leva, no mesmo dia (todos `:1`; nenhum contrato da primeira leva foi alterado).

- Contratos entre processos: `harness-adapter` (descritor do adaptador) e `pty-host.request`, `pty-host.response`, `pty-host.notification`, `pty-host.frame` (o contrato `pty-host/1`, com o layout do quadro binário no README).
