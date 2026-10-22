import assert from 'node:assert/strict'
import { test } from 'node:test'

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

test('live model text updates before completion, tool notes stay in progress, and late deltas are ignored after cancel', async () => {
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
    assert.equal(agent.output.value, 'Checking Java.')
    progress({ event_type: 'model_end', data: { content: 'Checking Java.', has_tools: true } })
    assert.equal(agent.output.value, '')
    assert.equal(agent.activity.value[1].text, 'Checking Java.')
    progress({ event_type: 'model_start', data: { turn: 2 } })
    progress({ event_type: 'delta', data: { content: 'The cause is' } })
    agent.cancel()
    progress({ event_type: 'delta', data: { content: ' stale text' } })
    assert.equal(agent.output.value, 'The cause is')
    assert.equal(agent.activity.value.at(-1)?.state, 'error')
    finish.resolve('old result')
    await pending
    assert.equal(agent.status.value, 'cancelled')
    assert.equal(agent.output.value, 'The cause is')
    scope.stop()
})

test('cancelled preparation cannot start cloud work or overwrite a replacement run', async () => {
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
    assert.equal(clouds, 0)
    assert.equal(agent.output.value, 'new local result')
    assert.equal(agent.status.value, 'ready')
    scope.stop()
})

test('cloud events are run-specific, errors can retry, local has no cloud fallback', async () => {
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
    assert.equal(agent.status.value, 'error')
    assert.equal(cloudCalls, 0)
    const pending = agent.start('instance', 'logshare', null, () => {})
    await listening.promise
    receive!({ run_id: runId, event_type: 'queued', data: { position: 2 } })
    assert.equal(agent.event.value?.event_type, 'queued')
    receive!({ run_id: runId, event_type: 'thinking', data: { content: 'private' } })
    assert.equal(agent.event.value?.event_type, 'thinking')
    assert.deepEqual(agent.event.value?.data, {})
    receive!({ run_id: runId, event_type: 'queued', data: { position: 1 } })
    receive!({ run_id: 'old-run', event_type: 'delta', data: { content: 'wrong' } })
    receive!({ run_id: runId, event_type: 'delta', data: { content: 'right' } })
    assert.equal(agent.event.value, null)
    assert.equal(agent.output.value, 'right')
    cloud.resolve({ content: 'complete', ticket: null })
    await pending
    assert.equal(agent.status.value, 'ready')
    assert.equal(agent.output.value, 'complete')
    scope.stop()
})

test('reuses an in-flight summary upload and cancellation prevents the cloud request', async () => {
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
                assert.equal(ticket?.id, 'existing-upload')
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
    assert.equal(agent.busy.value, true)
    agent.cancel()
    upload.resolve({ id: 'existing-upload', url: '', raw: '', token: '' })
    await first
    assert.equal(clouds, 0)
    await agent.start('instance', 'logshare', upload.promise, () => {})
    assert.equal(clouds, 1)
    scope.stop()
})
