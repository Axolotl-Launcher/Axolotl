import { expect, it } from 'vitest'

import { createAntiPiracyStatus, isOfflineAccountRestrictedError } from './useAntiPiracyStatus.ts'

it('a late region event cannot undo a cleared official login record', () => {
    const { status, setStatus } = createAntiPiracyStatus()
    setStatus({ region: 'non_cn', restricted: false, revision: 1 })
    setStatus({ region: 'non_cn', restricted: true, revision: 2 })
    setStatus({ region: 'non_cn', restricted: false, revision: 1 })
    expect(status.value.restricted).toBe(true)
    expect(status.value.revision).toBe(2)
})

it('a late eligibility read cannot undo a successful official login', () => {
    const { status, setStatus } = createAntiPiracyStatus()
    setStatus({ region: 'non_cn', restricted: true, revision: 1 })
    setStatus({ region: 'non_cn', restricted: false, revision: 2 })
    setStatus({ region: 'non_cn', restricted: true, revision: 1 })
    expect(status.value.restricted).toBe(false)
})

it('restricted backend errors are recognized in Tauri and Error forms', () => {
    expect(isOfflineAccountRestrictedError('Input error: OFFLINE_ACCOUNT_RESTRICTED')).toBe(true)
    expect(isOfflineAccountRestrictedError(new Error('OFFLINE_ACCOUNT_RESTRICTED'))).toBe(true)
    expect(isOfflineAccountRestrictedError({ message: 'OFFLINE_ACCOUNT_RESTRICTED' })).toBe(true)
    expect(isOfflineAccountRestrictedError(undefined)).toBe(false)
    expect(isOfflineAccountRestrictedError(new Error('Other failure'))).toBe(false)
})
