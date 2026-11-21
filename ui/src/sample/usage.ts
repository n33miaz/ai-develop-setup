import usageRecord from '../../../contracts/golden/record.usage_event.v1/valid/02-variant1.json'
import observedProfile from '../../../contracts/golden/event.account.status_observed.v1/valid/01-profile.json'
import routingProposal from '../../../contracts/golden/event.routing.proposed.v2/valid/01-profile.json'
import selectedProfile from '../../../contracts/golden/event.account.switched.v1/valid/01-profile.json'
import task from '../../../contracts/golden/record.task-packet.v1/valid/01-base.json'
import type { UsageTool } from '../components/UsagePanel/UsagePanel'

const primary = observedProfile.data.account_alias
const secondary = selectedProfile.data.account_alias

// No exemplo, acct-01 corresponde ao perfil primary e secondary ainda não tem observações.
export const sampleUsage: readonly UsageTool[] = [
  {
    harness: observedProfile.data.harness,
    nextAccount: secondary,
    profiles: [
      { alias: primary, quota: usageRecord.quota_window_after },
      { alias: secondary, quota: null },
    ],
    proposal: routingProposal.data,
    taskBudget: {
      task: task.task_id,
      remainingUsd: Math.max(0, task.budget.usd - usageRecord.cost_billable_usd),
    },
  },
]
