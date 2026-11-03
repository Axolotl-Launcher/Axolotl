import { expect, it } from 'vitest'

import { usableU16 } from './usable-u16.ts'

it('accepts integers inside the u16 window range', () => {
    expect(usableU16(1)).toBe(1)
    expect(usableU16(1280)).toBe(1280)
    expect(usableU16(65535)).toBe(65535)
})

it('rejects everything the u16 window field cannot hold', () => {
    const rejected = [
        0,
        -1,
        65536,
        1920.5,
        Number.NaN,
        Number.POSITIVE_INFINITY,
        undefined,
        null,
        '1280',
        true,
    ]

    for (const value of rejected) {
        expect(usableU16(value), `expected ${String(value)} to be rejected`).toBe(null)
    }
})
