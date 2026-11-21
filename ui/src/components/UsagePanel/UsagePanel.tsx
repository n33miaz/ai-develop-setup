import { useId } from 'react'
import type { EventRoutingProposedV2, RecordUsageEventV1 } from '../../api/contracts'
import { useT, type MessageKey } from '../../i18n'
import styles from './UsagePanel.module.css'

export interface UsageTool {
  harness: string
  nextAccount: string
  profiles: readonly { alias: string; quota: RecordUsageEventV1['quota_window_after'] }[]
  taskBudget: { task: string; remainingUsd: number }
  proposal: Pick<EventRoutingProposedV2['data'], 'account_alias' | 'model' | 'provider'>
}

const windows = [
  ['five_hour_pct', 'usage.fiveHour'],
  ['weekly_pct', 'usage.weekly'],
] as const satisfies readonly (readonly [
  keyof NonNullable<RecordUsageEventV1['quota_window_after']>,
  MessageKey,
])[]

function ToolUsage({ tool }: { tool: UsageTool }) {
  const t = useT()
  const id = useId()
  const percent = new Intl.NumberFormat(t.locale, {
    style: 'percent',
    maximumFractionDigits: 1,
  })
  return (
    <section className={styles.tool} aria-labelledby={id}>
      <h2 id={id}>{tool.harness}</h2>
      <div className={styles.picker}>
        <label htmlFor={id + '-picker'}>{t('usage.picker', { harness: tool.harness })}</label>
        <select
          id={id + '-picker'}
          value={tool.nextAccount}
          disabled
          aria-describedby={id + '-switch'}
        >
          {tool.profiles.map(({ alias }) => (
            <option key={alias} value={alias}>
              {alias === tool.nextAccount ? t('usage.next', { alias }) : alias}
            </option>
          ))}
        </select>
      </div>
      <p id={id + '-switch'}>{t('usage.switchDisabled')}</p>
      <ul className={styles.profiles}>
        {tool.profiles.map(({ alias, quota }) => (
          <li key={alias}>
            <h3>{alias}</h3>
            <dl>
              {windows.map(([field, label]) => {
                const observed = quota?.[field]
                const used = observed == null ? null : Math.min(100, Math.max(0, observed))
                return (
                  <div key={field}>
                    <dt>{t(label)}</dt>
                    <dd>
                      {used == null
                        ? t('usage.noObservation')
                        : t('usage.quota', {
                            used: percent.format(used / 100),
                            remaining: percent.format((100 - used) / 100),
                          })}
                    </dd>
                  </div>
                )
              })}
            </dl>
          </li>
        ))}
      </ul>
      <p>
        {t('usage.taskBudget', {
          task: tool.taskBudget.task,
          amount: new Intl.NumberFormat(t.locale, {
            style: 'currency',
            currency: 'USD',
          }).format(tool.taskBudget.remainingUsd),
        })}
      </p>
      <p>
        {t('usage.proposal', {
          alias: tool.proposal.account_alias ?? t('usage.noProfile'),
          model: tool.proposal.model,
          provider: tool.proposal.provider,
        })}
      </p>
    </section>
  )
}

export function UsagePanel({ tools }: { tools: readonly UsageTool[] | null }) {
  const t = useT()
  const id = useId()
  return (
    <details className={styles.panel}>
      <summary
        id={id}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            if (event.key === 'Enter' && !event.repeat) event.currentTarget.click()
          }
        }}
        onKeyUp={(event) => {
          if (event.key === ' ') {
            event.preventDefault()
            event.currentTarget.click()
          }
        }}
      >
        {t('usage.heading')}
      </summary>
      <div className={styles.content} role="region" aria-labelledby={id} tabIndex={0}>
        {tools === null ? (
          <p>{t('usage.unavailable')}</p>
        ) : (
          tools.map((tool) => <ToolUsage key={tool.harness} tool={tool} />)
        )}
      </div>
    </details>
  )
}
