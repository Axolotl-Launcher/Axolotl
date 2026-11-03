import { expect, it } from 'vitest'

import { createCrashModalSession } from './crash-modal-session.ts'

it('warning and delayed process-finished notifications share the visible diagnostic session', () => {
    const session = createCrashModalSession()
    expect(session.open('instance', 'run-1')).toBe(true)
    expect(session.open('instance', 'run-1')).toBe(false)
    expect(session.open('instance')).toBe(false)
    expect(session.open('other-instance', 'other-run')).toBe(true)
    session.close()
    expect(session.open('instance', 'run-1')).toBe(true)
    expect(session.open('instance', 'run-2')).toBe(true)
})

it('a warning without process metadata cannot be replaced by its later process notification', () => {
    const session = createCrashModalSession()
    expect(session.open('instance')).toBe(true)
    expect(session.open('instance', 'run-1')).toBe(false)
    session.launched('instance', 'run-2')
    expect(session.open('instance')).toBe(true)
    expect(session.open('instance', 'run-2')).toBe(false)
    session.launched('instance', 'run-2')
    expect(session.open('instance')).toBe(false)
})
