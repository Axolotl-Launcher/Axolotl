import { expect, it } from 'vitest'

import { updateRunningInstanceIds } from './instance-running-state.ts'

it('tracks process lifecycle events for menu state', () => {
    const started = updateRunningInstanceIds(new Set(), { instance_id: 'a', event: 'launched' })
    expect(started.has('a')).toBe(true)

    const finished = updateRunningInstanceIds(started, { instance_id: 'a', event: 'finished' })
    expect(finished.has('a')).toBe(false)
})
