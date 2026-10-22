import assert from 'node:assert/strict'
import { test } from 'node:test'

import { createCrashModalSession } from './crash-modal-session.ts'

test('warning and delayed process-finished notifications share the visible diagnostic session', () => {
    const session = createCrashModalSession()
    assert.equal(session.open('instance', 'run-1'), true)
    assert.equal(session.open('instance', 'run-1'), false)
    assert.equal(session.open('instance'), false)
    assert.equal(session.open('other-instance', 'other-run'), true)
    session.close()
    assert.equal(session.open('instance', 'run-1'), true)
    assert.equal(session.open('instance', 'run-2'), true)
})

test('a warning without process metadata cannot be replaced by its later process notification', () => {
    const session = createCrashModalSession()
    assert.equal(session.open('instance'), true)
    assert.equal(session.open('instance', 'run-1'), false)
    session.launched('instance', 'run-2')
    assert.equal(session.open('instance'), true)
    assert.equal(session.open('instance', 'run-2'), false)
    session.launched('instance', 'run-2')
    assert.equal(session.open('instance'), false)
})
