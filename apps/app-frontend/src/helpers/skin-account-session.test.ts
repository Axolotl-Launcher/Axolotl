import assert from 'node:assert/strict'
import test from 'node:test'

import { createSkinAccountSession } from './skin-account-session.ts'

test('invalidates skin results from the previous account session', () => {
    const sessions = createSkinAccountSession()
    const accountA = sessions.begin()
    const accountB = sessions.begin()

    assert.equal(sessions.isCurrent(accountA), false)
    assert.equal(sessions.isCurrent(accountB), true)
})
