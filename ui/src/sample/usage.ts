import alternateUsage from '../../../contracts/golden/record.usage_event.v1/valid/02-variant1.json'
import observedProfile from '../../../contracts/golden/event.account.status_observed.v1/valid/01-profile.json'
import routingProposal from '../../../contracts/golden/event.routing.proposed.v2/valid/01-profile.json'
import selectedProfile from '../../../contracts/golden/event.account.switched.v1/valid/01-profile.json'
import task from '../../../contracts/golden/record.task-packet.v1/valid/01-base.json'
import usageRecord from '../../../contracts/golden/record.usage_event.v1/valid/01-base.json'
import type { UsageTool } from '../components/UsagePanel/UsagePanel'

const primary = observedProfile.data.account_alias
const secondary = selectedProfile.data.account_alias

// Agrupa os exemplos pela mesma ferramenta e usa os apelidos dos eventos de conta.
export const sampleUsage: readonly UsageTool[] = [
  {
    harness: observedProfile.data.harness,
    nextAccount: secondary,
    profiles: [
      { alias: primary, quota: usageRecord.quota_window_after },
      { alias: secondary, quota: alternateUsage.quota_window_after },
    ],
    proposal: routingProposal.data,
    taskBudget: {
      task: task.task_id,
      remainingUsd: Math.max(0, task.budget.usd - alternateUsage.cost_billable_usd),
    },
  },
]
