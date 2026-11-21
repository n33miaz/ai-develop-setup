import { fireEvent, render, screen, within } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import recorded from '../../../contracts/golden/command.approval.list.response.v3/valid/02-with-effects.json'
import refused from '../../../contracts/golden/command.approval.list.response.v3/valid/04-refused-with-more.json'
import unreadable from '../../../contracts/golden/command.approval.list.response.v3/valid/05-empty-with-unreadable.json'
import queued from '../../../contracts/golden/command.queue.get.response.v2/valid/01-recorded-policy.json'
import {
  HARNESS_NOT_CONFIGURED,
  QUEUE_NOT_CONFIGURED,
} from '../../../contracts/generated/ts/contract.local-api.error-code.v4.constants'
import { CommandRefusal, type queueApprovals } from '../api/queueApprovals'
import { isApprovalList, isQueueGet } from '../api/wire'
import { ApprovalsPanel } from './ApprovalsPanel'
import { QueuePanel } from './QueuePanel'

type Api = ReturnType<typeof queueApprovals>

function mockApi() {
  return {
    get: vi.fn<Api['get']>(),
    create: vi.fn<Api['create']>(),
    start: vi.fn<Api['start']>(),
    stop: vi.fn<Api['stop']>(),
    list: vi.fn<Api['list']>(),
    answer: vi.fn<Api['answer']>(),
  }
}

function open(name: string) {
  const details = screen.getByText(name).closest('details')
  if (!details) throw new Error('missing panel')
  details.open = true
  return within(details)
}

it('creates, starts and stops a queue using the reported state', async () => {
  const api = mockApi()
  api.get
    .mockRejectedValueOnce(new CommandRefusal(QUEUE_NOT_CONFIGURED))
    .mockResolvedValueOnce({ queue_id: 'queue-01', state: 'stopped', policy: null })
    .mockResolvedValueOnce({ queue_id: 'queue-01', state: 'enabled', policy: null })
    .mockResolvedValueOnce({ queue_id: 'queue-01', state: 'stopped', policy: null })
  render(<QueuePanel api={api} />)
  const panel = open('Fila de tarefas')
  expect(panel.queryByRole('button', { name: 'Criar fila' })).toBeNull()
  fireEvent.change(panel.getByLabelText('ID da fila'), { target: { value: 'queue-01' } })
  fireEvent.click(panel.getByRole('button', { name: 'Consultar' }))
  expect(panel.getByRole('status')).toHaveTextContent('Consultando')
  fireEvent.click(await panel.findByRole('button', { name: 'Criar fila' }))
  fireEvent.click(await panel.findByRole('button', { name: 'Ligar fila' }))
  fireEvent.click(await panel.findByRole('button', { name: 'Parar fila' }))
  expect(await panel.findByText('Fila parada.')).toBeVisible()
  expect(panel.getByText('A fila não tem limites registrados no histórico.')).toBeVisible()
  expect(api.create).toHaveBeenCalledWith('queue-01')
  expect(api.start).toHaveBeenCalledWith('queue-01')
  expect(api.stop).toHaveBeenCalledWith('queue-01')
})

it('shows a failed query and a specific executor refusal', async () => {
  const api = mockApi()
  api.get
    .mockRejectedValueOnce(new Error('offline'))
    .mockResolvedValueOnce({ queue_id: 'queue-01', state: 'stopped', policy: null })
  api.start.mockRejectedValueOnce(new CommandRefusal(HARNESS_NOT_CONFIGURED))
  render(<QueuePanel api={api} />)
  const panel = open('Fila de tarefas')
  fireEvent.change(panel.getByLabelText('ID da fila'), { target: { value: 'queue-01' } })
  fireEvent.click(panel.getByRole('button', { name: 'Consultar' }))
  expect(await panel.findByRole('alert')).toHaveTextContent('Não foi possível')
  fireEvent.click(panel.getByRole('button', { name: 'Consultar' }))
  fireEvent.click(await panel.findByRole('button', { name: 'Ligar fila' }))
  expect(await panel.findByRole('alert')).toHaveTextContent('executor não está configurado')
})

it('shows the recorded effects and requires confirmation before answering', async () => {
  const api = mockApi()
  if (!isApprovalList(recorded)) throw new Error('invalid recorded fixture')
  const page = recorded.result
  const item = page.approvals[0]
  if (!item) throw new Error('missing recorded approval')
  api.list
    .mockResolvedValueOnce(page)
    .mockResolvedValueOnce({ approvals: [], more: false, unreadable: 0 })
  render(<ApprovalsPanel api={api} />)
  const panel = open('Aprovações')
  fireEvent.click(panel.getByRole('button', { name: 'Consultar' }))
  expect(panel.getByRole('status')).toHaveTextContent('Consultando')
  fireEvent.click(await panel.findByRole('button', { name: 'Aprovar' }))
  expect(panel.getByText('items/build.log')).toBeVisible()
  expect(api.answer).not.toHaveBeenCalled()
  fireEvent.click(panel.getByRole('button', { name: 'Cancelar' }))
  fireEvent.click(panel.getByRole('button', { name: 'Aprovar' }))
  fireEvent.click(panel.getByRole('button', { name: 'Confirmar aprovação' }))
  expect(await panel.findByText('Nenhuma aprovação pendente.')).toBeVisible()
  expect(panel.getByText('Alteração registrada.')).toBeVisible()
  expect(api.answer).toHaveBeenCalledWith({
    action_id: item.action_id,
    approval_id: item.approval_id,
    plan_hash: item.plan_hash,
    decision: 'approve',
  })
})

it('marks omitted and unapprovable items while allowing denial', async () => {
  const api = mockApi()
  if (!isApprovalList(refused)) throw new Error('invalid refused fixture')
  api.list.mockResolvedValueOnce(refused.result)
  api.answer.mockRejectedValueOnce(new CommandRefusal(123))
  render(<ApprovalsPanel api={api} />)
  const panel = open('Aprovações')
  fireEvent.click(panel.getByRole('button', { name: 'Consultar' }))
  expect(await panel.findByText(/Há mais itens pendentes/)).toBeVisible()
  expect(panel.getByText('O ensaio foi recusado.')).toBeVisible()
  expect(panel.getByRole('button', { name: 'Aprovar' })).toBeDisabled()
  fireEvent.click(panel.getByRole('button', { name: 'Negar' }))
  expect(await panel.findByRole('alert')).toHaveTextContent('O núcleo recusou')
  expect(api.answer).toHaveBeenCalledWith(expect.objectContaining({ decision: 'deny' }))
})

it('shows the limits recorded for the queue', async () => {
  const api = mockApi()
  if (!isQueueGet(queued)) throw new Error('invalid queue fixture')
  api.get.mockResolvedValueOnce(queued.result)
  render(<QueuePanel api={api} />)
  const panel = open('Fila de tarefas')
  fireEvent.change(panel.getByLabelText('ID da fila'), {
    target: { value: queued.result.queue_id },
  })
  fireEvent.click(panel.getByRole('button', { name: 'Consultar' }))
  expect(await panel.findByText('Limites registrados')).toBeVisible()
  // The fixture records 2 attempts, a window of 3600 s and 60 s for the host.
  expect(panel.getByText('Tentativas por tarefa: 2')).toBeVisible()
  expect(panel.getByText('Janela de admissão: 3600 s')).toBeVisible()
  expect(panel.getByText('Prazo de confirmação da sessão pelo host: 60 s')).toBeVisible()
  expect(panel.getAllByRole('listitem')).toHaveLength(9)
  expect(panel.queryByText('A fila não tem limites registrados no histórico.')).toBeNull()
})

it('tells the owner how many unreadable requests were left out', async () => {
  const api = mockApi()
  if (!isApprovalList(unreadable)) throw new Error('invalid unreadable fixture')
  api.list.mockResolvedValueOnce(unreadable.result)
  render(<ApprovalsPanel api={api} />)
  const panel = open('Aprovações')
  fireEvent.click(panel.getByRole('button', { name: 'Consultar' }))
  expect(await panel.findByText(/Pedidos com histórico ilegível fora da lista: 2\./)).toBeVisible()
  expect(panel.queryByText('Nenhuma aprovação pendente.')).toBeNull()
})
