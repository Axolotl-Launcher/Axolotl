import assert from 'node:assert/strict'
import test from 'node:test'

import { isVersionInRange, parseVersionRange } from './version-compatibility.ts'

test('parses a minor-pinned wildcard range', () => {
    assert.deepEqual(parseVersionRange('1.20.x'), {
        type: 'wildcard',
        major: 1,
        minor: 20,
        minorPinned: true,
    })
})

test('parses a major-only wildcard range', () => {
    assert.deepEqual(parseVersionRange('1.x'), {
        type: 'wildcard',
        major: 1,
        minor: 0,
        minorPinned: false,
    })
})

test('A.B.x only matches versions inside A.B', () => {
    assert.equal(isVersionInRange('1.20.4', '1.20.x'), true)
    assert.equal(isVersionInRange('1.20', '1.20.x'), true)
    assert.equal(isVersionInRange('1.20.4', '1.20.X'), true)
    assert.equal(isVersionInRange('1.12', '1.20.x'), false)
    assert.equal(isVersionInRange('1.21.0', '1.20.x'), false)
    assert.equal(isVersionInRange('2.20.1', '1.20.x'), false)
})

test('A.x still matches every version inside A', () => {
    assert.equal(isVersionInRange('1.0.0', '1.x'), true)
    assert.equal(isVersionInRange('1.20.4', '1.x'), true)
    assert.equal(isVersionInRange('2.0.0', '1.x'), false)
})

test('other range forms keep their behaviour', () => {
    assert.equal(isVersionInRange('1.20.4', '1.20.4'), true)
    assert.equal(isVersionInRange('1.20.3', '1.20.4'), false)
    assert.equal(isVersionInRange('1.20.4', '>=1.20.4'), true)
    assert.equal(isVersionInRange('1.12', '>=1.20'), false)
    assert.equal(isVersionInRange('1.12', '<=1.20'), true)
    assert.equal(isVersionInRange('1.19.4', '[1.19,1.20)'), true)
    assert.equal(isVersionInRange('1.20', '[1.19,1.20)'), false)
    assert.equal(isVersionInRange('nonsense', '1.20.x'), false)
    assert.equal(isVersionInRange('1.20.4', 'nonsense'), false)
})
