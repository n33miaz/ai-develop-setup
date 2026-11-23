import { expect, it } from 'vitest'
import queueGet from '../../../contracts/golden/contract.local-api.server-message.v10/valid/05-queue-get.json'
import approvalList from '../../../contracts/golden/contract.local-api.server-message.v10/valid/06-approval-list.json'
import legacyQueue from '../../../contracts/golden/contract.local-api.server-message.v10/valid/07-legacy-queue.json'
import unavailableWorkdir from '../../../contracts/golden/contract.local-api.server-message.v10/valid/08-unavailable-workdir.json'
import queueConfigured from '../../../contracts/golden/contract.local-api.server-message.v10/valid/09-queue-configured.json'
import {
  isApprovalList,
  isHarnessList,
  isQueueConfigured,
  isQueueGet,
  isServerMessage,
} from './wire'

it('accepts the queue and approval replies the daemon writes', () => {
  for (const reply of [queueGet, legacyQueue]) {
    expect(isServerMessage(reply)).toBe(true)
    expect(isQueueGet(reply)).toBe(true)
  }
  expect(isServerMessage(approvalList)).toBe(true)
  expect(isApprovalList(approvalList)).toBe(true)
})

it('accepts configured queue and missing work folder replies', () => {
  expect(isServerMessage(queueConfigured)).toBe(true)
  expect(isQueueConfigured(queueConfigured)).toBe(true)
  expect(isServerMessage(unavailableWorkdir)).toBe(true)
  expect(isHarnessList(unavailableWorkdir)).toBe(true)
})
