import { expect, it } from 'vitest'

import { preferredOnlineAccountId } from './account-selection.ts'

const accounts = [
    { account_id: 'offline', account_type: 'offline' },
    { account_id: 'microsoft', account_type: 'microsoft' },
] as const

it('switches from an offline account to the first online account', () => {
    expect(preferredOnlineAccountId('offline', accounts)).toBe('microsoft')
})

it('keeps an online selection and handles an unknown selection', () => {
    expect(preferredOnlineAccountId('microsoft', accounts)).toBe('microsoft')
    expect(preferredOnlineAccountId('missing', accounts)).toBe('missing')
})
