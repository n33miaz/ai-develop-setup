import type { CommandApprovalAnswerV1 } from '../../../contracts/generated/ts/command.approval.answer.v1'
import type { CommandApprovalListV1 } from '../../../contracts/generated/ts/command.approval.list.v1'
import type { CommandQueueConfiguredV1 } from '../../../contracts/generated/ts/command.queue.configured.v1'
import type { CommandQueueCreateV1 } from '../../../contracts/generated/ts/command.queue.create.v1'
import type { CommandQueueGetV1 } from '../../../contracts/generated/ts/command.queue.get.v1'
import type { CommandQueueStartV1 } from '../../../contracts/generated/ts/command.queue.start.v1'
import type { CommandQueueStopV1 } from '../../../contracts/generated/ts/command.queue.stop.v1'
import type { CoreClient } from './client'
import { newCommand } from './command'
import { isApprovalList, isQueueConfigured, isQueueGet } from './wire'

export class CommandRefusal extends Error {
  readonly code: number

  constructor(code: number) {
    super('command rejected')
    this.code = code
  }
}

type Request =
  | CommandQueueGetV1
  | CommandQueueConfiguredV1
  | CommandQueueCreateV1
  | CommandQueueStartV1
  | CommandQueueStopV1
  | CommandApprovalListV1
  | CommandApprovalAnswerV1

type Params<Method extends Request['method']> = Omit<
  Extract<Request, { method: Method }>['params'],
  '_meta'
>

export function queueApprovals(client: Pick<CoreClient, 'send'>) {
  async function request<Method extends Request['method']>(method: Method, params: Params<Method>) {
    const command = newCommand(method, params)
    const response = await client.send(command)
    if (response.id !== command.id) throw new Error('unexpected response id')
    if (response.error) throw new CommandRefusal(response.error.code)
    if (
      method !== 'queue.get' &&
      method !== 'queue.configured' &&
      method !== 'approval.list' &&
      (!response.result || !('accepted' in response.result))
    ) {
      throw new Error('invalid command acknowledgement')
    }
    return response
  }

  return {
    async get(queue_id: Params<'queue.get'>['queue_id']) {
      const response = await request('queue.get', { queue_id })
      if (!isQueueGet(response) || response.result.queue_id !== queue_id) {
        throw new Error('invalid queue response')
      }
      return response.result
    },
    /** O id da fila que o núcleo serve, criada ou não. */
    async configured() {
      const response = await request('queue.configured', {})
      if (!isQueueConfigured(response)) throw new Error('invalid queue response')
      return response.result.queue_id
    },
    create: (queue_id: Params<'queue.create'>['queue_id']) => request('queue.create', { queue_id }),
    start: (queue_id: Params<'queue.start'>['queue_id']) => request('queue.start', { queue_id }),
    stop: (queue_id: Params<'queue.stop'>['queue_id']) => request('queue.stop', { queue_id }),
    async list() {
      const response = await request('approval.list', {})
      if (!isApprovalList(response)) throw new Error('invalid approvals response')
      return response.result
    },
    answer: (params: Params<'approval.answer'>) => request('approval.answer', params),
  }
}
