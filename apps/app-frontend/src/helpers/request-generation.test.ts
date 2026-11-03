import { expect, it } from 'vitest'

import { createRequestGeneration } from './request-generation.ts'

it('only the latest request for the current key can publish', () => {
    const requests = createRequestGeneration()
    const first = requests.begin('instance-a:/')
    const second = requests.begin('instance-a:mods')

    expect(requests.isCurrent(first, 'instance-a:/')).toBe(false)
    expect(requests.isCurrent(second, 'instance-a:mods')).toBe(true)
    expect(requests.isCurrent(second, 'instance-a:/')).toBe(false)
})

it('invalidating a workspace makes an in-flight request stale', () => {
    const requests = createRequestGeneration()
    const token = requests.begin('server-a:/')
    requests.invalidate()

    expect(requests.isCurrent(token, 'server-a:/')).toBe(false)
})

it('a delayed instance event cannot replace a newer read of the same instance', async () => {
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
    expect(displayed).toBe('new state')
})
