import { expect, it } from 'vitest'

import { createHomeRefresh, homeWorldsChanged } from './home-refresh.ts'

it('bursts share one read, with one trailing read for changes arriving in flight', async () => {
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
    for (let i = 0; i < 100; i++) expect(refresh()).toBe(pending)
    await new Promise((resolve) => setTimeout(resolve, 150))
    expect(calls).toBe(1)
    for (let i = 0; i < 100; i++) void refresh()
    finish()
    await pending
    expect(calls).toBe(2)
})

it('disposed pages do not dispatch queued reads', async () => {
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
    expect(calls).toBe(0)
})

it('world refresh excludes content, screenshots and generic sync noise', () => {
    for (const event of ['content_changed', 'screenshots_updated', 'synced'])
        expect(homeWorldsChanged(event)).toBe(false)
    for (const event of [
        'created',
        'removed',
        'edited',
        'world_updated',
        'server_joined',
        'servers_updated',
    ])
        expect(homeWorldsChanged(event)).toBe(true)
})
