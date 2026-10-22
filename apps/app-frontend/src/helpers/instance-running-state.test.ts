import assert from 'node:assert/strict'
import test from 'node:test'

import { updateRunningInstanceIds } from './instance-running-state.ts'

test('tracks process lifecycle events for menu state', () => {
    const started = updateRunningInstanceIds(new Set(), { instance_id: 'a', event: 'launched' })
    assert.equal(started.has('a'), true)

    const finished = updateRunningInstanceIds(started, { instance_id: 'a', event: 'finished' })
    assert.equal(finished.has('a'), false)
})
