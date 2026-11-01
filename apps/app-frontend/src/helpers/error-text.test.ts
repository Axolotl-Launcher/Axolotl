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

    // The formatting form still returns something readable for those values.
    expect(errorText({ field_name: 'Theseus' })).toBe('[object Object]')
    expect(errorText({ message: 42 })).toBe('[object Object]')
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
