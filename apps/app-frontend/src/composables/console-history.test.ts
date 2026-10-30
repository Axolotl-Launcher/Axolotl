import { expect, it } from 'vitest'

import { mergeLiveLogHistory } from './console-history.ts'

it('retains the persisted latest.log history and appends a live-only tail', () => {
    expect(mergeLiveLogHistory('first\nsecond\nthird', 'second\nthird\nfourth')).toBe(
        'first\nsecond\nthird\nfourth',
    )
})

it('does not duplicate a buffer already persisted in latest.log', () => {
    expect(mergeLiveLogHistory('first\nsecond\nthird', 'second\nthird')).toBe(
        'first\nsecond\nthird',
    )
})

it('keeps live output when it is not present in latest.log', () => {
    expect(mergeLiveLogHistory('minecraft', 'launcher')).toBe('minecraft\nlauncher')
})

it('accepts an unavailable latest.log while the instance is starting', () => {
    expect(mergeLiveLogHistory('', 'launcher output')).toBe('launcher output')
})
