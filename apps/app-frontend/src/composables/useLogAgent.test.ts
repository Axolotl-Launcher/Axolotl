import { expect, it } from 'vitest'
import { effectScope } from 'vue'

import type { AnalysisEvent, PreparedSession } from '../logagent/types.ts'
import { useLogAgent } from './useLogAgent.ts'

function deferred<T>() {
    let resolve!: (value: T) => void
    const promise = new Promise<T>((done) => {
        resolve = done
    })
    return { promise, resolve }
}

it('live model text updates before completion, tool notes stay in progress, and late deltas are ignored after cancel', async () => {
    const finish = deferred<string>()
    let progress!: (event: import('../logagent/types.ts').LocalProgress) => void
    const started = deferred<void>()
    const scope = effectScope()
    const agent = scope.run(() =>
        useLogAgent({
            prepare: async (_instance, id) => ({
                run_id: id,
                provider_id: 'provider',
                model_id: 'model',
                locale: '',
                context: '',
            }),
            cancel: async () => {},
            cloud: async () => {
                throw new Error('Must not use cloud')
            },
            gateway: () => ({
                modelTurn: async () => {
                    throw new Error('unused')
                },
                tool: async () => null,
            }),
            local: async (_session, _gateway, _signal, _tool, onProgress) => {
                progress = onProgress!
                started.resolve()
                return finish.promise
            },
        }),
    )!
    const pending = agent.start('instance', 'custom', null, () => {})
    await started.promise
    progress({ event_type: 'model_start', data: { turn: 1 } })
    progress({ event_type: 'delta', data: { content: 'Checking Java.' } })
    expect(agent.output.value).toBe('Checking Java.')
    progress({ event_type: 'model_end', data: { content: 'Checking Java.', has_tools: true } })
    expect(agent.output.value).toBe('')
    expect(agent.activity.value[1].text).toBe('Checking Java.')
    progress({ event_type: 'model_start', data: { turn: 2 } })
    progress({ event_type: 'delta', data: { content: 'The cause is' } })
    agent.cancel()
    progress({ event_type: 'delta', data: { content: ' stale text' } })
    expect(agent.output.value).toBe('The cause is')
    expect(agent.activity.value.at(-1)?.state).toBe('error')
    finish.resolve('old result')
    await pending
    expect(agent.status.value).toBe('cancelled')
    expect(agent.output.value).toBe('The cause is')
    scope.stop()
})

it('cancelled preparation cannot start cloud work or overwrite a replacement run', async () => {
    const old = deferred<PreparedSession>()
    let prepares = 0
    let clouds = 0
    const scope = effectScope()
    const agent = scope.run(() =>
        useLogAgent({
            prepare: async (_instance, id) =>
                ++prepares === 1
                    ? old.promise
                    : {
                          run_id: id,
                          provider_id: 'provider',
                          model_id: 'model',
                          locale: 'zh-CN',
                          context: '',
                      },
            cancel: async () => {},
            cloud: async () => {
                clouds++
                return { content: 'cloud', ticket: null }
            },
            local: async () => 'new local result',
            gateway: () => ({
                modelTurn: async () => {
                    throw new Error('unused')
                },
                tool: async () => null,
            }),
        }),
    )!
    const first = agent.start('same-instance', 'logshare', null, () => {})
    agent.cancel()
    await agent.start('same-instance', 'custom', null, () => {})
    old.resolve({ run_id: 'old', provider_id: '', model_id: '', locale: '', context: '' })
    await first
    expect(clouds).toBe(0)
    expect(agent.output.value).toBe('new local result')
    expect(agent.status.value).toBe('ready')
    scope.stop()
})

it('cloud events are run-specific, errors can retry, local has no cloud fallback', async () => {
    let receive: ((event: AnalysisEvent) => void) | undefined
    const listening = deferred<void>()
    let runId = ''
    let cloudCalls = 0
    const cloud = deferred<{ content: string; ticket: null }>()
    const scope = effectScope()
    const agent = scope.run(() =>
        useLogAgent({
            prepare: async (_instance, id) => {
                runId = id
                return {
                    run_id: id,
                    provider_id: 'provider',
                    model_id: 'model',
                    locale: '',
                    context: '',
                }
            },
            cancel: async () => {},
            cloud: async (_run, _ticket, listener) => {
                cloudCalls++
                receive = listener
                listening.resolve()
                return cloud.promise
            },
            local: async () => {
                throw new Error('model has no tool support')
            },
            gateway: () => ({
                modelTurn: async () => {
                    throw new Error('unused')
                },
                tool: async () => null,
            }),
        }),
    )!
    await agent.start('instance', 'custom', null, () => {})
    expect(agent.status.value).toBe('error')
    expect(cloudCalls).toBe(0)
    const pending = agent.start('instance', 'logshare', null, () => {})
    await listening.promise
    receive!({ run_id: runId, event_type: 'queued', data: { position: 2 } })
    expect(agent.event.value?.event_type).toBe('queued')
    receive!({ run_id: runId, event_type: 'thinking', data: { content: 'private' } })
    expect(agent.event.value?.event_type).toBe('thinking')
    expect(agent.event.value?.data).toEqual({})
    receive!({ run_id: runId, event_type: 'queued', data: { position: 1 } })
    receive!({ run_id: 'old-run', event_type: 'delta', data: { content: 'wrong' } })
    receive!({ run_id: runId, event_type: 'delta', data: { content: 'right' } })
    expect(agent.event.value).toBe(null)
    expect(agent.output.value).toBe('right')
    cloud.resolve({ content: 'complete', ticket: null })
    await pending
    expect(agent.status.value).toBe('ready')
    expect(agent.output.value).toBe('complete')
    scope.stop()
})

it('reuses an in-flight summary upload and cancellation prevents the cloud request', async () => {
    const upload = deferred<import('../logagent/types.ts').UploadTicket | null>()
    let clouds = 0
    const scope = effectScope()
    const agent = scope.run(() =>
        useLogAgent({
            prepare: async (_instance, id) => ({
                run_id: id,
                provider_id: '',
                model_id: '',
                locale: '',
                context: '',
            }),
            cancel: async () => {},
            cloud: async (_id, ticket) => {
                clouds++
                expect(ticket?.id).toBe('existing-upload')
                return { content: 'done', ticket }
            },
            local: async () => '',
            gateway: () => ({
                modelTurn: async () => {
                    throw new Error('unused')
                },
                tool: async () => null,
            }),
        }),
    )!
    const first = agent.start('instance', 'logshare', upload.promise, () => {})
    await Promise.resolve()
    expect(agent.busy.value).toBe(true)
    agent.cancel()
    upload.resolve({ id: 'existing-upload', url: '', raw: '', token: '' })
    await first
    expect(clouds).toBe(0)
    await agent.start('instance', 'logshare', upload.promise, () => {})
    expect(clouds).toBe(1)
    scope.stop()
})
