import { expect, it } from 'vitest'

import { errorMessage, errorText } from './error-text.ts'
import { multiplayerErrorText } from './multiplayer.ts'

it('reads the message out of a serialized Tauri error', () => {
    expect(errorText({ field_name: 'Theseus', message: 'cache unavailable' })).toBe(
        'cache unavailable',
    )
})

it('reads Error instances and passes plain values through', () => {
    expect(errorText(new Error('cache unavailable'))).toBe('cache unavailable')
    expect(errorText('cache unavailable')).toBe('cache unavailable')
    expect(errorText(42)).toBe('42')
    expect(errorText(false)).toBe('false')
})

it('shares one implementation with the existing multiplayer helper', () => {
    const serialized = { field_name: 'Theseus', message: 'RedStone operation cancelled' }
    expect(errorText(serialized)).toBe(multiplayerErrorText(serialized))
    expect(errorText('cancelled')).toBe(multiplayerErrorText('cancelled'))
})

it('reports no message when the value has none, or a non-string one', () => {
    // The nullable form is what pattern-matching callers read, so it must not
    // turn a missing message into the string "undefined" or "[object Object]".
    expect(errorMessage({ field_name: 'Theseus' })).toBeNull()
    expect(errorMessage({ message: 42 })).toBeNull()
    expect(errorMessage({ message: null })).toBeNull()
    expect(errorMessage('plain text')).toBeNull()

    expect(errorText({ field_name: 'Theseus' })).toBe('{"field_name":"Theseus"}')
    expect(errorText({ message: 42 })).toBe('{"message":42}')
})

it('preserves file failure details in objects and arrays without a message', () => {
    const failure = { path: 'downloads/pack.zip', reason: 'permission denied' }
    expect(errorText(failure)).toBe('{"path":"downloads/pack.zip","reason":"permission denied"}')
    expect(errorText([failure])).toBe(
        '[{"path":"downloads/pack.zip","reason":"permission denied"}]',
    )
})

it('returns the localized fallback for absent or unserializable errors', () => {
    const cycle: { file: string; cause?: unknown } = { file: 'cache.db' }
    cycle.cause = cycle
    for (const error of [null, undefined, cycle, { size: 1n }]) {
        expect(errorText(error, '未知错误')).toBe('未知错误')
    }
})

it('keeps a string result when an object has no JSON representation', () => {
    expect(errorText({ toJSON: () => undefined }, '未知错误')).toBe('未知错误')
    expect(multiplayerErrorText({ toJSON: () => undefined }).includes('cancelled')).toBe(false)
})

it('never throws on a value that cannot be inspected', () => {
    // A rejected IPC value is normally a plain object, but this helper runs
    // inside catch blocks: throwing again there would hide the original failure.
    const hostile = new Proxy(
        {},
        {
            has() {
                throw new Error('has trap')
            },
            get() {
                throw new Error('get trap')
            },
            getPrototypeOf() {
                throw new Error('getPrototypeOf trap')
            },
        },
    )
    expect(errorMessage(hostile)).toBeNull()
    expect(errorText(hostile)).toBe('Unknown error')
})

it('never throws when reading the message property itself throws', () => {
    // `has` succeeds so the read proceeds; only the `get` trap throws. This is a
    // separate branch from the test above, where `has` rejects first.
    const hostile = new Proxy(
        {},
        {
            has: () => true,
            get() {
                throw new Error('get trap')
            },
        },
    )
    expect(errorMessage(hostile)).toBeNull()
    expect(errorText(hostile)).toBe('Unknown error')
})

it('uses the caller-provided fallback when object serialization throws', () => {
    const hostile = {
        toJSON() {
            throw new Error('serialization rejected')
        },
    }
    expect(errorMessage(hostile)).toBeNull()
    expect(errorText(hostile, '未知错误')).toBe('未知错误')
})
