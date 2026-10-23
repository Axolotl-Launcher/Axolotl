import assert from 'node:assert/strict'
import test from 'node:test'

import { errorText } from './error-text.ts'

test('reads the message of a real Error', () => {
    assert.equal(errorText(new Error('access denied')), 'access denied')
})

test('reads a serialized Tauri error object', () => {
    assert.equal(
        errorText({ field_name: 'Theseus', message: 'cache/user: access denied' }),
        'cache/user: access denied',
    )
})

test('reads a plain message object and a thrown string', () => {
    assert.equal(
        errorText({ message: 'RedStone operation cancelled' }),
        'RedStone operation cancelled',
    )
    assert.equal(errorText('RedStone operation cancelled'), 'RedStone operation cancelled')
    assert.equal(errorText('   spaced  '), 'spaced')
})

test('never renders an object as [object Object]', () => {
    assert.equal(errorText({ field_name: 'Theseus', code: 2 }), '{"field_name":"Theseus","code":2}')
    assert.doesNotMatch(errorText({ nested: { deep: true } }), /\[object Object\]/)
})

test('falls back for values with nothing readable in them', () => {
    assert.equal(errorText(null, 'fallback'), 'fallback')
    assert.equal(errorText(undefined, 'fallback'), 'fallback')
    assert.equal(errorText('', 'fallback'), 'fallback')
    assert.equal(errorText({ message: '   ' }, 'fallback'), 'fallback')
    assert.equal(errorText(0, 'fallback'), '0')

    const circular: Record<string, unknown> = {}
    circular.self = circular
    assert.equal(errorText(circular, 'fallback'), 'fallback')
})
