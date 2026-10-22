import assert from 'node:assert/strict'
import test from 'node:test'

import { createRequestGeneration } from './request-generation.ts'

test('only the latest request for the current key can publish', () => {
    const requests = createRequestGeneration()
    const first = requests.begin('instance-a:/')
    const second = requests.begin('instance-a:mods')

    assert.equal(requests.isCurrent(first, 'instance-a:/'), false)
    assert.equal(requests.isCurrent(second, 'instance-a:mods'), true)
    assert.equal(requests.isCurrent(second, 'instance-a:/'), false)
})

test('invalidating a workspace makes an in-flight request stale', () => {
    const requests = createRequestGeneration()
    const token = requests.begin('server-a:/')
    requests.invalidate()

    assert.equal(requests.isCurrent(token, 'server-a:/'), false)
})

test('a delayed instance event cannot replace a newer read of the same instance', async () => {
    const requests = createRequestGeneration()
    let finish!: (value: string) => void
    const response = new Promise<string>((resolve) => {
        finish = resolve
    })
    const old = requests.begin('instance-a')
    let displayed = 'initial'
    const pending = response.then((value) => {
        if (requests.isCurrent(old, 'instance-a')) displayed = value
    })
    const latest = requests.begin('instance-a')
    if (requests.isCurrent(latest, 'instance-a')) displayed = 'new state'
    finish('old state')
    await pending
    assert.equal(displayed, 'new state')
})
