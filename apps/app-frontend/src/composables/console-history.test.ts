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

it('retains the longest overlap when repeated prefixes fall back more than once', () => {
    expect(mergeLiveLogHistory('before\na\nb\na\nb\na\na\nb\na', 'a\nb\na\nb\na\nc')).toBe(
        'before\na\nb\na\nb\na\na\nb\na\nb\na\nc',
    )
})

it('keeps all live lines after a long repeated prefix does not overlap', () => {
    const repeated = 'repeated\n'.repeat(9_999)
    const latest = `${repeated}latest-only`
    const live = `${repeated}repeated`
    expect(mergeLiveLogHistory(latest, live)).toBe(`${latest}\n${live}`)
})
