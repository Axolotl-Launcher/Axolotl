import assert from 'node:assert/strict'
import test from 'node:test'

import { errorMessage, errorText } from './error-text.ts'
import { multiplayerErrorText } from './multiplayer.ts'

test('reads the message out of a serialized Tauri error', () => {
    assert.equal(
        errorText({ field_name: 'Theseus', message: 'cache unavailable' }),
        'cache unavailable',
    )
})

test('reads Error instances and passes plain values through', () => {
    assert.equal(errorText(new Error('cache unavailable')), 'cache unavailable')
    assert.equal(errorText('cache unavailable'), 'cache unavailable')
    assert.equal(errorText(42), '42')
})

test('shares one implementation with the existing multiplayer helper', () => {
    const serialized = { field_name: 'Theseus', message: 'RedStone operation cancelled' }
    assert.equal(errorText(serialized), multiplayerErrorText(serialized))
    assert.equal(errorText('cancelled'), multiplayerErrorText('cancelled'))
})

test('reports no message when the value has none, or a non-string one', () => {
    // The nullable form is what pattern-matching callers read, so it must not
    // turn a missing message into the string "undefined" or "[object Object]".
    assert.equal(errorMessage({ field_name: 'Theseus' }), null)
    assert.equal(errorMessage({ message: 42 }), null)
    assert.equal(errorMessage({ message: null }), null)
    assert.equal(errorMessage('plain text'), null)

    // The formatting form still returns something readable for those values.
    assert.equal(errorText({ field_name: 'Theseus' }), '[object Object]')
    assert.equal(errorText({ message: 42 }), '[object Object]')
})

test('never throws on a value that cannot be inspected', () => {
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
    assert.equal(errorMessage(hostile), null)
    assert.equal(errorText(hostile), 'Unknown error')
})
