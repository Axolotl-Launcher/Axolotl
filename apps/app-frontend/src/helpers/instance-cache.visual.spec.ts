import { expect, it } from 'vitest'

import { readInstanceCache, writeInstanceCache } from './instance-cache'

it('removes old content while keeping independent UI preferences even for an empty instance', () => {
    localStorage.setItem(
        'instance:test-refresh:data',
        JSON.stringify({ contentItems: [{ id: 'stale' }] }),
    )
    localStorage.setItem('instance:test-refresh:ui', JSON.stringify({ modpackHintDismissed: true }))
    expect(readInstanceCache('test-refresh')?.modpackHintDismissed).toBe(true)
    expect(localStorage.getItem('instance:test-refresh:data')).toBeNull()
    writeInstanceCache('test-refresh', { modpackHintDismissed: false })
    expect(readInstanceCache('test-refresh')?.modpackHintDismissed).toBe(false)
    localStorage.removeItem('instance:test-refresh:ui')
})
