import assert from 'node:assert/strict'
import test from 'node:test'

import { createHomeRefresh, homeWorldsChanged } from './home-refresh.ts'

test('bursts share one read, with one trailing read for changes arriving in flight', async () => {
    let calls = 0
    let finish!: () => void
    const refresh = createHomeRefresh(
        async () => {
            calls++
            if (calls === 1)
                await new Promise<void>((resolve) => {
                    finish = resolve
                })
        },
        () => false,
    )
    const pending = refresh()
    for (let i = 0; i < 100; i++) assert.equal(refresh(), pending)
    await new Promise((resolve) => setTimeout(resolve, 150))
    assert.equal(calls, 1)
    for (let i = 0; i < 100; i++) void refresh()
    finish()
    await pending
    assert.equal(calls, 2)
})

test('disposed pages do not dispatch queued reads', async () => {
    let disposed = false
    let calls = 0
    const refresh = createHomeRefresh(
        async () => {
            calls++
        },
        () => disposed,
    )
    const pending = refresh()
    disposed = true
    await pending
    await refresh()
    assert.equal(calls, 0)
})

test('world refresh excludes content, screenshots and generic sync noise', () => {
    for (const event of ['content_changed', 'screenshots_updated', 'synced'])
        assert.equal(homeWorldsChanged(event), false)
    for (const event of [
        'created',
        'removed',
        'edited',
        'world_updated',
        'server_joined',
        'servers_updated',
    ])
        assert.equal(homeWorldsChanged(event), true)
})
