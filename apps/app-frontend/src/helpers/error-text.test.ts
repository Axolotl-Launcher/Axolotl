import assert from 'node:assert/strict'
import test from 'node:test'

import { errorText } from './error-text.ts'
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
