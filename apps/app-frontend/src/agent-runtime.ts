import { runAgent, type RuntimeGateway, type RuntimeJob } from '@axolotl/agent-runtime'
import { Channel, invoke } from '@tauri-apps/api/core'

declare global {
    interface Window {
        __AXOLOTL_AGENT_WORKER_ID__?: string
    }
}

type LeasedJob = RuntimeJob & { lease_id: string }
const workerId = window.__AXOLOTL_AGENT_WORKER_ID__
if (!workerId) throw new Error('Agent worker bootstrap is missing')

const workerInvoke = <T>(command: string, args: Record<string, unknown> = {}) =>
    invoke<T>(`plugin:agent-runtime|agent_worker_${command}`, { workerId, ...args })

function gateway(job: LeasedJob): RuntimeGateway {
    const args = { attemptId: job.attempt_id, leaseId: job.lease_id }
    return {
        catalog: () => workerInvoke('catalog', args),
        modelTurn: async (turn, emit, signal) => {
            signal?.throwIfAborted()
            const channel = new Channel<Parameters<NonNullable<typeof emit>>[0]>()
            let receiving = true
            channel.onmessage = (event) => {
                if (receiving && !signal?.aborted) emit?.(event)
            }
            try {
                return await workerInvoke('model', {
                    ...args,
                    requestId: crypto.randomUUID(),
                    turn,
                    onEvent: channel,
                })
            } finally {
                receiving = false
            }
        },
        tool: (request, signal) => {
            signal.throwIfAborted()
            return workerInvoke('tool', { ...args, request })
        },
        prepareApproval: (actions) => workerInvoke('approval', { ...args, actions }),
        emit: (event) => workerInvoke('event', { ...args, eventId: crypto.randomUUID(), event }),
        checkpoint: (request) => workerInvoke('checkpoint', { ...args, request }),
    }
}

async function main() {
    await workerInvoke('ready')
    // Key by lease, because an approved continuation may be claimed immediately
    // after its previous invocation is acknowledged, before its JS finally runs.
    const active = new Map<string, { attempt: string; controller: AbortController }>()
    while (true) {
        const poll = await workerInvoke<{ job: LeasedJob | null; cancelled: string[] }>('next')
        for (const { attempt, controller } of active.values())
            if (poll.cancelled.includes(attempt)) controller.abort(new Error('Task stopped'))
        if (!poll.job) continue
        const job = poll.job
        const controller = new AbortController()
        active.set(job.lease_id, { attempt: job.attempt_id, controller })
        void (async () => {
            try {
                const outcome = await runAgent(job, gateway(job), controller.signal)
                await workerInvoke('complete', {
                    attemptId: job.attempt_id,
                    leaseId: job.lease_id,
                    outcome,
                })
            } catch (error) {
                if (!controller.signal.aborted) {
                    await workerInvoke('complete', {
                        attemptId: job.attempt_id,
                        leaseId: job.lease_id,
                        outcome: { status: 'failed', error: String(error) },
                    }).catch(console.error)
                }
            } finally {
                active.delete(job.lease_id)
            }
        })()
    }
}

void main().catch(console.error)
