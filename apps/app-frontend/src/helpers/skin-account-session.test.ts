import { expect, it } from 'vitest'

import { createSkinAccountSession } from './skin-account-session.ts'

it('invalidates skin results from the previous account session', () => {
    const sessions = createSkinAccountSession()
    const accountA = sessions.begin()
    const accountB = sessions.begin()

    expect(sessions.isCurrent(accountA)).toBe(false)
    expect(sessions.isCurrent(accountB)).toBe(true)
})
