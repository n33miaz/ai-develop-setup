import { expect, it } from 'vitest'
import queueGet from '../../../contracts/golden/contract.local-api.server-message.v7/valid/05-queue-get.json'
import legacyQueue from '../../../contracts/golden/contract.local-api.server-message.v7/valid/07-legacy-queue.json'
import approvalList from '../../../contracts/golden/contract.local-api.server-message.v7/valid/06-approval-list.json'
import { isApprovalList, isQueueGet, isServerMessage } from './wire'

it('accepts the queue and approval replies the daemon writes', () => {
  for (const reply of [queueGet, legacyQueue]) {
    expect(isServerMessage(reply)).toBe(true)
    expect(isQueueGet(reply)).toBe(true)
  }
  expect(isServerMessage(approvalList)).toBe(true)
  expect(isApprovalList(approvalList)).toBe(true)
})
