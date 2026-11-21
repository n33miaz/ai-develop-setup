import { useRef, useState } from 'react'
import { CommandRefusal, type queueApprovals } from '../api/queueApprovals'
import {
  APPROVAL_ALREADY_ANSWERED,
  APPROVAL_NOT_APPROVABLE,
  APPROVAL_NOT_CONFIGURED,
  APPROVAL_NOT_FOUND,
  APPROVAL_OUT_OF_ORDER,
  APPROVAL_POLICY_CHANGED,
  APPROVAL_STALE,
} from '../../../contracts/generated/ts/contract.local-api.error-code.v4.constants'
import { useT, type MessageKey } from '../i18n'
import styles from './UsagePanel/UsagePanel.module.css'

type Api = ReturnType<typeof queueApprovals>
type Page = Awaited<ReturnType<Api['list']>>
type Approval = Page['approvals'][number]
type Decision = Parameters<Api['answer']>[0]['decision']

const refusals: Partial<Record<number, MessageKey>> = {
  [APPROVAL_ALREADY_ANSWERED]: 'approvals.stale',
  [APPROVAL_NOT_APPROVABLE]: 'approvals.blocked',
  [APPROVAL_NOT_CONFIGURED]: 'approvals.unconfigured',
  [APPROVAL_NOT_FOUND]: 'approvals.stale',
  [APPROVAL_OUT_OF_ORDER]: 'approvals.clock',
  [APPROVAL_POLICY_CHANGED]: 'approvals.policy',
  [APPROVAL_STALE]: 'approvals.stale',
}

function RecordedEffects({ approval }: { approval: Approval }) {
  const t = useT()
  return (
    <>
      <p>{t(`approvals.${approval.plan.operation}`)}</p>
      <p>{t('approvals.targets')}</p>
      <ul>
        {approval.plan.targets.map((target, index) => (
          <li key={index}>
            <code>{target.path}</code>
            {target.pattern !== null && (
              <>
                {' '}
                (<code>{target.pattern}</code>)
              </>
            )}
          </li>
        ))}
      </ul>
      {approval.plan.destination !== null && (
        <p>
          {t('approvals.destination')} <code>{approval.plan.destination}</code>
        </p>
      )}
      <p>{t(approval.plan.quarantined ? 'approvals.quarantined' : 'approvals.noQuarantine')}</p>
      {approval.effects?.steps.map((step, index) => (
        <div key={index}>
          <p>
            {t(`approvals.${step.effect}`)} <code>{step.target}</code>
          </p>
          {step.stored !== null && (
            <p>
              {t('approvals.stored')} <code>{step.stored}</code>
            </p>
          )}
          {step.entries.length > 0 && (
            <ul>
              {step.entries.map((entry, entryIndex) => (
                <li key={entryIndex}>
                  <code>{entry.path.join('/') || step.target}</code> {t(`approvals.${entry.kind}`)}{' '}
                  {t('approvals.bytes', { count: String(entry.bytes) })}
                </li>
              ))}
            </ul>
          )}
        </div>
      ))}
      {approval.effects_error && (
        <p>
          {t('approvals.effectsError')} <code>{approval.effects_error}</code>
        </p>
      )}
      {approval.not_approvable.length > 0 && (
        <ul>
          {approval.not_approvable.map((reason) => (
            <li key={reason}>{t(`approvals.${reason}`)}</li>
          ))}
        </ul>
      )}
    </>
  )
}

export function ApprovalsPanel({ api }: { api: Api }) {
  const t = useT()
  const [page, setPage] = useState<Page>()
  const [message, setMessage] = useState<MessageKey>()
  const [busy, setBusy] = useState(false)
  const [confirming, setConfirming] = useState<string>()
  const lock = useRef(false)
  const opened = useRef(false)

  async function run(answer?: Parameters<Api['answer']>[0]) {
    if (lock.current) return
    lock.current = true
    setBusy(true)
    setConfirming(undefined)
    setMessage(undefined)
    setPage(undefined)
    let answered = false
    try {
      if (answer) {
        await api.answer(answer)
        answered = true
      }
      setPage(await api.list())
      if (answer) setMessage('controls.saved')
    } catch (error) {
      setMessage(
        answered
          ? 'approvals.savedRefreshFailed'
          : error instanceof CommandRefusal
            ? (refusals[error.code] ?? 'controls.refused')
            : 'controls.failed',
      )
    } finally {
      lock.current = false
      setBusy(false)
    }
  }

  function respond(approval: Approval, decision: Decision) {
    const { action_id, approval_id, plan_hash } = approval
    if (
      decision === 'approve' &&
      (confirming !== approval_id ||
        approval.not_approvable.length > 0 ||
        approval.effects === null ||
        approval.effects_hash === null ||
        approval.effects_error !== null)
    )
      return
    void run({ action_id, approval_id, plan_hash, decision })
  }

  return (
    <details
      className={styles.panel}
      onToggle={(event) => {
        if (!event.currentTarget.open || opened.current) return
        opened.current = true
        void run()
      }}
    >
      <summary>{t('approvals.heading')}</summary>
      <div className={styles.content}>
        <button
          disabled={busy}
          onClick={() => {
            opened.current = true
            void run()
          }}
        >
          {t('controls.refresh')}
        </button>
        {busy && <p role="status">{t('controls.loading')}</p>}
        {message && <p role={message === 'controls.saved' ? 'status' : 'alert'}>{t(message)}</p>}
        {page?.more && <p>{t('approvals.more')}</p>}
        {page?.approvals.length === 0 && <p>{t('approvals.empty')}</p>}
        {page?.approvals.map((approval) => {
          const confirmingThis = confirming === approval.approval_id
          const canApprove =
            approval.not_approvable.length === 0 &&
            approval.effects !== null &&
            approval.effects_hash !== null &&
            approval.effects_error === null
          return (
            <article className={styles.tool} key={approval.approval_id}>
              <h2>{approval.summary}</h2>
              {confirmingThis && <p role="status">{t('approvals.confirm')}</p>}
              <RecordedEffects approval={approval} />
              {!canApprove && <p>{t('approvals.blocked')}</p>}
              {confirmingThis ? (
                <div className={styles.picker}>
                  <button
                    onClick={() => {
                      respond(approval, 'approve')
                    }}
                  >
                    {t('approvals.confirmButton')}
                  </button>
                  <button
                    onClick={() => {
                      setConfirming(undefined)
                    }}
                  >
                    {t('approvals.cancel')}
                  </button>
                </div>
              ) : (
                <div className={styles.picker}>
                  <button
                    disabled={!canApprove || confirming !== undefined}
                    onClick={() => {
                      setConfirming(approval.approval_id)
                    }}
                  >
                    {t('approvals.approve')}
                  </button>
                  <button
                    disabled={confirming !== undefined}
                    onClick={() => {
                      respond(approval, 'deny')
                    }}
                  >
                    {t('approvals.deny')}
                  </button>
                </div>
              )}
            </article>
          )
        })}
      </div>
    </details>
  )
}
